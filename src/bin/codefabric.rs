//! CodeFabric local administrative shell.

use std::path::PathBuf;

use codefabric::daemon::{AdminCommand, administer};

fn default_discovery_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CODEFABRIC_DAEMON_DISCOVERY") {
        return PathBuf::from(path);
    }
    if cfg!(target_os = "linux")
        && let Some(root) = std::env::var_os("XDG_RUNTIME_DIR")
    {
        return PathBuf::from(root).join("codefabric/daemon.json");
    }
    std::env::temp_dir().join("codefabric/daemon.json")
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [scope, command, rest @ ..] = arguments.as_slice() else {
        return Err("usage: codefabric daemon <status|stop|drain> [--discovery <path>]".into());
    };
    if scope != "daemon" {
        return Err("only the daemon administrative scope is available in Wave 2".into());
    }
    let command = match command.as_str() {
        "status" => AdminCommand::Status,
        "stop" => AdminCommand::Stop,
        "drain" => AdminCommand::Drain,
        _ => return Err("unknown daemon command".into()),
    };
    let discovery = match rest {
        [] => default_discovery_path(),
        [flag, path] if flag == "--discovery" && !path.is_empty() => PathBuf::from(path),
        _ => return Err("expected optional --discovery <path>".into()),
    };
    let response = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Tokio runtime construction failed: {error}"))?
        .block_on(administer(&discovery, command))
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("response serialization failed: {error}"))?
    );
    Ok(())
}
