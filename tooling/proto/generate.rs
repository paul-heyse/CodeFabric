//! Hermetic Rust generator for the Wave 0 Protobuf compatibility contract.

use std::path::{Path, PathBuf};
use std::process::Command;

fn argument_value(name: &str) -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} requires a path"));
        }
    }
    Err(format!("missing required {name} argument"))
}

fn protoc_version(protoc: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(protoc).arg("--version").output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("vendored protoc --version failed").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    if std::env::args_os().any(|argument| argument == "--protoc-version") {
        eprintln!("{}", protoc_version(&protoc)?);
        return Ok(());
    }

    let output = argument_value("--rust-out").map_err(std::io::Error::other)?;
    std::fs::create_dir_all(&output)?;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = root.join("tooling/proto/source");
    let source = source_root.join("codefabric_cpg_mcp/daemon/generated/wave0_probe.proto");
    let vendored_include = protoc_bin_vendored::include_path()?;

    let mut prost = prost_build::Config::new();
    prost.protoc_executable(protoc);
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(output)
        .compile_with_config(prost, &[source], &[source_root, vendored_include])?;
    Ok(())
}
