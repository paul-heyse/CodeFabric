//! Deployment observations trigger reconciliation; provider facts still use captured context.

use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(crate) enum ProviderExecutable {
    Pyrefly,
    RustcExtractor,
}

impl ProviderExecutable {
    pub(crate) fn selected_path(self) -> io::Result<PathBuf> {
        let (variable, name) = match self {
            Self::Pyrefly => (
                "CODEFABRIC_PYREFLY_SIDECAR_BIN",
                "codefabric-pyrefly-sidecar",
            ),
            Self::RustcExtractor => (
                "CODEFABRIC_RUSTC_EXTRACTOR_BIN",
                "codefabric-rustc-extractor",
            ),
        };
        if let Some(path) = std::env::var_os(variable) {
            return Ok(PathBuf::from(path));
        }
        std::env::current_exe()?
            .parent()
            .map(|parent| parent.join(name))
            .ok_or_else(|| io::Error::other("daemon executable has no parent"))
    }
}

/// A small, cache-independent witness survives in the selected source inventory relation.
/// It observes only the two selected external provider executables, not an environment inventory.
pub(crate) fn observation_digest() -> io::Result<[u8; 32]> {
    observe_paths(&[
        ProviderExecutable::Pyrefly.selected_path()?,
        ProviderExecutable::RustcExtractor.selected_path()?,
    ])
}

fn frame(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

fn observe_paths(paths: &[PathBuf]) -> io::Result<[u8; 32]> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"codefabric.selected-provider-deployment-observation.v1\0");
    for path in paths {
        frame(&mut hash, path.as_os_str().as_bytes());
        observe_path(&mut hash, path)?;
    }
    Ok(*hash.finalize().as_bytes())
}

fn observe_path(hash: &mut blake3::Hasher, path: &Path) -> io::Result<()> {
    let resolved = match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            frame(hash, b"absent");
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let metadata = std::fs::metadata(&resolved)?;
    frame(hash, b"present");
    frame(hash, resolved.as_os_str().as_bytes());
    // Change time detects same-length replacement even if a deployer restores mtime.
    // No content identity or provider version is inferred from this observation.
    for bytes in [
        metadata.dev().to_be_bytes(),
        metadata.ino().to_be_bytes(),
        metadata.len().to_be_bytes(),
        u64::from(metadata.mode()).to_be_bytes(),
        u64::from(metadata.uid()).to_be_bytes(),
        u64::from(metadata.gid()).to_be_bytes(),
        metadata.mtime().to_be_bytes(),
        metadata.mtime_nsec().to_be_bytes(),
        metadata.ctime().to_be_bytes(),
        metadata.ctime_nsec().to_be_bytes(),
    ] {
        frame(hash, &bytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt as _;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    #[test]
    fn provider_observations_detect_restored_mtime_replacement_removal_and_symlink_retarget() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(std::ffi::OsString::from_vec(b"provider-\xff".to_vec()));
        let paths = [path.clone()];
        let absent = observe_paths(&paths).unwrap();
        std::fs::write(&path, b"old").unwrap();
        let first = observe_paths(&paths).unwrap();
        assert_ne!(absent, first);
        assert_eq!(first, observe_paths(&paths).unwrap());
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let replacement = root.path().join("replacement");
        std::fs::write(&replacement, b"new").unwrap();
        std::fs::File::open(&replacement)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        let replaced = observe_paths(&paths).unwrap();
        assert_ne!(first, replaced);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(replaced, observe_paths(&paths).unwrap());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(absent, observe_paths(&paths).unwrap());
        std::fs::write(&replacement, b"new").unwrap();
        symlink(&replacement, &path).unwrap();
        let linked = observe_paths(&paths).unwrap();
        let other = root.path().join("other");
        std::fs::write(&other, b"new").unwrap();
        std::fs::remove_file(&path).unwrap();
        symlink(other, &path).unwrap();
        assert_ne!(linked, observe_paths(&paths).unwrap());
        std::fs::remove_file(&path).unwrap();
        symlink(root.path().join("missing"), &path).unwrap();
        assert_eq!(absent, observe_paths(&paths).unwrap());
    }
}
