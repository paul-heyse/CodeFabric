//! Independent Arrow/DataFusion consumer for staged model schema projections.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BinaryArray, BooleanArray, FixedSizeBinaryBuilder, Float64Array, Int16Array,
    Int32Array, Int64Array, Int64Builder, ListBuilder, MapBuilder, MapFieldNames, StringArray,
    StringBuilder, TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Fields, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use arrow_schema::extension::{EXTENSION_TYPE_METADATA_KEY, EXTENSION_TYPE_NAME_KEY};
use datafusion::datasource::MemTable;
use datafusion::prelude::SessionContext;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ModelLogicalType {
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelColumn {
    field_id: String,
    name: String,
    logical_type: ModelLogicalType,
    arrow_type: serde_json::Value,
    nullable: bool,
    semantic_type: Option<String>,
    foreign_key: Option<String>,
    hidden_operational: bool,
    key_role: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelTable {
    table_id: String,
    table_code: i16,
    name: String,
    family: String,
    grain: String,
    schema_version: String,
    columns: Vec<ModelColumn>,
    primary_key: Vec<String>,
    partition_columns: Vec<String>,
    zorder_columns: Vec<String>,
    durable_mutation: String,
    overlay_mutation: String,
    materialization_role: String,
    publication_pin_role: String,
    dependencies: Vec<i16>,
    required_for_publication: bool,
    #[serde(rename = "row_encoder")]
    _row_encoder: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableManifest {
    model_version: u16,
    source: serde_json::Value,
    ontology_version: String,
    compatibility_mode: String,
    metadata_dictionary: Vec<serde_json::Value>,
    semantic_authorities: Vec<serde_json::Value>,
    semantic_type_bindings: Vec<serde_json::Value>,
    schema_evolution_policy: serde_json::Value,
    sqlite_foreign_key_posture: serde_json::Value,
    owner_bucket_count: u16,
    tables: Vec<ModelTable>,
    table_scopes: Vec<serde_json::Value>,
    operational_tables: Vec<serde_json::Value>,
    serving_projections: Vec<serde_json::Value>,
    control_projections: Vec<serde_json::Value>,
    serving_resource_profile: serde_json::Value,
    public_schema_instances: String,
    public_schemas: Vec<serde_json::Value>,
}

fn physical_type(logical: ModelLogicalType) -> DataType {
    match logical {
        ModelLogicalType::Id16 => DataType::FixedSizeBinary(16),
        ModelLogicalType::Hash32 | ModelLogicalType::Binary => DataType::Binary,
        ModelLogicalType::Code16 | ModelLogicalType::Bucket16 | ModelLogicalType::Int16 => {
            DataType::Int16
        }
        ModelLogicalType::Code32 | ModelLogicalType::Int32 => DataType::Int32,
        ModelLogicalType::Int64 => DataType::Int64,
        ModelLogicalType::Float64 => DataType::Float64,
        ModelLogicalType::Boolean => DataType::Boolean,
        ModelLogicalType::Utf8 => DataType::Utf8,
        ModelLogicalType::TimestampUtc => {
            DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC")))
        }
        ModelLogicalType::IdList => DataType::List(Arc::new(id16_field("element", false))),
        ModelLogicalType::Int64List => {
            DataType::List(Arc::new(Field::new("element", DataType::Int64, false)))
        }
        ModelLogicalType::StringMap => DataType::Map(
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

fn id16_field(name: &str, nullable: bool) -> Field {
    Field::new(name, DataType::FixedSizeBinary(16), nullable).with_metadata(HashMap::from([
        (
            EXTENSION_TYPE_NAME_KEY.to_owned(),
            "codefabric.id16".to_owned(),
        ),
        (
            EXTENSION_TYPE_METADATA_KEY.to_owned(),
            "version=1".to_owned(),
        ),
    ]))
}

fn sample_id16(value: &[u8; 16]) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::with_capacity(1, 16);
    builder.append_value(value).expect("typed Id16 width");
    Arc::new(builder.finish())
}

fn sample_array(logical: ModelLogicalType) -> ArrayRef {
    const ID: [u8; 16] = [7; 16];
    const HASH: [u8; 32] = [9; 32];
    match logical {
        ModelLogicalType::Id16 => sample_id16(&ID),
        ModelLogicalType::Hash32 => Arc::new(BinaryArray::from(vec![Some(HASH.as_slice())])),
        ModelLogicalType::Binary => Arc::new(BinaryArray::from(vec![Some(b"bytes".as_slice())])),
        ModelLogicalType::Code16 | ModelLogicalType::Bucket16 | ModelLogicalType::Int16 => {
            Arc::new(Int16Array::from(vec![1]))
        }
        ModelLogicalType::Code32 | ModelLogicalType::Int32 => Arc::new(Int32Array::from(vec![2])),
        ModelLogicalType::Int64 => Arc::new(Int64Array::from(vec![3])),
        ModelLogicalType::Float64 => Arc::new(Float64Array::from(vec![4.5])),
        ModelLogicalType::Boolean => Arc::new(BooleanArray::from(vec![true])),
        ModelLogicalType::Utf8 => Arc::new(StringArray::from(vec!["value"])),
        ModelLogicalType::TimestampUtc => {
            Arc::new(TimestampMicrosecondArray::from(vec![1_i64]).with_timezone("UTC"))
        }
        ModelLogicalType::IdList => {
            let mut builder = ListBuilder::new(FixedSizeBinaryBuilder::new(16))
                .with_field(Arc::new(id16_field("element", false)));
            builder.values().append_value(ID).expect("typed Id16 width");
            builder.append(true);
            Arc::new(builder.finish())
        }
        ModelLogicalType::Int64List => {
            let mut builder = ListBuilder::new(Int64Builder::new())
                .with_field(Arc::new(Field::new("element", DataType::Int64, false)));
            builder.values().append_value(5);
            builder.append(true);
            Arc::new(builder.finish())
        }
        ModelLogicalType::StringMap => {
            let mut builder = MapBuilder::new(
                Some(MapFieldNames {
                    entry: "entries".to_owned(),
                    key: "key".to_owned(),
                    value: "value".to_owned(),
                }),
                StringBuilder::new(),
                StringBuilder::new(),
            )
            .with_keys_field(Field::new("key", DataType::Utf8, false))
            .with_values_field(Field::new("value", DataType::Utf8, false));
            builder.keys().append_value("key");
            builder.values().append_value("value");
            builder.append(true).expect("one valid map row");
            Arc::new(builder.finish())
        }
    }
}

#[allow(clippy::too_many_lines)] // Keep the independent end-to-end consumer in one audit surface.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage_root = std::env::args()
        .nth(1)
        .ok_or("usage: codefabric-model-schema-consumer <stage-root>")?;
    let manifest: TableManifest = serde_json::from_slice(&std::fs::read(
        std::path::Path::new(&stage_root).join("contracts/generated/model/schema/table-specs.json"),
    )?)?;
    if manifest.model_version != 1
        || manifest.owner_bucket_count != 256
        || manifest.source.as_object().is_none()
        || manifest.ontology_version.is_empty()
        || manifest.compatibility_mode.is_empty()
        || manifest.metadata_dictionary.is_empty()
        || manifest.semantic_authorities.is_empty()
        || manifest.semantic_type_bindings.is_empty()
        || manifest.schema_evolution_policy.as_object().is_none()
        || manifest.sqlite_foreign_key_posture.is_null()
        || manifest.table_scopes.is_empty()
        || manifest.operational_tables.is_empty()
        || manifest.serving_projections.is_empty()
        || manifest.control_projections.is_empty()
        || manifest.serving_resource_profile.as_object().is_none()
        || manifest.public_schema_instances
            != "contracts/generated/model/schema/public-schema-golden-instances.json"
        || manifest.public_schemas.len() != 8
    {
        return Err("staged TableSpec manifest is incomplete".into());
    }
    let context = SessionContext::new();
    for table in &manifest.tables {
        if table.table_code <= 0
            || table.family.is_empty()
            || table.grain.is_empty()
            || table.schema_version.is_empty()
            || table.partition_columns.iter().any(String::is_empty)
            || table.zorder_columns.iter().any(String::is_empty)
            || table.durable_mutation.is_empty()
            || table.overlay_mutation.is_empty()
            || table.materialization_role.is_empty()
            || table.publication_pin_role.is_empty()
            || table.dependencies.contains(&table.table_code)
            || (!table.required_for_publication && table.publication_pin_role != "NOT_PUBLISHED")
        {
            return Err(format!("{}: incomplete table projection", table.name).into());
        }
        let fields = table
            .columns
            .iter()
            .map(|column| {
                let mut metadata = HashMap::from([(
                    "com.codefabric.cpg.field_id".to_owned(),
                    column.field_id.clone(),
                )]);
                if let Some(semantic_type) = &column.semantic_type {
                    metadata.insert(
                        "com.codefabric.cpg.semantic_type".to_owned(),
                        semantic_type.to_owned(),
                    );
                }
                if let Some(foreign_key) = &column.foreign_key {
                    metadata.insert(
                        "com.codefabric.cpg.foreign_key".to_owned(),
                        foreign_key.to_owned(),
                    );
                }
                let mut field = Field::new(
                    &column.name,
                    physical_type(column.logical_type),
                    column.nullable,
                )
                .with_metadata(metadata);
                if column.logical_type == ModelLogicalType::Id16 {
                    let mut extension_metadata = field.metadata().clone();
                    extension_metadata.insert(
                        EXTENSION_TYPE_NAME_KEY.to_owned(),
                        "codefabric.id16".to_owned(),
                    );
                    extension_metadata.insert(
                        EXTENSION_TYPE_METADATA_KEY.to_owned(),
                        "version=1".to_owned(),
                    );
                    field = field.with_metadata(extension_metadata);
                }
                field
            })
            .collect::<Vec<_>>();
        let schema = Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([
                (
                    "com.codefabric.cpg.table_id".to_owned(),
                    table.table_id.clone(),
                ),
                (
                    "com.codefabric.cpg.primary_key".to_owned(),
                    table.primary_key.join(","),
                ),
            ]),
        ));
        let datafusion_schema = datafusion::common::DFSchema::try_from(Arc::clone(&schema))?;
        if datafusion_schema.fields().len() != table.columns.len() {
            return Err(format!("{}: DataFusion field census differs", table.name).into());
        }
        let arrays = table
            .columns
            .iter()
            .map(|column| sample_array(column.logical_type))
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(Arc::clone(&schema), arrays)?;
        for (index, column) in table.columns.iter().enumerate() {
            let field = schema.field(index);
            if field.name() != &column.name
                || field.is_nullable() != column.nullable
                || column.arrow_type.as_object().is_none()
                || (column.hidden_operational && column.key_role == "primary")
                || batch.column(index).data_type() != field.data_type()
                || batch.column(index).len() != 1
                || batch.column(index).null_count() != 0
            {
                return Err(
                    format!("{}: row round-trip failed at {}", table.name, column.name).into(),
                );
            }
            let rebuilt = arrow::array::make_array(batch.column(index).to_data());
            if rebuilt.as_ref() != batch.column(index).as_ref() {
                return Err(format!(
                    "{}: Arrow data round-trip failed at {}",
                    table.name, column.name
                )
                .into());
            }
        }
        if RecordBatch::try_new(
            Arc::clone(&schema),
            batch
                .columns()
                .iter()
                .cloned()
                .chain(std::iter::once(sample_array(ModelLogicalType::Boolean)))
                .collect(),
        )
        .is_ok()
        {
            return Err(format!("{}: unknown extra column was accepted", table.name).into());
        }
        let provider = MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?;
        context.register_table(table.name.as_str(), Arc::new(provider))?;
    }

    for logical_type in [
        ModelLogicalType::Id16,
        ModelLogicalType::Hash32,
        ModelLogicalType::Code16,
        ModelLogicalType::Code32,
        ModelLogicalType::Bucket16,
        ModelLogicalType::Int16,
        ModelLogicalType::Int32,
        ModelLogicalType::Int64,
        ModelLogicalType::Float64,
        ModelLogicalType::Boolean,
        ModelLogicalType::Utf8,
        ModelLogicalType::Binary,
        ModelLogicalType::TimestampUtc,
        ModelLogicalType::IdList,
        ModelLogicalType::Int64List,
        ModelLogicalType::StringMap,
    ] {
        let array = sample_array(logical_type);
        let expected = physical_type(logical_type);
        if array.data_type() != &expected || array.len() != 1 {
            return Err(format!(
                "logical type {logical_type:?} did not round-trip: expected={expected:?} actual={:?}",
                array.data_type()
            )
            .into());
        }
    }
    println!(
        "validated {} model-derived TableSpecs",
        manifest.tables.len()
    );
    Ok(())
}
