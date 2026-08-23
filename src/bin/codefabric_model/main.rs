//! Generated-output-free administrative model compiler entry point.

use std::process::ExitCode;

pub mod model_control;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--identity") {
        println!(
            "{{\"generator_id\":\"codefabric-model\",\"generator_revision\":\"model-compiler-v1\"}}"
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("usage: codefabric-model --identity");
        ExitCode::FAILURE
    }
}
