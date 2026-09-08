//! Observes requested native layouts without changing allocation behavior.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use arrow_schema::resource::{RetainedResourceOwner, ResourceAllocationRequest, ResourceOwnerError, enter_resource_owner};
use buoyant_kernel::expressions::{Scalar, Expression};
use buoyant_kernel::schema::{DataType, StructField, StructType};

thread_local! {
    static FIRST: Cell<(usize,usize,usize)> = const { Cell::new((0,0,0)) };
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
            let _ = ADMITTED.try_with(|admitted| if total > admitted.get() { let _=FIRST.try_with(|v|if v.get().0==0 {v.set((total,admitted.get(),size))}); let _ = EARLY.try_with(|v| v.set(true)); });
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
    fn begin() -> Self { FIRST.with(|v|v.set((0,0,0))); ALLOCATED.with(|v| v.set(0)); ADMITTED.with(|v| v.set(0)); EARLY.with(|v| v.set(false)); TRACK.with(|v| v.set(true)); Self }
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

#[test]
fn coalesce_native_output_and_descriptors_are_admitted_before_allocation() {
    use arrow_array::{Array, ArrayRef, Int32Array, StringArray};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::coalesce_arrays;
    for strings in [false, true] {
        let owner = owner(""); let _guard = enter_resource_owner(owner);
        let arrays: [ArrayRef; 2] = if strings {
            [Arc::new(StringArray::from(vec![Some("first"),None,None])),Arc::new(StringArray::from(vec![None,Some("second"),None]))]
        } else {
            [Arc::new(Int32Array::from(vec![Some(1),None,None])),Arc::new(Int32Array::from(vec![None,Some(2),None]))]
        };
        let tracking = Tracking::begin();
        let output = coalesce_arrays(&arrays, Some(if strings { &DataType::STRING } else { &DataType::INTEGER }));
        let (used, admitted, early) = tracking.finish();
        assert!(output.is_ok(), "{:?}", output.as_ref().err());
        assert!(!early && used <= admitted, "strings={strings}: {used}/{admitted}, early={early}");
        let output = output.unwrap(); assert_eq!(output.len(),3); assert_eq!(output.null_count(),1);
    }
}

#[test]
fn array_expression_native_interleave_is_admitted_before_allocating() {
    use arrow_array::{Array, RecordBatch, RecordBatchOptions};
    use arrow_schema::Schema;
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    let owner = owner(""); let _guard = enter_resource_owner(owner);
    let expression = Expression::array([Expression::literal(Scalar::Integer(1)),Expression::literal(Scalar::Integer(2))]);
    let batch = RecordBatch::try_new_with_options(Arc::new(Schema::empty()),vec![],&RecordBatchOptions::new().with_row_count(Some(3))).unwrap();
    let tracking = Tracking::begin();
    let output = evaluate_expression(&expression, &batch, None);
    let (used, admitted, early) = tracking.finish();
    assert!(output.is_ok(), "{:?}", output.as_ref().err()); assert!(!early && used <= admitted, "{used}/{admitted}, early={early}");
    let output = output.unwrap(); assert_eq!(output.len(),3); assert_eq!(output.null_count(),0);
}

#[test]
fn public_null_row_and_create_many_admit_columns_schema_and_final_owners() {
    use buoyant_kernel::{EvaluationHandler, engine::arrow_expression::ArrowEvaluationHandler};
    use buoyant_kernel::engine::arrow_data::EngineDataArrowExt;
    let schema = Arc::new(StructType::new_unchecked(vec![StructField::nullable("value",DataType::STRING),StructField::nullable("number",DataType::INTEGER)]));
    let row = [Scalar::String("actual native value".repeat(16)),Scalar::Integer(7)];
    let owner = owner(""); let _guard = enter_resource_owner(owner);
    for rows in [0,1,3] {
        let input = [&row[..];3];
        let tracking = Tracking::begin(); let output = ArrowEvaluationHandler.create_many(schema.clone(), &input[..rows]);
        let (used, admitted, early) = tracking.finish();
        assert!(output.is_ok(), "{:?}", output.as_ref().err()); assert!(!early && used <= admitted, "rows={rows}: {used}/{admitted}, early={early}");
        assert_eq!(output.unwrap().try_into_record_batch().unwrap().num_rows(),rows);
    }
    let tracking = Tracking::begin(); let output = ArrowEvaluationHandler.null_row(schema);
    let (used, admitted, early) = tracking.finish();
    assert!(output.is_ok(), "{:?}", output.as_ref().err()); assert!(!early && used <= admitted, "null: {used}/{admitted}, early={early}");
    assert_eq!(output.unwrap().try_into_record_batch().unwrap().num_rows(),1);
}

#[test]
fn coalesce_descriptor_refusal_returns_inline_pressure_before_native_work() {
    use arrow_array::{ArrayRef,Int32Array};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::coalesce_arrays;
    let owner = owner("native_expression_ArrayData_descriptors"); let _guard = enter_resource_owner(owner);
    let arrays: [ArrayRef;2] = [Arc::new(Int32Array::from(vec![1])),Arc::new(Int32Array::from(vec![2]))];
    let tracking=Tracking::begin(); let output=coalesce_arrays(&arrays,None); let (used,_,_)=tracking.finish();
    assert!(matches!(output,Err(arrow_schema::ArrowError::ResourceOwnerError(_)))); assert_eq!(used,0);
}

#[test]
fn evaluator_constructor_retains_actual_arrow_admission_owner() {
    use buoyant_kernel::{EvaluationHandler, engine::arrow_expression::ArrowEvaluationHandler};
    let schema=Arc::new(StructType::new_unchecked(vec![]));
    let expression=Arc::new(Expression::literal(Scalar::Integer(3)));
    let original=owner(""); let weak=Arc::downgrade(&original); let guard=enter_resource_owner(original.clone());
    let tracking=Tracking::begin();
    let evaluator=ArrowEvaluationHandler.new_expression_evaluator(schema,expression,DataType::INTEGER).unwrap();
    let (used,admitted,early)=tracking.finish();
    assert!(!early && used<=admitted);
    drop(guard);drop(original);assert!(weak.upgrade().is_some());
    drop(evaluator);assert!(weak.upgrade().is_none());
}

#[test]
fn native_owner_pair_depth_is_bounded_across_fresh_scopes_before_node_admission() {
    use buoyant_kernel::engine::arrow_data::NativeDataOwners;
    use buoyant_kernel::resource::{AllocationAdmission, AllocationRequest, AllocationReceipt, NativeResourceScope, JsonResourceLimits, with_resource_scope, ResourceExhausted};
    #[derive(Debug,Default)] struct Budget { live: Arc<AtomicUsize>, nodes: AtomicUsize }
    #[derive(Debug)] struct Receipt { live: Arc<AtomicUsize>, bytes: usize }
    impl AllocationReceipt for Receipt { fn bytes(&self)->usize { self.bytes } }
    impl Drop for Receipt { fn drop(&mut self) {self.live.fetch_sub(self.bytes,Ordering::SeqCst);} }
    impl AllocationAdmission for Budget {
        fn try_reserve(&self, request:AllocationRequest)->Result<Arc<dyn AllocationReceipt>,ResourceExhausted> {
            self.live.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|live|live.checked_add(request.bytes).filter(|next|*next<=64*1024*1024)).map_err(|_|ResourceExhausted{kind:"probe ledger",requested:request.bytes,limit:64*1024*1024})?;
            if request.kind=="native_owner_transfer_node" {self.nodes.fetch_add(1,Ordering::SeqCst);}
            Ok(Arc::new(Receipt{live:self.live.clone(),bytes:request.bytes}))
        }
    }
    let budget=Arc::new(Budget::default());
    let limits=JsonResourceLimits{max_bytes:4096,max_tokens:128,max_depth:16,max_string_bytes:64,max_container_items:32};
    let fresh=|| {let scope=NativeResourceScope::try_new(budget.clone(),4).unwrap(); with_resource_scope(scope,limits,||Ok(NativeDataOwners::current())).unwrap()};
    let mut owner=fresh();
    for depth in 1..=NativeDataOwners::MAX_INHERITED_DEPTH {
        owner=owner.try_merge(fresh()).unwrap();assert_eq!(owner.inherited_depth(),depth);
    }
    let clone=owner.clone();assert_eq!(clone.inherited_depth(),64);
    let shallow=format!("{owner:?}");assert!(shallow.len()<100);
    let next=fresh();let nodes=budget.nodes.load(Ordering::SeqCst);
    let tracking=Tracking::begin();let result=owner.try_merge(next);let (used,_,_)=tracking.finish();
    assert!(result.unwrap_err().is_resource_exhausted());assert_eq!(used,0);
    assert_eq!(budget.nodes.load(Ordering::SeqCst),nodes);
    assert!(clone.clone().try_merge_with_depth_limit(NativeDataOwners::default(),8).unwrap_err().is_resource_exhausted());
    assert!(NativeDataOwners::default().try_merge_with_depth_limit(NativeDataOwners::default(),0).is_err());
    drop(clone);assert_eq!(budget.live.load(Ordering::SeqCst),0);
}

#[test]
fn literal_evaluator_admits_its_output_schema_columns_and_engine_owner() {
    use arrow_array::{RecordBatch,RecordBatchOptions};
    use arrow_schema::Schema;
    use buoyant_kernel::{EvaluationHandler, engine::arrow_expression::ArrowEvaluationHandler};
    use buoyant_kernel::engine::arrow_data::{ArrowEngineData,EngineDataArrowExt};
    let owner=owner("");let _guard=enter_resource_owner(owner);
    let schema=Arc::new(StructType::new_unchecked(vec![]));
    let evaluator=ArrowEvaluationHandler.new_expression_evaluator(schema,Arc::new(Expression::literal(Scalar::Integer(7))),DataType::INTEGER).unwrap();
    let input=ArrowEngineData::new(RecordBatch::try_new_with_options(Arc::new(Schema::empty()),vec![],&RecordBatchOptions::new().with_row_count(Some(3))).unwrap());
    let tracking=Tracking::begin();let result=evaluator.evaluate(&input);let (used,admitted,early)=tracking.finish();
    assert!(result.is_ok(),"{:?}",result.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}");
    assert_eq!(result.unwrap().try_into_record_batch().unwrap().num_rows(),3);
}

#[test]
fn struct_expression_native_nullable_bitmap_and_fields_are_admitted() {
    use arrow_array::{Array, ArrayRef, BooleanArray, Int32Array, RecordBatch, StructArray};
    use arrow_schema::{DataType as ArrowType, Field, Schema};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    for rows in [0,1,7,64,65,512,513,1025] {
        for offset in [0,1,7,64] {
            let owner=owner("");let _guard=enter_resource_owner(owner);
            let values=Int32Array::from_iter_values(0..(rows+offset) as i32).slice(offset, rows);
            let predicate=BooleanArray::from_iter((0..rows+offset).map(|i|if i%3==0 {None}else{Some(i%2==0)})).slice(offset,rows);
            let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("value",ArrowType::Int32,false),Field::new("valid",ArrowType::Boolean,true)])),vec![Arc::new(values) as ArrayRef,Arc::new(predicate)]).unwrap();
            let kind=DataType::Struct(Box::new(StructType::new_unchecked([StructField::nullable("renamed",DataType::INTEGER)])));
            let expression=Expression::struct_with_nullability_from([Expression::column(["value"])],Expression::column(["valid"]));
            let tracking=Tracking::begin();let output=evaluate_expression(&expression,&batch,Some(&kind));let (used,admitted,early)=tracking.finish();
            assert!(output.is_ok(),"rows={rows},offset={offset}: {:?}",output.as_ref().err());
            assert!(!early && used<=admitted,"rows={rows},offset={offset}: {used}/{admitted}, early={early}");
            let output=output.unwrap();let output=output.as_any().downcast_ref::<StructArray>().unwrap();
            for row in 0..rows { let i=row+offset;assert_eq!(output.is_valid(row),i%3!=0 && i%2==0); }
        }
    }
}

#[test]
fn nested_schema_reconstruction_preserves_values_and_admits_actual_metadata() {
    use arrow_array::{Array, ArrayRef, RecordBatch};
    use arrow_schema::{Field, Schema};
    use buoyant_kernel::{EvaluationHandler, engine::arrow_expression::ArrowEvaluationHandler};
    use buoyant_kernel::engine::arrow_data::{ArrowEngineData,EngineDataArrowExt};
    use buoyant_kernel::expressions::{ArrayData,MapData,ExpressionStructPatchBuilder};
    use buoyant_kernel::schema::{ArrayType,MapType,MetadataValue};
    let nested=Scalar::Array(ArrayData::try_new(ArrayType::new(DataType::STRING,true),[Scalar::String("東京".repeat(64)),Scalar::Null(DataType::STRING)]).unwrap());
    let value=Scalar::Map(MapData::try_new(MapType::new(DataType::STRING,nested.data_type(),true),[(Scalar::String("key".into()),nested)]).unwrap());
    let schema=Arc::new(StructType::new_unchecked([StructField::nullable("renamed",value.data_type()).with_metadata([
        ("parquet.field.id".to_owned(),MetadataValue::Number(7)),
        ("custom metadata".repeat(10),MetadataValue::String("payload".repeat(64))),
    ])]));
    let original=owner("");let weak=Arc::downgrade(&original);let guard=enter_resource_owner(original.clone());
    let column=value.to_array(3).unwrap();
    let input=ArrowEngineData::new(RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("old",column.data_type().clone(),true)])),vec![column.clone() as ArrayRef]).unwrap());
    let evaluator=ArrowEvaluationHandler.new_expression_evaluator(schema.clone(),Arc::new(Expression::struct_patch(ExpressionStructPatchBuilder::new()).unwrap()),DataType::Struct(Box::new(schema.as_ref().clone()))).unwrap();
    let tracking=Tracking::begin();let output=evaluator.evaluate(&input);let (used,admitted,early)=tracking.finish();
    assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}", FIRST.with(Cell::get));
    let output=output.unwrap().try_into_record_batch().unwrap();assert_eq!(output.num_rows(),3);assert_eq!(output.schema().field(0).name(),"renamed");assert_eq!(output.schema().field(0).metadata()["PARQUET:field_id"],"7");
    let escaped=output.schema().field(0).clone();
    drop(output);drop(evaluator);drop(input);drop(column);drop(guard);drop(original);assert!(weak.upgrade().is_some());drop(escaped);assert!(weak.upgrade().is_none());
}

#[test]
fn struct_expression_denies_descriptors_before_any_native_column() {
    use arrow_array::{RecordBatch,RecordBatchOptions};use arrow_schema::Schema;
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    let owner=owner("native_struct_expression_columns");let _guard=enter_resource_owner(owner);
    let expression=Expression::struct_from([Expression::literal(Scalar::String("payload".repeat(64)))]);
    let kind=DataType::Struct(Box::new(StructType::new_unchecked([StructField::nullable("value",DataType::STRING)])));
    let batch=RecordBatch::try_new_with_options(Arc::new(Schema::empty()),vec![],&RecordBatchOptions::new().with_row_count(Some(1024))).unwrap();
    let tracking=Tracking::begin();let result=evaluate_expression(&expression,&batch,Some(&kind));let (used,_,_)=tracking.finish();
    assert!(result.unwrap_err().is_resource_exhausted());assert_eq!(used,0);
}

#[test]
fn malformed_json_null_fallback_admits_native_buffers_and_nested_schema() {
    use arrow_array::{Array,ArrayRef,RecordBatch,StringArray};
    use arrow_schema::{DataType as ArrowType,Field,Schema};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    use arrow_json::resource::{ReaderResourceLimits,ReaderResourcePolicy,ResourceAdmission,ResourceReceipt,ResourceRequest,ResourceExhausted};
    #[derive(Debug)] struct Admission;
    #[derive(Debug)] struct Receipt(usize);
    impl ResourceReceipt for Receipt { fn bytes(&self)->usize {self.0} }
    impl ResourceAdmission for Admission {
        fn try_reserve(&self,request:ResourceRequest)->Result<Arc<dyn ResourceReceipt>,ResourceExhausted> {
            // The test application admits its own receipt Arc before allocating it.
            let own=Layout::new::<[usize;2]>().extend(Layout::new::<Receipt>()).unwrap().0.pad_to_align().size();
            ADMITTED.with(|v|v.set(v.get().checked_add(request.bytes+own).unwrap()));
            Ok(Arc::new(Receipt(request.bytes)))
        }
    }
    let original=owner("");let _guard=enter_resource_owner(original);
    let policy=ReaderResourcePolicy::try_new(Arc::new(Admission),ReaderResourceLimits{allocations:2048,collection_entries:4096,string_bytes:65536,nesting:32}).unwrap();
    let _json_guard=policy.enter_thread().unwrap();
    let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("json",ArrowType::Utf8,true)])),vec![Arc::new(StringArray::from(vec![Some("{ broken"),None])) as ArrayRef]).unwrap();
    let schema=Arc::new(StructType::new_unchecked([StructField::nullable("nested",buoyant_kernel::schema::ArrayType::new(DataType::STRING,true))]));
    let expression=Expression::parse_json(Expression::column(["json"]),schema);
    let tracking=Tracking::begin();let output=evaluate_expression(&expression,&batch,None);let (used,admitted,early)=tracking.finish();
    assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
    let output=output.unwrap();assert_eq!(output.len(),2);assert_eq!(output.null_count(),2);
}

#[test]
fn empty_struct_schema_preserves_explicit_native_rows() {
    use arrow_array::{RecordBatch,RecordBatchOptions};use arrow_schema::Schema;
    use buoyant_kernel::{EvaluationHandler,engine::arrow_expression::ArrowEvaluationHandler};
    use buoyant_kernel::engine::arrow_data::{ArrowEngineData,EngineDataArrowExt};
    use buoyant_kernel::expressions::ExpressionStructPatchBuilder;
    let owner=owner("");let _guard=enter_resource_owner(owner);
    let schema=Arc::new(StructType::new_unchecked([]));
    for expression in [Expression::struct_from([] as [Expression;0]),Expression::struct_patch(ExpressionStructPatchBuilder::new()).unwrap()] {
        let evaluator=ArrowEvaluationHandler.new_expression_evaluator(schema.clone(),Arc::new(expression),DataType::Struct(Box::new(schema.as_ref().clone()))).unwrap();
        let input=ArrowEngineData::new(RecordBatch::try_new_with_options(Arc::new(Schema::empty()),vec![],&RecordBatchOptions::new().with_row_count(Some(9))).unwrap());
        let tracking=Tracking::begin();let output=evaluator.evaluate(&input);let (used,admitted,early)=tracking.finish();
        assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
        assert_eq!(output.unwrap().try_into_record_batch().unwrap().num_rows(),9);
    }
}

#[test]
fn map_to_struct_native_parsers_builders_and_duplicate_resolution_are_admitted() {
    use arrow_array::{Array,ArrayRef,RecordBatch,StructArray,StringArray};use arrow_schema::{Field,Schema};
    use buoyant_kernel::expressions::MapData;
    use buoyant_kernel::schema::{MapType,DecimalType};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    let decimal:DataType=DecimalType::try_new(12,2).unwrap().into();
    let cases=[("text",DataType::STRING,"escaped 東京 ".repeat(1024)),("bytes",DataType::BINARY,"binary".repeat(256)),("number",DataType::LONG,"9223372036854775807".into()),("decimal",decimal,"-1234.50".into()),("date",DataType::DATE,"2026-09-07".into()),("timestamp",DataType::TIMESTAMP,"2026-09-07T12:13:14.123456+05:30".into()),("bool",DataType::BOOLEAN,"true".into()),("empty",DataType::INTEGER,String::new())];
    let mut pairs:Vec<_>=cases.iter().map(|(name,_,value)|(Scalar::String((*name).into()),Scalar::String(value.clone()))).collect();
    pairs.push((Scalar::String("text".into()),Scalar::String("rightmost original".repeat(1024))));
    let value=Scalar::Map(MapData::try_new(MapType::new(DataType::STRING,DataType::STRING,true),pairs).unwrap());
    let schema=DataType::Struct(Box::new(StructType::new_unchecked(cases.iter().map(|(name,kind,_)|StructField::nullable(*name,kind.clone())))));
    let original=owner("");let _guard=enter_resource_owner(original);
    let column=value.to_array(5).unwrap();
    let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("map",column.data_type().clone(),true)])),vec![column as ArrayRef]).unwrap();
    let expression=Expression::map_to_struct(Expression::column(["map"]));
    let tracking=Tracking::begin();let result=evaluate_expression(&expression,&batch,Some(&schema));let (used,admitted,early)=tracking.finish();
    assert!(result.is_ok(),"{:?}",result.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
    let result=result.unwrap();let result=result.as_any().downcast_ref::<StructArray>().unwrap();
    assert_eq!(result.len(),5);assert_eq!(result.column(0).as_any().downcast_ref::<StringArray>().unwrap().value(4),"rightmost original".repeat(1024));assert_eq!(result.column(7).null_count(),5);
}

#[test]
fn map_to_struct_parse_failures_remain_admitted_errors() {
    use arrow_array::{ArrayRef,RecordBatch};use arrow_schema::{Field,Schema};
    use buoyant_kernel::expressions::MapData;use buoyant_kernel::schema::{MapType,DecimalType};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    let decimal:DataType=DecimalType::try_new(5,2).unwrap().into();
    for (kind,raw) in [(DataType::TIMESTAMP,"bad zone ".repeat(1024)),(decimal,"12345678901234567890.00".into()),(DataType::INTEGER,"x".repeat(1024))] {
        let value=Scalar::Map(MapData::try_new(MapType::new(DataType::STRING,DataType::STRING,true),[(Scalar::String("value".into()),Scalar::String(raw))]).unwrap());
        let schema=DataType::Struct(Box::new(StructType::new_unchecked([StructField::nullable("value",kind)])));
        let owner=owner("");let _guard=enter_resource_owner(owner);
        let column=value.to_array(1).unwrap();
        let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("map",column.data_type().clone(),true)])),vec![column as ArrayRef]).unwrap();
        let expression=Expression::map_to_struct(Expression::column(["map"]));
        let tracking=Tracking::begin();let result=evaluate_expression(&expression,&batch,Some(&schema));let (used,admitted,early)=tracking.finish();
        assert!(result.is_err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
    }
}

#[test]
fn null_map_and_empty_stats_keep_their_native_row_counts_without_unadmitted_backing() {
    use arrow_array::{Array,ArrayRef,RecordBatch,StringArray};use arrow_schema::{DataType as ArrowType,Field,Schema};
    use buoyant_kernel::schema::MapType;
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    let owner=owner("");let _guard=enter_resource_owner(owner);
    let schema=Arc::new(StructType::new_unchecked([StructField::nullable("value",DataType::STRING)]));
    let map_kind:DataType=MapType::new(DataType::STRING,DataType::STRING,true).into();
    for rows in [0,3] {
        let column=Scalar::Null(map_kind.clone()).to_array(rows).unwrap();
        let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("map",column.data_type().clone(),true)])),vec![column as ArrayRef]).unwrap();
        let expression=Expression::map_to_struct(Expression::column(["map"]));
        let kind=DataType::Struct(Box::new(schema.as_ref().clone()));
        let tracking=Tracking::begin();let output=evaluate_expression(&expression,&batch,Some(&kind));let (used,admitted,early)=tracking.finish();
        assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
        let output=output.unwrap();assert_eq!(output.len(),rows);assert_eq!(output.null_count(),rows);
    }
    let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("json",ArrowType::Utf8,true)])),vec![Arc::new(StringArray::from(Vec::<&str>::new()))]).unwrap();
    let expression=Expression::parse_json(Expression::column(["json"]),schema);
    let tracking=Tracking::begin();let output=evaluate_expression(&expression,&batch,None);let (used,admitted,early)=tracking.finish();
    assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));assert_eq!(output.unwrap().len(),0);
}

#[test]
fn native_json_encoder_preserves_exact_bytes_with_preadmitted_formatter_scratch() {
    use arrow_array::{Array,ArrayRef,RecordBatch,StructArray,StringArray};
    use arrow_schema::{Field,Schema};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::to_json;
    use buoyant_kernel::expressions::{ArrayData,MapData};use buoyant_kernel::schema::{ArrayType,MapType};
    let nested=Scalar::Array(ArrayData::try_new(ArrayType::new(DataType::STRING,true),[Scalar::String("line\n\"東京".repeat(128)),Scalar::Null(DataType::STRING)]).unwrap());
    let map=Scalar::Map(MapData::try_new(MapType::new(DataType::STRING,DataType::STRING,true),[(Scalar::String("null".into()),Scalar::Null(DataType::STRING)),(Scalar::String("value".into()),Scalar::String("a\\b".into()))]).unwrap());
    let values=[Scalar::Integer(7),Scalar::Long(-9223372036854775807),Scalar::Float(f32::NAN),Scalar::Double(1.23456789),Scalar::Boolean(false),Scalar::String("quote\"\\\n\0東京".repeat(512)),Scalar::Binary(vec![0,10,255]),Scalar::decimal(-12345,12,2).unwrap(),Scalar::Timestamp(1_234_567),Scalar::TimestampNtz(1_234_567),Scalar::Date(20699),nested,map,Scalar::Null(DataType::INTEGER)];
    for rows in [0,1,3] {
        let owner=owner("");let _guard=enter_resource_owner(owner);
        let columns:Vec<ArrayRef>=values.iter().map(|value|value.to_array(rows).unwrap()).collect();
        let fields:Vec<_>=columns.iter().enumerate().map(|(i,col)|Field::new(format!("field\"\\{i}"),col.data_type().clone(),true)).collect();
        let batch=RecordBatch::try_new(Arc::new(Schema::new(fields)),columns).unwrap();
        let array=StructArray::from(batch);
        let input: ArrayRef=Arc::new(array);
        // Legacy encoding gives an independent native byte-for-byte oracle.
        let root=Arc::new(Field::new("root",input.data_type().clone(),true));
        let options=arrow_json::writer::EncoderOptions::default();
        let mut encoder=arrow_json::writer::make_encoder(&root,input.as_ref(),&options).unwrap();
        let mut expected=Vec::new();for row in 0..rows {let mut bytes=Vec::new();encoder.encode(row,&mut bytes);expected.push(bytes);}
        let tracking=Tracking::begin();let output=to_json(&input);let (used,admitted,early)=tracking.finish();
        assert!(output.is_ok(),"rows={rows}: {:?}",output.as_ref().err());assert!(!early && used<=admitted,"rows={rows}: {used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
        let output=output.unwrap();let output=output.as_any().downcast_ref::<StringArray>().unwrap();assert_eq!(output.len(),rows);
        for row in 0..rows {assert_eq!(output.value(row).as_bytes(),expected[row]);}
    }
}

#[test]
fn delta_json_byte_writer_keeps_map_nulls_filters_rows_and_retains_original_bytes_owner() {
    use arrow_array::{Array,ArrayRef,RecordBatch};use arrow_schema::{Field,Schema};
    use buoyant_kernel::engine::arrow_data::ArrowEngineData;
    use buoyant_kernel::engine::arrow_utils::to_json_bytes;
    use buoyant_kernel::engine_data::FilteredEngineData;
    use buoyant_kernel::expressions::MapData;use buoyant_kernel::schema::MapType;
    let value=Scalar::Map(MapData::try_new(MapType::new(DataType::STRING,DataType::STRING,true),[(Scalar::String("key".into()),Scalar::Null(DataType::STRING))]).unwrap());
    for selection in [vec![],vec![false,true],vec![false,false,false]] {
        let original=owner("");let weak=Arc::downgrade(&original);let guard=enter_resource_owner(original.clone());
        let column=value.to_array(3).unwrap();
        let schema=Arc::new(Schema::new(vec![Field::new("map",column.data_type().clone(),true)]));
        let batch=RecordBatch::try_new(schema,vec![column as ArrayRef]).unwrap();
        let data=Box::new(ArrowEngineData::new(batch));
        let filtered=FilteredEngineData::try_new(data,selection.clone()).unwrap();
        let tracking=Tracking::begin();let result=to_json_bytes(std::iter::once(Ok(filtered)));let (used,admitted,early)=tracking.finish();
        assert!(result.is_ok(),"{:?}",result.as_ref().err());assert!(!early && used<=admitted,"{used}/{admitted}, early={early}, first={:?}",FIRST.with(Cell::get));
        let result=result.unwrap();let rows=if selection.is_empty(){3}else{selection.iter().filter(|v|**v).count()+3-selection.len()};
        assert_eq!(&result[..],"{\"map\":{\"key\":null}}\n".repeat(rows).as_bytes());
        let clone=result.clone();let slice=result.slice(..);drop(result);drop(guard);drop(original);assert!(weak.upgrade().is_some());drop(clone);if rows!=0 {assert!(weak.upgrade().is_some());}else{assert!(slice.is_empty());}drop(slice);assert!(weak.upgrade().is_none());
    }
}

#[test]
fn prepared_json_refuses_output_growth_before_original_encode_and_preserves_existing_bytes() {
    use arrow_array::{Array,ArrayRef,StringArray};use arrow_schema::Field;
    use std::sync::atomic::AtomicBool;
    #[derive(Debug)] struct Deny(AtomicBool);
    impl RetainedResourceOwner for Deny {
        fn try_reserve_allocation(&self,request:ResourceAllocationRequest)->Result<(),ResourceOwnerError>{
            if self.0.load(Ordering::Relaxed) {Err(ResourceOwnerError{kind:request.kind,requested:request.bytes,limit:0})}
            else{ADMITTED.with(|v|v.set(v.get()+request.bytes));Ok(())}
        }
        fn try_adopt(&self,_:Arc<dyn RetainedResourceOwner>)->Result<(),ResourceOwnerError>{Ok(())}
        fn record_failure(&self,_:ResourceOwnerError){}
    }
    let owner=Arc::new(Deny(AtomicBool::new(false)));let _guard=enter_resource_owner(owner.clone());
    let array: ArrayRef=Arc::new(StringArray::from(vec!["large value".repeat(8192)]));
    let field=Arc::new(Field::new("value",array.data_type().clone(),true));let options=arrow_json::writer::EncoderOptions::default();
    let mut encoder=arrow_json::writer::make_encoder(&field,array.as_ref(),&options).unwrap();
    let mut output=b"old".to_vec();let pointer=output.as_ptr();let capacity=output.capacity();owner.0.store(true,Ordering::Relaxed);
    let tracking=Tracking::begin();let result=encoder.try_encode(0,&mut output);let (used,_,_)=tracking.finish();
    assert!(matches!(result,Err(arrow_schema::ArrowError::ResourceOwnerError(_))));assert_eq!(used,0);assert_eq!(&output,b"old");assert_eq!(output.as_ptr(),pointer);assert_eq!(output.capacity(),capacity);
}

#[test]
fn selected_stats_native_safe_casts_admit_output_and_malformed_parser_scratch() {
    use arrow_array::{Array, ArrayRef, StringArray, StructArray, RecordBatch};
    use arrow_schema::{DataType as A, Field, Schema, TimeUnit};
    use buoyant_kernel::engine::arrow_utils::safe_cast_back;
    for values in [vec![], vec![Some("2024-01-01")], vec![Some("bad"), None, Some("2024-01-01T00:00:00Z"),Some("1.2.3.4.5.6.7.8.9.10.11")], vec![Some("1.25");65]] {
        for target in [A::Date32,A::Timestamp(TimeUnit::Microsecond,None),A::Timestamp(TimeUnit::Nanosecond,Some("UTC".into())),A::Decimal128(10,2)] {
            let source: ArrayRef=Arc::new(StringArray::from(values.clone()));
            let native=arrow::compute::cast_with_options(source.as_ref(),&target,&arrow::compute::CastOptions{safe:true,..Default::default()}).unwrap();
            let original=owner(""); let weak=Arc::downgrade(&original); let guard=enter_resource_owner(original.clone());
            let child=StructArray::try_new(vec![Field::new("leaf",A::Utf8,true)].into(),vec![source],None).unwrap();
            let input=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("nested",child.data_type().clone(),true)])),vec![Arc::new(child)]).unwrap();
            let schema=Arc::new(Schema::new(vec![Field::new("nested",A::Struct(vec![Field::new("leaf",target.clone(),true)].into()),true)]));
            let tracking=Tracking::begin();let output=safe_cast_back(input,&schema);let(used,admitted,early)=tracking.finish();
            assert!(output.is_ok(),"target={target:?}: {:?}",output.as_ref().err());
            assert!(!early && used<=admitted,"target={target:?}, rows={}: {used}/{admitted}, first={:?}",values.len(),FIRST.with(Cell::get));
            let output=output.unwrap(); let nested=output.column(0).as_any().downcast_ref::<StructArray>().unwrap();
            assert_eq!(nested.column(0).to_data(),native.to_data());
            drop(schema);drop(guard);drop(original);assert!(weak.upgrade().is_some());drop(output);assert!(weak.upgrade().is_none());
        }
    }
}

#[test]
fn stats_output_admission_refusal_precedes_native_parser_and_output_allocation() {
    use arrow_array::{ArrayRef,StringArray,RecordBatch};
    use arrow_schema::{DataType as A,Field,Schema};
    use buoyant_kernel::engine::arrow_utils::safe_cast_back;
    let original=owner("native_stats_cast_values");let _guard=enter_resource_owner(original);
    let source:ArrayRef=Arc::new(StringArray::from(vec!["malformed decimal"]));
    let input=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("value",A::Utf8,true)])),vec![source]).unwrap();
    let target=Arc::new(Schema::new(vec![Field::new("value",A::Decimal128(10,2),true)]));
    let tracking=Tracking::begin();let result=safe_cast_back(input,&target);let(used,_,early)=tracking.finish();
    assert!(matches!(result,Err(buoyant_kernel::Error::ResourceExhausted(_))));
    assert!(!early);assert_eq!(used,std::mem::size_of::<ArrayRef>());
}

#[test]
fn native_checked_arithmetic_and_empty_decimal_outputs_preserve_semantics_with_admission() {
    use arrow_array::{Array,ArrayRef,Int32Array,Int64Array,Float64Array,Decimal128Array,Date32Array,IntervalYearMonthArray,RecordBatch};
    use arrow_schema::{Field,Schema};
    use buoyant_kernel::expressions::BinaryExpressionOp as Op;
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_expression;
    for rows in [0,1,65] {
        let cases:Vec<(ArrayRef,ArrayRef)>=vec![
            (Arc::new(Int32Array::from(vec![Some(8);rows])),Arc::new(Int32Array::from(vec![Some(2);rows]))),
            (Arc::new(Int64Array::from(vec![Some(i64::MAX);rows])),Arc::new(Int64Array::from(vec![Some(2);rows]))),
            (Arc::new(Float64Array::from(vec![Some(1.5);rows])),Arc::new(Float64Array::from(vec![Some(0.5);rows]))),
            (Arc::new(Int32Array::from((0..rows).map(|i|if i%3==0 {None}else{Some(8)}).collect::<Vec<_>>())),Arc::new(Int32Array::from((0..rows).map(|i|if i%3==1 {None}else{Some(2)}).collect::<Vec<_>>()))),
            (Arc::new(Decimal128Array::from(vec![Some(125);rows]).with_precision_and_scale(10,2).unwrap()),Arc::new(Decimal128Array::from(vec![Some(100);rows]).with_precision_and_scale(10,2).unwrap())),
            (Arc::new(Date32Array::from(vec![Some(20_000);rows])),Arc::new(IntervalYearMonthArray::from(vec![Some(1);rows]))),
        ];
        for (left,right) in cases {
            for op in [Op::Plus,Op::Minus,Op::Multiply,Op::Divide] {
                let native=match op {Op::Plus=>arrow::compute::kernels::numeric::add(&left,&right),Op::Minus=>arrow::compute::kernels::numeric::sub(&left,&right),Op::Multiply=>arrow::compute::kernels::numeric::mul(&left,&right),Op::Divide=>arrow::compute::kernels::numeric::div(&left,&right)};
                let original=owner("");let guard=enter_resource_owner(original.clone());
                let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("left",left.data_type().clone(),true),Field::new("right",right.data_type().clone(),true)])),vec![left.clone(),right.clone()]).unwrap();
                let expr=Expression::binary(op,Expression::column(["left"]),Expression::column(["right"]));
                let tracking=Tracking::begin();let actual=evaluate_expression(&expr,&batch,None);let(used,admitted,early)=tracking.finish();
                assert!(!early && used<=admitted,"op={op:?}, type={:?}, rows={rows}: {used}/{admitted}, first={:?}",left.data_type(),FIRST.with(Cell::get));
                match (native,actual) {(Ok(expected),Ok(actual))=>assert_eq!(expected.to_data(),actual.to_data()),(Err(_),Err(_))=>{},(expected,actual)=>panic!("native={expected:?}, actual={actual:?}")};
                drop(guard);
            }
        }
    }
}

#[test]
fn native_predicate_null_boolean_comparison_and_in_bitmaps_are_admitted() {
    use arrow_array::{Array,ArrayRef,BooleanArray,Int32Array,StringArray,RecordBatch,builder::{ListBuilder,Int32Builder,StringBuilder}};
    use arrow_schema::{Field,Schema};
    use buoyant_kernel::expressions::{Predicate,BinaryPredicateOp as Op};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_predicate;
    for rows in [0,1,7,65] {
        for offset in [0,1,3] {
            let bools=BooleanArray::from((0..rows+offset).map(|i|match i%3 {0=>None,1=>Some(true),_=>Some(false)}).collect::<Vec<_>>()).slice(offset,rows);
            let numbers=Int32Array::from((0..rows+offset).map(|i|if i%3==0 {None}else{Some(i as i32)}).collect::<Vec<_>>()).slice(offset,rows);
            let strings=StringArray::from((0..rows+offset).map(|i|if i%3==0 {None}else{Some(if i%3==1 {"first"}else{"second"})}).collect::<Vec<_>>()).slice(offset,rows);
            let mut numbers_list=ListBuilder::new(Int32Builder::new());let mut strings_list=ListBuilder::new(StringBuilder::new());
            for _ in 0..rows {numbers_list.values().append_value(2);numbers_list.values().append_null();numbers_list.append(true);strings_list.values().append_value("second");strings_list.values().append_null();strings_list.append(true);}
            let cols:Vec<ArrayRef>=vec![Arc::new(bools),Arc::new(numbers),Arc::new(strings),Arc::new(numbers_list.finish()),Arc::new(strings_list.finish())];
            let fields:Vec<_>=cols.iter().zip(["bools","numbers","strings","numbers_list","strings_list"]).map(|(a,name)|Field::new(name,a.data_type().clone(),true)).collect();
            let batch=RecordBatch::try_new(Arc::new(Schema::new(fields)),cols).unwrap();
            let mut preds=vec![Predicate::BooleanExpression(Expression::column(["bools"])),Predicate::is_null(Expression::column(["numbers"])),Predicate::and_from([]),Predicate::or_from([]),Predicate::and(Predicate::is_null(Expression::column(["numbers"])),Predicate::BooleanExpression(Expression::column(["bools"]))),Predicate::or(Predicate::is_null(Expression::column(["numbers"])),Predicate::BooleanExpression(Expression::column(["bools"]))),Predicate::binary(Op::In,Expression::literal(2),Expression::column(["numbers_list"])),Predicate::binary(Op::In,Expression::literal("second"),Expression::column(["strings_list"]))];
            for column in ["bools","numbers","strings"] {for op in [Op::LessThan,Op::GreaterThan,Op::Equal,Op::Distinct] {preds.push(Predicate::binary(op,Expression::column([column]),Expression::column([column])));}}
            for pred in preds {for inverted in [false,true] {
                let expected=evaluate_predicate(&pred,&batch,inverted).unwrap();
                let original=owner("");let guard=enter_resource_owner(original.clone());
                let tracking=Tracking::begin();let actual=evaluate_predicate(&pred,&batch,inverted);let(used,admitted,early)=tracking.finish();
                assert!(actual.is_ok(),"{pred:?}: {:?}",actual.as_ref().err());
                assert!(!early && used<=admitted,"{pred:?}, rows={rows}, offset={offset}, inverted={inverted}: {used}/{admitted}, first={:?}",FIRST.with(Cell::get));
                assert_eq!(actual.unwrap(),expected);drop(guard);
            }}
        }
    }
}

#[test]
fn native_mixed_view_comparison_and_overlapping_list_view_normalization_are_admitted() {
    use arrow_array::{Array,ArrayRef,StringArray,StringViewArray,ListViewArray,ListArray,RecordBatch};
    use arrow_buffer::ScalarBuffer;
    use arrow_schema::{DataType as A,Field,Schema};
    use buoyant_kernel::expressions::{Predicate,BinaryPredicateOp as Op};
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::evaluate_predicate;
    for rows in [0,1,65] {
        let original=owner("");let _guard=enter_resource_owner(original);
        let text="original long backing retained in a view";
        let strings=StringArray::from((0..rows).map(|i|if i%3==0{None}else{Some(text)}).collect::<Vec<_>>());
        let views=StringViewArray::from(&strings);
        let input=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("strings",A::Utf8,true),Field::new("views",A::Utf8View,true)])),vec![Arc::new(strings),Arc::new(views)]).unwrap();
        let predicate=Predicate::binary(Op::Equal,Expression::column(["strings"]),Expression::column(["views"]));
        let expected=arrow::compute::kernels::cmp::eq(input.column(1),input.column(1)).unwrap();
        let tracking=Tracking::begin();let actual=evaluate_predicate(&predicate,&input,false);let(used,admitted,early)=tracking.finish();
        assert!(actual.is_ok(),"{:?}",actual.as_ref().err());assert!(!early && used<=admitted,"mixed rows={rows}: {used}/{admitted}, first={:?}",FIRST.with(Cell::get));assert_eq!(actual.unwrap(),expected);
    }
    for views in [false,true] {
        let original=owner("");let _guard=enter_resource_owner(original);
        let strings=StringArray::from(vec![Some("first"),Some("second long retained value"),None,Some("last")]);
        let values:ArrayRef=if views {Arc::new(StringViewArray::from(&strings))} else {Arc::new(strings)};
        let field=Arc::new(Field::new("item",values.data_type().clone(),true).with_metadata([("original".to_owned(),"metadata".to_owned())].into()));
        let list=ListViewArray::try_new(field,ScalarBuffer::from(vec![1i32,0,1,0]),ScalarBuffer::from(vec![3i32,2,2,0]),values,None).unwrap();
        let target=A::List(Arc::new(Field::new("item",A::Utf8,true).with_metadata([("original".to_owned(),"metadata".to_owned())].into())));
        let contiguous=arrow::compute::cast(&list,&target).unwrap();
        let left=StringArray::from(vec!["second long retained value";4]);
        let expected=arrow::compute::kernels::comparison::in_list_utf8(&left,contiguous.as_any().downcast_ref::<ListArray>().unwrap()).unwrap();
        let batch=RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("list",list.data_type().clone(),true)])),vec![Arc::new(list)]).unwrap();
        let predicate=Predicate::binary(Op::In,Expression::literal("second long retained value"),Expression::column(["list"]));
        let tracking=Tracking::begin();let actual=evaluate_predicate(&predicate,&batch,false);let(used,admitted,early)=tracking.finish();
        assert!(actual.is_ok(),"views={views}: {:?}",actual.as_ref().err());assert!(!early && used<=admitted,"views={views}: {used}/{admitted}, first={:?}",FIRST.with(Cell::get));assert_eq!(actual.unwrap(),expected);
    }
}

#[test]
fn append_columns_owns_combined_descriptors_and_preserves_original_column_backing() {
    use arrow_array::{Array,ArrayRef,Int32Array,RecordBatch};
    use arrow_schema::{Field,Schema};
    use buoyant_kernel::{EngineData,engine::arrow_data::{ArrowEngineData,EngineDataArrowExt},expressions::ArrayData,schema::ArrayType};
    for rows in [0,1,65] {
        let original=owner("");let weak=Arc::downgrade(&original);let guard=enter_resource_owner(original.clone());
        let source:ArrayRef=Arc::new(Int32Array::from(vec![7;rows]));let pointer=source.as_any().downcast_ref::<Int32Array>().unwrap().values().as_ptr();
        let input=ArrowEngineData::new(RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("old",arrow_schema::DataType::Int32,false)])),vec![source]).unwrap());
        let schema=Arc::new(StructType::new_unchecked([StructField::nullable("new",DataType::STRING)]));
        let columns=vec![ArrayData::try_new(ArrayType::new(DataType::STRING,true),vec![Scalar::String("new native string".to_owned());rows]).unwrap()];
        let tracking=Tracking::begin();let output=input.append_columns(schema,columns);let(used,admitted,early)=tracking.finish();
        assert!(output.is_ok(),"{:?}",output.as_ref().err());assert!(!early && used<=admitted,"rows={rows}: {used}/{admitted}, first={:?}",FIRST.with(Cell::get));
        let output=output.unwrap().try_into_owned_record_batch().unwrap();assert_eq!(output.record_batch().num_rows(),rows);assert_eq!(output.record_batch().num_columns(),2);
        assert_eq!(output.record_batch().column(0).as_any().downcast_ref::<Int32Array>().unwrap().values().as_ptr(),pointer);
        drop(input);drop(guard);drop(original);assert!(weak.upgrade().is_some());drop(output);assert!(weak.upgrade().is_none());
    }
}

#[test]
fn evaluator_diagnostic_denial_is_inline_and_does_not_allocate_a_replacement_error() {
    use arrow_array::{ArrayRef,Int32Array,RecordBatch};
    use arrow_schema::Schema;
    use buoyant_kernel::engine::arrow_expression::evaluate_expression::{evaluate_expression,coalesce_arrays,to_json};
    let original=owner("native_expression_diagnostic");let _guard=enter_resource_owner(original);
    let batch=RecordBatch::try_new_with_options(Arc::new(Schema::empty()),vec![],&arrow_array::RecordBatchOptions::new().with_row_count(Some(1))).unwrap();
    let expression=Expression::column(["missing"]);
    let input:ArrayRef=Arc::new(Int32Array::from(vec![1]));
    let tracking=Tracking::begin();let result=evaluate_expression(&expression,&batch,None);let(used,_,_)=tracking.finish();assert!(matches!(result,Err(buoyant_kernel::Error::ResourceExhausted(_))));assert_eq!(used,0);
    let tracking=Tracking::begin();let result=coalesce_arrays(&[],None);let(used,_,_)=tracking.finish();assert!(matches!(result,Err(arrow_schema::ArrowError::ResourceOwnerError(_))));assert_eq!(used,0);
    let tracking=Tracking::begin();let result=to_json(&input);let(used,_,_)=tracking.finish();assert!(matches!(result,Err(arrow_schema::ArrowError::ResourceOwnerError(_))));assert_eq!(used,0);
}

#[test]
fn full_validator_compares_original_metadata_without_materializing_strings() {
    use buoyant_kernel::engine::ensure_data_types::{ensure_data_types, ValidationMode};
    use buoyant_kernel::schema::MetadataValue;
    use arrow_schema::{DataType as ArrowType, Field};
    let entries = [
        ("long".to_owned(), MetadataValue::String("東京/\\\"".repeat(128))),
        ("number".to_owned(), MetadataValue::Number(i64::MIN)),
        ("boolean".to_owned(), MetadataValue::Boolean(true)),
        ("other".to_owned(), MetadataValue::Other(serde_json::json!({"nested":[null,true,"string",-42,1.25]}))),
    ];
    let metadata = entries.iter().map(|(key,value)|(key.clone(), value.to_string())).collect();
    let kernel = DataType::Struct(Box::new(StructType::new_unchecked([StructField::nullable("value",DataType::STRING).with_metadata(entries)])));
    let arrow = ArrowType::Struct(vec![Field::new("value",ArrowType::Utf8,true).with_metadata(metadata)].into());
    let owner=owner(""); let _guard=enter_resource_owner(owner);
    let tracking=Tracking::begin(); let result=ensure_data_types(&kernel,&arrow,ValidationMode::Full);let (used,_,_)=tracking.finish();
    assert!(result.is_ok());assert_eq!(used,0);
}

#[test]
fn validator_missing_fields_and_denied_diagnostics_preserve_inline_pressure() {
    use buoyant_kernel::engine::ensure_data_types::{ensure_data_types,ValidationMode};
    use arrow_schema::{DataType as ArrowType,Field};
    let kernel=DataType::Struct(Box::new(StructType::new_unchecked((0..8).map(|n|StructField::nullable(format!("missing{n}"),DataType::STRING)))));
    let arrow=ArrowType::Struct(vec![Field::new("extra",ArrowType::Utf8,true)].into());
    for deny in ["","native_validation_names"] {
        let owner=owner(deny);let _guard=enter_resource_owner(owner);
        let tracking=Tracking::begin();let result=ensure_data_types(&kernel,&arrow,ValidationMode::Full);let (used,admitted,early)=tracking.finish();
        let error=result.err().unwrap();
        if deny.is_empty() {assert!(error.to_string().contains("missing0, missing1, missing2, missing3, missing4"));assert!(!early && used<=admitted,"{used}/{admitted}, first={:?}",FIRST.with(Cell::get));}
        else {assert!(error.is_resource_exhausted());assert_eq!(used,0);}
    }
}

struct BorrowedVisitor { calls:usize, rows:usize, sum:i64 }
impl buoyant_kernel::RowVisitor for BorrowedVisitor {
    fn try_selection(&self)->buoyant_kernel::DeltaResult<buoyant_kernel::engine_data::OwnedVisitorSelection>{
        buoyant_kernel::engine_data::OwnedVisitorSelection::try_from_paths(&[&[]],&[DataType::INTEGER])
    }
    fn selected_column_names_and_types(&self)->(&'static [buoyant_kernel::expressions::ColumnName],&'static [DataType]) {
        static TYPES: [DataType;1]=[DataType::INTEGER]; (&[], &TYPES)
    }
    fn visit<'a>(&mut self,rows:usize,getters:&[&'a dyn buoyant_kernel::engine_data::GetData<'a>])->buoyant_kernel::DeltaResult<()> {
        self.calls+=1;self.rows=rows;
        for row in 0..rows {self.sum+=i64::from(getters[0].get_int(row,"borrowed")?.unwrap_or(0));}
        Ok(())
    }
}
#[test]
fn visitor_borrows_nested_column_paths_and_admits_native_lookup_before_allocation() {
    use arrow_array::{Array, ArrayRef, Int32Array, RecordBatch, StructArray};
    use arrow_schema::{DataType as ArrowType,Field,Schema};
    use buoyant_kernel::{EngineData,engine::arrow_data::ArrowEngineData,expressions::ColumnName};
    for rows in [0,1,65] {
        let owner=owner("");let _guard=enter_resource_owner(owner);
        let nested=StructArray::new(vec![Field::new("leaf.`東京",ArrowType::Int32,true)].into(),vec![Arc::new(Int32Array::from_iter((0..rows).map(|n|if n%3==0 {None}else{Some(n as i32)}))) as ArrayRef],None);
        let schema=Arc::new(Schema::new(vec![Field::new("root.escaped",nested.data_type().clone(),true)]));
        let data=ArrowEngineData::new(RecordBatch::try_new(schema,vec![Arc::new(nested)]).unwrap());
        let names=[ColumnName::new(["root.escaped","leaf.`東京"])];let mut visitor=BorrowedVisitor{calls:0,rows:0,sum:0};
        let tracking=Tracking::begin();let result=data.visit_rows(&names,&mut visitor);let (used,admitted,early)=tracking.finish();
        assert!(result.is_ok(),"{:?}",result.err());assert!(!early && used<=admitted,"{used}/{admitted}, first={:?}",FIRST.with(Cell::get));assert_eq!(visitor.calls,1);assert_eq!(visitor.rows,rows);assert_eq!(visitor.sum,(0..rows).filter(|n|n%3!=0).sum::<usize>() as i64);
    }
}
#[test]
fn visitor_denied_lookup_and_malformed_names_never_invoke_callback() {
    use arrow_array::{Int32Array,RecordBatch};use arrow_schema::{DataType as ArrowType,Field,Schema};
    use buoyant_kernel::{EngineData,engine::arrow_data::ArrowEngineData,expressions::ColumnName};
    for deny in ["native_visitor_prefix_map","native_visitor_getters",""] {
        let owner=owner(deny);let _guard=enter_resource_owner(owner);
        let data=ArrowEngineData::new(RecordBatch::try_new(Arc::new(Schema::new(vec![Field::new("value",ArrowType::Int32,false)])),vec![Arc::new(Int32Array::from(vec![7]))]).unwrap());
        let names=[ColumnName::new([if deny.is_empty(){"absent.`leaf"}else{"value"}])];let mut visitor=BorrowedVisitor{calls:0,rows:0,sum:0};
        let tracking=Tracking::begin();let result=data.visit_rows(&names,&mut visitor);let (used,admitted,early)=tracking.finish();
        let error=result.err().unwrap();assert_eq!(visitor.calls,0);assert!(!early && used<=admitted,"{used}/{admitted}, first={:?}",FIRST.with(Cell::get));
        if deny.is_empty(){assert!(error.to_string().contains("`absent.``leaf`"));}else{assert!(error.is_resource_exhausted());if deny=="native_visitor_prefix_map" {assert_eq!(used,0);}}
    }
}

#[test]
fn visitor_map_and_selection_refusals_precede_native_payload_construction() {
    use arrow_array::{Int32Array, RecordBatch};
    use arrow_schema::{DataType as ArrowType, Field, Schema};
    use buoyant_kernel::engine_data::{GetData, OwnedVisitorSelection};
    use buoyant_kernel::engine::arrow_data::ArrowEngineData;
    use buoyant_kernel::expressions::ColumnName;
    use buoyant_kernel::{DeltaResult, EngineData, RowVisitor};

    struct SelectionProbe {
        selections: Cell<usize>,
        callbacks: usize,
    }
    impl RowVisitor for SelectionProbe {
        fn selected_column_names_and_types(
            &self,
        ) -> (&'static [ColumnName], &'static [DataType]) {
            panic!("the native owned path must not initialize a legacy selector")
        }
        fn try_selection(&self) -> DeltaResult<OwnedVisitorSelection> {
            self.selections.set(self.selections.get() + 1);
            OwnedVisitorSelection::try_from_paths(&[&["value"]], &[DataType::INTEGER])
        }
        fn visit<'a>(&mut self, _: usize, _: &[&'a dyn GetData<'a>]) -> DeltaResult<()> {
            self.callbacks += 1;
            Ok(())
        }
    }

    for (deny, expected_selections) in [
        ("native_visitor_prefix_map", 0),
        ("native_visitor_owned_selection", 1),
    ] {
        let owner = owner(deny);
        let _guard = enter_resource_owner(owner);
        let data = ArrowEngineData::new(RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new("value", ArrowType::Int32, false)])),
            vec![Arc::new(Int32Array::from(vec![7]))],
        ).unwrap());
        let names = [ColumnName::new(["value"])];
        let mut visitor = SelectionProbe { selections: Cell::new(0), callbacks: 0 };
        let tracking = Tracking::begin();
        let result = data.visit_rows(&names, &mut visitor);
        let (allocated, _, early) = tracking.finish();
        assert!(result.unwrap_err().is_resource_exhausted());
        assert_eq!(visitor.selections.get(), expected_selections);
        assert_eq!(visitor.callbacks, 0);
        assert_eq!(allocated, 0, "{deny} allocated native payload before rejection");
        assert!(!early);
    }
}
