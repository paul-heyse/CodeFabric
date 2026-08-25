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
use std::io::{self, Write};
use std::process::ExitCode;

const IDENTITY: &str = include_str!("../toolchain-identity.json");

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Identity,
    Wrapper {
        real_rustc: OsString,
        arguments: Vec<OsString>,
    },
}

fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some(value) if value == "--identity" && args.next().is_none() => Ok(Command::Identity),
        Some(real_rustc) => Ok(Command::Wrapper {
            real_rustc: real_rustc.to_os_string(),
            arguments: args.collect(),
        }),
        None => Err("expected --identity or a real rustc invocation"),
    }
}

fn run(args: impl IntoIterator<Item = OsString>, stderr: &mut impl Write) -> Result<i32, String> {
    assert_eq!(rustc_link::compiler_surface_smoke(), size_of::<usize>());
    match parse_command(args)? {
        Command::Identity => {
            writeln!(stderr, "{}", IDENTITY.trim())
                .map_err(|_| "failed to write identity".to_owned())?;
            Ok(0)
        }
        Command::Wrapper {
            real_rustc,
            arguments,
        } => wrapper::run(&real_rustc, &arguments, IDENTITY.as_bytes()),
    }
}

fn main() -> ExitCode {
    let mut stderr = io::stderr().lock();
    match run(std::env::args_os().skip(1), &mut stderr) {
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
    use std::process::Command as ProcessCommand;

    use super::*;

    #[test]
    fn identity_is_stderr_only_and_exact() {
        let mut stderr = Vec::new();

        run([OsString::from("--identity")], &mut stderr).unwrap();

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
}
