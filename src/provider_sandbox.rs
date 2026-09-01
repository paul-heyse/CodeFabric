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
/// Exact Linux containment executable required by the accepted host contract.
pub const LINUX_BUBBLEWRAP_PATH: &str = "/usr/bin/bwrap";
/// Application-owned seccomp policy identity that remains mandatory in addition to bubblewrap.
pub const LINUX_SECCOMP_POLICY_ID: &str = "codefabric-provider-v1";
#[cfg(target_os = "macos")]
static SANDBOX_PROBE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "linux")]
static SANDBOX_RUN_NONCE: AtomicU64 = AtomicU64::new(0);

const COMMON_UNTRUSTED_PROBES: [&str; 13] = [
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

const LINUX_UNTRUSTED_PROBES: [&str; 12] = [
    "linux-namespace-launch",
    "linux-user-namespace-enabled",
    "linux-cgroup-v2-mounted",
    "linux-cgroup-membership-resolved",
    "linux-cgroup-cpu-controller-available",
    "linux-cgroup-memory-controller-available",
    "linux-cgroup-pids-controller-available",
    "linux-cgroup-parent-control-writable",
    "compiled-seccomp-policy-authorized",
    "pre-exec-cgroup-placement",
    "kernel-complete-accounting",
    "seccomp-active",
];

pub(crate) fn required_untrusted_probes(mechanism: SandboxMechanism) -> Vec<&'static str> {
    let mut required = COMMON_UNTRUSTED_PROBES.to_vec();
    if mechanism == SandboxMechanism::LinuxBubblewrap {
        required.extend(LINUX_UNTRUSTED_PROBES);
    }
    required
}

const PROVIDER_LAUNCH_SHELL: &str = r#"
preserve="$1"
cpu="$2"
open_files="$3"
file_blocks="$4"
cgroup_procs="$5"
shift 5
if [ -n "$cgroup_procs" ]; then
  printf '%s\n' "$$" > "$cgroup_procs" || exit 125
fi
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
    /// Every failed input to this decision, including exact executable identity.
    pub unmet_requirements: Vec<String>,
    /// Required and observed executable identity retained with the typed unavailable decision.
    pub executable_identity: SandboxExecutableIdentityEvidence,
    pub probe_digest: String,
}

/// Exact host executable requirement and the observation evaluated against it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SandboxExecutableIdentityEvidence {
    pub required_path: PathBuf,
    pub required_version: String,
    pub require_root_owned: bool,
    pub forbidden_mode_bits: u32,
    pub require_setuid: bool,
    pub observed_path: PathBuf,
    pub observed_version: String,
    pub observed_root_owned: bool,
    pub observed_mode: u32,
    pub observed_setuid: bool,
}

/// Complete host matrix. Untrusted execution is advertised only after every required probe passes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SandboxCapabilityMatrix {
    rows: Vec<SandboxCapabilityRow>,
}

impl SandboxCapabilityMatrix {
    /// Probe the current host and mint its sole production capability matrix.
    ///
    /// A caller cannot supply claimed observations to this constructor. Synthetic observations
    /// are restricted to unit tests so a claimed sandbox digest cannot become launch authority.
    #[must_use]
    pub fn probe_current_host() -> Self {
        Self::evaluate(&probe_host_sandbox())
    }

    fn evaluate(observation: &SandboxProbeObservation) -> Self {
        let required = required_untrusted_probes(observation.mechanism);
        let (required_path, required_version) = match observation.mechanism {
            SandboxMechanism::DarwinSeatbelt => (PathBuf::from("/usr/bin/sandbox-exec"), ""),
            SandboxMechanism::LinuxBubblewrap => (PathBuf::from(LINUX_BUBBLEWRAP_PATH), ""),
            SandboxMechanism::None => (PathBuf::new(), "unsupported-host"),
        };
        let executable_identity = SandboxExecutableIdentityEvidence {
            required_path,
            required_version: required_version.into(),
            require_root_owned: true,
            forbidden_mode_bits: 0o022,
            require_setuid: false,
            observed_path: observation.executable_path.clone(),
            observed_version: observation.executable_version.clone(),
            observed_root_owned: observation.owned_by_root,
            observed_mode: observation.executable_mode,
            observed_setuid: observation.setuid,
        };
        let exact_identity = executable_identity.observed_path == executable_identity.required_path
            && (executable_identity.required_version.is_empty()
                || executable_identity.observed_version.trim()
                    == executable_identity.required_version)
            && executable_identity.observed_root_owned == executable_identity.require_root_owned
            && executable_identity.observed_mode & executable_identity.forbidden_mode_bits == 0
            && executable_identity.observed_setuid == executable_identity.require_setuid;
        let mut unmet_requirements = required
            .iter()
            .filter(|probe| observation.behavior.get(**probe).copied() != Some(true))
            .map(|probe| (*probe).to_owned())
            .collect::<Vec<_>>();
        if !exact_identity {
            unmet_requirements.insert(0, "exact-sandbox-executable-identity".into());
        }
        let probe_digest = sha256_json(observation).unwrap_or_else(|_| "sha256:invalid".into());
        let available = unmet_requirements.is_empty();
        let untrusted_reason = if available {
            "SANDBOX_PROVED"
        } else if !exact_identity {
            "SANDBOX_IDENTITY_UNPROVED"
        } else if unmet_requirements
            .iter()
            .any(|requirement| requirement == "compiled-seccomp-policy-authorized")
        {
            "SANDBOX_SECCOMP_POLICY_UNAVAILABLE"
        } else if unmet_requirements
            .iter()
            .any(|requirement| requirement == "linux-user-namespace-enabled")
        {
            "SANDBOX_USER_NAMESPACE_UNAVAILABLE"
        } else if unmet_requirements
            .iter()
            .any(|requirement| requirement == "linux-cgroup-v2-mounted")
        {
            "SANDBOX_CGROUP_V2_UNAVAILABLE"
        } else if unmet_requirements.iter().any(|requirement| {
            matches!(
                requirement.as_str(),
                "linux-cgroup-membership-resolved"
                    | "linux-cgroup-cpu-controller-available"
                    | "linux-cgroup-memory-controller-available"
                    | "linux-cgroup-pids-controller-available"
                    | "linux-cgroup-parent-control-writable"
                    | "pre-exec-cgroup-placement"
            )
        }) {
            "SANDBOX_CGROUP_DELEGATION_UNAVAILABLE"
        } else if unmet_requirements
            .iter()
            .any(|requirement| requirement == "kernel-complete-accounting")
        {
            "SANDBOX_KERNEL_ACCOUNTING_UNAVAILABLE"
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
                    unmet_requirements,
                    executable_identity: executable_identity.clone(),
                    probe_digest: probe_digest.clone(),
                },
                SandboxCapabilityRow {
                    trust_profile: ProviderTrustProfile::TrustedLocal,
                    mechanism: SandboxMechanism::None,
                    available: true,
                    reason_code: "TRUSTED_LOCAL_WEAKER_ISOLATION".into(),
                    unmet_requirements: Vec::new(),
                    executable_identity: executable_identity.clone(),
                    probe_digest: probe_digest.clone(),
                },
                SandboxCapabilityRow {
                    trust_profile: ProviderTrustProfile::ParsingOnly,
                    mechanism: SandboxMechanism::None,
                    available: true,
                    reason_code: "PARSING_ONLY_NO_SEMANTIC_CHILD".into(),
                    unmet_requirements: Vec::new(),
                    executable_identity,
                    probe_digest,
                },
            ],
        }
    }

    #[cfg(test)]
    pub(crate) fn evaluate_for_test(observation: &SandboxProbeObservation) -> Self {
        Self::evaluate(observation)
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
                        "",
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
    let executable = PathBuf::from(LINUX_BUBBLEWRAP_PATH);
    let metadata = fs::metadata(&executable).ok();
    let version = Command::new(&executable)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into());
    let namespace_launch = probe_linux_namespace_launch(&executable);
    let user_namespace_enabled = fs::read_to_string("/proc/sys/user/max_user_namespaces")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .is_some_and(|maximum| maximum > 0)
        && ["user", "pid", "net", "cgroup"]
            .iter()
            .all(|namespace| Path::new("/proc/self/ns").join(namespace).exists());
    let cgroup_v2_mounted = linux_cgroup_v2_mounted();
    let current_cgroup = linux_current_cgroup_directory();
    let cgroup_delegate = linux_cgroup_delegate_root();
    let cgroup_parent_control_writable = cgroup_delegate.is_some();
    let cgroup_controllers = cgroup_delegate
        .as_ref()
        .and_then(|directory| fs::read_to_string(directory.join("cgroup.controllers")).ok())
        .map(|controllers| {
            controllers
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let (pre_exec_cgroup_placement, kernel_complete_accounting) = probe_linux_run_cgroup_backend();
    let mut behavior = COMMON_UNTRUSTED_PROBES
        .into_iter()
        .map(|probe| (probe.to_owned(), false))
        .collect::<BTreeMap<_, _>>();
    behavior.extend([
        ("linux-namespace-launch".into(), namespace_launch),
        (
            "linux-user-namespace-enabled".into(),
            user_namespace_enabled,
        ),
        ("linux-cgroup-v2-mounted".into(), cgroup_v2_mounted),
        (
            "linux-cgroup-membership-resolved".into(),
            current_cgroup.is_some(),
        ),
        (
            "linux-cgroup-cpu-controller-available".into(),
            cgroup_parent_control_writable && cgroup_controllers.iter().any(|value| value == "cpu"),
        ),
        (
            "linux-cgroup-memory-controller-available".into(),
            cgroup_parent_control_writable
                && cgroup_controllers.iter().any(|value| value == "memory"),
        ),
        (
            "linux-cgroup-pids-controller-available".into(),
            cgroup_parent_control_writable
                && cgroup_controllers.iter().any(|value| value == "pids"),
        ),
        (
            "linux-cgroup-parent-control-writable".into(),
            cgroup_parent_control_writable,
        ),
        // A caller-supplied descriptor is not proof of an application-owned compiled policy. The
        // policy and the full escape matrix remain unavailable even when the independently tested
        // cgroup backend can place and account a child before its first untrusted exec.
        ("compiled-seccomp-policy-authorized".into(), false),
        (
            "pre-exec-cgroup-placement".into(),
            pre_exec_cgroup_placement,
        ),
        (
            "kernel-complete-accounting".into(),
            kernel_complete_accounting,
        ),
        ("seccomp-active".into(), false),
    ]);
    SandboxProbeObservation {
        mechanism: SandboxMechanism::LinuxBubblewrap,
        executable_path: executable,
        executable_version: version,
        owned_by_root: metadata.as_ref().is_some_and(|value| value.uid() == 0),
        executable_mode: metadata.as_ref().map_or(0, |value| value.mode() & 0o7777),
        setuid: metadata
            .as_ref()
            .is_some_and(|value| value.mode() & 0o4000 != 0),
        behavior,
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_namespace_launch(executable: &Path) -> bool {
    let mut command = Command::new(executable);
    command
        .args([
            "--unshare-all",
            "--unshare-net",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--symlink",
            "usr/sbin",
            "/sbin",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--chdir",
            "/tmp",
            "/usr/bin/true",
        ])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_cgroup_v2_mounted() -> bool {
    fs::read_to_string("/proc/self/mountinfo")
        .ok()
        .is_some_and(|mountinfo| {
            mountinfo.lines().any(|line| {
                let Some((mount, filesystem)) = line.split_once(" - ") else {
                    return false;
                };
                mount.split_whitespace().nth(4) == Some("/sys/fs/cgroup")
                    && filesystem.split_whitespace().next() == Some("cgroup2")
            })
        })
}

#[cfg(target_os = "linux")]
fn linux_current_cgroup_directory() -> Option<PathBuf> {
    use std::path::Component;

    let membership = fs::read_to_string("/proc/self/cgroup").ok()?;
    let mut unified = membership
        .lines()
        .filter_map(|line| line.strip_prefix("0::"));
    let relative = Path::new(unified.next()?).strip_prefix("/").ok()?;
    if unified.next().is_some()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(Path::new("/sys/fs/cgroup").join(relative))
}

#[cfg(target_os = "linux")]
fn linux_cgroup_delegate_root() -> Option<PathBuf> {
    const REQUIRED_CONTROLLERS: [&str; 3] = ["cpu", "memory", "pids"];

    let mount = Path::new("/sys/fs/cgroup");
    let current = linux_current_cgroup_directory()?;
    if !current.starts_with(mount) {
        return None;
    }
    let uid = rustix::process::getuid().as_raw();
    current
        .ancestors()
        .take_while(|path| *path != mount)
        .find_map(|path| {
            let metadata = fs::metadata(path).ok()?;
            if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o200 == 0 {
                return None;
            }
            if fs::read_to_string(path.join("cgroup.type")).ok()?.trim() != "domain" {
                return None;
            }
            let controllers = fs::read_to_string(path.join("cgroup.controllers")).ok()?;
            let subtree = fs::read_to_string(path.join("cgroup.subtree_control")).ok()?;
            let has_all = |value: &str| {
                REQUIRED_CONTROLLERS
                    .iter()
                    .all(|required| value.split_whitespace().any(|item| item == *required))
            };
            if !has_all(&controllers) || !has_all(&subtree) {
                return None;
            }
            if !fs::read_to_string(path.join("cgroup.procs"))
                .ok()?
                .trim()
                .is_empty()
            {
                return None;
            }
            ["cgroup.procs", "cgroup.subtree_control"]
                .iter()
                .all(|name| {
                    fs::OpenOptions::new()
                        .write(true)
                        .open(path.join(name))
                        .is_ok()
                })
                .then(|| path.to_path_buf())
        })
}

#[cfg(target_os = "linux")]
fn probe_linux_run_cgroup_backend() -> (bool, bool) {
    let limits = ProviderProcessLimits {
        cpu_seconds: 1,
        open_files: 16,
        address_space_bytes: 64 * 1024 * 1024,
        output_file_bytes: 1024 * 1024,
        process_count: 4,
    };
    let Ok(cgroup) = LinuxRunCgroup::create(limits) else {
        return (false, false);
    };
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(PROVIDER_LAUNCH_SHELL)
        .arg("codefabric-cgroup-probe")
        .arg("")
        .arg(limits.cpu_seconds.to_string())
        .arg(limits.open_files.to_string())
        .arg(limits.output_file_bytes.div_ceil(512).to_string())
        .arg(cgroup.procs_path())
        .arg("/bin/cat")
        .arg("/proc/self/cgroup")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let placed_before_exec = output.is_ok_and(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.ends_with(&format!("/{}", cgroup.name())))
    });
    let accounted_by_kernel = cgroup.usage().is_ok();
    (placed_before_exec, accounted_by_kernel)
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
    pub process_count: u32,
}

/// Kernel-owned aggregate usage for one delegated Linux run cgroup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderKernelUsage {
    pub cpu_millis: u64,
    pub peak_memory_bytes: u64,
    pub peak_process_count: u32,
    pub memory_limit_hit: bool,
    pub process_limit_hit: bool,
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct LinuxRunCgroup {
    path: PathBuf,
}

#[cfg(target_os = "linux")]
impl LinuxRunCgroup {
    fn create(limits: ProviderProcessLimits) -> std::io::Result<Self> {
        let delegate = linux_cgroup_delegate_root().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "delegated cgroup-v2 root with cpu, memory, and pids is unavailable",
            )
        })?;
        let mut created = None;
        for _ in 0..32 {
            let nonce = SANDBOX_RUN_NONCE.fetch_add(1, Ordering::Relaxed);
            let candidate = delegate.join(format!("codefabric-run-{}-{nonce}", std::process::id()));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    created = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        let cgroup = Self {
            path: created.ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "could not allocate a unique run cgroup",
                )
            })?,
        };
        cgroup.configure(limits)?;
        Ok(cgroup)
    }

    fn configure(&self, limits: ProviderProcessLimits) -> std::io::Result<()> {
        let settings = [
            ("cpu.max", "100000 100000".to_owned()),
            ("memory.max", limits.address_space_bytes.to_string()),
            ("memory.oom.group", "1".to_owned()),
            ("pids.max", limits.process_count.to_string()),
        ];
        for (name, value) in settings {
            self.write_and_verify(name, &value)?;
        }
        let swap = self.path.join("memory.swap.max");
        if swap.exists() {
            self.write_and_verify("memory.swap.max", "0")?;
        }
        let peak = self.path.join("memory.peak");
        if fs::OpenOptions::new().write(true).open(&peak).is_ok() {
            fs::write(&peak, b"0\n")?;
        }
        for name in [
            "cgroup.events",
            "cpu.stat",
            "memory.current",
            "memory.events",
            "memory.peak",
            "pids.current",
            "pids.events",
            "pids.peak",
        ] {
            fs::File::open(self.path.join(name))?;
        }
        fs::OpenOptions::new()
            .write(true)
            .open(self.path.join("cgroup.kill"))?;
        Ok(())
    }

    fn write_and_verify(&self, name: &str, value: &str) -> std::io::Result<()> {
        let path = self.path.join(name);
        fs::write(&path, format!("{value}\n"))?;
        let observed = fs::read_to_string(path)?;
        if observed.trim() != value {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("cgroup setting {name} read back as {observed:?}"),
            ));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
    }

    fn procs_path(&self) -> PathBuf {
        self.path.join("cgroup.procs")
    }

    fn contains_process(&self, process_id: u32) -> std::io::Result<bool> {
        let process_id = process_id.to_string();
        Ok(fs::read_to_string(self.procs_path())?
            .lines()
            .any(|line| line.trim() == process_id))
    }

    fn wait_for_process(&self, child: &mut Child) -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if self.contains_process(child.id())? {
                return Ok(());
            }
            if child.try_wait()?.is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "provider exited before run-cgroup placement was observed",
                ));
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "provider did not enter its run cgroup before the placement deadline",
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn is_empty(&self) -> std::io::Result<bool> {
        let events = fs::read_to_string(self.path.join("cgroup.events"))?;
        Ok(read_keyed_u64(&events, "populated")? == 0)
    }

    fn kill(&self) -> std::io::Result<()> {
        if self.is_empty()? {
            return Ok(());
        }
        fs::write(self.path.join("cgroup.kill"), b"1\n")
    }

    fn usage(&self) -> std::io::Result<ProviderKernelUsage> {
        let cpu = fs::read_to_string(self.path.join("cpu.stat"))?;
        let memory_events = fs::read_to_string(self.path.join("memory.events"))?;
        let pids_events = fs::read_to_string(self.path.join("pids.events"))?;
        let cpu_millis = read_keyed_u64(&cpu, "usage_usec")?.div_ceil(1_000);
        let peak_memory_bytes = read_plain_u64(&self.path.join("memory.peak"))?;
        let peak_process_count = u32::try_from(read_plain_u64(&self.path.join("pids.peak"))?)
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "pids.peak exceeds the supported process count",
                )
            })?;
        Ok(ProviderKernelUsage {
            cpu_millis,
            peak_memory_bytes,
            peak_process_count,
            memory_limit_hit: read_keyed_u64(&memory_events, "max")? > 0
                || read_keyed_u64(&memory_events, "oom")? > 0
                || read_keyed_u64(&memory_events, "oom_kill")? > 0,
            process_limit_hit: read_keyed_u64(&pids_events, "max")? > 0,
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxRunCgroup {
    fn drop(&mut self) {
        let _ = self.kill();
        let deadline = Instant::now() + Duration::from_secs(1);
        while matches!(self.is_empty(), Ok(false)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = fs::remove_dir(&self.path);
    }
}

#[cfg(target_os = "linux")]
fn read_plain_u64(path: &Path) -> std::io::Result<u64> {
    fs::read_to_string(path)?
        .trim()
        .parse::<u64>()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

#[cfg(target_os = "linux")]
fn read_keyed_u64(contents: &str, key: &str) -> std::io::Result<u64> {
    contents
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some(key))
                .then(|| fields.next())
                .flatten()
        })
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("cgroup accounting key {key} is absent"),
            )
        })?
        .parse::<u64>()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
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
    #[cfg(target_os = "linux")]
    run_cgroup: Option<LinuxRunCgroup>,
}

impl ProviderProcessGroupChild {
    #[cfg(target_os = "linux")]
    fn new(mut child: Child, run_cgroup: Option<LinuxRunCgroup>) -> Result<Self, SandboxError> {
        let process_group_id = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .ok_or(SandboxError::InvalidLaunch)?;
        if rustix::process::getpgid(Some(process_group_id)) != Ok(process_group_id) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SandboxError::ProcessGroupUnavailable);
        }
        if let Some(cgroup) = &run_cgroup
            && cgroup.wait_for_process(&mut child).is_err()
        {
            let _ = cgroup.kill();
            let _ = child.kill();
            let _ = child.wait();
            return Err(SandboxError::CgroupPlacement);
        }
        Ok(Self {
            child,
            process_group_id,
            run_cgroup,
        })
    }

    #[cfg(not(target_os = "linux"))]
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

    /// Read kernel-complete aggregate CPU, memory, and process usage when this child owns a
    /// delegated Linux run cgroup. Trusted-local and non-Linux launches return `None`.
    pub fn kernel_usage(&self) -> std::io::Result<Option<ProviderKernelUsage>> {
        #[cfg(target_os = "linux")]
        {
            return self
                .run_cgroup
                .as_ref()
                .map(LinuxRunCgroup::usage)
                .transpose();
        }
        #[cfg(not(target_os = "linux"))]
        Ok(None)
    }

    /// Send a graceful termination signal to the complete run-scoped group.
    pub fn terminate_group(&mut self) -> std::io::Result<()> {
        self.signal_group(rustix::process::Signal::TERM)
    }

    /// Send an unconditional kill signal to the complete run-scoped group.
    pub fn kill_group(&mut self) -> std::io::Result<()> {
        let process_group = self.signal_group(rustix::process::Signal::KILL);
        #[cfg(target_os = "linux")]
        let cgroup = self
            .run_cgroup
            .as_ref()
            .map_or(Ok(()), LinuxRunCgroup::kill);
        #[cfg(not(target_os = "linux"))]
        let cgroup = Ok(());
        process_group.and(cgroup)
    }

    /// Wait until the complete process group no longer exists, reaping its leader as it exits.
    pub fn wait_group_empty(&mut self, timeout: Duration) -> std::io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            let _ = self.child.try_wait()?;
            let process_group_empty =
                match rustix::process::test_kill_process_group(self.process_group_id) {
                    Ok(()) => false,
                    Err(error) if error == rustix::io::Errno::SRCH => true,
                    // Darwin can transiently report EPERM while a just-signalled group is being
                    // dismantled. It is not proof of emptiness: keep polling until ESRCH or timeout.
                    Err(error) if error == rustix::io::Errno::PERM => false,
                    Err(error) => return Err(error.into()),
                };
            #[cfg(target_os = "linux")]
            let cgroup_empty = self
                .run_cgroup
                .as_ref()
                .map_or(Ok(true), LinuxRunCgroup::is_empty)?;
            #[cfg(not(target_os = "linux"))]
            let cgroup_empty = true;
            if process_group_empty && cgroup_empty {
                return Ok(true);
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
                    "--symlink".into(),
                    "usr/bin".into(),
                    "/bin".into(),
                    "--symlink".into(),
                    "usr/lib".into(),
                    "/lib".into(),
                    "--symlink".into(),
                    "usr/lib64".into(),
                    "/lib64".into(),
                    "--symlink".into(),
                    "usr/sbin".into(),
                    "/sbin".into(),
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
        #[cfg(target_os = "linux")]
        let run_cgroup = if profile.mechanism == SandboxMechanism::LinuxBubblewrap {
            Some(
                LinuxRunCgroup::create(request.limits)
                    .map_err(|_| SandboxError::CgroupUnavailable)?,
            )
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let cgroup_procs = run_cgroup.as_ref().map_or_else(String::new, |cgroup| {
            cgroup.procs_path().display().to_string()
        });
        #[cfg(not(target_os = "linux"))]
        let cgroup_procs = String::new();
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
            .arg(cgroup_procs)
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
            let mut child = child;
            let pid = i32::try_from(child.id())
                .ok()
                .and_then(Pid::from_raw)
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
            return ProviderProcessGroupChild::new(child, run_cgroup);
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
    #[error("delegated provider run cgroup is unavailable")]
    CgroupUnavailable,
    #[error("provider child did not enter its delegated run cgroup before exec")]
    CgroupPlacement,
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
        "mechanism=bubblewrap\nno_new_privs=true\nunshare=user,pid,ipc,uts,cgroup,net\nro_bind=/usr:/usr\nsymlink=usr/bin:/bin\nsymlink=usr/lib:/lib\nsymlink=usr/lib64:/lib64\nsymlink=usr/sbin:/sbin\nro_bind={}:/workspace\nro_bind={}:/dependencies\nbind={}:/output\ntmpfs=/tmp\ncap_drop=ALL\nseccomp=codefabric-provider-v1\ndie_with_parent=true\nnew_session=true\n",
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

    fn linux_observation(behavior: bool) -> SandboxProbeObservation {
        SandboxProbeObservation {
            mechanism: SandboxMechanism::LinuxBubblewrap,
            executable_path: "/usr/bin/bwrap".into(),
            executable_version: "bubblewrap 0.11.2".into(),
            owned_by_root: true,
            executable_mode: 0o755,
            setuid: false,
            behavior: COMMON_UNTRUSTED_PROBES
                .into_iter()
                .chain(LINUX_UNTRUSTED_PROBES)
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
                        process_count: 4,
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
                        process_count: 8,
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
    fn linux_claimed_identity_cannot_replace_seccomp_or_kernel_accounting() {
        let mut claimed = linux_observation(true);
        claimed
            .behavior
            .insert("compiled-seccomp-policy-authorized".into(), false);
        let missing_seccomp = SandboxCapabilityMatrix::evaluate(&claimed);
        let row = missing_seccomp
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(!row.available);
        assert_eq!(row.reason_code, "SANDBOX_SECCOMP_POLICY_UNAVAILABLE");
        assert_eq!(
            row.unmet_requirements,
            ["compiled-seccomp-policy-authorized"]
        );

        claimed
            .behavior
            .insert("compiled-seccomp-policy-authorized".into(), true);
        claimed
            .behavior
            .insert("pre-exec-cgroup-placement".into(), false);
        let missing_atomic_placement = SandboxCapabilityMatrix::evaluate(&claimed);
        let row = missing_atomic_placement
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(!row.available);
        assert_eq!(row.reason_code, "SANDBOX_CGROUP_DELEGATION_UNAVAILABLE");
        assert_eq!(row.unmet_requirements, ["pre-exec-cgroup-placement"]);

        claimed
            .behavior
            .insert("pre-exec-cgroup-placement".into(), true);
        claimed
            .behavior
            .insert("kernel-complete-accounting".into(), false);
        let sampled_accounting = SandboxCapabilityMatrix::evaluate(&claimed);
        let row = sampled_accounting
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(!row.available);
        assert_eq!(row.reason_code, "SANDBOX_KERNEL_ACCOUNTING_UNAVAILABLE");
        assert_eq!(row.unmet_requirements, ["kernel-complete-accounting"]);
    }

    #[test]
    fn linux_missing_kernel_accounting_rejects_before_provider_spawn() {
        let mut claimed = linux_observation(true);
        claimed
            .behavior
            .insert("kernel-complete-accounting".into(), false);
        let matrix = SandboxCapabilityMatrix::evaluate(&claimed);
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let dependencies = root.path().join("dependencies");
        let output = root.path().join("output");
        for directory in [&workspace, &dependencies, &output] {
            fs::create_dir(directory).unwrap();
        }
        let executable = dependencies.join("provider");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf launched > /output/launched\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::LinuxBubblewrap,
            &workspace,
            &dependencies,
            &output,
        )
        .unwrap();
        let error = ProviderSandboxLauncher::new(matrix)
            .launch(
                &ProviderLaunchRequest {
                    host_executable: executable,
                    contained_executable: "/dependencies/provider".into(),
                    arguments: Vec::new(),
                    environment: BTreeMap::from([(
                        "PATH".to_owned(),
                        "/dependencies:/usr/bin:/bin".to_owned(),
                    )]),
                    output_root: output.clone(),
                    limits: ProviderProcessLimits {
                        cpu_seconds: 1,
                        open_files: 16,
                        address_space_bytes: 64 * 1024 * 1024,
                        output_file_bytes: 1024,
                        process_count: 4,
                    },
                },
                &profile,
                ProviderSandboxLaunchMaterial::None,
            )
            .unwrap_err();
        assert!(matches!(error, SandboxError::SandboxUnavailable));
        assert!(!output.join("launched").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_run_cgroup_limits_placement_and_descendant_cleanup() {
        if linux_cgroup_delegate_root().is_none() {
            assert_eq!(probe_linux_run_cgroup_backend(), (false, false));
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("entered-cgroup");
        let limits = ProviderProcessLimits {
            cpu_seconds: 2,
            open_files: 32,
            address_space_bytes: 64 * 1024 * 1024,
            output_file_bytes: 1024 * 1024,
            process_count: 8,
        };
        let cgroup = LinuxRunCgroup::create(limits).unwrap();
        let cgroup_path = cgroup.path.clone();
        assert_eq!(
            fs::read_to_string(cgroup_path.join("cpu.max"))
                .unwrap()
                .trim(),
            "100000 100000"
        );
        assert_eq!(
            fs::read_to_string(cgroup_path.join("memory.max"))
                .unwrap()
                .trim(),
            limits.address_space_bytes.to_string()
        );
        assert_eq!(
            fs::read_to_string(cgroup_path.join("pids.max"))
                .unwrap()
                .trim(),
            limits.process_count.to_string()
        );

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(PROVIDER_LAUNCH_SHELL)
            .arg("codefabric-cgroup-test")
            .arg("")
            .arg(limits.cpu_seconds.to_string())
            .arg(limits.open_files.to_string())
            .arg(limits.output_file_bytes.div_ceil(512).to_string())
            .arg(cgroup.procs_path())
            .arg("/bin/sh")
            .arg("-c")
            .arg("cat /proc/self/cgroup > \"$1\"; /usr/bin/setsid /bin/sleep 30 & wait")
            .arg("provider")
            .arg(&marker)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.process_group(0);
        let child = command.spawn().unwrap();
        let mut child = ProviderProcessGroupChild::new(child, Some(cgroup)).unwrap();

        let expected_membership =
            format!("/{}", cgroup_path.file_name().unwrap().to_string_lossy());
        let deadline = Instant::now() + Duration::from_secs(1);
        let membership = loop {
            let membership = fs::read_to_string(&marker).unwrap_or_default();
            if membership
                .lines()
                .any(|line| line.ends_with(&expected_membership))
            {
                break membership;
            }
            assert!(
                Instant::now() < deadline,
                "provider did not observe its pre-exec cgroup membership: {membership:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(
            membership
                .lines()
                .any(|line| line.ends_with(&expected_membership))
        );
        let usage = child.kernel_usage().unwrap().unwrap();
        assert!(usage.peak_process_count >= 1);

        child.kill_group().unwrap();
        assert!(child.wait_group_empty(Duration::from_secs(1)).unwrap());
        let _ = child.wait();
        drop(child);
        assert!(!cgroup_path.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_host_probe_records_remaining_authority_without_fallback() {
        let observation = probe_host_sandbox();
        for requirement in ["compiled-seccomp-policy-authorized", "seccomp-active"] {
            assert_eq!(
                observation.behavior.get(requirement),
                Some(&false),
                "Linux probe must not synthesize {requirement}: {observation:?}"
            );
        }
        for observed_prerequisite in [
            "linux-namespace-launch",
            "linux-user-namespace-enabled",
            "linux-cgroup-v2-mounted",
            "linux-cgroup-membership-resolved",
            "linux-cgroup-cpu-controller-available",
            "linux-cgroup-memory-controller-available",
            "linux-cgroup-pids-controller-available",
            "linux-cgroup-parent-control-writable",
        ] {
            assert!(
                observation.behavior.contains_key(observed_prerequisite),
                "Linux probe omitted {observed_prerequisite}: {observation:?}"
            );
        }
        let matrix = SandboxCapabilityMatrix::probe_current_host();
        let row = matrix
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(!row.available);
        assert!(
            row.unmet_requirements
                .iter()
                .any(|requirement| requirement == "compiled-seccomp-policy-authorized")
        );
        assert!(
            row.unmet_requirements
                .iter()
                .any(|requirement| requirement == "seccomp-active")
        );
        let cgroup_backend_available = observation
            .behavior
            .get("pre-exec-cgroup-placement")
            .copied()
            .unwrap_or(false)
            && observation
                .behavior
                .get("kernel-complete-accounting")
                .copied()
                .unwrap_or(false);
        assert_eq!(
            row.unmet_requirements.iter().all(|requirement| !matches!(
                requirement.as_str(),
                "pre-exec-cgroup-placement" | "kernel-complete-accounting"
            )),
            cgroup_backend_available
        );
        assert!(
            row.unmet_requirements
                .iter()
                .all(|requirement| requirement != "exact-sandbox-executable-identity"),
            "launcher version is observation metadata; exact path/ownership/mode plus behavior prove capability: {observation:?}"
        );
    }

    #[test]
    fn linux_launcher_version_is_not_a_substitute_for_capability_proof() {
        let mut observation = linux_observation(true);
        observation.executable_version = "bubblewrap 0.9.0".into();
        let matrix = SandboxCapabilityMatrix::evaluate(&observation);
        let row = matrix
            .row(ProviderTrustProfile::UntrustedSandboxed)
            .unwrap();
        assert!(row.available, "{row:?}");
        assert_eq!(row.reason_code, "SANDBOX_PROVED");
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
