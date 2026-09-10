//! Thin authenticated CodeFabric daemon bootstrap.

use codefabric::process_runtime::FabricDaemonProcessSettings;
use tracing_subscriber::prelude::*;

const USAGE: &str = "usage: codefabricd serve --config <path>";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    // The supervisor owns stderr and process lifetime. Keep runtime failures visible
    // without an application queue or an unbounded daemon-owned log history.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::io::stderr)
                .with_filter(
                    tracing_subscriber::filter::Targets::new()
                        .with_default(tracing::Level::ERROR)
                        .with_target("codefabric", tracing::Level::WARN),
                ),
        )
        .try_init()
        .map_err(|error| format!("daemon diagnostic sink: {error}"))?;
    FabricDaemonProcessSettings::parse(std::env::args_os().skip(1))
        .map_err(|error| format!("{USAGE}\n{error}"))?
        .execute()
        .map_err(|error| error.to_string())
}
