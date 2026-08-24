#![feature(rustc_private)]
#![deny(unsafe_code)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_public;

mod protocol;
mod rustc_link;
mod wrapper;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, ExitCode};

use serde::{Deserialize, Serialize};

const IDENTITY: &str = include_str!("../toolchain-identity.json");

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Identity,
    ExtractJson,
    Wrapper {
        real_rustc: OsString,
        arguments: Vec<OsString>,
    },
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some(value) if value == "--identity" && args.next().is_none() => Ok(Command::Identity),
        Some(value) if value == "--extract-json" && args.next().is_none() => {
            Ok(Command::ExtractJson)
        }
        Some(real_rustc) => Ok(Command::Wrapper {
            real_rustc: real_rustc.to_os_string(),
            arguments: args.collect(),
        }),
        None => Err("expected --identity, --extract-json, or a real rustc invocation"),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractRequest {
    protocol_version: String,
    workspace_root: PathBuf,
    source_path: PathBuf,
    crate_name: String,
    crate_type: String,
    edition: String,
    cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ExtractResponse {
    protocol_version: &'static str,
    toolchain_identity_digest: String,
    source_digest: String,
    compiler_exit_status: i32,
    diagnostics: Vec<String>,
    items: Vec<rustc_link::OwnedMirItem>,
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn validate_request(request: &ExtractRequest) -> Result<PathBuf, &'static str> {
    if request.protocol_version != "1.0"
        || request.crate_name.is_empty()
        || request.crate_name.len() > 128
        || !request
            .crate_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || !matches!(request.crate_type.as_str(), "lib" | "bin")
        || !matches!(request.edition.as_str(), "2021" | "2024")
    {
        return Err("invalid extractor request fields");
    }
    if request.cancelled {
        return Err("extraction cancelled before compiler admission");
    }
    let workspace = request
        .workspace_root
        .canonicalize()
        .map_err(|_| "workspace root is unavailable")?;
    let source = request
        .source_path
        .canonicalize()
        .map_err(|_| "source path is unavailable")?;
    if !source.starts_with(&workspace) || !source.is_file() {
        return Err("source path escapes workspace root");
    }
    let byte_length = source
        .metadata()
        .map_err(|_| "source metadata is unavailable")?
        .len();
    if byte_length > 8 * 1024 * 1024 {
        return Err("source exceeds extractor admission limit");
    }
    Ok(source)
}

fn sysroot() -> Result<String, &'static str> {
    let output = ProcessCommand::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .map_err(|_| "failed to execute pinned rustc")?;
    if !output.status.success() {
        return Err("pinned rustc did not report a sysroot");
    }
    String::from_utf8(output.stdout).map_err(|_| "rustc sysroot is not UTF-8")
}

fn extract(request: &ExtractRequest) -> Result<ExtractResponse, &'static str> {
    let source = validate_request(request)?;
    let source_bytes = fs::read(&source).map_err(|_| "failed to read admitted source")?;
    let output = tempfile::tempdir().map_err(|_| "failed to create extractor output root")?;
    let rustc_args = vec![
        "rustc".to_owned(),
        source.to_string_lossy().into_owned(),
        format!("--crate-name={}", request.crate_name),
        format!("--crate-type={}", request.crate_type),
        format!("--edition={}", request.edition),
        "--emit=metadata".to_owned(),
        format!("--out-dir={}", output.path().display()),
        format!("--sysroot={}", sysroot()?.trim()),
    ];
    let items = rustc_link::extract_owned(&rustc_args).map_err(|_| "compiler extraction failed")?;
    Ok(ExtractResponse {
        protocol_version: "1.0",
        toolchain_identity_digest: b3(IDENTITY.as_bytes()),
        source_digest: b3(&source_bytes),
        compiler_exit_status: 0,
        diagnostics: Vec::new(),
        items,
    })
}

fn extract_json(stdin: &mut impl Read, stdout: &mut impl Write) -> Result<(), &'static str> {
    let mut bytes = Vec::new();
    stdin
        .take(256 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "failed to read extraction request")?;
    let request: ExtractRequest =
        serde_json::from_slice(&bytes).map_err(|_| "malformed extraction request")?;
    let response = extract(&request)?;
    serde_json::to_writer(&mut *stdout, &response)
        .map_err(|_| "failed to encode extraction response")?;
    writeln!(stdout).map_err(|_| "failed to terminate extraction response")?;
    Ok(())
}

fn run(
    args: impl IntoIterator<Item = OsString>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<i32, String> {
    assert_eq!(rustc_link::compiler_surface_smoke(), size_of::<usize>());
    match parse_command(args)? {
        Command::Identity => {
            writeln!(stderr, "{}", IDENTITY.trim())
                .map_err(|_| "failed to write identity".to_owned())?;
            Ok(0)
        }
        Command::ExtractJson => {
            extract_json(stdin, stdout).map_err(str::to_owned)?;
            Ok(0)
        }
        Command::Wrapper {
            real_rustc,
            arguments,
        } => wrapper::run(&real_rustc, &arguments, IDENTITY.as_bytes()),
    }
}

fn main() -> ExitCode {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    match run(
        std::env::args_os().skip(1),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    ) {
        Ok(code) => ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1)),
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command as ProcessCommand;

    use super::*;

    #[test]
    fn identity_is_stderr_only_and_exact() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(
            [OsString::from("--identity")],
            &mut &b""[..],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(stdout, Vec::<u8>::new());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            format!("{}\n", IDENTITY.trim())
        );
    }

    #[test]
    fn default_build_runs_a_real_rustc_public_callback() {
        let unique = format!("codefabric-rustc-link-smoke-{}", std::process::id());
        let fixture_root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&fixture_root).unwrap();
        let source = fixture_root.join("fixture.rs");
        fs::write(&source, "pub fn answer() -> u8 { 42 }\n").unwrap();

        let sysroot_output = ProcessCommand::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot_output.status.success());
        let sysroot = String::from_utf8(sysroot_output.stdout).unwrap();
        let rustc_args = vec![
            "rustc".to_owned(),
            source.display().to_string(),
            "--crate-name=codefabric_link_smoke".to_owned(),
            "--crate-type=lib".to_owned(),
            "--edition=2024".to_owned(),
            "--emit=metadata".to_owned(),
            format!("--out-dir={}", fixture_root.display()),
            format!("--sysroot={}", sysroot.trim()),
        ];

        let item_count = rustc_link::count_local_items(&rustc_args).unwrap();
        assert!(item_count > 0);

        fs::remove_dir_all(&fixture_root).unwrap();
    }

    #[test]
    fn malformed_and_cancelled_protocol_requests_are_rejected() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run(
                [OsString::from("--extract-json")],
                &mut &b"{}"[..],
                &mut stdout,
                &mut stderr,
            ),
            Err("malformed extraction request".to_owned())
        );

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = root.join("tests/golden/codefabric-golden-v1/workspace/rust/src/lib.rs");
        let request = serde_json::json!({
            "protocol_version": "1.0",
            "workspace_root": root,
            "source_path": source,
            "crate_name": "golden_fixture",
            "crate_type": "lib",
            "edition": "2024",
            "cancelled": true
        });
        assert_eq!(
            extract_json(&mut request.to_string().as_bytes(), &mut stdout),
            Err("extraction cancelled before compiler admission")
        );
    }

    #[test]
    fn wp35_golden_rust_source_produces_deterministic_owned_mir() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let request = ExtractRequest {
            protocol_version: "1.0".to_owned(),
            workspace_root: root.to_path_buf(),
            source_path: root.join("tests/golden/codefabric-golden-v1/workspace/rust/src/lib.rs"),
            crate_name: "golden_fixture".to_owned(),
            crate_type: "lib".to_owned(),
            edition: "2024".to_owned(),
            cancelled: false,
        };
        let first = serde_json::to_vec(&extract(&request).unwrap()).unwrap();
        let second = serde_json::to_vec(&extract(&request).unwrap()).unwrap();
        assert_eq!(first, second);
        let response: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert!(
            response["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }
}
