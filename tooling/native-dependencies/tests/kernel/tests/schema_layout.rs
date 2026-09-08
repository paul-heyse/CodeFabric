//! Native source-bound falsifier. This private native probe observes System's
//! requested layouts; it never changes allocation results or production code.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use buoyant_kernel::{resource::*, schema::StructType};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}
struct NativeProbeAllocator;
#[global_allocator]
static ALLOCATOR: NativeProbeAllocator = NativeProbeAllocator;
fn record(bytes: usize) {
    let _ = TRACK.try_with(|track| {
        if track.get() { let _ = ALLOCATED.try_with(|allocated| allocated.set(allocated.get().checked_add(bytes).unwrap())); }
    });
}
// SAFETY: every allocation/deallocation is forwarded to System with unchanged
// layouts and pointers. Counters are nonallocating thread-local Cells; observation
// neither handles failures nor changes allocator results.
unsafe impl GlobalAlloc for NativeProbeAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size); // old requested backing remains in the cumulative sum
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
struct Tracking;
impl Tracking {
    fn begin() -> Self { ALLOCATED.with(|a| a.set(0)); TRACK.with(|a| a.set(true)); Self }
    fn finish(self) -> usize { drop(self); ALLOCATED.with(Cell::get) }
}
impl Drop for Tracking { fn drop(&mut self) { TRACK.with(|a| a.set(false)); } }

#[derive(Debug, Default)]
struct Budget { deny: &'static str, admitted: AtomicUsize }
#[derive(Debug)]
struct Receipt(usize);
impl AllocationReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
impl AllocationAdmission for Budget {
    fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
        if request.kind == self.deny { return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }); }
        self.admitted.fetch_add(request.bytes, Ordering::SeqCst);
        Ok(Arc::new(Receipt(request.bytes)))
    }
}
fn limits() -> JsonResourceLimits {
    JsonResourceLimits { max_bytes: 1 << 20, max_tokens: 65536, max_depth: 96, max_string_bytes: 1 << 18, max_container_items: 16384 }
}
fn schema(field_type: &str, metadata: &str) -> String {
    format!(r#"{{"type":"struct","fields":[{{"name":"I\u0307Σ東京","type":{field_type},"nullable":true,"metadata":{metadata}}}]}}"#)
}

#[test]
fn observed_schema_allocation_sum_stays_within_preparse_native_bound() {
    let mut cases = vec![
        "".into(), "{}".into(), "{\"type\":".into(),
        schema("\"long\"", "{}"), schema("\"variant\"", "{}"),
        schema("\"decimal(38,20)\"", "{}"), schema("\"decimal(1,99999999999999999999999)\"", "{}"),
        schema("{\"type\":\"alien\",\"payload\":[1,2,3]}", "{}"),
        schema("[\"x\",{\"k\":true}]", "{}"),
        schema("\"string\"", "{\"x\":{\"a\":[true,null,-1,1.25,\"x\"]}}"),
        schema("\"string\"", "{\"x\":1e9999999999999999999999999}"),
    ];
    for depth in [1, 2, 8, 20] {
        let mut data_type = "\"long\"".to_string();
        for _ in 0..depth { data_type = format!(r#"{{"type":"array","elementType":{data_type},"containsNull":true}}"#); }
        cases.push(schema(&data_type, "{}"));
        let mut metadata = "[\"escaped\\n\\u0000\",1,false]".to_string();
        for _ in 0..depth { metadata = format!("{{\"nested\":{metadata}}}"); }
        cases.push(schema("\"string\"", &metadata));
    }
    for count in [1, 7, 8, 15, 16, 31, 64, 255] {
        let entries = (0..count).map(|index| format!(r#""key{index}":{{"a":["x",2,false]}}"#)).collect::<Vec<_>>().join(",");
        cases.push(schema("\"long\"", &format!("{{{entries}}}")));
        let fields = (0..count).map(|index| format!(r#"{{"name":"field{index}","type":"variant","nullable":false,"metadata":{{}}}}"#)).collect::<Vec<_>>().join(",");
        cases.push(format!(r#"{{"type":"struct","fields":[{fields}]}}"#));
    }
    for encoded in cases {
        let budget = Arc::new(Budget::default());
        let scope = NativeResourceScope::try_new(budget.clone(), 4).unwrap();
        let before = budget.admitted.load(Ordering::SeqCst);
        let tracking = Tracking::begin();
        let result = StructType::try_from_json_with_resources(&encoded, scope, limits());
        let allocated = tracking.finish();
        let admitted = budget.admitted.load(Ordering::SeqCst) - before;
        assert!(allocated <= admitted, "native allocated {allocated}, admitted {admitted}, input bytes {}", encoded.len());
        drop(result);
    }
}

#[test]
fn denied_schema_admission_precedes_any_native_parser_allocation() {
    let budget = Arc::new(Budget { deny: "native_schema_retained", admitted: AtomicUsize::new(0) });
    let scope = NativeResourceScope::try_new(budget, 4).unwrap();
    let tracking = Tracking::begin();
    let result = StructType::try_from_json_with_resources("{\"type\":", scope, limits());
    let allocated = tracking.finish();
    assert!(result.unwrap_err().is_resource_exhausted());
    assert_eq!(allocated, 0);
}

#[test]
fn observed_native_configuration_and_physical_schema_allocation_is_preadmitted() {
    use buoyant_kernel::{actions::{Metadata, Protocol}, table_configuration::TableConfiguration, table_features::ColumnMappingMode};
    for fields in [1, 7, 16, 64] {
        for mapping in [false, true] {
            let fields_json = (0..fields).map(|index| {
                let metadata = if mapping { format!(r#"{{"delta.columnMapping.id":{},"delta.columnMapping.physicalName":"physical_{index}"}}"#, index + 1) } else { "{}".into() };
                format!(r#"{{"name":"field{index}","type":"string","nullable":true,"metadata":{metadata}}}"#)
            }).collect::<Vec<_>>().join(",");
            let encoded = format!(r#"{{"type":"struct","fields":[{fields_json}]}}"#);
            let mut configuration = serde_json::Map::new();
            for index in 0..fields { configuration.insert(format!("unknown.{index}"), "value\\escaped 東京".into()); }
            if mapping { configuration.insert("delta.columnMapping.mode".into(), "name".into()); }
            configuration.insert("delta.dataSkippingStatsColumns".into(), "`field0`, `strange``name`".into());
            let metadata: Metadata = serde_json::from_value(serde_json::json!({
                "id": "original", "format": {"provider":"parquet","options":{}},
                "schemaString": encoded, "partitionColumns": ["field0"], "configuration": configuration,
            })).unwrap();
            let protocol: Protocol = serde_json::from_str(if mapping {
                r#"{"minReaderVersion":2,"minWriterVersion":5}"#
            } else { r#"{"minReaderVersion":1,"minWriterVersion":2}"# }).unwrap();
            let root = url::Url::parse("file:///tmp/native-layout/").unwrap();
            let budget = Arc::new(Budget::default());
            let scope = NativeResourceScope::try_new(budget.clone(), 128).unwrap();
            let before = budget.admitted.load(Ordering::SeqCst);
            let tracking = Tracking::begin();
            let result = with_resource_scope(scope.clone(), limits(), || {
                let configuration = TableConfiguration::try_new(metadata.try_clone_admitted(Some(scope.clone()))?, protocol.try_clone_admitted(Some(scope.clone()))?, root, 0)?;
                let physical = configuration.logical_schema().make_physical(if mapping { ColumnMappingMode::Name } else { ColumnMappingMode::None })?;
                let copied = configuration.try_clone_admitted()?;
                Ok((configuration, physical, copied))
            });
            let allocated = tracking.finish();
            let admitted = budget.admitted.load(Ordering::SeqCst) - before;
            assert!(allocated <= admitted, "configuration allocated {allocated}, admitted {admitted}");
            let (configuration, physical, copied) = result.unwrap();
            assert_eq!(configuration, copied);
            assert_eq!(physical.num_fields(), fields);
            assert!(physical.contains(if mapping { "physical_0" } else { "field0" }));
        }
    }
}

#[test]
fn observed_native_hint_decode_and_copy_allocation_is_preadmitted() {
    use buoyant_kernel::last_checkpoint_hint::LastCheckpointHint;
    for count in [0, 7, 30, 31, 64, 255] {
        let tags = (0..count).map(|index| (format!("tag{index}"), "longer escaped\\東京".to_owned())).collect::<std::collections::HashMap<_, _>>();
        let sidecars = (0..count).map(|index| serde_json::json!({"path":format!("sidecar{index}.parquet"),"sizeInBytes":40,"modificationTime":0,"tags":tags})).collect::<Vec<_>>();
        let encoded = serde_json::to_vec(&serde_json::json!({
            "version": 5, "size": 1, "tags": tags,
            "checkpointSchema": {"type":"struct","fields":[{"name":"x","type":"variant","nullable":true,"metadata":{"nested":[false,1,{"f":"v"}]}}]},
            "v2Checkpoint":{"path":"00000000000000000005.checkpoint.native.json","sidecarFiles":sidecars,"nonFileActions":[{"protocol":{"minReaderVersion":3,"minWriterVersion":7,"readerFeatures":["variantType"],"writerFeatures":["variantType"]}}]}
        })).unwrap();
        let mut generous = limits(); generous.max_bytes = 8 << 20; generous.max_tokens = 1 << 20;
        let budget = Arc::new(Budget::default());
        let scope = NativeResourceScope::try_new(budget.clone(), 8).unwrap();
        let before = budget.admitted.load(Ordering::SeqCst);
        let tracking = Tracking::begin();
        let result = LastCheckpointHint::from_bytes_with_resources(&encoded, scope, generous).and_then(|hint| hint.try_clone_admitted());
        let allocated = tracking.finish();
        let admitted = budget.admitted.load(Ordering::SeqCst) - before;
        assert!(allocated <= admitted, "hint allocated {allocated}, admitted {admitted}, count {count}");
        result.unwrap();
    }
    let budget = Arc::new(Budget { deny: "native_checkpoint_hint_retained", admitted: AtomicUsize::new(0) });
    let scope = NativeResourceScope::try_new(budget, 4).unwrap();
    let tracking = Tracking::begin();
    let result = LastCheckpointHint::from_bytes_with_resources(b"{\"version\":", scope, limits());
    let allocated = tracking.finish();
    assert!(result.unwrap_err().is_resource_exhausted());
    assert_eq!(allocated, 0);
}

#[test]
fn typed_schema_filter_and_metadata_rebuild_admit_original_allocations_and_retain_owner() {
    use buoyant_kernel::actions::Metadata;
    use buoyant_kernel::schema::{MetadataColumnSpec, ToSchema};
    for keep_fields in [false, true] {
        let budget = Arc::new(Budget::default());
        let scope = NativeResourceScope::try_new(budget.clone(), 32).unwrap();
        let weak = Arc::downgrade(&scope);
        let result = with_resource_scope(scope.clone(), limits(), || {
            let source = Metadata::try_to_schema()?;
            let before = budget.admitted.load(Ordering::SeqCst);
            let tracking = Tracking::begin();
            let rebuilt = source.try_add_metadata_column_with_resources("_file", MetadataColumnSpec::FilePath)?;
            let filtered = rebuilt.with_fields_filtered(|field| keep_fields && field.get_metadata_column_spec().is_none())?;
            let output = filtered.try_into_arc()?;
            let allocated = tracking.finish();
            let admitted = budget.admitted.load(Ordering::SeqCst) - before;
            assert!(allocated <= admitted, "native rebuilt {allocated}, admitted {admitted}");
            assert_eq!(output.num_fields(), if keep_fields { source.num_fields() } else { 0 });
            if keep_fields { assert_eq!(*output, source); }
            assert!(Arc::ptr_eq(output.resource_scope().unwrap(), &scope));
            Ok(output)
        }).unwrap();
        drop(scope);
        assert!(weak.upgrade().is_some(), "even an empty filtered schema owns its admitted root");
        drop(result);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn schema_rebuild_denial_precedes_clone_or_filter_callback_and_unowned_source_stays_rejected() {
    use buoyant_kernel::actions::Metadata;
    use buoyant_kernel::schema::{MetadataColumnSpec, ToSchema};
    for deny in ["native_schema_rebuild_retained", "native_schema_rebuild"] {
        for add_column in [false, true] {
            let budget = Arc::new(Budget { deny, admitted: AtomicUsize::new(0) });
            let scope = NativeResourceScope::try_new(budget, 16).unwrap();
            with_resource_scope(scope, limits(), || {
                let source = Metadata::try_to_schema()?;
                let called = Cell::new(false);
                let tracking = Tracking::begin();
                let result = if add_column {
                    source.try_add_metadata_column_with_resources("_file", MetadataColumnSpec::FilePath)
                } else { source.with_fields_filtered(|_| { called.set(true); true }) };
                let allocated = tracking.finish();
                assert!(result.unwrap_err().is_resource_exhausted());
                // A successful retained admission creates its receipt before a
                // later scratch denial; no schema allocation or callback occurs.
                assert_eq!(allocated, if deny == "native_schema_rebuild" { 2 * std::mem::size_of::<usize>() + std::mem::size_of::<Receipt>() } else { 0 });
                assert!(!called.get());
                Ok(())
            }).unwrap_err(); // The same native scope latches the typed refusal.
        }
    }
    let unowned = Metadata::to_schema();
    let scope = NativeResourceScope::try_new(Arc::new(Budget::default()), 8).unwrap();
    let called = Cell::new(false);
    let tracking = Tracking::begin();
    let result = with_resource_scope(scope, limits(), || unowned.with_fields_filtered(|_| { called.set(true); true }));
    let allocated = tracking.finish();
    assert!(result.unwrap_err().to_string().contains("native_schema_unadmitted_transform_source"));
    assert_eq!(allocated, 0);
    assert!(!called.get());
}

#[test]
fn moved_schema_arc_rejects_a_different_bank_before_allocating_its_owner() {
    use buoyant_kernel::{actions::Metadata, schema::ToSchema};
    let original = NativeResourceScope::try_new(Arc::new(Budget::default()), 8).unwrap();
    let schema = with_resource_scope(original.clone(), limits(), Metadata::try_to_schema).unwrap();
    let weak = Arc::downgrade(&original);
    drop(original);
    let current = NativeResourceScope::try_new(Arc::new(Budget::default()), 8).unwrap();
    let tracking = Tracking::begin();
    let result = with_resource_scope(current, limits(), || schema.try_into_arc());
    let allocated = tracking.finish();
    assert!(result.unwrap_err().to_string().contains("native_schema_unadmitted_arc_source"));
    assert_eq!(allocated, 0);
    assert!(weak.upgrade().is_none());
}
