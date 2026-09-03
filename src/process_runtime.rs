//! Bounded runtime bootstrap for CodeFabric production process roles.
//!
//! Executable entrypoints select a semantic process role. This module owns the
//! corresponding Tokio scheduler shape so binaries do not become an independent
//! source of runtime policy.

use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::io;
use std::path::PathBuf;

use thiserror::Error;
use tokio::runtime::{Builder, Runtime};

use crate::daemon::{DaemonConfig, DaemonError, serve_controlled};
use crate::session_authority::LaunchPolicyId;
use crate::supervisor::{
    SupervisorControlCommand, SupervisorControlStatus, SupervisorError, administer_supervisor,
    check_supervisor_config, control_stream_from_stdin, serve_mcp_launcher, serve_supervisor,
};

const SUPERVISOR_WORKER_THREADS: usize = 2;
const MCP_LAUNCHER_WORKER_THREADS: usize = 2;
const DAEMON_WORKER_THREADS: usize = 4;

/// Strict library-owned process settings accepted by the `codefabric` shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodefabricProcessSettings {
    WorkspaceSupervisor {
        config_path: PathBuf,
    },
    CheckSupervisorConfig {
        config_path: PathBuf,
    },
    SupervisorControl {
        discovery_path: PathBuf,
        command: SupervisorControlCommand,
    },
    McpLauncher {
        discovery_path: PathBuf,
        policy_id: LaunchPolicyId,
    },
}

impl CodefabricProcessSettings {
    /// Parse the closed operational command grammar without accepting ambient defaults.
    pub fn parse<I>(arguments: I) -> Result<Self, ProcessSettingsError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        match arguments.as_slice() {
            [scope, command, flag, path]
                if scope == OsStr::new("supervisor")
                    && command == OsStr::new("serve")
                    && flag == OsStr::new("--config") =>
            {
                Ok(Self::WorkspaceSupervisor {
                    config_path: required_path(path)?,
                })
            }
            [scope, command, flag, path]
                if scope == OsStr::new("supervisor")
                    && command == OsStr::new("check-config")
                    && flag == OsStr::new("--config") =>
            {
                Ok(Self::CheckSupervisorConfig {
                    config_path: required_path(path)?,
                })
            }
            [scope, command, flag, path]
                if scope == OsStr::new("supervisor") && flag == OsStr::new("--discovery") =>
            {
                let command = match command.to_str() {
                    Some("status") => SupervisorControlCommand::Status,
                    _ => return Err(ProcessSettingsError::InvalidGrammar),
                };
                Ok(Self::SupervisorControl {
                    discovery_path: required_path(path)?,
                    command,
                })
            }
            [
                scope,
                command,
                discovery_flag,
                discovery,
                policy_flag,
                policy_id,
            ] if scope == OsStr::new("mcp")
                && command == OsStr::new("serve")
                && discovery_flag == OsStr::new("--supervisor")
                && policy_flag == OsStr::new("--policy-id") =>
            {
                let policy_id = policy_id
                    .to_str()
                    .ok_or(ProcessSettingsError::InvalidPolicyId)
                    .and_then(|value| {
                        LaunchPolicyId::try_new(value)
                            .map_err(|_| ProcessSettingsError::InvalidPolicyId)
                    })?;
                Ok(Self::McpLauncher {
                    discovery_path: required_path(discovery)?,
                    policy_id,
                })
            }
            _ => Err(ProcessSettingsError::InvalidGrammar),
        }
    }

    /// Execute one parsed command through the role-specific bounded runtime.
    pub fn execute(self) -> Result<ProcessCommandOutput, ProcessExecutionError> {
        match self {
            Self::WorkspaceSupervisor { config_path } => {
                RuntimeExecutionPolicy::WorkspaceSupervisor
                    .run(serve_supervisor(&config_path))??;
                Ok(ProcessCommandOutput::Completed)
            }
            Self::CheckSupervisorConfig { config_path } => {
                check_supervisor_config(&config_path)?;
                Ok(ProcessCommandOutput::Completed)
            }
            Self::SupervisorControl {
                discovery_path,
                command,
            } => {
                let status = RuntimeExecutionPolicy::SupervisorControl
                    .run(administer_supervisor(&discovery_path, command))??;
                Ok(ProcessCommandOutput::SupervisorStatus(status))
            }
            Self::McpLauncher {
                discovery_path,
                policy_id,
            } => {
                let status = RuntimeExecutionPolicy::McpLauncher
                    .run(serve_mcp_launcher(&discovery_path, &policy_id))??;
                if status == 0 {
                    Ok(ProcessCommandOutput::Completed)
                } else {
                    Err(ProcessExecutionError::AdapterExit(status))
                }
            }
        }
    }
}

/// Strict library-owned process settings accepted by `codefabricd`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricDaemonProcessSettings {
    pub config_path: PathBuf,
}

impl FabricDaemonProcessSettings {
    pub fn parse<I>(arguments: I) -> Result<Self, ProcessSettingsError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let [command, flag, path] = arguments.as_slice() else {
            return Err(ProcessSettingsError::InvalidGrammar);
        };
        if command != OsStr::new("serve") || flag != OsStr::new("--config") {
            return Err(ProcessSettingsError::InvalidGrammar);
        }
        Ok(Self {
            config_path: required_path(path)?,
        })
    }

    /// Load the closed daemon configuration and require inherited supervisor control.
    pub fn execute(self) -> Result<(), ProcessExecutionError> {
        let config = DaemonConfig::load(&self.config_path)?;
        RuntimeExecutionPolicy::FabricDaemon.run(async move {
            let control = control_stream_from_stdin()?;
            serve_controlled(config, control).await
        })??;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessCommandOutput {
    Completed,
    SupervisorStatus(SupervisorControlStatus),
}

fn required_path(value: &OsStr) -> Result<PathBuf, ProcessSettingsError> {
    if value.as_encoded_bytes().is_empty() {
        return Err(ProcessSettingsError::EmptyPath);
    }
    Ok(PathBuf::from(value))
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProcessSettingsError {
    #[error("operational process arguments do not match the closed command grammar")]
    InvalidGrammar,
    #[error("an operational process path is empty")]
    EmptyPath,
    #[error("the opaque launch policy identity is invalid")]
    InvalidPolicyId,
}

#[derive(Debug, Error)]
pub enum ProcessExecutionError {
    #[error(transparent)]
    Runtime(#[from] RuntimeBootstrapError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
    #[error(transparent)]
    Daemon(#[from] DaemonError),
    #[error("FastMCP adapter exited with status {0}")]
    AdapterExit(i32),
}

/// One released production process role with a fixed, bounded scheduler shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExecutionPolicy {
    /// Persistent workspace supervisor and owned-child lifecycle coordinator.
    WorkspaceSupervisor,
    /// One-shot supervisor administration client.
    SupervisorControl,
    /// Attach-only MCP adapter launcher.
    McpLauncher,
    /// Persistent data-fabric daemon child.
    FabricDaemon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SchedulerShape {
    CurrentThread,
    MultiThread { worker_threads: usize },
}

impl RuntimeExecutionPolicy {
    /// Construct the Tokio runtime fixed for this process role.
    pub fn bootstrap(self) -> Result<Runtime, RuntimeBootstrapError> {
        let mut builder = match self.scheduler_shape() {
            SchedulerShape::CurrentThread => Builder::new_current_thread(),
            SchedulerShape::MultiThread { worker_threads } => {
                let mut builder = Builder::new_multi_thread();
                builder.worker_threads(worker_threads);
                builder
            }
        };
        builder
            .enable_all()
            .build()
            .map_err(|source| RuntimeBootstrapError {
                policy: self,
                source,
            })
    }

    /// Bootstrap the role runtime and execute one production root future on it.
    pub fn run<F>(self, future: F) -> Result<F::Output, RuntimeBootstrapError>
    where
        F: Future,
    {
        Ok(self.bootstrap()?.block_on(future))
    }

    const fn scheduler_shape(self) -> SchedulerShape {
        match self {
            Self::WorkspaceSupervisor => SchedulerShape::MultiThread {
                worker_threads: SUPERVISOR_WORKER_THREADS,
            },
            Self::SupervisorControl => SchedulerShape::CurrentThread,
            Self::McpLauncher => SchedulerShape::MultiThread {
                worker_threads: MCP_LAUNCHER_WORKER_THREADS,
            },
            Self::FabricDaemon => SchedulerShape::MultiThread {
                worker_threads: DAEMON_WORKER_THREADS,
            },
        }
    }
}

/// Failure to construct the runtime assigned to a released process role.
#[derive(Debug, Error)]
#[error("Tokio runtime construction for {policy:?} failed: {source}")]
pub struct RuntimeBootstrapError {
    policy: RuntimeExecutionPolicy,
    #[source]
    source: io::Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn wp44_int_library_owns_the_closed_process_settings_grammar() {
        assert_eq!(
            CodefabricProcessSettings::parse(arguments(&[
                "supervisor",
                "serve",
                "--config",
                "/private/config.toml",
            ])),
            Ok(CodefabricProcessSettings::WorkspaceSupervisor {
                config_path: PathBuf::from("/private/config.toml"),
            })
        );
        assert_eq!(
            CodefabricProcessSettings::parse(arguments(&[
                "supervisor",
                "status",
                "--discovery",
                "/private/supervisor.json",
            ])),
            Ok(CodefabricProcessSettings::SupervisorControl {
                discovery_path: PathBuf::from("/private/supervisor.json"),
                command: SupervisorControlCommand::Status,
            })
        );
        assert_eq!(
            CodefabricProcessSettings::parse(arguments(&[
                "supervisor",
                "drain",
                "--discovery",
                "/private/supervisor.json",
            ])),
            Err(ProcessSettingsError::InvalidGrammar)
        );
        assert_eq!(
            CodefabricProcessSettings::parse(arguments(&[
                "supervisor",
                "stop",
                "--discovery",
                "/private/supervisor.json",
            ])),
            Err(ProcessSettingsError::InvalidGrammar)
        );
        assert_eq!(
            CodefabricProcessSettings::parse(arguments(&[
                "mcp",
                "serve",
                "--supervisor",
                "/private/supervisor.json",
                "--policy-id",
                "policy-one",
            ])),
            Ok(CodefabricProcessSettings::McpLauncher {
                discovery_path: PathBuf::from("/private/supervisor.json"),
                policy_id: LaunchPolicyId::try_new("policy-one").unwrap(),
            })
        );
        assert_eq!(
            FabricDaemonProcessSettings::parse(arguments(&[
                "serve",
                "--config",
                "/private/config.toml",
            ])),
            Ok(FabricDaemonProcessSettings {
                config_path: PathBuf::from("/private/config.toml"),
            })
        );
    }

    #[test]
    fn wp44_neg_process_settings_reject_defaults_overrides_and_ad_hoc_claims() {
        for invalid in [
            arguments(&["supervisor", "serve"]),
            arguments(&["supervisor", "serve", "--config", ""]),
            arguments(&[
                "supervisor",
                "serve",
                "--config",
                "/private/config.toml",
                "--workspace",
                "workspace:11",
            ]),
            arguments(&[
                "mcp",
                "serve",
                "--supervisor",
                "/private/supervisor.json",
                "--policy-id",
                "../policy",
            ]),
            arguments(&[
                "mcp",
                "serve",
                "--supervisor",
                "/private/supervisor.json",
                "--workspace",
                "workspace:11",
            ]),
        ] {
            assert!(CodefabricProcessSettings::parse(invalid).is_err());
        }
        assert!(
            FabricDaemonProcessSettings::parse(arguments(&[
                "check-config",
                "--config",
                "/private/config.toml",
            ]))
            .is_err()
        );
    }

    #[test]
    fn released_process_roles_have_fixed_bounded_scheduler_shapes() {
        assert_eq!(
            RuntimeExecutionPolicy::WorkspaceSupervisor.scheduler_shape(),
            SchedulerShape::MultiThread { worker_threads: 2 }
        );
        assert_eq!(
            RuntimeExecutionPolicy::SupervisorControl.scheduler_shape(),
            SchedulerShape::CurrentThread
        );
        assert_eq!(
            RuntimeExecutionPolicy::McpLauncher.scheduler_shape(),
            SchedulerShape::MultiThread { worker_threads: 2 }
        );
        assert_eq!(
            RuntimeExecutionPolicy::FabricDaemon.scheduler_shape(),
            SchedulerShape::MultiThread { worker_threads: 4 }
        );
    }

    #[test]
    fn every_released_process_role_bootstraps_and_executes() {
        for policy in [
            RuntimeExecutionPolicy::WorkspaceSupervisor,
            RuntimeExecutionPolicy::SupervisorControl,
            RuntimeExecutionPolicy::McpLauncher,
            RuntimeExecutionPolicy::FabricDaemon,
        ] {
            let output = policy.run(async { 42 }).expect("runtime bootstrap");
            assert_eq!(output, 42);
        }
    }
}
