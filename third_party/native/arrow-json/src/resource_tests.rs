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

use super::*;
use crate::ReaderBuilder;
use arrow_array::Array;
use arrow_array::cast::AsArray;
use arrow_schema::{ArrowError, DataType, Field, Schema};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
struct Ledger {
    live: Arc<AtomicUsize>,
    denied: Mutex<Option<&'static str>>,
    requests: Mutex<Vec<ResourceRequest>>,
}
#[derive(Debug)]
struct Receipt {
    live: Arc<AtomicUsize>,
    bytes: usize,
}
impl ResourceReceipt for Receipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Drop for Receipt {
    fn drop(&mut self) {
        self.live.fetch_sub(self.bytes, Ordering::SeqCst);
    }
}
impl ResourceAdmission for Ledger {
    fn try_reserve(
        &self,
        request: ResourceRequest,
    ) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
        self.requests.lock().unwrap().push(request);
        if *self.denied.lock().unwrap() == Some(request.kind) {
            return Err(ResourceExhausted {
                kind: request.kind,
                requested: request.bytes,
                limit: 0,
            });
        }
        self.live.fetch_add(request.bytes, Ordering::SeqCst);
        Ok(Arc::new(Receipt {
            live: self.live.clone(),
            bytes: request.bytes,
        }))
    }
}
fn limits() -> ReaderResourceLimits {
    ReaderResourceLimits {
        allocations: 4096,
        collection_entries: 65536,
        string_bytes: 65536,
        nesting: 128,
    }
}
fn policy(ledger: &Arc<Ledger>) -> ReaderResourcePolicy {
    ReaderResourcePolicy::try_new(ledger.clone(), limits()).unwrap()
}
fn error(error: ArrowError) -> ResourceExhausted {
    match error {
        ArrowError::ResourceOwnerError(e) => ResourceExhausted {
            kind: e.kind,
            requested: e.requested,
            limit: e.limit,
        },
        other => panic!("wrong failure: {other:?}"),
    }
}
fn builder(data_type: DataType, policy: ReaderResourcePolicy) -> ReaderBuilder {
    ReaderBuilder::new(Arc::new(Schema::new(vec![Field::new(
        "v", data_type, true,
    )])))
    .with_batch_size(1)
    .with_resource_policy(policy)
}
#[test]
fn native_resource_denies_tape_growth_before_copy() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Utf8, policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":\"").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON string tape");
    let failure = error(decoder.decode(&[b'x'; 1024]).unwrap_err());
    assert_eq!(failure.kind, "JSON string tape");
    assert!(failure.requested >= 1024);
}
#[test]
fn native_resource_denies_builder_before_scalar_parse() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Int64, policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":\"not a number\"}").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON primitive values");
    assert_eq!(
        error(decoder.flush().unwrap_err()).kind,
        "JSON primitive values"
    );
}
#[test]
fn native_resource_expanded_list_positions_are_admitted() {
    let ledger = Arc::new(Ledger::default());
    let ty = DataType::List(Arc::new(Field::new_list_field(DataType::Int32, true)));
    let mut decoder = builder(ty, policy(&ledger)).build_decoder().unwrap();
    decoder.decode(b"{\"v\":[1,2,3,4,5,6,7,8]}").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON child positions");
    assert_eq!(
        error(decoder.flush().unwrap_err()).kind,
        "JSON child positions"
    );
}
#[test]
fn native_resource_shared_backing_retains_full_policy_after_decoder_drop() {
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    let mut decoder = builder(DataType::Utf8, policy.clone())
        .build_decoder()
        .unwrap();
    decoder
        .decode(b"{\"v\":\"retained native allocation\"}")
        .unwrap();
    let batch = decoder.flush().unwrap().unwrap();
    let values = batch.column(0).as_string::<i32>().values().clone();
    let slice = values.slice(2);
    drop(values);
    drop(batch);
    drop(decoder);
    drop(policy);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    assert_eq!(&slice[..], b"tained native allocation");
    drop(slice);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_resource_lifetime_attachment_denial_rejects_whole_batch() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Int64, policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":2}").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON backing lifetime");
    assert_eq!(
        error(decoder.flush().unwrap_err()).kind,
        "JSON backing lifetime"
    );
    drop(decoder);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_resource_nested_stack_bound_survives_fragmented_input() {
    let ledger = Arc::new(Ledger::default());
    let policy = ReaderResourcePolicy::try_new(
        ledger,
        ReaderResourceLimits {
            nesting: 8,
            ..limits()
        },
    )
    .unwrap();
    let mut decoder = builder(DataType::Int64, policy).build_decoder().unwrap();
    decoder.decode(b"{\"ignored\":[[").unwrap();
    let failure = error(decoder.decode(b"[[[[[[[[[[[[").unwrap_err());
    assert_eq!(failure.kind, "JSON decoder stack");
}
#[test]
fn native_resource_context_restores_on_unwind_and_nested_scope() {
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scoped(Some(&policy), || {
            assert!(current().is_some());
            scoped(None, || assert!(current().is_none()));
            assert!(current().is_some());
            panic!("test unwind");
        })
    }));
    assert!(current().is_none());
    let mut ordinary = ReaderBuilder::new(Arc::new(Schema::new(vec![Field::new(
        "v",
        DataType::Int64,
        true,
    )])))
    .build_decoder()
    .unwrap();
    ordinary.serialize(&[serde_json::json!({"v": 1})]).unwrap();
    assert_eq!(ordinary.flush().unwrap().unwrap().num_rows(), 1);
}
#[test]
fn native_resource_unproved_paths_reject_before_native_work() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Int64, policy(&ledger))
        .build_decoder()
        .unwrap();
    assert_eq!(
        error(decoder.serialize(&[1]).unwrap_err()).kind,
        "JSON Serialize allocation path"
    );
    let ty = DataType::RunEndEncoded(
        Arc::new(Field::new("run_ends", DataType::Int32, false)),
        Arc::new(Field::new("values", DataType::Int64, true)),
    );
    assert_eq!(
        error(builder(ty, policy(&ledger)).build_decoder().unwrap_err()).kind,
        "JSON RunEndEncoded allocation path"
    );
}
#[test]
fn native_resource_nested_map_list_and_large_view_outputs() {
    for ty in [DataType::Utf8, DataType::LargeUtf8, DataType::Utf8View] {
        let ledger = Arc::new(Ledger::default());
        let mut decoder = builder(ty, policy(&ledger)).build_decoder().unwrap();
        for chunk in b"{\"v\":\"abcdefghijklmnop\\uD83D\\uDE00\"}".chunks(1) {
            decoder.decode(chunk).unwrap();
        }
        assert_eq!(decoder.flush().unwrap().unwrap().num_rows(), 1);
    }
    for ty in [
        DataType::Binary,
        DataType::LargeBinary,
        DataType::BinaryView,
        DataType::FixedSizeBinary(16),
    ] {
        let ledger = Arc::new(Ledger::default());
        let mut decoder = builder(ty, policy(&ledger)).build_decoder().unwrap();
        decoder
            .decode(b"{\"v\":\"000102030405060708090a0b0c0d0e0f\"}")
            .unwrap();
        assert_eq!(decoder.flush().unwrap().unwrap().num_rows(), 1);
    }
    let ledger = Arc::new(Ledger::default());
    let entries = Field::new(
        "entries",
        DataType::Struct(
            vec![
                Field::new("key", DataType::Utf8, false),
                Field::new(
                    "value",
                    DataType::List(Arc::new(Field::new_list_field(DataType::Int64, true))),
                    true,
                ),
            ]
            .into(),
        ),
        false,
    );
    let mut decoder = builder(DataType::Map(Arc::new(entries), false), policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder
        .decode(b"{\"v\":{\"a\":[1,2,3],\"b\":[4,5,6,7]}}")
        .unwrap();
    let batch = decoder.flush().unwrap().unwrap();
    assert_eq!(batch.column(0).as_map().entries().len(), 2);
}
#[test]
fn native_resource_checked_tape_geometry_precedes_constructor_allocation() {
    let ledger = Arc::new(Ledger::default());
    let failure = builder(DataType::Int64, policy(&ledger))
        .with_batch_size(usize::MAX)
        .build_decoder()
        .unwrap_err();
    assert!(error(failure).kind.starts_with("JSON tape"));
}

#[test]
fn native_resource_error_keeps_admitted_storage_after_decoder_drop() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Int32, policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":\"bad integer\"}").unwrap();
    let failure = decoder.flush().unwrap_err();
    drop(decoder);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    assert!(failure.to_string().contains("bad integer"));
    drop(failure);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_resource_type_conflict_mode_does_not_hide_admission_failure() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Int32, policy(&ledger))
        .with_ignore_type_conflicts(true)
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":[]}").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON diagnostic");
    assert_eq!(error(decoder.flush().unwrap_err()).kind, "JSON diagnostic");
}
#[test]
fn native_resource_builder_finish_metadata_is_admitted_before_finish() {
    let ledger = Arc::new(Ledger::default());
    let mut decoder = builder(DataType::Utf8, policy(&ledger))
        .build_decoder()
        .unwrap();
    decoder.decode(b"{\"v\":\"data\"}").unwrap();
    *ledger.denied.lock().unwrap() = Some("JSON builder finish metadata");
    assert_eq!(
        error(decoder.flush().unwrap_err()).kind,
        "JSON builder finish metadata"
    );
}

#[test]
fn native_resource_thread_policy_overrides_stale_and_absent_builder_options() {
    let required_ledger = Arc::new(Ledger::default());
    let foreign_ledger = Arc::new(Ledger::default());
    let required = policy(&required_ledger);
    let stale = builder(DataType::Utf8, policy(&foreign_ledger));
    let absent = ReaderBuilder::new(Arc::new(Schema::new(vec![Field::new(
        "v",
        DataType::Utf8,
        true,
    )])))
    .with_batch_size(1);
    let _guard = required.enter_thread().unwrap();
    for builder in [stale, absent] {
        let mut decoder = builder.build_decoder().unwrap();
        decoder.decode(b"{\"v\":\"value\"}").unwrap();
        *required_ledger.denied.lock().unwrap() = Some("JSON string backing");
        assert_eq!(
            error(decoder.flush().unwrap_err()).kind,
            "JSON string backing"
        );
    }
    assert_eq!(
        foreign_ledger.requests.lock().unwrap().len(),
        1,
        "foreign policy only admitted its registry"
    );
}
#[test]
fn native_resource_prebuilt_unguarded_decoder_cannot_enter_required_thread() {
    let mut decoder = ReaderBuilder::new(Arc::new(Schema::new(vec![Field::new(
        "v",
        DataType::Int64,
        true,
    )])))
    .build_decoder()
    .unwrap();
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    let guard = policy.enter_thread().unwrap();
    assert_eq!(
        error(decoder.decode(b"{\"v\":1}").unwrap_err()).kind,
        "JSON preexisting reader owner"
    );
    drop(guard);
    decoder.decode(b"{\"v\":1}").unwrap();
    assert_eq!(decoder.flush().unwrap().unwrap().num_rows(), 1);
}
#[test]
fn native_resource_thread_guard_restores_after_unwind_and_rejects_foreign_nesting() {
    let ledger = Arc::new(Ledger::default());
    let first = policy(&ledger);
    let second = policy(&ledger);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = first.enter_thread().unwrap();
        assert!(second.enter_thread().is_err());
        let nested = first.enter_thread().unwrap();
        drop(nested);
        assert!(required_policy().is_some());
        panic!("thread unwind");
    }));
    assert!(required_policy().is_none());
}
#[test]
fn native_resource_dedicated_workers_inherit_one_owned_policy() {
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    *ledger.denied.lock().unwrap() = Some("JSON primitive values");
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let policy = policy.clone();
            std::thread::spawn(move || {
                let _guard = policy.enter_thread().unwrap();
                let mut decoder = ReaderBuilder::new(Arc::new(Schema::new(vec![Field::new(
                    "v",
                    DataType::Int64,
                    true,
                )])))
                .with_batch_size(1)
                .build_decoder()
                .unwrap();
                decoder.decode(b"{\"v\":1}").unwrap();
                error(decoder.flush().unwrap_err())
            })
        })
        .collect();
    for worker in workers {
        assert_eq!(worker.join().unwrap().kind, "JSON primitive values");
    }
    drop(policy);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_resource_invalid_limits_reject_before_any_native_registry_allocation() {
    for invalid in [
        ReaderResourceLimits {
            allocations: 0,
            ..limits()
        },
        ReaderResourceLimits {
            nesting: 0,
            ..limits()
        },
        ReaderResourceLimits {
            string_bytes: 0,
            ..limits()
        },
        ReaderResourceLimits {
            collection_entries: 0,
            ..limits()
        },
        ReaderResourceLimits {
            allocations: usize::MAX,
            ..limits()
        },
        ReaderResourceLimits {
            string_bytes: usize::MAX,
            ..limits()
        },
    ] {
        let ledger = Arc::new(Ledger::default());
        assert!(ReaderResourcePolicy::try_new(ledger.clone(), invalid).is_err());
        assert!(ledger.requests.lock().unwrap().is_empty());
    }
}

#[test]
fn native_resource_combined_claim_keeps_both_original_and_later_policy_until_last_slice() {
    let first = Arc::new(Ledger::default());
    let later = Arc::new(Ledger::default());
    let first_policy = policy(&first);
    let later_policy = policy(&later);
    let mut decoder = builder(DataType::Utf8, first_policy.clone())
        .build_decoder()
        .unwrap();
    decoder
        .decode(b"{\"v\":\"two independent retained resource banks\"}")
        .unwrap();
    let batch = decoder.flush().unwrap().unwrap();
    scoped(Some(&later_policy), || claim(batch.column(0).as_ref())).unwrap();
    let backing = batch.column(0).as_string::<i32>().values().slice(4);
    drop(batch);
    drop(decoder);
    drop(first_policy);
    drop(later_policy);
    assert!(first.live.load(Ordering::SeqCst) > 0);
    assert!(later.live.load(Ordering::SeqCst) > 0);
    assert_eq!(&backing[..], b"independent retained resource banks");
    drop(backing);
    assert_eq!(first.live.load(Ordering::SeqCst), 0);
    assert_eq!(later.live.load(Ordering::SeqCst), 0);
}

#[test]
fn native_resource_combined_wrapper_refusal_preserves_original_claim() {
    let first = Arc::new(Ledger::default());
    let later = Arc::new(Ledger::default());
    let first_policy = policy(&first);
    let later_policy = policy(&later);
    let array = arrow_array::Int32Array::from(vec![3, 4]);
    scoped(Some(&first_policy), || claim(&array)).unwrap();
    *later.denied.lock().unwrap() = Some("JSON combined backing owner");
    let failure = scoped(Some(&later_policy), || claim(&array)).unwrap_err();
    assert_eq!(error(failure).kind, "JSON combined backing owner");
    let backing = array.values().inner().slice(4);
    drop(array);
    drop(first_policy);
    drop(later_policy);
    assert!(first.live.load(Ordering::SeqCst) > 0);
    assert_eq!(later.live.load(Ordering::SeqCst), 0);
    drop(backing);
    assert_eq!(first.live.load(Ordering::SeqCst), 0);
}

#[test]
fn native_resource_combined_claim_count_is_finite_and_preserves_prior_on_refusal() {
    let first = Arc::new(Ledger::default());
    let later = Arc::new(Ledger::default());
    let first_policy = policy(&first);
    let later_policy = ReaderResourcePolicy::try_new(
        later.clone(),
        ReaderResourceLimits {
            allocations: 1,
            ..limits()
        },
    )
    .unwrap();
    let array = arrow_array::Int32Array::from(vec![8]);
    scoped(Some(&first_policy), || claim(&array)).unwrap();
    let failure = scoped(Some(&later_policy), || claim(&array)).unwrap_err();
    assert_eq!(error(failure).kind, "JSON combined backing count");
    drop(first_policy);
    drop(later_policy);
    assert!(first.live.load(Ordering::SeqCst) > 0);
    assert_eq!(later.live.load(Ordering::SeqCst), 0);
    drop(array);
    assert_eq!(first.live.load(Ordering::SeqCst), 0);
}

#[test]
fn native_resource_denial_converts_to_inline_arrow_error_without_erasing_fields() {
    let denied = ResourceExhausted {
        kind: "inline resource denial",
        requested: 123,
        limit: 7,
    };
    let arrow: ArrowError = denied.into();
    assert!(matches!(arrow, ArrowError::ResourceOwnerError(_)));
    assert_eq!(error(arrow), denied);
}

#[test]
fn native_resource_prepared_encoder_rejects_old_owner_and_byte_limit_before_growth() {
    use crate::writer::{EncoderOptions, make_encoder};
    use arrow_array::StringArray;
    let array = StringArray::from(vec!["payload".repeat(128)]);
    let field = Arc::new(Field::new("value", DataType::Utf8, true));
    let options = EncoderOptions::default();
    let mut preexisting = make_encoder(&field, &array, &options).unwrap();
    let ledger = Arc::new(Ledger::default());
    let mut geometry = limits();
    geometry.string_bytes = 64;
    let policy = ReaderResourcePolicy::try_new(ledger.clone(), geometry).unwrap();
    let guard = policy.enter_thread().unwrap();
    let mut bytes = b"old".to_vec();
    let pointer = bytes.as_ptr();
    let capacity = bytes.capacity();
    let old = error(preexisting.try_encode(0, &mut bytes).unwrap_err());
    assert_eq!(old.kind, "JSON preexisting reader owner");
    let mut encoder = make_encoder(&field, &array, &options).unwrap();
    let requests = ledger.requests.lock().unwrap().len();
    let denied = error(encoder.try_encode(0, &mut bytes).unwrap_err());
    assert_eq!(denied.kind, "JSON encoded bytes");
    assert_eq!(denied.limit, 64);
    assert_eq!(ledger.requests.lock().unwrap().len(), requests);
    assert_eq!(bytes, b"old");
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(bytes.capacity(), capacity);
    drop(guard);
    drop(policy);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    drop(encoder);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn native_resource_prepared_map_null_override_preserves_ordinary_struct_omission() {
    use crate::writer::{EncoderOptions, make_encoder};
    use arrow_array::builder::{MapBuilder, StringBuilder};
    use arrow_array::{ArrayRef, StringArray, StructArray};
    let mut maps = MapBuilder::new(None, StringBuilder::new(), StringBuilder::new());
    maps.keys().append_value("partition");
    maps.values().append_null();
    maps.append(true).unwrap();
    let map = maps.finish();
    let fields = vec![
        Field::new("map", map.data_type().clone(), false),
        Field::new("missing", DataType::Utf8, true),
    ];
    let input = StructArray::try_new(
        fields.into(),
        vec![
            Arc::new(map) as ArrayRef,
            Arc::new(StringArray::from(vec![None::<&str>])),
        ],
        None,
    )
    .unwrap();
    let root = Arc::new(Field::new("root", input.data_type().clone(), false));
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    let _guard = policy.enter_thread().unwrap();
    let options = EncoderOptions::default().with_explicit_nulls_in_maps(true);
    let mut encoder = make_encoder(&root, &input, &options).unwrap();
    let mut output = Vec::new();
    encoder.try_encode_line(0, &mut output).unwrap();
    assert_eq!(output, b"{\"map\":{\"partition\":null}}\n");
    assert!(
        ledger
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.kind == "JSON struct encoders")
    );
}

#[test]
fn native_resource_prepared_custom_format_rejects_before_encoder_allocation() {
    use crate::writer::{EncoderOptions, make_encoder};
    use arrow_array::Date32Array;
    let array = Date32Array::from(vec![0]);
    let field = Arc::new(Field::new("date", DataType::Date32, true));
    let options = EncoderOptions::default().with_date_format("%Y custom".to_owned());
    // Ungoverned compatibility remains the original native formatter.
    assert!(make_encoder(&field, &array, &options).is_ok());
    let ledger = Arc::new(Ledger::default());
    let policy = policy(&ledger);
    let _guard = policy.enter_thread().unwrap();
    let before = ledger.requests.lock().unwrap().len();
    let denied = match make_encoder(&field, &array, &options) {
        Ok(_) => panic!("custom formatter was admitted without a source bound"),
        Err(error) => error,
    };
    assert_eq!(error(denied).kind, "JSON custom temporal format admission");
    assert_eq!(ledger.requests.lock().unwrap().len(), before);
}
