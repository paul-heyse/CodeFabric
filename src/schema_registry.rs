//! Application-owned Arrow logical extensions and live operational-store schemas.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use arrow_schema::extension::ExtensionType;
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::common::types::DFExtensionType;
use datafusion::logical_expr::registry::{ExtensionTypeRegistration, ExtensionTypeRegistrationRef};

/// Descriptor for one application-owned ID logical type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdDomainSpec {
    pub domain_slug: &'static str,
    pub extension_name: &'static str,
    pub rust_type: &'static str,
    pub preimage_recipe_id: &'static str,
    pub preimage_version: &'static str,
}

/// Common ID-extension behavior used by DataFusion registration factories.
pub trait CodeFabricIdExtension:
    ExtensionType<Metadata = ()> + Copy + std::fmt::Debug + Send + Sync + 'static
{
    const DOMAIN_SLUG: &'static str;
    const PREIMAGE_RECIPE_ID: &'static str;
    const PREIMAGE_VERSION: &'static str;
    const METADATA_V1: &'static str;
    fn v1() -> Self;
}

macro_rules! define_id_domain_extension {
    ($type:ident, $domain:literal, $name:literal, $recipe:literal, $version:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $type {
            metadata: (),
        }

        impl $type {
            pub const NAME: &'static str = $name;
            pub const METADATA_V1: &'static str = concat!(
                "{\"domain\":\"",
                $domain,
                "\",\"preimage_recipe_id\":\"",
                $recipe,
                "\",\"preimage_version\":\"",
                $version,
                "\"}"
            );

            #[must_use]
            pub const fn v1() -> Self {
                Self { metadata: () }
            }
        }

        impl ExtensionType for $type {
            const NAME: &'static str = Self::NAME;
            type Metadata = ();

            fn metadata(&self) -> &Self::Metadata {
                &self.metadata
            }

            fn serialize_metadata(&self) -> Option<String> {
                Some(Self::METADATA_V1.to_owned())
            }

            fn deserialize_metadata(metadata: Option<&str>) -> Result<Self::Metadata, ArrowError> {
                match metadata {
                    Some(Self::METADATA_V1) => Ok(()),
                    value => Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires metadata {}, received {value:?}",
                        Self::NAME,
                        Self::METADATA_V1
                    ))),
                }
            }

            fn supports_data_type(&self, data_type: &DataType) -> Result<(), ArrowError> {
                if data_type == &DataType::FixedSizeBinary(16) {
                    Ok(())
                } else {
                    Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires FixedSizeBinary(16), received {data_type}",
                        Self::NAME
                    )))
                }
            }

            fn try_new(data_type: &DataType, metadata: Self::Metadata) -> Result<Self, ArrowError> {
                let extension = Self { metadata };
                extension.supports_data_type(data_type)?;
                Ok(extension)
            }
        }

        impl CodeFabricIdExtension for $type {
            const DOMAIN_SLUG: &'static str = $domain;
            const PREIMAGE_RECIPE_ID: &'static str = $recipe;
            const PREIMAGE_VERSION: &'static str = $version;
            const METADATA_V1: &'static str = Self::METADATA_V1;

            fn v1() -> Self {
                Self::v1()
            }
        }
    };
}

macro_rules! define_hash32_extension {
    () => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct Hash32Extension {
            metadata: (),
        }

        impl Hash32Extension {
            pub const NAME: &'static str = "codefabric.hash32";
            pub const METADATA_V1: &'static str = "{\"width\":32,\"version\":1}";

            #[must_use]
            pub const fn v1() -> Self {
                Self { metadata: () }
            }
        }

        impl ExtensionType for Hash32Extension {
            const NAME: &'static str = Self::NAME;
            type Metadata = ();

            fn metadata(&self) -> &Self::Metadata {
                &self.metadata
            }

            fn serialize_metadata(&self) -> Option<String> {
                Some(Self::METADATA_V1.to_owned())
            }

            fn deserialize_metadata(metadata: Option<&str>) -> Result<Self::Metadata, ArrowError> {
                match metadata {
                    Some(Self::METADATA_V1) => Ok(()),
                    value => Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires metadata {}, received {value:?}",
                        Self::NAME,
                        Self::METADATA_V1
                    ))),
                }
            }

            fn supports_data_type(&self, data_type: &DataType) -> Result<(), ArrowError> {
                if data_type == &DataType::FixedSizeBinary(32) {
                    Ok(())
                } else {
                    Err(ArrowError::InvalidArgumentError(format!(
                        "{} requires FixedSizeBinary(32), received {data_type}",
                        Self::NAME
                    )))
                }
            }

            fn try_new(data_type: &DataType, metadata: Self::Metadata) -> Result<Self, ArrowError> {
                let extension = Self { metadata };
                extension.supports_data_type(data_type)?;
                Ok(extension)
            }
        }
    };
}

#[derive(Debug)]
struct DataFusionCodeFabricExtension {
    storage_type: DataType,
    metadata: String,
}

impl DFExtensionType for DataFusionCodeFabricExtension {
    fn storage_type(&self) -> DataType {
        self.storage_type.clone()
    }

    fn serialize_metadata(&self) -> Option<String> {
        Some(self.metadata.clone())
    }
}

fn id_domain_registration<T: CodeFabricIdExtension>() -> ExtensionTypeRegistrationRef {
    ExtensionTypeRegistration::new_arc(T::NAME, |storage_type, metadata| {
        T::deserialize_metadata(metadata)?;
        T::try_new(storage_type, ())?;
        Ok(Arc::new(DataFusionCodeFabricExtension {
            storage_type: storage_type.clone(),
            metadata: T::METADATA_V1.to_owned(),
        }))
    })
}

fn hash32_registration() -> ExtensionTypeRegistrationRef {
    ExtensionTypeRegistration::new_arc(Hash32Extension::NAME, |storage_type, metadata| {
        Hash32Extension::deserialize_metadata(metadata)?;
        Hash32Extension::try_new(storage_type, ())?;
        Ok(Arc::new(DataFusionCodeFabricExtension {
            storage_type: storage_type.clone(),
            metadata: Hash32Extension::METADATA_V1.to_owned(),
        }))
    })
}

include!("id_domain_extensions.rs");

/// Return the application-owned logical ID-domain registry.
#[must_use]
pub const fn id_domains() -> &'static [IdDomainSpec] {
    ID_DOMAINS
}

/// Create one DataFusion registration factory for every logical ID/hash extension.
#[must_use]
pub fn extension_type_registrations() -> Vec<ExtensionTypeRegistrationRef> {
    id_domain_registrations()
}

/// Validate an application-owned ID/hash field.
///
/// # Errors
///
/// Returns an argument error for an unregistered extension name or the wrong physical width.
pub fn validate_logical_extension_field(field: &Field) -> Result<(), ArrowError> {
    let Some(name) = field.extension_type_name() else {
        return Ok(());
    };
    let expected_width = if name == Hash32Extension::NAME {
        32
    } else if ID_DOMAINS
        .iter()
        .any(|domain| domain.extension_name == name)
    {
        16
    } else {
        return Err(ArrowError::InvalidArgumentError(format!(
            "unregistered CodeFabric extension type {name}"
        )));
    };
    if field.data_type() != &DataType::FixedSizeBinary(expected_width) {
        return Err(ArrowError::InvalidArgumentError(format!(
            "{name} requires FixedSizeBinary({expected_width}), received {}",
            field.data_type()
        )));
    }
    Ok(())
}

/// SQLite affinity mapped to one query-visible Arrow physical type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalSqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

/// One immutable operational SQLite/Arrow table contract.
#[derive(Clone, Debug)]
pub struct OperationalTableSpec {
    pub name: &'static str,
    pub sqlite_ddl: &'static str,
    pub sqlite_column_types: Vec<OperationalSqliteType>,
    pub arrow_schema: SchemaRef,
    pub primary_key: &'static [&'static str],
    pub workspace_scope: Option<OperationalWorkspaceScope>,
}

/// Route from an operational row to its owning workspace.
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

#[derive(Clone, Copy)]
enum OperationalLogicalType {
    Id16,
    Hash32,
    Int64,
    Utf8,
    Binary,
    TimestampUtc,
}

#[derive(Clone, Copy)]
struct OperationalColumnContract {
    name: &'static str,
    sqlite_type: OperationalSqliteType,
    logical_type: OperationalLogicalType,
    id_domain: Option<&'static str>,
    nullable: bool,
}

#[derive(Clone, Copy)]
struct OperationalTableContract {
    name: &'static str,
    sqlite_ddl: &'static str,
    columns: &'static [OperationalColumnContract],
    primary_key: &'static [&'static str],
    workspace_scope: Option<OperationalWorkspaceScope>,
}

include!("operational_schema_specs.rs");

fn operational_physical_type(logical: OperationalLogicalType) -> DataType {
    match logical {
        OperationalLogicalType::Id16 => DataType::FixedSizeBinary(16),
        OperationalLogicalType::Hash32 => DataType::FixedSizeBinary(32),
        OperationalLogicalType::Int64 => DataType::Int64,
        OperationalLogicalType::Utf8 => DataType::Utf8,
        OperationalLogicalType::Binary => DataType::Binary,
        OperationalLogicalType::TimestampUtc => {
            DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC")))
        }
    }
}

fn operational_field(contract: OperationalColumnContract, primary_key: &[&str]) -> Field {
    let mut metadata = HashMap::new();
    if matches!(contract.logical_type, OperationalLogicalType::Id16) {
        metadata.insert("com.codefabric.cpg.id_width".to_owned(), "16".to_owned());
    }
    if contract.id_domain.is_some() {
        metadata.insert(
            "com.codefabric.cpg.semantic_type".to_owned(),
            "id16".to_owned(),
        );
    }
    if primary_key.contains(&contract.name) {
        metadata.insert(
            "com.codefabric.cpg.primary_key_part".to_owned(),
            "true".to_owned(),
        );
    }
    let field = Field::new(
        contract.name,
        operational_physical_type(contract.logical_type),
        contract.nullable,
    )
    .with_metadata(metadata);
    match contract.logical_type {
        OperationalLogicalType::Id16 => attach_id_domain(
            field,
            contract
                .id_domain
                .expect("operational Id16 columns declare a logical domain"),
        )
        .expect("operational ID domains are compile-time validated"),
        OperationalLogicalType::Hash32 => field.with_extension_type(Hash32Extension::v1()),
        _ => field,
    }
}

fn build_operational(contract: OperationalTableContract) -> OperationalTableSpec {
    let fields = contract
        .columns
        .iter()
        .copied()
        .map(|column| operational_field(column, contract.primary_key))
        .collect::<Vec<_>>();
    OperationalTableSpec {
        name: contract.name,
        sqlite_ddl: contract.sqlite_ddl,
        sqlite_column_types: contract
            .columns
            .iter()
            .map(|column| column.sqlite_type)
            .collect(),
        arrow_schema: Arc::new(Schema::new(fields)),
        primary_key: contract.primary_key,
        workspace_scope: contract.workspace_scope,
    }
}

/// Return every live operational-store contract in source order.
#[must_use]
pub fn operational_table_specs() -> &'static [OperationalTableSpec] {
    static OPERATIONAL_SPECS: OnceLock<Vec<OperationalTableSpec>> = OnceLock::new();
    OPERATIONAL_SPECS.get_or_init(|| {
        OPERATIONAL_TABLE_CONTRACTS
            .iter()
            .copied()
            .map(build_operational)
            .collect()
    })
}

/// Resolve one operational-store contract by table name.
#[must_use]
pub fn operational_table_spec(name: &str) -> Option<&'static OperationalTableSpec> {
    operational_table_specs()
        .iter()
        .find(|table| table.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::{Cursor, Seek as _};
    use std::sync::Arc;

    use arrow::array::{ArrayRef, FixedSizeBinaryBuilder};
    use arrow::ipc::reader::StreamReader;
    use arrow::ipc::writer::StreamWriter;
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    use super::*;

    #[test]
    fn logical_extensions_have_registration_and_width_contracts() {
        assert_eq!(extension_type_registrations().len(), id_domains().len() + 1);
        for domain in id_domains() {
            let field = attach_id_domain(
                Field::new("id", DataType::FixedSizeBinary(16), false),
                domain.domain_slug,
            )
            .unwrap();
            assert_eq!(
                field.extension_type_name(),
                Some(domain.extension_name),
                "{}",
                domain.rust_type
            );
            validate_logical_extension_field(&field).unwrap();
        }
        let hash = Field::new("digest", DataType::FixedSizeBinary(32), false)
            .with_extension_type(Hash32Extension::v1());
        validate_logical_extension_field(&hash).unwrap();

        let wrong_width = Field::new("id", DataType::FixedSizeBinary(32), false)
            .with_metadata(hash.metadata().clone());
        assert!(validate_logical_extension_field(&wrong_width).is_err());
    }

    #[test]
    fn logical_extension_survives_arrow_ipc_and_parquet() {
        let field = Field::new("entity_id", DataType::FixedSizeBinary(16), false)
            .with_extension_type(EntityIdExtension::v1());
        let schema = Arc::new(Schema::new(vec![field]));
        let mut values = FixedSizeBinaryBuilder::with_capacity(1, 16);
        values.append_value([0x58; 16]).unwrap();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(values.finish()) as ArrayRef],
        )
        .unwrap();

        let mut ipc = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut ipc, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        let ipc_batch = StreamReader::try_new(Cursor::new(ipc), None)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            ipc_batch
                .schema()
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1()
        );

        let mut parquet_file = tempfile::tempfile().unwrap();
        {
            let mut writer =
                ArrowWriter::try_new(parquet_file.try_clone().unwrap(), schema, None).unwrap();
            writer.write(&batch).unwrap();
            writer.close().unwrap();
        }
        parquet_file.rewind().unwrap();
        let parquet_batch = ParquetRecordBatchReaderBuilder::try_new(parquet_file)
            .unwrap()
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            parquet_batch
                .schema()
                .field(0)
                .try_extension_type::<EntityIdExtension>()
                .unwrap(),
            EntityIdExtension::v1()
        );
    }

    #[test]
    fn operational_contracts_are_complete_and_uniquely_named() {
        let specs = operational_table_specs();
        let names = specs.iter().map(|spec| spec.name).collect::<BTreeSet<_>>();
        assert_eq!(names.len(), specs.len());
        assert!(names.contains("workspace_registration"));
        assert!(names.contains("source_inventory"));
        assert!(names.contains("query_execution_terminal"));

        for spec in specs {
            assert!(!spec.arrow_schema.fields().is_empty());
            assert_eq!(
                spec.arrow_schema.fields().len(),
                spec.sqlite_column_types.len()
            );
            assert!(
                spec.sqlite_ddl
                    .starts_with(&format!("CREATE TABLE {} (", spec.name))
            );
            assert!(spec.sqlite_ddl.ends_with(") STRICT;\n"));
            for field in spec.arrow_schema.fields() {
                validate_logical_extension_field(field).unwrap();
            }
        }
    }
}
