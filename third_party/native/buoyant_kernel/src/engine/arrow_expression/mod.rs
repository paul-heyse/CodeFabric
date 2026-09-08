//! Expression handling based on arrow-rs compute kernels.
use std::sync::Arc;

use evaluate_expression::{evaluate_expression, evaluate_predicate, extract_column};
use tracing::debug;

use crate::arrow::array::{self, ArrayBuilder, ArrayRef, RecordBatch, StructArray};
use crate::arrow::datatypes::DataType as ArrowDataType;
use crate::engine::arrow_data::{ArrowEngineData, NativeDataOwners, extract_record_batch};
use crate::engine::arrow_utils::apply_schema::{apply_schema, apply_schema_to};
use crate::error::{DeltaResult, Error};
use crate::expressions::{ArrayData, Expression, ExpressionRef, PredicateRef, Scalar};
use crate::schema::{DataType, PrimitiveType, SchemaRef};
use crate::utils::require;
use crate::{EngineData, EvaluationHandler, ExpressionEvaluator, PredicateEvaluator};

pub mod evaluate_expression;
pub mod opaque;
pub(crate) mod scalar_resource;
pub(crate) mod expression_resource;

#[cfg(test)]
mod tests;

// TODO leverage scalars / Datum

impl Scalar {
    /// Convert scalar to arrow array.
    pub fn to_array(&self, num_rows: usize) -> DeltaResult<ArrayRef> {
        let data_type = scalar_resource::prepare_scalar(self, num_rows)?;
        let mut builder = array::make_builder(&data_type, num_rows);
        self.append_to(&mut builder, num_rows)?;
        let output = builder.try_finish()?;
        scalar_resource::retain_fresh_buffers(output.as_ref())?;
        Ok(output)
    }

    // Arrow uses composable "builders" to assemble arrays one row at a time. Each concrete `Array`
    // type has a corresponding concrete `ArrayBuilder` type. For primitive types, the builder just
    // needs to `append` one value per row. For complex types, the builder needs to recursively
    // append values to each of its children as needed, and then its own `append` only defines the
    // validity for the row. Unfortunately, there is no generic way to append values to builders;
    // the `ArrayBuilder` trait only knows how to `finalize` itself to produce an `ArrayRef`. So we
    // have to cast each builder to the appropriate type, based on the scalar's data type. For
    // details, refer to the arrow documentation:
    //
    // https://docs.rs/arrow/latest/arrow/array/struct.PrimitiveBuilder.html
    // https://docs.rs/arrow/latest/arrow/array/struct.GenericListBuilder.html
    // https://docs.rs/arrow/latest/arrow/array/struct.StructBuilder.html
    //
    // NOTE: `ListBuilder` and `MapBuilder` are take generic element/key/value builders in order to
    // work with specific builder types directly. However, `array::make_builder` instantiates them
    // with `Box<dyn Builder>` instead, which greatly simplifies our job in working with them. We
    // can just extract the builder trait, and let recursive calls cast it to the desired type.
    //
    // WARNING: List and map builders do _NOT_ require appending any child entries to NULL list/map
    // rows, because empty list/map is a valid state. But struct builders _DO_ require appending
    // (possibly NULL) entries in order to preserve consistent row counts between the struct and its
    // fields.
    fn append_to(&self, builder: &mut dyn ArrayBuilder, num_rows: usize) -> DeltaResult<()> {
        use Scalar::*;
        macro_rules! builder_as {
            ($t:ty) => {{
                builder.as_any_mut().downcast_mut::<$t>().ok_or_else(|| {
                    Error::invalid_expression(format!("Invalid builder for {}", self.data_type()))
                })?
            }};
        }

        // Use append_value_n for primitive builders that support batch append
        macro_rules! append_val_n_as {
            ($t:ty, $val:expr) => {{
                let builder = builder_as!($t);
                builder.append_value_n($val, num_rows);
            }};
        }

        // Use append_value in a loop for builders without batch append (String, Binary)
        // TODO: Remove after https://github.com/apache/arrow-rs/pull/9426 gets in
        macro_rules! append_val_as {
            ($t:ty, $val:expr) => {{
                let builder = builder_as!($t);
                for _ in 0..num_rows {
                    builder.append_value($val);
                }
            }};
        }

        match self {
            Integer(val) => append_val_n_as!(array::Int32Builder, *val),
            Long(val) => append_val_n_as!(array::Int64Builder, *val),
            Short(val) => append_val_n_as!(array::Int16Builder, *val),
            Byte(val) => append_val_n_as!(array::Int8Builder, *val),
            Float(val) => append_val_n_as!(array::Float32Builder, *val),
            Double(val) => append_val_n_as!(array::Float64Builder, *val),
            String(val) => append_val_as!(array::StringBuilder, val),
            Boolean(val) => builder_as!(array::BooleanBuilder).append_n(num_rows, *val),
            Timestamp(val) | TimestampNtz(val) => {
                // timezone was already set at builder construction time
                append_val_n_as!(array::TimestampMicrosecondBuilder, *val)
            }
            #[cfg(feature = "nanosecond-timestamps")]
            TimestampNanos(val) | TimestampNanosNtz(val) => {
                // timezone was already set at builder construction time
                append_val_n_as!(array::TimestampNanosecondBuilder, *val)
            }
            Date(val) => append_val_n_as!(array::Date32Builder, *val),
            Binary(val) => append_val_as!(array::BinaryBuilder, val),
            // precision and scale were already set at builder construction time
            Decimal(val) => append_val_n_as!(array::Decimal128Builder, val.bits()),
            Struct(data) => {
                let builder = builder_as!(array::StructBuilder);
                require!(
                    builder.num_fields() == data.fields().len(),
                    Error::generic("Struct builder has wrong number of fields")
                );
                let field_builders = builder.field_builders_mut().iter_mut();
                for (builder, value) in field_builders.zip(data.values()) {
                    value.append_to(builder, num_rows)?;
                }
                // TODO: Loop can be removed after: https://github.com/apache/arrow-rs/pull/9430
                for _ in 0..num_rows {
                    builder.append(true);
                }
            }
            Array(data) => {
                let builder = builder_as!(array::ListBuilder<Box<dyn ArrayBuilder>>);
                for _ in 0..num_rows {
                    for value in data.array_elements() {
                        value.append_to(builder.values(), 1)?;
                    }
                    builder.append(true);
                }
            }
            Map(data) => {
                let builder =
                    builder_as!(array::MapBuilder<Box<dyn ArrayBuilder>, Box<dyn ArrayBuilder>>);
                for _ in 0..num_rows {
                    for (key, val) in data.pairs() {
                        key.append_to(builder.keys(), 1)?;
                        val.append_to(builder.values(), 1)?;
                    }
                    builder.append(true)?;
                }
            }
            Null(data_type) => Self::append_null(builder, data_type, num_rows)?,
        }

        Ok(())
    }

    fn append_null(
        builder: &mut dyn ArrayBuilder,
        data_type: &DataType,
        num_rows: usize,
    ) -> DeltaResult<()> {
        // Almost the same as above -- differs only in the data type parameter
        macro_rules! builder_as {
            ($t:ty) => {{
                builder.as_any_mut().downcast_mut::<$t>().ok_or_else(|| {
                    Error::invalid_expression(format!("Invalid builder for {data_type}"))
                })?
            }};
        }

        macro_rules! append_nulls_as {
            ($t:ty) => {{
                let builder = builder_as!($t);
                builder.append_nulls(num_rows);
            }};
        }

        match *data_type {
            DataType::INTEGER => append_nulls_as!(array::Int32Builder),
            DataType::LONG => append_nulls_as!(array::Int64Builder),
            DataType::SHORT => append_nulls_as!(array::Int16Builder),
            DataType::BYTE => append_nulls_as!(array::Int8Builder),
            DataType::FLOAT => append_nulls_as!(array::Float32Builder),
            DataType::DOUBLE => append_nulls_as!(array::Float64Builder),
            DataType::STRING => append_nulls_as!(array::StringBuilder),
            DataType::BOOLEAN => append_nulls_as!(array::BooleanBuilder),
            DataType::TIMESTAMP | DataType::TIMESTAMP_NTZ => {
                append_nulls_as!(array::TimestampMicrosecondBuilder)
            }
            #[cfg(feature = "nanosecond-timestamps")]
            DataType::TIMESTAMP_NANOS | DataType::TIMESTAMP_NANOS_NTZ => {
                append_nulls_as!(array::TimestampNanosecondBuilder)
            }
            DataType::DATE => append_nulls_as!(array::Date32Builder),
            DataType::BINARY => append_nulls_as!(array::BinaryBuilder),
            DataType::Primitive(PrimitiveType::Decimal(_)) => {
                append_nulls_as!(array::Decimal128Builder)
            }
            DataType::Struct(ref stype) => {
                // WARNING: Unlike ArrayBuilder and MapBuilder, StructBuilder always requires us to
                // insert an entry for each child builder, even when we're inserting NULL.
                let builder = builder_as!(array::StructBuilder);
                require!(
                    builder.num_fields() == stype.num_fields(),
                    Error::generic("Struct builder has wrong number of fields")
                );
                let field_builders = builder.field_builders_mut().iter_mut();
                for (builder, field) in field_builders.zip(stype.fields()) {
                    Self::append_null(builder, &field.data_type, num_rows)?;
                }
                builder.append_nulls(num_rows);
            }
            DataType::Array(_) => append_nulls_as!(array::ListBuilder<Box<dyn ArrayBuilder>>),
            DataType::Map(_) => {
                // For some reason, there is no `MapBuilder::append_null` method -- even tho
                // StructBuilder and ListBuilder both provide it.
                let builder =
                    builder_as!(array::MapBuilder<Box<dyn ArrayBuilder>, Box<dyn ArrayBuilder>>);
                // TODO: Can be removed after https://github.com/apache/arrow-rs/pull/9432
                for _ in 0..num_rows {
                    builder.append(false)?;
                }
            }
            DataType::VOID => append_nulls_as!(array::NullBuilder),
            DataType::Variant(_) => {
                return Err(Error::unsupported(
                    "Variant is not supported as scalar yet.",
                ));
            }
            DataType::INTERVAL_YEAR_MONTH | DataType::INTERVAL_DAY_TIME => {
                return Err(Error::unsupported(
                    "Interval is not supported as scalar yet.",
                ));
            }
        }
        Ok(())
    }
}

impl ArrayData {
    /// Convert kernel [`ArrayData`] to an Arrow [`ArrayRef`] of the equivalent type.
    pub fn to_arrow(&self) -> DeltaResult<ArrayRef> {
        let arrow_data_type = scalar_resource::prepare_array(self)?;

        let elements = self.array_elements();
        let mut builder = array::make_builder(&arrow_data_type, elements.len());
        for element in elements {
            element.append_to(&mut builder, 1)?;
        }

        let output = builder.try_finish()?;
        scalar_resource::retain_fresh_buffers(output.as_ref())?;
        Ok(output)
    }
}

#[derive(Debug)]
pub struct ArrowEvaluationHandler;

impl EvaluationHandler for ArrowEvaluationHandler {
    fn new_expression_evaluator(
        &self,
        schema: SchemaRef,
        expression: ExpressionRef,
        output_type: DataType,
    ) -> DeltaResult<Arc<dyn ExpressionEvaluator>> {
        expression_resource::arc::<DefaultExpressionEvaluator>("native_expression_evaluator_owner")?;
        Ok(Arc::new(DefaultExpressionEvaluator {
            _input_schema: schema,
            expression,
            output_type,
            native_owners: NativeDataOwners::current(),
            resource_owner: arrow_schema_59::resource::ResourceOwnerHandle::capture(),
        }))
    }

    fn new_predicate_evaluator(
        &self,
        schema: SchemaRef,
        predicate: PredicateRef,
    ) -> DeltaResult<Arc<dyn PredicateEvaluator>> {
        expression_resource::arc::<DefaultPredicateEvaluator>("native_predicate_evaluator_owner")?;
        Ok(Arc::new(DefaultPredicateEvaluator {
            _input_schema: schema,
            predicate,
            native_owners: NativeDataOwners::current(),
            resource_owner: arrow_schema_59::resource::ResourceOwnerHandle::capture(),
        }))
    }

    /// Create a single-row array with all-null leaf values. Note that if a nested struct is
    /// included in the `output_type`, the entire struct will be NULL (instead of a not-null struct
    /// with NULL fields).
    fn null_row(&self, output_schema: SchemaRef) -> DeltaResult<Box<dyn EngineData>> {
        let mut arrays = expression_resource::new_vec(output_schema.fields().len(), "native_null_row_columns")?;
        for field in output_schema.fields() {
            let kind = scalar_resource::prepare_null(field.data_type(), 1)?;
            let mut builder = array::make_builder(&kind, 1);
            Scalar::append_null(builder.as_mut(), field.data_type(), 1)?;
            let array = builder.try_finish()?;
            scalar_resource::retain_fresh_buffers(array.as_ref())?;
            arrays.push(array);
        }
        let schema = Arc::new(scalar_resource::prepare_schema(&output_schema)?);
        let batch = RecordBatch::try_new_with_options(schema, arrays,
            &array::RecordBatchOptions::new().with_row_count(Some(1)))?;
        expression_resource::boxed::<ArrowEngineData>("native_null_row_engine_data")?;
        Ok(Box::new(ArrowEngineData::new(batch)))
    }

    fn create_many(
        &self,
        schema: SchemaRef,
        rows: &[&[Scalar]],
    ) -> DeltaResult<Box<dyn EngineData>> {
        // Preserve the native ungoverned diagnostic; governed validation is
        // borrowed and preadmitted in prepare_rows before any builder work.
        if crate::resource::current_resource_scope().is_none() && arrow_schema_59::resource::current_resource_owner().is_none() {
            for (row_idx, row) in rows.iter().enumerate() {
                if row.len() != schema.fields().len() {
                    return Err(Error::generic(format!("Row {} has {} scalars but schema has {} fields", row_idx, row.len(), schema.fields().len())));
                }
            }
        }
        scalar_resource::prepare_rows(&schema, rows)?;
        let arrow_schema = Arc::new(scalar_resource::prepare_schema(&schema)?);
        let num_rows = rows.len();
        let mut builders = expression_resource::new_vec::<Box<dyn ArrayBuilder>>(arrow_schema.fields().len(), "native_create_many_builders")?;
        for field in arrow_schema.fields() {
            scalar_resource::prepare_constructor(field.data_type(), num_rows)?;
            builders.push(array::make_builder(field.data_type(), num_rows));
        }
        for ((col_idx, builder), field) in builders.iter_mut().enumerate().zip(schema.fields()) {
            for (row_idx, row) in rows.iter().enumerate() {
                row[col_idx].append_to(builder.as_mut(), 1).map_err(|error| {
                    if error.is_resource_exhausted() || crate::resource::current_resource_scope().is_some() || arrow_schema_59::resource::current_resource_owner().is_some() { return error; }
                    Error::generic(format!("Row {row_idx}, field '{}' (expected type {}, got {}): {error}", field.name(), field.data_type(), row[col_idx].data_type()))
                })?;
            }
        }
        let mut arrays = expression_resource::new_vec(builders.len(), "native_create_many_columns")?;
        for mut builder in builders {
            let array = builder.try_finish()?;
            scalar_resource::retain_fresh_buffers(array.as_ref())?;
            arrays.push(array);
        }
        let batch = RecordBatch::try_new_with_options(arrow_schema, arrays,
            &array::RecordBatchOptions::new().with_row_count(Some(num_rows)))?;
        expression_resource::boxed::<ArrowEngineData>("native_create_many_engine_data")?;
        Ok(Box::new(ArrowEngineData::new(batch)))
    }
}

#[derive(Debug)]
pub struct DefaultExpressionEvaluator {
    _input_schema: SchemaRef,
    expression: ExpressionRef,
    output_type: DataType,
    native_owners: NativeDataOwners,
    resource_owner: arrow_schema_59::resource::ResourceOwnerHandle,
}

impl ExpressionEvaluator for DefaultExpressionEvaluator {
    fn evaluate(&self, batch: &dyn EngineData) -> DeltaResult<Box<dyn EngineData>> {
        expression_resource::check_evaluator_owner(&self.native_owners)?;
        expression_resource::check_arrow_owner(&self.resource_owner)?;
        debug!("Arrow evaluator evaluating: {:#?}", self.expression);
        let input_owners = batch
            .any_ref()
            .downcast_ref::<ArrowEngineData>()
            .ok_or_else(|| Error::engine_data_type("ArrowEngineData"))?
            .native_owners()
            .clone();
        let batch = extract_record_batch(batch)?;
        // TODO: make sure we have matching schemas for validation
        // if batch.schema().as_ref() != &input_schema {
        //     return Err(Error::Generic(format!(
        //         "input schema does not match batch schema: {:?} != {:?}",
        //         input_schema,
        //         batch.schema()
        //     )));
        // };
        let batch = match (self.expression.as_ref(), &self.output_type) {
            (Expression::StructPatch(patch), DataType::Struct(_)) if patch.is_empty() => {
                // Empty patch optimization: Skip expression evaluation and directly apply the
                // output schema to the input RecordBatch. This is used to cheaply apply a new
                // output schema to existing data without changing it, e.g. for column mapping.
                let array = match patch.input_path() {
                    None => {
                        expression_resource::admit(std::alloc::Layout::array::<ArrayRef>(batch.num_columns()).map_err(|_| crate::resource::ResourceExhausted { kind: "native_evaluator_input_columns", requested: batch.num_columns(), limit: isize::MAX as usize })?, "native_evaluator_input_columns")?;
                        expression_resource::arc::<StructArray>("native_evaluator_input_struct")?;
                        Arc::new(StructArray::from(batch.clone()))
                    },
                    Some(path) => extract_column(batch, path)?,
                };
                apply_schema(&array, &self.output_type)?
            }
            (expr, output_type @ DataType::Struct(_)) => {
                let array_ref = evaluate_expression(expr, batch, Some(output_type))?;
                apply_schema(&array_ref, output_type)?
            }
            (expr, output_type) => {
                let array_ref = evaluate_expression(expr, batch, Some(output_type))?;
                let array_ref = apply_schema_to(&array_ref, output_type)?;
                let arrow_type = scalar_resource::prepare_type(output_type)?;
                let schema = expression_resource::output_schema(arrow_type)?;
                let mut columns = expression_resource::new_vec(1, "native_evaluator_output_columns")?;
                columns.push(array_ref);
                RecordBatch::try_new(schema, columns)?
            }
        };

        let owners = input_owners.try_merge(self.native_owners.clone())?;
        expression_resource::boxed::<ArrowEngineData>("native_evaluator_engine_data")?;
        Ok(Box::new(ArrowEngineData::new(batch).try_inherit_owners(owners)?))
    }
}

#[derive(Debug)]
pub struct DefaultPredicateEvaluator {
    _input_schema: SchemaRef,
    predicate: PredicateRef,
    native_owners: NativeDataOwners,
    resource_owner: arrow_schema_59::resource::ResourceOwnerHandle,
}

impl PredicateEvaluator for DefaultPredicateEvaluator {
    fn evaluate(&self, batch: &dyn EngineData) -> DeltaResult<Box<dyn EngineData>> {
        expression_resource::check_evaluator_owner(&self.native_owners)?;
        expression_resource::check_arrow_owner(&self.resource_owner)?;
        debug!("Arrow evaluator evaluating: {:#?}", self.predicate);
        let input_owners = batch
            .any_ref()
            .downcast_ref::<ArrowEngineData>()
            .ok_or_else(|| Error::engine_data_type("ArrowEngineData"))?
            .native_owners()
            .clone();
        let batch = extract_record_batch(batch)?;
        // TODO: make sure we have matching schemas for validation
        // if batch.schema().as_ref() != &input_schema {
        //     return Err(Error::Generic(format!(
        //         "input schema does not match batch schema: {:?} != {:?}",
        //         input_schema,
        //         batch.schema()
        //     )));
        // };
        let array = evaluate_predicate(&self.predicate, batch, false)?;
        let schema = expression_resource::output_schema(ArrowDataType::Boolean)?;
        expression_resource::arc::<array::BooleanArray>("native_predicate_output_array")?;
        let mut columns = expression_resource::new_vec(1, "native_predicate_output_columns")?;
        columns.push(Arc::new(array) as ArrayRef);
        let batch = RecordBatch::try_new(schema, columns)?;
        let owners = input_owners.try_merge(self.native_owners.clone())?;
        expression_resource::boxed::<ArrowEngineData>("native_evaluator_engine_data")?;
        Ok(Box::new(ArrowEngineData::new(batch).try_inherit_owners(owners)?))
    }
}
