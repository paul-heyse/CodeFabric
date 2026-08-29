//! CodeFabric local administrative shell.

use std::path::PathBuf;

use codefabric::daemon::{
    AdminCommand, AdminResponse, WorkspaceAdminCommand, administer, administer_workspace,
};
use codefabric::identity::{IdentityDomain, decode_public_id};
use codefabric::ontology_activation::OntologyCandidateSubmission;
use codefabric::secure_path::read_control_artifact;
use codefabric::workspace_registry::{RelinkProof, RemovalPolicy};

const MAXIMUM_ONTOLOGY_SUBMISSION_BYTES: u64 = 16 * 1024 * 1024;

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
    let (arguments, discovery) = extract_discovery(std::env::args().skip(1).collect())?;
    let Some((scope, rest)) = arguments.split_first() else {
        return Err("usage: codefabric <daemon|workspace> <command> ...".into());
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Tokio runtime construction failed: {error}"))?;
    let response = match scope.as_str() {
        "daemon" => runtime.block_on(administer(&discovery, daemon_command(rest)?)),
        "workspace" => runtime.block_on(administer_workspace(&discovery, workspace_command(rest)?)),
        _ => return Err("scope must be daemon or workspace".into()),
    }
    .map_err(|error| error.to_string())?;
    print_response(&response)?;
    if !response.accepted {
        return Err(response
            .error_code
            .clone()
            .unwrap_or_else(|| "administrative request rejected".to_owned()));
    }
    Ok(())
}

fn print_response(response: &AdminResponse) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("response serialization failed: {error}"))?
    );
    Ok(())
}

fn extract_discovery(mut arguments: Vec<String>) -> Result<(Vec<String>, PathBuf), String> {
    if arguments.len() >= 2 && arguments[arguments.len() - 2] == "--discovery" {
        let path = arguments
            .pop()
            .filter(|value| !value.is_empty())
            .ok_or("--discovery requires a path")?;
        let _flag = arguments.pop();
        Ok((arguments, PathBuf::from(path)))
    } else {
        Ok((arguments, default_discovery_path()))
    }
}

fn daemon_command(arguments: &[String]) -> Result<AdminCommand, String> {
    let [command] = arguments else {
        return Err("usage: codefabric daemon <status|stop|drain>".into());
    };
    match command.as_str() {
        "status" => Ok(AdminCommand::Status),
        "stop" => Ok(AdminCommand::Stop),
        "drain" => Ok(AdminCommand::Drain),
        _ => Err("unknown daemon command".into()),
    }
}

#[allow(clippy::too_many_lines)] // The closed CLI grammar is kept in one auditable match.
fn workspace_command(arguments: &[String]) -> Result<WorkspaceAdminCommand, String> {
    let Some((command, rest)) = arguments.split_first() else {
        return Err("workspace command is required".into());
    };
    match (command.as_str(), rest) {
        ("add", [root]) => Ok(WorkspaceAdminCommand::Add {
            root: PathBuf::from(root),
        }),
        ("list", []) => Ok(WorkspaceAdminCommand::List),
        ("show", [workspace_id]) => Ok(WorkspaceAdminCommand::Show {
            workspace_id: workspace_id_value(workspace_id)?,
        }),
        ("relink", [workspace_id, new_root, acknowledge, inventory])
            if acknowledge == "--acknowledge-non-git" && inventory == "--inventory-matches" =>
        {
            Ok(WorkspaceAdminCommand::Relink {
                workspace_id: workspace_id_value(workspace_id)?,
                new_root: PathBuf::from(new_root),
                proof: RelinkProof::NonGit {
                    operator_acknowledged: true,
                    inventory_matches: true,
                },
            })
        }
        ("configure", [workspace_id, flag, profile]) if flag == "--profile" => {
            Ok(WorkspaceAdminCommand::Configure {
                workspace_id: workspace_id_value(workspace_id)?,
                profile_manifest: PathBuf::from(profile),
            })
        }
        ("enable", [workspace_id]) => Ok(WorkspaceAdminCommand::Enable {
            workspace_id: workspace_id_value(workspace_id)?,
        }),
        ("disable", [workspace_id]) => Ok(WorkspaceAdminCommand::Disable {
            workspace_id: workspace_id_value(workspace_id)?,
        }),
        ("reconcile", [workspace_id]) => Ok(WorkspaceAdminCommand::Reconcile {
            workspace_id: workspace_id_value(workspace_id)?,
        }),
        (
            "activate-candidate",
            [
                workspace_id,
                submission_path,
                administrative_key_hex,
                request_key,
            ],
        ) if !submission_path.is_empty()
            && !administrative_key_hex.is_empty()
            && !request_key.is_empty() =>
        {
            let submission_bytes = read_control_artifact(
                std::path::Path::new(submission_path),
                MAXIMUM_ONTOLOGY_SUBMISSION_BYTES,
            )
            .map_err(|error| format!("cannot read candidate submission: {error}"))?;
            let submission: OntologyCandidateSubmission = serde_json::from_slice(&submission_bytes)
                .map_err(|error| format!("invalid candidate submission JSON: {error}"))?;
            Ok(WorkspaceAdminCommand::ActivateCandidate {
                workspace_id: workspace_id_value(workspace_id)?,
                submission: Box::new(submission),
                administrative_key: decode_hex_bytes(administrative_key_hex)?,
                request_key: request_key.clone(),
            })
        }
        ("remove", [workspace_id, flag]) if flag == "--retain-data" => {
            Ok(WorkspaceAdminCommand::Remove {
                workspace_id: workspace_id_value(workspace_id)?,
                policy: RemovalPolicy::RetainData,
                purge_confirmations: 0,
            })
        }
        ("remove", [workspace_id, purge, first_confirmation, second_confirmation])
            if purge == "--purge-data"
                && first_confirmation == "--confirm-purge"
                && second_confirmation == "--confirm-no-recovery" =>
        {
            Ok(WorkspaceAdminCommand::Remove {
                workspace_id: workspace_id_value(workspace_id)?,
                policy: RemovalPolicy::PurgeData,
                purge_confirmations: 2,
            })
        }
        _ => Err("invalid workspace command or confirmation flags".into()),
    }
}

fn workspace_id_value(value: &str) -> Result<[u8; 16], String> {
    let public = if value.starts_with("workspace:") {
        value.to_owned()
    } else {
        format!("workspace:{value}")
    };
    decode_public_id(IdentityDomain::Workspace, None, &public)
        .map_err(|error| format!("invalid workspace ID: {error}"))
}

fn decode_hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err("administrative key must be non-empty even-length hexadecimal".into());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = (pair[0] as char)
                .to_digit(16)
                .ok_or("administrative key contains non-hexadecimal bytes")?;
            let low = (pair[1] as char)
                .to_digit(16)
                .ok_or("administrative key contains non-hexadecimal bytes")?;
            Ok(u8::try_from((high << 4) | low).expect("hexadecimal byte fits u8"))
        })
        .collect()
}
