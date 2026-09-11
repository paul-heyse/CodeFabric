//! Detached Git input metadata in the exact source snapshot, outside the canonical CPG domains.

use std::sync::Arc;

use arrow_array::{
    BinaryArray, BooleanArray, StringArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};

use super::{ProductionWorkspaceStartupError, input_observations};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::git_state::captured_inputs::relation_digest;
use crate::inventory::SourceInventory;

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    inventory: &SourceInventory,
) -> Result<(), ProductionWorkspaceStartupError> {
    let Some(context) = &inventory.git_context else {
        return Ok(());
    };
    let identity = |digest: [u8; 32]| {
        super::super::digest32(
            b"codefabric.git-input-metadata.relations.v1\0",
            &[&inventory.workspace_id, &digest],
        )
    };
    let path_identity = identity(relation_digest(
        b"paths\0",
        context.paths.iter().map(|row| {
            (
                &row.path,
                &row.repository_root,
                row.classification,
                row.status,
            )
        }),
    ));
    let stages = || {
        context
            .paths
            .iter()
            .flat_map(|row| {
                row.stages
                    .iter()
                    .map(move |stage| (&row.path, &row.repository_root, stage))
            })
            .chain(context.submodules.iter().flat_map(|row| {
                row.path.iter().flat_map(move |path| {
                    row.stages
                        .iter()
                        .map(move |stage| (path, &row.repository_root, stage))
                })
            }))
    };
    let stage_identity = identity(relation_digest(b"stages\0", stages()));
    let attribute_identity = identity(relation_digest(
        b"attributes\0",
        context.paths.iter().flat_map(|row| {
            row.attributes
                .iter()
                .map(move |attribute| (&row.path, &row.repository_root, attribute))
        }),
    ));
    let paths = &context.paths;
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "git_path_context",
        vec![
            (
                "relative_path",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    paths.iter().map(|row| row.path.as_slice()),
                )),
            ),
            (
                "repository_root",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    paths.iter().map(|row| row.repository_root.as_slice()),
                )),
            ),
            (
                "classification_code",
                true,
                Arc::new(UInt16Array::from_iter(
                    paths.iter().map(|row| row.classification),
                )),
            ),
            (
                "observation_status",
                false,
                Arc::new(StringArray::from_iter_values(
                    paths.iter().map(|row| row.status),
                )),
            ),
        ],
        path_identity,
    )?;
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "git_index_stage",
        vec![
            (
                "relative_path",
                false,
                Arc::new(BinaryArray::from_iter(
                    (stages().map(|(path, _, _)| path.as_slice())).map(Some),
                )),
            ),
            (
                "repository_root",
                false,
                Arc::new(BinaryArray::from_iter(
                    (stages().map(|(_, root, _)| root.as_slice())).map(Some),
                )),
            ),
            (
                "stage",
                false,
                Arc::new(UInt8Array::from_iter_values(
                    stages().map(|(_, _, stage)| stage.stage),
                )),
            ),
            (
                "mode",
                false,
                Arc::new(UInt32Array::from_iter_values(
                    stages().map(|(_, _, stage)| stage.mode),
                )),
            ),
            (
                "flags",
                false,
                Arc::new(UInt32Array::from_iter_values(
                    stages().map(|(_, _, stage)| stage.flags),
                )),
            ),
            (
                "object_id",
                false,
                Arc::new(BinaryArray::from_iter(
                    (stages().map(|(_, _, stage)| stage.object_id.as_slice())).map(Some),
                )),
            ),
        ],
        stage_identity,
    )?;
    let attributes = || {
        paths
            .iter()
            .flat_map(|row| row.attributes.iter().map(move |attribute| (row, attribute)))
    };
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "git_attribute",
        vec![
            (
                "relative_path",
                false,
                Arc::new(BinaryArray::from_iter(
                    (attributes().map(|(row, _)| row.path.as_slice())).map(Some),
                )),
            ),
            (
                "repository_root",
                false,
                Arc::new(BinaryArray::from_iter(
                    (attributes().map(|(row, _)| row.repository_root.as_slice())).map(Some),
                )),
            ),
            (
                "name",
                false,
                Arc::new(StringArray::from_iter(
                    (attributes().map(|(_, attribute)| attribute.name.as_str())).map(Some),
                )),
            ),
            (
                "state",
                false,
                Arc::new(StringArray::from_iter(
                    (attributes().map(|(_, attribute)| attribute.state)).map(Some),
                )),
            ),
            (
                "value",
                true,
                Arc::new(BinaryArray::from_iter(
                    attributes().map(|(_, attribute)| attribute.value.as_deref()),
                )),
            ),
            (
                "pattern",
                false,
                Arc::new(BinaryArray::from_iter(
                    (attributes().map(|(_, attribute)| attribute.pattern.as_slice())).map(Some),
                )),
            ),
            (
                "source_path",
                true,
                Arc::new(BinaryArray::from_iter(
                    attributes().map(|(_, attribute)| attribute.source.as_deref()),
                )),
            ),
            (
                "line",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    attributes().map(|(_, attribute)| attribute.line),
                )),
            ),
        ],
        attribute_identity,
    )?;
    let boundaries = &context.submodules;
    input_observations::register_immutable(
        builder,
        FabricSchemaRole::Source,
        "git_submodule_boundary",
        vec![
            (
                "repository_root",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    boundaries.iter().map(|row| row.repository_root.as_slice()),
                )),
            ),
            (
                "relative_path",
                true,
                Arc::new(BinaryArray::from_iter(
                    boundaries.iter().map(|row| row.path.as_deref()),
                )),
            ),
            (
                "name",
                true,
                Arc::new(BinaryArray::from_iter(
                    boundaries.iter().map(|row| row.name.as_deref()),
                )),
            ),
            (
                "configuration_status",
                false,
                Arc::new(StringArray::from_iter_values(
                    boundaries.iter().map(|row| row.configuration_status),
                )),
            ),
            (
                "captured_sources",
                false,
                Arc::new(BooleanArray::from_iter(
                    boundaries.iter().map(|row| Some(row.captured_sources)),
                )),
            ),
            (
                "repository_observed",
                false,
                Arc::new(BooleanArray::from_iter(
                    boundaries.iter().map(|row| Some(row.repository_observed)),
                )),
            ),
        ],
        identity(relation_digest(
            b"submodule-boundaries\0",
            boundaries.iter().map(|row| {
                (
                    &row.repository_root,
                    &row.path,
                    &row.name,
                    row.configuration_status,
                    row.captured_sources,
                    row.repository_observed,
                )
            }),
        )),
    )
}
