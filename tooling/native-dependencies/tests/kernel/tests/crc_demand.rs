#[path = "../../../../../src/resource_budget.rs"]
mod resource_budget;
#[path = "../../../../../src/fabric/native_resource_policy.rs"]
mod native_resource_policy;
#[path = "support/native_owner.rs"]
mod native_owner;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use buoyant_kernel::{Engine, Error, Snapshot, crc::Crc, resource::*};
use buoyant_kernel_engine::DefaultEngineBuilder;

const CRC: &[u8] = br#"{"tableSizeBytes":0,"numFiles":0,"numMetadata":1,"numProtocol":1,"metadata":{"id":"original-owned-id","name":"quote\"\\\n\u0000","format":{"provider":"parquet","options":{}},"schemaString":"{\"type\":\"struct\",\"fields\":[{\"name\":\"id\",\"type\":\"long\",\"nullable\":true,\"metadata\":{}}]}","partitionColumns":[],"configuration":{}},"protocol":{"minReaderVersion":1,"minWriterVersion":2},"setTransactions":[{"appId":"app","version":0,"lastUpdated":0}],"domainMetadata":[{"domain":"example","configuration":"opaque","removed":false}],"fileSizeHistogram":{"sortedBinBoundaries":[0,10],"fileCounts":[0,0],"totalBytes":[0,0]}}"#;

fn limits() -> JsonResourceLimits {
    JsonResourceLimits { max_bytes: 1 << 20, max_tokens: 1 << 16, max_depth: 64, max_string_bytes: 1 << 20, max_container_items: 1 << 16 }
}

#[derive(Debug, Default)]
struct Accounting {
    live: AtomicUsize,
    released: AtomicUsize,
    deny: AtomicBool,
    deny_kind: Mutex<Option<&'static str>>,
    requests: Mutex<Vec<AllocationRequest>>,
}

#[derive(Debug)]
struct Admission(Arc<Accounting>);
#[derive(Debug)]
struct Receipt { bytes: usize, accounting: Arc<Accounting> }
impl AllocationReceipt for Receipt { fn bytes(&self) -> usize { self.bytes } }
impl Drop for Receipt {
    fn drop(&mut self) {
        self.accounting.live.fetch_sub(self.bytes, Ordering::SeqCst);
        self.accounting.released.fetch_add(self.bytes, Ordering::SeqCst);
    }
}
impl AllocationAdmission for Admission {
    fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
        self.0.requests.lock().unwrap().push(request);
        if self.0.deny.load(Ordering::SeqCst)
            || self.0.deny_kind.lock().unwrap().is_some_and(|kind| kind == request.kind)
        {
            return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 });
        }
        self.0.live.fetch_add(request.bytes, Ordering::SeqCst);
        Ok(Arc::new(Receipt { bytes: request.bytes, accounting: self.0.clone() }))
    }
}
fn scope() -> (Arc<NativeResourceScope>, Arc<Accounting>) {
    let accounting = Arc::new(Accounting::default());
    let scope = NativeResourceScope::try_new(Arc::new(Admission(accounting.clone())), 256).unwrap();
    (scope, accounting)
}
fn parse(scope: Arc<NativeResourceScope>) -> Crc {
    Crc::try_from_json_bytes_admitted(CRC, 0, Some(scope), Some(limits())).unwrap()
}
fn request_bytes(accounting: &Accounting, kind: &str) -> usize {
    accounting.requests.lock().unwrap().iter().rev().find(|request| request.kind == kind).unwrap().bytes
}

#[test]
fn crc_denial_precedes_native_deserialization() {
    let (scope, accounting) = scope();
    accounting.deny.store(true, Ordering::SeqCst);
    // Valid JSON geometry, invalid CrcRaw. Admission must win over missing-field serde failure.
    let error = Crc::try_from_json_bytes_admitted(b"{}", 0, Some(scope), Some(limits())).unwrap_err();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("crc_decode_and_conversion"));
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_original_owner_follows_arc_identity_and_releases_once() {
    let (scope, accounting) = scope();
    let crc = Arc::new(parse(scope.clone()));
    let pointer = crc.metadata.id().as_ptr();
    let shared = crc.clone();
    assert!(Arc::ptr_eq(&crc, &shared));
    drop(scope);
    drop(crc);
    assert_eq!(shared.metadata.id().as_ptr(), pointer);
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(shared);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
    assert!(accounting.released.load(Ordering::SeqCst) > 0);
}

#[test]
fn crc_deep_clone_requires_new_admission_and_preserves_original() {
    let (scope, accounting) = scope();
    let crc = parse(scope.clone());
    let copied = crc.try_clone_admitted(None).unwrap();
    assert_eq!(copied, crc);
    assert_ne!(copied.metadata.id().as_ptr(), crc.metadata.id().as_ptr());
    assert!(request_bytes(&accounting, "crc_deep_clone") > 0);
    assert!(copied.resource_scope().is_some());
    accounting.deny.store(true, Ordering::SeqCst);
    assert!(crc.try_clone_admitted(None).unwrap_err().is_resource_exhausted());
    assert_eq!(crc.metadata.id(), "original-owned-id");
    accounting.deny.store(false, Ordering::SeqCst);
    assert!(crc.try_clone_admitted(None).unwrap_err().is_resource_exhausted(), "operation pressure remains latched after the external budget recovers");
    let legacy = crc.clone();
    assert!(legacy.resource_scope().is_none(), "legacy deep clone must not falsely inherit original admission");
    drop(scope);
    drop(crc);
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(copied);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_clone_accounts_reserved_empty_hashmap_capacity() {
    let (scope, accounting) = scope();
    let mut crc = parse(scope.clone());
    crc.set_transaction_state = buoyant_kernel::crc::SetTransactionState::Complete(HashMap::with_capacity(1024));
    // This fixture mutation is deliberately outside the native governed route.
    let copied = crc.try_clone_admitted(None).unwrap();
    assert!(request_bytes(&accounting, "crc_deep_clone") > 1024 * std::mem::size_of::<String>());
    drop((scope, crc, copied));
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_error_keeps_its_native_diagnostic_admission() {
    let (scope, accounting) = scope();
    let error = Crc::try_from_json_bytes_admitted(b"{}", 0, Some(scope.clone()), Some(limits())).unwrap_err();
    assert!(!error.is_resource_exhausted());
    drop(scope);
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(error);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_numeric_lexeme_is_counted_without_string_tokens() {
    fn malformed(number: &str) -> Vec<u8> {
        format!("{{\"protocol\":{{\"readerFeatures\":[{number}]}}}}").into_bytes()
    }
    let (scope, accounting) = scope();
    let first = Crc::try_from_json_bytes_admitted(&malformed("1"), 0, Some(scope.clone()), Some(limits())).unwrap_err();
    let small = request_bytes(&accounting, "crc_decode_and_conversion");
    drop(first);
    let second = Crc::try_from_json_bytes_admitted(&malformed(&"7".repeat(16384)), 0, Some(scope.clone()), Some(limits())).unwrap_err();
    let large = request_bytes(&accounting, "crc_decode_and_conversion");
    assert!(large > small + 16384);
    drop((second, scope));
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_counted_writer_retains_actual_bytes_through_slice_after_delete() {
    let (scope, accounting) = scope();
    let crc = parse(scope.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let engine = DefaultEngineBuilder::new(store.clone()).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope.clone()).build().unwrap();
    let path = url::Url::parse("memory:///0.crc").unwrap();
    buoyant_kernel::crc::try_write_crc_file(&engine, &path, &crc).unwrap();
    let output = engine.storage_handler().read_files(vec![(path.clone(), None)]).unwrap().next().unwrap().unwrap();
    let decoded = Crc::try_from_json_bytes_admitted(&output, 0, None, Some(limits())).unwrap();
    assert_eq!(decoded, crc);
    let sliced = output.slice(0..1);
    let pointer = sliced.as_ptr();
    assert_eq!(pointer, output.as_ptr());
    engine.storage_handler().delete(&path).unwrap();
    drop((scope, engine, crc, output));
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    assert_eq!(sliced[0], b'{');
    drop(sliced);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn crc_writer_denial_precedes_storage_mutation() {
    let (scope, accounting) = scope();
    let crc = parse(scope.clone());
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::memory::InMemory::new())).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope.clone()).build().unwrap();
    let path = url::Url::parse("memory:///denied.crc").unwrap();
    accounting.deny.store(true, Ordering::SeqCst);
    assert!(buoyant_kernel::crc::try_write_crc_file(&engine, &path, &crc).unwrap_err().is_resource_exhausted());
    assert!(engine.storage_handler().head(&path).is_err());
}

#[test]
fn crc_capacity_failure_does_not_become_optional_snapshot_fallback() {
    let (scope, accounting) = scope();
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("_delta_log")).unwrap();
    std::fs::write(directory.path().join("_delta_log/00000000000000000000.crc"), CRC).unwrap();
    std::fs::write(directory.path().join("_delta_log/00000000000000000000.json"), b"{}").unwrap();
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope).build().unwrap();
    *accounting.deny_kind.lock().unwrap() = Some("crc_decode_and_conversion");
    let error = Snapshot::builder_for(url::Url::from_directory_path(directory.path()).unwrap().as_str()).build(&engine).unwrap_err();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("crc_decode_and_conversion"), "{error:?}");
}

#[test]
fn crc_engine_metering_forwards_original_scope() {
    struct RawEngine {
        scope: Arc<NativeResourceScope>,
        store: Arc<object_store::memory::InMemory>,
        executor: Arc<buoyant_kernel_engine::executor::tokio::TokioBackgroundExecutor>,
    }
    impl Engine for RawEngine {
        fn resource_scope(&self) -> Option<Arc<NativeResourceScope>> { Some(self.scope.clone()) }
        fn evaluation_handler(&self) -> Arc<dyn buoyant_kernel::EvaluationHandler> {
            Arc::new(buoyant_kernel::engine::arrow_expression::ArrowEvaluationHandler {})
        }
        fn storage_handler(&self) -> Arc<dyn buoyant_kernel::StorageHandler> {
            Arc::new(buoyant_kernel_engine::filesystem::ObjectStoreStorageHandler::new(self.store.clone(), self.executor.clone()))
        }
        fn json_handler(&self) -> Arc<dyn buoyant_kernel::JsonHandler> {
            Arc::new(buoyant_kernel_engine::json::DefaultJsonHandler::new(self.store.clone(), self.executor.clone()))
        }
        fn parquet_handler(&self) -> Arc<dyn buoyant_kernel::ParquetHandler> {
            Arc::new(buoyant_kernel_engine::parquet::DefaultParquetHandler::new(self.store.clone(), self.executor.clone()))
        }
    }
    let (scope, _) = scope();
    let engine = Arc::new(RawEngine {
        scope: scope.clone(), store: Arc::new(object_store::memory::InMemory::new()),
        executor: Arc::new(buoyant_kernel_engine::executor::tokio::TokioBackgroundExecutor::new()),
    });
    let metered = buoyant_kernel::metrics::MeteredDeltaEngine::new(engine);
    assert!(Arc::ptr_eq(&scope, &metered.resource_scope().unwrap()));
}

#[test]
fn arrow_and_nested_parquet_pressure_remains_typed_kernel_error() {
    let json = arrow::error::ArrowError::ExternalError(Box::new(arrow::json::resource::ResourceExhausted { kind: "json_test", requested: 7, limit: 3 }));
    let error: Error = json.into();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("json_test"));
    let leaf = parquet::errors::ParquetError::ResourceExhausted(parquet::resource::ResourceExhausted { kind: "parquet_test", requested: 9, limit: 2 });
    let wrapped = arrow::error::ArrowError::ExternalError(Box::new(leaf));
    let error: Error = wrapped.into();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("parquet_test"));
}

#[test]
fn arrow_owner_pressure_is_typed_and_latched_before_optional_fallback() {
    let (scope, _) = scope();
    let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits()).unwrap();
    let _thread = policy.enter_thread();
    let pressure = arrow_schema::resource::ResourceOwnerError {
        kind: "native_arrow_owner_test", requested: 5, limit: 0,
    };
    let error: Error = arrow::error::ArrowError::ExternalError(Box::new(
        arrow::error::ArrowError::ResourceOwnerError(pressure),
    )).into();
    assert!(matches!(error, Error::ResourceExhausted(ResourceExhausted {
        kind: "native_arrow_owner_test", requested: 5, limit: 0,
    })));
    assert_eq!(scope.failure().unwrap().kind, "native_arrow_owner_test");
    assert!(scope.reserve(AllocationRequest { kind: "after_swallowed_error", bytes: 1 }).is_err());
}

#[test]
fn crc_required_worker_policy_cannot_be_disabled_or_replaced() {
    let (scope, accounting) = scope();
    let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits()).unwrap();
    let _thread = policy.enter_thread();
    let crc = Crc::try_from_json_bytes_admitted(CRC, 0, None, None).unwrap();
    assert!(Arc::ptr_eq(crc.resource_scope().unwrap(), &scope));
    let foreign_accounting = Arc::new(Accounting::default());
    let foreign = NativeResourceScope::try_new(Arc::new(Admission(foreign_accounting.clone())), 4).unwrap();
    assert!(Crc::try_from_json_bytes_admitted(CRC, 0, Some(foreign.clone()), Some(limits())).unwrap_err().is_resource_exhausted());
    assert!(crc.try_clone_admitted(Some(foreign)).unwrap_err().is_resource_exhausted());
    accounting.deny.store(true, Ordering::SeqCst);
    assert!(Crc::try_from_json_bytes_admitted(CRC, 0, None, None).unwrap_err().is_resource_exhausted());
}

#[test]
fn crc_native_replay_preadmits_owned_getter_materialization() {
    let (scope, accounting) = scope();
    let directory = tempfile::tempdir().unwrap();
    let log = directory.path().join("_delta_log");
    std::fs::create_dir(&log).unwrap();
    std::fs::write(log.join("00000000000000000000.crc"), CRC).unwrap();
    std::fs::write(log.join("00000000000000000000.json"), b"{}").unwrap();
    std::fs::write(log.join("00000000000000000001.json"), b"{\"commitInfo\":{\"operation\":\"WRITE\"}}\n{\"add\":{\"path\":\"a.parquet\",\"size\":8}}\n").unwrap();
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope.clone()).build().unwrap();
    let url = url::Url::from_directory_path(directory.path()).unwrap();
    let snapshot = Snapshot::builder_for(url.as_str())
        .with_incremental_crc_replay(buoyant_kernel::snapshot::IncrementalReplay::UpToCommits(1))
        .build(&engine).unwrap();
    let crc = snapshot.crc().unwrap();
    assert_eq!(crc.version, 1);
    assert_eq!(crc.file_stats().unwrap().num_files(), 1);
    assert_eq!(crc.file_stats().unwrap().table_size_bytes(), 8);
    assert!(Arc::ptr_eq(crc.resource_scope().unwrap(), &scope));
    assert!(request_bytes(&accounting, "crc_replay_owned_batch") > 0);
    assert!(request_bytes(&accounting, "crc_deep_clone") > 0);
    assert!(request_bytes(&accounting, "crc_apply") > 0);
    *accounting.deny_kind.lock().unwrap() = Some("crc_replay_owned_batch");
    let error = Snapshot::builder_for(url.as_str())
        .with_incremental_crc_replay(buoyant_kernel::snapshot::IncrementalReplay::UpToCommits(1))
        .build(&engine).unwrap_err();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("crc_replay_owned_batch"));
}

#[test]
fn crc_native_create_transaction_preadmits_zero_state() {
    use buoyant_kernel::{committer::FileSystemCommitter, schema::{DataType, StructField, StructType}, transaction::{CommitResult, create_table::create_table}};
    let directory = tempfile::tempdir().unwrap();
    let budget = native_owner::budget();
    let owner = native_resource_policy::NativeResourceOwner::try_new(budget.clone(), resource_budget::ResourceClass::Data, native_owner::limits(), &[url::Url::from_directory_path(directory.path()).unwrap()]).unwrap();
    let runtime = native_owner::runtime(owner.clone());
    let entered = runtime.enter();
    let guard = owner.enter_thread().unwrap();
    let scope = owner.scope().clone();
    let before = budget.observation().used.memory_bytes;
    eprintln!("NATIVE_PHASE|owner|{}|{}", before, budget.observation().peak.memory_bytes);
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope.clone()).with_task_executor(buoyant_kernel_engine::executor::tokio::TokioMultiThreadExecutor::try_new_current_shared(runtime.handle().clone()).unwrap()).try_build().unwrap();
    let schema = Arc::new(StructType::try_new([StructField::nullable("id", DataType::LONG)]).unwrap());
    let transaction = create_table(directory.path().to_str().unwrap(), schema, "crc-resource-test").build(&engine, Box::new(FileSystemCommitter::new())).unwrap();
    eprintln!("NATIVE_PHASE|built|{}|{}", budget.observation().used.memory_bytes, budget.observation().peak.memory_bytes);
    let CommitResult::CommittedTransaction(committed) = transaction.commit(&engine).unwrap() else { panic!("native commit failed") };
    let crc = committed.post_commit_snapshot().unwrap().crc().unwrap();
    assert_eq!(crc.version, 0);
    assert!(Arc::ptr_eq(crc.resource_scope().unwrap(), &scope));
    assert!(budget.observation().used.memory_bytes > before);
    assert!(budget.observation().used.memory_bytes > before);
    assert!(budget.observation().used.memory_bytes > before);
    eprintln!("NATIVE_PHASE|committed|{}|{}", budget.observation().used.memory_bytes, budget.observation().peak.memory_bytes);
    drop(guard); drop(entered); drop(runtime);

}

#[test]
fn native_crc_moved_actions_keep_original_receipts() {
    let (scope, accounting) = scope();
    let mut crc = parse(scope.clone());
    let original = crc.metadata.id().as_ptr();
    let metadata = std::mem::take(&mut crc.metadata);
    let protocol = std::mem::take(&mut crc.protocol);
    drop((scope, crc));
    assert_eq!(metadata.id().as_ptr(), original);
    assert!(metadata.resource_scope().is_some());
    assert!(protocol.resource_scope().is_some());
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(metadata);
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(protocol);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn native_action_resource_marker_preserves_generated_schema_and_conversion() {
    use buoyant_kernel::{actions::{Metadata, Protocol}, schema::ToSchema, IntoEngineData};
    let metadata_schema = Metadata::to_schema();
    let metadata_names: Vec<_> = metadata_schema.fields().map(|field| field.name().as_str()).collect();
    assert_eq!(metadata_names, ["id", "name", "description", "format", "schemaString", "partitionColumns", "createdTime", "configuration"]);
    let protocol_schema = Protocol::to_schema();
    let protocol_names: Vec<_> = protocol_schema.fields().map(|field| field.name().as_str()).collect();
    assert_eq!(protocol_names, ["minReaderVersion", "minWriterVersion", "readerFeatures", "writerFeatures"]);
    let budget = native_owner::budget();
    let owner = native_owner::new_owner(budget.clone(), resource_budget::ResourceClass::Data, native_owner::limits()).unwrap();
    let runtime = native_owner::runtime(owner.clone());
    let entered = runtime.enter();
    let guard = owner.enter_thread().unwrap();
    let scope = owner.scope().clone();
    let before = budget.observation().used.memory_bytes;
    let crc = parse(scope.clone());
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::memory::InMemory::new())).with_task_executor(buoyant_kernel_engine::executor::tokio::TokioMultiThreadExecutor::try_new_current_shared(runtime.handle().clone()).unwrap()).try_build().unwrap();
    let batch = crc.protocol.into_engine_data(Arc::new(protocol_schema), &engine).unwrap();
    assert_eq!(batch.len(), 1);
    assert!(budget.observation().used.memory_bytes > before);
    let metadata_batch = crc.metadata.into_engine_data(Arc::new(metadata_schema), &engine).unwrap();
    assert_eq!(metadata_batch.len(), 1);
    drop(guard); drop(entered); drop(runtime);

}

#[test]
fn owned_action_conversion_requires_an_active_native_scope() {
    use buoyant_kernel::{actions::Protocol, schema::ToSchema, IntoEngineData};
    let (scope, _) = scope();
    let crc = parse(scope);
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::memory::InMemory::new())).build().unwrap();
    let error = crc.protocol.into_engine_data(Arc::new(Protocol::to_schema()), &engine).err().unwrap();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("native_action_conversion_scope"));
}

#[test]
fn native_normal_log_replay_admits_original_metadata_and_protocol() {
    let (scope, accounting) = scope();
    let directory = tempfile::tempdir().unwrap();
    let log = directory.path().join("_delta_log");
    std::fs::create_dir(&log).unwrap();
    std::fs::write(log.join("00000000000000000000.json"), br#"{"protocol":{"minReaderVersion":1,"minWriterVersion":2}}
{"metaData":{"id":"original-replay","format":{"provider":"parquet","options":{}},"schemaString":"{\"type\":\"struct\",\"fields\":[{\"name\":\"id\",\"type\":\"long\",\"nullable\":true,\"metadata\":{}}]}","partitionColumns":[],"configuration":{"test":"original-value"}}}
"#).unwrap();
    let engine = DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).with_json_resource_limits(limits()).unwrap().with_resource_scope(scope.clone()).build().unwrap();
    let url = url::Url::from_directory_path(directory.path()).unwrap();
    let snapshot = Snapshot::builder_for(url.as_str()).build(&engine).unwrap();
    let configuration = snapshot.table_configuration();
    assert_eq!(configuration.metadata().id(), "original-replay");
    assert!(Arc::ptr_eq(configuration.metadata().resource_scope().unwrap(), &scope));
    assert!(Arc::ptr_eq(configuration.protocol().resource_scope().unwrap(), &scope));
    assert!(request_bytes(&accounting, "native_metadata_from_getters") > 0);
    assert!(request_bytes(&accounting, "native_protocol_from_getters") > 0);
    *accounting.deny_kind.lock().unwrap() = Some("native_metadata_from_getters");
    let error = Snapshot::builder_for(url.as_str()).build(&engine).unwrap_err();
    assert!(error.is_resource_exhausted());
    assert!(error.to_string().contains("native_metadata_from_getters"));
}

#[test]
fn native_metadata_configuration_insertion_denied_before_mutating_shared_original() {
    let (scope, accounting) = scope();
    let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits()).unwrap();
    let thread = policy.enter_thread();
    let crc = parse(scope.clone());
    let copied = crc.metadata.try_clone_admitted(None).unwrap();
    *accounting.deny_kind.lock().unwrap() = Some("native_metadata_configuration_entry");
    assert!(copied.with_configuration_entry("new-key", "new-value").unwrap_err().is_resource_exhausted());
    assert!(!crc.metadata.configuration().contains_key("new-key"));
    drop(crc); drop(thread); drop(policy); drop(scope);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
