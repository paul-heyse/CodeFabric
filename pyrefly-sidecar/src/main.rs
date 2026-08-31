#![deny(unsafe_code)]

mod protocol;
mod pyrefly_link;
#[path = "../../src/relation_ipc_contract.rs"]
mod relation_ipc_contract;
pub(crate) use protocol::generated::codefabric::provider::v1 as relation_ipc_proto_types;
#[path = "../../src/relation_ipc_proto.rs"]
mod relation_ipc_proto;
mod server;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

const IDENTITY: &str = include_str!("../toolchain-identity.json");
const PYREFLY_LOCK_SOURCE_BLAKE3: &str = env!("PYREFLY_LOCK_SOURCE_BLAKE3");

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Identity,
    Serve(Option<OsString>),
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some(value) if value == "--identity" && args.next().is_none() => Ok(Command::Identity),
        Some(value) if value == "--serve" => {
            let endpoint = args.next();
            if args.next().is_none() {
                Ok(Command::Serve(endpoint))
            } else {
                Err("expected at most one private UDS endpoint")
            }
        }
        _ => Err("expected --identity or --serve"),
    }
}

fn run(
    args: impl IntoIterator<Item = OsString>,
    _stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), String> {
    assert!(IDENTITY.contains(PYREFLY_LOCK_SOURCE_BLAKE3));
    assert!(pyrefly_link::query_surface_smoke() > 0);
    match parse_command(args).map_err(str::to_owned)? {
        Command::Identity => {
            writeln!(stderr, "{}", IDENTITY.trim())
                .map_err(|_| "failed to write identity".to_owned())?;
        }
        Command::Serve(endpoint) => {
            let endpoint = endpoint
                .map_or_else(
                    || std::env::var("CODEFABRIC_PYREFLY_ENDPOINT"),
                    |value| Ok(value.to_string_lossy().into_owned()),
                )
                .map_err(|_| "CODEFABRIC_PYREFLY_ENDPOINT is required".to_owned())?;
            let socket = endpoint
                .strip_prefix("unix://")
                .ok_or_else(|| "Pyrefly endpoint must use unix://".to_owned())?;
            if socket.is_empty() {
                return Err("Pyrefly endpoint path is empty".to_owned());
            }
            server::serve(std::path::Path::new(socket))?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    match run(std::env::args_os().skip(1), &mut stdout, &mut stderr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stderr_only_and_exact() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run([OsString::from("--identity")], &mut stdout, &mut stderr).unwrap();

        assert_eq!(stdout, Vec::<u8>::new());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            format!("{}\n", IDENTITY.trim())
        );
    }

    #[test]
    fn serve_requires_the_private_uds_endpoint() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = run([OsString::from("--serve")], &mut stdout, &mut stderr);

        assert!(result.is_err());
        assert_eq!(stdout, Vec::<u8>::new());
        assert_eq!(stderr, Vec::<u8>::new());
    }
}
