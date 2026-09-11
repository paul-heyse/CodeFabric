//! Linux provider policy. Bubblewrap limits filesystem/network visibility, cgroups
//! bound the entire process tree, and seccomp rejects namespace and kernel escapes.
//! Policy compilation uses seccompiler's architecture validation and syscall table.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Seek as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{GeneratedSandboxProfile, ProviderTrustProfile, SandboxMechanism};

/// Only application-compiled, sealed policy bytes can enter the Linux launch path.
#[derive(Debug)]
pub struct CompiledProviderSeccomp(File);

impl CompiledProviderSeccomp {
    /// Compile the fixed policy for the running architecture into a sealed descriptor.
    ///
    /// # Errors
    /// Unsupported architectures, compilation or descriptor failures leave containment unavailable.
    pub fn compile() -> io::Result<Self> {
        use rustix::fs::{MemfdFlags, SealFlags};
        fn invalid(error: impl std::fmt::Display) -> io::Error {
            io::Error::other(format!("provider seccomp compilation: {error}"))
        }
        let mut rules = [
            "unshare",
            "setns",
            "mount",
            "umount2",
            "pivot_root",
            "chroot",
            "ptrace",
            "process_vm_readv",
            "process_vm_writev",
            "open_by_handle_at",
            "bpf",
            "perf_event_open",
            "userfaultfd",
            "io_uring_setup",
            "kexec_load",
            "reboot",
            "init_module",
            "finit_module",
            "delete_module",
            "keyctl",
            "add_key",
            "request_key",
            "swapon",
            "swapoff",
            "setsid",
            "setpgid",
            "clone3",
        ]
        .into_iter()
        .map(|syscall| serde_json::json!({"syscall": syscall}))
        .collect::<Vec<_>>();
        // Denying clone3 with ENOSYS lets libc fall back to inspectable clone flags.
        // clone's first argument is flags on the supported x86_64/aarch64/riscv64 ABIs.
        for flag in [
            0x0002_0000_u64,
            0x0200_0000,
            0x0400_0000,
            0x0800_0000,
            0x1000_0000,
            0x2000_0000,
            0x4000_0000,
        ] {
            rules.push(serde_json::json!({"syscall":"clone", "args":[{
                "index":0,"type":"qword","op":{"masked_eq":flag},"val":flag}]}));
        }
        for syscall in ["socket", "socketpair"] {
            rules.push(serde_json::json!({"syscall":syscall, "args":[{
                "index":0,"type":"dword","op":"ne","val":1}]}));
        }
        let json = serde_json::to_vec(&serde_json::json!({"provider":{
            "mismatch_action":"allow", "match_action":{"errno":38}, "filter":rules
        }}))
        .map_err(invalid)?;
        let architecture = std::env::consts::ARCH.try_into().map_err(invalid)?;
        let mut compiled =
            seccompiler::compile_from_json(json.as_slice(), architecture).map_err(invalid)?;
        let instructions = compiled
            .remove("provider")
            .ok_or_else(|| io::Error::other("missing provider filter"))?;
        let mut file = File::from(rustix::fs::memfd_create(
            "codefabric-provider-seccomp-v1",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?);
        for instruction in instructions {
            file.write_all(&instruction.code.to_ne_bytes())?;
            file.write_all(&[instruction.jt, instruction.jf])?;
            file.write_all(&instruction.k.to_ne_bytes())?;
        }
        file.rewind()?;
        rustix::fs::fcntl_add_seals(
            &file,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
        )?;
        Ok(Self(file))
    }

    /// Compile an independent sealed descriptor with its own read cursor.
    ///
    /// # Errors
    /// Returns the compilation or descriptor error from [`Self::compile`].
    pub fn try_clone(&self) -> io::Result<Self> {
        Self::compile()
    }
    pub(super) fn descriptor(&self) -> &File {
        &self.0
    }

    pub(super) fn into_stdin(self) -> Stdio {
        Stdio::from(self.0)
    }
}

pub(super) fn probe_behavior(executable: &Path) -> BTreeMap<String, bool> {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "codefabric-linux-provider-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<BTreeMap<String, bool>, super::SandboxError> {
        let policy = CompiledProviderSeccomp::compile()?;
        let view = root.join("view");
        let dependencies = root.join("dependencies");
        let output = root.join("output");
        for directory in [&view, &dependencies, &output] { fs::create_dir_all(directory)?; }
        fs::write(view.join("leased.py"), b"leased")?;
        // Only the leased view is mounted, never the live checkout or its .git directory.
        let live = root.join("live");
        fs::create_dir_all(live.join(".git"))?;
        fs::write(live.join(".git/config"), b"git")?;
        fs::write(live.join("source.py"), b"live")?;
        fs::write(root.join("credential"), b"secret fixture")?;
        let profile = GeneratedSandboxProfile::generate(ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::LinuxBubblewrap, &view, &dependencies, &output)?;
        let run = |program: &str, args: &[&str]| -> bool {
            // A fresh descriptor starts at offset zero for every bubblewrap invocation.
            let Ok(policy) = CompiledProviderSeccomp::compile() else { return false; };
            let fd = "3";
            let inherited_probe = format!("exec 9<\"$1\"; shift\n{}", super::PROVIDER_LAUNCH_SHELL);
            Command::new("/bin/sh").args(["-c", &inherited_probe,
                "provider-probe", &root.join("credential").to_string_lossy(), fd, "10", "64", "2048", ""])
                .args(super::linux_sandbox_arguments(&profile, fd))
                .arg(program).args(args).env_clear().env("PATH", "/usr/bin:/bin")
                .stdin(policy.into_stdin()).stdout(Stdio::null()).stderr(Stdio::null())
                .status().is_ok_and(|status| status.success())
        };
        let launch = executable == Path::new(super::LINUX_BUBBLEWRAP_PATH) && run("/bin/true", &[]);
        let denied_read = |path: &Path| !run("/bin/cat", &[&path.to_string_lossy()]);
        let outside = root.join("outside");
        let outside_text = outside.to_string_lossy();
        let behavior = BTreeMap::from([
            ("compiled-seccomp-policy-authorized".into(), policy.descriptor().metadata()?.len() > 0),
            ("launch-confined".into(), launch),
            ("seccomp-active".into(), run("/bin/sh", &["-c", "grep -Eq '^Seccomp:[[:space:]]+2$' /proc/self/status"])),
            ("leased-workspace-read-allowed".into(), run("/bin/cat", &["/workspace/leased.py"])),
            ("workspace-write-denied".into(), launch && !run("/bin/sh", &["-c", "echo bad > /workspace/forbidden"]) && !view.join("forbidden").exists()),
            ("out-of-root-write-denied".into(), launch && !run("/bin/sh", &["-c", "echo bad > \"$1\"", "probe", &outside_text]) && !outside.exists()),
            ("live-workspace-read-denied".into(), launch && denied_read(&live.join("source.py"))),
            ("credential-read-denied".into(), launch && denied_read(&root.join("credential"))),
            ("git-read-denied".into(), launch && denied_read(&live.join(".git/config"))),
            ("network-denied".into(), run("/usr/bin/python3", &["-c", "import socket\ntry: socket.socket(socket.AF_INET)\nexcept OSError: pass\nelse: raise SystemExit(1)\nsocket.socket(socket.AF_UNIX).close()"])),
            ("inherited-fd-read-denied".into(), launch && !run("/bin/cat", &["/proc/self/fd/9"])),
            ("child-process-contained".into(), run("/usr/bin/python3", &["-c", "import os\npid=os.fork()\nif pid==0:\n try: os.setsid()\n except OSError: os._exit(0)\n os._exit(1)\n_,status=os.waitpid(pid,0)\nraise SystemExit(os.waitstatus_to_exitcode(status))"])),
            ("resource-limit-enforceable".into(), run("/bin/sh", &["-c", "test \"$(ulimit -n)\" -eq 64"])),
            ("cleanup-escape-denied".into(), run("/bin/sh", &["-c", "trap 'echo bad > \"$1\"' EXIT; exit 0", "probe", &outside_text]) && !outside.exists()),
            ("output-write-allowed".into(), run("/bin/sh", &["-c", "echo allowed > /output/allowed"]) && fs::read(output.join("allowed")).is_ok_and(|bytes| bytes == b"allowed\n")),
        ]);
        Ok(behavior)
    })().unwrap_or_default();
    let _ = fs::remove_dir_all(root);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_host_probes_preserve_child_local_seccomp_descriptors() {
        use super::super::{SandboxCapabilityMatrix, ProviderTrustProfile};
        // Force descriptor numbers above the shell's single-digit redirection range. These
        // ordinary parent descriptors must stay CLOEXEC during both concurrent launches.
        let _parent_descriptors = (0..32)
            .map(|_| File::open("/dev/null").unwrap()).collect::<Vec<_>>();
        let ready = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let workers = (0..2).map(|_| scope.spawn(|| {
                ready.wait();
                let matrix = SandboxCapabilityMatrix::probe_current_host();
                let row = matrix.row(ProviderTrustProfile::UntrustedSandboxed).unwrap();
                assert!(row.available, "concurrent native containment: {row:?}");
            })).collect::<Vec<_>>();
            for worker in workers { worker.join().unwrap(); }
        });
    }

    #[test]
    fn production_launcher_contains_threads_and_joins_the_whole_process_tree() {
        use super::super::{
            ProviderLaunchRequest, ProviderProcessLimits, ProviderSandboxLaunchMaterial,
            ProviderSandboxLauncher, SandboxCapabilityMatrix,
        };
        use std::time::{Duration, Instant};
        struct Joined(super::super::ProviderProcessGroupChild);
        impl Drop for Joined {
            fn drop(&mut self) {
                let _ = self.0.kill_group();
                let _ = self.0.wait_group_empty(Duration::from_secs(3));
                let _ = self.0.wait();
            }
        }
        let root = tempfile::tempdir().unwrap();
        let view = root.path().join("view");
        let output = root.path().join("output");
        fs::create_dir(&view).unwrap();
        fs::create_dir(&output).unwrap();
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::LinuxBubblewrap,
            &view,
            Path::new("/usr"),
            &output,
        )
        .unwrap();
        let policy = CompiledProviderSeccomp::compile().unwrap();
        let matrix = SandboxCapabilityMatrix::probe_current_host();
        assert!(
            matrix
                .row(ProviderTrustProfile::UntrustedSandboxed)
                .unwrap()
                .available,
            "{matrix:?}"
        );
        let program = "import os,threading,time\nt=threading.Thread(target=lambda:open('/output/thread','w').write('ran'))\nt.start(); t.join()\npid=os.fork()\nif pid==0: time.sleep(10); os._exit(0)\nopen('/output/ready','w').write('ready')\ntime.sleep(10)";
        let mut child = Joined(
            ProviderSandboxLauncher::new(matrix)
                .launch(
                    &ProviderLaunchRequest {
                        host_executable: "/usr/bin/python3".into(),
                        contained_executable: "/dependencies/bin/python3".into(),
                        arguments: vec!["-c".into(), program.into()],
                        environment: BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
                        output_root: output.clone(),
                        limits: ProviderProcessLimits {
                            cpu_seconds: 5,
                            open_files: 64,
                            resident_memory_bytes: 512 * 1024 * 1024,
                            output_file_bytes: 1024 * 1024,
                            process_count: 16,
                        },
                    },
                    &profile,
                    ProviderSandboxLaunchMaterial::LinuxSeccomp(&policy),
                )
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !output.join("ready").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "confined worker exited before ready"
            );
            assert!(
                Instant::now() < deadline,
                "confined worker did not become ready"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(fs::read(output.join("thread")).unwrap(), b"ran");
        let usage = child.0.kernel_usage().unwrap().unwrap();
        assert!(usage.peak_memory_bytes > 0);
        assert!(usage.peak_process_count >= 2);
        child.0.kill_group().unwrap();
        assert!(child.0.wait_group_empty(Duration::from_secs(3)).unwrap());
    }

    #[test]
    fn compiled_policy_is_sealed_and_real_linux_launch_is_confined() {
        let mut policy = CompiledProviderSeccomp::compile().unwrap();
        assert!(policy.0.write_all(b"substitute").is_err());
        let behavior = probe_behavior(Path::new(super::super::LINUX_BUBBLEWRAP_PATH));
        assert!(!behavior.is_empty());
        assert!(behavior.values().all(|value| *value), "{behavior:?}");
    }
}
