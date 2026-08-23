//! Generated-output-free administrative model compiler entry point.

use std::process::ExitCode;

pub mod desired_tree;
pub mod driver_protocol;
pub mod model_control;
pub mod model_git_state;
pub mod registry_cbef_driver;
pub mod release_census;
pub mod repository_model;
pub mod schema_driver;

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
        Some("plan") => plan(&arguments.collect::<Vec<_>>()),
        Some("check") => check(&arguments.collect::<Vec<_>>()),
        Some("family-check") => family_check(&arguments.collect::<Vec<_>>()),
        Some("release-census-candidate") => {
            release_census_candidate(&arguments.collect::<Vec<_>>())
        }
        Some("release-census-check") => release_census_check(&arguments.collect::<Vec<_>>()),
        Some("accept") => accept(&arguments.collect::<Vec<_>>()),
        _ => {
            eprintln!(
                "usage: codefabric-model --identity | inventory [--no-gix] [root] | explain <id-or-path> [root] | plan [changed-id-or-path ...] [--root root] | check [--root root] | family-check <registry-cbef|schemas> [root] | release-census-candidate [root] | release-census-check [root] | accept release-census --owner <id> --provenance <text> --reviewed [root]"
            );
            ExitCode::FAILURE
        }
    }
}

fn family_check(arguments: &[String]) -> ExitCode {
    let Some(family) = arguments.first() else {
        eprintln!("family-check requires a closed family name");
        return ExitCode::FAILURE;
    };
    let root = arguments
        .get(1)
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    let result = match family.as_str() {
        "registry-cbef" => registry_cbef_driver::check_family(&root)
            .and_then(|report| {
                serde_json::to_string(&report)
                    .map_err(registry_cbef_driver::RegistryCbefError::Json)
            })
            .map_err(|error| error.to_string()),
        "schemas" => schema_driver::check_family(&root)
            .and_then(|report| {
                serde_json::to_string(&report).map_err(schema_driver::SchemaDriverError::Json)
            })
            .map_err(|error| error.to_string()),
        _ => Err(format!("unknown model family {family}")),
    };
    print_result(result)
}

fn compile_repository(root: &std::path::Path) -> Result<repository_model::RepositoryModel, String> {
    repository_model::RepositoryModel::discover(
        root,
        repository_model::InventoryBounds::default(),
        true,
    )
    .map_err(|error| error.to_string())
}

fn compile_plan(
    root: &std::path::Path,
) -> Result<(repository_model::RepositoryModel, desired_tree::ModelPlan), String> {
    let model = compile_repository(root)?;
    let executable =
        desired_tree::ActionExecutableIdentity::current().map_err(|error| error.to_string())?;
    let plan = desired_tree::ModelPlan::compile(root, &model, &executable)
        .map_err(|error| error.to_string())?;
    Ok((model, plan))
}

fn root_and_values(arguments: &[String]) -> Result<(std::path::PathBuf, Vec<String>), String> {
    let mut root = std::path::PathBuf::from(".");
    let mut values = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--root" {
            let Some(value) = arguments.get(index + 1) else {
                return Err("--root requires a path".to_owned());
            };
            root = std::path::PathBuf::from(value);
            index += 2;
        } else {
            values.push(arguments[index].clone());
            index += 1;
        }
    }
    Ok((root, values))
}

fn plan(arguments: &[String]) -> ExitCode {
    let result = root_and_values(arguments).and_then(|(root, changed)| {
        let (model, plan) = compile_plan(&root)?;
        let changed_ids = changed
            .iter()
            .map(|target| resolve_model_id(&model, target))
            .collect::<Result<Vec<_>, _>>()?;
        serde_json::to_string(&plan.report(&changed_ids)).map_err(|error| error.to_string())
    });
    print_result(result)
}

fn check(arguments: &[String]) -> ExitCode {
    let result = root_and_values(arguments).and_then(|(root, values)| {
        let profile = match values.as_slice() {
            [] => "edit",
            [profile] if matches!(profile.as_str(), "shadow" | "edit" | "full" | "release") => {
                profile
            }
            _ => return Err("check accepts one of shadow, edit, full, or release".to_owned()),
        };
        let (_, plan) = compile_plan(&root)?;
        plan.check(&root).map_err(|error| error.to_string())?;
        serde_json::to_string(&serde_json::json!({
            "profile": profile,
            "plan": plan.report(&[]),
        }))
        .map_err(|error| error.to_string())
    });
    print_result(result)
}

fn resolve_model_id(
    model: &repository_model::RepositoryModel,
    target: &str,
) -> Result<model_control::StableId, String> {
    let claim = model
        .claims
        .values()
        .find(|claim| claim.path.display() == target)
        .or_else(|| {
            model.claims.values().find(|claim| {
                claim.role != repository_model::ArtifactRole::Derived
                    && claim
                        .header
                        .as_ref()
                        .is_some_and(|header| header.artifact_id.as_str() == target)
            })
        })
        .or_else(|| {
            model.claims.values().find(|claim| {
                claim
                    .header
                    .as_ref()
                    .is_some_and(|header| header.artifact_id.as_str() == target)
            })
        });
    if let Some(claim) = claim {
        return claim.header.as_ref().map_or_else(
            || {
                repository_model::output_id(claim.path.raw_bytes())
                    .map_err(|error| error.to_string())
            },
            |header| Ok(header.artifact_id.clone()),
        );
    }
    model_control::StableId::parse(target.to_owned()).map_err(|error| error.to_string())
}

fn print_result(result: Result<String, String>) -> ExitCode {
    match result {
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
    match compile_plan(&root) {
        Ok((model, plan)) => {
            let explanations = model.explain(target);
            let plan_explanations = plan.explain(target);
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
            if explanations.is_empty() && plan_explanations.is_empty() && shadow.is_empty() {
                eprintln!("no model node or path matches {target}");
                ExitCode::FAILURE
            } else {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "model": explanations,
                        "plan": plan_explanations,
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
