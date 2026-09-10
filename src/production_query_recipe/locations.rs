//! Location subjects reuse the exact native identity/context join used by literal subjects.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{
    EpochBoundSelectionTarget, ProductionQueryRecipeError, ProductionSemanticFormProgram,
    ProgrammaticFabricEpoch, facts, named, release_field_id,
};

pub(super) fn install(
    epoch: &ProgrammaticFabricEpoch,
    programs: &mut [ProductionSemanticFormProgram],
) -> Result<(), ProductionQueryRecipeError> {
    const RELATION: &str = "fact.code_entity_location";
    let Some(source) = facts::families::definition(epoch, RELATION)? else {
        return Ok(());
    };
    let field = |name| release_field_id(&format!("{RELATION}.{name}"));
    let fields = [
        "relative_path",
        "start_byte",
        "end_byte",
        "start_line",
        "start_column",
        "end_line",
        "end_column",
        "entity_kind",
        "language",
        "raw_kind",
    ]
    .into_iter()
    .map(|name| Ok((Arc::from(name), field(name)?)))
    .collect::<Result<BTreeMap<_, _>, ProductionQueryRecipeError>>()?;
    if fields.values().any(|field| !source.fields.contains(field)) {
        return Ok(());
    }
    named::install_selection(
        &named::SubjectSelection {
            source,
            public_id: field("public_entity_id")?,
            context_id: field("context_id")?,
            name: "location",
            selection_id: "selection.source-location",
            target: EpochBoundSelectionTarget::SourceLocations { fields },
        },
        programs,
    );
    Ok(())
}
