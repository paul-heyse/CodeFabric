//! A contained checker is a workspace accelerator; accepted Arrow owns every published fact.

use crate::resource_budget::native_cpu::{NativeCpuActivity, NativeCpuLease};

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::cancellation::StructuredCancellationScope;
use crate::provider_contracts::{ProviderJob, ProviderResourceCeilings};
use crate::pyrefly_service::{
    PyreflyProviderRunResult, PyreflyServiceError, PyreflyWorkspaceInput,
    SupervisedPyreflyWorkspace, analyze_pyrefly_uds,
};
use crate::resource_budget::{ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation};

use super::pyrefly::StartupPyreflyError;

#[derive(Eq, PartialEq)]
struct Compatibility {
    context: [u8; 32],
    ceilings: ProviderResourceCeilings,
    executable: PathBuf,
    // Detect ordinary replacement or in-place deployment changes without rereading the binary
    // for every source edit. The selected provider build/protocol are also checked by the service.
    executable_metadata: (u64, u64, u64, i64, i64, i64, i64),
}

impl Compatibility {
    fn read(job: &ProviderJob, executable: &Path) -> Result<Self, PyreflyServiceError> {
        let metadata = std::fs::metadata(executable)
            .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
        Ok(Self {
            // The native checker configuration is the exact canonical manifest. Complete
            // source/module inventories are freshly validated and installed by each run;
            // negative lookup evidence can change without changing this configuration.
            context: job.context().semantic_environment_id(),
            ceilings: job.ceilings(),
            executable: executable.to_owned(),
            executable_metadata: (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
        })
    }
}

struct Retained {
    cpu: NativeCpuActivity,
    process: SupervisedPyreflyWorkspace,
    output_root: PathBuf,
    compatibility: Compatibility,
    last_used: Instant,
    reusable: bool,
    _metadata: ResourceReservation,
}

#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct PyreflyCacheObservation {
    process_starts: u64,
    reused_runs: u64,
    retirements: u64,
    retained_processes: usize,
    retained_completed_generations: u64,
    sampled_cpu_millis: Option<u64>,
    sampled_peak_memory_bytes: Option<u64>,
}

pub(in crate::fabric) struct PyreflyCache {
    budget: ResourceBudget,
    scope: Option<StructuredCancellationScope>,
    retained: Option<Retained>,
    observation: PyreflyCacheObservation,
}

impl PyreflyCache {
    pub(in crate::fabric) fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            scope: None,
            retained: None,
            observation: PyreflyCacheObservation::default(),
        }
    }

    pub(super) fn bind(&mut self, parent: &StructuredCancellationScope) -> Result<(), String> {
        if self.scope.is_some() {
            return Err("workspace checker lifetime was already bound".into());
        }
        self.scope = Some(
            parent
                .child_control("retained-pyrefly")
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    }

    pub(super) fn observation(&self) -> PyreflyCacheObservation {
        PyreflyCacheObservation {
            retained_processes: usize::from(self.retained.is_some()),
            retained_completed_generations: self
                .retained
                .as_ref()
                .map_or(0, |entry| entry.process.completed_generations()),
            sampled_cpu_millis: self
                .retained
                .as_ref()
                .and_then(|entry| entry.process.kernel_usage())
                .map(|usage| usage.cpu_millis),
            sampled_peak_memory_bytes: self
                .retained
                .as_ref()
                .and_then(|entry| entry.process.kernel_usage())
                .map(|usage| usage.peak_memory_bytes),
            ..self.observation
        }
    }

    pub(super) async fn retire(&mut self) -> Result<(), StartupPyreflyError> {
        if let Some(entry) = &mut self.retained {
            entry.reusable = false;
            // Keep ownership on a failed join. A later request retries retirement before it
            // can launch another process; cancellation intent alone never releases this slot.
            entry
                .process
                .drain_and_join()
                .await
                .map_err(|error| StartupPyreflyError::Join(error.to_string()))?;
            clear_checker_output(&entry.output_root)?;
            self.retained = None;
            self.observation.retirements += 1;
        }
        Ok(())
    }

    pub(super) async fn evict_idle(&mut self, now: Instant) -> Result<(), StartupPyreflyError> {
        let used = self.budget.observation();
        let working = self.retained.as_ref().map_or(0, |entry| {
            entry.compatibility.ceilings.max_bytes().saturating_mul(2)
        });
        let pressure = used.used.memory_bytes + u128::from(working)
            > u128::from(
                used.policy.limits.memory_bytes - used.policy.control_reserve.memory_bytes,
            );
        if self.retained.as_ref().is_some_and(|entry| {
            pressure || now.saturating_duration_since(entry.last_used) >= Duration::from_secs(600)
        }) {
            self.retire().await?;
        }
        Ok(())
    }

    pub(super) async fn analyze(
        &mut self,
        job: &ProviderJob,
        input: PyreflyWorkspaceInput,
        executable: PathBuf,
        input_root: &Path,
        output_root: PathBuf,
        cpu: NativeCpuLease,
    ) -> Result<PyreflyProviderRunResult, StartupPyreflyError> {
        if cpu.workers().get() != usize::from(job.ceilings().max_workers()) {
            return Err(PyreflyServiceError::Invalid(
                "native CPU allocation differs from checker job".into(),
            )
            .into());
        }
        let compatibility = Compatibility::read(job, &executable)?;
        if self.retained.as_mut().is_some_and(|entry| {
            !entry.reusable
                || entry.compatibility != compatibility
                || !entry.process.is_healthy()
                || entry
                    .process
                    .kernel_usage()
                    .is_some_and(|usage| usage.cpu_millis >= 600_000)
        }) {
            self.retire().await?;
        }
        let mut cpu = Some(cpu);
        if self.retained.is_none() {
            let scope = self.scope.as_ref().ok_or_else(|| {
                PyreflyServiceError::ProcessTermination(
                    "workspace checker lifetime is absent".into(),
                )
            })?;
            let metadata = self
                .budget
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: (std::mem::size_of::<Retained>()
                            + executable.as_os_str().len()
                            + output_root.as_os_str().len()
                            + 512) as u64,
                        ..ResourceAmounts::default()
                    },
                )
                .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
            // Prior failed process output is recomputable. Clear it only before a new
            // process owns this private directory, after any prior retained owner joined.
            clear_checker_output(&output_root)?;
            let activity = NativeCpuActivity::default();
            activity
                .begin(cpu.take().expect("one new activity"))
                .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
            let process = launch(
                job,
                executable,
                input_root,
                output_root.clone(),
                scope,
                activity.clone(),
            )
            .await?;
            self.retained = Some(Retained {
                cpu: activity,
                process,
                output_root,
                compatibility,
                last_used: Instant::now(),
                reusable: false,
                _metadata: metadata,
            });
            self.observation.process_starts += 1;
        } else {
            self.observation.reused_runs += 1;
        }
        let entry = self
            .retained
            .as_mut()
            .expect("created or retained workspace checker");
        if let Some(cpu) = cpu {
            entry
                .cpu
                .begin(cpu)
                .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
        }
        entry.reusable = false;
        let result = analyze_pyrefly_uds(&mut entry.process, job, &input).await;
        // Retain only a complete accepted observation. Cancellation or partial extraction may
        // have advanced the private checker, so the next attempt receives a fresh context.
        if result
            .as_ref()
            .is_ok_and(|result| result.accepted().is_some())
        {
            entry.cpu.complete();
            entry.reusable = true;
            entry.last_used = Instant::now();
        } else {
            self.retire().await?;
        }
        result.map_err(Into::into)
    }
}

// Invoked on the existing owned blocking provider/census task, with no operational writer held.
// Entries may be provider-authored symlinks; directory iteration and removal never follow them.
fn clear_checker_output(root: &Path) -> Result<(), PyreflyServiceError> {
    let clear = || -> std::io::Result<()> {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    };
    clear().map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))
}

#[cfg(target_os = "linux")]
async fn launch(
    job: &crate::provider_contracts::ProviderJob,
    executable: PathBuf,
    input_root: &Path,
    output_root: PathBuf,
    scope: &StructuredCancellationScope,
    cpu: NativeCpuActivity,
) -> Result<SupervisedPyreflyWorkspace, StartupPyreflyError> {
    use crate::provider_sandbox::{
        CompiledProviderSeccomp, GeneratedSandboxProfile, ProviderLaunchRequest,
        ProviderProcessLimits, ProviderSandboxLaunchMaterial, ProviderSandboxLauncher,
        ProviderTrustProfile, SandboxCapabilityMatrix, SandboxMechanism,
    };
    use std::collections::BTreeMap;
    let cleanup = scope
        .child_control("pyrefly-process")
        .map_err(PyreflyServiceError::process_admission)?;
    let profile = GeneratedSandboxProfile::generate(
        ProviderTrustProfile::UntrustedSandboxed,
        SandboxMechanism::LinuxBubblewrap,
        input_root,
        executable
            .parent()
            .ok_or(PyreflyServiceError::TrustUnavailable)?,
        &output_root,
    )
    .map_err(|_| PyreflyServiceError::TrustUnavailable)?;
    let policy =
        CompiledProviderSeccomp::compile().map_err(|_| PyreflyServiceError::TrustUnavailable)?;
    // Descriptor-relative dialing supports long state paths and pins the private output directory.
    let output = crate::secure_path::open_absolute_directory_nofollow(&output_root)
        .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
    let request = ProviderLaunchRequest {
        contained_executable: Path::new("/dependencies").join(executable.file_name().unwrap()),
        host_executable: executable,
        arguments: vec![
            "--serve".into(),
            "unix:///output/pyrefly.sock".into(),
            "--workers".into(),
            job.ceilings().max_workers().to_string(),
        ],
        environment: BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
        output_root,
        limits: ProviderProcessLimits {
            // A retained service's kernel CPU limit is cumulative. Rotate between jobs after
            // 600 CPU seconds (sampled above), leaving ample room for an admitted final run.
            // Each run still has its original 120-second wall deadline and joined cancellation.
            cpu_seconds: 1800,
            open_files: 4096,
            resident_memory_bytes: 16 * 1024 * 1024 * 1024,
            output_file_bytes: 1024 * 1024 * 1024,
            process_count: 256,
        },
    };
    let process =
        match SupervisedPyreflyWorkspace::try_new(job, output, cleanup.clone(), cpu, move || {
            ProviderSandboxLauncher::new(SandboxCapabilityMatrix::probe_current_host()).launch(
                &request,
                &profile,
                ProviderSandboxLaunchMaterial::LinuxSeccomp(&policy),
            )
        })
        .await
        {
            Ok(process) => process,
            Err(error) => {
                cleanup
                    .cancel_and_join(Duration::from_secs(10))
                    .await
                    .map_err(|join| StartupPyreflyError::Join(join.to_string()))?;
                return Err(error.into());
            }
        };
    Ok(process)
}

#[cfg(not(target_os = "linux"))]
async fn launch(
    _: &ProviderJob,
    _: PathBuf,
    _: &Path,
    _: PathBuf,
    _: &StructuredCancellationScope,
    _: NativeCpuActivity,
) -> Result<SupervisedPyreflyWorkspace, StartupPyreflyError> {
    Err(PyreflyServiceError::TrustUnavailable.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checker_output_cleanup_keeps_input_trees_and_removes_only_private_entries() {
        let root = tempfile::tempdir().unwrap();
        let inputs = root.path().join("inputs");
        let output = root.path().join("output");
        std::fs::create_dir_all(output.join("contexts/old")).unwrap();
        std::fs::create_dir(&inputs).unwrap();
        std::fs::write(inputs.join("source.py"), b"value = 7\n").unwrap();
        std::fs::write(output.join("contexts/old/cache"), b"native").unwrap();
        std::os::unix::fs::symlink(&inputs, output.join("input-link")).unwrap();
        clear_checker_output(&output).unwrap();
        assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
        assert_eq!(
            std::fs::read(inputs.join("source.py")).unwrap(),
            b"value = 7\n"
        );
    }
}
