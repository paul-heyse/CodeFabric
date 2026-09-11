//! Declared submodule boundaries over admitted source, never a request to initialize or fetch.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct GitSubmoduleBoundary {
    pub repository_root: Vec<u8>,
    pub path: Option<Vec<u8>>,
    pub name: Option<Vec<u8>>,
    pub configuration_status: &'static str,
    pub captured_sources: bool,
    pub repository_observed: bool,
    pub stages: Vec<GitIndexStage>,
}

pub(super) fn capture(
    directory: &Path,
    prefix: &[u8],
    index: Option<&gix::index::State>,
    source_directories: &BTreeSet<Vec<u8>>,
    repositories: &BTreeSet<Vec<u8>>,
    policy: &crate::source_inclusion::SourceInclusionPolicy,
    cancellation: &Cancellation,
) -> Vec<GitSubmoduleBoundary> {
    let metadata_path = directory.join(".gitmodules");
    let declared = crate::secure_path::read_control_artifact(
        &metadata_path,
        crate::source_inclusion::SourceInclusionPolicy::MAXIMUM_CONFIGURATION_BYTES,
    );
    let (configuration_status, declarations) = match declared {
        Ok(bytes) => match crate::source_inclusion::declared_submodules(&bytes) {
            Ok(declarations) => ("observed", declarations),
            Err(_) => ("invalid", Vec::new()),
        },
        Err(_)
            if std::fs::symlink_metadata(&metadata_path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            ("absent", Vec::new())
        }
        Err(_) => ("unavailable", Vec::new()),
    };
    let mut boundaries = BTreeMap::<Vec<u8>, GitSubmoduleBoundary>::new();
    let mut invalid = Vec::new();
    let boundary = |path: Option<Vec<u8>>, name, status| GitSubmoduleBoundary {
        repository_root: prefix.to_vec(),
        captured_sources: path
            .as_ref()
            .is_some_and(|path| source_directories.contains(path)),
        repository_observed: path
            .as_ref()
            .is_some_and(|path| repositories.contains(path)),
        path,
        name,
        configuration_status: status,
        stages: Vec::new(),
    };
    for declared in declarations {
        if cancellation.is_cancelled() {
            break;
        }
        let Some(relative) = declared.path else {
            invalid.push(boundary(None, Some(declared.name), "invalid_path"));
            continue;
        };
        let full = full_path(prefix, &relative);
        if !policy.includes(&full, true) {
            continue;
        }
        let entry = boundaries
            .entry(relative)
            .or_insert_with(|| boundary(Some(full), None, configuration_status));
        if entry.name.is_some() {
            entry.configuration_status = "ambiguous_path";
            invalid.push(boundary(
                entry.path.clone(),
                Some(declared.name),
                "ambiguous_path",
            ));
        } else {
            entry.name = Some(declared.name);
        }
    }
    if let Some(index) = index {
        for entry in index.entries() {
            if cancellation.is_cancelled() {
                break;
            }
            if entry.mode != gix::index::entry::Mode::COMMIT {
                continue;
            }
            let raw = entry.path(index).as_bytes();
            let Some(relative) = crate::source_inclusion::relative_root_bytes(raw) else {
                continue;
            };
            let full = full_path(prefix, &relative);
            if !policy.includes(&full, true) {
                continue;
            }
            boundaries
                .entry(relative)
                .or_insert_with(|| boundary(Some(full), None, configuration_status))
                .stages
                .push(GitIndexStage {
                    stage: entry.stage() as u8,
                    mode: entry.mode.bits(),
                    flags: entry.flags.bits(),
                    object_id: entry.id.as_bytes().to_vec(),
                });
        }
    }
    if matches!(configuration_status, "invalid" | "unavailable") {
        invalid.push(boundary(None, None, configuration_status));
    }
    invalid.extend(boundaries.into_values());
    invalid
}

fn full_path(prefix: &[u8], relative: &[u8]) -> Vec<u8> {
    if prefix.is_empty() {
        relative.to_vec()
    } else {
        [prefix, relative].join(&b'/')
    }
}
