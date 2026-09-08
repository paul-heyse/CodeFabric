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

//! Exact preallocation for the selected fallible native transform path.
//! Synchronous only: these guards never cross an await or escape a call.

use arrow_buffer::{BufferAllocationError, MutableBuffer};
use arrow_schema::resource::{ResourceAllocationRequest, ResourceOwnerError, ResourceOwnerHandle};
use arrow_schema::{ArrowError, DataType};
use std::alloc::Layout;
use std::cell::RefCell;

thread_local! {
    static OWNER: RefCell<ResourceOwnerHandle> = const { RefCell::new(ResourceOwnerHandle::empty()) };
}

pub(super) struct Scope(
    ResourceOwnerHandle,
    std::marker::PhantomData<std::rc::Rc<()>>,
);
impl Drop for Scope {
    fn drop(&mut self) {
        OWNER.with(|owner| {
            *owner.borrow_mut() = std::mem::replace(&mut self.0, ResourceOwnerHandle::empty())
        });
    }
}
pub(super) fn enter(owner: ResourceOwnerHandle) -> Scope {
    Scope(
        OWNER.with(|slot| slot.replace(owner)),
        std::marker::PhantomData,
    )
}
pub(super) fn current() -> ResourceOwnerHandle {
    OWNER.with(|owner| owner.borrow().clone())
}

pub(super) fn validate_owner(owner: &ResourceOwnerHandle) -> Result<(), ArrowError> {
    let required = ResourceOwnerHandle::capture();
    if !required.is_empty() && !required.same_owner(owner) {
        return Err(ResourceOwnerError {
            kind: "transform belongs to another allocation scope",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    Ok(())
}

pub(super) fn overflow(kind: &'static str) -> ArrowError {
    ResourceOwnerError {
        kind,
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}
pub(super) fn count_bytes(
    count: usize,
    width: usize,
    kind: &'static str,
) -> Result<usize, ArrowError> {
    count.checked_mul(width).ok_or_else(|| overflow(kind))
}
pub(super) fn admit(layout: Layout, kind: &'static str) -> Result<(), ArrowError> {
    if layout.size() != 0 {
        if let Some(owner) = current().owner() {
            owner.try_reserve_allocation(ResourceAllocationRequest {
                bytes: layout.size(),
                alignment: layout.align(),
                kind,
            })?;
        }
    }
    Ok(())
}
pub(super) fn vec<T>(capacity: usize, kind: &'static str) -> Result<Vec<T>, ArrowError> {
    let layout = Layout::array::<T>(capacity).map_err(|_| overflow(kind))?;
    admit(layout, kind)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ResourceOwnerError {
            kind,
            requested: layout.size(),
            limit: layout.size(),
        })?;
    Ok(values)
}
pub(super) fn boxed<T>(value: T, kind: &'static str) -> Result<Box<T>, ArrowError> {
    admit(Layout::new::<T>(), kind)?;
    Ok(Box::new(value))
}
fn allocation_error(error: BufferAllocationError) -> ArrowError {
    ResourceOwnerError {
        kind: "native mutable allocation",
        requested: error.requested,
        limit: isize::MAX as usize,
    }
    .into()
}
pub(super) fn buffer(capacity: usize, kind: &'static str) -> Result<MutableBuffer, ArrowError> {
    let layout = MutableBuffer::try_capacity_layout(capacity).map_err(allocation_error)?;
    admit(layout, kind)?;
    MutableBuffer::try_with_capacity(capacity).map_err(allocation_error)
}
pub(super) fn zeroed(len: usize, kind: &'static str) -> Result<MutableBuffer, ArrowError> {
    let layout =
        Layout::from_size_align(len, arrow_buffer::alloc::ALIGNMENT).map_err(|_| overflow(kind))?;
    admit(layout, kind)?;
    MutableBuffer::try_from_len_zeroed(len).map_err(allocation_error)
}
pub(super) fn reserve(
    buffer: &mut MutableBuffer,
    additional: usize,
    kind: &'static str,
) -> Result<(), ArrowError> {
    if let Some(layout) = buffer
        .try_reserve_layout(additional)
        .map_err(allocation_error)?
    {
        admit(layout, kind)?;
        buffer.try_reserve(additional).map_err(allocation_error)?;
    }
    Ok(())
}
pub(super) fn resize(
    buffer: &mut MutableBuffer,
    len: usize,
    value: u8,
    kind: &'static str,
) -> Result<(), ArrowError> {
    if len > buffer.len() {
        reserve(buffer, len - buffer.len(), kind)?;
    }
    buffer.try_resize(len, value).map_err(allocation_error)
}

pub(super) fn finish_buffer(buffer: MutableBuffer) -> Result<arrow_buffer::Buffer, ArrowError> {
    #[cfg(not(feature = "pool"))]
    if !current().is_empty() {
        return Err(ResourceOwnerError {
            kind: "transform backing lifetime requires pool",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    admit(
        arrow_buffer::allocation_owner_layout(),
        "transform immutable buffer owner",
    )?;
    let buffer: arrow_buffer::Buffer = buffer.into();
    #[cfg(feature = "pool")]
    if !current().is_empty() {
        retain_buffer(&buffer)?;
    }
    Ok(buffer)
}

#[cfg(feature = "pool")]
fn retain_buffer(buffer: &arrow_buffer::Buffer) -> Result<(), ArrowError> {
    retain_buffer_with_owner(buffer, current())
}

#[cfg(feature = "pool")]
pub(super) fn retain_buffer_with_owner(
    buffer: &arrow_buffer::Buffer,
    owner: ResourceOwnerHandle,
) -> Result<(), ArrowError> {
    use arrow_buffer::{MemoryPool, MemoryReservation};
    use std::sync::Mutex;
    #[derive(Debug)]
    struct Lifetime {
        size: usize,
        _owner: ResourceOwnerHandle,
    }
    impl MemoryReservation for Lifetime {
        fn size(&self) -> usize {
            self.size
        }
        fn resize(&mut self, _: usize) {}
    }
    #[derive(Debug)]
    struct Failed;
    impl MemoryReservation for Failed {
        fn size(&self) -> usize {
            0
        }
        fn resize(&mut self, _: usize) {}
    }
    #[derive(Debug)]
    struct Pool {
        owner: ResourceOwnerHandle,
        failure: Mutex<Option<ResourceOwnerError>>,
    }
    impl MemoryPool for Pool {
        fn reserve(&self, size: usize) -> Box<dyn MemoryReservation> {
            let layout = Layout::new::<Lifetime>();
            if let Some(owner) = self.owner.owner() {
                match owner.try_reserve_allocation(ResourceAllocationRequest {
                    bytes: layout.size(),
                    alignment: layout.align(),
                    kind: "transform backing lifetime",
                }) {
                    Ok(()) => {
                        return Box::new(Lifetime {
                            size,
                            _owner: self.owner.clone(),
                        });
                    }
                    Err(error) => *self.failure.lock().unwrap() = Some(error),
                }
            }
            Box::new(Failed)
        }
        fn available(&self) -> isize {
            0
        }
        fn used(&self) -> usize {
            0
        }
        fn capacity(&self) -> usize {
            0
        }
    }
    let pool = Pool {
        owner,
        failure: Mutex::new(None),
    };
    buffer.claim_if_unclaimed(&pool);
    if let Some(error) = pool.failure.into_inner().unwrap() {
        return Err(error.into());
    }
    Ok(())
}
pub(super) fn data_type_clone(kind: &DataType) -> Result<DataType, ArrowError> {
    if let DataType::Dictionary(key, value) = kind {
        admit(Layout::new::<DataType>(), "transform dictionary key type")?;
        admit(Layout::new::<DataType>(), "transform dictionary value type")?;
        return Ok(DataType::Dictionary(
            Box::new(data_type_clone(key)?),
            Box::new(data_type_clone(value)?),
        ));
    }
    Ok(kind.clone())
}

pub(super) fn clone_data(data: &crate::ArrayData) -> Result<crate::ArrayData, ArrowError> {
    let mut owner = data.resource_owner().clone();
    owner.try_capture_current()?;
    let mut buffers = vec(data.buffers().len(), "transform cloned buffer descriptors")?;
    buffers.extend_from_slice(data.buffers());
    let mut children = vec(
        data.child_data().len(),
        "transform cloned child descriptors",
    )?;
    for child in data.child_data() {
        children.push(clone_data(child)?);
    }
    let builder =
        crate::ArrayDataBuilder::new_with_resource_owner(data_type_clone(data.data_type())?, owner)
            .len(data.len())
            .offset(data.offset())
            .nulls(data.nulls().cloned())
            .buffers(buffers)
            .child_data(children);
    // SAFETY: all logical metadata and buffers are an unchanged clone of the
    // original native ArrayData, whose existing validity contract is preserved.
    Ok(unsafe { builder.build_unchecked() })
}

pub(super) fn new_buffers(
    kind: &DataType,
    capacity: usize,
) -> Result<[MutableBuffer; 2], ArrowError> {
    use arrow_schema::UnionMode;
    let empty = || MutableBuffer::new(0);
    let values = |width| {
        buffer(
            count_bytes(capacity, width, "transform value geometry")?,
            "transform value backing",
        )
    };
    let offsets = |width| -> Result<MutableBuffer, ArrowError> {
        let count = capacity
            .checked_add(1)
            .ok_or_else(|| overflow("transform offset geometry"))?;
        let mut out = buffer(
            count_bytes(count, width, "transform offset geometry")?,
            "transform offset backing",
        )?;
        if width == 4 {
            out.push(0i32);
        } else {
            out.push(0i64);
        }
        Ok(out)
    };
    Ok(match kind {
        DataType::Null
        | DataType::Struct(_)
        | DataType::FixedSizeList(_, _)
        | DataType::RunEndEncoded(_, _) => [empty(), empty()],
        DataType::Boolean => [
            buffer(
                capacity / 8 + usize::from(capacity % 8 != 0),
                "transform boolean backing",
            )?,
            empty(),
        ],
        DataType::Utf8 | DataType::Binary => [offsets(4)?, values(1)?],
        DataType::LargeUtf8 | DataType::LargeBinary => [offsets(8)?, values(1)?],
        DataType::BinaryView | DataType::Utf8View => [values(16)?, empty()],
        DataType::List(_) | DataType::Map(_, _) => [offsets(4)?, empty()],
        DataType::LargeList(_) => [offsets(8)?, empty()],
        DataType::ListView(_) => [values(4)?, values(4)?],
        DataType::LargeListView(_) => [values(8)?, values(8)?],
        DataType::FixedSizeBinary(width) => [
            values(usize::try_from(*width).map_err(|_| overflow("negative fixed width"))?)?,
            empty(),
        ],
        DataType::Dictionary(key, _) => [
            values(
                key.primitive_width()
                    .ok_or_else(|| overflow("nonprimitive dictionary key"))?,
            )?,
            empty(),
        ],
        DataType::Union(_, UnionMode::Sparse) => [values(1)?, empty()],
        DataType::Union(_, UnionMode::Dense) => [values(1)?, values(4)?],
        primitive => [
            values(
                primitive
                    .primitive_width()
                    .ok_or_else(|| overflow("unsupported primitive width"))?,
            )?,
            empty(),
        ],
    })
}

#[cfg(all(test, feature = "pool"))]
mod tests {
    use super::*;
    use crate::{ArrayData, transform::MutableArrayData};
    use arrow_buffer::Buffer;
    use arrow_schema::resource::{RetainedResourceOwner, enter_resource_owner};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    #[derive(Debug)]
    struct Owner {
        deny: &'static str,
        requests: Mutex<Vec<ResourceAllocationRequest>>,
        failed: AtomicBool,
    }
    impl RetainedResourceOwner for Owner {
        fn try_reserve_allocation(
            &self,
            request: ResourceAllocationRequest,
        ) -> Result<(), ResourceOwnerError> {
            let mut requests = self.requests.lock().unwrap();
            assert!(requests.len() < requests.capacity());
            requests.push(request);
            if request.kind == self.deny {
                return Err(ResourceOwnerError {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
            }
            Ok(())
        }
        fn try_adopt(&self, _: Arc<dyn RetainedResourceOwner>) -> Result<(), ResourceOwnerError> {
            Err(ResourceOwnerError {
                kind: "test foreign owner",
                requested: 1,
                limit: 0,
            })
        }
        fn record_failure(&self, _: ResourceOwnerError) {
            self.failed.store(true, Ordering::Release);
        }
    }
    fn owner(deny: &'static str) -> Arc<Owner> {
        Arc::new(Owner {
            deny,
            requests: Mutex::new(Vec::with_capacity(256)),
            failed: AtomicBool::new(false),
        })
    }
    fn integers(count: usize) -> ArrayData {
        ArrayData::builder(DataType::Int32)
            .len(count)
            .add_buffer(Buffer::from_vec((0..count as i32).collect::<Vec<_>>()))
            .build()
            .unwrap()
    }
    fn denied(error: ArrowError, kind: &'static str) {
        assert!(matches!(error, ArrowError::ResourceOwnerError(error) if error.kind == kind));
    }

    #[test]
    fn resource_fresh_buffer_helper_retains_original_claim_without_replacement() {
        let original = owner("");
        let weak = Arc::downgrade(&original);
        let buffer = Buffer::from_vec(vec![1i32, 2]);
        {
            let _guard = enter_resource_owner(original.clone());
            crate::try_retain_fresh_buffer(&buffer).unwrap();
        }
        let other = owner("transform backing lifetime");
        {
            let _guard = enter_resource_owner(other.clone());
            // Existing receipt survives even an owner that refuses a new claim.
            crate::try_retain_fresh_buffer(&buffer.slice(4)).unwrap();
        }
        assert!(other.requests.lock().unwrap().is_empty());
        drop(original);
        assert!(weak.upgrade().is_some());
        drop(buffer);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn resource_fresh_empty_buffer_and_denial_are_typed() {
        let original = owner("");
        let weak = Arc::downgrade(&original);
        let buffer = Buffer::from_vec(Vec::<u8>::new());
        {
            let _guard = enter_resource_owner(original.clone());
            crate::try_retain_fresh_buffer(&buffer).unwrap();
        }
        drop(original);
        assert!(weak.upgrade().is_some());
        drop(buffer);
        assert!(weak.upgrade().is_none());
        let owner = owner("transform backing lifetime");
        let _guard = enter_resource_owner(owner);
        denied(
            crate::try_retain_fresh_buffer(&Buffer::from_vec(vec![1u8])).unwrap_err(),
            "transform backing lifetime",
        );
    }

    #[test]
    fn resource_transform_constructor_denies_before_native_backing_and_closure_allocations() {
        let input = integers(4);
        for kind in ["transform value backing", "transform value callback"] {
            let owner = owner(kind);
            let _guard = enter_resource_owner(owner.clone());
            denied(
                MutableArrayData::try_new(&[&input], false, 4).unwrap_err(),
                kind,
            );
            let requests = owner.requests.lock().unwrap();
            assert_eq!(requests.last().unwrap().kind, kind);
            if kind == "transform value backing" {
                assert_eq!(requests.len(), 2);
            }
        }
    }

    #[test]
    fn resource_transform_growth_denial_is_terminal_and_does_not_export_partial_output() {
        let input = integers(4);
        let owner = owner("transform primitive growth");
        let _guard = enter_resource_owner(owner.clone());
        let mut mutable = MutableArrayData::try_new(&[&input], false, 0).unwrap();
        denied(
            mutable.try_extend(0, 0, 4).unwrap_err(),
            "transform primitive growth",
        );
        assert!(owner.failed.load(Ordering::Acquire));
        assert_eq!(mutable.data.buffer1.capacity(), 0);
        denied(
            mutable.try_extend(0, 0, 0).unwrap_err(),
            "transform primitive growth",
        );
        denied(
            mutable.try_freeze().unwrap_err(),
            "transform primitive growth",
        );
    }

    #[test]
    fn resource_transform_full_replacement_and_bare_buffer_clone_keep_original_owner() {
        let input = integers(20);
        let owner = owner("");
        let weak = Arc::downgrade(&owner);
        let guard = enter_resource_owner(owner.clone());
        let mut mutable = MutableArrayData::try_new(&[&input], false, 1).unwrap();
        mutable.try_extend(0, 0, 20).unwrap();
        let output = mutable.try_freeze().unwrap();
        let cloned_buffer = output.buffers()[0].clone();
        assert_eq!(
            cloned_buffer.typed_data::<i32>(),
            input.buffers()[0].typed_data::<i32>()
        );
        let requests = owner.requests.lock().unwrap();
        assert!(
            requests
                .iter()
                .any(|r| r.kind == "transform value backing" && r.bytes == 64)
        );
        assert!(
            requests
                .iter()
                .any(|r| r.kind == "transform primitive growth" && r.bytes == 128)
        );
        drop(requests);
        drop(output);
        drop(guard);
        drop(owner);
        assert!(weak.upgrade().is_some());
        drop(cloned_buffer);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn resource_transform_nested_list_growth_and_null_padding_preserve_native_values() {
        let input = ArrayData::builder(DataType::List(Arc::new(arrow_schema::Field::new(
            "item",
            DataType::Int32,
            true,
        ))))
        .len(2)
        .add_buffer(Buffer::from_vec(vec![0i32, 2, 4]))
        .add_child_data(integers(4))
        .build()
        .unwrap();
        let owner = owner("");
        let _guard = enter_resource_owner(owner.clone());
        let mut mutable = MutableArrayData::try_new(&[&input], true, 0).unwrap();
        mutable.try_extend(0, 0, 1).unwrap();
        mutable.try_extend_nulls(1).unwrap();
        let result = mutable.try_freeze().unwrap();
        assert_eq!(result.buffers()[0].typed_data::<i32>(), [0, 2, 2]);
        assert_eq!(
            result.child_data()[0].buffers()[0].typed_data::<i32>(),
            [0, 1]
        );
        assert!(result.is_null(1));
        assert_eq!(result.len(), 2);
        assert!(
            owner
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r.kind == "transform child descriptors")
        );
    }

    #[test]
    fn resource_transform_rejects_foreign_worker_before_growth() {
        let input = integers(1);
        let original = owner("");
        let mut mutable = {
            let _guard = enter_resource_owner(original.clone());
            MutableArrayData::try_new(&[&input], false, 0).unwrap()
        };
        let other = owner("");
        let _guard = enter_resource_owner(other.clone());
        denied(
            mutable.try_extend(0, 0, 1).unwrap_err(),
            "transform belongs to another allocation scope",
        );
        assert!(other.requests.lock().unwrap().is_empty());
        assert_eq!(mutable.data.buffer1.capacity(), 0);
    }

    #[test]
    fn resource_transform_final_owner_denial_returns_typed_error_without_panic() {
        let input = integers(1);
        let owner = owner("transform backing lifetime");
        let _guard = enter_resource_owner(owner);
        let mut mutable = MutableArrayData::try_new(&[&input], false, 1).unwrap();
        mutable.try_extend(0, 0, 1).unwrap();
        denied(
            mutable.try_freeze().unwrap_err(),
            "transform backing lifetime",
        );
    }
}
