//! Explicit repository-owned feature/profile selections over captured Cargo inputs.

use super::{BuildSelection, ContextFileInput, ProductionWorkspaceStartupError, join, step};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    #[serde(default)]
    features: Vec<String>,
    #[serde(default = "enabled")]
    default_features: bool,
    #[serde(default = "development")]
    profile: String,
    platforms: Option<Vec<String>>,
}

const fn enabled() -> bool {
    true
}
fn development() -> String {
    "dev".into()
}

pub(super) fn for_manifest(
    files: &[ContextFileInput],
    manifest: &ContextFileInput,
    document: &toml::Value,
) -> Vec<BuildSelection> {
    let selected: Result<_, ProductionWorkspaceStartupError> = (|| {
        if let Some(value) = context_list(document, "package") {
            return Ok((Some(value.clone()), None));
        }
        let Some((workspace, contents)) = workspace_manifest(files, manifest, document)? else {
            return Ok((None, None));
        };
        Ok((
            context_list(&contents, "workspace").cloned(),
            Some(workspace.relative_path.clone()),
        ))
    })();
    let (value, workspace) = match selected {
        Ok(value) => value,
        Err(error) => return vec![BuildSelection::invalid(error.to_string())],
    };
    let Some(value) = value else {
        return vec![BuildSelection::default()];
    };
    let Some(entries) = value
        .as_array()
        .filter(|entries| !entries.is_empty() && entries.len() <= 1024)
    else {
        return vec![BuildSelection::invalid(
            "rust_contexts must be a nonempty array of at most 1024 selections".into(),
        )];
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, value)| {
            parse(value.clone(), workspace.clone()).unwrap_or_else(|error| {
                BuildSelection::invalid(format!("rust_contexts[{index}]: {error}"))
            })
        })
        .collect()
}

fn context_list<'a>(document: &'a toml::Value, scope: &str) -> Option<&'a toml::Value> {
    document
        .get(scope)?
        .get("metadata")?
        .get("codefabric")?
        .get("rust_contexts")
}

fn parse(value: toml::Value, workspace: Option<Vec<u8>>) -> Result<BuildSelection, String> {
    let mut selected: Configuration = value.try_into().map_err(|error| format!("{error}"))?;
    if selected.profile.is_empty()
        || selected.profile.len() > 16_384
        || selected.features.len() > 1024
        || selected.features.iter().map(String::len).sum::<usize>() > 65_536
        || selected
            .features
            .iter()
            .any(|value| value.is_empty() || value.len() > 16_384)
        || selected.platforms.as_ref().is_some_and(|values| {
            values.is_empty()
                || values.len() > 1024
                || values
                    .iter()
                    .any(|value| value.is_empty() || value.len() > 16_384)
        })
    {
        return Err("empty or excessive build selection".into());
    }
    selected.features.sort();
    selected.features.dedup();
    if let Some(platforms) = &mut selected.platforms {
        platforms.sort();
        platforms.dedup();
    }
    Ok(BuildSelection {
        features: Some(selected.features),
        default_features: Some(selected.default_features),
        profile: Some(selected.profile),
        platforms: selected.platforms,
        inherited_workspace: workspace,
        error: None,
    })
}

fn workspace_manifest<'a>(
    files: &'a [ContextFileInput],
    manifest: &'a ContextFileInput,
    document: &toml::Value,
) -> Result<Option<(&'a ContextFileInput, toml::Value)>, ProductionWorkspaceStartupError> {
    if document.get("workspace").is_some() {
        return Ok(Some((manifest, document.clone())));
    }
    let parent = manifest
        .relative_path
        .strip_suffix(b"Cargo.toml")
        .expect("manifest suffix");
    if let Some(workspace) = document
        .get("package")
        .and_then(|package| package.get("workspace"))
    {
        let workspace = workspace.as_str().ok_or_else(|| {
            step(
                "rust-context-configuration",
                "package.workspace must be a path",
            )
        })?;
        let path = join(parent, &format!("{workspace}/Cargo.toml"))?;
        let file = files
            .iter()
            .find(|file| file.relative_path == path)
            .ok_or_else(|| {
                step(
                    "rust-context-configuration",
                    "selected workspace manifest is not captured",
                )
            })?;
        let document = parse_manifest(file)?;
        if document.get("workspace").is_none() {
            return Err(step(
                "rust-context-configuration",
                "selected manifest is not a Cargo workspace",
            ));
        }
        return Ok(Some((file, document)));
    }
    let mut ancestors = files
        .iter()
        .filter(|file| file.relative_path != manifest.relative_path)
        .filter_map(|file| {
            file.relative_path
                .strip_suffix(b"Cargo.toml")
                .filter(|root| root.is_empty() || root.ends_with(b"/"))
                .filter(|root| manifest.relative_path.starts_with(root))
                .map(|root| (root.len(), file))
        })
        .collect::<Vec<_>>();
    ancestors.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
    for (_, file) in ancestors {
        let document = parse_manifest(file)?;
        if document.get("workspace").is_some() {
            return Ok(Some((file, document)));
        }
    }
    Ok(None)
}

fn parse_manifest(file: &ContextFileInput) -> Result<toml::Value, ProductionWorkspaceStartupError> {
    toml::from_str(
        std::str::from_utf8(&file.contents)
            .map_err(|error| step("rust-context-configuration", error))?,
    )
    .map_err(|error| step("rust-context-configuration", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::production_workspace_startup::rustc::targets::{discover, resolve_host};

    fn file(path: &str, contents: &str) -> ContextFileInput {
        ContextFileInput {
            file_id: path.to_owned(),
            relative_path: path.as_bytes().to_vec(),
            digest: crate::integrity::digest_bytes(contents.as_bytes()),
            contents: contents.as_bytes().to_vec(),
        }
    }

    #[test]
    fn cargo_context_selections_inherit_override_and_coalesce_effective_platforms() {
        let mut files = vec![
            file(
                "Cargo.toml",
                "[workspace]\nmembers=['member']\n[workspace.metadata.codefabric]\nrust_contexts=[{features=['one','one']},{features=['one'],platforms=['host-tuple','x86_64-unknown-linux-gnu']},{profile='release',default_features=false,features=['two']},{profile=3}]\n",
            ),
            file(
                "member/Cargo.toml",
                "[package]\nname='member'\nversion='0.1.0'\n",
            ),
            file("member/src/lib.rs", ""),
        ];
        let selected = resolve_host(discover(&files).unwrap(), "x86_64-unknown-linux-gnu");
        assert_eq!(selected.len(), 3);
        assert_eq!(
            selected
                .iter()
                .filter(|target| target.build.error.is_some())
                .count(),
            1
        );
        let one = selected
            .iter()
            .find(|target| target.build.features.as_deref() == Some(&["one".to_owned()]))
            .unwrap();
        assert_eq!(one.build.profile.as_deref(), Some("dev"));
        assert_eq!(one.build.default_features, Some(true));
        assert_eq!(
            one.build.inherited_workspace.as_deref(),
            Some(b"Cargo.toml".as_slice())
        );
        let two = selected
            .iter()
            .find(|target| target.build.profile.as_deref() == Some("release"))
            .unwrap();
        assert_eq!(two.build.default_features, Some(false));
        files[1] = file(
            "member/Cargo.toml",
            "[package]\nname='member'\nversion='0.1.0'\n[package.metadata.codefabric]\nrust_contexts=[{profile='checking',features=['local'],platforms=['wasm32-unknown-unknown']}]\n",
        );
        let selected = resolve_host(discover(&files).unwrap(), "x86_64-unknown-linux-gnu");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].build.profile.as_deref(), Some("checking"));
        assert_eq!(
            selected[0].build.features.as_deref(),
            Some(["local".to_owned()].as_slice())
        );
        assert_eq!(
            selected[0].target_triple.as_deref(),
            Some("wasm32-unknown-unknown")
        );
        assert_eq!(selected[0].build.inherited_workspace, None);
    }

    #[test]
    fn cargo_context_selections_keep_invalid_entries_and_missing_workspace_scope() {
        for invalid in [
            "[]",
            "3",
            "[{unknown=true}]",
            "[{platforms=[]}]",
            "[{features=[3]}]",
        ] {
            let files = vec![
                file(
                    "Cargo.toml",
                    &format!(
                        "[package]\nname='sample'\nversion='0.1.0'\n[package.metadata.codefabric]\nrust_contexts={invalid}\n"
                    ),
                ),
                file("src/lib.rs", ""),
            ];
            let selected = discover(&files).unwrap();
            assert_eq!(selected.len(), 1);
            assert!(selected[0].build.error.is_some(), "{invalid}");
            assert_eq!(selected[0].build.features, None);
            assert_eq!(selected[0].build.default_features, None);
        }
        let files = vec![
            file(
                "member/Cargo.toml",
                "[package]\nname='member'\nversion='0.1.0'\nworkspace='..'\n",
            ),
            file("member/src/lib.rs", ""),
        ];
        assert!(
            discover(&files).unwrap()[0]
                .build
                .error
                .as_deref()
                .unwrap()
                .contains("not captured")
        );
    }
}
