//! AC-G-11 descriptor-relative source authorization and byte reads.

use std::ffi::{OsStr, OsString};
use std::io::Read as _;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rustix::fs::{FileType, Mode, OFlags, fstat, open, openat};
use serde::Serialize;
use thiserror::Error;

use crate::identity::{
    CaseSensitivityMode, IdentityError, PlatformCode, RootAuthorizationInput, WorkspacePath,
    random_registration_nonce, root_authorization_fingerprint, source_file_identity,
    validate_workspace_paths,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::registries::SourceTrustState;
use crate::workspace_registry::{WorkspaceRecord, WorkspaceRegistry, WorkspaceRegistryError};

const WORKSPACE_NOT_AUTHORIZED: u16 = 2_000;
const PATH_OUTSIDE_AUTHORIZED_ROOT: u16 = 2_010;
const SOURCE_ACCESS_DENIED: u16 = 2_020;
const BLOCKED_PATH_COLLISION: u16 = 2_030;

/// A stable, redaction-safe diagnostic emitted for every rejected path operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurePathDiagnostic {
    pub code: u16,
    pub name: &'static str,
}

/// Stable secure-path failures. No variant carries source bytes or host paths.
#[derive(Debug, Error)]
pub enum SecurePathError {
    #[error("WORKSPACE_NOT_AUTHORIZED: root authorization no longer matches")]
    RootAuthorizationChanged,
    #[error("PATH_OUTSIDE_AUTHORIZED_ROOT: invalid workspace-relative path ({0})")]
    InvalidRelativePath(&'static str),
    #[error("PATH_OUTSIDE_AUTHORIZED_ROOT: descriptor-relative open rejected the path")]
    OutsideAuthorizedRoot,
    #[error("SOURCE_ACCESS_DENIED: selected entry is not an authorized regular file")]
    SourceAccessDenied,
    #[error("BLOCKED_PATH_COLLISION: distinct paths share one comparison key")]
    PathCollision,
    #[error("secure-path operating-system operation failed")]
    OperatingSystem,
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Registry(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

impl SecurePathError {
    /// Return the released error-registry identity without exposing rejected input.
    #[must_use]
    pub const fn diagnostic(&self) -> SecurePathDiagnostic {
        match self {
            Self::RootAuthorizationChanged => SecurePathDiagnostic {
                code: WORKSPACE_NOT_AUTHORIZED,
                name: "WORKSPACE_NOT_AUTHORIZED",
            },
            Self::InvalidRelativePath(_) | Self::OutsideAuthorizedRoot => SecurePathDiagnostic {
                code: PATH_OUTSIDE_AUTHORIZED_ROOT,
                name: "PATH_OUTSIDE_AUTHORIZED_ROOT",
            },
            Self::PathCollision => SecurePathDiagnostic {
                code: BLOCKED_PATH_COLLISION,
                name: "BLOCKED_PATH_COLLISION",
            },
            Self::SourceAccessDenied
            | Self::OperatingSystem
            | Self::Store(_)
            | Self::Registry(_)
            | Self::Identity(_) => SecurePathDiagnostic {
                code: SOURCE_ACCESS_DENIED,
                name: "SOURCE_ACCESS_DENIED",
            },
        }
    }
}

/// Platform-native, lexically validated relative path supplied at an ingress boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformPath {
    platform_code: PlatformCode,
    raw_relative_path_bytes: Vec<u8>,
    components: Vec<Vec<u8>>,
}

impl PlatformPath {
    /// Validate exact platform bytes without interpreting a display string.
    ///
    /// # Errors
    ///
    /// Returns a stable path rejection for absolute, empty, dot, NUL, or device paths.
    pub fn from_raw_relative_bytes(
        platform_code: PlatformCode,
        bytes: Vec<u8>,
    ) -> Result<Self, SecurePathError> {
        if platform_code == PlatformCode::WindowsWtf8 {
            return Err(SecurePathError::InvalidRelativePath("unsupported-platform"));
        }
        if bytes.is_empty() {
            return Err(SecurePathError::InvalidRelativePath("empty"));
        }
        if bytes.first() == Some(&b'/') {
            return Err(SecurePathError::InvalidRelativePath("absolute"));
        }
        if bytes.contains(&0) {
            return Err(SecurePathError::InvalidRelativePath("nul"));
        }
        if has_device_prefix(&bytes) {
            return Err(SecurePathError::InvalidRelativePath("device-prefix"));
        }
        let components = bytes
            .split(|byte| *byte == b'/')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if components
            .iter()
            .any(|component| component.is_empty() || matches!(component.as_slice(), b"." | b".."))
        {
            return Err(SecurePathError::InvalidRelativePath("dot-component"));
        }
        Ok(Self {
            platform_code,
            raw_relative_path_bytes: bytes,
            components,
        })
    }

    /// Exact canonical input bytes. These are never derived from `display_string`.
    #[must_use]
    pub fn raw_relative_path_bytes(&self) -> &[u8] {
        &self.raw_relative_path_bytes
    }
}

/// A Git-provided path that remains advisory until revalidated by a `SecureRoot`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitRepoPath(PlatformPath);

impl GitRepoPath {
    /// Parse byte-safe Git path evidence through the same lexical boundary.
    ///
    /// # Errors
    ///
    /// Returns a stable path rejection when Git supplied an unsafe relative path.
    pub fn from_raw_relative_bytes(
        platform_code: PlatformCode,
        bytes: Vec<u8>,
    ) -> Result<Self, SecurePathError> {
        PlatformPath::from_raw_relative_bytes(platform_code, bytes).map(Self)
    }
}

/// The exact eight-field AC-G-11 record, including its derived fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootAuthorizationRecord {
    pub workspace_id: [u8; 16],
    pub root_path_bytes: Vec<u8>,
    pub root_directory_file_identity: Vec<u8>,
    pub platform_code: PlatformCode,
    pub case_sensitivity_mode: CaseSensitivityMode,
    pub authorization_revision: u64,
    pub authorization_fingerprint: [u8; 32],
    pub allowed_source_disclosure_rules: Vec<String>,
}

impl TryFrom<&WorkspaceRecord> for RootAuthorizationRecord {
    type Error = SecurePathError;

    fn try_from(record: &WorkspaceRecord) -> Result<Self, Self::Error> {
        let platform_code = match record.platform_code {
            1 => PlatformCode::Unix,
            2 => PlatformCode::MacOs,
            value => return Err(IdentityError::Platform(value).into()),
        };
        let case_sensitivity_mode = match record.case_sensitivity_mode.as_str() {
            "sensitive" => CaseSensitivityMode::Sensitive,
            "insensitive" => CaseSensitivityMode::Insensitive,
            _ => return Err(SecurePathError::RootAuthorizationChanged),
        };
        Ok(Self {
            workspace_id: record.workspace_id,
            root_path_bytes: record.root_path_bytes.clone(),
            root_directory_file_identity: record.root_directory_file_identity.clone(),
            platform_code,
            case_sensitivity_mode,
            authorization_revision: record.authorization_revision,
            authorization_fingerprint: record.authorization_fingerprint,
            allowed_source_disclosure_rules: record.allowed_source_disclosure_rules.clone(),
        })
    }
}

/// One authorized root descriptor; no descendant path is joined onto its host path.
pub struct SecureRoot {
    authorization: RootAuthorizationRecord,
    descriptor: OwnedFd,
    root_identity: FileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

/// Byte-exact source content bound to its canonical workspace path and file ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedSourceBytes {
    pub path: WorkspacePath,
    pub file_id: [u8; 16],
    pub bytes: Vec<u8>,
}

impl SecureRoot {
    /// Validate the persisted fingerprint, open the root no-follow, and verify its identity.
    ///
    /// # Errors
    ///
    /// Returns a stable authorization error for a changed or malformed root record.
    pub fn authorize(record: RootAuthorizationRecord) -> Result<Self, SecurePathError> {
        let expected_fingerprint = root_authorization_fingerprint(&RootAuthorizationInput {
            workspace_id: record.workspace_id,
            root_path_bytes: record.root_path_bytes.clone(),
            root_directory_file_identity: record.root_directory_file_identity.clone(),
            platform_code: record.platform_code as u8,
            case_sensitivity_mode: case_mode_name(record.case_sensitivity_mode).to_owned(),
            authorization_revision: record.authorization_revision,
            allowed_source_disclosure_rules: record.allowed_source_disclosure_rules.clone(),
        })?;
        if expected_fingerprint != record.authorization_fingerprint {
            return Err(SecurePathError::RootAuthorizationChanged);
        }
        let expected_identity = decode_file_identity(&record.root_directory_file_identity)?;
        let descriptor = open_root_descriptor(&record)?;
        let actual_identity = descriptor_identity(&descriptor)?;
        if actual_identity != expected_identity {
            return Err(SecurePathError::RootAuthorizationChanged);
        }
        Ok(Self {
            authorization: record,
            descriptor,
            root_identity: expected_identity,
        })
    }

    /// Revalidate a Git advisory path into the canonical workspace path type.
    ///
    /// # Errors
    ///
    /// Returns an identity or authorization error when the path is not usable.
    pub fn revalidate_git_path(
        &self,
        path: &GitRepoPath,
    ) -> Result<WorkspacePath, SecurePathError> {
        self.workspace_path(&path.0)
    }

    /// Convert a raw platform path into its governed workspace-relative representation.
    ///
    /// # Errors
    ///
    /// Returns an identity error when the canonical path views cannot be constructed.
    pub fn workspace_path(&self, path: &PlatformPath) -> Result<WorkspacePath, SecurePathError> {
        if path.platform_code != self.authorization.platform_code {
            return Err(SecurePathError::InvalidRelativePath("platform-mismatch"));
        }
        WorkspacePath::from_components(
            self.authorization.workspace_id,
            self.authorization.platform_code,
            self.authorization.case_sensitivity_mode,
            &path.components,
        )
        .map_err(Into::into)
    }

    /// Open one regular file beneath the root and return its owned descriptor.
    ///
    /// # Errors
    ///
    /// Returns a stable rejection for symlinks, mount escapes, root changes, or non-files.
    pub fn open_file(&self, path: &PlatformPath) -> Result<OwnedFd, SecurePathError> {
        self.revalidate_root()?;
        let descriptor = self.open_beneath(path)?;
        let file_stat = fstat(&descriptor).map_err(|_| SecurePathError::OperatingSystem)?;
        if !FileType::from_raw_mode(file_stat.st_mode).is_file() {
            return Err(SecurePathError::SourceAccessDenied);
        }
        ensure_same_device(self.root_identity.device, device_id(file_stat.st_dev)?)?;
        self.revalidate_root()?;
        Ok(descriptor)
    }

    /// Read authoritative bytes only from an already authorized owned descriptor.
    ///
    /// # Errors
    ///
    /// Returns a stable source-access rejection when identity changes during the read.
    pub fn read_file(&self, path: &PlatformPath) -> Result<Vec<u8>, SecurePathError> {
        let descriptor = self.open_file(path)?;
        let before = descriptor_identity(&descriptor)?;
        let mut file = std::fs::File::from(descriptor);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| SecurePathError::OperatingSystem)?;
        let after = descriptor_identity(&file)?;
        if before != after {
            return Err(SecurePathError::SourceAccessDenied);
        }
        self.revalidate_root()?;
        Ok(bytes)
    }

    fn revalidate_root(&self) -> Result<(), SecurePathError> {
        if descriptor_identity(&self.descriptor)? != self.root_identity {
            return Err(SecurePathError::RootAuthorizationChanged);
        }
        let current = open_root_descriptor(&self.authorization)
            .and_then(|descriptor| descriptor_identity(&descriptor));
        if !matches!(current, Ok(identity) if identity == self.root_identity) {
            return Err(SecurePathError::RootAuthorizationChanged);
        }
        Ok(())
    }

    fn open_beneath(&self, path: &PlatformPath) -> Result<OwnedFd, SecurePathError> {
        #[cfg(target_os = "linux")]
        if let Some(descriptor) = self.open_linux(path)? {
            return Ok(descriptor);
        }
        self.open_component_walk(path)
    }

    #[cfg(target_os = "linux")]
    fn open_linux(&self, path: &PlatformPath) -> Result<Option<OwnedFd>, SecurePathError> {
        use rustix::fs::{ResolveFlags, openat2};
        use rustix::io::Errno;

        let relative = OsStr::from_bytes(path.raw_relative_path_bytes());
        match openat2(
            &self.descriptor,
            relative,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_XDEV,
        ) {
            Ok(descriptor) => Ok(Some(descriptor)),
            Err(Errno::NOSYS | Errno::INVAL) => Ok(None),
            Err(_) => Err(SecurePathError::OutsideAuthorizedRoot),
        }
    }

    fn open_component_walk(&self, path: &PlatformPath) -> Result<OwnedFd, SecurePathError> {
        let (final_component, directory_components) = path
            .components
            .split_last()
            .ok_or(SecurePathError::InvalidRelativePath("empty"))?;
        let mut current = None::<OwnedFd>;
        for component in directory_components {
            let opened = if let Some(directory) = &current {
                openat(
                    directory,
                    OsStr::from_bytes(component),
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
                    Mode::empty(),
                )
            } else {
                openat(
                    &self.descriptor,
                    OsStr::from_bytes(component),
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
                    Mode::empty(),
                )
            }
            .map_err(|_| SecurePathError::OutsideAuthorizedRoot)?;
            let stat = fstat(&opened).map_err(|_| SecurePathError::OperatingSystem)?;
            if !FileType::from_raw_mode(stat.st_mode).is_dir() {
                return Err(SecurePathError::OutsideAuthorizedRoot);
            }
            ensure_same_device(self.root_identity.device, device_id(stat.st_dev)?)?;
            current = Some(opened);
        }
        let opened = if let Some(directory) = &current {
            openat(
                directory,
                OsStr::from_bytes(final_component),
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
        } else {
            openat(
                &self.descriptor,
                OsStr::from_bytes(final_component),
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
        }
        .map_err(|_| SecurePathError::OutsideAuthorizedRoot)?;
        Ok(opened)
    }
}

/// Load and authorize a persisted workspace root, marking changed roots `VERIFYING`.
///
/// # Errors
///
/// Returns a store, registry, identity, or stable root-authorization rejection.
pub fn open_workspace_root(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
) -> Result<SecureRoot, SecurePathError> {
    let record = WorkspaceRegistry::new(store).show(workspace_id)?;
    let authorization = RootAuthorizationRecord::try_from(&record)?;
    match SecureRoot::authorize(authorization) {
        Ok(root) => Ok(root),
        Err(error @ SecurePathError::RootAuthorizationChanged) => {
            mark_source_verifying(store, workspace_id, error.diagnostic())?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

/// Sole production port for authoritative source-byte reads.
///
/// # Errors
///
/// Returns a stable authorization, path, identity, or store error.
pub fn read_workspace_source(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    path: &PlatformPath,
) -> Result<AuthorizedSourceBytes, SecurePathError> {
    let root = open_workspace_root(store, workspace_id)?;
    let workspace_path = root.workspace_path(path)?;
    let bytes = root.read_file(path)?;
    let file_id = source_file_identity(&workspace_path)?.id;
    Ok(AuthorizedSourceBytes {
        path: workspace_path,
        file_id,
        bytes,
    })
}

/// Reject comparison-key collisions with the released blocking diagnostic.
///
/// # Errors
///
/// Returns `BLOCKED_PATH_COLLISION` for distinct raw paths sharing one key.
pub fn validate_inventory_paths(paths: &[WorkspacePath]) -> Result<(), SecurePathError> {
    validate_workspace_paths(paths).map_err(|error| match error {
        IdentityError::PathCollision => SecurePathError::PathCollision,
        other => SecurePathError::Identity(other),
    })
}

fn has_device_prefix(bytes: &[u8]) -> bool {
    bytes.starts_with(br"\\")
        || bytes.starts_with(br"\?")
        || bytes.starts_with(br"\.")
        || bytes
            .get(..2)
            .is_some_and(|prefix| prefix[0].is_ascii_alphabetic() && prefix[1] == b':')
}

fn case_mode_name(mode: CaseSensitivityMode) -> &'static str {
    match mode {
        CaseSensitivityMode::Sensitive => "sensitive",
        CaseSensitivityMode::Insensitive => "insensitive",
    }
}

fn root_path(record: &RootAuthorizationRecord) -> Result<PathBuf, SecurePathError> {
    if record.root_path_bytes.is_empty() || record.root_path_bytes.contains(&0) {
        return Err(SecurePathError::RootAuthorizationChanged);
    }
    Ok(PathBuf::from(OsString::from_vec(
        record.root_path_bytes.clone(),
    )))
}

fn open_root_descriptor(record: &RootAuthorizationRecord) -> Result<OwnedFd, SecurePathError> {
    open(
        root_path(record)?,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| SecurePathError::RootAuthorizationChanged)
}

fn descriptor_identity(
    descriptor: impl std::os::fd::AsFd,
) -> Result<FileIdentity, SecurePathError> {
    let stat = fstat(descriptor).map_err(|_| SecurePathError::OperatingSystem)?;
    Ok(FileIdentity {
        device: device_id(stat.st_dev)?,
        inode: stat.st_ino as u64,
    })
}

fn device_id(raw: rustix::fs::Dev) -> Result<u64, SecurePathError> {
    raw.try_into().map_err(|_| SecurePathError::OperatingSystem)
}

fn decode_file_identity(bytes: &[u8]) -> Result<FileIdentity, SecurePathError> {
    let fixed =
        <&[u8; 16]>::try_from(bytes).map_err(|_| SecurePathError::RootAuthorizationChanged)?;
    let (device, inode) = fixed.split_at(8);
    Ok(FileIdentity {
        device: u64::from_be_bytes(device.try_into().unwrap()),
        inode: u64::from_be_bytes(inode.try_into().unwrap()),
    })
}

fn ensure_same_device(root: u64, candidate: u64) -> Result<(), SecurePathError> {
    if root == candidate {
        Ok(())
    } else {
        Err(SecurePathError::OutsideAuthorizedRoot)
    }
}

fn mark_source_verifying(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    diagnostic: SecurePathDiagnostic,
) -> Result<(), SecurePathError> {
    let event_id = random_registration_nonce()?;
    let occurred_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SecurePathError::OperatingSystem)?
        .as_millis()
        .to_string();
    let details = *blake3::hash(diagnostic.name.as_bytes()).as_bytes();
    store.write_transaction(|transaction| {
        let updated = transaction.execute(
            "UPDATE worktree_state SET source_trust_state_code=?2, reconcile_required=1, updated_at=?3 WHERE workspace_id=?1",
            rusqlite::params![workspace_id.as_slice(), SourceTrustState::Verifying as u16, &occurred_at],
        )
        .map_err(OperationalStoreError::from)?;
        if updated != 1 {
            return Err(SecurePathError::RootAuthorizationChanged);
        }
        transaction.execute(
            "INSERT INTO audit_event(event_id, workspace_id, event_code, actor_id, occurred_at, details_digest, diagnostic_id) VALUES (?1, ?2, ?3, 'secure-path', ?4, ?5, ?1)",
            rusqlite::params![event_id.as_slice(), workspace_id.as_slice(), diagnostic.code, &occurred_at, details.as_slice()],
        )
        .map_err(OperationalStoreError::from)?;
        Ok::<(), SecurePathError>(())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd as _;
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::identity::{IdentityDomain, decode_public_id};
    use crate::workspace_registry::WorkspaceSourceRegistration;

    fn registered_root() -> (
        tempfile::TempDir,
        OperationalStore,
        WorkspaceRecord,
        PathBuf,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let record = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap();
        (directory, store, record, root)
    }

    #[test]
    fn wp15_behavioral_acceptance() {
        let (_directory, mut store, record, root_path) = registered_root();
        fs::create_dir(root_path.join("src")).unwrap();
        fs::write(root_path.join("src/lib.rs"), b"fn main() {}\n").unwrap();
        let path = PlatformPath::from_raw_relative_bytes(
            if cfg!(target_os = "macos") {
                PlatformCode::MacOs
            } else {
                PlatformCode::Unix
            },
            b"src/lib.rs".to_vec(),
        )
        .unwrap();
        let root = open_workspace_root(&mut store, record.workspace_id).unwrap();
        let descriptor = root.open_file(&path).unwrap();
        assert!(descriptor.as_raw_fd() >= 0);
        drop(descriptor);
        let source = read_workspace_source(&mut store, record.workspace_id, &path).unwrap();
        assert_eq!(source.bytes, b"fn main() {}\n");
        assert_eq!(source.file_id.len(), 16);

        let upper = WorkspacePath::from_components(
            record.workspace_id,
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &[b"SRC".to_vec(), b"LIB.RS".to_vec()],
        )
        .unwrap();
        let lower = WorkspacePath::from_components(
            record.workspace_id,
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &[b"src".to_vec(), b"lib.rs".to_vec()],
        )
        .unwrap();
        assert_eq!(upper.comparison_key_bytes, lower.comparison_key_bytes);
        assert_eq!(
            source_file_identity(&upper).unwrap().id,
            source_file_identity(&lower).unwrap().id
        );
    }

    #[test]
    fn wp15_structural_acceptance() {
        let (_directory, mut store, record, root_path) = registered_root();
        fs::write(root_path.join("source.rs"), b"let x = 1;\n").unwrap();
        let git_path = GitRepoPath::from_raw_relative_bytes(
            if cfg!(target_os = "macos") {
                PlatformCode::MacOs
            } else {
                PlatformCode::Unix
            },
            b"source.rs".to_vec(),
        )
        .unwrap();
        let root = open_workspace_root(&mut store, record.workspace_id).unwrap();
        let path = root.revalidate_git_path(&git_path).unwrap();
        assert_eq!(path.raw_relative_path_bytes, b"source.rs");
        assert!(decode_public_id(IdentityDomain::Workspace, None, &record.public_id()).is_ok());
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One adversarial matrix keeps all eight AC-G-11 checks visible.
    fn wp15_negative_zero_state() {
        let (directory, mut store, record, root_path) = registered_root();
        let platform = if cfg!(target_os = "macos") {
            PlatformCode::MacOs
        } else {
            PlatformCode::Unix
        };
        for (bytes, reason) in [
            (b"../escape".as_slice(), "dot-component"),
            (b"/absolute".as_slice(), "absolute"),
            (b"nul\0path".as_slice(), "nul"),
            (b"C:device".as_slice(), "device-prefix"),
            (br"\\server\share".as_slice(), "device-prefix"),
            (b"empty//component".as_slice(), "dot-component"),
        ] {
            let error =
                PlatformPath::from_raw_relative_bytes(platform, bytes.to_vec()).unwrap_err();
            assert!(
                matches!(error, SecurePathError::InvalidRelativePath(found) if found == reason)
            );
            assert_eq!(error.diagnostic().code, PATH_OUTSIDE_AUTHORIZED_ROOT);
        }

        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("secret.rs"), b"secret").unwrap();
        symlink(&outside, root_path.join("escape")).unwrap();
        let escaped =
            PlatformPath::from_raw_relative_bytes(platform, b"escape/secret.rs".to_vec()).unwrap();
        let root = open_workspace_root(&mut store, record.workspace_id).unwrap();
        assert_eq!(
            root.read_file(&escaped).unwrap_err().diagnostic().code,
            PATH_OUTSIDE_AUTHORIZED_ROOT
        );

        let real = root_path.join("middle");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("source.rs"), b"inside").unwrap();
        let swapped =
            PlatformPath::from_raw_relative_bytes(platform, b"middle/source.rs".to_vec()).unwrap();
        fs::rename(&real, root_path.join("middle-old")).unwrap();
        symlink(&outside, &real).unwrap();
        assert!(root.read_file(&swapped).is_err());

        assert!(matches!(
            ensure_same_device(1, 2),
            Err(SecurePathError::OutsideAuthorizedRoot)
        ));

        let insensitive_a = WorkspacePath::from_components(
            record.workspace_id,
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &[b"Source.rs".to_vec()],
        )
        .unwrap();
        let insensitive_b = WorkspacePath::from_components(
            record.workspace_id,
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &[b"source.rs".to_vec()],
        )
        .unwrap();
        assert_eq!(
            validate_inventory_paths(&[insensitive_a, insensitive_b])
                .unwrap_err()
                .diagnostic(),
            SecurePathDiagnostic {
                code: BLOCKED_PATH_COLLISION,
                name: "BLOCKED_PATH_COLLISION",
            }
        );

        drop(root);
        fs::rename(&root_path, directory.path().join("retired-root")).unwrap();
        fs::create_dir(&root_path).unwrap();
        let Err(error) = open_workspace_root(&mut store, record.workspace_id) else {
            panic!("replacement root was accepted");
        };
        assert_eq!(error.diagnostic().code, WORKSPACE_NOT_AUTHORIZED);
        let trust_state = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT source_trust_state_code FROM worktree_state WHERE workspace_id=?1",
                    [record.workspace_id.as_slice()],
                    |row| row.get::<_, u16>(0),
                )
            })
            .unwrap();
        assert_eq!(trust_state, SourceTrustState::Verifying as u16);
    }

    #[test]
    fn wp15_operational_acceptance() {
        let (_directory, mut store, record, root_path) = registered_root();
        fs::write(root_path.join("source.rs"), b"source").unwrap();
        let platform = if cfg!(target_os = "macos") {
            PlatformCode::MacOs
        } else {
            PlatformCode::Unix
        };
        let errors = [
            PlatformPath::from_raw_relative_bytes(platform, b"../outside".to_vec()).unwrap_err(),
            SecurePathError::SourceAccessDenied,
            SecurePathError::PathCollision,
            SecurePathError::RootAuthorizationChanged,
        ];
        assert_eq!(
            errors.map(|error| error.diagnostic()),
            [
                SecurePathDiagnostic {
                    code: PATH_OUTSIDE_AUTHORIZED_ROOT,
                    name: "PATH_OUTSIDE_AUTHORIZED_ROOT",
                },
                SecurePathDiagnostic {
                    code: SOURCE_ACCESS_DENIED,
                    name: "SOURCE_ACCESS_DENIED",
                },
                SecurePathDiagnostic {
                    code: BLOCKED_PATH_COLLISION,
                    name: "BLOCKED_PATH_COLLISION",
                },
                SecurePathDiagnostic {
                    code: WORKSPACE_NOT_AUTHORIZED,
                    name: "WORKSPACE_NOT_AUTHORIZED",
                },
            ]
        );
        let path = PlatformPath::from_raw_relative_bytes(platform, b"source.rs".to_vec()).unwrap();
        assert_eq!(
            read_workspace_source(&mut store, record.workspace_id, &path)
                .unwrap()
                .bytes,
            b"source"
        );
    }
}
