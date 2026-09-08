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

//! Native selected-kernel allocation admission. Layouts describe complete new
//! allocations: original receipts remain live across replacement and output.
use arrow_array::Array;
use arrow_buffer::{Buffer, MutableBuffer};
#[cfg(feature = "pool")]
use arrow_schema::resource::ResourceAllocationRequest;
use arrow_schema::{
    ArrowError,
    resource::{ResourceOwnerError, ResourceOwnerHandle},
};
use std::alloc::Layout;
use std::sync::atomic::AtomicUsize;

pub(crate) fn current() -> ResourceOwnerHandle {
    ResourceOwnerHandle::capture()
}
pub(crate) fn overflow(kind: &'static str) -> ArrowError {
    ResourceOwnerError {
        kind,
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}
pub(crate) fn admit(layout: Layout, kind: &'static str) -> Result<(), ArrowError> {
    if let Some(owner) = current().owner() {
        #[cfg(not(feature = "pool"))]
        {
            let _ = (owner, layout, kind);
            return Err(ResourceOwnerError {
                kind: "selection backing lifetime requires pool",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        #[cfg(feature = "pool")]
        if layout.size() != 0 {
            owner.try_reserve_allocation(ResourceAllocationRequest {
                bytes: layout.size(),
                alignment: layout.align(),
                kind,
            })?;
        }
    }
    Ok(())
}
pub(crate) fn array<T>(count: usize, kind: &'static str) -> Result<(), ArrowError> {
    admit(Layout::array::<T>(count).map_err(|_| overflow(kind))?, kind)
}
pub(crate) fn mutable(bytes: usize, kind: &'static str) -> Result<(), ArrowError> {
    let layout = MutableBuffer::try_capacity_layout(bytes).map_err(|_| overflow(kind))?;
    admit(layout, kind)
}
pub(crate) fn bitmap(bits: usize, kind: &'static str) -> Result<(), ArrowError> {
    mutable(bits.div_ceil(8), kind)
}
pub(crate) fn owner() -> Result<(), ArrowError> {
    admit(
        arrow_buffer::allocation_owner_layout(),
        "selection immutable buffer owner",
    )
}
pub(crate) fn arc<T>() -> Result<(), ArrowError> {
    // Exact ArcInner<T> geometry in the version-pinned Rust standard library.
    #[repr(C)]
    struct ArcInner<T> {
        strong: AtomicUsize,
        weak: AtomicUsize,
        value: T,
    }
    admit(Layout::new::<ArcInner<T>>(), "selection array Arc")
}
pub(crate) fn adopt(array: &dyn Array) -> Result<(), ArrowError> {
    if !current().is_empty() && array.resource_owner().is_empty() {
        return Err(ResourceOwnerError {
            kind: "unowned native array input",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    current().try_inherit(array.resource_owner())?;
    Ok(())
}
pub(crate) fn retain(buffer: &Buffer) -> Result<(), ArrowError> {
    arrow_data::try_retain_fresh_buffer(buffer)
}
pub(crate) fn total(arrays: &[&dyn Array]) -> Result<usize, ArrowError> {
    arrays.iter().try_fold(0usize, |n, a| {
        n.checked_add(a.len())
            .ok_or_else(|| overflow("selection row count"))
    })
}

#[cfg(all(test, feature = "pool"))]
mod tests {
    use super::*;
    use arrow_array::{BooleanArray, Int32Array, PrimitiveArray, types::Int32Type};
    use arrow_schema::resource::{RetainedResourceOwner, enter_resource_owner};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct Owner {
        requests: Mutex<Vec<ResourceAllocationRequest>>,
        deny: Option<&'static str>,
        adopted: Mutex<Vec<Arc<dyn RetainedResourceOwner>>>,
    }
    impl RetainedResourceOwner for Owner {
        fn try_reserve_allocation(
            &self,
            request: ResourceAllocationRequest,
        ) -> Result<(), ResourceOwnerError> {
            self.requests.lock().unwrap().push(request);
            if self.deny == Some(request.kind) {
                return Err(ResourceOwnerError {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
            }
            Ok(())
        }
        fn try_adopt(
            &self,
            original: Arc<dyn RetainedResourceOwner>,
        ) -> Result<(), ResourceOwnerError> {
            if self.deny == Some("adopt") {
                return Err(ResourceOwnerError {
                    kind: "adopt",
                    requested: 1,
                    limit: 0,
                });
            }
            self.adopted.lock().unwrap().push(original);
            Ok(())
        }
        fn record_failure(&self, _: ResourceOwnerError) {}
    }
    // Source fixture ownership is established independently from the operation
    // under test. These constructors are fixture setup, not an allocation proof.
    fn source<T>(f: impl FnOnce() -> T) -> T {
        let _scope = enter_resource_owner(Arc::new(Owner::default()));
        f()
    }

    fn typed(error: ArrowError, kind: &'static str) {
        assert!(matches!(error, ArrowError::ResourceOwnerError(error) if error.kind == kind));
    }

    #[test]
    fn resource_primitive_filter_all_native_strategies_and_null_mask() {
        let input = source(|| {
            Int32Array::from(vec![
                Some(0),
                Some(1),
                None,
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
                Some(9),
            ])
        });
        for selected in [
            vec![false; 10],
            vec![true; 10],
            vec![
                true, false, false, true, false, false, false, false, false, false,
            ],
            vec![true, true, true, true, true, true, true, true, true, false],
        ] {
            let predicate = source(|| BooleanArray::from(selected.clone()));
            let expected = crate::filter::filter(&input, &predicate).unwrap();
            for optimize in [false, true] {
                let owner = Arc::new(Owner::default());
                let _scope = enter_resource_owner(owner);
                let builder = crate::filter::FilterBuilder::try_new(&predicate).unwrap();
                let predicate = if optimize {
                    builder.try_optimize().unwrap()
                } else {
                    builder
                }
                .build();
                let actual = predicate.filter(&input).unwrap();
                assert_eq!(actual.as_ref(), expected.as_ref());
            }
        }
        let predicate = source(|| {
            BooleanArray::from(vec![
                Some(true),
                None,
                Some(true),
                Some(false),
                Some(true),
                Some(true),
                Some(true),
                Some(true),
                Some(true),
                Some(true),
            ])
        })
        .slice(1, 8);
        let input = input.slice(1, 8);
        let expected = crate::filter::filter(&input, &predicate).unwrap();
        let owner = Arc::new(Owner::default());
        let _scope = enter_resource_owner(owner);
        assert_eq!(
            crate::filter::filter(&input, &predicate).unwrap().as_ref(),
            expected.as_ref()
        );
    }

    #[test]
    fn resource_primitive_denial_precedes_payload_and_claim() {
        let input = source(|| Int32Array::from(vec![Some(1), None, Some(3)]));
        let predicate = source(|| BooleanArray::from(vec![true, false, true]));
        for kind in [
            "selection array Arc",
            "filter primitive values",
            "selection immutable buffer owner",
            "transform backing lifetime",
        ] {
            let owner = Arc::new(Owner {
                deny: Some(kind),
                ..Default::default()
            });
            let _scope = enter_resource_owner(owner.clone());
            typed(crate::filter::filter(&input, &predicate).unwrap_err(), kind);
            assert_eq!(owner.requests.lock().unwrap().last().unwrap().kind, kind);
        }
    }

    #[test]
    fn resource_primitive_concat_exact_payload_and_validity() {
        let left = source(|| Int32Array::from(vec![Some(1), None]));
        let right = source(|| Int32Array::from(vec![Some(3)]));
        let expected = crate::concat::concat(&[&left, &right]).unwrap();
        let owner = Arc::new(Owner::default());
        let _scope = enter_resource_owner(owner.clone());
        let actual = crate::concat::concat(&[&left, &right]).unwrap();
        assert_eq!(actual.as_ref(), expected.as_ref());
        let requests = owner.requests.lock().unwrap();
        assert!(requests.iter().any(|r| r.kind == "concat primitive values"
            && r.bytes == 3 * size_of::<i32>()
            && r.alignment == align_of::<i32>()));
        assert!(
            requests
                .iter()
                .any(|r| r.kind == "concat primitive validity" && r.bytes == 64)
        );
        assert!(
            requests
                .iter()
                .any(|r| r.kind == "concat primitive finish descriptors"
                    && r.bytes == 4 * size_of::<Buffer>())
        );
    }

    #[test]
    fn resource_primitive_raw_values_and_validity_keep_receipts() {
        for concat in [false, true] {
            let input = source(|| Int32Array::from(vec![Some(1), None, Some(3)]));
            let predicate = source(|| BooleanArray::from(vec![true, true, false]));
            let owner = Arc::new(Owner::default());
            let weak = Arc::downgrade(&owner);
            let scope = enter_resource_owner(owner.clone());
            let output = if concat {
                crate::concat::concat(&[&input, &input]).unwrap()
            } else {
                crate::filter::filter(&input, &predicate).unwrap()
            };
            let native = output
                .as_any()
                .downcast_ref::<PrimitiveArray<Int32Type>>()
                .unwrap();
            let values = native.values().inner().clone();
            let nulls = native.nulls().unwrap().inner().inner().clone();
            drop(output);
            drop(scope);
            drop(owner);
            assert!(weak.upgrade().is_some());
            drop(values);
            assert!(weak.upgrade().is_some());
            drop(nulls);
            assert!(weak.upgrade().is_none());
        }
    }

    #[test]
    fn resource_primitive_foreign_adoption_rejected_before_allocation() {
        let original = Arc::new(Owner::default());
        let scope = enter_resource_owner(original);
        let input = source(|| Int32Array::from(vec![1, 2]));
        drop(scope);
        let predicate = source(|| BooleanArray::from(vec![true, false]));
        let owner = Arc::new(Owner {
            deny: Some("adopt"),
            ..Default::default()
        });
        let _scope = enter_resource_owner(owner.clone());
        typed(
            crate::filter::filter(&input, &predicate).unwrap_err(),
            "adopt",
        );
        assert!(owner.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn resource_filter_optimized_predicate_keeps_descriptor_owner() {
        let input = source(|| BooleanArray::from(vec![true, false, true, false]));
        let owner = Arc::new(Owner::default());
        let weak = Arc::downgrade(&owner);
        let scope = enter_resource_owner(owner.clone());
        let predicate = crate::filter::FilterBuilder::try_new(&input)
            .unwrap()
            .try_optimize()
            .unwrap()
            .build();
        drop(scope);
        drop(owner);
        assert!(weak.upgrade().is_some());
        drop(predicate);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn resource_boolean_native_strategies_concat_and_bare_buffer() {
        let input = source(|| {
            BooleanArray::from(vec![Some(true), None, Some(false), Some(true), Some(false)])
        });
        for mask in [
            vec![false; 5],
            vec![true; 5],
            vec![true, true, false, false, true],
            vec![true, true, true, true, false],
        ] {
            let mask = source(|| BooleanArray::from(mask));
            let expected = crate::filter::filter(&input, &mask).unwrap();
            for optimize in [false, true] {
                let owner = Arc::new(Owner::default());
                let _scope = enter_resource_owner(owner);
                let builder = crate::filter::FilterBuilder::try_new(&mask).unwrap();
                let predicate = if optimize {
                    builder.try_optimize().unwrap()
                } else {
                    builder
                }
                .build();
                assert_eq!(
                    predicate.filter(&input).unwrap().as_ref(),
                    expected.as_ref()
                );
            }
        }
        let expected = crate::concat::concat(&[&input, &input]).unwrap();
        let owner = Arc::new(Owner::default());
        let weak = Arc::downgrade(&owner);
        let scope = enter_resource_owner(owner.clone());
        let output = crate::concat::concat(&[&input, &input]).unwrap();
        assert_eq!(output.as_ref(), expected.as_ref());
        let native = output.as_any().downcast_ref::<BooleanArray>().unwrap();
        let backing = native.values().inner().clone();
        drop(output);
        drop(scope);
        drop(owner);
        assert!(weak.upgrade().is_some());
        drop(backing);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn resource_filter_batch_descriptors_are_admitted_before_column_work() {
        use arrow_array::RecordBatch;
        use arrow_schema::{DataType, Field, Schema};
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Int32, false)]));
        let batch =
            source(|| RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![1, 2]))]))
                .unwrap();
        let mask = source(|| BooleanArray::from(vec![true, false]));
        let owner = Arc::new(Owner {
            deny: Some("filter batch column descriptors"),
            ..Default::default()
        });
        let _scope = enter_resource_owner(owner.clone());
        typed(
            crate::filter::filter_record_batch(&batch, &mask).unwrap_err(),
            "filter batch column descriptors",
        );
        assert_eq!(owner.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn resource_generic_concat_and_zip_use_fallible_transform() {
        use arrow_array::{FixedSizeBinaryArray, Scalar};
        let input =
            source(|| FixedSizeBinaryArray::try_from_iter([b"ab", b"cd"].into_iter())).unwrap();
        let expected = crate::concat::concat(&[&input, &input]).unwrap();
        let truthy = Scalar::new(source(|| Int32Array::from(vec![7])));
        let falsy = Scalar::new(source(|| Int32Array::from(vec![9])));
        let mask = source(|| BooleanArray::from(vec![Some(true), None, Some(false), Some(true)]));
        let expected_zip = crate::zip::zip(&mask, &truthy, &falsy).unwrap();
        let owner = Arc::new(Owner::default());
        let _scope = enter_resource_owner(owner.clone());
        assert_eq!(
            crate::concat::concat(&[&input, &input]).unwrap().as_ref(),
            expected.as_ref()
        );
        assert_eq!(
            crate::zip::zip(&mask, &truthy, &falsy).unwrap().as_ref(),
            expected_zip.as_ref()
        );
        assert!(
            owner
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r.kind == "transform value backing")
        );
    }

    #[test]
    fn resource_nonprimitive_filter_concat_preserve_native_results() {
        use arrow_array::{
            ArrayRef, DictionaryArray, ListArray, NullArray, StringArray, StringViewArray,
            StructArray, types::Int8Type,
        };
        use arrow_schema::{DataType, Field};
        let inputs: Vec<ArrayRef> = source(|| {
            vec![
                Arc::new(StringArray::from(vec![Some("one"), None, Some("three")])) as ArrayRef,
                Arc::new(StringViewArray::from(vec![
                    Some("a long value that uses an external buffer"),
                    None,
                    Some("short"),
                ])) as ArrayRef,
                Arc::new(ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
                    Some(vec![Some(1), None]),
                    None,
                    Some(vec![Some(3)]),
                ])) as ArrayRef,
                Arc::new(StructArray::from(vec![(
                    Arc::new(Field::new("x", DataType::Int32, false)),
                    Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef,
                )])) as ArrayRef,
                Arc::new(NullArray::new(3)) as ArrayRef,
                Arc::new(DictionaryArray::<Int8Type>::from_iter([
                    Some("x"),
                    None,
                    Some("y"),
                ])) as ArrayRef,
            ]
        });
        let mask = source(|| BooleanArray::from(vec![true, false, true]));
        for input in inputs {
            let filtered = crate::filter::filter(input.as_ref(), &mask).unwrap();
            let concatenated = crate::concat::concat(&[input.as_ref(), input.as_ref()]).unwrap();
            let owner = Arc::new(Owner::default());
            let _scope = enter_resource_owner(owner);
            assert_eq!(
                crate::filter::filter(input.as_ref(), &mask)
                    .unwrap()
                    .as_ref(),
                filtered.as_ref()
            );
            assert_eq!(
                crate::concat::concat(&[input.as_ref(), input.as_ref()])
                    .unwrap()
                    .as_ref(),
                concatenated.as_ref()
            );
        }
    }

    #[test]
    fn resource_unowned_primitive_and_mask_are_rejected() {
        let input = Int32Array::from(vec![1, 2]);
        let owned_mask = source(|| BooleanArray::from(vec![true, false]));
        let owned_input = source(|| Int32Array::from(vec![1, 2]));
        let unowned_mask = BooleanArray::from(vec![true, false]);
        let _scope = enter_resource_owner(Arc::new(Owner::default()));
        typed(
            crate::filter::filter(&input, &owned_mask).unwrap_err(),
            "unowned native array input",
        );
        typed(
            crate::filter::filter(&owned_input, &unowned_mask).unwrap_err(),
            "unowned native array input",
        );
        typed(
            crate::concat::concat(&[&input, &input]).unwrap_err(),
            "unowned native array input",
        );
    }

    #[test]
    fn resource_generic_zip_refuses_before_output_growth() {
        use arrow_array::Scalar;
        let truthy = Scalar::new(source(|| Int32Array::from(vec![7])));
        let falsy = Scalar::new(source(|| Int32Array::from(vec![9])));
        let mask = source(|| BooleanArray::from(vec![true, false, true]));
        let owner = Arc::new(Owner {
            deny: Some("transform value backing"),
            ..Default::default()
        });
        let _scope = enter_resource_owner(owner);
        typed(
            crate::zip::zip(&mask, &truthy, &falsy).unwrap_err(),
            "transform value backing",
        );
    }

    #[test]
    fn resource_layout_overflow_is_typed_before_callback() {
        let owner = Arc::new(Owner::default());
        let _scope = enter_resource_owner(owner.clone());
        typed(
            array::<u128>(usize::MAX, "test layout").unwrap_err(),
            "test layout",
        );
        assert!(owner.requests.lock().unwrap().is_empty());
    }
}
