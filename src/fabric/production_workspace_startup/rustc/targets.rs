//! Cargo target selection from captured manifests and source paths.

use std::collections::BTreeMap;

use super::{ProductionWorkspaceStartupError, step};
use crate::analysis_context::{ContextFileInput, RustTargetKind, RustTargetSettings};

mod selections;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct BuildSelection {
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
    pub profile: Option<String>,
    pub platforms: Option<Vec<String>>,
    pub inherited_workspace: Option<Vec<u8>>,
    pub error: Option<String>,
}

impl Default for BuildSelection {
    fn default() -> Self {
        Self {
            features: Some(Vec::new()),
            default_features: Some(true),
            profile: Some("dev".into()),
            platforms: None,
            inherited_workspace: None,
            error: None,
        }
    }
}

impl BuildSelection {
    pub(super) fn processing(
        &self,
    ) -> Option<crate::fabric::processing_status::ProcessingRustBuildSelection> {
        Some(
            crate::fabric::processing_status::ProcessingRustBuildSelection {
                profile: self.profile.clone()?,
                features: self.features.clone()?,
                default_features: self.default_features?,
            },
        )
    }

    fn invalid(error: String) -> Self {
        Self {
            features: None,
            default_features: None,
            profile: None,
            platforms: None,
            inherited_workspace: None,
            error: Some(error),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct CargoTarget {
    pub manifest: Vec<u8>,
    pub package: String,
    pub target: RustTargetSettings,
    pub target_triple: Option<String>,
    pub build: BuildSelection,
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
        let selections = selections::for_manifest(files, manifest, &document);
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
                for selection in &selections {
                    targets.insert(
                        (
                            manifest.relative_path.clone(),
                            format!("{kind:?}"),
                            name.clone(),
                            selection.clone(),
                        ),
                        CargoTarget {
                            manifest: manifest.relative_path.clone(),
                            package: package["name"].as_str().expect("package name").to_owned(),
                            target_triple: None,
                            build: selection.clone(),
                            target: RustTargetSettings {
                                name: name.clone(),
                                kind,
                                crate_root: path.clone(),
                            },
                        },
                    );
                }
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
    let default_platforms = configured_platforms(files)?;
    Ok(targets
        .into_values()
        .flat_map(|target| {
            let platforms = target.build.platforms.as_ref().map_or_else(
                || default_platforms.clone(),
                |platforms| platforms.iter().cloned().map(Some).collect(),
            );
            platforms.into_iter().map(move |platform| CargoTarget {
                target_triple: platform,
                ..target.clone()
            })
        })
        .collect())
}

/// Cargo is invoked from the captured workspace root, so member-local configuration
/// does not participate. The extensionless configuration wins, matching Cargo.
fn configured_platforms(
    files: &[ContextFileInput],
) -> Result<Vec<Option<String>>, ProductionWorkspaceStartupError> {
    let config = files
        .iter()
        .find(|file| file.relative_path == b".cargo/config")
        .or_else(|| {
            files
                .iter()
                .find(|file| file.relative_path == b".cargo/config.toml")
        });
    let Some(config) = config else {
        return Ok(vec![None]);
    };
    let document: toml::Value = toml::from_str(
        std::str::from_utf8(&config.contents)
            .map_err(|error| step("rust-target-configuration", error))?,
    )
    .map_err(|error| step("rust-target-configuration", error))?;
    let Some(target) = document.get("build").and_then(|build| build.get("target")) else {
        return Ok(vec![None]);
    };
    let values = match target {
        toml::Value::String(value) => vec![value.as_str()],
        toml::Value::Array(values) => values
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    step(
                        "rust-target-configuration",
                        "build.target contains a non-string value",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(step(
                "rust-target-configuration",
                "build.target must be a string or array of strings",
            ));
        }
    };
    if values.is_empty()
        || values.len() > 1024
        || values
            .iter()
            .any(|value| value.is_empty() || value.len() > 16_384)
    {
        return Err(step(
            "rust-target-configuration",
            "build.target is empty or exceeds its selection bounds",
        ));
    }
    Ok(values
        .into_iter()
        .map(|value| Some(value.to_owned()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect())
}

pub(super) fn resolve_host(targets: Vec<CargoTarget>, host: &str) -> Vec<CargoTarget> {
    let mut selected = BTreeMap::new();
    for mut target in targets {
        if target
            .target_triple
            .as_deref()
            .is_none_or(|value| value == "host-tuple")
        {
            target.target_triple = Some(host.to_owned());
        }
        selected.insert(
            (
                target.manifest.clone(),
                target.target.name.clone(),
                format!("{:?}", target.target.kind),
                target.target_triple.clone(),
                target.build.features.clone(),
                target.build.default_features,
                target.build.profile.clone(),
                target.build.error.clone(),
            ),
            target,
        );
    }
    selected.into_values().collect()
}

pub(super) fn build_inputs(
    files: &[ContextFileInput],
) -> Result<Vec<crate::analysis_context::ContextArtifactInput>, ProductionWorkspaceStartupError> {
    let mut inputs = BTreeMap::new();
    for manifest in files.iter().filter(|file| {
        file.relative_path == b"Cargo.toml" || file.relative_path.ends_with(b"/Cargo.toml")
    }) {
        let document: toml::Value = toml::from_str(
            std::str::from_utf8(&manifest.contents)
                .map_err(|error| step("rust-build-manifest", error))?,
        )
        .map_err(|error| step("rust-build-manifest", error))?;
        let Some(package) = document.get("package") else {
            continue;
        };
        let relative = match package.get("build") {
            Some(toml::Value::Boolean(false)) => continue,
            None | Some(toml::Value::Boolean(true)) => "build.rs",
            Some(toml::Value::String(path)) => path,
            _ => return Err(step("rust-build-manifest", "invalid package build setting")),
        };
        let path = join(
            manifest
                .relative_path
                .strip_suffix(b"Cargo.toml")
                .expect("manifest suffix"),
            relative,
        )?;
        if let Some(source) = files.iter().find(|file| file.relative_path == path) {
            inputs.insert(
                source.file_id.clone(),
                crate::analysis_context::ContextArtifactInput {
                    file_id: source.file_id.clone(),
                    digest: source.digest,
                },
            );
        } else if package.get("build").is_some() {
            return Err(step(
                "rust-build-input",
                "explicit build script is not captured",
            ));
        }
    }
    Ok(inputs.into_values().collect())
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
    #[test]
    fn captured_build_inputs_follow_custom_default_and_disabled_manifest_settings() {
        let inputs = vec![
            file(
                "Cargo.toml",
                "[workspace]\nmembers = ['custom', 'default', 'disabled']\n",
            ),
            file(
                "custom/Cargo.toml",
                "[package]\nname = 'custom'\nbuild = '../shared/configure.rs'\n",
            ),
            file("shared/configure.rs", "fn main() {}"),
            file("custom/build.rs", "not selected"),
            file("default/Cargo.toml", "[package]\nname = 'default'\n"),
            file("default/build.rs", "fn main() {}"),
            file(
                "disabled/Cargo.toml",
                "[package]\nname = 'disabled'\nbuild = false\n",
            ),
            file("disabled/build.rs", "not selected"),
        ];
        let captured = build_inputs(&inputs).unwrap();
        assert_eq!(
            captured
                .iter()
                .map(|input| input.file_id.as_str())
                .collect::<Vec<_>>(),
            ["default/build.rs", "shared/configure.rs"]
        );
        assert_eq!(
            captured[0].digest,
            crate::integrity::digest_bytes(b"fn main() {}")
        );
        assert_eq!(captured[1].digest, captured[0].digest);
    }

    #[test]
    fn captured_build_inputs_reject_missing_explicit_or_escaping_scripts() {
        for setting in [
            "true",
            "'missing.rs'",
            "'../outside.rs'",
            "'/outside.rs'",
            "3",
        ] {
            let manifest = file(
                "Cargo.toml",
                &format!("[package]\nname = 'fixture'\nbuild = {setting}\n"),
            );
            assert!(build_inputs(&[manifest]).is_err(), "{setting}");
        }
        let manifest = file("Cargo.toml", "[package]\nname = 'fixture'\n");
        assert!(build_inputs(&[manifest]).unwrap().is_empty());
    }
    #[test]
    fn cargo_platform_configuration_resolves_host_alias_and_configuration_precedence() {
        let mut files = vec![
            file("Cargo.toml", "[package]\nname = 'fixture'\n"),
            file("src/lib.rs", ""),
        ];
        let host = "x86_64-unknown-linux-gnu";
        assert_eq!(
            resolve_host(discover(&files).unwrap(), host)[0]
                .target_triple
                .as_deref(),
            Some(host)
        );
        files.push(file(".cargo/config.toml", "[build]\ntarget = ['host-tuple', 'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu']\n"));
        let selected = resolve_host(discover(&files).unwrap(), host);
        assert_eq!(
            selected
                .iter()
                .map(|target| target.target_triple.as_deref().unwrap())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([host, "aarch64-unknown-linux-gnu"])
        );
        assert_eq!(selected.len(), 2);
        files.push(file(
            ".cargo/config",
            "[build]\ntarget = 'wasm32-unknown-unknown'\n",
        ));
        let selected = resolve_host(discover(&files).unwrap(), host);
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].target_triple.as_deref(),
            Some("wasm32-unknown-unknown")
        );
    }

    #[test]
    fn cargo_platform_configuration_rejects_malformed_and_empty_selections() {
        for value in ["3", "''", "[]", "['host-tuple', 1]"] {
            let files = vec![
                file("Cargo.toml", "[package]\nname = 'fixture'\n"),
                file("src/lib.rs", ""),
                file(
                    ".cargo/config.toml",
                    &format!("[build]\ntarget = {value}\n"),
                ),
            ];
            assert!(discover(&files).is_err(), "{value}");
        }
    }
}
