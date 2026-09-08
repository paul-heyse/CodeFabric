//! Admission for the native table configuration's own backing and parse phases.

use std::mem::size_of;
use std::sync::Arc;

use crate::actions::Metadata;
use crate::crc::resource::{add, map_layout, mul};
use crate::expressions::ColumnName;
use crate::resource::{AllocationReceipt, AllocationRequest, NativeResourceScope};
use crate::schema::StructType;
use crate::table_properties::TableProperties;
use crate::{DeltaResult, Error};

use super::TableConfiguration;

#[derive(Debug, Default)]
pub(super) struct ConfigurationResourceOwner(Option<Arc<NativeResourceScope>>);
impl Clone for ConfigurationResourceOwner {
    fn clone(&self) -> Self {
        Self(None)
    }
}
impl PartialEq for ConfigurationResourceOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for ConfigurationResourceOwner {}

fn sum(values: impl IntoIterator<Item = usize>) -> DeltaResult<usize> {
    values.into_iter().try_fold(0, add)
}

fn property_parse_bytes(metadata: &Metadata) -> DeltaResult<usize> {
    let source_bytes = metadata
        .configuration()
        .iter()
        .try_fold(0, |bytes, (key, value)| {
            add(bytes, add(key.len(), value.len())?)
        })?;
    let count = metadata.configuration().len();
    // ColumnName parser: at most one path/column/name entry per source byte,
    // plus an empty component at each property's end. Native FromIterator moves
    // the path vector, but the bound permits a second owned vector as well.
    let elements = add(source_bytes, add(count, 8)?)?;
    let vectors = mul(
        mul(elements, 8)?,
        add(size_of::<ColumnName>(), size_of::<String>())?,
    )?;
    let unknown_map = mul(map_layout::<String, String>(add(count, 8)?)?, 4)?;
    // Parsed field names, three recognized string-valued properties, unknown
    // pairs and invalid interval/name diagnostics. Vec/String growth includes
    // old/new overlap; Debug name escaping is <=10 bytes per source byte.
    let strings_and_errors = mul(add(mul(source_bytes, 10)?, mul(add(count, 1)?, 512)?)?, 4)?;
    sum([
        vectors,
        unknown_map,
        strings_and_errors,
        size_of::<TableProperties>(),
    ])
}

fn property_clone_bytes(properties: &TableProperties) -> DeltaResult<usize> {
    let mut bytes = map_layout::<String, String>(properties.unknown_properties.capacity())?;
    for (key, value) in &properties.unknown_properties {
        bytes = add(bytes, add(key.len(), value.len())?)?;
    }
    for value in [
        &properties.materialized_row_id_column_name,
        &properties.materialized_row_commit_version_column_name,
        &properties.parquet_format_version,
    ]
    .into_iter()
    .flatten()
    {
        bytes = add(bytes, value.len())?;
    }
    if let Some(columns) = &properties.data_skipping_stats_columns {
        bytes = add(bytes, mul(columns.len(), size_of::<ColumnName>())?)?;
        for column in columns {
            bytes = add(bytes, mul(column.path().len(), size_of::<String>())?)?;
            for part in column.path() {
                bytes = add(bytes, part.len())?;
            }
        }
    }
    Ok(bytes)
}

pub(super) struct ConfigurationAdmission {
    scope: Arc<NativeResourceScope>,
    scratch: Arc<dyn AllocationReceipt>,
}
impl ConfigurationAdmission {
    pub(super) fn begin(metadata: &Metadata, schema: &StructType) -> DeltaResult<Option<Self>> {
        let Some(scope) = crate::resource::current_resource_scope() else {
            return Ok(None);
        };
        scope.check_available()?;
        // Native TableProperties has no Arc container; the original owning
        // TableConfiguration retains this charge, distinct from its schema Arcs.
        let properties = property_parse_bytes(metadata)?;
        scope.reserve(AllocationRequest {
            kind: "native_table_configuration",
            bytes: sum([
                properties,
                size_of::<TableConfiguration>(),
                2 * size_of::<usize>(),
            ])?,
        })?;
        let partitions = mul(
            map_layout::<&str, ()>(add(metadata.partition_columns().len(), 8)?)?,
            8,
        )?;
        let scratch = scope.reserve_transient(AllocationRequest {
            kind: "native_table_configuration_validation",
            bytes: sum([
                properties,
                partitions,
                schema.native_validation_bound()?,
                size_of::<ConfigurationError>(),
            ])?,
        })?;
        Ok(Some(Self { scope, scratch }))
    }

    pub(super) fn finish(
        self,
        result: DeltaResult<TableConfiguration>,
    ) -> DeltaResult<TableConfiguration> {
        self.scope.check_available()?;
        match result {
            Ok(mut configuration) => {
                configuration.resource_owner = ConfigurationResourceOwner(Some(self.scope));
                Ok(configuration)
            }
            Err(error) if error.is_resource_exhausted() => Err(error),
            Err(error) => Err(Error::generic_err(ConfigurationError {
                error,
                _scope: self.scope,
                _scratch: self.scratch,
            })),
        }
    }
}

#[derive(Debug)]
struct ConfigurationError {
    error: Error,
    _scope: Arc<NativeResourceScope>,
    _scratch: Arc<dyn AllocationReceipt>,
}
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl TableConfiguration {
    /// Copy the original URL only after reserving its new native String backing.
    pub(super) fn clone_table_root_admitted(&self) -> DeltaResult<url::Url> {
        if let Some(scope) = crate::resource::current_resource_scope() {
            scope.reserve(AllocationRequest {
                kind: "native_table_root_copy",
                bytes: self.table_root.as_str().len(),
            })?;
        }
        Ok(self.table_root.clone())
    }

    /// Share original schema Arcs and pre-admit the configuration's actual deep
    /// copies. Generic infallible Clone has no governed-owner implication.
    pub fn try_clone_admitted(&self) -> DeltaResult<Self> {
        let scope =
            crate::resource::current_resource_scope().or_else(|| self.resource_owner.0.clone());
        let Some(scope) = scope else {
            return Ok(self.clone());
        };
        crate::resource::ensure_required_scope(&scope)?;
        if self.resource_owner.0.is_none() {
            return Err(crate::resource::ResourceExhausted {
                kind: "native_table_configuration_unadmitted_copy_source",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        scope.reserve(AllocationRequest {
            kind: "native_table_configuration_copy",
            bytes: sum([
                size_of::<Self>(),
                2 * size_of::<usize>(),
                self.table_root.as_str().len(),
                property_clone_bytes(&self.table_properties)?,
            ])?,
        })?;
        let metadata = self.metadata.try_clone_admitted(Some(scope.clone()))?;
        let protocol = self.protocol.try_clone_admitted(Some(scope.clone()))?;
        Ok(Self {
            metadata,
            protocol,
            logical_schema: self.logical_schema.clone(),
            logical_schema_without_partition_columns: self
                .logical_schema_without_partition_columns
                .clone(),
            physical_schema: self.physical_schema.clone(),
            physical_data_schema_without_partition_columns: self
                .physical_data_schema_without_partition_columns
                .clone(),
            table_properties: self.table_properties.clone(),
            column_mapping_mode: self.column_mapping_mode,
            table_root: self.table_root.clone(),
            version: self.version,
            resource_owner: ConfigurationResourceOwner(Some(scope)),
        })
    }

    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.resource_owner.0.as_ref()
    }
}
