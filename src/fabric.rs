//! Delta implementation boundary.

use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationTransaction {
    pub(crate) version: i64,
}

pub(crate) fn application_transaction() -> ApplicationTransaction {
    let transaction = deltalake::kernel::Transaction::new("codefabric/compatibility".to_owned(), 1);
    ApplicationTransaction {
        version: transaction.version,
    }
}

/// Validate that one application-owned Arrow schema has an exact Delta Kernel mapping.
pub(crate) fn validate_delta_schema(
    schema: arrow_schema::SchemaRef,
) -> Result<(), arrow_schema::ArrowError> {
    let _: deltalake::kernel::StructType = schema.try_into_kernel()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn wp09_delta_conversion_stays_inside_the_fabric_boundary() {
        for table in crate::schema_registry::table_specs() {
            super::validate_delta_schema(table.arrow_schema.clone()).unwrap();
        }
    }
}
