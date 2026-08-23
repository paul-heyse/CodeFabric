//! Generated-output-free administrative model compiler entry point.

use std::process::ExitCode;

pub mod model_control;
pub mod model_git_state;
pub mod release_census;
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
        Some("release-census-candidate") => {
            release_census_candidate(&arguments.collect::<Vec<_>>())
        }
        Some("release-census-check") => release_census_check(&arguments.collect::<Vec<_>>()),
        Some("accept") => accept(&arguments.collect::<Vec<_>>()),
        _ => {
            eprintln!(
                "usage: codefabric-model --identity | inventory [--no-gix] [root] | explain <id-or-path> [root] | release-census-candidate [root] | release-census-check [root] | accept release-census --owner <id> --provenance <text> --reviewed [root]"
            );
            ExitCode::FAILURE
        }
    }
}

fn compile_repository(root: &std::path::Path) -> Result<repository_model::RepositoryModel, String> {
    repository_model::RepositoryModel::discover(
        root,
        repository_model::InventoryBounds::default(),
        true,
    )
    .map_err(|error| error.to_string())
}

fn release_census_candidate(arguments: &[String]) -> ExitCode {
    let root = arguments
        .first()
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    match compile_repository(&root)
        .and_then(|model| release_census::write_candidate(&root, &model).map_err(|e| e.to_string()))
        .and_then(|report| serde_json::to_string(&report).map_err(|e| e.to_string()))
    {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn release_census_check(arguments: &[String]) -> ExitCode {
    let root = arguments
        .first()
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    match compile_repository(&root)
        .and_then(|model| release_census::check(&root, &model).map_err(|error| error.to_string()))
    {
        Ok(()) => {
            println!("release census check passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn accept(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) != Some("release-census") {
        eprintln!("accept requires the closed kind release-census");
        return ExitCode::FAILURE;
    }
    let option = |name: &str| {
        arguments
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    };
    let Some(owner_identity) = option("--owner") else {
        eprintln!("accept release-census requires --owner");
        return ExitCode::FAILURE;
    };
    let Some(acceptance_provenance) = option("--provenance") else {
        eprintln!("accept release-census requires --provenance");
        return ExitCode::FAILURE;
    };
    let root = arguments
        .iter()
        .rev()
        .find(|argument| {
            argument.as_str() != "release-census"
                && argument.as_str() != "--reviewed"
                && !argument.starts_with("--")
                && *argument != &owner_identity
                && *argument != &acceptance_provenance
        })
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    let authorization = release_census::AcceptanceAuthorization {
        owner_identity,
        acceptance_provenance,
        reviewed_candidate: arguments.iter().any(|argument| argument == "--reviewed"),
    };
    match release_census::accept_candidate(&root, &authorization) {
        Ok(census) => {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "accepted_path": release_census::accepted_relative_path(),
                    "artifact_id": census.artifact_id,
                    "candidate_digest": census.owner_acceptance.candidate_digest,
                }))
                .expect("acceptance report serializes")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
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
