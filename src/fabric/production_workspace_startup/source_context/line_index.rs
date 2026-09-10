//! Captured coordinate metadata can resolve source locations without disclosing source text.

use std::sync::Arc;

use arrow_array::{ArrayRef, ListArray, UInt64Array, types::UInt64Type};

use super::super::{ProductionWorkspaceStartupError, input_observations};
use crate::fabric::epoch_runtime::FabricSchemaRole;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::{hash32_array, id16_array};
use crate::source_image::InventoryCaptureBundle;

pub(in crate::fabric::production_workspace_startup) const RELATION: &str = "source.code_line_index";

pub(super) fn install(
    builder: &mut ProgrammaticFabricEpochBuilder,
    capture: &InventoryCaptureBundle,
) -> Result<(), ProductionWorkspaceStartupError> {
    let images = capture.images();
    let offsets: ArrayRef = Arc::new(ListArray::from_iter_primitive::<UInt64Type, _, _>(
        images
            .iter()
            .map(|image| Some(image.line_index.offsets.iter().copied().map(Some))),
    ));
    input_observations::register(
        builder,
        FabricSchemaRole::Source,
        "code_line_index",
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
                "byte_length",
                false,
                Arc::new(UInt64Array::from_iter_values(
                    images.iter().map(|image| image.byte_length),
                )),
            ),
            ("line_starts", false, offsets),
        ],
    )
}
