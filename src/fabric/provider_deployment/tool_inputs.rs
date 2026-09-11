//! Bounded deployment observations of the exact compiler capture roots. These are invalidation
//! witnesses, not content identities or an inventory of the host environment.

use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt as _;

use super::{frame, observe_path};
use crate::cancellation::Cancellation;
use crate::resource_budget::{ResourceAmounts, ResourceBudget};

pub(in crate::fabric) const COMPILER_INPUT_ROOTS: [&str; 3] = ["bin/cargo", "bin/rustc", "lib"];
pub(in crate::fabric) const MAX_COMPILER_INPUT_ENTRIES: usize = 100_000;
const MAX_DURATION: Duration = Duration::from_secs(30);

/// Called by an owned blocking capture/census worker. Inline Tokio I/O bounds discovery output
/// and drives cancellation while retaining the child through its actual kill/wait cleanup.
pub(in crate::fabric) fn command_output(
    command: Command,
    cancellation: &Cancellation,
) -> io::Result<Output> {
    tokio::runtime::Handle::current().block_on(command_output_async(
        command,
        cancellation,
        Duration::from_secs(10),
    ))
}

async fn command_output_async(
    command: Command,
    cancellation: &Cancellation,
    deadline: Duration,
) -> io::Result<Output> {
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "tool selection cancelled",
        ));
    }
    let mut command = tokio::process::Command::from(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let group = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .and_then(rustix::process::Pid::from_raw);
    let stdout = child.stdout.take().expect("piped tool stdout");
    let run = async {
        let mut bytes = Vec::new();
        stdout.take(65_537).read_to_end(&mut bytes).await?;
        if bytes.len() > 65_536 {
            return Err(io::Error::other("tool selection output exceeds 64 KiB"));
        }
        let status = child.wait().await?;
        Ok(Output {
            status,
            stdout: bytes,
            stderr: Vec::new(),
        })
    };
    let result = tokio::select! {
        result = run => result,
        () = tokio::time::sleep(deadline) => Err(io::Error::new(io::ErrorKind::TimedOut, "tool selection timed out")),
        () = async {
            while !cancellation.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        } => Err(io::Error::new(io::ErrorKind::Interrupted, "tool selection cancelled")),
    };
    if result.is_err() {
        let group_cleanup =
            group.map_or(Ok(()), |group| {
                match rustix::process::kill_process_group(group, rustix::process::Signal::KILL) {
                    Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
                    Err(error) => Err(error),
                }
            });
        // start_kill can race natural exit. wait is still mandatory and reaps the owned child.
        let _ = child.start_kill();
        let joined = child.wait().await;
        if group_cleanup.is_err() || joined.is_err() {
            return Err(io::Error::other(format!(
                "tool cleanup failed: group={group_cleanup:?}, join={joined:?}; selection={result:?}"
            )));
        }
    }
    result
}

pub(super) fn observe(
    hash: &mut blake3::Hasher,
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> io::Result<()> {
    frame(
        hash,
        crate::rustc_relation_schema::RUSTC_TOOLCHAIN.as_bytes(),
    );
    let mut command = Command::new("rustup");
    command.args([
        "which",
        "--toolchain",
        crate::rustc_relation_schema::RUSTC_TOOLCHAIN,
        "rustc",
    ]);
    match command_output(command, cancellation) {
        Ok(output) if output.status.success() => {
            let path = selected_path(&output.stdout)?;
            let root = path
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| io::Error::other("selected compiler has no sysroot"))?;
            observe_roots(hash, root, budget, cancellation)?;
        }
        Ok(_) => frame(hash, b"selected-rust-toolchain-unavailable"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => frame(hash, b"rustup-unavailable"),
        Err(error) => return Err(error),
    }
    observe_driver(hash, Path::new("/usr/bin/cc"), cancellation)
}

fn observe_driver(
    hash: &mut blake3::Hasher,
    driver: &Path,
    cancellation: &Cancellation,
) -> io::Result<()> {
    // A missing driver is a negative input. Reappearance or /etc/alternatives retargeting is
    // observed without retaining a previous process-local toolchain cache.
    frame(hash, driver.as_os_str().as_bytes());
    observe_path(hash, driver)?;
    if let Ok(resolved) = std::fs::canonicalize(driver) {
        // Match compiler capture's selected runtime boundary before running the discovery tool.
        if !resolved.starts_with("/usr") {
            frame(hash, b"escaped-system-compiler");
            return Ok(());
        }
        let mut command = Command::new(&resolved);
        command.env_clear().arg("-print-libgcc-file-name");
        let output = command_output(command, cancellation)?;
        frame(
            hash,
            if output.status.success() {
                b"runtime-selected"
            } else {
                b"runtime-unavailable"
            },
        );
        if output.status.success() {
            let library = selected_path(&output.stdout)?;
            frame(hash, library.as_os_str().as_bytes());
            if !library.starts_with("/usr") {
                frame(hash, b"escaped-compiler-runtime");
                return Ok(());
            }
            observe_path(hash, &library)?;
            if let Some(directory) = library.parent() {
                observe_path(hash, directory)?;
            }
        }
    }
    Ok(())
}

fn selected_path(bytes: &[u8]) -> io::Result<PathBuf> {
    // The selected command owns one newline-delimited path. Preserve non-UTF-8 Unix bytes.
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.is_empty() || bytes.contains(&0) || !bytes.starts_with(b"/") {
        return Err(io::Error::other("tool selection returned no absolute path"));
    }
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

fn observe_roots(
    hash: &mut blake3::Hasher,
    root: &Path,
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> io::Result<()> {
    let started = Instant::now();
    let mut memory = crate::inventory::reserve_memory(budget, 4096).map_err(io::Error::other)?;
    let mut pending = Vec::new();
    // Exactly the roots consumed by immutable compiler capture. Directory witnesses fence
    // additions/removals, while each positive member fences replacement, mode and symlink target.
    for relative in COMPILER_INPUT_ROOTS {
        pending.push(root.join(relative));
    }
    let mut count = 0;
    let resolved_root = std::fs::canonicalize(root)?;
    frame(hash, root.as_os_str().as_bytes());
    frame(hash, resolved_root.as_os_str().as_bytes());
    while let Some(path) = pending.pop() {
        if cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "tool observation cancelled",
            ));
        }
        count += 1;
        if count > MAX_COMPILER_INPUT_ENTRIES || started.elapsed() > MAX_DURATION {
            return Err(io::Error::other("selected tool observation bound exceeded"));
        }
        frame(hash, path.as_os_str().as_bytes());
        observe_path(hash, &path)?;
        let resolved = match std::fs::canonicalize(&path) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !resolved.starts_with(&resolved_root) {
            // Capture rejects this escaped selection. Observe its boundary without traversing
            // the host so repairing the link still invalidates a terminal unavailable result.
            frame(hash, b"escaped-toolchain-member");
            continue;
        }
        if std::fs::metadata(&resolved)?.is_dir() {
            if path
                .components()
                .count()
                .saturating_sub(root.components().count())
                > 64
            {
                return Err(io::Error::other("selected tool directory depth exceeded"));
            }
            let mut children = Vec::new();
            for entry in std::fs::read_dir(&path)? {
                if count + pending.len() + children.len() >= MAX_COMPILER_INPUT_ENTRIES {
                    return Err(io::Error::other("selected tool entry bound exceeded"));
                }
                let child = entry?.path();
                memory
                    .try_grow(ResourceAmounts {
                        memory_bytes: child.as_os_str().len() as u64 + 128,
                        ..ResourceAmounts::default()
                    })
                    .map_err(io::Error::other)?;
                children.push(child);
            }
            children.sort_unstable();
            pending.extend(children.into_iter().rev());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe_fixture(root: &Path) -> [u8; 32] {
        let mut hash = blake3::Hasher::new();
        observe_roots(
            &mut hash,
            root,
            &crate::fabric::workspace_resources::test_workspace_budget(),
            &Cancellation::default(),
        )
        .unwrap();
        *hash.finalize().as_bytes()
    }

    #[test]
    fn compiler_observation_fences_members_negative_inputs_and_directory_additions() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir(root.join("bin")).unwrap();
        std::fs::create_dir(root.join("lib")).unwrap();
        std::fs::write(root.join("bin/rustc"), b"compiler").unwrap();
        let absent = observe_fixture(root);
        std::fs::write(root.join("bin/cargo"), b"cargo").unwrap();
        let present = observe_fixture(root);
        assert_ne!(absent, present);
        assert_eq!(present, observe_fixture(root));
        std::fs::write(root.join("lib/new.rlib"), b"library").unwrap();
        let added = observe_fixture(root);
        assert_ne!(present, added);
        let path = root.join("lib/new.rlib");
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, b"updated").unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let replaced = observe_fixture(root);
        assert_ne!(added, replaced);
        std::fs::remove_file(&path).unwrap();
        assert_ne!(replaced, observe_fixture(root));
        std::os::unix::fs::symlink(root.join("lib"), root.join("lib/cycle")).unwrap();
        let mut hash = blake3::Hasher::new();
        assert!(
            observe_roots(
                &mut hash,
                root,
                &crate::fabric::workspace_resources::test_workspace_budget(),
                &Cancellation::default()
            )
            .is_err()
        );
    }

    #[test]
    fn escaped_system_driver_is_observed_without_executing_it() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let driver = root.path().join("cc");
        // Executing this path would fail the observation. The same /usr policy as actual capture
        // must reject it before querying a linker search path.
        std::fs::write(&driver, b"#!/bin/sh\nexit 97\n").unwrap();
        std::fs::set_permissions(&driver, std::fs::Permissions::from_mode(0o700)).unwrap();
        observe_driver(
            &mut blake3::Hasher::new(),
            &driver,
            &Cancellation::default(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn discovery_output_deadline_and_cancellation_join_the_owned_process() {
        let mut normal = Command::new("/bin/printf");
        normal.arg("/selected/tool\n");
        let output = command_output_async(normal, &Cancellation::default(), Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(
            selected_path(&output.stdout).unwrap(),
            Path::new("/selected/tool")
        );
        let mut excess = Command::new("/usr/bin/head");
        excess.args(["-c", "70000", "/dev/zero"]);
        assert!(
            command_output_async(excess, &Cancellation::default(), Duration::from_secs(2))
                .await
                .is_err()
        );
        let fixture = tempfile::tempdir().unwrap();
        let marker = fixture.path().join("pid");
        let mut timeout = Command::new("/bin/sh");
        timeout
            .args(["-c", "echo $$ > \"$1\"; exec sleep 30", "fixture"])
            .arg(&marker);
        assert_eq!(
            command_output_async(
                timeout,
                &Cancellation::default(),
                Duration::from_millis(200)
            )
            .await
            .unwrap_err()
            .kind(),
            io::ErrorKind::TimedOut
        );
        let pid = std::fs::read_to_string(marker)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        assert_eq!(
            rustix::process::test_kill_process(rustix::process::Pid::from_raw(pid).unwrap()),
            Err(rustix::io::Errno::SRCH)
        );
        let cancellation = Cancellation::default();
        let mut sleeping = Command::new("/bin/sleep");
        sleeping.arg("30");
        let (result, ()) = tokio::join!(
            command_output_async(sleeping, &cancellation, Duration::from_secs(2)),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                cancellation.cancel();
            }
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert_eq!(
            command_output_async(
                Command::new("/bin/true"),
                &cancelled,
                Duration::from_secs(2)
            )
            .await
            .unwrap_err()
            .kind(),
            io::ErrorKind::Interrupted
        );
    }
}
