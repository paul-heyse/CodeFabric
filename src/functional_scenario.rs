//! Declarative materialization for behavior-first functional scenarios.
//!
//! This module owns only fixture operations and deterministic barrier receipts. Provider,
//! publication, query, and delivery execution remain in their production owners.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::functional_golden::{Scenario, ScenarioOperation};

#[derive(Debug, Error)]
pub enum ScenarioMaterializationError {
    #[error("scenario path is not a safe relative path: {0}")]
    UnsafePath(String),
    #[error("scenario precondition differs at {path}")]
    Precondition { path: String },
    #[error("scenario checkpoint exceeds its operation list: {0}")]
    InvalidCheckpoint(String),
    #[error("scenario fixture I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioDirective {
    Barrier(String),
    ProviderFault { provider: String, fault: String },
    DroppedWatchHint(String),
    ReconcileInventory,
    FlushOverlay,
    RestartDaemon,
    SourceAcl { path: String, visibility: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedCheckpoint {
    pub checkpoint: String,
    pub after_operation: usize,
    pub files: BTreeMap<String, Vec<u8>>,
    pub directives: Vec<ScenarioDirective>,
}

fn io(path: &Path, source: std::io::Error) -> ScenarioMaterializationError {
    ScenarioMaterializationError::Io {
        path: path.to_owned(),
        source,
    }
}

fn safe_path(root: &Path, relative: &str) -> Result<PathBuf, ScenarioMaterializationError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ScenarioMaterializationError::UnsafePath(
            relative.to_owned(),
        ));
    }
    Ok(root.join(path))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), ScenarioMaterializationError> {
    fs::create_dir_all(target).map_err(|source| io(target, source))?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| io(source, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io(source, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| io(&source_path, error))?;
        if file_type.is_dir() {
            copy_tree(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| io(&target_path, error))?;
        } else {
            return Err(ScenarioMaterializationError::UnsafePath(
                source_path.display().to_string(),
            ));
        }
    }
    Ok(())
}

fn snapshot_files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, ScenarioMaterializationError> {
    fn visit(
        root: &Path,
        directory: &Path,
        output: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), ScenarioMaterializationError> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| io(directory, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io(directory, error))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io(&path, error))?;
            if file_type.is_dir() {
                visit(root, &path, output)?;
            } else if file_type.is_file() {
                let relative = path.strip_prefix(root).map_err(|_| {
                    ScenarioMaterializationError::UnsafePath(path.display().to_string())
                })?;
                output.insert(
                    relative.to_string_lossy().replace('\\', "/"),
                    fs::read(&path).map_err(|error| io(&path, error))?,
                );
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn verify_previous(
    path: &Path,
    relative: &str,
    expected: &str,
) -> Result<(), ScenarioMaterializationError> {
    let actual = fs::read(path).map_err(|error| io(path, error))?;
    if actual != expected.as_bytes() {
        return Err(ScenarioMaterializationError::Precondition {
            path: relative.to_owned(),
        });
    }
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), ScenarioMaterializationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
    }
    fs::write(path, contents.as_bytes()).map_err(|error| io(path, error))
}

fn apply_operation(
    root: &Path,
    operation: &ScenarioOperation,
    directives: &mut Vec<ScenarioDirective>,
) -> Result<(), ScenarioMaterializationError> {
    match operation {
        ScenarioOperation::Barrier { name } => {
            directives.push(ScenarioDirective::Barrier(name.clone()));
        }
        ScenarioOperation::WriteFile {
            path,
            expected_previous,
            contents,
        } => {
            let target = safe_path(root, path)?;
            if let Some(expected) = expected_previous {
                verify_previous(&target, path, expected)?;
            }
            write_file(&target, contents)?;
        }
        ScenarioOperation::RemoveFile {
            path,
            expected_previous,
        } => {
            let target = safe_path(root, path)?;
            verify_previous(&target, path, expected_previous)?;
            fs::remove_file(&target).map_err(|error| io(&target, error))?;
        }
        ScenarioOperation::RenameFile {
            from,
            to,
            expected_contents,
        } => {
            let source = safe_path(root, from)?;
            let target = safe_path(root, to)?;
            verify_previous(&source, from, expected_contents)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
            }
            let intermediate = source.with_extension("codefabric-case-rename");
            fs::rename(&source, &intermediate).map_err(|error| io(&source, error))?;
            fs::rename(&intermediate, &target).map_err(|error| io(&target, error))?;
        }
        ScenarioOperation::SetContext { path, contents } => {
            write_file(&safe_path(root, path)?, contents)?;
        }
        ScenarioOperation::ProviderFault { provider, fault } => {
            directives.push(ScenarioDirective::ProviderFault {
                provider: provider.clone(),
                fault: fault.clone(),
            });
        }
        ScenarioOperation::DropWatchHint { path } => {
            let _ = safe_path(root, path)?;
            directives.push(ScenarioDirective::DroppedWatchHint(path.clone()));
        }
        ScenarioOperation::ReconcileInventory => {
            directives.push(ScenarioDirective::ReconcileInventory);
        }
        ScenarioOperation::FlushOverlay => {
            directives.push(ScenarioDirective::FlushOverlay);
        }
        ScenarioOperation::RestartDaemon => {
            directives.push(ScenarioDirective::RestartDaemon);
        }
        ScenarioOperation::SetSourceAcl { path, visibility } => {
            let _ = safe_path(root, path)?;
            directives.push(ScenarioDirective::SourceAcl {
                path: path.clone(),
                visibility: visibility.clone(),
            });
        }
    }
    Ok(())
}

/// Copy a fixture workspace, execute operations in authored order, and capture exact checkpoints.
///
/// # Errors
///
/// Returns an error for unsafe paths, a failed previous-content precondition, unsupported
/// filesystem entries, I/O failure, or a checkpoint beyond the operation list.
pub fn materialize_scenario(
    source_workspace: &Path,
    target_workspace: &Path,
    scenario: &Scenario,
) -> Result<Vec<MaterializedCheckpoint>, ScenarioMaterializationError> {
    copy_tree(source_workspace, target_workspace)?;
    let mut checkpoints = scenario.checkpoints.iter().collect::<Vec<_>>();
    checkpoints.sort_by_key(|checkpoint| checkpoint.after_operation);
    if checkpoints
        .last()
        .is_some_and(|checkpoint| checkpoint.after_operation > scenario.operations.len())
    {
        return Err(ScenarioMaterializationError::InvalidCheckpoint(
            scenario.scenario_id.clone(),
        ));
    }
    let mut directives = Vec::new();
    let mut applied = 0;
    let mut output = Vec::new();
    for checkpoint in checkpoints {
        while applied < checkpoint.after_operation {
            apply_operation(
                target_workspace,
                &scenario.operations[applied],
                &mut directives,
            )?;
            applied += 1;
        }
        output.push(MaterializedCheckpoint {
            checkpoint: checkpoint.checkpoint.clone(),
            after_operation: applied,
            files: snapshot_files(target_workspace)?,
            directives: directives.clone(),
        });
    }
    Ok(output)
}
