// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership. The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License. You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the
// specific language governing permissions and limitations
// under the License.

//! Admission for the existing Arrow IPC schema-hint converter. The borrowed
//! FlatBuffer walk does not replace schema conversion or interpret Delta data.

use std::alloc::Layout;
use std::mem::size_of;

use arrow_schema::{DataType, Field, FieldRef, Schema};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;

use crate::errors::{ParquetError, Result};
use crate::resource::{ReaderResourcePolicy as Policy, ResourceExhausted};

fn overflow(kind: &'static str) -> ResourceExhausted {
    ResourceExhausted {
        kind,
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
}

fn shared_allocation(policy: &Policy, payload: Layout, kind: &'static str) -> Result<()> {
    // Pinned Rust 1.98 ArcInner<T> is repr(C, align(2)), with two AtomicUsize
    // counters followed by T. This also covers Arc<str> and Arc<[FieldRef]>.
    let (layout, _) = Layout::new::<[std::sync::atomic::AtomicUsize; 2]>()
        .extend(payload)
        .map_err(|_| overflow(kind))?;
    policy.reserve(layout.pad_to_align().size(), kind)?;
    Ok(())
}

fn shared_slice<T>(policy: &Policy, count: usize, kind: &'static str) -> Result<()> {
    Policy::check(
        count,
        policy.limits().collection_entries,
        "arrow schema collection entries",
    )?;
    shared_allocation(
        policy,
        Layout::array::<T>(count).map_err(|_| overflow(kind))?,
        kind,
    )
}

fn vector_growth<T>(policy: &Policy, count: usize, kind: &'static str) -> Result<()> {
    Policy::check(
        count,
        policy.limits().collection_entries,
        "arrow schema collection entries",
    )?;
    vector_growth_layout::<T>(policy, count, kind)
}

fn vector_growth_layout<T>(policy: &Policy, count: usize, kind: &'static str) -> Result<()> {
    if count == 0 || size_of::<T>() == 0 {
        return Ok(());
    }
    // alloc::raw_vec::RawVecInner::grow_amortized: min_non_zero_cap then
    // doubling. Reserve every full replacement capacity, retaining old peaks.
    let mut capacity: usize = if size_of::<T>() == 1 {
        8
    } else if size_of::<T>() <= 1024 {
        4
    } else {
        1
    };
    loop {
        policy.reserve(
            capacity
                .checked_mul(size_of::<T>())
                .ok_or_else(|| overflow(kind))?,
            kind,
        )?;
        if capacity >= count {
            return Ok(());
        }
        capacity = capacity.checked_mul(2).ok_or_else(|| overflow(kind))?;
    }
}

fn metadata_tables(policy: &Policy, count: usize) -> Result<()> {
    Policy::check(
        count,
        policy.limits().collection_entries,
        "arrow schema metadata entries",
    )?;
    if count == 0 {
        return Ok(());
    }
    // Rust 1.98's vendored hashbrown 0.17.1 raw::{capacity_to_buckets,
    // TableLayout::calculate_layout_for}: String pairs exceed all small-item
    // cases, so start at four buckets, then double at 7/8 occupancy. Supported
    // native control groups are at most 16 bytes (SSE2/NEON/LSX; generic <=8).
    // Using that maximum also covers target-specific alignment padding. These
    // are native table-layout bounds, not a multiplier over encoded input.
    let entry = Layout::new::<(String, String)>();
    let align = entry.align().max(16);
    let mut buckets: usize = 4;
    loop {
        let control_offset = entry
            .size()
            .checked_mul(buckets)
            .and_then(|bytes| bytes.checked_add(align - 1))
            .map(|bytes| bytes & !(align - 1))
            .ok_or_else(|| overflow("arrow metadata hash table"))?;
        let bytes = control_offset
            .checked_add(buckets)
            .and_then(|bytes| bytes.checked_add(16))
            .ok_or_else(|| overflow("arrow metadata hash table"))?;
        policy.reserve(bytes, "arrow metadata hash table")?;
        let capacity = if buckets < 8 {
            buckets - 1
        } else {
            buckets / 8 * 7
        };
        if capacity >= count {
            return Ok(());
        }
        buckets = buckets
            .checked_mul(2)
            .ok_or_else(|| overflow("arrow metadata hash table"))?;
    }
}

pub(super) fn admit_metadata<'a>(
    policy: &Policy,
    entries: impl Iterator<Item = (&'a str, &'a str)>,
) -> Result<()> {
    let mut count = 0_usize;
    for (key, value) in entries {
        count = count
            .checked_add(1)
            .ok_or_else(|| overflow("arrow schema metadata entries"))?;
        Policy::check(
            count,
            policy.limits().collection_entries,
            "arrow schema metadata entries",
        )?;
        policy.string(key.len())?;
        policy.string(value.len())?;
    }
    metadata_tables(policy, count)
}

fn field(policy: &Policy, value: arrow_ipc::Field<'_>, depth: usize) -> Result<()> {
    Policy::check(depth, policy.limits().schema_depth, "arrow schema depth")?;
    policy.string(value.name().unwrap_or_default().len())?;
    shared_allocation(policy, Layout::new::<Field>(), "arrow schema field owner")?;
    if let Some(metadata) = value.custom_metadata() {
        admit_metadata(
            policy,
            metadata
                .iter()
                .filter_map(|kv| Some((kv.key()?, kv.value()?))),
        )?;
    }
    if value.dictionary().is_some() {
        // Native get_data_type allocates an index-type Box and value-type Box.
        policy.reserve(size_of::<DataType>(), "arrow schema dictionary type")?;
        policy.reserve(size_of::<DataType>(), "arrow schema dictionary type")?;
    }
    if let Some(timestamp) = value.type_as_timestamp() {
        if let Some(timezone) = timestamp.timezone() {
            Policy::check(
                timezone.len(),
                policy.limits().string_bytes,
                "arrow schema timezone bytes",
            )?;
            shared_allocation(
                policy,
                Layout::array::<u8>(timezone.len())
                    .map_err(|_| overflow("arrow schema timezone"))?,
                "arrow schema timezone",
            )?;
        }
    }
    let children = value.children();
    let count = children.map_or(0, |children| children.len());
    Policy::check(
        count,
        policy.limits().collection_entries,
        "arrow schema children",
    )?;
    match value.type_type() {
        arrow_ipc::Type::Struct_ => {
            // Fields collects a non-TrustedLen FlatBuffers iterator through a
            // Vec<FieldRef>, then Arc<[FieldRef]> (alloc::sync::ToArcSlice).
            vector_growth::<FieldRef>(policy, count, "arrow schema child fields")?;
            shared_slice::<FieldRef>(policy, count, "arrow schema child owner")?;
        }
        arrow_ipc::Type::Union => {
            vector_growth::<Field>(policy, count, "arrow schema union fields")?;
            vector_growth::<(i8, FieldRef)>(policy, count, "arrow schema union pairs")?;
            shared_slice::<(i8, FieldRef)>(policy, count, "arrow schema union owner")?;
        }
        _ => {}
    }
    if let Some(children) = children {
        for child in children {
            field(
                policy,
                child,
                depth
                    .checked_add(1)
                    .ok_or_else(|| overflow("arrow schema depth"))?,
            )?;
        }
    }
    Ok(())
}

pub(super) fn clone_data_type(value: &DataType) -> Result<DataType> {
    fn admit(policy: &Policy, value: &DataType, depth: usize) -> Result<()> {
        Policy::check(depth, policy.limits().schema_depth, "arrow data type depth")?;
        if let DataType::Dictionary(key, value) = value {
            policy.reserve(size_of::<DataType>(), "arrow data type clone")?;
            policy.reserve(size_of::<DataType>(), "arrow data type clone")?;
            admit(policy, key, depth + 1)?;
            admit(policy, value, depth + 1)?;
        }
        Ok(())
    }
    if let Some(policy) = Policy::current() {
        admit(&policy, value, 1)?;
    }
    Ok(value.clone())
}

pub(super) fn clone_metadata(
    value: &std::collections::HashMap<String, String>,
) -> Result<std::collections::HashMap<String, String>> {
    if let Some(policy) = Policy::current() {
        // HashMap::clone preserves table geometry even when a caller supplied
        // a sparse map; its capacity, not just its len, bounds that allocation.
        metadata_tables(&policy, value.capacity())?;
        for (key, value) in value {
            policy.string(key.len())?;
            policy.string(value.len())?;
        }
    }
    Ok(value.clone())
}

pub(super) fn timezone(value: &str) -> Result<std::sync::Arc<str>> {
    if let Some(policy) = Policy::current() {
        Policy::check(
            value.len(),
            policy.limits().string_bytes,
            "arrow schema timezone bytes",
        )?;
        shared_allocation(
            &policy,
            Layout::array::<u8>(value.len()).map_err(|_| overflow("arrow schema timezone"))?,
            "arrow schema timezone",
        )?;
    }
    Ok(value.into())
}

pub(super) fn native_field(name: &str) -> Result<()> {
    if let Some(policy) = Policy::current() {
        policy.string(name.len())?;
        shared_allocation(
            &policy,
            Layout::new::<Field>(),
            "arrow converted field owner",
        )?;
    }
    Ok(())
}

pub(super) fn native_children(count: usize) -> Result<()> {
    if let Some(policy) = Policy::current() {
        policy.collection(
            count,
            size_of::<super::complex::ParquetField>(),
            "arrow parquet child descriptors",
        )?;
    }
    Ok(())
}

pub(super) fn native_struct_fields(count: usize) -> Result<()> {
    if let Some(policy) = Policy::current() {
        policy.collection(count, size_of::<FieldRef>(), "arrow converted child fields")?;
        shared_slice::<FieldRef>(&policy, count, "arrow converted children owner")?;
    }
    Ok(())
}

pub(super) fn native_field_id(id: i32) -> Result<()> {
    if let Some(policy) = Policy::current() {
        metadata_tables(&policy, 1)?;
        policy.string(super::PARQUET_FIELD_ID_META_KEY.len())?;
        let digits = if id == 0 {
            1
        } else {
            id.unsigned_abs().ilog10() as usize + 1
        } + usize::from(id < 0);
        // integer Display has at most the exact decimal digits; this also
        // covers String's generic initial8 / doubling growth if selected.
        vector_growth_layout::<u8>(&policy, digits, "arrow field id string")?;
    }
    Ok(())
}

pub(super) fn decode_embedded_schema(policy: &Policy, encoded: &str) -> Result<Schema> {
    let limit = policy.limits();
    Policy::check(
        encoded.len(),
        limit.string_bytes,
        "arrow schema encoded bytes",
    )?;
    // base64 0.22's GeneralPurpose estimate allocates 3 bytes per encoded
    // four-byte quantum, including a partial final quantum, before decode.
    let decoded = (encoded.len() / 4)
        .checked_add(usize::from(encoded.len() % 4 != 0))
        .and_then(|quanta| quanta.checked_mul(3))
        .ok_or_else(|| overflow("arrow schema base64"))?;
    policy.reserve(decoded, "arrow schema base64")?;
    // FlatBuffers validation uses no heap on success. On an ordinary invalid
    // field it appends TableField, VectorElement, and UnionVariant trace frames;
    // at most these three frames per nested table on the generated Message /
    // Schema / Field route. Admission precedes the verifier's error-path Vec.
    let traces = limit
        .schema_depth
        .checked_mul(3)
        .ok_or_else(|| overflow("arrow schema verifier trace"))?;
    vector_growth_layout::<flatbuffers::ErrorTraceDetail>(
        policy,
        traces,
        "arrow schema verifier trace",
    )?;
    const INVALID: &str = "Invalid embedded Arrow schema";
    policy.reserve(INVALID.len(), "arrow schema error diagnostic")?;
    let invalid = || ParquetError::General(INVALID.to_string());
    let bytes = BASE64_STANDARD.decode(encoded).map_err(|_| invalid())?;
    let slice = if bytes.len() > 8 && bytes[..4] == [255_u8; 4] {
        &bytes[8..]
    } else {
        &bytes
    };
    let options = flatbuffers::VerifierOptions {
        max_depth: limit.schema_depth,
        max_tables: limit.collection_entries,
        max_apparent_size: limit.output_bytes,
        ignore_missing_null_terminator: false,
    };
    let message = arrow_ipc::root_as_message_with_opts(&options, slice).map_err(|error| {
        let (kind, bound) = match error {
            flatbuffers::InvalidFlatbuffer::DepthLimitReached => {
                ("arrow schema verifier depth", options.max_depth)
            }
            flatbuffers::InvalidFlatbuffer::TooManyTables => {
                ("arrow schema verifier tables", options.max_tables)
            }
            flatbuffers::InvalidFlatbuffer::ApparentSizeTooLarge => {
                ("arrow schema verifier expansion", options.max_apparent_size)
            }
            _ => return invalid(),
        };
        ResourceExhausted {
            kind,
            requested: bound.saturating_add(1),
            limit: bound,
        }
        .into()
    })?;
    let schema = message.header_as_schema().ok_or_else(invalid)?;
    let fields = schema.fields().ok_or_else(invalid)?;
    vector_growth::<Field>(policy, fields.len(), "arrow schema root fields")?;
    shared_slice::<FieldRef>(policy, fields.len(), "arrow schema root owner")?;
    for value in fields {
        field(policy, value, 1)?;
    }
    if let Some(metadata) = schema.custom_metadata() {
        admit_metadata(
            policy,
            metadata
                .iter()
                .filter_map(|kv| Some((kv.key()?, kv.value()?))),
        )?;
    }
    Ok(arrow_ipc::convert::fb_to_schema(schema))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{
        ReaderResourceLimits, ResourceAdmission, ResourceReceipt, ResourceRequest,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Debug)]
    struct Receipt(usize);
    impl ResourceReceipt for Receipt {
        fn bytes(&self) -> usize {
            self.0
        }
    }
    #[derive(Debug)]
    struct Admit {
        deny: &'static str,
    }
    impl ResourceAdmission for Admit {
        fn try_reserve(
            &self,
            request: ResourceRequest,
        ) -> std::result::Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
            if request.kind == self.deny {
                return Err(ResourceExhausted {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
            }
            Ok(Arc::new(Receipt(request.bytes)))
        }
    }
    fn policy(deny: &'static str) -> Policy {
        Policy::try_new(
            Arc::new(Admit { deny }),
            ReaderResourceLimits {
                allocations: 4096,
                collection_entries: 10000,
                string_bytes: 100000,
                footer_bytes: 100000,
                page_bytes: 100000,
                page_values: 10000,
                output_values: 10000,
                output_bytes: 100000,
                schema_depth: 32,
                codec_bytes: 100000,
            },
        )
        .unwrap()
    }

    #[test]
    fn resource_embedded_schema_base64_denial_precedes_decode() {
        let policy = policy("arrow schema base64");
        // Invalid input would otherwise produce a base64 parse error.
        assert!(matches!(
            decode_embedded_schema(&policy, "????"),
            Err(ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "arrow schema base64",
                ..
            }))
        ));
    }

    #[test]
    fn resource_embedded_schema_field_denial_precedes_native_conversion() {
        let schema = Schema::new(vec![Field::new("v", DataType::Int64, true)]);
        let encoded = super::super::encode_arrow_schema(&schema);
        let policy = policy("arrow schema field owner");
        assert!(matches!(
            decode_embedded_schema(&policy, &encoded),
            Err(ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "arrow schema field owner",
                ..
            }))
        ));
    }

    #[test]
    fn resource_embedded_schema_native_nested_roundtrip_preserves_metadata() {
        let schema = Schema::new_with_metadata(
            vec![
                Field::new(
                    "nested",
                    DataType::Struct(
                        vec![
                            Field::new(
                                "values",
                                DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
                                true,
                            ),
                            Field::new(
                                "timestamp",
                                DataType::Timestamp(
                                    arrow_schema::TimeUnit::Microsecond,
                                    Some("UTC".into()),
                                ),
                                false,
                            ),
                            Field::new(
                                "category",
                                DataType::Dictionary(
                                    Box::new(DataType::Int32),
                                    Box::new(DataType::Utf8),
                                ),
                                true,
                            ),
                        ]
                        .into(),
                    ),
                    true,
                )
                .with_metadata(HashMap::from([("key".into(), "value".into())])),
            ],
            HashMap::from([("schema-key".into(), "schema-value".into())]),
        );
        let encoded = super::super::encode_arrow_schema(&schema);
        assert_eq!(
            decode_embedded_schema(&policy(""), &encoded).unwrap(),
            schema
        );
    }
}

/// Admit the allocations performed by Field::clone before invoking it. Native
/// DataType clones only allocate recursively for Dictionary's two Boxes;
/// Fields/UnionFields/timezones and retained child Fields are Arc clones.
pub(super) fn clone_field(value: &Field) -> Result<Field> {
    if let Some(policy) = Policy::current() {
        policy.string(value.name().len())?;
        fn data_type(policy: &Policy, value: &DataType, depth: usize) -> Result<()> {
            Policy::check(depth, policy.limits().schema_depth, "arrow cloned field type depth")?;
            if let DataType::Dictionary(key, value) = value {
                policy.reserve(size_of::<DataType>(), "arrow cloned field dictionary")?;
                policy.reserve(size_of::<DataType>(), "arrow cloned field dictionary")?;
                data_type(policy, key, depth + 1)?;
                data_type(policy, value, depth + 1)?;
            }
            Ok(())
        }
        data_type(&policy, value.data_type(), 1)?;
        metadata_tables(&policy, value.metadata().capacity())?;
        for (key, value) in value.metadata() { policy.string(key.len())?; policy.string(value.len())?; }
    }
    Ok(value.clone())
}

pub(super) fn parquet_type_owner(value: crate::schema::types::Type) -> Result<std::sync::Arc<crate::schema::types::Type>> {
    if let Some(policy) = Policy::current() { shared_allocation(&policy, Layout::new::<crate::schema::types::Type>(), "writer schema Type owner")?; }
    Ok(std::sync::Arc::new(value))
}
