//! Current, trust-qualified compiler input and produced-fact census. No replay admission.

use super::super::ProductionWorkspaceStartupError;
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{id16_array, production_workspace_startup::input_observations};
use crate::rustc_service::{AcceptedRustcCompilation, TrustQualifiedRustcCompilation};
use arrow_array::{ArrayRef, BinaryArray, BooleanArray, Int32Array, StringArray, UInt64Array};
use std::sync::Arc;

type Column = (&'static str, bool, ArrayRef);

fn pins(runs: &[&AcceptedRustcCompilation]) -> Vec<Column> {
    vec![
        (
            "workspace_id",
            false,
            id16_array(
                runs.iter()
                    .map(|run| Some(&run.admission.canonical_workspace_id)),
            ),
        ),
        (
            "source_generation",
            false,
            Arc::new(UInt64Array::from_iter_values(
                runs.iter().map(|run| run.admission.source_generation),
            )),
        ),
        (
            "context_id",
            false,
            id16_array(
                runs.iter()
                    .map(|run| Some(&run.admission.canonical_analysis_context_id)),
            ),
        ),
        (
            "provider_run_id",
            false,
            strings(runs.iter().map(|run| Some(run.control.header.run.as_str()))),
        ),
        (
            "compilation_unit_id",
            false,
            strings(
                runs.iter()
                    .map(|run| Some(run.control.header.compilation_unit.as_str())),
            ),
        ),
    ]
}

fn strings<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> ArrayRef {
    Arc::new(StringArray::from_iter(values))
}

fn binary<'a>(values: impl IntoIterator<Item = Option<&'a [u8]>>) -> ArrayRef {
    Arc::new(BinaryArray::from_iter(values))
}

fn terminal(value: crate::provider_contracts::ProviderTerminalStatus) -> &'static str {
    use crate::provider_contracts::ProviderTerminalStatus as T;
    match value {
        T::Complete => "complete",
        T::Partial => "partial",
        T::Unknown => "unknown",
        T::TimedOut => "timed_out",
        T::Cancelled => "cancelled",
        T::Corrupt => "corrupt",
        T::Oversized => "oversized",
        T::Failed => "failed",
    }
}

fn coverage(value: &crate::provider_contracts::ProviderCoverageState) -> &'static str {
    use crate::provider_contracts::ProviderCoverageState as C;
    match value {
        C::Complete { .. } => "complete",
        C::IntentionalRemainder { .. } => "intentional_remainder",
        C::Unknown { .. } => "unknown",
    }
}

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    compilations: &[TrustQualifiedRustcCompilation],
) -> Result<(), ProductionWorkspaceStartupError> {
    let runs = compilations
        .iter()
        .map(TrustQualifiedRustcCompilation::accepted)
        .collect::<Vec<_>>();
    let mut columns = pins(&runs);
    columns.push((
        "terminal_state",
        false,
        strings(
            runs.iter()
                .map(|run| Some(terminal(run.control.terminal.terminal))),
        ),
    ));
    columns.extend([
        (
            "census_present",
            false,
            Arc::new(BooleanArray::from_iter(
                runs.iter().map(|run| Some(run.invocation.is_some())),
            )) as ArrayRef,
        ),
        (
            "compiler_exit_status",
            false,
            Arc::new(Int32Array::from_iter_values(
                runs.iter()
                    .map(|run| run.control.terminal.compiler_exit_status),
            )),
        ),
        (
            "compiler_path",
            true,
            binary(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.compiler_path.as_slice())
            })),
        ),
        (
            "working_directory",
            true,
            binary(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.working_directory.as_slice())
            })),
        ),
        (
            "source_path",
            true,
            binary(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.source_path.as_slice())
            })),
        ),
        (
            "source_content_digest",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.source_content_digest.as_str())
            })),
        ),
        (
            "normalized_invocation_digest",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.normalized_invocation_digest.as_str())
            })),
        ),
        (
            "package_id",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.package_id.as_str())
            })),
        ),
        (
            "target_name",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.target_name.as_str())
            })),
        ),
        (
            "target_kind",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.target_kind.as_str())
            })),
        ),
        (
            "crate_name",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.crate_name.as_str())
            })),
        ),
        (
            "crate_type",
            true,
            strings(runs.iter().map(|run| {
                run.invocation
                    .as_ref()
                    .map(|value| value.crate_type.as_str())
            })),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "rustc_invocation",
        columns,
    )?;

    let arguments = runs
        .iter()
        .flat_map(|run| {
            run.invocation.iter().flat_map(move |value| {
                value
                    .arguments
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, argument)| (*run, ordinal, argument))
            })
        })
        .collect::<Vec<_>>();
    let mut columns = pins(&arguments.iter().map(|(run, _, _)| *run).collect::<Vec<_>>());
    columns.extend([
        (
            "ordinal",
            false,
            Arc::new(UInt64Array::from_iter_values(
                arguments.iter().map(|(_, ordinal, _)| *ordinal as u64),
            )) as ArrayRef,
        ),
        (
            "argument",
            false,
            binary(arguments.iter().map(|(_, _, value)| Some(value.as_slice()))),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "rustc_invocation_argument",
        columns,
    )?;

    let environment = runs
        .iter()
        .flat_map(|run| {
            run.invocation.iter().flat_map(move |value| {
                value
                    .environment
                    .iter()
                    .map(move |(name, digest)| (*run, name, digest))
            })
        })
        .collect::<Vec<_>>();
    let mut columns = pins(
        &environment
            .iter()
            .map(|(run, _, _)| *run)
            .collect::<Vec<_>>(),
    );
    columns.extend([
        (
            "name",
            false,
            binary(environment.iter().map(|(_, name, _)| Some(name.as_slice()))),
        ),
        (
            "value_digest",
            false,
            strings(
                environment
                    .iter()
                    .map(|(_, _, digest)| Some(digest.as_str())),
            ),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "rustc_invocation_environment",
        columns,
    )?;

    let facts = runs
        .iter()
        .flat_map(|run| {
            run.owners.iter().flat_map(move |owner| {
                owner
                    .relations
                    .iter()
                    .map(move |relation| (*run, owner, relation))
            })
        })
        .collect::<Vec<_>>();
    let mut columns = pins(&facts.iter().map(|(run, _, _)| *run).collect::<Vec<_>>());
    columns.push((
        "owner_coverage",
        false,
        strings(
            facts
                .iter()
                .map(|(_, owner, _)| Some(coverage(&owner.control.terminal.coverage))),
        ),
    ));
    columns.extend([
        (
            "owner_id",
            false,
            strings(
                facts
                    .iter()
                    .map(|(_, owner, _)| Some(owner.control.header.owner.as_str())),
            ),
        ),
        (
            "family_code",
            false,
            Arc::new(UInt64Array::from_iter_values(facts.iter().map(
                |(_, _, relation)| u64::from(relation.relation.family_code()),
            ))),
        ),
        (
            "logical_sequence",
            false,
            Arc::new(UInt64Array::from_iter_values(
                facts
                    .iter()
                    .map(|(_, _, relation)| relation.logical_sequence),
            )),
        ),
        (
            "row_count",
            false,
            Arc::new(UInt64Array::from_iter_values(
                facts.iter().map(|(_, _, relation)| relation.row_count),
            )),
        ),
        (
            "ipc_digest",
            false,
            strings(
                facts
                    .iter()
                    .map(|(_, _, relation)| Some(relation.arrow_ipc_digest.as_str())),
            ),
        ),
        (
            "schema_digest",
            false,
            strings(
                facts
                    .iter()
                    .map(|(_, _, relation)| Some(relation.schema_digest.as_str())),
            ),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "rustc_produced_relation",
        columns,
    )
}
