// Licensed to the Apache Software Foundation (ASF) under one or more contributor
// license agreements. See NOTICE for additional information. Licensed under the
// Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy at
// http://www.apache.org/licenses/LICENSE-2.0 . Unless required by applicable law
// or agreed in writing, software is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

//! Admission for the selected native encoder. Vec payloads are admitted before
//! reserve; this is independent of attaching an owner to the encoded result.
use arrow_schema::{
    ArrowError,
    resource::{ResourceAllocationRequest, ResourceOwnerError, current_resource_owner},
};
use std::{alloc::Layout, cell::Cell};

pub(super) fn failure(kind: &'static str, requested: usize, limit: usize) -> ArrowError {
    let error = ResourceOwnerError {
        kind,
        requested,
        limit,
    };
    if let Some(owner) = current_resource_owner() {
        owner.record_failure(error);
    }
    error.into()
}
pub(super) fn governed() -> bool {
    crate::resource::ReaderResourcePolicy::current().is_some() || current_resource_owner().is_some()
}
pub(super) fn add(left: usize, right: usize) -> Result<usize, ArrowError> {
    left.checked_add(right)
        .ok_or_else(|| failure("JSON encoded geometry", usize::MAX, isize::MAX as usize))
}
pub(super) fn mul(left: usize, right: usize) -> Result<usize, ArrowError> {
    left.checked_mul(right)
        .ok_or_else(|| failure("JSON encoded geometry", usize::MAX, isize::MAX as usize))
}
pub(super) fn reserve(layout: Layout, kind: &'static str) -> Result<(), ArrowError> {
    if layout.size() == 0 {
        return Ok(());
    }
    if let Some(policy) = crate::resource::ReaderResourcePolicy::current() {
        policy
            .reserve(layout.size(), kind)
            .map_err(|error| failure(error.kind, error.requested, error.limit))
    } else if let Some(owner) = current_resource_owner() {
        owner
            .try_reserve_allocation(ResourceAllocationRequest {
                bytes: layout.size(),
                alignment: layout.align(),
                kind,
            })
            .map_err(|error| failure(error.kind, error.requested, error.limit))
    } else {
        Ok(())
    }
}
pub(super) fn bytes(bytes: usize, kind: &'static str) -> Result<(), ArrowError> {
    reserve(
        Layout::array::<u8>(bytes).map_err(|_| failure(kind, bytes, isize::MAX as usize))?,
        kind,
    )
}
pub(super) fn boxed<T>(value: T) -> Result<Box<T>, ArrowError> {
    reserve(Layout::new::<T>(), "JSON encoder owner")?;
    Ok(Box::new(value))
}
pub(super) fn vec<T>(count: usize, kind: &'static str) -> Result<Vec<T>, ArrowError> {
    if let Some(policy) = crate::resource::ReaderResourcePolicy::current() {
        let limit = policy.limits().collection_entries;
        if count > limit {
            return Err(failure(kind, count, limit));
        }
    }
    let layout =
        Layout::array::<T>(count).map_err(|_| failure(kind, count, isize::MAX as usize))?;
    reserve(layout, kind)?;
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| failure(kind, layout.size(), 0))?;
    Ok(value)
}
pub(super) fn output(value: &mut Vec<u8>, additional: usize) -> Result<(), ArrowError> {
    let required = add(value.len(), additional)?;
    let limit = crate::resource::ReaderResourcePolicy::current()
        .map_or(isize::MAX as usize, |p| p.limits().string_bytes);
    if required > limit {
        return Err(failure("JSON encoded bytes", required, limit));
    }
    if required > value.capacity() {
        let capacity = required.max(value.capacity().saturating_mul(2)).min(limit);
        bytes(capacity, "JSON encoded bytes")?;
        value
            .try_reserve_exact(capacity - value.len())
            .map_err(|_| failure("JSON encoded bytes", capacity, 0))?;
    }
    Ok(())
}
thread_local! {static DEPTH:Cell<usize>=const{Cell::new(0)};}
pub(super) struct Depth(usize);
impl Depth {
    pub(super) fn enter() -> Result<Self, ArrowError> {
        let old = DEPTH.with(Cell::get);
        let next = add(old, 1)?;
        if governed() {
            let limit =
                crate::resource::ReaderResourcePolicy::current().map_or(64, |p| p.limits().nesting);
            if next > limit {
                return Err(failure("JSON encoder nesting", next, limit));
            }
        }
        DEPTH.with(|depth| depth.set(next));
        Ok(Self(old))
    }
}
impl Drop for Depth {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(self.0));
    }
}

pub(super) fn string_len(value: &str) -> Result<usize, ArrowError> {
    use serde_core::Serializer;
    // Every UTF-8 byte emits at most a six-byte JSON escape; prove counting
    // cannot overflow before passing the infallible counting sink to serde.
    add(mul(value.len(), 6)?, 2)?;
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
            self.0 += value.len();
            Ok(value.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    serde_json::Serializer::new(&mut count)
        .serialize_str(value)
        .expect("counting sink cannot fail");
    Ok(count.0)
}

/// Native formatter boxes store State, an array reference and the null string.
/// This repr-C layout dominates Rust's possible field reordering without using
/// a coefficient based on the logical Arrow type.
pub(super) fn formatter<State>() -> Result<(), ArrowError> {
    let layout = Layout::new::<State>()
        .extend(Layout::new::<&()>())
        .and_then(|(layout, _)| layout.extend(Layout::new::<&str>()))
        .map_err(|_| failure("JSON native formatter", usize::MAX, isize::MAX as usize))?
        .0
        .pad_to_align();
    reserve(layout, "JSON native formatter")
}
/// Native decimal format_decimal first materializes its bounded integer then
/// formats/truncates/pads it. Timestamp UTC/offset formatting can build an
/// RFC3339 String; invalid native values format a bounded type/value diagnostic.
/// The caller admits both counting and encoding passes before the first pass.
pub(super) fn formatter_scratch(kind: &arrow_schema::DataType) -> Result<(), ArrowError> {
    use arrow_schema::DataType::*;
    let size = match kind {
        Decimal32(_, scale) | Decimal64(_, scale) | Decimal128(_, scale) | Decimal256(_, scale) => {
            // i256 has at most 78 digits plus sign; padding adds <=128 bytes.
            let output = add(81, usize::from(scale.unsigned_abs()))?;
            mul(add(mul(add(output, 128)?, 4)?, mul(80, 4)?)?, 2)?
        }
        Timestamp(_, zone) => mul(mul(add(zone.as_ref().map_or(0, |z| z.len()), 256)?, 4)?, 2)?,
        _ => mul(mul(256, 4)?, 2)?,
    };
    bytes(size, "JSON formatter scratch")
}
pub(super) fn nulls(len: usize) -> Result<(), ArrowError> {
    let size = add(len, 7)? / 8;
    let layout = arrow_buffer::MutableBuffer::try_capacity_layout(size)
        .map_err(|_| failure("JSON null encoder validity", size, isize::MAX as usize))?;
    reserve(layout, "JSON null encoder validity")?;
    reserve(
        arrow_buffer::allocation_owner_layout(),
        "JSON null encoder validity owner",
    )
}
