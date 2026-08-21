//! CodeFabric local daemon process.

use std::path::PathBuf;

use codefabric::daemon::{DaemonConfig, serve};

fn config_argument(arguments: &[String]) -> Result<PathBuf, String> {
    let [flag, value] = arguments else {
        return Err("expected --config <path>".into());
    };
    if flag != "--config" || value.is_empty() {
        return Err("expected --config <path>".into());
    }
    Ok(PathBuf::from(value))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some((command, rest)) = arguments.split_first() else {
        return Err("usage: codefabricd <serve|check-config> --config <path>".into());
    };
    let config = DaemonConfig::load(&config_argument(rest)?).map_err(|error| error.to_string())?;
    match command.as_str() {
        "check-config" => Ok(()),
        "serve" => tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| format!("Tokio runtime construction failed: {error}"))?
            .block_on(serve(config))
            .map(|_| ())
            .map_err(|error| error.to_string()),
        _ => Err("usage: codefabricd <serve|check-config> --config <path>".into()),
    }
}
