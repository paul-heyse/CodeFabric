//! Observes requested native layouts without changing allocation behavior.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use arrow_schema::resource::{RetainedResourceOwner, ResourceAllocationRequest, ResourceOwnerError, enter_resource_owner};
use buoyant_kernel::expressions::{Scalar, ArrayData, MapData, StructData};
use buoyant_kernel::schema::{DataType, ArrayType, MapType, StructField, MetadataValue};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static ADMITTED: Cell<usize> = const { Cell::new(0) };
    static EARLY: Cell<bool> = const { Cell::new(false) };
}
struct Observer;
#[global_allocator] static ALLOCATOR: Observer = Observer;
fn record(size: usize) {
    let _ = TRACK.try_with(|track| if track.get() {
        let _ = ALLOCATED.try_with(|used| {
            let total = used.get().saturating_add(size); used.set(total);
            let _ = ADMITTED.try_with(|admitted| if total > admitted.get() { let _ = EARLY.try_with(|v| v.set(true)); });
        });
    });
}
// SAFETY: System receives the original layouts and pointers unchanged. All
// observation is nonallocating TLS, with no failure interception or injection.
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 { record(layout.size()); unsafe { System.alloc(layout) } }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { record(layout.size()); unsafe { System.alloc_zeroed(layout) } }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 { record(size); unsafe { System.realloc(pointer, layout, size) } }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { unsafe { System.dealloc(pointer, layout) } }
}
struct Tracking;
impl Tracking {
    fn begin() -> Self { ALLOCATED.with(|v| v.set(0)); ADMITTED.with(|v| v.set(0)); EARLY.with(|v| v.set(false)); TRACK.with(|v| v.set(true)); Self }
    fn finish(self) -> (usize, usize, bool) { drop(self); (ALLOCATED.with(Cell::get), ADMITTED.with(Cell::get), EARLY.with(Cell::get)) }
}
impl Drop for Tracking { fn drop(&mut self) { TRACK.with(|v| v.set(false)); } }
#[derive(Debug)]
struct Owner { deny: &'static str, live: Arc<AtomicUsize> }
impl Drop for Owner { fn drop(&mut self) { self.live.fetch_sub(1, Ordering::SeqCst); } }
impl RetainedResourceOwner for Owner {
    fn try_reserve_allocation(&self, request: ResourceAllocationRequest) -> Result<(), ResourceOwnerError> {
        if request.kind == self.deny { return Err(ResourceOwnerError { kind: request.kind, requested: request.bytes, limit: 0 }); }
        ADMITTED.with(|v| v.set(v.get().checked_add(request.bytes).unwrap()));
        Ok(())
    }
    fn try_adopt(&self, original: Arc<dyn RetainedResourceOwner>) -> Result<(), ResourceOwnerError> {
        if (original.as_ref() as &dyn std::any::Any).downcast_ref::<Self>().is_some_and(|original| std::ptr::eq(self, original)) { Ok(()) }
        else { Err(ResourceOwnerError { kind: "foreign probe owner", requested: 1, limit: 0 }) }
    }
    fn record_failure(&self, _: ResourceOwnerError) {}
}
fn owner(deny: &'static str) -> Arc<Owner> { Arc::new(Owner { deny, live: Arc::new(AtomicUsize::new(1)) }) }
fn list() -> Scalar {
    Scalar::Array(ArrayData::try_new(ArrayType::new(DataType::STRING, true), [Scalar::String("a long original string".repeat(50)), Scalar::Null(DataType::STRING), Scalar::String("東京".into())]).unwrap())
}
fn map() -> Scalar {
    let value = list();
    Scalar::Map(MapData::try_new(MapType::new(DataType::STRING, value.data_type(), true),
        [(Scalar::String("original key".into()), value.clone()), (Scalar::String("other key".into()), Scalar::Null(value.data_type()))]).unwrap())
}
fn structured() -> Scalar {
    let values = vec![map(), list(), Scalar::Timestamp(123), Scalar::Null(DataType::STRING)];
    let fields = values.iter().enumerate().map(|(i, v)| {
        StructField::nullable(format!("original.field.{i}"), v.data_type()).with_metadata([
            ("kept.metadata", MetadataValue::Other(serde_json::json!({"key": [1,2,"東京".repeat(128)]})))
        ])
    }).collect();
    Scalar::Struct(StructData::try_new(fields, values).unwrap())
}
#[test]
fn scalar_native_allocations_are_admitted_before_constructor_growth_and_finish() {
    let mut values = vec![Scalar::Byte(1), Scalar::Short(2), Scalar::Integer(3), Scalar::Long(4), Scalar::Float(1.2), Scalar::Double(2.3), Scalar::Boolean(true),
        Scalar::Timestamp(1), Scalar::TimestampNtz(2), Scalar::Date(3), Scalar::decimal(7, 12, 4).unwrap(),
        Scalar::String("long 東京".repeat(257)), Scalar::Binary(vec![1;4097]), list(), map(), structured(),
        Scalar::Struct(StructData::try_new(vec![], vec![]).unwrap())];
    for kind in [DataType::BYTE, DataType::STRING, list().data_type(), map().data_type(), structured().data_type(), DataType::VOID] { values.push(Scalar::Null(kind)); }
    for (case, value) in values.iter().enumerate() {
        for rows in [0, 1, 7, 64, 1025] {
            let owner = owner(""); let guard = enter_resource_owner(owner.clone());
            let tracking = Tracking::begin();
            let result = value.to_array(rows);
            let (used, admitted, early) = tracking.finish();
            assert!(result.is_ok(), "case={case} rows={rows} result={result:?}");
            assert!(!early && used <= admitted, "case={case} rows={rows} used={used} admitted={admitted} early={early}");
            assert_eq!(result.unwrap().len(), rows);
            drop(guard);
        }
    }
}
#[test]
fn native_array_data_elements_have_independent_admission() {
    for value in [Scalar::String("large".repeat(1000)), list(), map(), structured()] {
        let array = ArrayData::try_new(ArrayType::new(value.data_type(), true), [value.clone(), Scalar::Null(value.data_type()), value]).unwrap();
        let owner = owner(""); let _guard = enter_resource_owner(owner);
        let tracking = Tracking::begin();
        let result = array.to_arrow();
        let (used, admitted, early) = tracking.finish();
        assert!(result.is_ok(), "{result:?}");
        assert!(!early && used <= admitted, "used={used} admitted={admitted} early={early}");
        assert_eq!(result.unwrap().len(), 3);
    }
}
#[test]
fn denied_scalar_schema_and_builder_allocate_no_payload() {
    for deny in ["native_scalar_schema", "native_scalar_builder"] {
        let value = Scalar::Long(7); let owner = owner(deny); let _guard = enter_resource_owner(owner);
        let tracking = Tracking::begin(); let result = value.to_array(4096); let (used, _, early) = tracking.finish();
        assert!(result.unwrap_err().is_resource_exhausted()); assert_eq!(used, 0, "{deny}"); assert!(!early);
    }
}
#[test]
fn scalar_buffer_escape_retains_the_actual_original_owner() {
    use arrow_array::cast::AsArray;
    use arrow_array::types::Int64Type;
    let owner = owner(""); let live = owner.live.clone(); let guard = enter_resource_owner(owner.clone());
    let output = Scalar::Long(7).to_array(1025).unwrap();
    let buffer = output.as_primitive::<Int64Type>().values().inner().clone();
    drop(output); drop(guard); drop(owner);
    assert_eq!(live.load(Ordering::SeqCst), 1);
    drop(buffer); assert_eq!(live.load(Ordering::SeqCst), 0);
}
#[test]
fn scalar_offset_overflow_fails_before_schema_or_builder_allocation() {
    let value = list(); let owner = owner(""); let _guard = enter_resource_owner(owner);
    let tracking = Tracking::begin(); let result = value.to_array(i32::MAX as usize); let (used, _, _) = tracking.finish();
    assert!(result.unwrap_err().is_resource_exhausted()); assert_eq!(used, 0);
}

#[test]
fn malformed_deserialized_scalar_rejects_before_native_builder_panics() {
    let mut encoded = serde_json::to_value(map()).unwrap();
    encoded["Map"]["pairs"][0][0] = serde_json::to_value(Scalar::Null(DataType::STRING)).unwrap();
    let malformed: Scalar = serde_json::from_value(encoded).unwrap();
    let owner = owner(""); let _guard = enter_resource_owner(owner);
    let tracking = Tracking::begin(); let result = malformed.to_array(4); let (used, admitted, early) = tracking.finish();
    assert!(result.is_err()); assert!(!early && used <= admitted);
}
