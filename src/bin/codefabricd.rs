//! Thin authenticated CodeFabric daemon bootstrap.

use codefabric::process_runtime::FabricDaemonProcessSettings;

const USAGE: &str = "usage: codefabricd serve --config <path>";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    FabricDaemonProcessSettings::parse(std::env::args_os().skip(1))
        .map_err(|error| format!("{USAGE}\n{error}"))?
        .execute()
        .map_err(|error| error.to_string())
}
