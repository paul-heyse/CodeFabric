// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use crate::{
    Array, ArrayRef, BinaryArray, BinaryViewArray, Int32Array, Int64Array, NullArray, RecordBatch,
    RecordBatchOptions, RunArray, StringArray, StringViewArray, StructArray, make_array,
    new_null_array, types::Int32Type,
};
use arrow_data::ArrayDataBuilder;
use arrow_schema::resource::{
    ResourceOwnerError, ResourceOwnerHandle, RetainedResourceOwner, enter_resource_owner,
};
use arrow_schema::{DataType, Field, Fields, Schema, UnionFields, UnionMode};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Owner {
    // A finite registry makes the lifetime oracle independent of an unbounded
    // test-only chain. These tests assert native roots, not memory quantities.
    originals: Mutex<[Option<Arc<dyn RetainedResourceOwner>>; 4]>,
    failure: Mutex<Option<ResourceOwnerError>>,
    reject: bool,
}

impl RetainedResourceOwner for Owner {
    fn try_adopt(
        &self,
        original: Arc<dyn RetainedResourceOwner>,
    ) -> Result<(), ResourceOwnerError> {
        let error = ResourceOwnerError {
            kind: "array owner adoption",
            requested: 1,
            limit: 0,
        };
        if self.reject {
            return Err(error);
        }
        let mut owners = self.originals.lock().unwrap();
        if owners
            .iter()
            .flatten()
            .any(|owner| Arc::ptr_eq(owner, &original))
        {
            return Ok(());
        }
        let slot = owners
            .iter_mut()
            .find(|owner| owner.is_none())
            .ok_or(error)?;
        *slot = Some(original);
        Ok(())
    }

    fn record_failure(&self, error: ResourceOwnerError) {
        self.failure.lock().unwrap().get_or_insert(error);
    }
}

pub(crate) fn types() -> Vec<DataType> {
    let value = Arc::new(Field::new("value", DataType::Int32, true));
    let entries = Arc::new(Field::new(
        "entries",
        DataType::Struct(
            vec![
                Field::new("key", DataType::Utf8, false),
                Field::new("value", DataType::Int32, true),
            ]
            .into(),
        ),
        false,
    ));
    vec![
        DataType::Null,
        DataType::Boolean,
        DataType::Int32,
        DataType::Utf8,
        DataType::LargeUtf8,
        DataType::Binary,
        DataType::LargeBinary,
        DataType::Utf8View,
        DataType::BinaryView,
        DataType::FixedSizeBinary(4),
        DataType::List(value.clone()),
        DataType::LargeList(value.clone()),
        DataType::ListView(value.clone()),
        DataType::LargeListView(value.clone()),
        DataType::FixedSizeList(value.clone(), 2),
        DataType::Map(entries, false),
        DataType::Struct(vec![value.clone()].into()),
        DataType::Struct(Fields::empty()),
        DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
        DataType::Union(
            UnionFields::try_new(vec![0], vec![value.clone()]).unwrap(),
            UnionMode::Dense,
        ),
        DataType::Union(
            UnionFields::try_new(vec![0], vec![value.clone()]).unwrap(),
            UnionMode::Sparse,
        ),
        DataType::RunEndEncoded(
            Arc::new(Field::new("run_ends", DataType::Int32, false)),
            value,
        ),
    ]
}

#[test]
fn native_concrete_slice_and_array_data_keep_original_owner_for_empty_and_null_outputs() {
    for kind in types() {
        for len in [0, 3] {
            let owner = Arc::new(Owner::default());
            let weak = Arc::downgrade(&owner);
            let expected = ResourceOwnerHandle::new(owner.clone());
            let array = {
                let _guard = enter_resource_owner(owner.clone());
                new_null_array(&kind, len)
            };
            assert!(
                array.resource_owner().same_owner(&expected),
                "{kind:?} length {len}"
            );
            let slice = array.slice(0, len);
            assert!(
                slice.resource_owner().same_owner(&expected),
                "slice {kind:?}"
            );
            let data = slice.to_data();
            assert!(
                data.resource_owner().same_owner(&expected),
                "to_data {kind:?}"
            );
            let reconstructed = make_array(data);
            assert!(
                reconstructed.resource_owner().same_owner(&expected),
                "from data {kind:?}"
            );
            drop(expected);
            drop(owner);
            drop(array);
            drop(slice);
            assert!(
                weak.upgrade().is_some(),
                "native concrete root lost for {kind:?}"
            );
            let data = reconstructed.to_data();
            drop(reconstructed);
            assert!(
                weak.upgrade().is_some(),
                "native data root lost for {kind:?}"
            );
            drop(data);
            assert!(weak.upgrade().is_none(), "native owner leaked for {kind:?}");
        }
    }
}

#[test]
fn native_downcast_clone_of_zero_buffer_null_array_keeps_owner() {
    let owner = Arc::new(Owner::default());
    let weak = Arc::downgrade(&owner);
    let original = {
        let _guard = enter_resource_owner(owner.clone());
        Arc::new(NullArray::new(19)) as ArrayRef
    };
    let concrete = original
        .as_any()
        .downcast_ref::<NullArray>()
        .unwrap()
        .clone();
    drop(original);
    drop(owner);
    assert!(weak.upgrade().is_some());
    assert_eq!(concrete.len(), 19);
    drop(concrete);
    assert!(weak.upgrade().is_none());
}

#[test]
fn denied_array_data_rebinding_latches_and_preserves_original_after_current_owner_drops() {
    let original = Arc::new(Owner::default());
    let weak = Arc::downgrade(&original);
    let data = {
        let _guard = enter_resource_owner(original.clone());
        NullArray::new(7).into_data()
    };
    let refusing = Arc::new(Owner {
        reject: true,
        ..Owner::default()
    });
    let output = {
        let _guard = enter_resource_owner(refusing.clone());
        NullArray::from(data)
    };
    assert!(refusing.failure.lock().unwrap().is_some());
    drop(original);
    drop(refusing);
    assert!(weak.upgrade().is_some());
    drop(output);
    assert!(weak.upgrade().is_none());
}

#[test]
fn successful_array_data_rebinding_keeps_adopted_original_until_last_native_output() {
    let original = Arc::new(Owner::default());
    let weak = Arc::downgrade(&original);
    let data = {
        let _guard = enter_resource_owner(original.clone());
        Int32Array::from(vec![1, 2]).into_data()
    };
    let current = Arc::new(Owner::default());
    let output = {
        let _guard = enter_resource_owner(current.clone());
        Int32Array::from(data)
    };
    drop(original);
    drop(current);
    assert!(weak.upgrade().is_some());
    let slice = output.slice(1, 1);
    drop(output);
    assert_eq!(slice.value(0), 2);
    assert!(weak.upgrade().is_some());
    drop(slice);
    assert!(weak.upgrade().is_none());
}

#[test]
fn byte_string_view_and_primitive_reinterpretation_keep_original_roots() {
    let owner = Arc::new(Owner::default());
    let weak = Arc::downgrade(&owner);
    let (binary, primitive) = {
        let _guard = enter_resource_owner(owner.clone());
        (
            BinaryArray::from(vec![
                b"hello".as_slice(),
                b"longer than inline view".as_slice(),
            ]),
            Int64Array::from(vec![1, 2]),
        )
    };
    let string = StringArray::try_from_binary(binary).unwrap();
    let view = StringViewArray::from(&string);
    drop(string);
    let binary_view: BinaryViewArray = view.to_binary_view();
    let view = binary_view.to_string_view().unwrap().gc();
    let timestamp: crate::TimestampNanosecondArray = primitive.reinterpret_cast();
    drop(primitive);
    drop(owner);
    assert!(weak.upgrade().is_some());
    drop(timestamp);
    assert!(weak.upgrade().is_some());
    assert_eq!(view.value(1), "longer than inline view");
    drop(view);
    assert!(weak.upgrade().is_none());
}

#[test]
fn record_batch_struct_project_and_schema_roots_survive_zero_columns() {
    let owner = Arc::new(Owner::default());
    let weak = Arc::downgrade(&owner);
    let batch = {
        let _guard = enter_resource_owner(owner.clone());
        RecordBatch::try_new_with_options(
            Arc::new(Schema::empty()),
            vec![],
            &RecordBatchOptions::new().with_row_count(Some(5)),
        )
        .unwrap()
    };
    let projected = batch.project(&[]).unwrap();
    drop(batch);
    let batch = projected;
    assert!(!batch.resource_owner().is_empty());
    let structure = StructArray::from(batch);
    let batch = RecordBatch::from(structure);
    assert_eq!(batch.num_rows(), 5);
    let schema = batch.schema();
    drop(owner);
    drop(batch);
    assert!(weak.upgrade().is_some());
    drop(schema);
    assert!(weak.upgrade().is_none());
}

#[test]
fn run_end_child_keeps_distinct_original_even_when_parent_adoption_is_denied() {
    let run_owner = Arc::new(Owner::default());
    let parent_owner = Arc::new(Owner::default());
    let run_weak = Arc::downgrade(&run_owner);
    let parent_weak = Arc::downgrade(&parent_owner);
    let run_data = {
        let _guard = enter_resource_owner(run_owner.clone());
        Int32Array::from(vec![3]).into_data()
    };
    let parent_data = {
        let _guard = enter_resource_owner(parent_owner.clone());
        ArrayDataBuilder::new(DataType::RunEndEncoded(
            Arc::new(Field::new("run_ends", DataType::Int32, false)),
            Arc::new(Field::new("values", DataType::Null, true)),
        ))
        .len(3)
        .child_data(vec![run_data, NullArray::new(1).into_data()])
        .build()
        .unwrap()
    };
    let refusing = Arc::new(Owner {
        reject: true,
        ..Owner::default()
    });
    let output = {
        let _guard = enter_resource_owner(refusing.clone());
        RunArray::<Int32Type>::from(parent_data)
    };
    drop(run_owner);
    drop(parent_owner);
    drop(refusing);
    assert!(run_weak.upgrade().is_some());
    assert!(parent_weak.upgrade().is_some());
    let data = output.into_data();
    assert!(run_weak.upgrade().is_some());
    assert!(parent_weak.upgrade().is_some());
    drop(data);
    assert!(run_weak.upgrade().is_none());
    assert!(parent_weak.upgrade().is_none());
}

#[test]
fn mutable_builder_conversion_returns_original_governed_array_intact() {
    let owner = Arc::new(Owner::default());
    let weak = Arc::downgrade(&owner);
    let (primitive, string) = {
        let _guard = enter_resource_owner(owner.clone());
        (Int32Array::from(vec![1]), StringArray::from(vec!["a"]))
    };
    let primitive = primitive.into_builder().unwrap_err();
    let string = string.into_builder().unwrap_err();
    drop(owner);
    drop(primitive);
    assert!(weak.upgrade().is_some());
    drop(string);
    assert!(weak.upgrade().is_none());
}

#[test]
fn dictionary_parent_rebinding_does_not_reverse_adoption_or_leak_original() {
    let original = Arc::new(Owner::default());
    let weak = Arc::downgrade(&original);
    let keys = {
        let _guard = enter_resource_owner(original.clone());
        Int32Array::from(vec![0, 0])
    };
    let middle = Arc::new(Owner::default());
    let dictionary = {
        let _guard = enter_resource_owner(middle.clone());
        crate::DictionaryArray::try_new(keys, Arc::new(NullArray::new(1))).unwrap()
    };
    let last = Arc::new(Owner::default());
    let data = {
        let _guard = enter_resource_owner(last.clone());
        dictionary.into_data()
    };
    drop(original);
    drop(middle);
    drop(last);
    assert!(weak.upgrade().is_some());
    drop(data);
    assert!(weak.upgrade().is_none());
}

#[cfg(feature = "pool")]
#[test]
fn payload_destructors_run_before_original_resource_owner_is_released() {
    use arrow_buffer::{MemoryPool, MemoryReservation};
    use std::sync::Weak;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Pool {
        owner: Weak<Owner>,
        dropped: Arc<AtomicUsize>,
    }
    #[derive(Debug)]
    struct Reservation {
        owner: Weak<Owner>,
        dropped: Arc<AtomicUsize>,
        size: usize,
    }
    impl Drop for Reservation {
        fn drop(&mut self) {
            assert!(
                self.owner.upgrade().is_some(),
                "resource owner released before native buffer destructor"
            );
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    impl MemoryReservation for Reservation {
        fn size(&self) -> usize {
            self.size
        }
        fn resize(&mut self, value: usize) {
            self.size = value;
        }
    }
    impl MemoryPool for Pool {
        fn reserve(&self, size: usize) -> Box<dyn MemoryReservation> {
            Box::new(Reservation {
                owner: self.owner.clone(),
                dropped: self.dropped.clone(),
                size,
            })
        }
        fn available(&self) -> isize {
            isize::MAX
        }
        fn used(&self) -> usize {
            0
        }
        fn capacity(&self) -> usize {
            usize::MAX
        }
    }
    for kind in types() {
        let owner = Arc::new(Owner::default());
        let weak = Arc::downgrade(&owner);
        let dropped = Arc::new(AtomicUsize::new(0));
        let array = {
            let _guard = enter_resource_owner(owner.clone());
            new_null_array(&kind, 3)
        };
        array.claim(&Pool {
            owner: weak.clone(),
            dropped: dropped.clone(),
        });
        drop(owner);
        drop(array);
        assert!(weak.upgrade().is_none(), "owner leaked for {kind:?}");
        // Null and empty Struct have no backing to claim. Other selected types
        // invoke their actual native Buffer reservation destructors above.
        if !matches!(kind, DataType::Null | DataType::Struct(_)) {
            assert!(
                dropped.load(Ordering::Relaxed) > 0,
                "no native backing destructor for {kind:?}"
            );
        }
    }
}
