#![feature(rustc_private)]
#![deny(unsafe_code)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_public;

mod rustc_link;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

const IDENTITY: &str = include_str!("../toolchain-identity.json");

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Identity,
    Serve { endpoint: OsString },
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some(value) if value == "--identity" && args.next().is_none() => Ok(Command::Identity),
        Some(value) if value == "--serve" => {
            let endpoint = args.next().ok_or("--serve requires an endpoint")?;
            if args.next().is_some() {
                return Err("--serve accepts exactly one endpoint");
            }
            Ok(Command::Serve { endpoint })
        }
        _ => Err("expected --identity or --serve <endpoint>"),
    }
}

fn run(
    args: impl IntoIterator<Item = OsString>,
    _stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), &'static str> {
    assert_eq!(rustc_link::compiler_surface_smoke(), size_of::<usize>());
    match parse_command(args)? {
        Command::Identity => {
            writeln!(stderr, "{}", IDENTITY.trim()).map_err(|_| "failed to write identity")?;
        }
        Command::Serve { endpoint } => {
            // Wave 0 proves launch and protocol silence only. Transport connection
            // and request handling arrive in their roadmap-owned packets.
            drop(endpoint);
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
    use std::fs;
    use std::process::Command as ProcessCommand;

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

        run(
            [
                OsString::from("--serve"),
                OsString::from("unix:///tmp/codefabric.sock"),
            ],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(stdout, Vec::<u8>::new());
        assert_eq!(stderr, Vec::<u8>::new());
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
}
