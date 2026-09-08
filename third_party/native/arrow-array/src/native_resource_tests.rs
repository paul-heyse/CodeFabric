// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information.
// The ASF licenses this file to you under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::*;
use arrow_data::ArrayData;
use arrow_schema::resource::{
    ResourceAllocationRequest, ResourceOwnerError, RetainedResourceOwner, enter_resource_owner,
};
use arrow_schema::{ArrowError, DataType};
use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ADMITTED: Cell<usize> = const { Cell::new(0) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static EARLY: Cell<usize> = const { Cell::new(0) };
    static SIZES: Cell<[usize;64]> = const {Cell::new([0;64])};
    static COUNT: Cell<usize> = const {Cell::new(0)};
}
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn allocated(bytes: usize) {
    let _ = ACTIVE.try_with(|active| {
        if active.get() {
            COUNT.with(|count| {
                let index = count.get();
                if index < 64 {
                    SIZES.with(|sizes| {
                        let mut v = sizes.get();
                        v[index] = bytes;
                        sizes.set(v);
                    });
                }
                count.set(index + 1);
            });
            let next = ALLOCATED.with(|count| {
                count.set(count.get() + bytes);
                count.get()
            });
            if next > ADMITTED.with(Cell::get) {
                EARLY.with(|early| early.set(early.get() + 1));
            }
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        allocated(bytes);
        unsafe { System.realloc(ptr, layout, bytes) }
    }
}
#[derive(Debug)]
struct Owner {
    remaining: AtomicUsize,
    deny: AtomicBool,
}
impl Owner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            remaining: AtomicUsize::new(64 * 1024 * 1024),
            deny: AtomicBool::new(false),
        })
    }
}
impl RetainedResourceOwner for Owner {
    fn try_reserve_allocation(
        &self,
        request: ResourceAllocationRequest,
    ) -> Result<(), ResourceOwnerError> {
        let fail = || ResourceOwnerError {
            kind: request.kind,
            requested: request.bytes,
            limit: self.remaining.load(Ordering::Relaxed),
        };
        if self.deny.load(Ordering::Relaxed) {
            return Err(fail());
        }
        self.remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(request.bytes)
            })
            .map_err(|_| fail())?;
        ACTIVE.with(|active| {
            if active.get() {
                ADMITTED.with(|count| count.set(count.get() + request.bytes));
            }
        });
        Ok(())
    }
    fn try_adopt(&self, _: Arc<dyn RetainedResourceOwner>) -> Result<(), ResourceOwnerError> {
        Err(ResourceOwnerError {
            kind: "test rejects foreign original",
            requested: 1,
            limit: 0,
        })
    }
    fn record_failure(&self, _: ResourceOwnerError) {}
}
struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}
fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize, usize, usize) {
    ACTIVE.with(|active| assert!(!active.get()));
    COUNT.with(|n| n.set(0));
    SIZES.with(|n| n.set([0; 64]));
    ADMITTED.with(|n| n.set(0));
    ALLOCATED.with(|n| n.set(0));
    EARLY.with(|n| n.set(0));
    ACTIVE.with(|active| active.set(true));
    let stop = Stop;
    let output = operation();
    drop(stop);
    (
        output,
        ADMITTED.with(Cell::get),
        ALLOCATED.with(Cell::get),
        EARLY.with(Cell::get),
    )
}

#[test]
fn every_native_array_family_admits_actual_descriptor_allocations_before_they_happen() {
    for kind in crate::resource_owner_tests::types() {
        for len in [0, 3] {
            let owner = Owner::new();
            let array = {
                let _guard = enter_resource_owner(owner.clone());
                new_null_array(&kind, len)
            };
            let (data, admitted, actual, early) = measure(|| array.try_to_data());
            let data = data.unwrap_or_else(|error| panic!("to_data {kind:?}, {len}: {error}"));
            assert_eq!(
                early, 0,
                "to_data allocation preceded admission: {kind:?}, {len}, {admitted}/{actual}"
            );
            assert!(
                admitted >= actual,
                "to_data {kind:?}: {admitted} < {actual}"
            );
            let (result, admitted, actual, early) = measure(|| try_make_array(data));
            let result = result.unwrap();
            assert_eq!(
                early,
                0,
                "factory allocation preceded admission: {kind:?}, {len}, {admitted}/{actual}, sizes {:?}",
                SIZES.with(|sizes| sizes.get())
            );
            assert!(
                admitted >= actual,
                "factory {kind:?}: {admitted} < {actual}"
            );
            assert_eq!(result.len(), len);
            assert_eq!(result.data_type(), &kind);
        }
    }
}

#[test]
fn concrete_arc_layout_is_exact_for_zero_buffer_null_array() {
    let owner = Owner::new();
    let data = {
        let _guard = enter_resource_owner(owner);
        NullArray::new(17).into_data()
    };
    let (array, admitted, actual, early) = measure(|| try_make_array(data));
    assert_eq!(array.unwrap().len(), 17);
    assert_eq!(early, 0);
    assert_eq!(admitted, actual);
    assert!(actual > std::mem::size_of::<NullArray>());
}

#[test]
fn refusal_is_typed_and_happens_before_any_native_descriptor_allocation() {
    let owner = Owner::new();
    let array = {
        let _guard = enter_resource_owner(owner.clone());
        Int32Array::from(vec![1, 2])
    };
    let data = array.to_data();
    owner.deny.store(true, Ordering::Relaxed);
    let (error, _, actual, _) = measure(|| array.try_to_data());
    assert!(matches!(error, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
    let (error, _, actual, _) = measure(|| try_make_array(data));
    assert!(matches!(error, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
}

#[test]
fn required_owner_cannot_relabel_prebuilt_unowned_input() {
    let array = NullArray::new(2);
    let data = array.to_data();
    let _guard = enter_resource_owner(Owner::new());
    let (result, _, actual, _) = measure(|| array.try_to_data());
    assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
    let (result, _, actual, _) = measure(|| try_make_array(data));
    assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
}

#[derive(Debug)]
struct Custom {
    inner: NullArray,
    calls: Arc<AtomicUsize>,
}
unsafe impl Array for Custom {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn to_data(&self) -> ArrayData {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.to_data()
    }
    fn into_data(self) -> ArrayData {
        self.to_data()
    }
    fn data_type(&self) -> &DataType {
        self.inner.data_type()
    }
    fn slice(&self, offset: usize, len: usize) -> ArrayRef {
        Arc::new(self.inner.slice(offset, len))
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    fn offset(&self) -> usize {
        0
    }
    fn nulls(&self) -> Option<&arrow_buffer::NullBuffer> {
        None
    }
    fn get_buffer_memory_size(&self) -> usize {
        0
    }
    fn get_array_memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
#[test]
fn custom_array_default_rejects_before_calling_unproved_conversion() {
    let calls = Arc::new(AtomicUsize::new(0));
    let array = Custom {
        inner: NullArray::new(0),
        calls: calls.clone(),
    };
    {
        let _guard = enter_resource_owner(Owner::new());
        let (result, _, actual, _) = measure(|| array.try_to_data());
        assert!(result.is_err());
        assert_eq!(actual, 0);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    array.try_to_data().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn nested_custom_child_rejects_before_native_parent_clone_allocates() {
    let calls = Arc::new(AtomicUsize::new(0));
    let owner = Owner::new();
    let array = {
        let _guard = enter_resource_owner(owner);
        StructArray::new(
            vec![arrow_schema::Field::new("custom", DataType::Null, true)].into(),
            vec![Arc::new(Custom {
                inner: NullArray::new(1),
                calls: calls.clone(),
            })],
            None,
        )
    };
    let (result, _, actual, _) = measure(|| array.try_to_data());
    assert!(result.is_err());
    assert_eq!(actual, 0);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

fn check_roundtrip(array: &dyn Array) {
    let (data, admitted, actual, early) = measure(|| array.try_to_data());
    let data = data.unwrap();
    assert_eq!(
        early,
        0,
        "to_data {:?}: {admitted}/{actual}",
        array.data_type()
    );
    assert!(admitted >= actual);
    let (roundtrip, admitted, actual, early) = measure(|| try_make_array(data));
    let roundtrip = roundtrip.unwrap();
    assert_eq!(
        early,
        0,
        "factory {:?}: {admitted}/{actual}, sizes {:?}",
        array.data_type(),
        SIZES.with(Cell::get)
    );
    assert!(admitted >= actual);
    assert_eq!(roundtrip.to_data(), array.to_data());
}

#[test]
fn nonempty_nested_dictionary_view_map_and_sparse_union_descriptors_are_admitted() {
    use crate::types::Int8Type;
    use arrow_schema::{Field, UnionFields};
    let owner = Owner::new();
    let arrays: Vec<ArrayRef> = {
        let _guard = enter_resource_owner(owner);
        let strings: ArrayRef = Arc::new(StringViewArray::from_iter_values([
            "first string longer than the inline view",
            "another string that needs backing",
            "tiny",
        ]));
        assert!(
            !strings
                .as_any()
                .downcast_ref::<StringViewArray>()
                .unwrap()
                .data_buffers()
                .is_empty()
        );
        let keys = Int8Array::from(vec![0, 1, 2]);
        let dictionary: ArrayRef =
            Arc::new(DictionaryArray::<Int8Type>::try_new(keys, strings.clone()).unwrap());
        let nested: ArrayRef = Arc::new(
            DictionaryArray::<Int8Type>::try_new(
                Int8Array::from(vec![2, 1, 0]),
                dictionary.clone(),
            )
            .unwrap(),
        );
        let entries = StructArray::new(
            vec![
                Field::new("key", DataType::Int32, false),
                Field::new("value", strings.data_type().clone(), false),
            ]
            .into(),
            vec![Arc::new(Int32Array::from(vec![4, 5, 6])), strings.clone()],
            None,
        );
        let map: ArrayRef = Arc::new(MapArray::new(
            Arc::new(Field::new("entries", entries.data_type().clone(), false)),
            arrow_buffer::OffsetBuffer::new(vec![0, 1, 3].into()),
            entries,
            None,
            false,
        ));
        let union: ArrayRef = Arc::new(
            UnionArray::try_new(
                UnionFields::try_new(
                    [0, 127],
                    [
                        Field::new("view", strings.data_type().clone(), true),
                        Field::new("dictionary", dictionary.data_type().clone(), true),
                    ],
                )
                .unwrap(),
                vec![0, 127, 0].into(),
                None,
                vec![strings.clone(), dictionary.clone()],
            )
            .unwrap(),
        );
        let structure: ArrayRef = Arc::new(StructArray::new(
            vec![
                Field::new("view", strings.data_type().clone(), true),
                Field::new("dictionary", nested.data_type().clone(), true),
            ]
            .into(),
            vec![strings.clone(), nested.clone()],
            None,
        ));
        vec![strings, dictionary, nested, map, union, structure]
    };
    for array in arrays {
        check_roundtrip(array.as_ref());
    }
}

#[test]
fn sliced_struct_and_fixed_list_factory_clones_are_admitted() {
    use arrow_schema::Field;
    for fixed in [false, true] {
        let owner = Owner::new();
        let data = {
            let _guard = enter_resource_owner(owner);
            let values: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5, 6]));
            let data = if fixed {
                FixedSizeListArray::new(
                    Arc::new(Field::new("value", DataType::Int32, false)),
                    2,
                    values,
                    None,
                )
                .into_data()
            } else {
                StructArray::new(
                    vec![Field::new("value", DataType::Int32, false)].into(),
                    vec![values],
                    None,
                )
                .into_data()
            };
            data.into_builder().offset(1).len(2).build().unwrap()
        };
        let (result, admitted, actual, early) = measure(|| try_make_array(data));
        let result = result.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(early, 0, "fixed={fixed}: {admitted}/{actual}");
        assert!(admitted >= actual);
    }
}

#[test]
fn nested_struct_slice_geometry_refuses_before_upstream_bounds_panic_or_allocation() {
    use arrow_schema::Field;
    let owner = Owner::new();
    let data = {
        let _guard = enter_resource_owner(owner);
        let inner: ArrayRef = Arc::new(StructArray::new(
            vec![Field::new("value", DataType::Int32, false)].into(),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3, 4]))],
            None,
        ));
        StructArray::new(
            vec![Field::new("inner", inner.data_type().clone(), false)].into(),
            vec![inner],
            None,
        )
        .into_data()
        .slice(1, 2)
    };
    // The existing native factory slices the already-sliced Struct child again.
    // The fallible preflight must detect the nested out-of-bounds geometry.
    let (result, _, actual, _) = measure(|| try_make_array(data));
    assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
}

#[test]
fn original_owner_survives_try_conversions_without_required_thread_scope() {
    let owner = Owner::new();
    let weak = Arc::downgrade(&owner);
    let array = {
        let _guard = enter_resource_owner(owner.clone());
        Int32Array::from(vec![1, 2])
    };
    drop(owner);
    let data = array.try_to_data().unwrap();
    drop(array);
    assert!(weak.upgrade().is_some());
    let array = try_make_array(data).unwrap();
    assert!(weak.upgrade().is_some());
    drop(array);
    assert!(weak.upgrade().is_none());
}

#[test]
fn selected_builder_finish_admits_actual_immutable_and_descriptor_allocations() {
    use crate::builder::*;
    use crate::types::{Decimal128Type, TimestampMicrosecondType};
    let owner = Owner::new();
    let _guard = enter_resource_owner(owner);
    let mut numbers = Int32Builder::new();
    numbers.append_value(7);
    numbers.append_null();
    let mut booleans = BooleanBuilder::new();
    booleans.append_value(true);
    booleans.append_null();
    let mut strings = StringBuilder::new();
    strings.append_value("native string");
    strings.append_null();
    let mut binary = LargeBinaryBuilder::new();
    binary.append_value(b"binary");
    binary.append_null();
    let mut decimal = PrimitiveBuilder::<Decimal128Type>::new();
    decimal.append_value(456);
    decimal.append_null();
    let mut time = PrimitiveBuilder::<TimestampMicrosecondType>::new();
    time.append_value(789);
    time.append_null();
    let mut list = ListBuilder::new(StringBuilder::new());
    list.values().append_value("entry");
    list.append(true);
    list.append(false);
    let mut map = MapBuilder::new(
        None,
        StringBuilder::new(),
        ListBuilder::new(Int32Builder::new()),
    );
    map.keys().append_value("key");
    map.values().values().append_value(14);
    map.values().append(true);
    map.append(true).unwrap();
    map.append(false).unwrap();
    let mut structure = StructBuilder::new(
        vec![arrow_schema::Field::new("field", DataType::Int32, true)],
        vec![Box::new(numbers)],
    );
    structure.append(true);
    structure.append(true);
    let mut null = NullBuilder::new();
    null.append_nulls(2);
    let mut builders: Vec<Box<dyn ArrayBuilder>> = vec![
        Box::new(booleans),
        Box::new(strings),
        Box::new(binary),
        Box::new(decimal),
        Box::new(time),
        Box::new(list),
        Box::new(map),
        Box::new(structure),
        Box::new(null),
    ];
    for builder in &mut builders {
        let expected = builder.finish_cloned();
        let (result, admitted, actual, early) = measure(|| builder.try_finish());
        let result = result.unwrap();
        assert_eq!(
            early,
            0,
            "finish {:?}: {admitted}/{actual}, sizes {:?}",
            expected.data_type(),
            SIZES.with(Cell::get)
        );
        assert!(admitted >= actual);
        assert_eq!(result.to_data(), expected.to_data());
        // Empty reuse still constructs immutable buffer owners and offset resets.
        let (result, admitted, actual, early) = measure(|| builder.try_finish());
        result.unwrap();
        assert_eq!(
            early,
            0,
            "empty finish {:?}: {admitted}/{actual}, sizes {:?}",
            expected.data_type(),
            SIZES.with(Cell::get)
        );
        assert!(admitted >= actual);
    }
}

#[test]
fn selected_builder_denial_happens_before_finalization_and_preserves_payload() {
    use crate::builder::*;
    let owner = Owner::new();
    let _guard = enter_resource_owner(owner.clone());
    let mut builder = StringBuilder::new();
    builder.append_value("kept payload");
    owner.deny.store(true, Ordering::Relaxed);
    let (result, _, actual, _) = measure(|| ArrayBuilder::try_finish(&mut builder));
    assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
    assert_eq!(builder.len(), 1);
    owner.deny.store(false, Ordering::Relaxed);
    let output = ArrayBuilder::try_finish(&mut builder).unwrap();
    assert_eq!(
        output
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .value(0),
        "kept payload"
    );
}

#[test]
fn impossible_descriptor_layout_denies_without_calling_even_an_approving_owner() {
    let owner = Owner::new();
    let before = owner.remaining.load(Ordering::Relaxed);
    let erased: Arc<dyn RetainedResourceOwner> = owner.clone();
    let (result, _, actual, _) = measure(|| {
        crate::native_resource::vector::<ArrayData>(&erased, usize::MAX, "overflow probe")
    });
    assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
    assert_eq!(actual, 0);
    assert_eq!(owner.remaining.load(Ordering::Relaxed), before);
}

#[test]
fn unknown_builder_rejects_required_owner_before_calling_finish() {
    use crate::builder::ArrayBuilder;
    #[derive(Debug)]
    struct CustomBuilder {
        calls: usize,
    }
    impl ArrayBuilder for CustomBuilder {
        fn len(&self) -> usize {
            0
        }
        fn finish(&mut self) -> ArrayRef {
            self.calls += 1;
            Arc::new(NullArray::new(0))
        }
        fn finish_cloned(&self) -> ArrayRef {
            Arc::new(NullArray::new(0))
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
        fn into_box_any(self: Box<Self>) -> Box<dyn Any> {
            self
        }
    }
    let mut builder = CustomBuilder { calls: 0 };
    {
        let _guard = enter_resource_owner(Owner::new());
        let (result, _, actual, _) = measure(|| builder.try_finish());
        assert!(matches!(result, Err(ArrowError::ResourceOwnerError(_))));
        assert_eq!(actual, 0);
        assert_eq!(builder.calls, 0);
    }
    assert_eq!(builder.try_finish().unwrap().len(), 0);
    assert_eq!(builder.calls, 1);
}

#[test]
fn borrowed_native_buffer_visitation_matches_actual_data_backing_without_allocating() {
    fn count(data: &ArrayData) -> usize {
        data.buffers().len()
            + usize::from(data.nulls().is_some())
            + data.child_data().iter().map(count).sum::<usize>()
    }
    for kind in crate::resource_owner_tests::types() {
        for len in [0, 3] {
            let array = new_null_array(&kind, len);
            let expected = count(&array.to_data());
            let mut seen = 0;
            let (result, _, actual, _) = measure(|| {
                array.try_visit_buffers(&mut |_| {
                    seen += 1;
                    Ok(())
                })
            });
            result.unwrap();
            assert_eq!(actual, 0, "visitor allocated for {kind:?}");
            assert_eq!(seen, expected, "visitor missed backing for {kind:?}");
        }
    }
}

#[test]
fn borrowed_visitor_stops_on_failure_and_never_calls_custom_to_data() {
    let array = StringArray::from(vec![Some("value"), None]);
    let mut seen = 0;
    let (result, _, actual, _) = measure(|| {
        array.try_visit_buffers(&mut |_| {
            seen += 1;
            Err(ResourceOwnerError {
                kind: "visitor refusal",
                requested: 1,
                limit: 0,
            }
            .into())
        })
    });
    assert!(result.is_err());
    assert_eq!(seen, 1);
    assert_eq!(actual, 0);
    let calls = Arc::new(AtomicUsize::new(0));
    let custom = Custom {
        inner: NullArray::new(1),
        calls: calls.clone(),
    };
    let (result, _, actual, _) = measure(|| custom.try_visit_buffers(&mut |_| Ok(())));
    assert!(result.is_err());
    assert_eq!(actual, 0);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn consuming_record_batch_transfers_actual_columns_without_allocating() {
    for len in [0, 7] {
        let owner = Owner::new();
        let weak = Arc::downgrade(&owner);
        let guard = enter_resource_owner(owner.clone());
        let columns: Vec<ArrayRef> = vec![Arc::new(Int32Array::from_iter_values(0..len as i32))];
        let pointer = columns.as_ptr();
        let schema = Arc::new(arrow_schema::Schema::new(vec![arrow_schema::Field::new(
            "value",
            DataType::Int32,
            false,
        )]));
        let batch = RecordBatch::try_new(schema, columns).unwrap();
        let (array, _, actual, _) = measure(|| StructArray::from(batch));
        assert_eq!(actual, 0);
        assert_eq!(array.columns().as_ptr(), pointer);
        assert_eq!(array.len(), len);
        drop(guard);
        drop(owner);
        assert!(weak.upgrade().is_some());
        drop(array);
        assert!(weak.upgrade().is_none());
    }
}
