//! Original Protocol/Metadata allocation ownership, separate from their containers.

use std::sync::Arc;

use crate::engine_data::GetData;
use crate::resource::{AllocationReceipt, AllocationRequest, NativeResourceScope};
use crate::{DeltaResult, Error};

use super::{Metadata, Protocol, Sidecar};

/// A compatibility deep clone is not a resource admission. Native governed copy
/// APIs attach a newly admitted receipt to the original copied action itself.
#[derive(Debug, Default)]
pub(crate) struct ActionResourceOwner(Option<Arc<NativeResourceScope>>);
impl Clone for ActionResourceOwner {
    fn clone(&self) -> Self {
        Self(None)
    }
}
impl PartialEq for ActionResourceOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for ActionResourceOwner {}

impl ActionResourceOwner {
    pub(crate) fn validate_current_scope(&self) -> DeltaResult<()> {
        if self.0.is_some() && crate::resource::current_resource_scope().is_none() {
            return Err(crate::resource::ResourceExhausted {
                kind: "native_action_conversion_scope",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        Ok(())
    }
}

fn admit_conversion(bytes: usize) -> DeltaResult<Option<Arc<dyn AllocationReceipt>>> {
    crate::resource::current_resource_scope()
        .map(|scope| {
            scope.reserve_transient(AllocationRequest {
                kind: "native_action_scalar_conversion",
                bytes,
            })
        })
        .transpose()
}

pub(crate) struct ActionAdmission {
    scope: Option<Arc<NativeResourceScope>>,
    diagnostics: Option<Arc<dyn AllocationReceipt>>,
}
impl ActionAdmission {
    pub(crate) fn try_new() -> DeltaResult<Self> {
        let scope = crate::resource::current_resource_scope();
        let diagnostics = scope
            .as_ref()
            .map(|scope| {
                scope.reserve_transient(AllocationRequest {
                    kind: "native_action_diagnostics",
                    bytes: crate::crc::resource::diagnostic_bytes(0)?,
                })
            })
            .transpose()?;
        Ok(Self { scope, diagnostics })
    }
    pub(crate) fn scope(&self) -> Option<Arc<NativeResourceScope>> {
        self.scope.clone()
    }
    pub(crate) fn finish<T>(self, result: DeltaResult<T>) -> DeltaResult<T> {
        result.map_err(|error| {
            if error.is_resource_exhausted() || self.scope.is_none() {
                return error;
            }
            Error::generic_err(ActionError {
                error,
                _scope: self.scope,
                _diagnostics: self.diagnostics,
            })
        })
    }
    pub(crate) fn metadata<'a>(
        &self,
        row: usize,
        getters: &[&'a dyn GetData<'a>],
    ) -> DeltaResult<()> {
        use crate::crc::resource::{add, map_layout, mul};
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        if getters[0].get_str(row, "metadata.id")?.is_none() {
            return Ok(());
        }
        let mut bytes = std::mem::size_of::<Metadata>();
        for column in [0, 1, 2, 3, 5] {
            bytes = add(
                bytes,
                getters[column]
                    .get_str(row, "metadata original string")?
                    .map_or(0, str::len),
            )?;
        }
        if let Some(list) = getters[6].get_list(row, "metadata.partition_list")? {
            bytes = add(bytes, mul(list.len(), std::mem::size_of::<String>())?)?;
            for index in 0..list.len() {
                bytes = add(bytes, list.get_borrowed(index).len())?;
            }
        }
        if let Some(map) = getters[8].get_map(row, "metadata.configuration")? {
            bytes = add(bytes, map_layout::<String, String>(map.len())?)?;
            for (key, value) in map.borrowed_entries() {
                bytes = add(bytes, add(key.len(), value.map_or(0, str::len))?)?;
            }
        }
        scope.reserve(AllocationRequest {
            kind: "native_metadata_from_getters",
            bytes,
        })
    }
    pub(crate) fn sidecar<'a>(&self, row: usize, getters: &[&'a dyn GetData<'a>]) -> DeltaResult<()> {
        use crate::crc::resource::{add, map_layout};
        let Some(scope) = &self.scope else { return Ok(()); };
        let Some(path) = getters[0].get_str(row, "sidecar.path")? else { return Ok(()); };
        let mut bytes = add(std::mem::size_of::<Sidecar>(), path.len())?;
        if let Some(map) = getters[3].get_map(row, "sidecar.tags")? {
            bytes = add(bytes, map_layout::<String, String>(map.len())?)?;
            for (key, value) in map.borrowed_entries() {
                bytes = add(bytes, add(key.len(), value.map_or(0, str::len))?)?;
            }
        }
        scope.reserve(AllocationRequest { kind: "native_sidecar_from_getters", bytes })
    }
    pub(crate) fn protocol<'a>(
        &self,
        row: usize,
        getters: &[&'a dyn GetData<'a>],
    ) -> DeltaResult<()> {
        use crate::crc::resource::{add, mul};
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        if getters[0]
            .get_int(row, "protocol.min_reader_version")?
            .is_none()
        {
            return Ok(());
        }
        let mut bytes = std::mem::size_of::<Protocol>();
        for column in [2, 3] {
            if let Some(list) = getters[column].get_list(row, "protocol original features")? {
                bytes = add(
                    bytes,
                    mul(
                        list.len(),
                        std::mem::size_of::<String>()
                            + std::mem::size_of::<crate::table_features::TableFeature>(),
                    )?,
                )?;
                for index in 0..list.len() {
                    // Materialized String and independently allocated Unknown conversion.
                    bytes = add(bytes, mul(list.get_borrowed(index).len(), 2)?)?;
                }
            }
        }
        scope.reserve(AllocationRequest {
            kind: "native_protocol_from_getters",
            bytes,
        })
    }
}

#[derive(Debug)]
struct ActionError {
    error: Error,
    _scope: Option<Arc<NativeResourceScope>>,
    _diagnostics: Option<Arc<dyn AllocationReceipt>>,
}
impl std::fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}
impl std::error::Error for ActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl Metadata {
    pub(crate) fn admit_engine_data_conversion(
        &self,
    ) -> DeltaResult<Option<Arc<dyn AllocationReceipt>>> {
        use crate::crc::resource::{add, mul};
        // Vec<Result<Scalar>> collection may grow from a zero lower hint. Both
        // maps become Scalar pairs; partition columns become a Scalar vector.
        let pairs = add(self.format.options.len(), self.configuration.len())?;
        let scalars = add(self.partition_columns.len(), mul(pairs, 2)?)?;
        let bytes = add(
            mul(
                add(scalars, 24)?,
                4 * std::mem::size_of::<crate::expressions::Scalar>(),
            )?,
            3 * std::mem::size_of::<crate::schema::DataType>()
                + 2 * std::mem::size_of::<crate::schema::MapType>()
                + std::mem::size_of::<crate::schema::ArrayType>(),
        )?;
        admit_conversion(bytes)
    }
    /// Resource owner of this original native action, independent of its parent CRC/snapshot.
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.resource_owner.0.as_ref()
    }

    /// Reserve the complete original action copy before invoking native deep Clone.
    pub fn try_clone_admitted(&self, scope: Option<Arc<NativeResourceScope>>) -> DeltaResult<Self> {
        let scope = crate::crc::resource::select_scope(scope, self.resource_owner.0.clone())?;
        if let Some(scope) = &scope {
            scope.reserve(AllocationRequest {
                kind: "native_metadata_clone",
                bytes: crate::crc::resource::add(
                    std::mem::size_of::<Self>(),
                    crate::crc::resource::metadata_bytes(self)?,
                )?,
            })?;
        }
        let mut result = self.clone();
        result.attach_admitted_owner(scope);
        Ok(result)
    }

    /// Caller must have admitted the actual decode/copy before constructing this action.
    pub(crate) fn attach_admitted_owner(&mut self, scope: Option<Arc<NativeResourceScope>>) {
        self.resource_owner = ActionResourceOwner(scope);
    }
}

impl Protocol {
    pub(crate) fn admit_engine_data_conversion(
        &self,
    ) -> DeltaResult<Option<Arc<dyn AllocationReceipt>>> {
        use crate::crc::resource::{add, mul};
        let mut bytes = 2
            * (std::mem::size_of::<crate::schema::DataType>()
                + std::mem::size_of::<crate::schema::ArrayType>());
        for features in [&self.reader_features, &self.writer_features]
            .into_iter()
            .flatten()
        {
            bytes = add(
                bytes,
                mul(
                    add(features.len(), 4)?,
                    4 * std::mem::size_of::<crate::expressions::Scalar>(),
                )?,
            )?;
            for feature in features {
                bytes = add(bytes, mul(add(feature.as_ref().len(), 8)?, 4)?)?;
            }
        }
        admit_conversion(bytes)
    }
    /// Resource owner of this original native action, independent of its parent CRC/snapshot.
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.resource_owner.0.as_ref()
    }

    /// Reserve the complete original action copy before invoking native deep Clone.
    pub fn try_clone_admitted(&self, scope: Option<Arc<NativeResourceScope>>) -> DeltaResult<Self> {
        let scope = crate::crc::resource::select_scope(scope, self.resource_owner.0.clone())?;
        if let Some(scope) = &scope {
            scope.reserve(AllocationRequest {
                kind: "native_protocol_clone",
                bytes: crate::crc::resource::add(
                    std::mem::size_of::<Self>(),
                    crate::crc::resource::protocol_bytes(self)?,
                )?,
            })?;
        }
        let mut result = self.clone();
        result.attach_admitted_owner(scope);
        Ok(result)
    }

    /// Caller must have admitted the actual decode/copy before constructing this action.
    pub(crate) fn attach_admitted_owner(&mut self, scope: Option<Arc<NativeResourceScope>>) {
        self.resource_owner = ActionResourceOwner(scope);
    }
}

impl Sidecar {
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> { self.resource_owner.0.as_ref() }
    pub(crate) fn attach_admitted_owner(&mut self, scope: Option<Arc<NativeResourceScope>>) { self.resource_owner = ActionResourceOwner(scope); }
    pub fn try_clone_admitted(&self, explicit: Option<Arc<NativeResourceScope>>) -> DeltaResult<Self> {
        use crate::crc::resource::{add, map_layout};
        let scope = crate::crc::resource::select_scope(explicit, self.resource_owner.0.clone())?;
        let mut bytes = add(std::mem::size_of::<Self>(), self.path.len())?;
        if let Some(tags) = &self.tags {
            bytes = add(bytes, map_layout::<String, String>(tags.capacity())?)?;
            for (key, value) in tags { bytes = add(bytes, add(key.len(), value.len())?)?; }
        }
        if let Some(scope) = &scope { scope.reserve(AllocationRequest { kind: "native_sidecar_clone", bytes })?; }
        let mut cloned = self.clone();
        cloned.attach_admitted_owner(scope);
        Ok(cloned)
    }
}
