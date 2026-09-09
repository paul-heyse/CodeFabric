//! Query-facing work partitions, derived from requested inputs and admitted provider outcomes.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{ProductionWorkspaceStartupError, input_observations, rustc::RustTargetProgress};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{hash32_array, id16_array};
use crate::provider_contracts::{
    AdmittedProviderResult, ProviderCoverageState, ProviderInputDisposition, ProviderLane,
    ProviderSourceInventory, ProviderSourceSelection,
};
use crate::provider_native_syntax::NativeSyntaxRelation;
use crate::rustc_relation_schema::RustcRelation;
use arrow_array::{ArrayRef, BinaryArray, StringArray, UInt64Array};

struct Partition<'a> {
    language: &'static str,
    scope_kind: &'static str,
    path: &'a [u8],
    target: Option<&'a str>,
    target_kind: Option<&'a str>,
    context: Option<[u8; 16]>,
    file: Option<[u8; 16]>,
    state: &'static str,
    reason: &'static str,
}

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventory: &ProviderSourceInventory,
    runs: &[AdmittedProviderResult],
    targets: &[RustTargetProgress],
) -> Result<(), ProductionWorkspaceStartupError> {
    let python = runs
        .iter()
        .filter_map(|run| {
            if run.job().lane() != ProviderLane::Ruff {
                return None;
            }
            let ProviderSourceSelection::File { file_id, .. } = run.job().source().selection()
            else {
                return None;
            };
            Some((*file_id, run))
        })
        .collect::<BTreeMap<_, _>>();
    let rust = runs
        .iter()
        .filter(|run| run.job().lane() == ProviderLane::Rustc)
        .map(|run| (run.job().context().analysis_context_id(), run))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    let mut rust_requested = false;
    for member in inventory.members() {
        let path = member.relative_path.as_slice();
        rust_requested |= path.ends_with(b".rs");
        if !path.ends_with(b".py") && !path.ends_with(b".pyi") {
            continue;
        }
        let file = match member.disposition {
            ProviderInputDisposition::Captured { file_id, .. } => Some(file_id),
            _ => None,
        };
        let run = file.and_then(|id| python.get(&id).copied());
        let (state, reason) = if file.is_none() {
            (
                if matches!(
                    member.disposition,
                    ProviderInputDisposition::ExcludedPolicy
                        | ProviderInputDisposition::ExcludedSpecialFile
                        | ProviderInputDisposition::Generated
                        | ProviderInputDisposition::Vendored
                ) {
                    "excluded"
                } else {
                    "unavailable"
                },
                input_observations::disposition(&member.disposition),
            )
        } else {
            family_state(run, NativeSyntaxRelation::RuffBinding.as_str())
        };
        rows.push(Partition {
            language: "python",
            scope_kind: "source_file",
            path,
            target: None,
            target_kind: None,
            context: run.map(|run| run.job().context().analysis_context_id()),
            file,
            state,
            reason,
        });
    }
    for target in targets {
        let run = target
            .context_id
            .and_then(|context| rust.get(&context).copied());
        let (state, reason) = if target.state == "processed" {
            family_state(run, RustcRelation::PublicItem.relation_id())
        } else {
            ("unavailable", "compiler_target_unavailable")
        };
        rows.push(Partition {
            language: "rust",
            scope_kind: "cargo_target",
            path: &target.manifest,
            target: Some(&target.target),
            target_kind: Some(&target.target_kind),
            context: target.context_id,
            file: None,
            state,
            reason,
        });
    }
    if rust_requested && targets.is_empty() {
        rows.push(Partition {
            language: "rust",
            scope_kind: "workspace_context",
            path: b".",
            target: None,
            target_kind: None,
            context: None,
            file: None,
            state: "unavailable",
            reason: "cargo_context_preparation_incomplete",
        });
    }
    register(builder, inventory, &rows)
}

fn family_state(
    run: Option<&AdmittedProviderResult>,
    family: &str,
) -> (&'static str, &'static str) {
    let Some(run) = run else {
        return ("unavailable", "provider_not_run");
    };
    let Some(request) = run
        .job()
        .requests()
        .iter()
        .find(|request| request.relation().as_str() == family)
    else {
        return ("unknown", "family_not_requested");
    };
    let Some(coverage) = run
        .result()
        .coverage()
        .iter()
        .find(|row| row.family() == request.family())
    else {
        return ("unknown", "family_output_missing");
    };
    match coverage.state() {
        ProviderCoverageState::Complete { .. } => ("complete", ""),
        ProviderCoverageState::IntentionalRemainder { reason, .. } => match reason {
            crate::provider_contracts::ProviderRemainderReason::BudgetExhausted => {
                ("limited", "provider_budget_exhausted")
            }
            crate::provider_contracts::ProviderRemainderReason::DeadlineReached => {
                ("limited", "provider_deadline_reached")
            }
            crate::provider_contracts::ProviderRemainderReason::ProviderDeclaredScope => {
                ("partial", "provider_scope_incomplete")
            }
        },
        ProviderCoverageState::Unknown { cause, .. } => match cause {
            crate::provider_contracts::ProviderUnknownCause::MissingOutput => {
                ("unknown", "provider_output_missing")
            }
            crate::provider_contracts::ProviderUnknownCause::Unsupported => {
                ("unsupported", "provider_unsupported")
            }
            crate::provider_contracts::ProviderUnknownCause::Timeout => {
                ("failed", "provider_timeout")
            }
            crate::provider_contracts::ProviderUnknownCause::Cancelled => {
                ("cancelled", "provider_cancelled")
            }
            crate::provider_contracts::ProviderUnknownCause::Corruption => {
                ("failed", "provider_output_corrupt")
            }
            crate::provider_contracts::ProviderUnknownCause::Oversized => {
                ("limited", "provider_output_too_large")
            }
            crate::provider_contracts::ProviderUnknownCause::ProviderFailure => {
                ("failed", "provider_failed")
            }
            crate::provider_contracts::ProviderUnknownCause::TrustLoss => {
                ("unavailable", "provider_trust_unavailable")
            }
        },
    }
}

fn register(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventory: &ProviderSourceInventory,
    rows: &[Partition<'_>],
) -> Result<(), ProductionWorkspaceStartupError> {
    let strings = |get: fn(&Partition<'_>) -> &'static str| -> ArrayRef {
        Arc::new(StringArray::from_iter_values(rows.iter().map(get)))
    };
    let workspace = inventory.workspace_id();
    let input_set = inventory.identity();
    input_observations::register(
        builder,
        FabricSchemaRole::System,
        "entity_processing_scope",
        vec![
            (
                "workspace_id",
                false,
                id16_array(rows.iter().map(|_| Some(&workspace))),
            ),
            (
                "source_generation",
                false,
                Arc::new(UInt64Array::from(vec![
                    inventory.source_generation();
                    rows.len()
                ])),
            ),
            (
                "input_set_id",
                false,
                hash32_array(rows.iter().map(|_| Some(&input_set))),
            ),
            ("language", false, strings(|row| row.language)),
            ("scope_kind", false, strings(|row| row.scope_kind)),
            (
                "relative_path",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    rows.iter().map(|row| row.path),
                )),
            ),
            (
                "target_name",
                true,
                Arc::new(StringArray::from_iter(rows.iter().map(|row| row.target))),
            ),
            (
                "context_id",
                true,
                id16_array(rows.iter().map(|row| row.context.as_ref())),
            ),
            (
                "target_kind",
                true,
                Arc::new(StringArray::from_iter(
                    rows.iter().map(|row| row.target_kind),
                )),
            ),
            (
                "file_id",
                true,
                id16_array(rows.iter().map(|row| row.file.as_ref())),
            ),
            (
                "family",
                false,
                Arc::new(StringArray::from(vec!["function-declarations"; rows.len()])),
            ),
            ("processing_state", false, strings(|row| row.state)),
            ("reason", false, strings(|row| row.reason)),
        ],
    )
}
