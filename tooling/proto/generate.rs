//! Rust code generation from the compiler-independent Wave 0 descriptor IR.

use prost::Message as _;
use std::path::PathBuf;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = argument_value("--rust-out").map_err(std::io::Error::other)?;
    let descriptor = argument_value("--descriptor").map_err(std::io::Error::other)?;
    let roundtrip = argument_value("--roundtrip-descriptor-out").map_err(std::io::Error::other)?;
    std::fs::create_dir_all(&output)?;

    let descriptors =
        prost_types::FileDescriptorSet::decode(std::fs::read(descriptor)?.as_slice())?;
    std::fs::write(roundtrip, descriptors.encode_to_vec())?;
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(output)
        .compile_fds(descriptors)?;
    Ok(())
}
