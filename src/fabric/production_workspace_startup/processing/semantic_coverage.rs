//! Compact native coverage indexing; each accepted module/family row is inspected once.

use super::{
    AdmittedProviderResult, BTreeMap, Partition, ProductionWorkspaceStartupError, ProviderLane,
};
use crate::identity::{IdentityDomain, decode_public_id};
use crate::pyrefly_service::PyreflyRelation;
use arrow_array::{Array, BooleanArray, StringArray, UInt64Array};

type CoverageIndex = BTreeMap<([u8; 16], [u8; 16], &'static str), (&'static str, &'static str)>;

pub(super) fn index(
    runs: &[AdmittedProviderResult],
) -> Result<CoverageIndex, ProductionWorkspaceStartupError> {
    let mut index = BTreeMap::new();
    for run in runs
        .iter()
        .filter(|run| run.job().lane() == ProviderLane::Pyrefly)
    {
        let context = run.job().context().analysis_context_id();
        for batch in run
            .result()
            .relations()
            .iter()
            .filter(|relation| {
                relation.relation().as_str() == PyreflyRelation::Coverage.relation_id()
            })
            .flat_map(crate::provider_contracts::ProviderRelationOutput::batches)
        {
            let error = || {
                super::super::step(
                    "pyrefly-semantic-coverage",
                    "invalid accepted native coverage schema",
                )
            };
            let text = |name| {
                batch
                    .column_by_name(name)
                    .and_then(|column| column.as_any().downcast_ref::<StringArray>())
                    .ok_or_else(error)
            };
            let number = |name| {
                batch
                    .column_by_name(name)
                    .and_then(|column| column.as_any().downcast_ref::<UInt64Array>())
                    .ok_or_else(error)
            };
            let families = text("fact_family")?;
            let files = text("file_id")?;
            let states = text("completeness")?;
            let requested = number("requested_units")?;
            let completed = number("completed_units")?;
            let unknown = batch
                .column_by_name("unknown_semantics")
                .and_then(|column| column.as_any().downcast_ref::<BooleanArray>())
                .ok_or_else(error)?;
            for row in 0..batch.num_rows() {
                if [
                    families as &dyn Array,
                    files,
                    states,
                    requested,
                    completed,
                    unknown,
                ]
                .iter()
                .any(|column| column.is_null(row))
                {
                    return Err(error());
                }
                let family = match families.value(row) {
                    "semantic_references" => "semantic-references",
                    "import_resolution" => "imports",
                    "structural_types" => "types",
                    "members" => "members",
                    _ => continue,
                };
                let file = decode_public_id(IdentityDomain::SourceFile, None, files.value(row))
                    .map_err(|error| super::super::step("pyrefly-semantic-coverage", error))?;
                let state = if states.value(row) == "complete"
                    && !unknown.value(row)
                    && requested.value(row) == completed.value(row)
                {
                    ("complete", "")
                } else {
                    ("partial", "native_semantic_coverage_incomplete")
                };
                index
                    .entry((context, file, family))
                    .and_modify(|previous| {
                        if *previous != state {
                            *previous = ("partial", "conflicting_semantic_coverage");
                        }
                    })
                    .or_insert(state);
            }
        }
    }
    Ok(index)
}

pub(super) fn append<'a>(
    rows: &mut Vec<Partition<'a>>,
    partition: Partition<'a>,
    run: Option<&AdmittedProviderResult>,
    syntax: Option<&AdmittedProviderResult>,
    index: &CoverageIndex,
    publication: super::super::PublicationStage,
) {
    for family in [
        "modules",
        "semantic-references",
        "imports",
        "types",
        "members",
    ] {
        let mut semantic = Partition {
            family,
            ..partition
        };
        if let Some(file) = partition.file {
            semantic.context = run.map(|run| run.job().context().analysis_context_id());
            (semantic.state, semantic.reason) = super::family_state(
                run,
                if family == "modules" {
                    PyreflyRelation::ModuleContext.relation_id()
                } else if family == "members" {
                    PyreflyRelation::MemberObservation.relation_id()
                } else if family == "types" {
                    PyreflyRelation::TypeNode.relation_id()
                } else {
                    PyreflyRelation::Reference.relation_id()
                },
            );
            if family != "modules" {
                // The accepted native per-file census refines a stream-wide remainder. An
                // unresolved name in another file (or outside imports) cannot widen this scope.
                if let Some(state) = semantic
                    .context
                    .and_then(|context| index.get(&(context, file, family)))
                {
                    (semantic.state, semantic.reason) = *state;
                } else if semantic.state == "complete" {
                    (semantic.state, semantic.reason) =
                        ("unknown", "native_semantic_coverage_missing");
                }
            }
            if family == "imports" {
                let syntax_state = super::family_state(
                    syntax,
                    crate::provider_native_syntax::NativeSyntaxRelation::RuffImport.as_str(),
                );
                if syntax_state.0 != "complete" {
                    (semantic.state, semantic.reason) = syntax_state;
                }
            }
            if publication == super::super::PublicationStage::Source
                && semantic.reason == "provider_not_run"
            {
                semantic.state = "pending";
                semantic.reason = "semantic_work_pending";
            }
        }
        rows.push(semantic);
    }
}
