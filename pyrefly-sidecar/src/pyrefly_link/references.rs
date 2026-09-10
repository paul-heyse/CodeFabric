//! Typed native reference observations with original-byte definition anchors.

use pyrefly::query::{SemanticReferenceKind, SemanticReferenceResponse, SemanticReferenceTarget};

use super::{
    Arc, BTreeMap, BooleanArray, CheckerSource, CommonIdentity, CoverageRow,
    FixedSizeBinaryBuilder, MAX_RELATION_ROWS, ModuleInput, PathBuf, PyreflyRelation, RecordBatch,
    StringArray, UInt64Array, as_u64, parse_digest, record_batch,
};

pub(super) struct ReferenceRow {
    occurrence: u64,
    start: u64,
    end: u64,
    name: String,
    kind: &'static str,
    target_ordinal: Option<u64>,
    definition: Option<DefinitionRow>,
    symbol_kind: Option<String>,
    is_module: Option<bool>,
    state: &'static str,
    mapping: &'static str,
}

struct DefinitionRow {
    file_id: String,
    content_digest: [u8; 32],
    start_byte: Option<u64>,
    end_byte: Option<u64>,
}

pub(super) fn project(
    response: Option<&SemanticReferenceResponse>,
    source: &CheckerSource,
    definitions: &BTreeMap<PathBuf, (&ModuleInput, &CheckerSource)>,
) -> Result<Vec<ReferenceRow>, String> {
    let Some(response) = response else {
        return Ok(vec![]);
    };
    let mut rows = Vec::new();
    for (ordinal, reference) in response.references.iter().enumerate() {
        if reference.start_byte >= reference.end_byte {
            return Err("checker reference range is empty or reversed".to_owned());
        }
        let start = source.original(reference.start_byte as usize)?;
        let end = source.original(reference.end_byte as usize)?;
        let kind = match reference.kind {
            SemanticReferenceKind::Read => "read",
            SemanticReferenceKind::Write => "write",
            SemanticReferenceKind::Delete => "delete",
            SemanticReferenceKind::Import => "import",
            SemanticReferenceKind::Unknown => "unknown",
        };
        let state = if reference.definition_unavailable || reference.targets.is_empty() {
            "unresolved"
        } else if reference.targets.len() == 1 {
            "resolved"
        } else {
            "candidates"
        };
        let targets = if reference.targets.is_empty() {
            vec![None]
        } else {
            reference.targets.iter().map(Some).collect()
        };
        for (target_ordinal, target) in targets.into_iter().enumerate() {
            if rows.len() >= MAX_RELATION_ROWS {
                return Err(
                    "semantic reference candidates exceed the relation row limit".to_owned(),
                );
            }
            let (definition, mapping) = map_definition(target, definitions)?;
            rows.push(ReferenceRow {
                occurrence: as_u64(ordinal),
                start,
                end,
                name: reference.name.clone(),
                kind,
                target_ordinal: target.map(|_| as_u64(target_ordinal)),
                definition,
                symbol_kind: target.and_then(|target| target.symbol_kind.clone()),
                is_module: target.map(|target| target.is_module),
                state,
                mapping,
            });
        }
    }
    Ok(rows)
}

fn map_definition(
    target: Option<&SemanticReferenceTarget>,
    definitions: &BTreeMap<PathBuf, (&ModuleInput, &CheckerSource)>,
) -> Result<(Option<DefinitionRow>, &'static str), String> {
    let Some(target) = target else {
        return Ok((None, "definition_unavailable"));
    };
    let Some((module, source)) = definitions.get(&target.definition.path) else {
        return Ok((None, "definition_outside_inventory"));
    };
    if !target.is_module && target.definition.start_byte >= target.definition.end_byte {
        // Native module endpoints and generated definitions may have no source occurrence.
        return Ok((None, "definition_without_source_range"));
    }
    Ok((
        Some(DefinitionRow {
            file_id: module.file_id.clone(),
            content_digest: parse_digest(&module.source_digest)?,
            start_byte: (!target.is_module)
                .then(|| source.original(target.definition.start_byte as usize))
                .transpose()?,
            end_byte: (!target.is_module)
                .then(|| source.original(target.definition.end_byte as usize))
                .transpose()?,
        }),
        if target.is_module {
            "exact_checker_module"
        } else {
            "exact_checker_definition"
        },
    ))
}

pub(super) fn qualify_coverage(
    coverage: &mut Vec<CoverageRow>,
    response: Option<&SemanticReferenceResponse>,
    rows: &[ReferenceRow],
) {
    coverage.retain(|row| row.family != "import_resolution");
    for (family, imports_only) in [("semantic_references", false), ("import_resolution", true)] {
        let selected = || {
            rows.iter()
                .filter(|row| !imports_only || row.kind == "import")
        };
        let complete = response.is_some_and(|response| response.complete)
            && selected().all(|row| row.state == "resolved" && row.definition.is_some());
        coverage.push(CoverageRow {
            family,
            surface: "Query::get_semantic_references_in_file / native definition resolver",
            requested: 1,
            completed: u64::from(complete),
            emitted: as_u64(selected().count()),
            completeness: if complete {
                "complete"
            } else if response.is_some() {
                "partial"
            } else {
                "unknown"
            },
            remainder: (!complete).then_some(if response.is_none() {
                "SEMANTIC_REFERENCE_QUERY_UNAVAILABLE"
            } else if response.is_some_and(|response| !response.complete) {
                "SEMANTIC_REFERENCE_CENSUS_LIMIT"
            } else {
                "UNRESOLVED_OR_UNMAPPED_SEMANTIC_REFERENCE"
            }),
            unknown: !complete,
        });
    }
}

pub(super) fn batch(
    common: &CommonIdentity<'_>,
    rows: &[ReferenceRow],
) -> Result<RecordBatch, String> {
    let mut digests = FixedSizeBinaryBuilder::with_capacity(rows.len(), 32);
    for row in rows {
        if let Some(definition) = &row.definition {
            digests
                .append_value(definition.content_digest)
                .map_err(|error| error.to_string())?;
        } else {
            digests.append_null();
        }
    }
    record_batch(
        common,
        PyreflyRelation::Reference,
        rows.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.occurrence),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.start),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.end),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.name.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.kind),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter().map(|row| row.target_ordinal),
            )),
            Arc::new(StringArray::from_iter(rows.iter().map(|row| {
                row.definition
                    .as_ref()
                    .map(|definition| definition.file_id.as_str())
            }))),
            Arc::new(digests.finish()),
            Arc::new(UInt64Array::from_iter(rows.iter().map(|row| {
                row.definition
                    .as_ref()
                    .and_then(|definition| definition.start_byte)
            }))),
            Arc::new(UInt64Array::from_iter(rows.iter().map(|row| {
                row.definition
                    .as_ref()
                    .and_then(|definition| definition.end_byte)
            }))),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|row| row.symbol_kind.as_deref()),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| row.is_module),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.state),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.mapping),
            )),
        ],
    )
}
