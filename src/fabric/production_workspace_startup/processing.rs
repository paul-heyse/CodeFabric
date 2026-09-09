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

#[derive(Clone, Copy)]
struct Partition<'a> {
    family: &'static str,
    language: &'static str,
    scope_kind: &'static str,
    path: &'a [u8],
    target: Option<&'a str>,
    target_kind: Option<&'a str>,
    target_platform: Option<&'a str>,
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
    publication: super::PublicationStage,
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
    let pyrefly = pyrefly_by_file(runs);
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
        let partition = Partition {
            family: "function-declarations",
            language: "python",
            scope_kind: "source_file",
            path,
            target: None,
            target_kind: None,
            target_platform: None,
            context: run.map(|run| run.job().context().analysis_context_id()),
            file,
            state,
            reason,
        };
        rows.push(partition);
        rows.push(python_references(partition, run));
        let mut calls = Partition {
            family: "call-targets",
            ..partition
        };
        if calls.state == "complete" {
            (calls.state, calls.reason) =
                family_state(run, NativeSyntaxRelation::RuffCallSite.as_str());
            if calls.state == "complete" {
                (calls.state, calls.reason) = family_state(
                    file.and_then(|id| pyrefly.get(&id).copied()),
                    crate::pyrefly_service::PyreflyRelation::CallTarget.relation_id(),
                );
            }
        }
        if publication == super::PublicationStage::Source && calls.reason == "provider_not_run" {
            calls.state = "pending";
            calls.reason = "semantic_work_pending";
        }
        rows.push(calls);
    }
    append_rust_partitions(&mut rows, runs, targets);
    if rust_requested && targets.is_empty() {
        let partition = undiscovered_rust_partition(publication);
        rows.push(partition);
        rows.push(Partition {
            family: "call-targets",
            ..partition
        });
        rows.push(unsupported_rust_references(partition));
    }
    register(builder, inventory, &rows)
}

fn undiscovered_rust_partition(publication: super::PublicationStage) -> Partition<'static> {
    Partition {
        family: "function-declarations",
        language: "rust",
        scope_kind: "workspace_context",
        path: b".",
        target: None,
        target_kind: None,
        target_platform: None,
        context: None,
        file: None,
        state: if publication == super::PublicationStage::Source {
            "pending"
        } else {
            "unavailable"
        },
        reason: if publication == super::PublicationStage::Source {
            "semantic_work_pending"
        } else {
            "cargo_context_preparation_incomplete"
        },
    }
}

fn pyrefly_by_file(runs: &[AdmittedProviderResult]) -> BTreeMap<[u8; 16], &AdmittedProviderResult> {
    runs.iter()
        .filter(|run| run.job().lane() == ProviderLane::Pyrefly)
        .flat_map(|run| match run.job().source().selection() {
            ProviderSourceSelection::Inventory(inventory) => inventory
                .selected_files()
                .map(|(file, _)| (file, run))
                .collect::<Vec<_>>(),
            ProviderSourceSelection::File { .. } => Vec::new(),
        })
        .collect()
}

fn python_references<'a>(
    partition: Partition<'a>,
    run: Option<&AdmittedProviderResult>,
) -> Partition<'a> {
    let (state, reason) = if partition.state == "complete" {
        family_state(run, NativeSyntaxRelation::RuffReference.as_str())
    } else {
        (partition.state, partition.reason)
    };
    Partition {
        family: "lexical-references",
        state,
        reason,
        ..partition
    }
}

fn unsupported_rust_references(partition: Partition<'_>) -> Partition<'_> {
    Partition {
        family: "lexical-references",
        state: "unsupported",
        reason: "rust_canonical_references_unimplemented",
        ..partition
    }
}

fn append_rust_partitions<'a>(
    rows: &mut Vec<Partition<'a>>,
    runs: &[AdmittedProviderResult],
    targets: &'a [RustTargetProgress],
) {
    let rust = runs
        .iter()
        .filter(|run| run.job().lane() == ProviderLane::Rustc)
        .map(|run| (run.job().context().analysis_context_id(), run))
        .collect::<BTreeMap<_, _>>();
    for target in targets {
        let run = target
            .context_id
            .and_then(|context| rust.get(&context).copied());
        let (state, reason) = if target.state == "processed" {
            family_state(run, RustcRelation::PublicItem.relation_id())
        } else if target.state == "pending" {
            ("pending", "semantic_work_pending")
        } else {
            ("unavailable", "compiler_target_unavailable")
        };
        let partition = Partition {
            family: "function-declarations",
            language: "rust",
            scope_kind: "cargo_target",
            path: &target.manifest,
            target: Some(&target.target),
            target_kind: Some(&target.target_kind),
            target_platform: target.target_platform.as_deref(),
            context: target.context_id,
            file: None,
            state,
            reason,
        };
        rows.push(partition);
        rows.push(unsupported_rust_references(partition));
        let mut calls = Partition {
            family: "call-targets",
            ..partition
        };
        if target.state == "processed" {
            (calls.state, calls.reason) = family_state(run, RustcRelation::Call.relation_id());
        }
        rows.push(calls);
    }
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
        "requested_processing_scope",
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
                "target_platform",
                true,
                Arc::new(StringArray::from_iter(
                    rows.iter().map(|row| row.target_platform),
                )),
            ),
            (
                "file_id",
                true,
                id16_array(rows.iter().map(|row| row.file.as_ref())),
            ),
            ("family", false, strings(|row| row.family)),
            ("processing_state", false, strings(|row| row.state)),
            ("reason", false, strings(|row| row.reason)),
        ],
    )
}
