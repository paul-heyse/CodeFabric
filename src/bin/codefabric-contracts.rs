//! Administrative compiler and verifier for CodeFabric machine contracts.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codefabric::contracts::artifacts::{
    VerificationProfile, generate, identity, verify, verify_checksum_fixture,
};

const USAGE: &str = "usage:\n  codefabric-contracts --identity\n  codefabric-contracts generate [--root PATH]\n  codefabric-contracts verify [--profile full|released] [--root PATH]\n  codefabric-contracts verify-checksum-fixture PATH\n";

#[derive(Debug)]
enum Command {
    Identity,
    Generate {
        root: PathBuf,
    },
    Verify {
        root: PathBuf,
        profile: VerificationProfile,
    },
    VerifyChecksumFixture {
        path: PathBuf,
    },
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn take_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_root_and_profile(
    arguments: impl IntoIterator<Item = String>,
    allow_profile: bool,
) -> Result<(PathBuf, VerificationProfile), String> {
    let mut arguments = arguments.into_iter();
    let mut root = repository_root();
    let mut profile = VerificationProfile::Full;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(take_value(&mut arguments, "--root")?),
            "--profile" if allow_profile => {
                let value = take_value(&mut arguments, "--profile")?;
                profile = VerificationProfile::parse(&value).map_err(|error| error.to_string())?;
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok((root, profile))
}

fn parse() -> Result<Command, String> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(USAGE.to_owned());
    };
    match command.as_str() {
        "--identity" => {
            if arguments.next().is_some() {
                return Err("--identity accepts no arguments".to_owned());
            }
            Ok(Command::Identity)
        }
        "generate" => {
            let (root, _) = parse_root_and_profile(arguments, false)?;
            Ok(Command::Generate { root })
        }
        "verify" => {
            let (root, profile) = parse_root_and_profile(arguments, true)?;
            Ok(Command::Verify { root, profile })
        }
        "verify-checksum-fixture" => {
            let path = PathBuf::from(take_value(&mut arguments, command.as_str())?);
            if arguments.next().is_some() {
                return Err("verify-checksum-fixture accepts exactly one path".to_owned());
            }
            Ok(Command::VerifyChecksumFixture { path })
        }
        "--help" | "-h" => Err(USAGE.to_owned()),
        _ => Err(format!("unknown command: {command}\n{USAGE}")),
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Identity => {
            let encoded = serde_json::to_string(&identity()).map_err(|error| error.to_string())?;
            println!("{encoded}");
        }
        Command::Generate { root } => {
            let count = generate(Path::new(&root)).map_err(|error| error.to_string())?;
            eprintln!("generated {count} contract outputs");
        }
        Command::Verify { root, profile } => {
            let report = verify(Path::new(&root), profile).map_err(|error| error.to_string())?;
            eprintln!(
                "verified {} source artifacts with {} warnings",
                report.artifact_count, report.warning_count
            );
        }
        Command::VerifyChecksumFixture { path } => {
            verify_checksum_fixture(Path::new(&path)).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
