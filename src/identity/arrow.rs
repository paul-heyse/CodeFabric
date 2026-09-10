//! Arrow scalar presentation of application-owned entity identities.

use std::sync::Arc;

use arrow_array::{Array as _, FixedSizeBinaryArray, StringArray};
use arrow_schema::DataType;
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};

pub(crate) fn public_entity_id() -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_public_entity_id_v1",
        vec![DataType::FixedSizeBinary(16), DataType::Utf8],
        DataType::Utf8,
        Volatility::Immutable,
        Arc::new(|values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let [ids, kinds] = arrays.as_slice() else {
                return Err(invalid("entity identity encoding requires ID and kind"));
            };
            let ids = ids
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .ok_or_else(|| invalid("entity identity requires fixed binary IDs"))?;
            let kinds = kinds
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| invalid("entity identity requires UTF-8 kinds"))?;
            let values = (0..ids.len())
                .map(|row| {
                    if ids.is_null(row) || kinds.is_null(row) {
                        return Ok(None);
                    }
                    let id = ids
                        .value(row)
                        .try_into()
                        .map_err(|_| invalid("entity identity width differs from 16 bytes"))?;
                    super::encode_public_id(
                        super::IdentityDomain::Entity,
                        Some(kinds.value(row)),
                        id,
                    )
                    .map(Some)
                    .map_err(|error| invalid(&error.to_string()))
                })
                .collect::<Result<Vec<_>, DataFusionError>>()?;
            Ok(ColumnarValue::Array(Arc::new(StringArray::from(values))))
        }),
    ))
}

fn invalid(message: &str) -> DataFusionError {
    DataFusionError::Execution(message.to_owned())
}
