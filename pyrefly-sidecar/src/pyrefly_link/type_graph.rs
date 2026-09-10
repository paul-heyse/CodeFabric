//! Native type graph DTOs cross the process boundary as typed Arrow nodes, edges and observations.

use arrow_array::{ArrayRef, BinaryArray};
use pyrefly::query::{NativeTypeGraph, NativeTypeKind, NativeTypeLiteral, NativeTypeNode};

use super::{
    Arc, BTreeMap, BooleanArray, CheckerSource, CommonIdentity, CoverageRow,
    FixedSizeBinaryBuilder, MAX_RELATION_ROWS, ModuleInput, PathBuf, PyreflyRelation,
    RelationAnalysis, StringArray, UInt64Array, as_u64, encode_relation, parse_digest,
    record_batch,
};

pub(super) fn project(
    common: &CommonIdentity<'_>,
    graph: Option<&NativeTypeGraph>,
    source: &CheckerSource,
    definitions: &BTreeMap<PathBuf, (&ModuleInput, &CheckerSource)>,
) -> Result<Vec<RelationAnalysis>, String> {
    let nodes = graph.map_or(&[][..], |graph| graph.nodes.as_slice());
    let edges = nodes
        .iter()
        .enumerate()
        .flat_map(|(owner, node)| node.components.iter().map(move |edge| (owner, edge)))
        .collect::<Vec<_>>();
    let observations = graph.map_or(&[][..], |graph| graph.types.as_slice());
    if [nodes.len(), edges.len(), observations.len()]
        .into_iter()
        .any(|rows| rows > MAX_RELATION_ROWS)
    {
        return Err("native type graph exceeds the relation row limit".to_owned());
    }
    let locations = observations
        .iter()
        .map(|observation| source.byte_range(&observation.location))
        .collect::<Result<Vec<_>, _>>()?;
    let node_batch = record_batch(
        common,
        PyreflyRelation::TypeNode,
        nodes.len(),
        node_columns(nodes, definitions)?,
    )?;
    let edge_batch = record_batch(
        common,
        PyreflyRelation::TypeEdge,
        edges.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                edges.iter().map(|(owner, _)| as_u64(*owner)),
            )),
            Arc::new(UInt64Array::from_iter(
                edges.iter().map(|(_, edge)| edge.target.map(as_u64)),
            )),
            Arc::new(StringArray::from_iter_values(
                edges.iter().map(|(_, edge)| edge.role),
            )),
            Arc::new(UInt64Array::from_iter_values(
                edges.iter().map(|(_, edge)| as_u64(edge.ordinal)),
            )),
            Arc::new(StringArray::from_iter(
                edges.iter().map(|(_, edge)| edge.parameter_kind),
            )),
            Arc::new(StringArray::from_iter(
                edges.iter().map(|(_, edge)| edge.parameter_name.as_deref()),
            )),
            Arc::new(BooleanArray::from_iter(
                edges.iter().map(|(_, edge)| edge.parameter_required),
            )),
        ],
    )?;
    let occurrence_batch = record_batch(
        common,
        PyreflyRelation::TypeObservation,
        observations.len(),
        vec![
            Arc::new(UInt64Array::from_iter_values(
                (0..observations.len()).map(as_u64),
            )),
            Arc::new(UInt64Array::from_iter_values(
                locations.iter().map(|location| location.0),
            )),
            Arc::new(UInt64Array::from_iter_values(
                locations.iter().map(|location| location.1),
            )),
            Arc::new(UInt64Array::from_iter(
                observations
                    .iter()
                    .map(|location| location.type_index.map(as_u64)),
            )),
            Arc::new(StringArray::from_iter_values(
                observations.iter().map(|_| "checker-observed"),
            )),
        ],
    )?;
    [
        (PyreflyRelation::TypeNode, node_batch),
        (PyreflyRelation::TypeEdge, edge_batch),
        (PyreflyRelation::TypeObservation, occurrence_batch),
    ]
    .into_iter()
    .map(|(relation, batch)| encode_relation(relation, &batch))
    .collect()
}

fn node_columns(
    nodes: &[NativeTypeNode],
    definitions: &BTreeMap<PathBuf, (&ModuleInput, &CheckerSource)>,
) -> Result<Vec<ArrayRef>, String> {
    let mut files = Vec::with_capacity(nodes.len());
    let mut starts = Vec::with_capacity(nodes.len());
    let mut ends = Vec::with_capacity(nodes.len());
    let mut mappings = Vec::with_capacity(nodes.len());
    let mut digests = FixedSizeBinaryBuilder::with_capacity(nodes.len(), 32);
    for node in nodes {
        let definition = node
            .definition
            .as_ref()
            .and_then(|definition| {
                definitions
                    .get(&definition.path)
                    .map(|captured| (definition, captured))
            })
            .filter(|(definition, _)| definition.start_byte < definition.end_byte);
        if let Some((definition, (module, source))) = definition {
            files.push(Some(module.file_id.as_str()));
            starts.push(Some(source.original(definition.start_byte as usize)?));
            ends.push(Some(source.original(definition.end_byte as usize)?));
            digests
                .append_value(parse_digest(&module.source_digest)?)
                .map_err(|error| error.to_string())?;
            mappings.push("exact_checker_definition");
        } else {
            files.push(None);
            starts.push(None);
            ends.push(None);
            digests.append_null();
            mappings.push(if node.definition.is_some() {
                "definition_not_captured"
            } else {
                "no_definition_anchor"
            });
        }
    }
    Ok(vec![
        Arc::new(UInt64Array::from_iter_values((0..nodes.len()).map(as_u64))),
        Arc::new(StringArray::from_iter_values(
            nodes.iter().map(|node| kind(node.kind)),
        )),
        Arc::new(StringArray::from_iter_values(
            nodes.iter().map(|node| node.native_kind),
        )),
        Arc::new(StringArray::from_iter(
            nodes.iter().map(|node| node.name.as_deref()),
        )),
        Arc::new(StringArray::from_iter(
            nodes.iter().map(|node| node.intrinsic),
        )),
        Arc::new(StringArray::from_iter(nodes.iter().map(|node| node.style))),
        Arc::new(StringArray::from(files)),
        Arc::new(digests.finish()),
        Arc::new(UInt64Array::from(starts)),
        Arc::new(UInt64Array::from(ends)),
        Arc::new(StringArray::from(mappings)),
        Arc::new(StringArray::from_iter(nodes.iter().map(|node| {
            node.literal.as_ref().map(|value| match value {
                NativeTypeLiteral::String(_) => "string",
                NativeTypeLiteral::Bytes(_) => "bytes",
                NativeTypeLiteral::Integer(_) => "integer",
                NativeTypeLiteral::Boolean(_) => "boolean",
            })
        }))),
        Arc::new(StringArray::from_iter(nodes.iter().map(
            |node| match &node.literal {
                Some(NativeTypeLiteral::String(value) | NativeTypeLiteral::Integer(value)) => {
                    Some(value.as_str())
                }
                _ => None,
            },
        ))),
        Arc::new(BinaryArray::from_iter(nodes.iter().map(
            |node| match &node.literal {
                Some(NativeTypeLiteral::Bytes(value)) => Some(value.as_slice()),
                _ => None,
            },
        ))),
        Arc::new(BooleanArray::from_iter(nodes.iter().map(
            |node| match &node.literal {
                Some(NativeTypeLiteral::Boolean(value)) => Some(*value),
                _ => None,
            },
        ))),
    ])
}

const fn kind(value: NativeTypeKind) -> &'static str {
    match value {
        NativeTypeKind::Any => "any",
        NativeTypeKind::Error => "error",
        NativeTypeKind::Unknown => "unknown",
        NativeTypeKind::Never => "never",
        NativeTypeKind::None => "none",
        NativeTypeKind::Nominal => "nominal",
        NativeTypeKind::ClassObject => "class-object",
        NativeTypeKind::TypeObject => "type-object",
        NativeTypeKind::Union => "union",
        NativeTypeKind::Intersection => "intersection",
        NativeTypeKind::Tuple => "tuple",
        NativeTypeKind::Callable => "callable",
        NativeTypeKind::Function => "function",
        NativeTypeKind::Literal => "literal",
        NativeTypeKind::Unsupported => "unsupported",
    }
}

pub(super) fn coverage(rows: &mut Vec<CoverageRow>, graph: Option<&NativeTypeGraph>) {
    let complete = graph.is_some_and(|graph| {
        graph.complete
            && graph.nodes.iter().all(|node| {
                !matches!(
                    node.kind,
                    NativeTypeKind::Unsupported | NativeTypeKind::Unknown | NativeTypeKind::Error
                )
            })
    });
    rows.push(CoverageRow {
        family: "structural_types",
        surface: "Query::get_type_facts_in_file / native Type discriminants",
        requested: 1,
        completed: u64::from(complete),
        emitted: graph.map_or(0, |graph| as_u64(graph.nodes.len() + graph.types.len())),
        completeness: if complete {
            "complete"
        } else if graph.is_some() {
            "partial"
        } else {
            "unknown"
        },
        remainder: (!complete).then_some(if graph.is_none() {
            "NATIVE_TYPE_QUERY_UNAVAILABLE"
        } else if graph.is_some_and(|graph| !graph.complete) {
            "NATIVE_TYPE_GRAPH_LIMIT"
        } else {
            "NATIVE_TYPE_SHAPE_INCOMPLETE"
        }),
        unknown: !complete,
    });
}
