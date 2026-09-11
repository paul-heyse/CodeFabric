//! Git metadata over an already bounded, authoritative source census. No Git source reads,
//! status dirwalk, helper process, filter, repository mutation or attached handle escapes.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::Instant;

mod submodules;
pub(crate) use submodules::GitSubmoduleBoundary;

use gix::bstr::ByteSlice as _;
use serde::Serialize;

use crate::cancellation::Cancellation;
use crate::inventory::{InventoryError, InventoryLimits, SourceInventoryRecord, reserve_memory};
use crate::resource_budget::{ChargedValue, ResourceAmounts, ResourceBudget};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct GitIndexStage {
    pub stage: u8,
    pub mode: u32,
    pub flags: u32,
    pub object_id: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct GitAttribute {
    pub name: String,
    pub state: &'static str,
    pub value: Option<Vec<u8>>,
    pub pattern: Vec<u8>,
    pub source: Option<Vec<u8>>,
    pub line: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct GitPathContext {
    pub path: Vec<u8>,
    pub repository_root: Vec<u8>,
    pub classification: Option<u16>,
    pub status: &'static str,
    pub stages: Vec<GitIndexStage>,
    pub attributes: Vec<GitAttribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapturedGitInputs {
    pub paths: Vec<GitPathContext>,
    pub submodules: Vec<GitSubmoduleBoundary>,
    pub digest: [u8; 32],
}

/// Content of the exact rows consumed by a metadata relation. No generation or observation time
/// is present in those rows; unchanged rows may retain their original exact Delta version.
pub(crate) fn relation_digest<T: Serialize>(
    domain: &[u8],
    rows: impl IntoIterator<Item = T>,
) -> [u8; 32] {
    let mut digest = DigestWriter(crate::integrity::IntegrityHasher::new());
    digest.0.update(domain);
    for row in rows {
        serde_json::to_writer(&mut digest, &row)
            .expect("detached metadata serialization is infallible");
    }
    digest.0.finalize()
}

/// Repositories are opened sequentially on the caller's blocking owner. A failed Git observation
/// is explicit metadata unavailability, never evidence that source bytes or semantic facts vanished.
pub(crate) fn capture(
    root: &Path,
    records: &[SourceInventoryRecord],
    limits: InventoryLimits,
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> Result<ChargedValue<CapturedGitInputs>, InventoryError> {
    let started = Instant::now();
    let mut retained = reserve_memory(budget, 1024)?;
    let mut scratch = reserve_memory(budget, 1024)?;
    let mut directories = BTreeSet::from([Vec::new()]);
    for record in records {
        let path = &record.path.raw_relative_path_bytes;
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' && !directories.contains(&path[..index]) {
                scratch.try_grow(ResourceAmounts {
                    memory_bytes: index as u64 + 128,
                    ..ResourceAmounts::default()
                })?;
                directories.insert(path[..index].to_vec());
            }
        }
    }
    let mut repositories = BTreeSet::new();
    for directory in &directories {
        check_progress(started, limits, cancellation)?;
        let marker = root
            .join(Path::new(std::ffi::OsStr::from_bytes(directory)))
            .join(".git");
        if std::fs::symlink_metadata(marker).is_ok() {
            repositories.insert(directory.clone());
        }
    }
    let policy = crate::source_inclusion::SourceInclusionPolicy::capture_watch(root);
    let mut submodules = Vec::new();
    let mut groups = BTreeMap::<Vec<u8>, Vec<&SourceInventoryRecord>>::new();
    for record in records {
        let path = &record.path.raw_relative_path_bytes;
        let mut length = path.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
        let owner = loop {
            if let Some(owner) = repositories.get(&path[..length]) {
                break Some(owner);
            }
            if length == 0 {
                break None;
            }
            length = path[..length]
                .iter()
                .rposition(|byte| *byte == b'/')
                .unwrap_or(0);
        };
        if let Some(owner) = owner {
            scratch.try_grow(ResourceAmounts {
                memory_bytes: owner.len() as u64 + 64,
                ..ResourceAmounts::default()
            })?;
            groups.entry(owner.clone()).or_default().push(record);
        }
    }
    let mut paths = Vec::new();
    for repository in &repositories {
        groups.entry(repository.clone()).or_default();
    }
    for (prefix, members) in groups {
        check_progress(started, limits, cancellation)?;
        let directory = root.join(Path::new(std::ffi::OsStr::from_bytes(&prefix)));
        let repository = super::open_isolated(&directory);
        let result = repository.as_ref().ok().and_then(|repository| {
            let bytes = std::fs::metadata(repository.index_path()).map_or(0, |value| value.len());
            // Index files include paths and extensions. Bound native loading using the existing
            // census byte budget, then account a broad decoded-index working allowance.
            (bytes <= limits.maximum_total_bytes_considered).then_some((repository, bytes))
        });
        let index_charge = result
            .map(|(_, bytes)| reserve_memory(budget, bytes.saturating_mul(4)))
            .transpose()?;
        let loaded_index = result.and_then(|(repository, _)| repository.index_or_empty().ok());
        let index = loaded_index.as_ref().filter(|index| {
            index.entries().len() as u64 <= limits.maximum_file_count.saturating_mul(4)
        });
        let boundaries = submodules::capture(
            &directory,
            &prefix,
            index.map(|index| -> &gix::index::State { index }),
            &directories,
            &repositories,
            &policy,
            cancellation,
        );
        check_progress(started, limits, cancellation)?;
        retained.try_grow(ResourceAmounts {
            memory_bytes: boundaries
                .iter()
                .map(|boundary| {
                    512 + boundary.repository_root.len()
                        + boundary.path.as_ref().map_or(0, Vec::len)
                        + boundary.name.as_ref().map_or(0, Vec::len)
                        + boundary.stages.len() * 128
                })
                .sum::<usize>() as u64,
            ..ResourceAmounts::default()
        })?;
        submodules.extend(boundaries);
        let mut attributes = repository
            .as_ref()
            .ok()
            .zip(index)
            .and_then(|(repository, index)| {
                repository.attributes(
            index,
            gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
            None,
        ).ok()
            });
        let mut matches = gix::attrs::search::Outcome::default();
        for member in members {
            check_progress(started, limits, cancellation)?;
            let path = member.path.raw_relative_path_bytes.as_slice();
            let relative = if prefix.is_empty() {
                path
            } else {
                &path[prefix.len() + 1..]
            };
            let mut context = GitPathContext {
                path: path.to_vec(),
                repository_root: prefix.clone(),
                classification: None,
                status: "unavailable",
                stages: Vec::new(),
                attributes: Vec::new(),
            };
            if let Some(index) = index {
                for stage in [
                    gix::index::entry::Stage::Unconflicted,
                    gix::index::entry::Stage::Base,
                    gix::index::entry::Stage::Ours,
                    gix::index::entry::Stage::Theirs,
                ] {
                    if let Some(entry) = index.entry_by_path_and_stage(relative.as_bstr(), stage) {
                        context.stages.push(GitIndexStage {
                            stage: stage as u8,
                            mode: entry.mode.bits(),
                            flags: entry.flags.bits(),
                            object_id: entry.id.as_bytes().to_vec(),
                        });
                    }
                }
            }
            if let Some(stack) = attributes.as_mut()
                && let Ok(platform) = stack.at_entry(relative.as_bstr(), None)
            {
                let ignored = platform.is_excluded();
                context.classification = Some(if member.file_kind
                    != crate::inventory::InventoryFileKind::Regular
                {
                    super::GitInventoryClassification::SpecialFile
                } else {
                    match (!context.stages.is_empty(), ignored) {
                        (true, true) => {
                            super::GitInventoryClassification::TrackedButIgnoredPatternMatches
                        }
                        (true, false) => super::GitInventoryClassification::Tracked,
                        (false, true) => super::GitInventoryClassification::UntrackedIgnored,
                        (false, false) => super::GitInventoryClassification::UntrackedNotIgnored,
                    }
                } as u16);
                platform.matching_attributes(&mut matches);
                for attribute in matches.iter() {
                    let (state, value) = match attribute.assignment.state {
                        gix::attrs::StateRef::Set => ("set", None),
                        gix::attrs::StateRef::Unset => ("unset", None),
                        gix::attrs::StateRef::Unspecified => ("unspecified", None),
                        gix::attrs::StateRef::Value(value) => {
                            ("value", Some(value.as_bstr().to_vec()))
                        }
                    };
                    context.attributes.push(GitAttribute {
                        name: attribute.assignment.name.as_str().to_owned(),
                        state,
                        value,
                        pattern: attribute.pattern.text.to_vec(),
                        source: attribute
                            .location
                            .source
                            .map(|path| path.as_os_str().as_bytes().to_vec()),
                        line: attribute.location.sequence_number as u64,
                    });
                }
                context
                    .attributes
                    .sort_by(|left, right| left.name.cmp(&right.name));
                context.status = "observed";
            }
            let bytes = 256
                + context.path.len()
                + context.repository_root.len()
                + context.stages.len() * 128
                + context
                    .attributes
                    .iter()
                    .map(|attribute| {
                        256 + attribute.name.len()
                            + attribute.pattern.len()
                            + attribute.value.as_ref().map_or(0, Vec::len)
                            + attribute.source.as_ref().map_or(0, Vec::len)
                    })
                    .sum::<usize>();
            retained.try_grow(ResourceAmounts {
                memory_bytes: bytes as u64,
                ..ResourceAmounts::default()
            })?;
            paths.push(context);
        }
        drop(attributes);
        drop(loaded_index);
        drop(repository);
        drop(index_charge);
    }
    paths.sort_by(|left, right| left.path.cmp(&right.path));
    let mut digest = DigestWriter(crate::integrity::IntegrityHasher::new());
    digest
        .0
        .update(b"codefabric.captured-git-input-context.v2\0");
    serde_json::to_writer(&mut digest, &(&paths, &submodules))
        .expect("detached metadata serialization is infallible");
    Ok(retained.into_charged_value(CapturedGitInputs {
        paths,
        submodules,
        digest: digest.0.finalize(),
    }))
}

fn check_progress(
    started: Instant,
    limits: InventoryLimits,
    cancellation: &Cancellation,
) -> Result<(), InventoryError> {
    if cancellation.is_cancelled() {
        return Err(InventoryError::Cancelled);
    }
    if started.elapsed() > limits.maximum_duration {
        return Err(InventoryError::BoundExceeded("git-context-duration"));
    }
    Ok(())
}

struct DigestWriter(crate::integrity::IntegrityHasher);
impl std::io::Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
