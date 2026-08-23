//! Model-derived production Proto unit with one Python descriptor compiler authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverEnvironment, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot, executable_tool_identity,
    process_stage_root,
};
use super::incremental::{CacheLookup, render_with_cache};
use super::model_control::StableId;
use super::repository_model::{RepositoryModelError, read_stable};

const PROTOCOL_VERSION: &str = "codefabric-external-proto-driver-v1";
const DRIVER_PATH: &str = "tooling/model/proto_driver.py";
const PROTO_CONTRACT_LIBRARY_PATH: &str = "tooling/model/proto_contract.py";
const RUST_GENERATOR_PATH: &str = "tooling/proto/generate.rs";
const BASELINE_PATH: &str = "tooling/proto/compatibility-baseline.json";
const PYPROJECT_PATH: &str = "codefabric-cpg-mcp/pyproject.toml";
const UV_LOCK_PATH: &str = "codefabric-cpg-mcp/uv.lock";
const CARGO_MANIFEST_PATH: &str = "Cargo.toml";
const CARGO_LOCK_PATH: &str = "Cargo.lock";
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

fn safe(path: &str) -> Result<SafeOutputPath, DriverProtocolError> {
    SafeOutputPath::parse(path.as_bytes().to_vec())
        .map_err(|_| DriverProtocolError::InvalidDescriptor)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalProtoSource {
    path: String,
    contents: String,
    source_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalOutputPlan {
    output_id: String,
    path: String,
    role: DriverOutputRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtoPythonToolIdentity {
    python_path: String,
    python_digest: String,
    python_version: String,
    script_digest: String,
    lock_digest: String,
    project_digest: String,
    grpcio: String,
    #[serde(rename = "grpcio-tools")]
    grpcio_tools: String,
    protobuf: String,
    protoc: String,
}

impl ProtoPythonToolIdentity {
    fn validate(&self, root: &Path, python: &Path) -> Result<(), ProtoDriverError> {
        let expected_python = python
            .canonicalize()
            .map_err(|source| ProtoDriverError::Io {
                path: python.to_owned(),
                source,
            })?;
        if Path::new(&self.python_path) != expected_python
            || self.python_version != "3.14.7"
            || self.grpcio != "1.83.0"
            || self.grpcio_tools != "1.83.0"
            || self.protobuf != "7.36.0"
            || self.protoc != "libprotoc 35.1"
            || self.python_digest != digest_file(&expected_python)?
            || self.script_digest != digest_file(&root.join(DRIVER_PATH))?
            || self.lock_digest != digest_file(&root.join(UV_LOCK_PATH))?
            || self.project_digest != digest_file(&root.join(PYPROJECT_PATH))?
        {
            return Err(ProtoDriverError::ToolIdentity);
        }
        Ok(())
    }

    fn portable(&self) -> Value {
        json!({
            "python_digest": self.python_digest,
            "python_version": self.python_version,
            "script_digest": self.script_digest,
            "lock_digest": self.lock_digest,
            "project_digest": self.project_digest,
            "grpcio": self.grpcio,
            "grpcio-tools": self.grpcio_tools,
            "protobuf": self.protobuf,
            "protoc": self.protoc,
        })
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalRequest<'a> {
    protocol_version: &'static str,
    operation: &'static str,
    sources: &'a [ExternalProtoSource],
    planned_outputs: &'a [ExternalOutputPlan],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalPlanResponse {
    protocol_version: String,
    tool_identity: ProtoPythonToolIdentity,
    outputs: Vec<ExternalOutputPlan>,
    compiler_invocations: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalRenderedOutput {
    output_id: String,
    path: String,
    role: DriverOutputRole,
    contents_hex: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalRenderResponse {
    protocol_version: String,
    tool_identity: ProtoPythonToolIdentity,
    outputs: Vec<ExternalRenderedOutput>,
    compiler_invocations: u64,
    descriptor_sha256: String,
}

pub struct ProtoPlan {
    repository_root: PathBuf,
    descriptor: DriverDescriptor,
    sources: Vec<ExternalProtoSource>,
    outputs: Vec<ExternalOutputPlan>,
    source_fence: DriverSourceFence,
    python_identity: ProtoPythonToolIdentity,
}

/// Driver whose source and output members are discovered from the closed Proto family rules.
pub struct ProtoDriver {
    repository_root: PathBuf,
    python: PathBuf,
}

impl ProtoDriver {
    #[must_use]
    pub fn for_repository(repository_root: &Path) -> Self {
        Self {
            repository_root: repository_root.to_owned(),
            python: repository_root.join("codefabric-cpg-mcp/.venv/bin/python"),
        }
    }

    fn proto_paths(&self) -> Result<Vec<String>, ProtoDriverError> {
        let root = self.repository_root.join("contracts/rpc");
        let mut paths = Vec::new();
        for entry in fs::read_dir(&root).map_err(|source| ProtoDriverError::Io {
            path: root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| ProtoDriverError::Io {
                path: root.clone(),
                source,
            })?;
            let metadata = entry.file_type().map_err(|source| ProtoDriverError::Io {
                path: entry.path(),
                source,
            })?;
            if metadata.is_symlink() {
                return Err(ProtoDriverError::Protocol);
            }
            if metadata.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "proto")
            {
                let relative = entry
                    .path()
                    .strip_prefix(&self.repository_root)
                    .map_err(|_| ProtoDriverError::Protocol)?
                    .to_str()
                    .ok_or(ProtoDriverError::Protocol)?
                    .replace(std::path::MAIN_SEPARATOR, "/");
                paths.push(relative);
            }
        }
        paths.sort();
        if paths.is_empty() {
            return Err(ProtoDriverError::Protocol);
        }
        Ok(paths)
    }

    fn invoke<T: DeserializeOwned>(
        &self,
        request: &ExternalRequest<'_>,
    ) -> Result<T, ProtoDriverError> {
        let protocol_root = process_stage_root(&self.repository_root, "proto-external");
        let home = protocol_root.join("home");
        let temporary = protocol_root.join("tmp");
        fs::create_dir_all(&home).map_err(|source| ProtoDriverError::Io {
            path: home.clone(),
            source,
        })?;
        fs::create_dir_all(&temporary).map_err(|source| ProtoDriverError::Io {
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
            .insert("PYTHONDONTWRITEBYTECODE".to_owned(), "1".to_owned());
        environment.variables.insert(
            "PYTHONPATH".to_owned(),
            self.repository_root.to_string_lossy().into_owned(),
        );
        let mut child = Command::new(&self.python)
            .arg(self.repository_root.join(DRIVER_PATH))
            .current_dir(&self.repository_root)
            .env_clear()
            .envs(&environment.variables)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ProtoDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        child
            .stdin
            .take()
            .ok_or(ProtoDriverError::Protocol)?
            .write_all(&serde_json::to_vec(request)?)
            .map_err(|source| ProtoDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        let output = child
            .wait_with_output()
            .map_err(|source| ProtoDriverError::Io {
                path: self.python.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(ProtoDriverError::External(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        if output.stdout.len() > MAX_RESPONSE_BYTES {
            return Err(ProtoDriverError::ResourceLimit);
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    fn validate_outputs(
        outputs: &[ExternalOutputPlan],
        roots: &[SafeOutputPath],
    ) -> Result<Vec<DriverOutputSpec>, ProtoDriverError> {
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut resolved = Vec::with_capacity(outputs.len());
        for output in outputs {
            let output_id = StableId::parse(output.output_id.clone())
                .map_err(|_| ProtoDriverError::Protocol)?;
            let path = safe(&output.path)?;
            if !roots.iter().any(|root| {
                path.as_bytes().starts_with(root.as_bytes())
                    && path.as_bytes().get(root.as_bytes().len()) == Some(&b'/')
            }) || !ids.insert(output_id.clone())
                || !paths.insert(path.clone())
            {
                return Err(ProtoDriverError::Protocol);
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

impl ModelDriver for ProtoDriver {
    type Plan = ProtoPlan;

    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError> {
        let mut sources = [
            DRIVER_PATH,
            PROTO_CONTRACT_LIBRARY_PATH,
            RUST_GENERATOR_PATH,
            BASELINE_PATH,
            PYPROJECT_PATH,
            UV_LOCK_PATH,
            CARGO_MANIFEST_PATH,
            CARGO_LOCK_PATH,
        ]
        .into_iter()
        .map(safe)
        .collect::<Result<Vec<_>, _>>()?;
        sources.extend(
            self.proto_paths()
                .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?
                .iter()
                .map(|path| safe(path))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let descriptor = DriverDescriptor {
            driver_id: StableId::parse("driver:production-proto-v1".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            family: StableId::parse("family:proto".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            rule_version: "one-fds-grpc-tools-compile-fds-v1".to_owned(),
            sources,
            output_roots: [
                "tooling/proto",
                "src/generated",
                "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated",
            ]
            .into_iter()
            .map(safe)
            .collect::<Result<Vec<_>, _>>()?,
            outputs: Vec::new(),
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_SOURCE_BYTES,
                max_output_bytes: MAX_OUTPUT_BYTES,
                max_outputs: 64,
            },
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError> {
        let mut descriptor = self.describe()?;
        let source_fence = DriverSourceFence::capture(repository_root, &descriptor)?;
        let sources = self
            .proto_paths()
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?
            .into_iter()
            .map(|path| {
                let bytes = read_stable(&repository_root.join(&path), MAX_SOURCE_BYTES)?;
                let contents = String::from_utf8(bytes.clone())
                    .map_err(|_| RepositoryModelError::NonUtf8Content(path.clone()))?;
                Ok(ExternalProtoSource {
                    path,
                    contents,
                    source_digest: digest_bytes(&bytes),
                })
            })
            .collect::<Result<Vec<_>, RepositoryModelError>>()
            .map_err(DriverProtocolError::from)?;
        let response: ExternalPlanResponse = self
            .invoke(&ExternalRequest {
                protocol_version: PROTOCOL_VERSION,
                operation: "plan",
                sources: &sources,
                planned_outputs: &[],
            })
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        if response.protocol_version != PROTOCOL_VERSION || response.compiler_invocations != 0 {
            return Err(DriverProtocolError::InvalidAuthority(
                "Proto planning executed or changed compiler protocol".to_owned(),
            ));
        }
        response
            .tool_identity
            .validate(repository_root, &self.python)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        descriptor.outputs = Self::validate_outputs(&response.outputs, &descriptor.output_roots)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        descriptor.validate()?;
        Ok(ProtoPlan {
            repository_root: repository_root.to_owned(),
            descriptor,
            sources,
            outputs: response.outputs,
            source_fence,
            python_identity: response.tool_identity,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        render_plan(self, plan, staging_root)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))
    }
}

fn render_plan(
    driver: &ProtoDriver,
    plan: &ProtoPlan,
    staging: &StagingRoot,
) -> Result<Vec<SafeOutputPath>, ProtoDriverError> {
    let response: ExternalRenderResponse = driver.invoke(&ExternalRequest {
        protocol_version: PROTOCOL_VERSION,
        operation: "render",
        sources: &plan.sources,
        planned_outputs: &plan.outputs,
    })?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.compiler_invocations != 1
        || response.tool_identity != plan.python_identity
    {
        return Err(ProtoDriverError::Protocol);
    }
    let expected = plan
        .outputs
        .iter()
        .map(|output| (output.path.as_str(), output))
        .collect::<BTreeMap<_, _>>();
    let mut rendered = Vec::new();
    for output in response.outputs {
        let planned = expected
            .get(output.path.as_str())
            .ok_or(ProtoDriverError::Protocol)?;
        if output.output_id != planned.output_id || output.role != planned.role {
            return Err(ProtoDriverError::Protocol);
        }
        let path = safe(&output.path)?;
        staging.write(&path, &decode_hex(&output.contents_hex)?)?;
        rendered.push(path);
    }
    let descriptor_path = safe("tooling/proto/production-descriptor.pb")?;
    if !rendered.contains(&descriptor_path) {
        return Err(ProtoDriverError::Protocol);
    }
    let rust_identity = generate_rust(plan, staging, &descriptor_path, &mut rendered)?;
    let identity_path = safe("tooling/proto/toolchain-identity.json")?;
    let sources = plan
        .sources
        .iter()
        .map(|source| (source.path.clone(), source.source_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let identity = json!({
        "schema": 4,
        "authority": "one model-derived grpc_tools.protoc invocation emits the sole FDS and Python bindings; the same FDS drives tonic_prost_build::Builder::compile_fds",
        "sources": sources,
        "descriptor_sha256": response.descriptor_sha256,
        "python": plan.python_identity.portable(),
        "rust": rust_identity,
    });
    staging.write(&identity_path, &encoded_json(&identity)?)?;
    rendered.push(identity_path);
    rendered.sort();
    if rendered.len() != plan.outputs.len() {
        return Err(ProtoDriverError::ProjectionMismatch);
    }
    plan.source_fence.verify(&plan.repository_root)?;
    Ok(rendered)
}

fn generate_rust(
    plan: &ProtoPlan,
    staging: &StagingRoot,
    descriptor_path: &SafeOutputPath,
    rendered: &mut Vec<SafeOutputPath>,
) -> Result<Value, ProtoDriverError> {
    let generator = build_rust_generator(plan)?;
    let executable = &generator.executable;
    let rustc = &generator.rustc;
    let action_key = &generator.action_key;
    let generated = process_stage_root(&plan.repository_root, "proto-rust-output");
    if generated.exists() {
        fs::remove_dir_all(&generated).map_err(|source| ProtoDriverError::Io {
            path: generated.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&generated).map_err(|source| ProtoDriverError::Io {
        path: generated.clone(),
        source,
    })?;
    let roundtrip = generated.join("roundtrip.pb");
    let descriptor = staging.output_path(descriptor_path)?;
    let status = Command::new(executable)
        .arg("--descriptor")
        .arg(&descriptor)
        .arg("--roundtrip-descriptor-out")
        .arg(&roundtrip)
        .arg("--rust-out")
        .arg(&generated)
        .env_clear()
        .status()
        .map_err(|source| ProtoDriverError::Io {
            path: executable.clone(),
            source,
        })?;
    if !status.success()
        || read_stable(&roundtrip, MAX_OUTPUT_BYTES)? != read_stable(&descriptor, MAX_OUTPUT_BYTES)?
    {
        return Err(ProtoDriverError::RustGenerator);
    }
    let header = generated_header(&plan.sources, "//")?;
    for output in plan
        .outputs
        .iter()
        .filter(|output| output.role == DriverOutputRole::RustBinding)
    {
        let name = Path::new(&output.path)
            .file_name()
            .ok_or(ProtoDriverError::Protocol)?;
        let source = generated.join(name);
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend(read_stable(&source, MAX_OUTPUT_BYTES)?);
        fs::write(&source, bytes).map_err(|source_error| ProtoDriverError::Io {
            path: source.clone(),
            source: source_error,
        })?;
        let status = Command::new("rustfmt")
            .args(["--edition", "2024"])
            .arg(&source)
            .status()
            .map_err(|source_error| ProtoDriverError::Io {
                path: source.clone(),
                source: source_error,
            })?;
        if !status.success() {
            return Err(ProtoDriverError::RustGenerator);
        }
        let path = safe(&output.path)?;
        staging.write(&path, &read_stable(&source, MAX_OUTPUT_BYTES)?)?;
        rendered.push(path);
    }
    let host = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or("unknown");
    Ok(json!({
        "action_key": format!("b3:{action_key}"),
        "binary_digest": digest_file(executable)?,
        "cargo_lock_digest": digest_file(&plan.repository_root.join(CARGO_LOCK_PATH))?,
        "cargo_manifest_digest": digest_file(&plan.repository_root.join(CARGO_MANIFEST_PATH))?,
        "features": ["proto-tooling"],
        "profile": "debug",
        "target_triple": host,
        "rustc": rustc.trim(),
        "descriptor_api": "tonic_prost_build::Builder::compile_fds",
    }))
}

struct RustGenerator {
    executable: PathBuf,
    rustc: String,
    action_key: String,
}

fn build_rust_generator(plan: &ProtoPlan) -> Result<RustGenerator, ProtoDriverError> {
    let rustc = command_output(Command::new("rustc").args(["--version", "--verbose"]))?;
    let mut identity_material = Vec::new();
    identity_material.extend(read_stable(
        &plan.repository_root.join(CARGO_LOCK_PATH),
        MAX_SOURCE_BYTES,
    )?);
    identity_material.extend(read_stable(
        &plan.repository_root.join(CARGO_MANIFEST_PATH),
        MAX_SOURCE_BYTES,
    )?);
    identity_material.extend(read_stable(
        &plan.repository_root.join(RUST_GENERATOR_PATH),
        MAX_SOURCE_BYTES,
    )?);
    identity_material.extend(rustc.as_bytes());
    identity_material.extend(b"proto-tooling|debug|host");
    let action_key = blake3::hash(&identity_material).to_hex().to_string();
    let target = plan
        .repository_root
        .join("target/model-tools/proto")
        .join(&action_key);
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--no-default-features",
            "--features",
            "proto-tooling",
            "--bin",
            "codefabric-proto-gen",
            "--target-dir",
        ])
        .arg(&target)
        .current_dir(&plan.repository_root)
        .status()
        .map_err(|source| ProtoDriverError::Io {
            path: PathBuf::from("cargo"),
            source,
        })?;
    if !status.success() {
        return Err(ProtoDriverError::RustGenerator);
    }
    let executable = target.join("debug/codefabric-proto-gen");
    Ok(RustGenerator {
        executable,
        rustc,
        action_key,
    })
}

fn cache_tool_identity(plan: &ProtoPlan) -> Result<Value, ProtoDriverError> {
    let generator = build_rust_generator(plan)?;
    Ok(json!({
        "python": plan.python_identity.portable(),
        "rust_generator": {
            "action_key": format!("b3:{}", generator.action_key),
            "executable_digest": digest_file(&generator.executable)?,
            "cargo_lock_digest": digest_file(&plan.repository_root.join(CARGO_LOCK_PATH))?,
            "cargo_manifest_digest": digest_file(&plan.repository_root.join(CARGO_MANIFEST_PATH))?,
            "features": ["proto-tooling"],
            "profile": "debug",
            "rustc": generator.rustc.trim(),
        },
        "rustfmt": executable_tool_identity("rustfmt", &["--version"])?
    }))
}

fn generated_header(
    sources: &[ExternalProtoSource],
    comment: &str,
) -> Result<String, ProtoDriverError> {
    let mut identities = Vec::new();
    for source in sources {
        let matches = source
            .contents
            .lines()
            .filter_map(|line| line.strip_prefix("// canonical_digest: "))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(ProtoDriverError::Protocol);
        }
        identities.push(matches[0]);
    }
    Ok(format!(
        "{comment} @generated from catalog primary semantic identity {}; do not edit.\n",
        identities.join(",")
    ))
}

fn command_output(command: &mut Command) -> Result<String, ProtoDriverError> {
    let output = command.output().map_err(|source| ProtoDriverError::Io {
        path: PathBuf::from("external-command"),
        source,
    })?;
    if !output.status.success() {
        return Err(ProtoDriverError::RustGenerator);
    }
    String::from_utf8(output.stdout).map_err(|_| ProtoDriverError::Protocol)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ProtoDriverError> {
    if !value.len().is_multiple_of(2) || value.len() > MAX_RESPONSE_BYTES.saturating_mul(2) {
        return Err(ProtoDriverError::Protocol);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).map_err(|_| ProtoDriverError::Protocol)?;
            u8::from_str_radix(text, 16).map_err(|_| ProtoDriverError::Protocol)
        })
        .collect()
}

fn encoded_json(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn digest_file(path: &Path) -> Result<String, ProtoDriverError> {
    Ok(digest_bytes(&read_stable(path, MAX_RESPONSE_BYTES)?))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtoReport {
    pub family: String,
    pub rule_version: String,
    pub resource_profile: DriverResourceProfile,
    pub source_count: usize,
    pub rendered_outputs: Vec<String>,
    pub descriptor_file_count: usize,
    pub package_count: usize,
    pub compiler_invocations: u64,
    pub tool_identity: Value,
    pub cache_lookup: CacheLookup,
    pub stage_root: String,
}

/// Render the complete production unit and cross-check it against independent consumers.
///
/// # Errors
///
/// Returns a typed protocol, tool-identity, staging, descriptor, or consumer error.
pub fn check_family(repository_root: &Path) -> Result<ProtoReport, ProtoDriverError> {
    let driver = ProtoDriver::for_repository(repository_root);
    let plan = driver.plan(repository_root)?;
    let stage = process_stage_root(repository_root, "proto-shadow");
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|source| ProtoDriverError::Io {
            path: stage.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage).map_err(|source| ProtoDriverError::Io {
        path: stage.clone(),
        source,
    })?;
    let staging = StagingRoot::new(repository_root, &stage, &plan.descriptor)?;
    let (rendered, cache_lookup) = render_with_cache(
        repository_root,
        "proto",
        &plan.descriptor,
        &plan.source_fence,
        &staging,
        || {
            cache_tool_identity(&plan)
                .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))
        },
        || driver.render(&plan, &staging),
    )?;
    let census: Value = serde_json::from_slice(&read_stable(
        &stage.join("tooling/proto/descriptor-census.json"),
        MAX_OUTPUT_BYTES,
    )?)?;
    let files = census["files"]
        .as_array()
        .ok_or(ProtoDriverError::ProjectionMismatch)?;
    let packages = files
        .iter()
        .filter_map(|file| file["package"].as_str())
        .collect::<BTreeSet<_>>();
    let tool_identity: Value = serde_json::from_slice(&read_stable(
        &stage.join("tooling/proto/toolchain-identity.json"),
        MAX_OUTPUT_BYTES,
    )?)?;
    if files.len() != plan.sources.len()
        || packages.len() != plan.sources.len()
        || tool_identity["schema"] != 4
    {
        return Err(ProtoDriverError::ProjectionMismatch);
    }
    Ok(ProtoReport {
        family: "proto".to_owned(),
        rule_version: plan.descriptor.rule_version.clone(),
        resource_profile: plan.descriptor.resource_profile.clone(),
        source_count: plan.sources.len(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        descriptor_file_count: files.len(),
        package_count: packages.len(),
        compiler_invocations: u64::from(!matches!(&cache_lookup, CacheLookup::Hit { .. })),
        tool_identity,
        cache_lookup,
        stage_root: stage.to_string_lossy().into_owned(),
    })
}

#[derive(Debug, Error)]
pub enum ProtoDriverError {
    #[error("Proto driver I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("external Proto driver failed: {0}")]
    External(String),
    #[error("Proto driver protocol is invalid")]
    Protocol,
    #[error("Proto driver resource limit exceeded")]
    ResourceLimit,
    #[error("Proto tool identity differs from the exact frozen environment")]
    ToolIdentity,
    #[error("isolated Rust compile_fds generator failed")]
    RustGenerator,
    #[error("Proto staged projection does not match the typed source unit")]
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
    fn model_proto_plan_contains_no_wave0_filename_dispatch() {
        let source = include_str!("../../../tooling/model/proto_driver.py");
        assert!(!source.contains("wave0"));
        assert!(!source.contains("SOURCE_RELATIVE"));
    }

    #[test]
    fn model_proto_rejects_import_escape_duplicate_output_and_package_collision() {
        let duplicate = vec![
            ExternalOutputPlan {
                output_id: "output:one".to_owned(),
                path: "tooling/proto/one.pb".to_owned(),
                role: DriverOutputRole::ProtoDescriptor,
            },
            ExternalOutputPlan {
                output_id: "output:two".to_owned(),
                path: "tooling/proto/one.pb".to_owned(),
                role: DriverOutputRole::ProtoDescriptor,
            },
        ];
        assert!(
            ProtoDriver::validate_outputs(&duplicate, &[safe("tooling/proto").unwrap()]).is_err()
        );
        assert!(safe("../escape.proto").is_err());
    }

    #[test]
    fn model_proto_has_one_descriptor_compiler_identity() {
        assert_eq!(PROTOCOL_VERSION, "codefabric-external-proto-driver-v1");
        assert!(include_str!("../../../tooling/proto/generate.rs").contains("compile_fds"));
    }

    #[test]
    fn model_proto_feature_distinct_rust_consumers_use_isolated_executables() {
        let material_a = blake3::hash(b"proto-tooling|debug|host");
        let material_b = blake3::hash(b"rpc|debug|host");
        assert_ne!(material_a, material_b);
    }

    #[test]
    fn model_proto_provenance_never_serializes_host_specific_executable_paths() {
        let source = include_str!("proto_driver.rs");
        assert!(!source.contains("\"binary_path\""));
        assert!(source.contains("\"binary_digest\""));

        let identity = ProtoPythonToolIdentity {
            python_path: "/host-specific/venv/python".to_owned(),
            python_digest: "b3:python".to_owned(),
            python_version: "3.14.7".to_owned(),
            script_digest: "b3:script".to_owned(),
            lock_digest: "b3:lock".to_owned(),
            project_digest: "b3:project".to_owned(),
            grpcio: "1.83.0".to_owned(),
            grpcio_tools: "1.83.0".to_owned(),
            protobuf: "7.36.0".to_owned(),
            protoc: "libprotoc 35.1".to_owned(),
        };
        let portable = identity.portable();
        assert!(portable.get("python_path").is_none());
        assert_eq!(portable["python_digest"], "b3:python");
    }
}
