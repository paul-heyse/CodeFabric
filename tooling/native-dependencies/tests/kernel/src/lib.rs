#[cfg(test)]
mod tests {
    use buoyant_kernel::resource::*;

    fn limits() -> JsonResourceLimits {
        JsonResourceLimits {
            max_bytes: 4096,
            max_tokens: 128,
            max_depth: 16,
            max_string_bytes: 64,
            max_container_items: 32,
        }
    }

    #[test]
    fn native_json_geometry_precedes_decode() {
        let mut limits = limits();
        limits.max_string_bytes = 3;
        let mut called = false;
        let result = limits.inspect(br#"{"x":"oversized"}"#).and_then(|_| {
            called = true;
            Ok(())
        });
        assert!(result.unwrap_err().is_resource_exhausted());
        assert!(!called);
    }

    #[test]
    fn native_json_counts_escaped_strings_without_decoding() {
        let shape = limits()
            .inspect(br#"{"x":["a\"b","\u0061",null]}"#)
            .unwrap();
        assert_eq!(shape.tokens, 6);
        assert_eq!(shape.containers, 2);
        assert_eq!(shape.max_depth, 2);
        assert_eq!(shape.string_bytes, 11);
    }

    #[test]
    fn native_json_rejects_depth_count_and_invalid_policy_before_parser() {
        let mut limits = limits();
        limits.max_depth = 2;
        assert!(limits
            .inspect(b"[[[]]]")
            .unwrap_err()
            .is_resource_exhausted());
        limits.max_container_items = 2;
        assert!(limits
            .inspect(b"[0,0,0]")
            .unwrap_err()
            .is_resource_exhausted());
        limits.max_bytes = 0;
        assert!(limits.inspect(b"{}").unwrap_err().is_resource_exhausted());
    }

}

#[cfg(test)]
mod native_fallbacks {
    use std::sync::Arc;
    use buoyant_kernel::{resource::JsonResourceLimits, Snapshot};
    use buoyant_kernel_engine::DefaultEngineBuilder;

    fn table() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("_delta_log");
        std::fs::create_dir(&log).unwrap();
        std::fs::write(log.join("00000000000000000000.json"), concat!(
            "{\"protocol\":{\"minReaderVersion\":1,\"minWriterVersion\":2}}\n",
            "{\"metaData\":{\"id\":\"resource-test\",\"format\":{\"provider\":\"parquet\",\"options\":{}},\"schemaString\":\"{\\\"type\\\":\\\"struct\\\",\\\"fields\\\":[{\\\"name\\\":\\\"id\\\",\\\"type\\\":\\\"long\\\",\\\"nullable\\\":true,\\\"metadata\\\":{}}]}\",\"partitionColumns\":[],\"configuration\":{},\"createdTime\":1}}\n"
        )).unwrap();
        directory
    }

    fn assert_optional_resource_propagates(file: &str) {
        let directory = table();
        let table_url = url::Url::from_directory_path(directory.path()).unwrap();
        let body = format!("{{\"payload\":\"{}\"}}", "x".repeat(256));
        std::fs::write(directory.path().join("_delta_log").join(file), body).unwrap();
        let ordinary = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).build().unwrap();
        // This optional malformed input used to fall back to the valid JSON log.
        assert!(Snapshot::builder_for(table_url.as_str()).build(&ordinary).is_ok());
        let guarded = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new()))
            .with_json_resource_limits(JsonResourceLimits {
                max_bytes: 4096, max_tokens: 128, max_depth: 16,
                max_string_bytes: 64, max_container_items: 32,
            }).unwrap().build().unwrap();
        let error = Snapshot::builder_for(table_url.as_str()).build(&guarded).unwrap_err();
        assert!(error.is_resource_exhausted(), "{error:?}");
        assert!(error.to_string().contains("json_string_bytes"));
    }

    #[test]
    fn native_crc_capacity_cannot_fall_back_to_log_replay() {
        assert_optional_resource_propagates("00000000000000000000.crc");
    }

    #[test]
    fn native_checkpoint_hint_capacity_cannot_fall_back_to_log_replay() {
        assert_optional_resource_propagates("_last_checkpoint");
    }
}

#[cfg(test)]
mod scope_lifetimes {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    use buoyant_kernel::resource::*;

    #[derive(Debug)]
    struct Budget { limit: usize, live: Arc<AtomicUsize> }
    #[derive(Debug)]
    struct Charge { bytes: usize, live: Arc<AtomicUsize> }
    impl AllocationReceipt for Charge { fn bytes(&self) -> usize { self.bytes } }
    impl Drop for Charge { fn drop(&mut self) { self.live.fetch_sub(self.bytes, Ordering::SeqCst); } }
    impl AllocationAdmission for Budget {
        fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
            self.live.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| current.checked_add(request.bytes).filter(|next| *next <= self.limit))
                .map_err(|_| ResourceExhausted { kind: request.kind, requested: request.bytes, limit: self.limit })?;
            Ok(Arc::new(Charge { bytes: request.bytes, live: self.live.clone() }))
        }
    }

    #[test]
    fn native_scope_retains_full_reallocation_peak_until_last_owner() {
        let live = Arc::new(AtomicUsize::new(0));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { limit: 4096, live: live.clone() }), 3).unwrap();
        let bookkeeping = live.load(Ordering::SeqCst);
        scope.reserve(AllocationRequest { kind: "old", bytes: 100 }).unwrap();
        scope.reserve(AllocationRequest { kind: "new", bytes: 200 }).unwrap();
        assert_eq!(live.load(Ordering::SeqCst), bookkeeping + 300);
        let retained = scope.clone();
        drop(scope);
        assert_eq!(live.load(Ordering::SeqCst), bookkeeping + 300);
        drop(retained);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_scope_denies_before_allocation_and_bounds_bookkeeping() {
        let live = Arc::new(AtomicUsize::new(0));
        assert!(NativeResourceScope::try_new(Arc::new(Budget { limit: 0, live: live.clone() }), 8).unwrap_err().is_resource_exhausted());
        assert_eq!(live.load(Ordering::SeqCst), 0);
        let scope = NativeResourceScope::try_new(Arc::new(Budget { limit: 4096, live: live.clone() }), 1).unwrap();
        scope.reserve(AllocationRequest { kind: "first", bytes: 1 }).unwrap();
        let mut allocated = false;
        let result = scope.reserve(AllocationRequest { kind: "second", bytes: 1 }).map(|()| { allocated = true; });
        assert!(result.unwrap_err().is_resource_exhausted());
        assert!(!allocated);
        drop(scope);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
mod schema_admission {
    use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
    use buoyant_kernel::{resource::*, schema::StructType};

    #[derive(Debug)]
    struct Budget { deny: &'static str, live: Arc<AtomicUsize>, requests: Arc<Mutex<Vec<AllocationRequest>>> }
    #[derive(Debug)]
    struct Charge { bytes: usize, live: Arc<AtomicUsize> }
    impl AllocationReceipt for Charge { fn bytes(&self) -> usize { self.bytes } }
    impl Drop for Charge { fn drop(&mut self) { self.live.fetch_sub(self.bytes, Ordering::SeqCst); } }
    impl AllocationAdmission for Budget {
        fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
            self.requests.lock().unwrap().push(request);
            if request.kind == self.deny {
                return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 });
            }
            self.live.fetch_add(request.bytes, Ordering::SeqCst);
            Ok(Arc::new(Charge { bytes: request.bytes, live: self.live.clone() }))
        }
    }
    fn limits() -> JsonResourceLimits {
        JsonResourceLimits { max_bytes: 65536, max_tokens: 4096, max_depth: 32, max_string_bytes: 16384, max_container_items: 1024 }
    }
    const SCHEMA: &str = r#"{"type":"struct","fields":[{"name":"id","type":"long","nullable":true,"metadata":{}},{"name":"payload","type":{"type":"array","elementType":"variant","containsNull":true},"nullable":false,"metadata":{"custom":{"nested":[false,1,"x"]}}}]}"#;

    #[test]
    fn native_schema_denial_precedes_even_malformed_serde_input() {
        for kind in ["native_schema_retained", "native_schema_decode"] {
            let live = Arc::new(AtomicUsize::new(0));
            let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: kind, live: live.clone(), requests: Arc::default() }), 4).unwrap();
            // Serde would report syntax/type failure if it ran first.
            let error = StructType::try_from_json_with_resources("{\"type\":", scope.clone(), limits()).unwrap_err();
            assert!(error.is_resource_exhausted(), "{error:?}");
            assert!(error.to_string().contains(kind));
            drop(scope);
            assert_eq!(live.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn native_schema_original_owner_survives_arc_clones_and_releases_scratch() {
        let live = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: requests.clone() }), 4).unwrap();
        let schema = Arc::new(StructType::try_from_json_with_resources(SCHEMA, scope.clone(), limits()).unwrap());
        assert_eq!(schema.num_fields(), 2);
        assert!(Arc::ptr_eq(schema.resource_scope().unwrap(), &scope));
        let retained_bytes = requests.lock().unwrap().iter().filter(|request| request.kind != "native_schema_decode").map(|request| request.bytes).sum::<usize>();
        assert_eq!(live.load(Ordering::SeqCst), retained_bytes);
        // A deep legacy Clone cannot masquerade as an admitted shared owner.
        assert!(schema.as_ref().clone().resource_scope().is_none());
        let other = schema.clone();
        drop(scope);
        drop(schema);
        assert_eq!(live.load(Ordering::SeqCst), retained_bytes);
        drop(other);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_schema_error_retains_its_decode_charge_until_drop() {
        let live = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: requests.clone() }), 4).unwrap();
        let error = StructType::try_from_json_with_resources("{\"type\":", scope.clone(), limits()).unwrap_err();
        assert!(!error.is_resource_exhausted());
        let total = requests.lock().unwrap().iter().map(|request| request.bytes).sum::<usize>();
        assert_eq!(live.load(Ordering::SeqCst), total);
        drop(scope);
        assert_eq!(live.load(Ordering::SeqCst), total);
        drop(error);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_scope_restores_context_after_unwind_and_omits_automatic_backtrace() {
        let live = Arc::new(AtomicUsize::new(0));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live, requests: Arc::default() }), 4).unwrap();
        assert!(current_resource_scope().is_none());
        let error = with_resource_scope(scope.clone(), limits(), || {
            Ok(buoyant_kernel::Error::generic("native bounded diagnostic").with_backtrace())
        }).unwrap();
        assert!(!matches!(error, buoyant_kernel::Error::Backtraced { .. }));
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: buoyant_kernel::DeltaResult<()> = with_resource_scope(scope, limits(), || panic!("probe unwind"));
        }));
        assert!(unwind.is_err());
        assert!(current_resource_scope().is_none());
    }

    #[test]
    fn native_schema_copy_requires_new_admission_for_distinct_backing() {
        let live = Arc::new(AtomicUsize::new(0));
        let first = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 4).unwrap();
        let original = StructType::try_from_json_with_resources(SCHEMA, first, limits()).unwrap();
        let second = NativeResourceScope::try_new(Arc::new(Budget { deny: "native_schema_copy", live: live.clone(), requests: Arc::default() }), 4).unwrap();
        assert!(original.try_clone_with_resources(second).unwrap_err().is_resource_exhausted());
        let third = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 4).unwrap();
        let copied = original.try_clone_with_resources(third.clone()).unwrap();
        assert_eq!(original, copied);
        assert_ne!(original.field("id").unwrap().name.as_ptr(), copied.field("id").unwrap().name.as_ptr());
        assert!(Arc::ptr_eq(copied.resource_scope().unwrap(), &third));
        drop(original);
        assert!(live.load(Ordering::SeqCst) > 0);
        drop(copied);
        drop(third);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_required_thread_scope_cannot_be_replaced_or_weakened() {
        let live = Arc::new(AtomicUsize::new(0));
        let first = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 8).unwrap();
        let second = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 8).unwrap();
        let mut strict = limits();
        strict.max_bytes = 16;
        let policy = NativeResourceThreadPolicy::try_new(first.clone(), strict).unwrap();
        let guard = policy.enter_thread();
        assert!(Arc::ptr_eq(&current_resource_scope().unwrap(), &first));
        let before = live.load(Ordering::SeqCst);
        let error = StructType::try_from_json_with_resources(SCHEMA, first.clone(), limits()).unwrap_err();
        assert!(error.is_resource_exhausted());
        assert!(error.to_string().contains("json_bytes"));
        assert_eq!(live.load(Ordering::SeqCst), before);
        let error = StructType::try_from_json_with_resources(SCHEMA, second, limits()).unwrap_err();
        assert!(error.to_string().contains("native_foreign_scope"));
        drop(guard);
        assert!(current_resource_scope().is_none());
        // A denial is terminal for this operation even after the thread guard
        // is removed; the owner cannot be recycled to erase resource pressure.
        assert!(first.check_available().unwrap_err().is_resource_exhausted());
        drop(first);
    }

    #[test]
    fn native_required_thread_owners_remain_isolated_across_os_workers() {
        let workers: Vec<_> = (0..2).map(|_| std::thread::spawn(|| {
            let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: Arc::default(), requests: Arc::default() }), 8).unwrap();
            let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits()).unwrap();
            let guard = policy.enter_thread();
            assert!(Arc::ptr_eq(&current_resource_scope().unwrap(), &scope));
            let schema = StructType::try_from_json_with_resources(SCHEMA, scope.clone(), limits()).unwrap();
            assert!(Arc::ptr_eq(schema.resource_scope().unwrap(), &scope));
            drop(guard);
            assert!(current_resource_scope().is_none());
        })).collect();
        for worker in workers { worker.join().unwrap(); }
        assert!(current_resource_scope().is_none());
    }


    #[test]
    fn swallowed_native_exhaustion_is_terminal_and_blocks_later_admission() {
        let live = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "denied", live, requests: requests.clone() }), 8).unwrap();
        let result = with_resource_scope(scope.clone(), limits(), || {
            // Models an upstream trait with Option<T>, such as pruning stats.
            let unavailable = scope.reserve(AllocationRequest { kind: "denied", bytes: 17 }).ok();
            assert!(unavailable.is_none());
            let calls = requests.lock().unwrap().len();
            let error = scope.reserve(AllocationRequest { kind: "would_have_succeeded", bytes: 1 }).unwrap_err();
            assert!(error.to_string().contains("denied"));
            assert_eq!(requests.lock().unwrap().len(), calls);
            Ok(42)
        });
        assert!(result.unwrap_err().is_resource_exhausted());
        assert_eq!(scope.failure().unwrap().kind, "denied");
        scope.record_failure(ResourceExhausted { kind: "later", requested: 2, limit: 1 });
        assert_eq!(scope.failure().unwrap().kind, "denied");
    }

    fn native_metadata() -> buoyant_kernel::actions::Metadata {
        use buoyant_kernel::{actions::Metadata, schema::{DataType, StructField}};
        Metadata::try_new(
            None, None,
            Arc::new(StructType::try_new([
                StructField::nullable("partition", DataType::STRING),
                StructField::nullable("id", DataType::LONG),
            ]).unwrap()),
            vec!["partition".into()], 0,
            [("unrecognized.setting".into(), "original property".into())].into(),
        ).unwrap()
    }

    #[test]
    fn native_configuration_original_schemas_and_property_copies_retain_their_owner() {
        use buoyant_kernel::{actions::Protocol, table_configuration::TableConfiguration};
        let metadata = native_metadata();
        let protocol: Protocol = serde_json::from_str(r#"{"minReaderVersion":1,"minWriterVersion":2}"#).unwrap();
        let live = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: requests.clone() }), 64).unwrap();
        let root = url::Url::parse("file:///tmp/native-configuration/").unwrap();
        let configuration = with_resource_scope(scope.clone(), limits(), || {
            TableConfiguration::try_new(
                metadata.try_clone_admitted(Some(scope.clone()))?,
                protocol.try_clone_admitted(Some(scope.clone()))?, root, 0,
            )
        }).unwrap();
        assert!(Arc::ptr_eq(configuration.resource_scope().unwrap(), &scope));
        let logical = configuration.logical_schema();
        let physical = configuration.physical_schema();
        let filtered = with_resource_scope(scope.clone(), limits(), || logical.with_fields_filtered(|field| field.name() == "id")).unwrap();
        for schema in [logical.as_ref(), physical.as_ref(), &filtered] {
            assert!(Arc::ptr_eq(schema.resource_scope().unwrap(), &scope));
        }
        assert_eq!(filtered.num_fields(), 1);
        assert!(filtered.contains("id"));
        assert_eq!(physical.num_fields(), 2);
        let copied = configuration.try_clone_admitted().unwrap();
        assert!(Arc::ptr_eq(&copied.logical_schema(), &logical));
        let source_property = &configuration.table_properties().unknown_properties["unrecognized.setting"];
        let copy_property = &copied.table_properties().unknown_properties["unrecognized.setting"];
        assert_eq!(source_property, copy_property);
        assert_ne!(source_property.as_ptr(), copy_property.as_ptr());
        assert!(requests.lock().unwrap().iter().any(|request| request.kind == "native_table_configuration_copy"));
        drop(scope); drop(configuration); drop(copied); drop(logical); drop(physical);
        assert!(live.load(Ordering::SeqCst) > 0);
        drop(filtered);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_configuration_and_schema_transform_denials_precede_building() {
        use buoyant_kernel::{actions::Protocol, table_configuration::TableConfiguration};
        for deny in ["native_table_configuration", "native_table_configuration_validation", "native_schema_transform_retained", "native_schema_transform"] {
            let metadata = native_metadata();
            let protocol: Protocol = serde_json::from_str(r#"{"minReaderVersion":1,"minWriterVersion":2}"#).unwrap();
            let scope = NativeResourceScope::try_new(Arc::new(Budget { deny, live: Arc::default(), requests: Arc::default() }), 64).unwrap();
            let root = url::Url::parse("file:///tmp/native-configuration/").unwrap();
            let result = with_resource_scope(scope.clone(), limits(), || {
                TableConfiguration::try_new(metadata.try_clone_admitted(Some(scope.clone()))?, protocol.try_clone_admitted(Some(scope.clone()))?, root, 0)
            });
            let error = result.unwrap_err();
            assert!(error.is_resource_exhausted(), "{error:?}");
            assert!(error.to_string().contains(deny));
        }
    }

    #[test]
    fn native_schema_serialization_counts_original_before_admitting_output() {
        for deny in ["", "native_schema_serialize", "native_schema_serialized_output"] {
            let metadata = native_metadata();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let scope = NativeResourceScope::try_new(Arc::new(Budget { deny, live: Arc::default(), requests: requests.clone() }), 64).unwrap();
            let schema = Arc::new(StructType::try_from_json_with_resources(SCHEMA, scope.clone(), limits()).unwrap());
            let result = with_resource_scope(scope.clone(), limits(), || metadata.with_schema(schema.clone()));
            if deny.is_empty() {
                let updated = result.unwrap();
                assert!(Arc::ptr_eq(updated.resource_scope().unwrap(), &scope));
                let output = requests.lock().unwrap().iter().find(|request| request.kind == "native_schema_serialized_output").unwrap().bytes;
                assert_eq!(updated.schema_string().len(), output);
                let reparsed = with_resource_scope(scope, limits(), || updated.parse_schema()).unwrap();
                assert_eq!(reparsed, *schema);
            } else {
                let error = result.unwrap_err();
                assert!(error.is_resource_exhausted());
                assert!(error.to_string().contains(deny));
            }
        }
    }

    #[test]
    fn native_hint_retains_original_schema_and_actions_and_admits_distinct_copies() {
        use buoyant_kernel::last_checkpoint_hint::{LastCheckpointHint, HintAction};
        let metadata = native_metadata();
        let encoded = serde_json::to_vec(&serde_json::json!({
            "version": 7, "size": 2,
            "checkpointSchema": serde_json::from_str::<serde_json::Value>(SCHEMA).unwrap(),
            "checksum": "original checksum",
            "tags": {"native.tag": "original tag backing"},
            "v2Checkpoint": {"path": "00000000000000000007.checkpoint.native.json", "sidecarFiles": [{"path": "original.parquet", "sizeInBytes": 8, "modificationTime": 0, "tags": {"original": "tag"}}], "nonFileActions": [
                {"metaData": metadata}, {"protocol": {"minReaderVersion": 1, "minWriterVersion": 2}}
            ]}
        })).unwrap();
        let live = Arc::new(AtomicUsize::new(0));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 64).unwrap();
        let hint = Arc::new(LastCheckpointHint::from_bytes_with_resources(&encoded, scope.clone(), limits()).unwrap());
        assert_eq!(hint.version, 7);
        let schema = hint.native_checkpoint_schema().unwrap().clone();
        assert!(Arc::ptr_eq(schema.resource_scope().unwrap(), &scope));
        let original_metadata = match &hint.native_non_file_actions().unwrap()[0] { HintAction::Metadata(metadata) => metadata, _ => unreachable!() };
        assert!(Arc::ptr_eq(original_metadata.resource_scope().unwrap(), &scope));
        let original_sidecar = &hint.native_sidecars().unwrap()[0];
        assert!(Arc::ptr_eq(original_sidecar.resource_scope().unwrap(), &scope));
        let copied = hint.try_clone_admitted().unwrap();
        let copied_sidecar = &copied.native_sidecars().unwrap()[0];
        assert!(Arc::ptr_eq(copied_sidecar.resource_scope().unwrap(), &scope));
        assert_ne!(copied_sidecar.path.as_ptr(), original_sidecar.path.as_ptr());
        let copied_metadata = match &copied.native_non_file_actions().unwrap()[0] { HintAction::Metadata(metadata) => metadata, _ => unreachable!() };
        assert_eq!(copied_metadata, original_metadata);
        assert_ne!(copied_metadata.schema_string().as_ptr(), original_metadata.schema_string().as_ptr());
        assert!(Arc::ptr_eq(copied.native_checkpoint_schema().unwrap(), &schema));
        let clone = hint.clone();
        drop(scope); drop(hint); drop(copied); drop(clone);
        assert!(live.load(Ordering::SeqCst) > 0);
        drop(schema);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn native_hint_pressure_never_becomes_optional_corruption() {
        use buoyant_kernel::last_checkpoint_hint::LastCheckpointHint;
        for deny in ["native_checkpoint_hint_retained", "native_checkpoint_hint_decode"] {
            let scope = NativeResourceScope::try_new(Arc::new(Budget { deny, live: Arc::default(), requests: Arc::default() }), 64).unwrap();
            let error = LastCheckpointHint::from_bytes_with_resources(b"{\"version\":", scope, limits()).unwrap_err();
            assert!(error.is_resource_exhausted());
            assert!(error.to_string().contains(deny));
        }
        let live = Arc::new(AtomicUsize::new(0));
        let scope = NativeResourceScope::try_new(Arc::new(Budget { deny: "", live: live.clone(), requests: Arc::default() }), 64).unwrap();
        let error = LastCheckpointHint::from_bytes_with_resources(b"{\"version\":", scope.clone(), limits()).unwrap_err();
        assert!(!error.is_resource_exhausted());
        drop(scope);
        assert!(live.load(Ordering::SeqCst) > 0);
        drop(error);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

}
