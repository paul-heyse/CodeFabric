//! Closed family-driver protocol and staging-only write capability.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::model_control::StableId;
use super::repository_model::read_stable;

const MAX_DRIVER_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Return a process-isolated disposable staging path for a model operation.
///
/// Multiple read-only model checks may run concurrently. They must not share mutable
/// staging directories even though none of them writes the governed repository tree.
#[must_use]
pub fn process_stage_root(repository_root: &Path, name: &str) -> PathBuf {
    repository_root
        .join("target/model-stage/processes")
        .join(std::process::id().to_string())
        .join(name)
}

/// Resource limits named by a family driver before it renders.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverResourceProfile {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_outputs: usize,
}

/// One complete output declaration made before rendering.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverOutputSpec {
    pub output_id: StableId,
    pub path: SafeOutputPath,
    pub role: DriverOutputRole,
}

/// Closed output roles for the model-driver protocol.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverOutputRole {
    RustBinding,
    PythonBinding,
    ProtoDescriptor,
    DescriptorCensus,
    ToolchainIdentity,
    CanonicalProjection,
    PublicJsonSchema,
    SqliteDdl,
    TableSpec,
    ValidationReport,
    TransitionOverlay,
}

/// Read/output/resource declaration returned without reading source bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverDescriptor {
    pub driver_id: StableId,
    pub family: StableId,
    pub rule_version: String,
    pub sources: Vec<SafeOutputPath>,
    /// Closed roots below which `plan` may resolve authority-declared exact outputs.
    pub output_roots: Vec<SafeOutputPath>,
    pub outputs: Vec<DriverOutputSpec>,
    pub resource_profile: DriverResourceProfile,
}

impl DriverDescriptor {
    /// Validate declaration closure and authority boundaries.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty rule, duplicate source/output, or unsafe output count.
    pub fn validate(&self) -> Result<(), DriverProtocolError> {
        if self.rule_version.trim().is_empty()
            || self.sources.len() > self.resource_profile.max_outputs.saturating_mul(8)
            || self.outputs.len() > self.resource_profile.max_outputs
        {
            return Err(DriverProtocolError::InvalidDescriptor);
        }
        let sources: BTreeSet<_> = self.sources.iter().collect();
        let output_ids: BTreeSet<_> = self
            .outputs
            .iter()
            .map(|output| &output.output_id)
            .collect();
        let output_paths: BTreeSet<_> = self.outputs.iter().map(|output| &output.path).collect();
        let output_roots: BTreeSet<_> = self.output_roots.iter().collect();
        if sources.len() != self.sources.len()
            || output_ids.len() != self.outputs.len()
            || output_paths.len() != self.outputs.len()
            || output_roots.len() != self.output_roots.len()
        {
            return Err(DriverProtocolError::InvalidDescriptor);
        }
        Ok(())
    }
}

/// Ambient environment projected into a closed external-driver input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverEnvironment {
    pub variables: BTreeMap<String, String>,
}

impl DriverEnvironment {
    /// Keep only deterministic, non-credential process settings.
    #[must_use]
    pub fn sanitized<I, K, V>(ambient: I, staging_home: &Path) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        const ALLOWED: &[&str] = &[
            "LANG",
            "LC_ALL",
            "PATH",
            "PYTHONUTF8",
            "SOURCE_DATE_EPOCH",
            "SYSTEMROOT",
            "TEMP",
            "TMP",
            "TMPDIR",
        ];
        let mut variables = ambient
            .into_iter()
            .filter_map(|(name, value)| {
                let name = name.as_ref();
                ALLOWED
                    .contains(&name)
                    .then(|| (name.to_owned(), value.as_ref().to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        variables.insert(
            "HOME".to_owned(),
            staging_home.to_string_lossy().into_owned(),
        );
        variables.insert("NO_PROXY".to_owned(), "*".to_owned());
        variables.insert("no_proxy".to_owned(), "*".to_owned());
        Self { variables }
    }
}

/// Source identities fenced around one render.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverSourceFence {
    pub digests: BTreeMap<SafeOutputPath, String>,
}

impl DriverSourceFence {
    /// Capture stable bytes for every declared source.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read or exceeds the declared budget.
    pub fn capture(
        root: &Path,
        descriptor: &DriverDescriptor,
    ) -> Result<Self, DriverProtocolError> {
        let mut total = 0_usize;
        let mut digests = BTreeMap::new();
        for source in &descriptor.sources {
            let bytes = read_stable(
                &root.join(source_path(source)?),
                descriptor.resource_profile.max_source_bytes,
            )?;
            total = total
                .checked_add(bytes.len())
                .ok_or(DriverProtocolError::ResourceLimit)?;
            if total > descriptor.resource_profile.max_source_bytes {
                return Err(DriverProtocolError::ResourceLimit);
            }
            digests.insert(source.clone(), digest_bytes(&bytes));
        }
        Ok(Self { digests })
    }

    /// Verify every declared source still has the captured bytes.
    ///
    /// # Errors
    ///
    /// Returns the first source whose stable bytes changed.
    pub fn verify(&self, root: &Path) -> Result<(), DriverProtocolError> {
        for (source, expected) in &self.digests {
            let bytes = read_stable(&root.join(source_path(source)?), MAX_DRIVER_SOURCE_BYTES)?;
            if digest_bytes(&bytes) != *expected {
                return Err(DriverProtocolError::SourceChanged(source.display()));
            }
        }
        Ok(())
    }
}

/// The only filesystem write capability supplied to a driver.
#[derive(Debug)]
pub struct StagingRoot {
    root: PathBuf,
    allowed: BTreeMap<SafeOutputPath, DriverOutputSpec>,
    max_output_bytes: usize,
}

impl StagingRoot {
    /// Construct a staging capability below `target/model-stage`.
    ///
    /// # Errors
    ///
    /// Returns an error if the root escapes the disposable staging namespace.
    pub fn new(
        repository_root: &Path,
        staging_root: &Path,
        descriptor: &DriverDescriptor,
    ) -> Result<Self, DriverProtocolError> {
        descriptor.validate()?;
        let repository_root =
            repository_root
                .canonicalize()
                .map_err(|source| DriverProtocolError::Io {
                    path: repository_root.to_owned(),
                    source,
                })?;
        let required = repository_root.join("target/model-stage");
        fs::create_dir_all(&required).map_err(|source| DriverProtocolError::Io {
            path: required.clone(),
            source,
        })?;
        let required = required
            .canonicalize()
            .map_err(|source| DriverProtocolError::Io {
                path: required.clone(),
                source,
            })?;
        fs::create_dir_all(staging_root).map_err(|source| DriverProtocolError::Io {
            path: staging_root.to_owned(),
            source,
        })?;
        let root = staging_root
            .canonicalize()
            .map_err(|source| DriverProtocolError::Io {
                path: staging_root.to_owned(),
                source,
            })?;
        if !root.starts_with(&required) || root == required {
            return Err(DriverProtocolError::StagingEscape(root));
        }
        Ok(Self {
            root,
            allowed: descriptor
                .outputs
                .iter()
                .cloned()
                .map(|output| (output.path.clone(), output))
                .collect(),
            max_output_bytes: descriptor.resource_profile.max_output_bytes,
        })
    }

    /// Write one predeclared output below the staging root.
    ///
    /// # Errors
    ///
    /// Returns an error for undeclared paths, oversized bytes, symlink ancestors, or I/O failure.
    pub fn write(&self, path: &SafeOutputPath, bytes: &[u8]) -> Result<(), DriverProtocolError> {
        if !self.allowed.contains_key(path) {
            return Err(DriverProtocolError::UndeclaredOutput(path.display()));
        }
        if bytes.len() > self.max_output_bytes {
            return Err(DriverProtocolError::ResourceLimit);
        }
        let relative = source_path(path)?;
        let destination = self.root.join(&relative);
        let parent = destination
            .parent()
            .ok_or_else(|| DriverProtocolError::UndeclaredOutput(path.display()))?;
        reject_symlink_ancestors(&self.root, parent)?;
        fs::create_dir_all(parent).map_err(|source| DriverProtocolError::Io {
            path: parent.to_owned(),
            source,
        })?;
        reject_symlink_ancestors(&self.root, parent)?;
        if destination
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(DriverProtocolError::Symlink(destination));
        }
        fs::write(&destination, bytes).map_err(|source| DriverProtocolError::Io {
            path: destination,
            source,
        })
    }

    /// Resolve a declared staged output for a consumer compiler.
    ///
    /// # Errors
    ///
    /// Returns an error when the path was not declared.
    pub fn output_path(&self, path: &SafeOutputPath) -> Result<PathBuf, DriverProtocolError> {
        if !self.allowed.contains_key(path) {
            return Err(DriverProtocolError::UndeclaredOutput(path.display()));
        }
        Ok(self.root.join(source_path(path)?))
    }

    /// Borrow the staging root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }
}

/// Closed family-driver lifecycle.
pub trait ModelDriver {
    type Plan;

    /// Declare every source, output, and resource bound without reading source bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the closed declaration is invalid.
    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError>;

    /// Resolve stable typed inputs without writing repository state.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read or violates its family authority.
    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError>;

    /// Render only predeclared outputs through the staging capability.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid typed input, resource excess, or an attempted write outside
    /// the resolved output plan.
    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError>;
}

/// Format generated Rust with the repository toolchain before it enters `DesiredTree`.
///
/// # Errors
///
/// Returns a protocol error when rustfmt cannot be started, rejects the source, or emits no
/// successful output. The caller's action identity already binds the exact Rust toolchain.
pub fn rustfmt_source(bytes: &[u8]) -> Result<Vec<u8>, DriverProtocolError> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| DriverProtocolError::ExternalTool {
            tool: "rustfmt",
            detail: source.to_string(),
        })?;
    child
        .stdin
        .take()
        .ok_or_else(|| DriverProtocolError::ExternalTool {
            tool: "rustfmt",
            detail: "stdin is unavailable".to_owned(),
        })?
        .write_all(bytes)
        .map_err(|source| DriverProtocolError::ExternalTool {
            tool: "rustfmt",
            detail: source.to_string(),
        })?;
    let output = child
        .wait_with_output()
        .map_err(|source| DriverProtocolError::ExternalTool {
            tool: "rustfmt",
            detail: source.to_string(),
        })?;
    if !output.status.success() {
        return Err(DriverProtocolError::ExternalTool {
            tool: "rustfmt",
            detail: String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(512)
                .collect(),
        });
    }
    Ok(output.stdout)
}

fn source_path(path: &SafeOutputPath) -> Result<PathBuf, DriverProtocolError> {
    let path = PathBuf::from(std::ffi::OsString::from(
        String::from_utf8(path.as_bytes().to_vec())
            .map_err(|_| DriverProtocolError::NonUtf8ProtocolPath)?,
    ));
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DriverProtocolError::NonUtf8ProtocolPath);
    }
    Ok(path)
}

fn reject_symlink_ancestors(root: &Path, parent: &Path) -> Result<(), DriverProtocolError> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| DriverProtocolError::StagingEscape(parent.to_owned()))?;
    let mut cursor = root.to_owned();
    for component in relative.components() {
        cursor.push(component);
        if cursor
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(DriverProtocolError::Symlink(cursor));
        }
    }
    Ok(())
}

/// Resolve an external executable by PATH and bind both its bytes and reported version.
/// Paths remain diagnostic; action identity never trusts a path alone.
///
/// # Errors
///
/// Returns an error when the executable cannot be resolved/read or its version query fails.
pub fn executable_tool_identity(
    program: &str,
    version_arguments: &[&str],
) -> Result<Value, DriverProtocolError> {
    let path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| DriverProtocolError::ExternalTool {
            tool: "executable-identity",
            detail: format!("{program} is absent from PATH"),
        })?;
    let canonical = path
        .canonicalize()
        .map_err(|source| DriverProtocolError::Io {
            path: path.clone(),
            source,
        })?;
    let bytes = read_stable(&canonical, MAX_DRIVER_SOURCE_BYTES)?;
    let output = Command::new(&path)
        .args(version_arguments)
        .output()
        .map_err(|source| DriverProtocolError::ExternalTool {
            tool: "executable-identity",
            detail: source.to_string(),
        })?;
    if !output.status.success() {
        return Err(DriverProtocolError::ExternalTool {
            tool: "executable-identity",
            detail: format!("{program} version query failed"),
        });
    }
    Ok(serde_json::json!({
        "program": program,
        "resolved_path": path,
        "canonical_path": canonical,
        "executable_digest": digest_bytes(&bytes),
        "version_stdout": String::from_utf8_lossy(&output.stdout).trim(),
        "version_stderr": String::from_utf8_lossy(&output.stderr).trim(),
    }))
}

/// Configure a Cargo subprocess so debug artifacts are byte-identical across worktree roots.
pub fn configure_reproducible_cargo_build(command: &mut Command, repository_root: &Path) {
    command
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("RUSTFLAGS")
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!(
                "--remap-path-prefix={}=/codefabric\u{1f}-Cstrip=debuginfo",
                repository_root.display()
            ),
        );
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

/// Driver protocol failures.
#[derive(Debug, Error)]
pub enum DriverProtocolError {
    #[error("invalid driver descriptor")]
    InvalidDescriptor,
    #[error("driver resource limit exceeded")]
    ResourceLimit,
    #[error("driver protocol paths must be UTF-8 normal components")]
    NonUtf8ProtocolPath,
    #[error("staging root escapes target/model-stage: {0}")]
    StagingEscape(PathBuf),
    #[error("driver attempted undeclared output {0}")]
    UndeclaredOutput(String),
    #[error("driver staging path contains a symlink: {0}")]
    Symlink(PathBuf),
    #[error("driver source changed during render: {0}")]
    SourceChanged(String),
    #[error("driver authority is invalid: {0}")]
    InvalidAuthority(String),
    #[error("driver external tool {tool} failed: {detail}")]
    ExternalTool { tool: &'static str, detail: String },
    #[error("driver I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> DriverDescriptor {
        DriverDescriptor {
            driver_id: StableId::parse("driver:test").unwrap(),
            family: StableId::parse("family:test").unwrap(),
            rule_version: "v1".to_owned(),
            sources: vec![],
            output_roots: vec![],
            outputs: vec![DriverOutputSpec {
                output_id: StableId::parse("output:test").unwrap(),
                path: SafeOutputPath::parse(b"src/generated/test.rs".to_vec()).unwrap(),
                role: DriverOutputRole::RustBinding,
            }],
            resource_profile: DriverResourceProfile {
                max_source_bytes: 1024,
                max_output_bytes: 1024,
                max_outputs: 4,
            },
        }
    }

    #[test]
    fn model_registry_driver_rejects_out_of_plan_output_and_repository_write() {
        let temporary = std::env::temp_dir().join(format!(
            "codefabric-model-driver-protocol-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        if temporary.exists() {
            fs::remove_dir_all(&temporary).unwrap();
        }
        fs::create_dir_all(temporary.join("target/model-stage/family")).unwrap();
        let staging = StagingRoot::new(
            &temporary,
            &temporary.join("target/model-stage/family"),
            &descriptor(),
        )
        .unwrap();
        let undeclared = SafeOutputPath::parse(b"src/generated/other.rs".to_vec()).unwrap();
        assert!(matches!(
            staging.write(&undeclared, b"bad"),
            Err(DriverProtocolError::UndeclaredOutput(_))
        ));
        assert!(StagingRoot::new(&temporary, &temporary, &descriptor()).is_err());
        assert!(!temporary.join("src/generated/test.rs").exists());
        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn model_registry_driver_environment_strips_credentials_and_proxy_settings() {
        let environment = DriverEnvironment::sanitized(
            [
                ("PATH", "/bin"),
                ("AWS_SECRET_ACCESS_KEY", "secret"),
                ("HTTPS_PROXY", "https://proxy.invalid"),
                ("CODEFABRIC_TOKEN", "token"),
            ],
            Path::new("/stage/home"),
        );
        assert_eq!(
            environment.variables.get("PATH").map(String::as_str),
            Some("/bin")
        );
        assert_eq!(
            environment.variables.get("NO_PROXY").map(String::as_str),
            Some("*")
        );
        assert_eq!(
            environment.variables.get("no_proxy").map(String::as_str),
            Some("*")
        );
        assert!(!environment.variables.keys().any(|name| {
            let name = name.to_ascii_lowercase();
            name.contains("secret")
                || name.contains("token")
                || matches!(name.as_str(), "http_proxy" | "https_proxy" | "all_proxy")
        }));
    }

    #[test]
    fn model_cargo_tool_builds_use_a_closed_reproducible_path_contract() {
        let mut command = Command::new("cargo");
        command.env("RUSTFLAGS", "ambient-flag");
        configure_reproducible_cargo_build(&mut command, Path::new("/tmp/source root"));
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            environment
                .get(std::ffi::OsStr::new("CARGO_INCREMENTAL"))
                .and_then(Option::as_deref),
            Some(std::ffi::OsStr::new("0"))
        );
        assert_eq!(
            environment
                .get(std::ffi::OsStr::new("CARGO_ENCODED_RUSTFLAGS"))
                .and_then(Option::as_deref),
            Some(std::ffi::OsStr::new(
                "--remap-path-prefix=/tmp/source root=/codefabric\u{1f}-Cstrip=debuginfo"
            ))
        );
        assert_eq!(
            environment.get(std::ffi::OsStr::new("RUSTFLAGS")),
            Some(&None)
        );
    }
}
