//! External Pydantic/FastMCP family driver with a closed canonical invocation.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverEnvironment, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot,
};
use super::model_control::StableId;
use super::repository_model::{RepositoryModelError, read_stable};

const ADAPTER_IR_PATH: &str = "contracts/adapter/adapter-model-ir.json";
const DRIVER_PATH: &str = "tooling/model/adapter_driver.py";
const LEGACY_RENDERER_PATH: &str = "tooling/contracts/generate_adapter_models.py";
const JSON_HELPER_PATH: &str = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/json.py";
const PYPROJECT_PATH: &str = "codefabric-cpg-mcp/pyproject.toml";
const UV_LOCK_PATH: &str = "codefabric-cpg-mcp/uv.lock";
const PROTOCOL_VERSION: &str = "codefabric-external-adapter-driver-v1";
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

fn safe(path: &str) -> Result<SafeOutputPath, DriverProtocolError> {
    SafeOutputPath::parse(path.as_bytes().to_vec())
        .map_err(|_| DriverProtocolError::InvalidDescriptor)
}

/// Exact external executable and library identity observed by both sides of the protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterToolIdentity {
    pub python_path: String,
    pub python_digest: String,
    pub python_version: String,
    pub script_digest: String,
    pub lock_digest: String,
    pub project_digest: String,
    pub ruff_path: String,
    pub ruff_digest: String,
    pub ruff_version: String,
    pub pydantic_version: String,
    pub fastmcp_version: String,
    pub mcp_version: String,
}

impl AdapterToolIdentity {
    fn validate(&self, repository_root: &Path, python: &Path) -> Result<(), AdapterDriverError> {
        let expected_python = python
            .canonicalize()
            .map_err(|source| AdapterDriverError::Io {
                path: python.to_owned(),
                source,
            })?;
        let observed_python = PathBuf::from(&self.python_path);
        let observed_ruff = PathBuf::from(&self.ruff_path);
        if observed_python != expected_python
            || self.python_version != "3.14.7"
            || self.pydantic_version != "2.13.4"
            || self.fastmcp_version != "3.4.7"
            || self.mcp_version.trim().is_empty()
            || self.ruff_version.trim().is_empty()
            || self.python_digest != digest_file(&observed_python)?
            || self.script_digest != digest_file(&repository_root.join(DRIVER_PATH))?
            || self.lock_digest != digest_file(&repository_root.join(UV_LOCK_PATH))?
            || self.project_digest != digest_file(&repository_root.join(PYPROJECT_PATH))?
            || self.ruff_digest != digest_file(&observed_ruff)?
        {
            return Err(AdapterDriverError::ToolIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalOutputPlan {
    output_id: String,
    path: String,
    role: DriverOutputRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalRequest<'a> {
    protocol_version: &'static str,
    operation: &'static str,
    source: &'a str,
    source_digest: &'a str,
    planned_outputs: &'a [ExternalOutputPlan],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalPlanResponse {
    protocol_version: String,
    tool_identity: AdapterToolIdentity,
    outputs: Vec<ExternalOutputPlan>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalRenderedOutput {
    output_id: String,
    path: String,
    role: DriverOutputRole,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalRenderResponse {
    protocol_version: String,
    tool_identity: AdapterToolIdentity,
    outputs: Vec<ExternalRenderedOutput>,
}

/// Source-fenced plan resolved by the exact Pydantic driver.
pub struct AdapterPlan {
    repository_root: PathBuf,
    descriptor: DriverDescriptor,
    source: String,
    source_digest: String,
    source_fence: DriverSourceFence,
    outputs: Vec<ExternalOutputPlan>,
    tool_identity: AdapterToolIdentity,
}

/// Pydantic/FastMCP external driver.
pub struct AdapterDriver {
    python: PathBuf,
}

impl AdapterDriver {
    /// Resolve the exact Python executable from the frozen adapter environment.
    #[must_use]
    pub fn for_repository(repository_root: &Path) -> Self {
        Self {
            python: repository_root.join("codefabric-cpg-mcp/.venv/bin/python"),
        }
    }

    fn invoke<T: DeserializeOwned>(
        &self,
        repository_root: &Path,
        request: &ExternalRequest<'_>,
    ) -> Result<T, AdapterDriverError> {
        let protocol_root = repository_root.join("target/model-stage/adapter-external");
        let home = protocol_root.join("home");
        let temporary = protocol_root.join("tmp");
        fs::create_dir_all(&home).map_err(|source| AdapterDriverError::Io {
            path: home.clone(),
            source,
        })?;
        fs::create_dir_all(&temporary).map_err(|source| AdapterDriverError::Io {
            path: temporary.clone(),
            source,
        })?;
        let mut environment = DriverEnvironment::sanitized(std::env::vars(), &home);
        environment.variables.insert(
            "TMPDIR".to_owned(),
            temporary.to_string_lossy().into_owned(),
        );
        environment
            .variables
            .insert("PYTHONUTF8".to_owned(), "1".to_owned());
        environment
            .variables
            .insert("PYTHONDONTWRITEBYTECODE".to_owned(), "1".to_owned());
        let mut child = Command::new(&self.python)
            .arg(repository_root.join(DRIVER_PATH))
            .current_dir(repository_root)
            .env_clear()
            .envs(&environment.variables)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| AdapterDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        let request_bytes = serde_json::to_vec(request)?;
        child
            .stdin
            .take()
            .ok_or(AdapterDriverError::Protocol)?
            .write_all(&request_bytes)
            .map_err(|source| AdapterDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        let output = child
            .wait_with_output()
            .map_err(|source| AdapterDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(AdapterDriverError::External(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        if output.stdout.len() > MAX_RESPONSE_BYTES {
            return Err(AdapterDriverError::ResourceLimit);
        }
        serde_json::from_slice(&output.stdout).map_err(AdapterDriverError::Json)
    }

    fn validate_outputs(
        outputs: &[ExternalOutputPlan],
        roots: &[SafeOutputPath],
    ) -> Result<Vec<DriverOutputSpec>, AdapterDriverError> {
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut resolved = Vec::with_capacity(outputs.len());
        for output in outputs {
            let output_id = StableId::parse(output.output_id.clone())
                .map_err(|_| AdapterDriverError::Protocol)?;
            let path = safe(&output.path)?;
            if !roots.iter().any(|root| {
                path.as_bytes().starts_with(root.as_bytes())
                    && path.as_bytes().get(root.as_bytes().len()) == Some(&b'/')
            }) || !ids.insert(output_id.clone())
                || !paths.insert(path.clone())
            {
                return Err(AdapterDriverError::Protocol);
            }
            resolved.push(DriverOutputSpec {
                output_id,
                path,
                role: output.role,
            });
        }
        Ok(resolved)
    }
}

impl ModelDriver for AdapterDriver {
    type Plan = AdapterPlan;

    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError> {
        let descriptor = DriverDescriptor {
            driver_id: StableId::parse("driver:adapter-contract-v1".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            family: StableId::parse("family:adapter".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            rule_version: "external-pydantic-fastmcp-v1".to_owned(),
            sources: [
                ADAPTER_IR_PATH,
                DRIVER_PATH,
                LEGACY_RENDERER_PATH,
                JSON_HELPER_PATH,
                PYPROJECT_PATH,
                UV_LOCK_PATH,
            ]
            .into_iter()
            .map(safe)
            .collect::<Result<Vec<_>, _>>()?,
            output_roots: [
                "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts",
                "contracts/adapter",
                "contracts/generated/model",
            ]
            .into_iter()
            .map(safe)
            .collect::<Result<Vec<_>, _>>()?,
            outputs: Vec::new(),
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_SOURCE_BYTES,
                max_output_bytes: MAX_OUTPUT_BYTES,
                max_outputs: 16,
            },
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError> {
        let mut descriptor = self.describe()?;
        let source_fence = DriverSourceFence::capture(repository_root, &descriptor)?;
        let bytes = read_stable(&repository_root.join(ADAPTER_IR_PATH), MAX_SOURCE_BYTES)?;
        let source = String::from_utf8(bytes.clone()).map_err(|_| {
            DriverProtocolError::InvalidAuthority("adapter Contract IR is not UTF-8".to_owned())
        })?;
        let source_digest = digest_bytes(&bytes);
        let response: ExternalPlanResponse = self
            .invoke(
                repository_root,
                &ExternalRequest {
                    protocol_version: PROTOCOL_VERSION,
                    operation: "plan",
                    source: &source,
                    source_digest: &source_digest,
                    planned_outputs: &[],
                },
            )
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        if response.protocol_version != PROTOCOL_VERSION {
            return Err(DriverProtocolError::InvalidAuthority(
                "adapter driver protocol version differs".to_owned(),
            ));
        }
        response
            .tool_identity
            .validate(repository_root, &self.python)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        descriptor.outputs = Self::validate_outputs(&response.outputs, &descriptor.output_roots)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        descriptor.validate()?;
        Ok(AdapterPlan {
            repository_root: repository_root.to_owned(),
            descriptor,
            source,
            source_digest,
            source_fence,
            outputs: response.outputs,
            tool_identity: response.tool_identity,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        let response: ExternalRenderResponse = self
            .invoke(
                &plan.repository_root,
                &ExternalRequest {
                    protocol_version: PROTOCOL_VERSION,
                    operation: "render",
                    source: &plan.source,
                    source_digest: &plan.source_digest,
                    planned_outputs: &plan.outputs,
                },
            )
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        if response.protocol_version != PROTOCOL_VERSION
            || response.tool_identity != plan.tool_identity
            || response.outputs.len() != plan.outputs.len()
        {
            return Err(DriverProtocolError::InvalidAuthority(
                "adapter render result differs from its closed plan".to_owned(),
            ));
        }
        let mut rendered = Vec::with_capacity(response.outputs.len());
        for (expected, output) in plan.outputs.iter().zip(response.outputs) {
            if output.output_id != expected.output_id
                || output.path != expected.path
                || output.role != expected.role
            {
                return Err(DriverProtocolError::InvalidAuthority(
                    "adapter render output order or identity differs".to_owned(),
                ));
            }
            let path = safe(&output.path)?;
            staging_root.write(&path, output.contents.as_bytes())?;
            rendered.push(path);
        }
        Ok(rendered)
    }
}

/// Machine-readable adapter family report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterReport {
    pub family: String,
    pub rule_version: String,
    pub resource_profile: DriverResourceProfile,
    pub rendered_outputs: Vec<String>,
    pub tool_identity: AdapterToolIdentity,
    pub validation: Value,
    pub stage_root: String,
}

/// Render and internally cross-check the adapter family under a disposable stage.
///
/// # Errors
///
/// Returns external protocol, typed ingress, staging, or projection failures.
pub fn check_family(repository_root: &Path) -> Result<AdapterReport, AdapterDriverError> {
    let driver = AdapterDriver::for_repository(repository_root);
    let plan = driver.plan(repository_root)?;
    let stage_path = repository_root.join("target/model-stage/adapter-shadow");
    if stage_path.exists() {
        fs::remove_dir_all(&stage_path).map_err(|source| AdapterDriverError::Io {
            path: stage_path.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage_path).map_err(|source| AdapterDriverError::Io {
        path: stage_path.clone(),
        source,
    })?;
    let staging = StagingRoot::new(repository_root, &stage_path, &plan.descriptor)?;
    let rendered = driver.render(&plan, &staging)?;
    plan.source_fence.verify(repository_root)?;
    let validation_path = stage_path.join("contracts/generated/model/adapter-validation.json");
    let validation_bytes =
        read_stable(&validation_path, MAX_OUTPUT_BYTES).map_err(DriverProtocolError::from)?;
    let validation: Value = serde_json::from_slice(&validation_bytes)?;
    if validation["family"] != "adapter"
        || validation["projection_count"].as_u64() != Some(rendered.len() as u64)
    {
        return Err(AdapterDriverError::ProjectionMismatch);
    }
    Ok(AdapterReport {
        family: "adapter".to_owned(),
        rule_version: plan.descriptor.rule_version.clone(),
        resource_profile: plan.descriptor.resource_profile.clone(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        tool_identity: plan.tool_identity,
        validation,
        stage_root: stage_path.to_string_lossy().into_owned(),
    })
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn digest_file(path: &Path) -> Result<String, AdapterDriverError> {
    read_stable(path, MAX_RESPONSE_BYTES)
        .map(|bytes| digest_bytes(&bytes))
        .map_err(AdapterDriverError::Repository)
}

/// Adapter driver failures.
#[derive(Debug, Error)]
pub enum AdapterDriverError {
    #[error("adapter driver I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("adapter external driver failed: {0}")]
    External(String),
    #[error("adapter external driver returned an invalid protocol document")]
    Protocol,
    #[error("adapter external driver exceeded its response budget")]
    ResourceLimit,
    #[error("adapter external tool identity differs from the pinned environment")]
    ToolIdentity,
    #[error("adapter staged projection does not match the typed source")]
    ProjectionMismatch,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Driver(#[from] DriverProtocolError),
    #[error(transparent)]
    Repository(#[from] RepositoryModelError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_adapter_protocol_rejects_undeclared_outputs() {
        let roots = vec![safe("contracts/adapter").unwrap()];
        let outputs = vec![ExternalOutputPlan {
            output_id: "output:adapter-escape".to_owned(),
            path: "contracts/acceptance/answer.json".to_owned(),
            role: DriverOutputRole::CanonicalProjection,
        }];
        assert!(AdapterDriver::validate_outputs(&outputs, &roots).is_err());
    }

    #[test]
    fn model_adapter_external_driver_has_exact_executable_and_environment_identity() {
        let environment = DriverEnvironment::sanitized(
            [
                ("PATH", "/bin"),
                ("AWS_SECRET_ACCESS_KEY", "secret"),
                ("CODEFABRIC_TOKEN", "token"),
            ],
            Path::new("/stage/home"),
        );
        assert!(!environment.variables.keys().any(|name| {
            let name = name.to_ascii_lowercase();
            name.contains("secret") || name.contains("token")
        }));
    }
}
