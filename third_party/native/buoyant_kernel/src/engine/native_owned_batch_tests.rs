use std::sync::{Arc, Mutex};

use crate::arrow::array::{Array, NullArray, RecordBatch};
use crate::arrow::datatypes::{DataType, Field, Schema};
use crate::engine::arrow_data::{ArrowEngineData, EngineDataArrowExt, NativeOwnedRecordBatch};
use crate::resource::{
    AllocationAdmission, AllocationReceipt, AllocationRequest, JsonResourceLimits,
    NativeResourceScope, ResourceExhausted, with_resource_scope,
};
use crate::{DeltaResult, EngineData};

#[derive(Debug, Default)]
struct Admission {
    denied: Mutex<Option<&'static str>>,
    admitted: Mutex<Vec<&'static str>>,
}
#[derive(Debug)]
struct Receipt(usize);
impl AllocationReceipt for Receipt {
    fn bytes(&self) -> usize {
        self.0
    }
}
impl AllocationAdmission for Admission {
    fn try_reserve(
        &self,
        request: AllocationRequest,
    ) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
        if *self.denied.lock().unwrap() == Some(request.kind) {
            return Err(ResourceExhausted {
                kind: request.kind,
                requested: request.bytes,
                limit: 0,
            });
        }
        self.admitted.lock().unwrap().push(request.kind);
        Ok(Arc::new(Receipt(request.bytes)))
    }
}
fn limits() -> JsonResourceLimits {
    JsonResourceLimits {
        max_bytes: 4096,
        max_tokens: 128,
        max_depth: 16,
        max_string_bytes: 256,
        max_container_items: 64,
    }
}
fn scope() -> (Arc<NativeResourceScope>, Arc<Admission>) {
    let admission = Arc::new(Admission::default());
    (
        NativeResourceScope::try_new(admission.clone(), 32).unwrap(),
        admission,
    )
}
fn empty(scope: Arc<NativeResourceScope>) -> NativeOwnedRecordBatch {
    with_resource_scope(scope, limits(), || {
        Ok(
            ArrowEngineData::new(RecordBatch::new_empty(Arc::new(Schema::empty())))
                .into_owned_record_batch(),
        )
    })
    .unwrap()
}

#[test]
fn empty_batch_retains_original_native_bookkeeping_through_last_shared_owner() {
    let (scope, admission) = scope();
    let weak = Arc::downgrade(&scope);
    let batch = empty(scope.clone()).try_into_shared().unwrap();
    let admissions = admission.admitted.lock().unwrap().len();
    let last = batch.clone();
    assert_eq!(admission.admitted.lock().unwrap().len(), admissions);
    assert_eq!(batch.num_columns(), 0);
    drop(batch);
    drop(scope);
    assert!(weak.upgrade().is_some());
    drop(last);
    assert!(weak.upgrade().is_none());
}

#[test]
fn all_null_batch_and_struct_conversion_keep_scope_without_data_buffers() {
    let (scope, _) = scope();
    let weak = Arc::downgrade(&scope);
    let batch = with_resource_scope(scope.clone(), limits(), || {
        let nulls = NullArray::new(3);
        assert_eq!(nulls.get_buffer_memory_size(), 0);
        let schema = Arc::new(Schema::new(vec![Field::new("nulls", DataType::Null, true)]));
        let batch = RecordBatch::try_new(schema, vec![Arc::new(nulls)])?;
        Ok(ArrowEngineData::new(batch).into_owned_record_batch())
    })
    .unwrap();
    let (array, owner) = batch.into_struct_array();
    drop(scope);
    assert_eq!(array.len(), 3);
    assert!(weak.upgrade().is_some());
    drop(array);
    assert!(weak.upgrade().is_some());
    drop(owner);
    assert!(weak.upgrade().is_none());
}

#[test]
fn transformed_batch_retains_every_prior_native_generation() {
    let (first, _) = scope();
    let (second, _) = scope();
    let (third, _) = scope();
    let weak = [
        Arc::downgrade(&first),
        Arc::downgrade(&second),
        Arc::downgrade(&third),
    ];
    let batch = empty(first.clone());
    let batch = with_resource_scope(second.clone(), limits(), || {
        batch.try_map(Ok::<_, crate::Error>)
    })
    .unwrap();
    let batch = with_resource_scope(third.clone(), limits(), || {
        batch.try_map(Ok::<_, crate::Error>)
    })
    .unwrap();
    drop(first);
    drop(second);
    drop(third);
    assert!(weak.iter().all(|scope| scope.upgrade().is_some()));
    drop(batch);
    assert!(weak.iter().all(|scope| scope.upgrade().is_none()));
}

#[test]
fn merging_two_existing_owner_chains_preserves_both_predecessors() {
    let (first, _) = scope();
    let (second, _) = scope();
    let (third, _) = scope();
    let weak = [
        Arc::downgrade(&first),
        Arc::downgrade(&second),
        Arc::downgrade(&third),
    ];
    let a = empty(first.clone());
    let a = with_resource_scope(second.clone(), limits(), || {
        a.try_map(Ok::<_, crate::Error>)
    })
    .unwrap();
    let b = empty(third.clone());
    let b = with_resource_scope(second.clone(), limits(), || {
        b.try_map(Ok::<_, crate::Error>)
    })
    .unwrap();
    let owners = a.owners().clone().try_merge(b.owners().clone()).unwrap();
    drop(a);
    drop(b);
    drop(first);
    drop(second);
    drop(third);
    assert!(weak.iter().all(|scope| scope.upgrade().is_some()));
    drop(owners);
    assert!(weak.iter().all(|scope| scope.upgrade().is_none()));
}

#[test]
fn rejected_owner_merge_precedes_transform_allocation() {
    let (first, _) = scope();
    let (second, admission) = scope();
    let batch = empty(first);
    *admission.denied.lock().unwrap() = Some("native_owner_transfer_node");
    let mut called = false;
    let error = with_resource_scope(second, limits(), || {
        batch.try_map(|batch| {
            called = true;
            Ok::<_, crate::Error>(batch)
        })
    })
    .unwrap_err();
    assert!(!called);
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("native_owner_transfer_node"));
}

#[test]
fn shared_owner_and_descriptor_clone_are_fallibly_admitted() {
    let (scope, admission) = scope();
    let batch = empty(scope.clone());
    *admission.denied.lock().unwrap() = Some("native_batch_descriptor");
    let error = batch.try_clone().unwrap_err();
    assert!(error.is_resource_exhausted());
    *admission.denied.lock().unwrap() = Some("native_batch_shared_owner");
    assert!(batch.try_into_shared().unwrap_err().is_resource_exhausted());
}

#[test]
fn bare_legacy_conversion_rejects_governed_owner_and_governed_consumer() {
    let (scope, _) = scope();
    let batch = empty(scope.clone());
    assert!(
        batch
            .try_into_ungoverned_record_batch()
            .unwrap_err()
            .is_resource_exhausted()
    );
    let unowned = ArrowEngineData::new(RecordBatch::new_empty(Arc::new(Schema::empty())))
        .into_owned_record_batch();
    let error = with_resource_scope(scope, limits(), || {
        unowned.try_into_ungoverned_record_batch()
    })
    .unwrap_err();
    assert!(error.is_resource_exhausted());
}

#[test]
fn engine_extension_preserves_owner_and_rejects_bare_escape() -> DeltaResult<()> {
    let (scope, _) = scope();
    let data: Box<dyn EngineData> = Box::new(empty(scope.clone()).into_engine_data());
    let error = data.try_into_record_batch().unwrap_err();
    assert!(error.is_resource_exhausted());
    let data: Box<dyn EngineData> = Box::new(empty(scope).into_engine_data());
    assert!(data.try_into_owned_record_batch()?.owners().is_governed());
    Ok(())
}
