//! Generated-output-free administrative model compiler entry point.

use std::process::ExitCode;

pub mod model_control;
pub mod model_git_state;
pub mod repository_model;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--identity") => {
            println!(
                "{{\"generator_id\":\"codefabric-model\",\"generator_revision\":\"model-compiler-v1\"}}"
            );
            ExitCode::SUCCESS
        }
        Some("inventory") => inventory(&arguments.collect::<Vec<_>>()),
        Some("explain") => explain(&arguments.collect::<Vec<_>>()),
        _ => {
            eprintln!(
                "usage: codefabric-model --identity | inventory [--no-gix] [root] | explain <id-or-path> [root]"
            );
            ExitCode::FAILURE
        }
    }
}

fn inventory(arguments: &[String]) -> ExitCode {
    let use_gix = !arguments.iter().any(|argument| argument == "--no-gix");
    let root = arguments
        .iter()
        .find(|argument| argument.as_str() != "--no-gix")
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    match repository_model::RepositoryModel::discover(
        &root,
        repository_model::InventoryBounds::default(),
        use_gix,
    )
    .and_then(|model| {
        let summary = model.summary()?;
        let shadow = repository_model::compare_legacy_catalog(&root, &model).ok();
        serde_json::to_string(&serde_json::json!({"summary": summary, "shadow": shadow}))
            .map_err(repository_model::RepositoryModelError::Json)
    }) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn explain(arguments: &[String]) -> ExitCode {
    let Some(target) = arguments.first() else {
        eprintln!("explain requires an artifact ID or repository path");
        return ExitCode::FAILURE;
    };
    let root = arguments
        .get(1)
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    match repository_model::RepositoryModel::discover(
        &root,
        repository_model::InventoryBounds::default(),
        true,
    ) {
        Ok(model) => {
            let explanations = model.explain(target);
            let shadow = repository_model::compare_legacy_catalog(&root, &model)
                .ok()
                .map(|report| {
                    report
                        .mismatches
                        .into_iter()
                        .filter(|mismatch| mismatch.target == *target)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if explanations.is_empty() && shadow.is_empty() {
                eprintln!("no model node or path matches {target}");
                ExitCode::FAILURE
            } else {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "model": explanations,
                        "shadow": shadow,
                    }))
                    .expect("typed explanations serialize")
                );
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
