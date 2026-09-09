//! Exact source images travel with the selected Delta vector, independently of live disk.

use std::sync::Arc;

use arrow_array::{BinaryArray, BooleanArray, UInt64Array};

use super::{ProductionWorkspaceStartupError, input_observations};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{hash32_array, id16_array};
use crate::source_image::InventoryCaptureBundle;

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    capture: &InventoryCaptureBundle,
    stage: super::PublicationStage,
) -> Result<(), ProductionWorkspaceStartupError> {
    let images = capture.images();
    // Capture already bounds the total bytes. Arrow owns these copies through publication;
    // subsequent source reads select this exact relation version, never a workspace pathname.
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "exact_source_bytes",
        vec![
            (
                "workspace_id",
                false,
                id16_array(images.iter().map(|image| Some(&image.workspace_id))),
            ),
            (
                "source_generation",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    images.iter().map(|image| image.source_generation),
                )),
            ),
            (
                "file_id",
                false,
                id16_array(images.iter().map(|image| Some(&image.file_id))),
            ),
            (
                "content_digest",
                false,
                hash32_array(images.iter().map(|image| Some(&image.digest))),
            ),
            (
                "relative_path",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    images
                        .iter()
                        .map(|image| image.path.raw_relative_path_bytes.as_slice()),
                )),
            ),
            (
                "source_bytes",
                false,
                Arc::new(BinaryArray::from_iter_values(
                    images.iter().map(|image| image.bytes.as_ref()),
                )),
            ),
        ],
    )?;
    let inventory = capture.inventory().inventory();
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "input_inventory_state",
        vec![
            (
                "semantic_pending",
                false,
                Arc::new(BooleanArray::from(vec![
                    stage == super::PublicationStage::Source,
                ])),
            ),
            (
                "workspace_id",
                false,
                id16_array([Some(&inventory.workspace_id)]),
            ),
            (
                "source_generation",
                false,
                Arc::new(UInt64Array::from(vec![inventory.source_generation])),
            ),
            (
                "inventory_digest",
                false,
                hash32_array([Some(&inventory.digest)]),
            ),
        ],
    )
}
