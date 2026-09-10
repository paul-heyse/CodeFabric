//! Associated member census and typed evidence from the same bounded native type transaction.

use pyrefly::query::{NativeClassMembers, NativeMember, NativeTypeGraph};

use super::{
    Arc, BooleanArray, CheckerSource, CommonIdentity, CoverageRow, MAX_RELATION_ROWS, MemberRow,
    PyreflyRelation, RelationAnalysis, StringArray, UInt64Array, as_u64, encode_relation,
    record_batch,
};

pub(super) fn display_rows(graph: Option<&NativeTypeGraph>) -> Vec<MemberRow> {
    graph
        .into_iter()
        .flat_map(|graph| &graph.classes)
        .flat_map(|class| {
            class.members.iter().filter_map(|member| {
                Some(MemberRow {
                    class_name: class.name.clone(),
                    ordinal: as_u64(member.ordinal),
                    name: member.name.clone(),
                    kind: member
                        .is_property
                        .filter(|value| *value)
                        .map(|_| "property".to_owned()),
                    annotation: member.annotation_rendering.clone()?,
                    is_final: member.is_final?,
                })
            })
        })
        .collect()
}

pub(super) fn coverage(rows: &mut Vec<CoverageRow>, graph: Option<&NativeTypeGraph>) {
    let complete = graph.is_some_and(|graph| {
        graph.member_census_complete
            && graph.classes.iter().all(|class| {
                class.complete
                    && class
                        .members
                        .iter()
                        .all(|member| member.unknown_reason.is_none())
            })
    });
    rows.push(CoverageRow {
        family: "members",
        surface: "Query::get_type_facts_in_file / associated class field census",
        requested: 1,
        completed: u64::from(complete),
        emitted: graph.map_or(0, |graph| {
            as_u64(graph.classes.iter().map(|class| class.members.len()).sum())
        }),
        completeness: if complete {
            "complete"
        } else if graph.is_some() {
            "partial"
        } else {
            "unknown"
        },
        remainder: (!complete).then_some(if graph.is_some() {
            "NATIVE_ASSOCIATED_MEMBER_CENSUS_OR_TYPE_INCOMPLETE"
        } else {
            "NATIVE_CLASS_CENSUS_UNAVAILABLE"
        }),
        unknown: !complete,
    });
}

struct Row<'a> {
    ordinal: u64,
    class: &'a NativeClassMembers,
    member: Option<&'a NativeMember>,
    class_start: u64,
    class_end: u64,
    member_start: Option<u64>,
    member_end: Option<u64>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one typed Arrow projection keeps all member columns in schema order"
)]
pub(super) fn project(
    common: &CommonIdentity<'_>,
    graph: Option<&NativeTypeGraph>,
    source: &CheckerSource,
) -> Result<RelationAnalysis, String> {
    let mut rows = Vec::new();
    for (ordinal, class) in graph
        .into_iter()
        .flat_map(|graph| &graph.classes)
        .enumerate()
    {
        let class_start = source.original(class.definition.start_byte as usize)?;
        let class_end = source.original(class.definition.end_byte as usize)?;
        for member in std::iter::once(None).chain(class.members.iter().map(Some)) {
            if rows.len() >= MAX_RELATION_ROWS {
                return Err("native member census exceeds the relation row limit".to_owned());
            }
            let definition = member.and_then(|member| member.definition.as_ref());
            rows.push(Row {
                ordinal: as_u64(ordinal),
                class,
                member,
                class_start,
                class_end,
                member_start: definition
                    .map(|anchor| source.original(anchor.start_byte as usize))
                    .transpose()?,
                member_end: definition
                    .map(|anchor| source.original(anchor.end_byte as usize))
                    .transpose()?,
            });
        }
    }
    let batch = record_batch(
        common,
        PyreflyRelation::MemberObservation,
        rows.len(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| {
                if row.member.is_some() {
                    "member"
                } else {
                    "class"
                }
            }))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.ordinal),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.class.name.as_str()),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.class_start),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.class_end),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter()
                    .map(|row| row.class.expected_members.map(as_u64)),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| Some(row.class.complete)),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|row| row.class.unknown_reason),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter()
                    .map(|row| row.member.map(|member| as_u64(member.ordinal))),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter()
                    .map(|row| row.member.map(|member| member.name.as_str())),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter().map(|row| row.member_start),
            )),
            Arc::new(UInt64Array::from_iter(
                rows.iter().map(|row| row.member_end),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter()
                    .map(|row| row.member.map(|member| member.definition_kind)),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.native_kind)),
            )),
            Arc::new(UInt64Array::from_iter(rows.iter().map(|row| {
                row.member
                    .and_then(|member| member.declared_type_index)
                    .map(as_u64)
            }))),
            Arc::new(UInt64Array::from_iter(rows.iter().map(|row| {
                row.member
                    .and_then(|member| member.computed_type_index)
                    .map(as_u64)
            }))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|row| {
                row.member.and_then(|member| member.annotation_present)
            }))),
            Arc::new(BooleanArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.is_property)),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.is_class_var)),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.is_final)),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.is_abstract)),
            )),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|row| {
                row.member.and_then(|member| member.descriptor_has_set)
            }))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|row| {
                row.member.and_then(|member| member.descriptor_has_delete)
            }))),
            Arc::new(StringArray::from_iter(rows.iter().map(|row| {
                row.member
                    .and_then(|member| member.annotation_rendering.as_deref())
            }))),
            Arc::new(StringArray::from_iter(
                rows.iter()
                    .map(|row| row.member.and_then(|member| member.unknown_reason)),
            )),
        ],
    )?;
    encode_relation(PyreflyRelation::MemberObservation, &batch)
}
