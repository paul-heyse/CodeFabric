//! Fail-closed semantic-provider containment and process-launch substrate.

use std::collections::BTreeMap;
use std::fs;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Stable reason returned when the requested containment profile cannot be proved.
pub const SANDBOX_UNAVAILABLE_REASON: &str = "SANDBOX_UNAVAILABLE";
static SANDBOX_PROBE_NONCE: AtomicU64 = AtomicU64::new(0);

const PROVIDER_LAUNCH_SHELL: &str = r#"
preserve="$1"
cpu="$2"
open_files="$3"
file_blocks="$4"
shift 4
for descriptor in /dev/fd/*; do
  fd="${descriptor##*/}"
  case "$fd" in
    ''|*[!0-9]*|0|1|2|"$preserve") continue ;;
  esac
  eval "exec ${fd}>&-" 2>/dev/null || :
done
ulimit -t "$cpu"
ulimit -n "$open_files"
ulimit -f "$file_blocks"
exec "$@"
"#;

/// Provider trust is explicit input, never inferred from provider placement.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderTrustProfile {
    UntrustedSandboxed,
    TrustedLocal,
    ParsingOnly,
}

/// Host mechanism selected by the governed platform contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SandboxMechanism {
    DarwinSeatbelt,
    LinuxBubblewrap,
    None,
}

/// Exact observation used to decide whether a host may advertise containment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SandboxProbeObservation {
    pub mechanism: SandboxMechanism,
    pub executable_path: PathBuf,
    pub executable_version: String,
    pub owned_by_root: bool,
    pub executable_mode: u32,
    pub setuid: bool,
    pub behavior: BTreeMap<String, bool>,
}

/// Fail-closed capability row pinned into daemon capability reporting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SandboxCapabilityRow {
    pub trust_profile: ProviderTrustProfile,
    pub mechanism: SandboxMechanism,
    pub available: bool,
    pub reason_code: String,
    pub probe_digest: String,
}

/// Complete host matrix. Untrusted execution is advertised only after every required probe passes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SandboxCapabilityMatrix {
    pub rows: Vec<SandboxCapabilityRow>,
}

impl SandboxCapabilityMatrix {
    /// Evaluate one immutable observation without probing or launching as a side effect.
    #[must_use]
    pub fn evaluate(observation: &SandboxProbeObservation) -> Self {
        let mut required = vec![
            "launch-confined",
            "leased-workspace-read-allowed",
            "workspace-write-denied",
            "out-of-root-write-denied",
            "live-workspace-read-denied",
            "credential-read-denied",
            "git-read-denied",
            "network-denied",
            "inherited-fd-read-denied",
            "child-process-contained",
            "resource-limit-enforceable",
            "cleanup-escape-denied",
            "output-write-allowed",
        ];
        if observation.mechanism == SandboxMechanism::LinuxBubblewrap {
            required.push("seccomp-active");
        }
        let exact_identity = match observation.mechanism {
            SandboxMechanism::DarwinSeatbelt => {
                observation.executable_path == Path::new("/usr/bin/sandbox-exec")
                    && observation.owned_by_root
                    && observation.executable_mode & 0o022 == 0
                    && !observation.setuid
            }
            SandboxMechanism::LinuxBubblewrap => {
                observation.executable_path == Path::new("/usr/bin/bwrap")
                    && observation.executable_version.trim() == "bubblewrap 0.11.2"
                    && observation.owned_by_root
                    && observation.executable_mode & 0o022 == 0
                    && !observation.setuid
            }
            SandboxMechanism::None => false,
        };
        let behavior_proved = required
            .iter()
            .all(|probe| observation.behavior.get(*probe).copied() == Some(true));
        let probe_digest = sha256_json(observation).unwrap_or_else(|_| "sha256:invalid".into());
        let available = exact_identity && behavior_proved;
        let untrusted_reason = if available {
            "SANDBOX_PROVED"
        } else if !exact_identity {
            "SANDBOX_IDENTITY_UNPROVED"
        } else {
            "SANDBOX_BEHAVIOR_UNPROVED"
        };
        Self {
            rows: vec![
                SandboxCapabilityRow {
                    trust_profile: ProviderTrustProfile::UntrustedSandboxed,
                    mechanism: observation.mechanism,
                    available,
                    reason_code: untrusted_reason.into(),
                    probe_digest: probe_digest.clone(),
                },
                SandboxCapabilityRow {
                    trust_profile: ProviderTrustProfile::TrustedLocal,
                    mechanism: SandboxMechanism::None,
                    available: true,
                    reason_code: "TRUSTED_LOCAL_WEAKER_ISOLATION".into(),
                    probe_digest: probe_digest.clone(),
                },
                SandboxCapabilityRow {
                    trust_profile: ProviderTrustProfile::ParsingOnly,
                    mechanism: SandboxMechanism::None,
                    available: true,
                    reason_code: "PARSING_ONLY_NO_SEMANTIC_CHILD".into(),
                    probe_digest,
                },
            ],
        }
    }

    /// Resolve one trust profile from the closed matrix.
    #[must_use]
    pub fn row(&self, profile: ProviderTrustProfile) -> Option<&SandboxCapabilityRow> {
        self.rows.iter().find(|row| row.trust_profile == profile)
    }
}

/// Probe the current host using the exact governed executable path and behavioral controls.
/// Probe failure is data: callers receive an unavailable observation rather than an error that
/// could tempt a fallback launch.
#[must_use]
pub fn probe_host_sandbox() -> SandboxProbeObservation {
    #[cfg(target_os = "macos")]
    {
        return probe_darwin_seatbelt();
    }
    #[cfg(target_os = "linux")]
    {
        return probe_linux_bubblewrap();
    }
    #[allow(unreachable_code)]
    SandboxProbeObservation {
        mechanism: SandboxMechanism::None,
        executable_path: PathBuf::new(),
        executable_version: "unsupported-host".into(),
        owned_by_root: false,
        executable_mode: 0,
        setuid: false,
        behavior: BTreeMap::new(),
    }
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_lines)] // The advertised host row must be derived from one auditable escape-probe transaction.
fn probe_darwin_seatbelt() -> SandboxProbeObservation {
    let executable = PathBuf::from("/usr/bin/sandbox-exec");
    let metadata = fs::metadata(&executable).ok();
    let probe_nonce = SANDBOX_PROBE_NONCE.fetch_add(1, Ordering::Relaxed);
    let probe_root = std::env::temp_dir().join(format!(
        "codefabric-seatbelt-probe-{}-{probe_nonce}",
        std::process::id()
    ));
    let behavior = (|| -> Result<BTreeMap<String, bool>, SandboxError> {
        let view = probe_root.join("view");
        let dependencies = probe_root.join("dependencies");
        let output = probe_root.join("output");
        fs::create_dir_all(view.join(".git"))?;
        fs::create_dir_all(&dependencies)?;
        fs::create_dir_all(&output)?;
        fs::write(view.join(".git/config"), b"probe")?;
        fs::write(view.join("leased.py"), b"leased")?;
        let live_workspace = probe_root.join("live-workspace.py");
        let credential = probe_root.join("credential-token");
        let outside_write = probe_root.join("outside-write");
        let cleanup_escape = probe_root.join("cleanup-escape");
        fs::write(&live_workspace, b"live")?;
        fs::write(&credential, b"credential")?;
        let view = fs::canonicalize(view)?;
        let dependencies = fs::canonicalize(dependencies)?;
        let output = fs::canonicalize(output)?;
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::DarwinSeatbelt,
            &view,
            &dependencies,
            &output,
        )?;
        let profile_path = profile.materialize(&probe_root.join("profiles"))?;
        let run = |program: &str, arguments: &[&str]| {
            Command::new(&executable)
                .arg("-f")
                .arg(&profile_path)
                .arg(program)
                .args(arguments)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        let forbidden_write = view.join("forbidden");
        let allowed_write = output.join("allowed");
        let git_config = view.join(".git/config");
        let leased_source = view.join("leased.py");
        let network_denied = Path::new("/usr/bin/python3").is_file()
            && !run(
                "/usr/bin/python3",
                &["-c", "import socket; socket.socket()"],
            );
        let inherited_fd_program = format!("exec 9<\"$1\"; shift; {PROVIDER_LAUNCH_SHELL}");
        Ok(BTreeMap::from([
            ("launch-confined".into(), run("/usr/bin/true", &[])),
            (
                "workspace-write-denied".into(),
                !run(
                    "/bin/sh",
                    &[
                        "-c",
                        "printf probe > \"$1\"",
                        "probe",
                        &forbidden_write.to_string_lossy(),
                    ],
                ) && !forbidden_write.exists(),
            ),
            (
                "leased-workspace-read-allowed".into(),
                run("/bin/cat", &[&leased_source.to_string_lossy()]),
            ),
            (
                "live-workspace-read-denied".into(),
                !run("/bin/cat", &[&live_workspace.to_string_lossy()]),
            ),
            (
                "credential-read-denied".into(),
                !run("/bin/cat", &[&credential.to_string_lossy()]),
            ),
            (
                "git-read-denied".into(),
                !run("/bin/cat", &[&git_config.to_string_lossy()]),
            ),
            ("network-denied".into(), network_denied),
            (
                "inherited-fd-read-denied".into(),
                !Command::new("/bin/sh")
                    .args([
                        "-c",
                        &inherited_fd_program,
                        "probe",
                        &credential.to_string_lossy(),
                        "",
                        "10",
                        "64",
                        "2",
                        &executable.to_string_lossy(),
                        "-f",
                        &profile_path.to_string_lossy(),
                        "/bin/cat",
                        "/dev/fd/9",
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success()),
            ),
            (
                "child-process-contained".into(),
                !run(
                    "/bin/sh",
                    &[
                        "-c",
                        "exec /bin/sh -c 'exec /bin/cat \"$1\"' child \"$1\"",
                        "child",
                        &credential.to_string_lossy(),
                    ],
                ),
            ),
            (
                "resource-limit-enforceable".into(),
                run(
                    "/bin/sh",
                    &["-c", "ulimit -n 64; test \"$(ulimit -n)\" -eq 64"],
                ),
            ),
            (
                "cleanup-escape-denied".into(),
                run(
                    "/bin/sh",
                    &[
                        "-c",
                        "trap 'printf escaped > \"$1\"' EXIT; exit 0",
                        "cleanup",
                        &cleanup_escape.to_string_lossy(),
                    ],
                ) && !cleanup_escape.exists(),
            ),
            (
                "out-of-root-write-denied".into(),
                !run(
                    "/bin/sh",
                    &[
                        "-c",
                        "printf probe > \"$1\"",
                        "outside",
                        &outside_write.to_string_lossy(),
                    ],
                ) && !outside_write.exists(),
            ),
            (
                "output-write-allowed".into(),
                run(
                    "/bin/sh",
                    &[
                        "-c",
                        "printf probe > \"$1\"",
                        "probe",
                        &allowed_write.to_string_lossy(),
                    ],
                ) && fs::read(&allowed_write).is_ok_and(|bytes| bytes == b"probe"),
            ),
        ]))
    })()
    .unwrap_or_default();
    let _ = fs::remove_dir_all(&probe_root);
    SandboxProbeObservation {
        mechanism: SandboxMechanism::DarwinSeatbelt,
        executable_path: executable,
        executable_version: "darwin-seatbelt-system".into(),
        owned_by_root: metadata.as_ref().is_some_and(|value| value.uid() == 0),
        executable_mode: metadata.as_ref().map_or(0, |value| value.mode() & 0o7777),
        setuid: metadata
            .as_ref()
            .is_some_and(|value| value.mode() & 0o4000 != 0),
        behavior,
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_bubblewrap() -> SandboxProbeObservation {
    let executable = PathBuf::from("/usr/bin/bwrap");
    let metadata = fs::metadata(&executable).ok();
    let version = Command::new(&executable)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into());
    // WP16 installs and escape-tests the lane-specific compiled seccomp descriptor. Until that
    // descriptor is present, the shared substrate intentionally advertises no untrusted Linux
    // capability even when bubblewrap itself has the exact identity.
    SandboxProbeObservation {
        mechanism: SandboxMechanism::LinuxBubblewrap,
        executable_path: executable,
        executable_version: version,
        owned_by_root: metadata.as_ref().is_some_and(|value| value.uid() == 0),
        executable_mode: metadata.as_ref().map_or(0, |value| value.mode() & 0o7777),
        setuid: metadata
            .as_ref()
            .is_some_and(|value| value.mode() & 0o4000 != 0),
        behavior: BTreeMap::new(),
    }
}

/// Canonical profile bytes and the digest recorded in provider-run/snapshot provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedSandboxProfile {
    pub trust_profile: ProviderTrustProfile,
    pub mechanism: SandboxMechanism,
    pub bytes: Vec<u8>,
    pub sha256_digest: String,
    pub workspace_view: PathBuf,
    pub dependency_root: PathBuf,
    pub output_root: PathBuf,
}

impl GeneratedSandboxProfile {
    /// Generate a deterministic platform profile over an immutable input view and distinct output.
    ///
    /// # Errors
    ///
    /// Rejects relative or overlapping roots and profiles without a containment mechanism.
    pub fn generate(
        trust_profile: ProviderTrustProfile,
        mechanism: SandboxMechanism,
        workspace_view: &Path,
        dependency_root: &Path,
        output_root: &Path,
    ) -> Result<Self, SandboxError> {
        for path in [workspace_view, dependency_root, output_root] {
            if !path.is_absolute() {
                return Err(SandboxError::InvalidProfile(
                    "sandbox roots must be absolute",
                ));
            }
        }
        let workspace_view = fs::canonicalize(workspace_view)?;
        let dependency_root = fs::canonicalize(dependency_root)?;
        let output_root = fs::canonicalize(output_root)?;
        if roots_overlap(&workspace_view, &output_root)
            || roots_overlap(&dependency_root, &output_root)
        {
            return Err(SandboxError::InvalidProfile(
                "provider output must be separate from immutable inputs",
            ));
        }
        let bytes = match (trust_profile, mechanism) {
            (ProviderTrustProfile::UntrustedSandboxed, SandboxMechanism::DarwinSeatbelt) => {
                darwin_profile(&workspace_view, &dependency_root, &output_root).into_bytes()
            }
            (ProviderTrustProfile::UntrustedSandboxed, SandboxMechanism::LinuxBubblewrap) => {
                linux_profile(&workspace_view, &dependency_root, &output_root).into_bytes()
            }
            (
                ProviderTrustProfile::TrustedLocal | ProviderTrustProfile::ParsingOnly,
                SandboxMechanism::None,
            ) => format!("profile={trust_profile:?}\nmechanism=NONE\n").into_bytes(),
            _ => return Err(SandboxError::InvalidProfile("trust and mechanism differ")),
        };
        let sha256_digest = sha256_bytes(&bytes);
        Ok(Self {
            trust_profile,
            mechanism,
            bytes,
            sha256_digest,
            workspace_view,
            dependency_root,
            output_root,
        })
    }

    /// Materialize immutable profile bytes under the daemon-owned state root.
    ///
    /// # Errors
    ///
    /// Rejects pre-existing bytes that differ or any permission/durability failure.
    pub fn materialize(&self, state_root: &Path) -> Result<PathBuf, SandboxError> {
        fs::create_dir_all(state_root)?;
        fs::set_permissions(state_root, fs::Permissions::from_mode(0o700))?;
        let name = self.sha256_digest.replace(':', "-");
        let path = state_root.join(format!("{name}.sb"));
        if path.exists() {
            let existing = fs::read(&path)?;
            if existing != self.bytes {
                return Err(SandboxError::ProfileDigestMismatch);
            }
            return Ok(path);
        }
        let temporary = state_root.join(format!(".{name}-{}.tmp", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options.open(&temporary)?;
        std::io::Write::write_all(&mut file, &self.bytes)?;
        file.sync_all()?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o400))?;
        fs::rename(&temporary, &path)?;
        fs::File::open(state_root)?.sync_all()?;
        Ok(path)
    }
}

/// App-owned limits inherited by the provider and its process group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderProcessLimits {
    pub cpu_seconds: u64,
    pub open_files: u64,
    pub address_space_bytes: u64,
    pub output_file_bytes: u64,
}

/// Closed launch request. Environment and standard descriptors are rebuilt by the launcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderLaunchRequest {
    /// Canonical host path whose immutable file identity is validated before launch.
    pub host_executable: PathBuf,
    /// Exact executable spelling inside the selected containment namespace.
    pub contained_executable: PathBuf,
    pub arguments: Vec<String>,
    /// Complete environment installed after `env_clear`.
    pub environment: BTreeMap<String, String>,
    pub output_root: PathBuf,
    pub limits: ProviderProcessLimits,
}

/// Platform-specific launch material. Linux accepts an already compiled seccomp descriptor,
/// never a path that bubblewrap would misinterpret as an inherited file descriptor.
#[derive(Clone, Copy)]
pub enum ProviderSandboxLaunchMaterial<'a> {
    DarwinProfile(&'a Path),
    LinuxSeccomp(&'a fs::File),
    None,
}

/// One provider child and the run-scoped process group created at spawn time.
///
/// The group leader PID is never accepted from provider output. It is captured from the child
/// created by the sole launcher after `process_group(0)` has been installed on the command.
#[derive(Debug)]
pub struct ProviderProcessGroupChild {
    child: Child,
    process_group_id: rustix::process::Pid,
}

impl ProviderProcessGroupChild {
    fn new(mut child: Child) -> Result<Self, SandboxError> {
        let process_group_id = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .ok_or(SandboxError::InvalidLaunch)?;
        if rustix::process::getpgid(Some(process_group_id)) != Ok(process_group_id) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SandboxError::ProcessGroupUnavailable);
        }
        Ok(Self {
            child,
            process_group_id,
        })
    }

    /// OS child identity, which is also the run-scoped process-group identity.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Observe and reap the group leader without blocking.
    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Wait for and reap the group leader.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Take the captured standard-error stream exactly once.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Take the captured standard-output stream exactly once.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    /// Numeric identity of the process group created by this launcher.
    ///
    /// The value is an observation input for the daemon-owned resource supervisor. It is never
    /// accepted from provider output and must not be used to signal an unrelated process.
    #[must_use]
    pub fn process_group_id(&self) -> i32 {
        self.process_group_id.as_raw_nonzero().get()
    }

    /// Send a graceful termination signal to the complete run-scoped group.
    pub fn terminate_group(&mut self) -> std::io::Result<()> {
        self.signal_group(rustix::process::Signal::TERM)
    }

    /// Send an unconditional kill signal to the complete run-scoped group.
    pub fn kill_group(&mut self) -> std::io::Result<()> {
        self.signal_group(rustix::process::Signal::KILL)
    }

    /// Wait until the complete process group no longer exists, reaping its leader as it exits.
    pub fn wait_group_empty(&mut self, timeout: Duration) -> std::io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            let _ = self.child.try_wait()?;
            match rustix::process::test_kill_process_group(self.process_group_id) {
                Ok(()) => {}
                Err(error) if error == rustix::io::Errno::SRCH => return Ok(true),
                // Darwin can transiently report EPERM while a just-signalled group is being
                // dismantled. It is not proof of emptiness: keep polling until ESRCH or timeout.
                Err(error) if error == rustix::io::Errno::PERM => {}
                Err(error) => return Err(error.into()),
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn signal_group(&mut self, signal: rustix::process::Signal) -> std::io::Result<()> {
        match rustix::process::kill_process_group(self.process_group_id, signal) {
            Ok(()) => Ok(()),
            Err(error) if error == rustix::io::Errno::SRCH => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for ProviderProcessGroupChild {
    fn drop(&mut self) {
        // A launcher-owned group must not outlive the object that proves and supervises it. This
        // is a last-resort guard; the trust supervisor performs the graceful, receipted sequence.
        let _ = self.kill_group();
        let _ = self.child.wait();
    }
}

/// The sole semantic-child launcher. Untrusted execution has no unsandboxed fallback.
pub struct ProviderSandboxLauncher {
    matrix: SandboxCapabilityMatrix,
}

impl ProviderSandboxLauncher {
    #[must_use]
    pub const fn new(matrix: SandboxCapabilityMatrix) -> Self {
        Self { matrix }
    }

    /// Launch one child with an explicit trust decision and inherited resource limits.
    ///
    /// # Errors
    ///
    /// Fails closed when containment is unavailable, profile identity differs, or launch fails.
    #[allow(clippy::too_many_lines)] // Keep the fail-closed launch sequence auditable as one transaction.
    pub fn launch(
        &self,
        request: &ProviderLaunchRequest,
        profile: &GeneratedSandboxProfile,
        material: ProviderSandboxLaunchMaterial<'_>,
    ) -> Result<ProviderProcessGroupChild, SandboxError> {
        self.launch_with_output(request, profile, material, false)
    }

    /// Launch one child with both output streams reserved for a bounded daemon supervisor.
    ///
    /// This is separate from [`Self::launch`] so long-lived providers that own their own protocol
    /// transport do not accidentally acquire an undrained stdout pipe.
    pub fn launch_captured(
        &self,
        request: &ProviderLaunchRequest,
        profile: &GeneratedSandboxProfile,
        material: ProviderSandboxLaunchMaterial<'_>,
    ) -> Result<ProviderProcessGroupChild, SandboxError> {
        self.launch_with_output(request, profile, material, true)
    }

    #[allow(clippy::too_many_lines)] // Keep the fail-closed launch sequence auditable as one transaction.
    fn launch_with_output(
        &self,
        request: &ProviderLaunchRequest,
        profile: &GeneratedSandboxProfile,
        material: ProviderSandboxLaunchMaterial<'_>,
        capture_stdout: bool,
    ) -> Result<ProviderProcessGroupChild, SandboxError> {
        let row = self
            .matrix
            .row(profile.trust_profile)
            .ok_or(SandboxError::SandboxUnavailable)?;
        if !row.available {
            return Err(SandboxError::SandboxUnavailable);
        }
        if !request.host_executable.is_absolute()
            || !request.host_executable.is_file()
            || !request.contained_executable.is_absolute()
            || !request.output_root.is_absolute()
            || request.output_root != profile.output_root
            || profile.sha256_digest != sha256_bytes(&profile.bytes)
        {
            return Err(SandboxError::InvalidLaunch);
        }
        validate_launch_environment(&request.environment, profile)?;
        if profile.trust_profile == ProviderTrustProfile::ParsingOnly {
            return Err(SandboxError::ParsingOnly);
        }
        let mut confined = Vec::<String>::new();
        let mut inherited_seccomp = None::<OwnedFd>;
        match profile.mechanism {
            SandboxMechanism::DarwinSeatbelt => {
                if request.contained_executable != request.host_executable {
                    return Err(SandboxError::InvalidLaunch);
                }
                let ProviderSandboxLaunchMaterial::DarwinProfile(path) = material else {
                    return Err(SandboxError::InvalidLaunch);
                };
                confined.extend([
                    "/usr/bin/sandbox-exec".into(),
                    "-f".into(),
                    path.to_string_lossy().into_owned(),
                ]);
            }
            SandboxMechanism::LinuxBubblewrap => {
                let relative_executable = request
                    .host_executable
                    .strip_prefix(&profile.dependency_root)
                    .map_err(|_| SandboxError::InvalidLaunch)?;
                if request.contained_executable
                    != Path::new("/dependencies").join(relative_executable)
                {
                    return Err(SandboxError::InvalidLaunch);
                }
                let ProviderSandboxLaunchMaterial::LinuxSeccomp(descriptor) = material else {
                    return Err(SandboxError::InvalidLaunch);
                };
                // `dup` deliberately clears close-on-exec so bubblewrap can consume the
                // already-compiled seccomp program by descriptor number. The owned duplicate
                // remains alive through `spawn` and is closed in the daemon immediately after.
                let inherited =
                    rustix::io::dup(descriptor).map_err(|_| SandboxError::InvalidLaunch)?;
                let inherited_fd = inherited.as_raw_fd().to_string();
                inherited_seccomp = Some(inherited);
                confined.extend([
                    "/usr/bin/bwrap".into(),
                    "--unshare-all".into(),
                    "--unshare-net".into(),
                    "--die-with-parent".into(),
                    "--new-session".into(),
                    "--cap-drop".into(),
                    "ALL".into(),
                    "--seccomp".into(),
                    inherited_fd,
                    "--ro-bind".into(),
                    "/usr".into(),
                    "/usr".into(),
                    "--proc".into(),
                    "/proc".into(),
                    "--dev".into(),
                    "/dev".into(),
                    "--tmpfs".into(),
                    "/tmp".into(),
                    "--ro-bind".into(),
                    profile.workspace_view.to_string_lossy().into_owned(),
                    "/workspace".into(),
                    "--ro-bind".into(),
                    profile.dependency_root.to_string_lossy().into_owned(),
                    "/dependencies".into(),
                    "--bind".into(),
                    profile.output_root.to_string_lossy().into_owned(),
                    "/output".into(),
                    "--chdir".into(),
                    "/output".into(),
                ]);
            }
            SandboxMechanism::None => {
                if request.contained_executable != request.host_executable {
                    return Err(SandboxError::InvalidLaunch);
                }
                if !matches!(material, ProviderSandboxLaunchMaterial::None) {
                    return Err(SandboxError::InvalidLaunch);
                }
            }
        }
        confined.push(request.contained_executable.to_string_lossy().into_owned());
        confined.extend(request.arguments.iter().cloned());

        // The fixed shell program only applies inherited limits. Provider-controlled strings are
        // positional arguments, never interpolated into shell source.
        let mut command = Command::new("/bin/sh");
        let preserved_descriptor = inherited_seccomp
            .as_ref()
            .map_or_else(String::new, |descriptor| descriptor.as_raw_fd().to_string());
        command
            .arg("-c")
            .arg(PROVIDER_LAUNCH_SHELL)
            .arg("codefabric-provider-launch")
            .arg(preserved_descriptor)
            .arg(request.limits.cpu_seconds.to_string())
            .arg(request.limits.open_files.to_string())
            .arg(request.limits.output_file_bytes.div_ceil(512).to_string())
            .args(confined)
            .env_clear()
            .envs(&request.environment)
            .current_dir(&request.output_root)
            .stdin(Stdio::null())
            .stdout(if capture_stdout {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::piped());
        command.process_group(0);
        // Address-space limits are applied after spawn on Linux by the safe rustix API. Darwin's
        // shell does not expose a portable byte-granularity limit, so Seatbelt plus CPU/FD/file
        // limits is the advertised Darwin contract.
        let child = command.spawn()?;
        drop(inherited_seccomp);
        #[cfg(target_os = "linux")]
        {
            use rustix::process::{Pid, Resource, Rlimit, prlimit};
            use std::num::NonZeroI32;
            let mut child = child;
            let pid = i32::try_from(child.id())
                .ok()
                .and_then(NonZeroI32::new)
                .map(Pid::from_raw)
                .ok_or(SandboxError::InvalidLaunch)?;
            let limit = Rlimit {
                current: Some(request.limits.address_space_bytes),
                maximum: Some(request.limits.address_space_bytes),
            };
            if prlimit(Some(pid), Resource::As, limit).is_err() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SandboxError::ResourceLimit);
            }
            return ProviderProcessGroupChild::new(child);
        }
        #[cfg(not(target_os = "linux"))]
        ProviderProcessGroupChild::new(child)
    }
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("{SANDBOX_UNAVAILABLE_REASON}")]
    SandboxUnavailable,
    #[error("sandbox profile is invalid: {0}")]
    InvalidProfile(&'static str),
    #[error("sandbox profile digest differs")]
    ProfileDigestMismatch,
    #[error("provider launch request is invalid")]
    InvalidLaunch,
    #[error("provider launch environment is not closed: {0}")]
    InvalidEnvironment(&'static str),
    #[error("semantic child execution is forbidden by PARSING_ONLY")]
    ParsingOnly,
    #[error("provider resource limit could not be applied")]
    ResourceLimit,
    #[error("provider child did not enter its required run-scoped process group")]
    ProcessGroupUnavailable,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn validate_launch_environment(
    environment: &BTreeMap<String, String>,
    profile: &GeneratedSandboxProfile,
) -> Result<(), SandboxError> {
    if environment.is_empty() || environment.len() > 64 {
        return Err(SandboxError::InvalidEnvironment("variable count"));
    }
    let path = environment
        .get("PATH")
        .ok_or(SandboxError::InvalidEnvironment("PATH is absent"))?;
    if path.is_empty() || path.len() > 4_096 || path.bytes().any(|byte| byte == 0) {
        return Err(SandboxError::InvalidEnvironment("PATH is invalid"));
    }
    for entry in path.split(':') {
        let entry = Path::new(entry);
        let dependency_visible = match profile.mechanism {
            SandboxMechanism::LinuxBubblewrap => entry.starts_with("/dependencies"),
            SandboxMechanism::DarwinSeatbelt | SandboxMechanism::None => {
                entry.starts_with(&profile.dependency_root)
            }
        };
        if !entry.is_absolute()
            || !(entry == Path::new("/usr/bin") || entry == Path::new("/bin") || dependency_visible)
        {
            return Err(SandboxError::InvalidEnvironment(
                "PATH entry is unauthorized",
            ));
        }
    }
    const FORBIDDEN_EXACT: [&str; 8] = [
        "CARGO_ENCODED_RUSTFLAGS",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "RUSTFLAGS",
        "SSH_AGENT_PID",
        "SSH_AUTH_SOCK",
        "SSLKEYLOGFILE",
        "GIT_ASKPASS",
    ];
    for (name, value) in environment {
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            || value.len() > 16 * 1_024
            || value.bytes().any(|byte| byte == 0)
            || FORBIDDEN_EXACT.contains(&name.as_str())
            || ["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "PROXY"]
                .iter()
                .any(|fragment| name.contains(fragment))
        {
            return Err(SandboxError::InvalidEnvironment("variable rejected"));
        }
    }
    Ok(())
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn quote_seatbelt(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn darwin_profile(view: &Path, dependencies: &Path, output: &Path) -> String {
    let isolation_root = view
        .ancestors()
        .find(|candidate| {
            candidate.parent().is_some()
                && dependencies.starts_with(candidate)
                && output.starts_with(candidate)
        })
        .unwrap_or(view);
    format!(
        "(version 1)\n(allow default)\n(deny network*)\n(allow network-bind network-inbound (prefix \"{}/\"))\n(deny file-read* (subpath \"{}\") (subpath \"/Users\") (subpath \"/Volumes\"))\n(allow file-read* (subpath \"{}\") (subpath \"{}\") (subpath \"{}\"))\n(deny file-read* (subpath \"{}/.git\"))\n(deny file-write*)\n(allow file-write* (subpath \"{}\"))\n",
        quote_seatbelt(output),
        quote_seatbelt(isolation_root),
        quote_seatbelt(view),
        quote_seatbelt(dependencies),
        quote_seatbelt(output),
        quote_seatbelt(view),
        quote_seatbelt(output),
    )
}

fn linux_profile(view: &Path, dependencies: &Path, output: &Path) -> String {
    format!(
        "mechanism=bubblewrap-0.11.2\nno_new_privs=true\nunshare=user,pid,ipc,uts,cgroup,net\nro_bind={}:/workspace\nro_bind={}:/dependencies\nbind={}:/output\ntmpfs=/tmp\ncap_drop=ALL\nseccomp=codefabric-provider-v1\ndie_with_parent=true\nnew_session=true\n",
        view.display(),
        dependencies.display(),
        output.display(),
    )
}

fn sha256_json(value: &impl Serialize) -> Result<String, SandboxError> {
    Ok(sha256_bytes(&serde_json_canonicalizer::to_vec(value)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(behavior: bool) -> SandboxProbeObservation {
        SandboxProbeObservation {
            mechanism: SandboxMechanism::DarwinSeatbelt,
            executable_path: "/usr/bin/sandbox-exec".into(),
            executable_version: "darwin-seatbelt".into(),
            owned_by_root: true,
            executable_mode: 0o755,
            setuid: false,
            behavior: [
                "launch-confined",
                "leased-workspace-read-allowed",
                "workspace-write-denied",
                "out-of-root-write-denied",
                "live-workspace-read-denied",
                "credential-read-denied",
                "git-read-denied",
                "network-denied",
                "inherited-fd-read-denied",
                "child-process-contained",
                "resource-limit-enforceable",
                "cleanup-escape-denied",
                "output-write-allowed",
            ]
            .into_iter()
            .map(|name| (name.into(), behavior))
            .collect(),
        }
    }

    #[test]
    fn sandbox_unavailable_fail_closed_falsification() {
        let matrix = SandboxCapabilityMatrix::evaluate(&observation(false));
        let row = matrix
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(!row.available);
        assert_eq!(row.reason_code, "SANDBOX_BEHAVIOR_UNPROVED");

        let directory = tempfile::tempdir().unwrap();
        for path in [
            directory.path().join("view"),
            directory.path().join("dependencies"),
            directory.path().join("output"),
        ] {
            std::fs::create_dir(path).unwrap();
        }
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::DarwinSeatbelt,
            &directory.path().join("view"),
            &directory.path().join("dependencies"),
            &directory.path().join("output"),
        )
        .unwrap();
        let error = ProviderSandboxLauncher::new(matrix)
            .launch(
                &ProviderLaunchRequest {
                    host_executable: "/usr/bin/true".into(),
                    contained_executable: "/usr/bin/true".into(),
                    arguments: Vec::new(),
                    environment: BTreeMap::from([("PATH".to_owned(), "/usr/bin:/bin".to_owned())]),
                    output_root: directory.path().join("output"),
                    limits: ProviderProcessLimits {
                        cpu_seconds: 1,
                        open_files: 16,
                        address_space_bytes: 64 * 1024 * 1024,
                        output_file_bytes: 1024,
                    },
                },
                &profile,
                ProviderSandboxLaunchMaterial::DarwinProfile(Path::new("/unreachable/profile")),
            )
            .unwrap_err();
        assert!(matches!(error, SandboxError::SandboxUnavailable));
    }

    #[test]
    fn profile_is_deterministic_and_separates_writable_output() {
        let root = tempfile::tempdir().unwrap();
        let view = root.path().join("view");
        let dependencies = root.path().join("dependencies");
        let output = root.path().join("output");
        for path in [&view, &dependencies, &output] {
            std::fs::create_dir(path).unwrap();
        }
        let first = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::DarwinSeatbelt,
            &view,
            &dependencies,
            &output,
        )
        .unwrap();
        let second = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::DarwinSeatbelt,
            &view,
            &dependencies,
            &output,
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.sha256_digest.starts_with("sha256:"));
        assert!(
            String::from_utf8(first.bytes)
                .unwrap()
                .contains("deny network")
        );
    }

    #[test]
    fn trusted_local_launch_owns_and_terminates_complete_process_group() {
        let root = tempfile::tempdir().unwrap();
        let view = root.path().join("view");
        let dependencies = root.path().join("dependencies");
        let output = root.path().join("output");
        for path in [&view, &dependencies, &output] {
            std::fs::create_dir(path).unwrap();
        }
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::TrustedLocal,
            SandboxMechanism::None,
            &view,
            &dependencies,
            &output,
        )
        .unwrap();
        let matrix = SandboxCapabilityMatrix::evaluate(&observation(false));
        let marker = output.join("environment-marker");
        let mut child = ProviderSandboxLauncher::new(matrix)
            .launch(
                &ProviderLaunchRequest {
                    host_executable: "/bin/sh".into(),
                    contained_executable: "/bin/sh".into(),
                    arguments: vec![
                        "-c".into(),
                        "printf '%s' \"$CF_TEST_MARKER\" > environment-marker; sleep 30 & wait"
                            .into(),
                    ],
                    environment: BTreeMap::from([
                        ("CF_TEST_MARKER".to_owned(), "expected".to_owned()),
                        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                    ]),
                    output_root: profile.output_root.clone(),
                    limits: ProviderProcessLimits {
                        cpu_seconds: 10,
                        open_files: 32,
                        address_space_bytes: 128 * 1024 * 1024,
                        output_file_bytes: 1024,
                    },
                },
                &profile,
                ProviderSandboxLaunchMaterial::None,
            )
            .unwrap();

        let marker_deadline = Instant::now() + Duration::from_secs(1);
        let environment_installed = loop {
            if std::fs::read(&marker).is_ok_and(|bytes| bytes == b"expected") {
                break true;
            }
            if Instant::now() >= marker_deadline || child.try_wait().unwrap().is_some() {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };

        child.terminate_group().unwrap();
        if !child.wait_group_empty(Duration::from_secs(1)).unwrap() {
            child.kill_group().unwrap();
        }
        assert!(child.wait_group_empty(Duration::from_secs(1)).unwrap());
        let _ = child.wait();
        assert!(
            environment_installed,
            "trusted launch did not install the exact declared environment"
        );
    }

    #[test]
    fn launch_environment_is_closed_and_path_scoped() {
        let root = tempfile::tempdir().unwrap();
        let view = root.path().join("view");
        let dependencies = root.path().join("dependencies");
        let output = root.path().join("output");
        for path in [&view, &dependencies, &output] {
            std::fs::create_dir(path).unwrap();
        }
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::TrustedLocal,
            SandboxMechanism::None,
            &view,
            &dependencies,
            &output,
        )
        .unwrap();
        let allowed = BTreeMap::from([
            ("LC_ALL".to_owned(), "C".to_owned()),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ]);
        assert!(validate_launch_environment(&allowed, &profile).is_ok());

        for rejected in [
            BTreeMap::from([("PATH".to_owned(), "relative/bin".to_owned())]),
            BTreeMap::from([
                ("HTTP_PROXY".to_owned(), "http://127.0.0.1".to_owned()),
                ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ]),
            BTreeMap::from([
                ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                ("RUSTFLAGS".to_owned(), "-C link-arg=-evil".to_owned()),
            ]),
        ] {
            assert!(validate_launch_environment(&rejected, &profile).is_err());
        }
    }

    #[test]
    fn semantic_sandbox_current_host_escape_matrix() {
        let observation = probe_host_sandbox();
        let matrix = SandboxCapabilityMatrix::evaluate(&observation);
        let untrusted = matrix
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .expect("the closed matrix always contains the untrusted row");
        if observation.mechanism == SandboxMechanism::DarwinSeatbelt {
            let required = [
                "launch-confined",
                "leased-workspace-read-allowed",
                "workspace-write-denied",
                "out-of-root-write-denied",
                "live-workspace-read-denied",
                "credential-read-denied",
                "git-read-denied",
                "network-denied",
                "inherited-fd-read-denied",
                "child-process-contained",
                "resource-limit-enforceable",
                "cleanup-escape-denied",
                "output-write-allowed",
            ];
            for probe in required {
                assert_eq!(
                    observation.behavior.get(probe),
                    Some(&true),
                    "advertised Darwin containment lacks {probe}: {observation:?}"
                );
            }
            assert!(untrusted.available, "{observation:?}");
            assert_eq!(untrusted.reason_code, "SANDBOX_PROVED");
        } else {
            assert!(!untrusted.available);
            assert_ne!(untrusted.reason_code, "SANDBOX_PROVED");
        }
    }
}
