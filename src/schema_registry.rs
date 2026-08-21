//! Generated Arrow schema registry for durable, overlay, and operational surfaces.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use arrow_schema::{DataType, Field, Fields, Schema, SchemaRef, TimeUnit};

/// Durable Delta mutation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableMutationClass {
    StaticDimension,
    CurrentSingleton,
    OwnerReplacedFact,
    PublicationAppend,
    DerivedOwnerReplaced,
    GlobalDerivedReplacement,
}

/// Hot-overlay mutation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayMutationPolicy {
    OwnerReplace,
    PrimaryKeyUpsert,
    FullTableReplace,
    BaseImmutable,
    NotApplicable,
}

/// Query-visible materialization role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializationRole {
    DurableEffective,
    BundleDimension,
    QueryTimeDerived,
    OperationalProjection,
}

/// One immutable generated schema contract.
#[derive(Clone, Debug)]
pub struct TableSpec {
    pub table_code: i16,
    pub name: &'static str,
    pub schema_version: &'static str,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub partition_columns: &'static [&'static str],
    pub zorder_columns: &'static [&'static str],
    pub durable_mutation: DurableMutationClass,
    pub overlay_mutation: OverlayMutationPolicy,
    pub materialization_role: MaterializationRole,
    pub dependencies: &'static [i16],
    pub required_for_publication: bool,
}

impl TableSpec {
    /// Construct the DataFusion planning schema without changing field identity.
    ///
    /// # Errors
    ///
    /// Returns DataFusion's schema validation error for an invalid Arrow schema.
    pub fn datafusion_schema(
        &self,
    ) -> Result<datafusion::common::DFSchema, datafusion::common::DataFusionError> {
        datafusion::common::DFSchema::try_from(self.arrow_schema.clone())
    }

    /// Validate that this Arrow contract has an exact Delta Kernel mapping.
    ///
    /// # Errors
    ///
    /// Returns Arrow's conversion error when a physical type is not Delta-compatible.
    pub fn validate_delta_compatibility(&self) -> Result<(), arrow_schema::ArrowError> {
        crate::fabric::validate_delta_schema(self.arrow_schema.clone())
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)] // The generated IR owns the full reusable §7 logical-type vocabulary.
enum LogicalType {
    Id16,
    Hash32,
    Code16,
    Code32,
    Bucket16,
    Int16,
    Int32,
    Int64,
    Float64,
    Boolean,
    Utf8,
    Binary,
    TimestampUtc,
    IdList,
    StringMap,
}

#[derive(Clone, Copy)]
struct GeneratedColumn {
    name: &'static str,
    logical_type: LogicalType,
    nullable: bool,
    semantic_type: Option<&'static str>,
    foreign_key: Option<&'static str>,
    hidden_operational: bool,
}

#[derive(Clone, Copy)]
struct GeneratedTableSpec {
    table_code: i16,
    name: &'static str,
    family: &'static str,
    grain: &'static str,
    schema_version: &'static str,
    columns: &'static [GeneratedColumn],
    primary_key: &'static [&'static str],
    partition_columns: &'static [&'static str],
    zorder_columns: &'static [&'static str],
    durable_mutation: DurableMutationClass,
    overlay_mutation: OverlayMutationPolicy,
    materialization_role: MaterializationRole,
    dependencies: &'static [i16],
    required_for_publication: bool,
}

include!("generated/table_specs.rs");

fn physical_type(logical: LogicalType) -> DataType {
    match logical {
        LogicalType::Id16 | LogicalType::Hash32 | LogicalType::Binary => DataType::Binary,
        LogicalType::Code16 | LogicalType::Bucket16 | LogicalType::Int16 => DataType::Int16,
        LogicalType::Code32 | LogicalType::Int32 => DataType::Int32,
        LogicalType::Int64 => DataType::Int64,
        LogicalType::Float64 => DataType::Float64,
        LogicalType::Boolean => DataType::Boolean,
        LogicalType::Utf8 => DataType::Utf8,
        LogicalType::TimestampUtc => {
            DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC")))
        }
        LogicalType::IdList => {
            DataType::List(Arc::new(Field::new("item", DataType::Binary, false)))
        }
        LogicalType::StringMap => DataType::Map(
            Arc::new(Field::new(
                "entries",
                DataType::Struct(Fields::from(vec![
                    Field::new("key", DataType::Utf8, false),
                    Field::new("value", DataType::Utf8, false),
                ])),
                false,
            )),
            false,
        ),
    }
}

fn field(contract: GeneratedColumn, primary_key: &[&str]) -> Field {
    let mut metadata = HashMap::new();
    if matches!(contract.logical_type, LogicalType::Id16) {
        metadata.insert("com.codefabric.cpg.id_width".to_owned(), "16".to_owned());
    }
    if let Some(value) = contract.semantic_type {
        metadata.insert(
            "com.codefabric.cpg.semantic_type".to_owned(),
            value.to_owned(),
        );
    }
    if let Some(value) = contract.foreign_key {
        metadata.insert(
            "com.codefabric.cpg.foreign_key".to_owned(),
            value.to_owned(),
        );
    }
    if primary_key.contains(&contract.name) {
        metadata.insert(
            "com.codefabric.cpg.primary_key_part".to_owned(),
            "true".to_owned(),
        );
    }
    if contract.hidden_operational {
        metadata.insert(
            "com.codefabric.cpg.hidden_operational".to_owned(),
            "true".to_owned(),
        );
    }
    Field::new(
        contract.name,
        physical_type(contract.logical_type),
        contract.nullable,
    )
    .with_metadata(metadata)
}

const fn durable_name(value: DurableMutationClass) -> &'static str {
    match value {
        DurableMutationClass::StaticDimension => "STATIC_DIMENSION",
        DurableMutationClass::CurrentSingleton => "CURRENT_SINGLETON",
        DurableMutationClass::OwnerReplacedFact => "OWNER_REPLACED_FACT",
        DurableMutationClass::PublicationAppend => "PUBLICATION_APPEND",
        DurableMutationClass::DerivedOwnerReplaced => "DERIVED_OWNER_REPLACED",
        DurableMutationClass::GlobalDerivedReplacement => "GLOBAL_DERIVED_REPLACEMENT",
    }
}

const fn overlay_name(value: OverlayMutationPolicy) -> &'static str {
    match value {
        OverlayMutationPolicy::OwnerReplace => "OWNER_REPLACE",
        OverlayMutationPolicy::PrimaryKeyUpsert => "PRIMARY_KEY_UPSERT",
        OverlayMutationPolicy::FullTableReplace => "FULL_TABLE_REPLACE",
        OverlayMutationPolicy::BaseImmutable => "BASE_IMMUTABLE",
        OverlayMutationPolicy::NotApplicable => "NOT_APPLICABLE",
    }
}

const fn materialization_name(value: MaterializationRole) -> &'static str {
    match value {
        MaterializationRole::DurableEffective => "DURABLE_EFFECTIVE",
        MaterializationRole::BundleDimension => "BUNDLE_DIMENSION",
        MaterializationRole::QueryTimeDerived => "QUERY_TIME_DERIVED",
        MaterializationRole::OperationalProjection => "OPERATIONAL_PROJECTION",
    }
}

fn build(contract: GeneratedTableSpec) -> TableSpec {
    let metadata = HashMap::from([
        (
            "com.codefabric.cpg.table_name".to_owned(),
            contract.name.to_owned(),
        ),
        (
            "com.codefabric.cpg.table_family".to_owned(),
            contract.family.to_owned(),
        ),
        (
            "com.codefabric.cpg.table_grain".to_owned(),
            contract.grain.to_owned(),
        ),
        (
            "com.codefabric.cpg.schema_version".to_owned(),
            contract.schema_version.to_owned(),
        ),
        (
            "com.codefabric.cpg.ontology_version".to_owned(),
            "1.3".to_owned(),
        ),
        (
            "com.codefabric.cpg.primary_key".to_owned(),
            contract.primary_key.join(","),
        ),
        (
            "com.codefabric.cpg.partition_columns".to_owned(),
            contract.partition_columns.join(","),
        ),
        (
            "com.codefabric.cpg.durable_mutation_class".to_owned(),
            durable_name(contract.durable_mutation).to_owned(),
        ),
        (
            "com.codefabric.cpg.overlay_mutation_policy".to_owned(),
            overlay_name(contract.overlay_mutation).to_owned(),
        ),
        (
            "com.codefabric.cpg.materialization_role".to_owned(),
            materialization_name(contract.materialization_role).to_owned(),
        ),
        (
            "com.codefabric.cpg.compatibility_mode".to_owned(),
            "suite-major-1".to_owned(),
        ),
    ]);
    let fields = contract
        .columns
        .iter()
        .copied()
        .map(|column| field(column, contract.primary_key))
        .collect::<Vec<_>>();
    TableSpec {
        table_code: contract.table_code,
        name: contract.name,
        schema_version: contract.schema_version,
        arrow_schema: Arc::new(Schema::new_with_metadata(fields, metadata)),
        primary_key: contract.primary_key,
        partition_columns: contract.partition_columns,
        zorder_columns: contract.zorder_columns,
        durable_mutation: contract.durable_mutation,
        overlay_mutation: contract.overlay_mutation,
        materialization_role: contract.materialization_role,
        dependencies: contract.dependencies,
        required_for_publication: contract.required_for_publication,
    }
}

/// Return the process-wide immutable `TableSpec` set.
#[must_use]
pub fn table_specs() -> &'static [TableSpec] {
    static TABLE_SPECS: OnceLock<Vec<TableSpec>> = OnceLock::new();
    TABLE_SPECS.get_or_init(|| GENERATED_TABLE_SPECS.iter().copied().map(build).collect())
}

/// Resolve a table by stable numeric code.
#[must_use]
pub fn table_spec(table_code: i16) -> Option<&'static TableSpec> {
    table_specs()
        .iter()
        .find(|table| table.table_code == table_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wp09_structural_acceptance() {
        let tables = table_specs();
        assert_eq!(tables.len(), 17);
        let entity = table_spec(100).unwrap();
        assert_eq!(
            entity.partition_columns,
            ["entity_family_code", "owner_bucket"]
        );
        assert_eq!(
            entity
                .arrow_schema
                .field_with_name("entity_id")
                .unwrap()
                .data_type(),
            &DataType::Binary
        );
        assert_eq!(entity.arrow_schema.metadata().len(), 11);
        assert_eq!(
            entity
                .arrow_schema
                .metadata()
                .get("com.codefabric.cpg.durable_mutation_class")
                .map(String::as_str),
            Some("OWNER_REPLACED_FACT")
        );
        assert!(tables.iter().all(|table| {
            !table
                .arrow_schema
                .fields()
                .iter()
                .any(|field| matches!(field.data_type(), DataType::Utf8View))
        }));
        assert!(
            tables
                .iter()
                .filter(|table| table.materialization_role
                    == MaterializationRole::OperationalProjection)
                .all(|table| table.name.ends_with("tombstone"))
        );
        assert!(
            tables
                .iter()
                .filter(|table| table.materialization_role == MaterializationRole::DurableEffective)
                .all(|table| table.name != "owner_tombstone"
                    && table.name != "primary_key_tombstone")
        );
    }

    #[test]
    fn wp09_behavioral_acceptance() {
        let property = table_spec(120).unwrap();
        let value_columns = [
            "value_entity_id",
            "value_bool",
            "value_int64",
            "value_float64",
            "value_text",
            "value_bytes",
            "value_type_id",
        ];
        assert!(
            value_columns
                .iter()
                .all(|name| property.arrow_schema.field_with_name(name).is_ok())
        );
        let workspace = table_spec(1).unwrap();
        assert!(
            workspace
                .arrow_schema
                .field_with_name("registration_revision")
                .is_ok()
        );
        assert!(workspace.arrow_schema.field_with_name("updated_at").is_ok());
        for table in table_specs() {
            let batch = arrow::record_batch::RecordBatch::new_empty(table.arrow_schema.clone());
            assert_eq!(batch.num_columns(), table.arrow_schema.fields().len());
            assert_eq!(batch.num_rows(), 0);
            let datafusion = table.datafusion_schema().unwrap();
            assert_eq!(datafusion.fields().len(), table.arrow_schema.fields().len());
            table.validate_delta_compatibility().unwrap();
        }
    }

    #[cfg(feature = "repository-state")]
    #[test]
    fn wp09_operational_store_ddl_executes() {
        let ddl = include_str!("../contracts/schema/operational-store.sql");
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch(ddl).unwrap();
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let generated_table_count = ddl
            .lines()
            .filter(|line| line.starts_with("CREATE TABLE "))
            .count();
        assert_eq!(table_count, i64::try_from(generated_table_count).unwrap());
        let view_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'view' AND name = 'workspace_update_state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(view_count, 1);
    }
}
