//! Native Cargo output selections and observations, separate from admitted compiler facts.

use super::super::{ProductionWorkspaceStartupError, step};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{id16_array, production_workspace_startup::input_observations};
use crate::resource_budget::ChargedValue;
use crate::rust_compilation_trust::CargoOutputObservation;
use arrow_array::{ArrayRef, BinaryArray, BooleanArray, StringArray, UInt64Array};
use std::sync::Arc;

pub(super) struct ObservedRun {
    pub context: [u8; 16],
    pub run: [u8; 16],
    pub run_label: String,
    pub output: Option<ChargedValue<CargoOutputObservation>>,
}

type Column = (&'static str, bool, ArrayRef);

fn columns(runs: &[&ObservedRun], workspace: [u8; 16], generation: u64) -> Vec<Column> {
    vec![
        (
            "workspace_id",
            false,
            id16_array(runs.iter().map(|_| Some(&workspace))),
        ),
        (
            "source_generation",
            false,
            Arc::new(UInt64Array::from_iter_values(
                runs.iter().map(|_| generation),
            )),
        ),
        (
            "context_id",
            false,
            id16_array(runs.iter().map(|run| Some(&run.context))),
        ),
        (
            "provider_run_id",
            false,
            id16_array(runs.iter().map(|run| Some(&run.run))),
        ),
        (
            "provider_run_label",
            false,
            Arc::new(StringArray::from_iter_values(
                runs.iter().map(|run| run.run_label.as_str()),
            )),
        ),
    ]
}

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    runs: &[ObservedRun],
    workspace: [u8; 16],
    generation: u64,
) -> Result<(), ProductionWorkspaceStartupError> {
    let mut selection = columns(&runs.iter().collect::<Vec<_>>(), workspace, generation);
    selection.extend([
        (
            "census_present",
            false,
            Arc::new(BooleanArray::from_iter(
                runs.iter().map(|run| Some(run.output.is_some())),
            )) as ArrayRef,
        ),
        (
            "cargo_succeeded",
            true,
            Arc::new(BooleanArray::from_iter(
                runs.iter()
                    .map(|run| run.output.as_ref().map(|output| output.succeeded)),
            )),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "cargo_output_selection",
        selection,
    )?;

    let artifacts = runs
        .iter()
        .flat_map(|run| {
            run.output.iter().flat_map(move |output| {
                output
                    .artifacts
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, artifact)| (run, ordinal, artifact))
            })
        })
        .collect::<Vec<_>>();
    let native = artifacts
        .iter()
        .map(|(_, _, artifact)| {
            serde_json::to_vec(artifact).map_err(|error| step("cargo-artifact-encoding", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = columns(
        &artifacts.iter().map(|(run, _, _)| *run).collect::<Vec<_>>(),
        workspace,
        generation,
    );
    rows.extend([
        (
            "ordinal",
            false,
            Arc::new(UInt64Array::from_iter_values(
                artifacts.iter().map(|(_, ordinal, _)| *ordinal as u64),
            )) as ArrayRef,
        ),
        (
            "package_id",
            false,
            Arc::new(StringArray::from_iter_values(
                artifacts
                    .iter()
                    .map(|(_, _, artifact)| artifact.package_id.as_str()),
            )),
        ),
        (
            "manifest_path",
            false,
            Arc::new(StringArray::from_iter_values(
                artifacts
                    .iter()
                    .map(|(_, _, artifact)| artifact.manifest_path.as_str()),
            )),
        ),
        (
            "target_name",
            false,
            Arc::new(StringArray::from_iter_values(
                artifacts
                    .iter()
                    .map(|(_, _, artifact)| artifact.target.name.as_str()),
            )),
        ),
        (
            "fresh",
            false,
            Arc::new(BooleanArray::from_iter(
                artifacts
                    .iter()
                    .map(|(_, _, artifact)| Some(artifact.fresh)),
            )),
        ),
        (
            "native_artifact_json",
            false,
            Arc::new(BinaryArray::from_iter_values(
                native.iter().map(Vec::as_slice),
            )),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "cargo_artifact_observation",
        rows,
    )?;

    let scripts = runs
        .iter()
        .flat_map(|run| {
            run.output.iter().flat_map(move |output| {
                output
                    .build_scripts
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, script)| (run, ordinal, script))
            })
        })
        .collect::<Vec<_>>();
    // Serialization uses the typed observation's digest-only environment field. Raw Cargo
    // stdout is never an input relation, so build-script environment values cannot escape here.
    let native = scripts
        .iter()
        .map(|(_, _, script)| {
            serde_json::to_vec(script).map_err(|error| step("cargo-build-output-encoding", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = columns(
        &scripts.iter().map(|(run, _, _)| *run).collect::<Vec<_>>(),
        workspace,
        generation,
    );
    rows.extend([
        (
            "ordinal",
            false,
            Arc::new(UInt64Array::from_iter_values(
                scripts.iter().map(|(_, ordinal, _)| *ordinal as u64),
            )) as ArrayRef,
        ),
        (
            "package_id",
            false,
            Arc::new(StringArray::from_iter_values(
                scripts
                    .iter()
                    .map(|(_, _, script)| script.package_id.as_str()),
            )),
        ),
        (
            "out_dir",
            false,
            Arc::new(StringArray::from_iter_values(
                scripts.iter().map(|(_, _, script)| script.out_dir.as_str()),
            )),
        ),
        (
            "observed_output_json",
            false,
            Arc::new(BinaryArray::from_iter_values(
                native.iter().map(Vec::as_slice),
            )),
        ),
    ]);
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "cargo_build_script_output",
        rows,
    )
}
