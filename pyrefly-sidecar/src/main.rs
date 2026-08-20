#![deny(unsafe_code)]

mod pyrefly_link;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

const IDENTITY: &str = include_str!("../toolchain-identity.json");
const PYREFLY_LOCK_SOURCE_BLAKE3: &str = env!("PYREFLY_LOCK_SOURCE_BLAKE3");

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Identity,
    Serve,
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some(value) if value == "--identity" && args.next().is_none() => Ok(Command::Identity),
        Some(value) if value == "--serve" && args.next().is_none() => Ok(Command::Serve),
        _ => Err("expected --identity or --serve"),
    }
}

fn run(
    args: impl IntoIterator<Item = OsString>,
    _stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), &'static str> {
    assert!(IDENTITY.contains(PYREFLY_LOCK_SOURCE_BLAKE3));
    assert!(pyrefly_link::query_surface_smoke() > 0);
    match parse_command(args)? {
        Command::Identity => {
            writeln!(stderr, "{}", IDENTITY.trim()).map_err(|_| "failed to write identity")?;
        }
        Command::Serve => {
            // Wave 0 proves the isolated process and unstable-API link only.
            // The application-owned protocol arrives in its roadmap packet.
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
    fn serve_stub_is_protocol_silent() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run([OsString::from("--serve")], &mut stdout, &mut stderr).unwrap();

        assert_eq!(stdout, Vec::<u8>::new());
        assert_eq!(stderr, Vec::<u8>::new());
    }
}
