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

/// Closed role in the acyclic durable-publication manifest graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPinRole {
    PinnedData,
    ManifestControl,
    PointerControl,
    NotPublished,
}

/// One immutable generated schema contract.
#[derive(Clone, Debug)]
pub struct TableSpec {
    pub table_code: i16,
    pub name: &'static str,
    pub family: &'static str,
    pub grain: &'static str,
    pub schema_version: &'static str,
    pub schema_digest: String,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub partition_columns: &'static [&'static str],
    pub zorder_columns: &'static [&'static str],
    pub durable_mutation: DurableMutationClass,
    pub overlay_mutation: OverlayMutationPolicy,
    pub materialization_role: MaterializationRole,
    pub publication_pin_role: PublicationPinRole,
    pub dependencies: &'static [i16],
    pub required_for_publication: bool,
}

/// Generated row-scope selectors applied at publication and provider construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableScopeSpec {
    pub table_code: i16,
    pub workspace_column: Option<&'static str>,
    pub analysis_context_column: Option<&'static str>,
    pub source_generation_column: Option<&'static str>,
    pub analysis_context_set_column: Option<&'static str>,
    pub owner_column: Option<&'static str>,
}

/// Closed role of one generated serving projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServingProjectionRole {
    EffectiveFact,
}

/// One generated `cpg_serving` projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingProjectionSpec {
    pub view_name: &'static str,
    pub source_table_code: i16,
    pub availability_wave: u16,
    pub projection_role: ServingProjectionRole,
}

/// Closed implementation role for one generated control projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlProjectionRole {
    OperationalSource,
    DerivedOperational,
    ActiveServingSnapshot,
}

/// One generated `cpg_control` projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlProjectionSpec {
    pub view_name: &'static str,
    pub availability_wave: u16,
    pub projection_role: ControlProjectionRole,
    pub source_table: Option<&'static str>,
    pub columns: &'static [&'static str],
}

/// Generated non-timing resource limits for serving and candidate construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingResourceProfile {
    pub batch_size: usize,
    pub max_output_rows: usize,
    pub max_output_bytes: usize,
    pub max_output_batches: usize,
    pub max_control_rows: usize,
    pub max_control_bytes: usize,
    pub max_control_batches: usize,
    pub max_snapshot_validation_rows: usize,
    pub max_snapshot_validation_bytes: usize,
    pub max_snapshot_validation_batches: usize,
}

/// `SQLite` affinity mapped to one query-visible Arrow physical type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalSqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

/// One generated read-only operational projection schema.
#[derive(Clone, Debug)]
pub struct OperationalTableSpec {
    pub name: &'static str,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub workspace_scope: Option<OperationalWorkspaceScope>,
}

/// Generated route from an operational row to its owning workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalWorkspaceScope {
    Direct {
        workspace_column: &'static str,
    },
    ViaParent {
        parent_table: &'static str,
        child_column: &'static str,
        parent_column: &'static str,
        workspace_column: &'static str,
    },
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
        crate::fabric::validate_delta_schema(&self.arrow_schema)
    }
}

fn schema_digest(schema: &SchemaRef) -> String {
    crate::fabric::delta_schema_digest(schema)
        .expect("generated TableSpec must have a canonical Delta schema identity")
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
    Int64List,
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
#[allow(dead_code)] // Legacy metadata remains only until the model driver projects every role.
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
    publication_pin_role: PublicationPinRole,
    dependencies: &'static [i16],
    required_for_publication: bool,
}

#[derive(Clone, Copy)]
struct GeneratedOperationalColumn {
    name: &'static str,
    sqlite_type: OperationalSqliteType,
    nullable: bool,
}

#[derive(Clone, Copy)]
struct GeneratedOperationalTableSpec {
    name: &'static str,
    columns: &'static [GeneratedOperationalColumn],
    primary_key: &'static [&'static str],
    workspace_scope: Option<OperationalWorkspaceScope>,
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
            // Delta's Arrow conversion canonicalizes list children to `element`.
            // Emit that library-native name so the generated schema round-trips exactly.
            DataType::List(Arc::new(Field::new("element", DataType::Binary, false)))
        }
        LogicalType::Int64List => {
            DataType::List(Arc::new(Field::new("element", DataType::Int64, false)))
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

fn model_logical_type(
    logical: crate::model_generated::schema_tables::ModelLogicalType,
) -> LogicalType {
    use crate::model_generated::schema_tables::ModelLogicalType as Model;
    match logical {
        Model::Id16 => LogicalType::Id16,
        Model::Hash32 => LogicalType::Hash32,
        Model::Code16 => LogicalType::Code16,
        Model::Code32 => LogicalType::Code32,
        Model::Bucket16 => LogicalType::Bucket16,
        Model::Int16 => LogicalType::Int16,
        Model::Int32 => LogicalType::Int32,
        Model::Int64 => LogicalType::Int64,
        Model::Float64 => LogicalType::Float64,
        Model::Boolean => LogicalType::Boolean,
        Model::Utf8 => LogicalType::Utf8,
        Model::Binary => LogicalType::Binary,
        Model::TimestampUtc => LogicalType::TimestampUtc,
        Model::IdList => LogicalType::IdList,
        Model::Int64List => LogicalType::Int64List,
        Model::StringMap => LogicalType::StringMap,
    }
}

fn model_field(
    column: crate::model_generated::schema_tables::ModelColumn,
    primary_key: &[&str],
    legacy: GeneratedTableSpec,
) -> Field {
    let hidden_operational = legacy
        .columns
        .iter()
        .find(|candidate| candidate.name == column.name)
        .is_some_and(|candidate| candidate.hidden_operational);
    field(
        GeneratedColumn {
            name: column.name,
            logical_type: model_logical_type(column.logical_type),
            nullable: column.nullable,
            semantic_type: column.semantic_type,
            foreign_key: column.foreign_key,
            hidden_operational,
        },
        primary_key,
    )
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

const fn publication_pin_name(value: PublicationPinRole) -> &'static str {
    match value {
        PublicationPinRole::PinnedData => "PINNED_DATA",
        PublicationPinRole::ManifestControl => "MANIFEST_CONTROL",
        PublicationPinRole::PointerControl => "POINTER_CONTROL",
        PublicationPinRole::NotPublished => "NOT_PUBLISHED",
    }
}

fn build(contract: GeneratedTableSpec) -> TableSpec {
    let model = crate::model_generated::schema_tables::MODEL_TABLES
        .iter()
        .find(|candidate| candidate.table_code == contract.table_code)
        .expect("model-generated table is complete for every runtime table");
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
            model.primary_key.join(","),
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
            "com.codefabric.cpg.publication_pin_role".to_owned(),
            publication_pin_name(contract.publication_pin_role).to_owned(),
        ),
        (
            "com.codefabric.cpg.compatibility_mode".to_owned(),
            "suite-major-1".to_owned(),
        ),
    ]);
    let fields = model
        .columns
        .iter()
        .copied()
        .map(|column| model_field(column, model.primary_key, contract))
        .collect::<Vec<_>>();
    let arrow_schema = Arc::new(Schema::new_with_metadata(fields, metadata));
    TableSpec {
        table_code: contract.table_code,
        name: contract.name,
        family: contract.family,
        grain: contract.grain,
        schema_version: contract.schema_version,
        schema_digest: schema_digest(&arrow_schema),
        arrow_schema,
        primary_key: model.primary_key,
        partition_columns: contract.partition_columns,
        zorder_columns: contract.zorder_columns,
        durable_mutation: contract.durable_mutation,
        overlay_mutation: contract.overlay_mutation,
        materialization_role: contract.materialization_role,
        publication_pin_role: contract.publication_pin_role,
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

/// Resolve generated row-scope selectors by table code.
#[must_use]
pub fn table_scope_spec(table_code: i16) -> Option<&'static TableScopeSpec> {
    GENERATED_TABLE_SCOPE_SPECS
        .iter()
        .find(|scope| scope.table_code == table_code)
}

/// Return every generated Wave-owned serving projection.
#[must_use]
pub const fn serving_projection_specs() -> &'static [ServingProjectionSpec] {
    GENERATED_SERVING_PROJECTION_SPECS
}

/// Return every generated control projection.
#[must_use]
pub const fn control_projection_specs() -> &'static [ControlProjectionSpec] {
    GENERATED_CONTROL_PROJECTION_SPECS
}

/// Return the generated serving resource profile.
#[must_use]
pub const fn serving_resource_profile() -> ServingResourceProfile {
    GENERATED_SERVING_RESOURCE_PROFILE
}

fn build_operational(contract: GeneratedOperationalTableSpec) -> OperationalTableSpec {
    let fields = contract
        .columns
        .iter()
        .map(|column| {
            let data_type = match column.sqlite_type {
                OperationalSqliteType::Integer => DataType::Int64,
                OperationalSqliteType::Real => DataType::Float64,
                OperationalSqliteType::Text => DataType::Utf8,
                OperationalSqliteType::Blob => DataType::Binary,
            };
            Field::new(column.name, data_type, column.nullable)
        })
        .collect::<Vec<_>>();
    OperationalTableSpec {
        name: contract.name,
        arrow_schema: Arc::new(Schema::new(fields)),
        primary_key: contract.primary_key,
        workspace_scope: contract.workspace_scope,
    }
}

/// Return every generated operational-store projection in source order.
#[must_use]
pub fn operational_table_specs() -> &'static [OperationalTableSpec] {
    static OPERATIONAL_SPECS: OnceLock<Vec<OperationalTableSpec>> = OnceLock::new();
    OPERATIONAL_SPECS.get_or_init(|| {
        GENERATED_OPERATIONAL_TABLE_SPECS
            .iter()
            .copied()
            .map(build_operational)
            .collect()
    })
}

/// Resolve one generated operational-store projection by table name.
#[must_use]
pub fn operational_table_spec(name: &str) -> Option<&'static OperationalTableSpec> {
    operational_table_specs()
        .iter()
        .find(|table| table.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_wave4_batch(table: &TableSpec) -> arrow::record_batch::RecordBatch {
        use arrow::array::{
            ArrayRef, BinaryArray, BooleanArray, Int16Array, Int32Array, Int64Array, Int64Builder,
            ListBuilder, StringArray,
        };

        const BYTES: [u8; 32] = [7; 32];
        let columns = table
            .arrow_schema
            .fields()
            .iter()
            .map(|field| -> ArrayRef {
                match field.data_type() {
                    DataType::Binary => Arc::new(BinaryArray::from(vec![Some(BYTES.as_slice())])),
                    DataType::Int16 => Arc::new(Int16Array::from(vec![0])),
                    DataType::Int32 => Arc::new(Int32Array::from(vec![0])),
                    DataType::Int64 => Arc::new(Int64Array::from(vec![0])),
                    DataType::Boolean => Arc::new(BooleanArray::from(vec![false])),
                    DataType::Utf8 => Arc::new(StringArray::from(vec!["value"])),
                    DataType::List(element) if element.data_type() == &DataType::Int64 => {
                        let mut values =
                            ListBuilder::new(Int64Builder::new()).with_field(Arc::clone(element));
                        values.values().append_value(0);
                        values.append(true);
                        Arc::new(values.finish())
                    }
                    other => panic!("unhandled Wave-4 synthetic type {other:?}"),
                }
            })
            .collect();
        arrow::record_batch::RecordBatch::try_new(table.arrow_schema.clone(), columns).unwrap()
    }

    #[test]
    fn wp09_structural_acceptance() {
        let tables = table_specs();
        assert!(tables.len() >= 17);
        for code in [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 100, 110, 120, 130, 900, 901,
        ] {
            assert!(table_spec(code).is_some(), "base table code {code} drifted");
        }
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
        assert_eq!(entity.arrow_schema.metadata().len(), 12);
        assert_eq!(
            entity
                .arrow_schema
                .metadata()
                .get("com.codefabric.cpg.durable_mutation_class")
                .map(String::as_str),
            Some("OWNER_REPLACED_FACT")
        );
        assert_eq!(
            entity
                .arrow_schema
                .metadata()
                .get("com.codefabric.cpg.publication_pin_role")
                .map(String::as_str),
            Some("PINNED_DATA")
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

    #[test]
    fn wp28_behavioral_acceptance() {
        for code in [140, 150, 160, 170] {
            let table = table_spec(code).unwrap();
            let batch = synthetic_wave4_batch(table);
            assert_eq!(batch.num_rows(), 1);
            assert_eq!(batch.schema().fields(), table.arrow_schema.fields());
            table.validate_delta_compatibility().unwrap();
            assert_eq!(
                table.datafusion_schema().unwrap().fields().len(),
                table.arrow_schema.fields().len()
            );
            datafusion::datasource::MemTable::try_new(
                table.arrow_schema.clone(),
                vec![vec![batch]],
            )
            .unwrap();
        }
        assert_eq!(
            table_spec(140)
                .unwrap()
                .arrow_schema
                .field_with_name("line_start_offsets")
                .unwrap()
                .data_type(),
            &DataType::List(Arc::new(Field::new("element", DataType::Int64, false)))
        );
    }

    #[test]
    fn wp28_structural_acceptance() {
        for (code, name, primary_key) in [
            (140, "source_file", &["workspace_id", "file_id"][..]),
            (150, "source_token", &["token_id"][..]),
            (160, "source_annotation", &["annotation_id"][..]),
            (170, "syntax_detail", &["entity_id"][..]),
        ] {
            let table = table_spec(code).unwrap();
            assert_eq!(table.name, name);
            assert_eq!(table.primary_key, primary_key);
            assert_eq!(table.partition_columns, ["owner_bucket"]);
            assert_eq!(
                table.durable_mutation,
                DurableMutationClass::OwnerReplacedFact
            );
            assert_eq!(table.overlay_mutation, OverlayMutationPolicy::OwnerReplace);
            assert_eq!(
                table.materialization_role,
                MaterializationRole::DurableEffective
            );
            assert_eq!(table.publication_pin_role, PublicationPinRole::PinnedData);
        }
    }

    #[test]
    fn wp28_negative_zero_state() {
        let views = serving_projection_specs();
        assert!(
            views
                .iter()
                .any(|view| view.view_name == "files" && view.source_table_code == 140)
        );
        assert!(
            views
                .iter()
                .any(|view| view.view_name == "syntax" && view.source_table_code == 170)
        );
        assert!(!views.iter().any(|view| matches!(
            view.view_name,
            "tokens" | "annotations" | "python" | "rust" | "derived"
        )));
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
