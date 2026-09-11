//! Exact source images travel with the selected Delta vector, independently of live disk.

use std::sync::Arc;

use arrow_array::{BinaryArray, BooleanArray, UInt64Array};

use super::{ProductionWorkspaceStartupError, input_observations};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{hash32_array, id16_array};
use crate::source_image::InventoryCaptureBundle;

pub(super) mod line_index;

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    capture: &InventoryCaptureBundle,
    stage: super::PublicationStage,
) -> Result<(), ProductionWorkspaceStartupError> {
    line_index::install(builder, capture)?;
    let images = capture.images();
    // Capture already bounds the total bytes. Arrow owns these copies through publication;
    // subsequent source reads select this exact relation version, never a workspace pathname.
    input_observations::register_immutable(
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
        input_identity(capture, b"codefabric.exact-source-bytes.inputs.v1\0"),
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

// These producers read only the captured inventory/images. Generation is part of every row;
// a new generation must not relabel or reuse an older produced fact, even with unchanged bytes.
fn input_identity(capture: &InventoryCaptureBundle, producer: &[u8]) -> [u8; 32] {
    let inventory = capture.inventory().inventory();
    let mut identity = super::digest32(
        producer,
        &[
            &inventory.workspace_id,
            &inventory.source_generation.to_be_bytes(),
            &inventory.digest,
        ],
    );
    // Inventory identity alone is insufficient: capture may retain a different admitted subset
    // after a read failure. Hash the existing immutable image/line-index identities, not the bytes.
    for image in capture.images() {
        identity = super::digest32(
            b"codefabric.captured-source-input.v1\0",
            &[
                &identity,
                &image.file_id,
                &image.path.raw_relative_path_bytes,
                &image.digest,
                &image.byte_length.to_be_bytes(),
                &image.line_index.digest,
                &image.line_index.format_version.to_be_bytes(),
            ],
        );
    }
    identity
}
