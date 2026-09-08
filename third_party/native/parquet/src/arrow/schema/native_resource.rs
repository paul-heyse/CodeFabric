//! Borrowed source-layout admission for Arrow IPC 59.2 schema hints.
//! See IPC_RESOURCE_BOUND.md for the exact native allocation inventory.
use std::collections::HashMap;
use std::mem::size_of;

use crate::errors::Result;
use crate::file::properties::WriterProperties;
use crate::resource::{ReaderResourcePolicy, ResourceExhausted};
use arrow_schema::{DataType, Field, Schema};

fn overflow() -> ResourceExhausted {
    ResourceExhausted {
        kind: "writer IPC schema layout",
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .filter(|v| *v <= isize::MAX as usize)
        .ok_or_else(|| overflow().into())
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .filter(|v| *v <= isize::MAX as usize)
        .ok_or_else(|| overflow().into())
}
// Sum of complete RawVec replacements, including native minimum4 (8 for u8).
fn growth(count: usize, width: usize) -> Result<usize> {
    if count == 0 {
        return Ok(0);
    }
    mul(mul(count.max(if width == 1 { 8 } else { 4 }), 4)?, width)
}

struct Bound<'a> {
    policy: &'a ReaderResourcePolicy,
    wire: usize,
    scratch: usize,
    tables: usize,
    fields: usize,
    dictionaries: usize,
}
impl Bound<'_> {
    fn table(&mut self) -> Result<()> {
        // Generated Schema.fbs/Message.fbs tables have at most8 slots and
        // scalars/offsets <=8 bytes. Include alignment before every slot,
        // vtable length/object-size entries, and the signed table offset.
        self.wire = add(self.wire, 8 * (8 + 7) + (8 + 2) * 2 + 4)?;
        self.tables = add(self.tables, 1)?;
        Ok(())
    }
    fn string(&mut self, text: &str) -> Result<()> {
        ReaderResourcePolicy::check(
            text.len(),
            self.policy.limits().string_bytes,
            "writer IPC string",
        )?;
        self.wire = add(self.wire, add(text.len(), 4 + 1 + 7)?)?;
        Ok(())
    }
    fn vector(&mut self, count: usize, width: usize, allocated: bool) -> Result<()> {
        ReaderResourcePolicy::check(
            count,
            self.policy.limits().collection_entries,
            "writer IPC collection",
        )?;
        self.wire = add(self.wire, add(mul(count, width)?, 4 + 7)?)?;
        if allocated {
            self.scratch = add(self.scratch, growth(count, width)?)?;
        }
        Ok(())
    }
    fn metadata(&mut self, metadata: &HashMap<String, String>) -> Result<()> {
        if metadata.is_empty() {
            return Ok(());
        }
        self.vector(metadata.len(), size_of::<u32>(), true)?;
        // keys.collect + stable sort scratch: native driftsort uses at most
        // n borrowed references, separately from the collected key Vec.
        self.scratch = add(
            self.scratch,
            add(
                growth(metadata.len(), size_of::<&String>())?,
                mul(metadata.len(), size_of::<&String>())?,
            )?,
        )?;
        for (key, value) in metadata {
            self.string(key)?;
            self.string(value)?;
            self.table()?;
        }
        Ok(())
    }
    fn depth(&self, depth: usize) -> Result<()> {
        ReaderResourcePolicy::check(
            depth,
            self.policy.limits().schema_depth,
            "writer IPC schema depth",
        )?;
        Ok(())
    }
    fn field(&mut self, field: &Field, depth: usize) -> Result<()> {
        self.depth(depth)?;
        self.fields = add(self.fields, 1)?;
        ReaderResourcePolicy::check(
            self.fields,
            self.policy.limits().collection_entries,
            "writer IPC total fields",
        )?;
        self.table()?;
        self.string(field.name())?;
        self.metadata(field.metadata())?;
        self.data_type(field.data_type(), depth)?;
        if matches!(field.data_type(), DataType::Dictionary(_, _)) {
            self.table()?;
            self.table()?;
            self.dictionaries = add(self.dictionaries, 1)?;
        }
        Ok(())
    }
    fn data_type(&mut self, data_type: &DataType, depth: usize) -> Result<()> {
        self.depth(depth)?;
        if let DataType::Dictionary(_, value) = data_type {
            return self.data_type(value, add(depth, 1)?);
        }
        self.table()?;
        match data_type {
            DataType::Struct(fields) => {
                self.vector(fields.len(), 4, true)?;
                for field in fields {
                    self.field(field, add(depth, 1)?)?;
                }
            }
            DataType::Union(fields, _) => {
                self.vector(fields.len(), 4, true)?;
                self.vector(fields.len(), 4, true)?;
                for (_, field) in fields.iter() {
                    self.field(field, add(depth, 1)?)?;
                }
            }
            DataType::List(field)
            | DataType::LargeList(field)
            | DataType::ListView(field)
            | DataType::LargeListView(field)
            | DataType::FixedSizeList(field, _)
            | DataType::Map(field, _) => {
                self.vector(1, 4, false)?;
                self.field(field, add(depth, 1)?)?;
            }
            DataType::RunEndEncoded(ends, values) => {
                self.vector(2, 4, false)?;
                self.field(ends, add(depth, 1)?)?;
                self.field(values, add(depth, 1)?)?;
            }
            DataType::Timestamp(_, timezone) => {
                self.string(timezone.as_deref().unwrap_or_default())?;
                self.vector(0, 4, false)?;
            }
            _ => self.vector(0, 4, false)?,
        }
        Ok(())
    }
}

fn metadata_clone(metadata: &HashMap<String, String>) -> Result<usize> {
    if metadata.capacity() == 0 {
        return Ok(0);
    }
    // hashbrown0.17 RawTable clone preserves source bucket count, including
    // spare capacity. For nonempty native table capacity c, buckets <=2(c+1).
    let buckets = mul(add(metadata.capacity(), 1)?, 2)?;
    let mut bytes = add(mul(buckets, size_of::<(String, String)>() + 1)?, 31)?;
    for (key, value) in metadata {
        bytes = add(bytes, add(key.len(), value.len())?)?;
    }
    Ok(bytes)
}
fn type_clone(data_type: &DataType, depth: usize, policy: &ReaderResourcePolicy) -> Result<usize> {
    ReaderResourcePolicy::check(
        depth,
        policy.limits().schema_depth,
        "writer IPC clone depth",
    )?;
    match data_type {
        DataType::Dictionary(key, value) => add(
            2 * size_of::<DataType>(),
            add(
                type_clone(key, add(depth, 1)?, policy)?,
                type_clone(value, add(depth, 1)?, policy)?,
            )?,
        ),
        // Nested fields, UnionFields and timezone strings use original Arcs.
        _ => Ok(0),
    }
}

/// Admit the exact selected native encoder call before any intermediate Vec,
/// FlatBuffer, cloned field, String, or WriterProperties metadata growth.
pub(super) fn admit(schema: &Schema, props: &WriterProperties) -> Result<()> {
    let Some(policy) = crate::resource::current() else {
        return Ok(());
    };
    let mut bound = Bound {
        policy: &policy,
        wire: 0,
        scratch: 0,
        tables: 0,
        fields: 0,
        dictionaries: 0,
    };
    bound.table()?;
    bound.table()?; // Schema + Message
    bound.vector(schema.fields().len(), 4, true)?;
    bound.metadata(schema.metadata())?;
    for field in schema.fields() {
        bound.field(field, 1)?;
    }
    // Root offset and final max alignment. Serialized size is < native 2GiB.
    let wire = add(bound.wire, 4 + 7)?;
    ReaderResourcePolicy::check(
        wire,
        policy.limits().footer_bytes.min((1 << 31) - 1),
        "writer IPC wire bytes",
    )?;
    let prefixed = add(wire, 8)?;
    let base64 = mul(add(prefixed, 2)? / 3, 4)?;
    let mut bytes = add(bound.scratch, growth(wire, 1)?)?;
    bytes = add(bytes, growth(8, 8)?)?; // field_locs: native {u16,u32}, at most8 slots
    bytes = add(bytes, growth(bound.tables, 4)?)?;
    bytes = add(bytes, growth(bound.dictionaries, 8)?)?;
    bytes = add(bytes, add(add(wire, prefixed)?, add(base64, 8)?)?)?;
    bytes = add(bytes, super::super::ARROW_SCHEMA_META_KEY.len())?;
    if let Some(metadata) = &props.key_value_metadata {
        // Native remove+push needs no growth when replacing an existing hint.
        if !metadata
            .iter()
            .any(|item| item.key == super::super::ARROW_SCHEMA_META_KEY)
            && metadata.len() == metadata.capacity()
        {
            bytes = add(
                bytes,
                mul(
                    add(metadata.len(), 1)?
                        .max(4)
                        .max(mul(metadata.capacity(), 2)?),
                    size_of::<crate::file::metadata::KeyValue>(),
                )?,
            )?;
        }
    } else {
        bytes = add(bytes, 4 * size_of::<crate::file::metadata::KeyValue>())?;
    }
    if schema
        .fields()
        .iter()
        .any(|field| matches!(field.data_type(), DataType::RunEndEncoded(_, _)))
    {
        // Existing flatten_ree_field clones every top-level field, then creates
        // new per-field Arcs and the Fields Arc slice. Include both Vec layouts.
        let count = schema.fields().len();
        bytes = add(bytes, metadata_clone(schema.metadata())?)?;
        bytes = add(
            bytes,
            mul(
                count,
                size_of::<Field>()
                    + 2 * size_of::<std::sync::Arc<Field>>()
                    + 2 * size_of::<usize>()
                    + size_of::<Field>(),
            )?,
        )?;
        bytes = add(bytes, 2 * size_of::<usize>())?;
        for field in schema.fields() {
            bytes = add(
                bytes,
                add(field.name().len(), metadata_clone(field.metadata())?)?,
            )?;
            bytes = add(bytes, type_clone(field.data_type(), 1, &policy)?)?;
            if let DataType::RunEndEncoded(_, value) = field.data_type() {
                bytes = add(bytes, type_clone(value.data_type(), 1, &policy)?)?;
            }
        }
    }
    ReaderResourcePolicy::check(
        bytes,
        policy.limits().output_bytes,
        "writer IPC allocation bytes",
    )?;
    policy.reserve(bytes, "writer IPC schema encoding")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{ResourceAdmission, ResourceReceipt, ResourceRequest};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    thread_local! {
        static TRACK: Cell<bool> = const { Cell::new(false) };
        static USED: Cell<usize> = const { Cell::new(0) };
        static ADMITTED: Cell<usize> = const { Cell::new(0) };
        static EARLY: Cell<bool> = const { Cell::new(false) };
    }
    struct Observer;
    #[global_allocator]
    static ALLOCATOR: Observer = Observer;
    fn observe(bytes: usize) {
        let _ = TRACK.try_with(|track| {
            if track.get() {
                USED.with(|used| {
                    let total = used.get().saturating_add(bytes);
                    used.set(total);
                    ADMITTED.with(|admitted| {
                        if total > admitted.get() {
                            EARLY.with(|early| early.set(true));
                        }
                    });
                });
            }
        });
    }
    // SAFETY: This falsifier passes every pointer/layout unchanged to System.
    // Observation is allocation-free TLS and never changes allocation results.
    unsafe impl GlobalAlloc for Observer {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            observe(layout.size());
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            observe(layout.size());
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
            observe(bytes);
            unsafe { System.realloc(ptr, layout, bytes) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }
    struct Tracking;
    impl Tracking {
        fn begin() -> Self {
            USED.with(|v| v.set(0));
            ADMITTED.with(|v| v.set(0));
            EARLY.with(|v| v.set(false));
            TRACK.with(|v| v.set(true));
            Self
        }
        fn finish(self) -> (usize, usize, bool) {
            drop(self);
            (
                USED.with(Cell::get),
                ADMITTED.with(Cell::get),
                EARLY.with(Cell::get),
            )
        }
    }
    impl Drop for Tracking {
        fn drop(&mut self) {
            TRACK.with(|v| v.set(false));
        }
    }
    #[derive(Debug)]
    struct Receipt(AtomicUsize);
    impl ResourceReceipt for Receipt {
        fn bytes(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }
    }
    #[derive(Debug)]
    struct Admission {
        deny: bool,
        next: AtomicUsize,
        receipts: Vec<Arc<Receipt>>,
    }
    impl ResourceAdmission for Admission {
        fn try_reserve(
            &self,
            request: ResourceRequest,
        ) -> std::result::Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
            if self.deny && request.kind == "writer IPC schema encoding" {
                return Err(ResourceExhausted {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
            }
            let index = self.next.fetch_add(1, Ordering::Relaxed);
            let receipt = &self.receipts[index];
            receipt.0.store(request.bytes, Ordering::Relaxed);
            ADMITTED.with(|v| v.set(v.get().checked_add(request.bytes).unwrap()));
            Ok(receipt.clone())
        }
    }
    fn policy(deny: bool) -> ReaderResourcePolicy {
        ReaderResourcePolicy::try_new(
            Arc::new(Admission {
                deny,
                next: AtomicUsize::new(0),
                receipts: (0..32)
                    .map(|_| Arc::new(Receipt(AtomicUsize::new(0))))
                    .collect(),
            }),
            crate::resource::ReaderResourceLimits {
                allocations: 32,
                collection_entries: 65536,
                string_bytes: 1 << 20,
                footer_bytes: 16 << 20,
                page_bytes: 1 << 20,
                page_values: 65536,
                output_values: 65536,
                output_bytes: 128 << 20,
                schema_depth: 32,
                codec_bytes: 1 << 20,
            },
        )
        .unwrap()
    }
    #[test]
    fn resource_writer_ipc_original_encoder_allocates_only_after_admission() {
        for count in [0, 1, 7, 16, 257] {
            let mut metadata = HashMap::with_capacity(1024);
            metadata.insert("escaped\0key".into(), "value\\\"\n".repeat(count + 1));
            let fields: Vec<_> = (0..count)
                .map(|i| {
                    Field::new(format!("field_{i}"), DataType::Utf8, true)
                        .with_metadata(metadata.clone())
                })
                .collect();
            let complex = [
                DataType::Timestamp(
                    arrow_schema::TimeUnit::Nanosecond,
                    Some("America/New_York".into()),
                ),
                DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(fields.clone().into()),
                    true,
                ))),
                DataType::RunEndEncoded(
                    Arc::new(Field::new("ends", DataType::Int32, false)),
                    Arc::new(Field::new(
                        "values",
                        DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
                        true,
                    )),
                ),
            ];
            for data_type in complex {
                let schema = Schema::new_with_metadata(
                    vec![Field::new("original", data_type, true).with_metadata(metadata.clone())],
                    metadata.clone(),
                );
                let mut expected = WriterProperties::builder().build();
                super::super::add_encoded_arrow_schema_to_metadata(&schema, &mut expected);
                let policy = policy(false);
                let _scope = policy.enter_thread();
                for replace in [false, true] {
                    let mut props = if replace {
                        expected.clone()
                    } else {
                        WriterProperties::builder().build()
                    };
                    let track = Tracking::begin();
                    let result =
                        super::super::try_add_encoded_arrow_schema_to_metadata(&schema, &mut props);
                    let (used, admitted, early) = track.finish();
                    assert!(result.is_ok(), "{result:?}");
                    assert!(
                        !early && used <= admitted,
                        "count={count} replace={replace} used={used} admitted={admitted} early={early}"
                    );
                    assert_eq!(props.key_value_metadata, expected.key_value_metadata);
                }
            }
        }
    }
    #[test]
    fn resource_writer_ipc_denial_preserves_metadata_before_allocation() {
        let schema = Schema::new(vec![Field::new("field", DataType::Int64, false)]);
        let mut props = WriterProperties::builder().build();
        let policy = policy(true);
        let _scope = policy.enter_thread();
        let track = Tracking::begin();
        let result = super::super::try_add_encoded_arrow_schema_to_metadata(&schema, &mut props);
        let (used, _, _) = track.finish();
        assert!(
            matches!(result, Err(crate::errors::ParquetError::ResourceExhausted(e)) if e.kind == "writer IPC schema encoding")
        );
        assert_eq!(used, 0);
        assert!(props.key_value_metadata.is_none());
    }
}
