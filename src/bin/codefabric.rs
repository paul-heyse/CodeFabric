//! Thin CodeFabric supervisor and attach-only MCP process entrypoint.

use codefabric::process_runtime::{CodefabricProcessSettings, ProcessCommandOutput};

const USAGE: &str = "usage: codefabric supervisor <serve|check-config> --config <path>\n       codefabric supervisor <status|drain|stop> --discovery <path>\n       codefabric mcp serve --supervisor <discovery> --policy-id <opaque-id>";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let settings = CodefabricProcessSettings::parse(std::env::args_os().skip(1))
        .map_err(|error| format!("{USAGE}\n{error}"))?;
    match settings.execute().map_err(|error| error.to_string())? {
        ProcessCommandOutput::Completed => Ok(()),
        ProcessCommandOutput::SupervisorStatus(status) => {
            println!(
                "{}",
                serde_json::to_string(&status).map_err(|error| {
                    format!("supervisor response serialization failed: {error}")
                })?
            );
            status
                .accepted
                .then_some(())
                .ok_or_else(|| status.code.clone())
        }
    }
}
