//! Exact entity spans and captured line indexes provide coordinate selection without text reads.

use std::sync::Arc;

use arrow_array::{Array, ArrayRef, ListArray, StructArray, UInt64Array};
use arrow_schema::{DataType, Field, Fields};
use datafusion::functions::core::expr_fn::get_field;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};

use super::{
    FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, empty, invalid, lit, plan,
};

pub(super) const RELATION: &str = "fact.code_entity_location";
const INDEX: &str = super::super::source_context::line_index::RELATION;
const COORDINATES: [&str; 4] = ["start_line", "start_column", "end_line", "end_column"];
const SUBJECT: [&str; 14] = [
    "entity_id",
    "public_entity_id",
    "entity_kind",
    "raw_kind",
    "language",
    "context_id",
    "file_id",
    "workspace_id",
    "content_digest",
    "source_generation",
    "relative_path",
    "start_byte",
    "end_byte",
    "fact_family",
];

pub(super) fn fields() -> Vec<FieldSpec> {
    super::source_context::fields()
        .into_iter()
        .filter(|(name, _, _)| SUBJECT.contains(name))
        .chain(COORDINATES.map(|name| (name, DataType::UInt64, true)))
        .collect()
}

pub(super) fn dependencies(indexed: bool) -> Vec<&'static str> {
    if indexed {
        vec![super::source_context::RELATION, INDEX]
    } else {
        vec![]
    }
}

pub(super) fn build(
    inputs: &TransformationInputs,
    indexed: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    if !indexed {
        return empty(fields());
    }
    let index = LogicalPlanBuilder::from(plan(inputs, INDEX)?)
        .alias("i")?
        .build()?;
    let selected = LogicalPlanBuilder::from(plan(inputs, super::source_context::RELATION)?)
        .filter(col("context_kind").eq(lit("exact source span")))?
        .alias("d")?
        .join(
            index,
            JoinType::Inner,
            (
                vec![
                    "d.workspace_id",
                    "d.file_id",
                    "d.content_digest",
                    "d.source_generation",
                ],
                vec![
                    "i.workspace_id",
                    "i.file_id",
                    "i.content_digest",
                    "i.source_generation",
                ],
            ),
            None,
        )?
        .project(
            SUBJECT
                .map(|name| col(format!("d.{name}")))
                .into_iter()
                .chain([coordinates()
                    .call(vec![
                        col("i.line_starts"),
                        col("i.byte_length"),
                        col("d.start_byte"),
                        col("d.end_byte"),
                    ])
                    .alias("coordinates")]),
        )?
        .build()?;
    Ok(LogicalPlanBuilder::from(selected)
        .project(fields().into_iter().map(|(name, _, _)| {
            if COORDINATES.contains(&name) {
                get_field(col("coordinates"), name).alias(name)
            } else {
                col(name)
            }
        }))?
        .distinct()?
        .build()?)
}

fn coordinates() -> Arc<ScalarUDF> {
    let output: Fields = COORDINATES
        .map(|name| Field::new(name, DataType::UInt64, false))
        .to_vec()
        .into();
    Arc::new(create_udf(
        "codefabric_captured_span_coordinates_v1",
        vec![
            DataType::List(Arc::new(Field::new("item", DataType::UInt64, true))),
            DataType::UInt64,
            DataType::UInt64,
            DataType::UInt64,
        ],
        DataType::Struct(output.clone()),
        Volatility::Immutable,
        Arc::new(move |values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let starts = arrays[0]
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| invalid("invalid captured line index"))?;
            let mut columns = [const { Vec::new() }; 4];
            for row in 0..starts.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    return Err(invalid("null captured coordinate input"));
                }
                let offsets = starts.value(row);
                let offsets = offsets
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .ok_or_else(|| invalid("invalid captured line offsets"))?;
                let length = super::number(&arrays[1], row)?;
                let start = super::number(&arrays[2], row)?;
                let end = super::number(&arrays[3], row)?;
                if offsets.is_empty()
                    || offsets.null_count() != 0
                    || offsets.value(0) != 0
                    || start > end
                    || end > length
                {
                    return Err(invalid("invalid captured coordinate span"));
                }
                for (side, offset) in [start, end].into_iter().enumerate() {
                    let line = offsets.values().partition_point(|value| *value <= offset);
                    columns[side * 2].push(line as u64);
                    columns[side * 2 + 1].push(offset - offsets.value(line - 1));
                }
            }
            let arrays = columns
                .into_iter()
                .map(|values| Arc::new(UInt64Array::from(values)) as ArrayRef)
                .collect();
            Ok(ColumnarValue::Array(Arc::new(StructArray::new(
                output.clone(),
                arrays,
                None,
            ))))
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{RecordBatch, types::UInt64Type};
    use datafusion::prelude::SessionContext;

    #[tokio::test]
    async fn captured_coordinates_preserve_crlf_cr_unicode_and_eof() {
        // Independently counted original bytes: a CRLF, b CR, cafe with an accented e, LF.
        let batch = RecordBatch::try_from_iter([
            (
                "lines",
                Arc::new(ListArray::from_iter_primitive::<UInt64Type, _, _>(
                    (0..5).map(|_| Some([Some(0), Some(3), Some(5), Some(11)])),
                )) as ArrayRef,
            ),
            ("length", Arc::new(UInt64Array::from(vec![11; 5]))),
            ("start", Arc::new(UInt64Array::from(vec![0, 3, 5, 8, 11]))),
            ("end", Arc::new(UInt64Array::from(vec![3, 4, 10, 8, 11]))),
        ])
        .unwrap();
        let frame = SessionContext::new().read_batch(batch).unwrap();
        let rows = frame
            .select(vec![
                coordinates()
                    .call(vec![col("lines"), col("length"), col("start"), col("end")])
                    .alias("position"),
            ])
            .unwrap()
            .collect()
            .await
            .unwrap();
        let output = rows[0]
            .column(0)
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        for (name, expected) in [
            ("start_line", vec![1, 2, 3, 3, 4]),
            ("start_column", vec![0, 0, 0, 3, 0]),
            ("end_line", vec![2, 2, 3, 3, 4]),
            ("end_column", vec![0, 1, 5, 3, 0]),
        ] {
            let values = output
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            assert_eq!(values.values().as_ref(), expected);
        }
    }
}
