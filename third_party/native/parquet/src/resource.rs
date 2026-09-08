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

//! Opt-in fallible admission for native reader allocations.
//!
//! This is an incremental reader facility, not a general allocator or an RSS limit.
//! A policy retains every successful reservation until its last owner is dropped.
//! Growth requests charge the complete new allocation while keeping the previous
//! reservation: replacement/reallocation peaks are never charged as a size delta.

use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

/// A finite, allocation-free resource failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceExhausted {
    /// Allocation or structural bound that was denied.
    pub kind: &'static str,
    /// Requested bytes or elements, according to `kind`.
    pub requested: usize,
    /// Applicable finite ceiling.
    pub limit: usize,
}
impl Display for ResourceExhausted {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} requires {}, limit {}",
            self.kind, self.requested, self.limit
        )
    }
}
impl std::error::Error for ResourceExhausted {}

/// A reservation released by its concrete owner's destructor.
pub trait ResourceReceipt: Debug + Send + Sync {
    /// Number of bytes admitted by this reservation.
    fn bytes(&self) -> usize;
}

/// One allocation's complete new capacity, including reallocation peaks.
#[derive(Clone, Copy, Debug)]
pub struct ResourceRequest {
    /// Required new allocation capacity, not its delta from an existing allocation.
    pub bytes: usize,
    /// Static diagnostic category.
    pub kind: &'static str,
}

/// Application-owned shared budget. Returning `Err` must not consume capacity.
pub trait ResourceAdmission: Debug + Send + Sync {
    /// Admit before allocating. A successful receipt must cover the requested bytes.
    fn try_reserve(
        &self,
        request: ResourceRequest,
    ) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted>;
}

/// Explicit finite reader geometry, independently of shared byte admission.
#[derive(Clone, Copy, Debug)]
pub struct ReaderResourceLimits {
    /// Maximum reservations retained by one reader scope.
    pub allocations: usize,
    /// Maximum entries in a decoded metadata collection.
    pub collection_entries: usize,
    /// Maximum bytes in a metadata string or binary value.
    pub string_bytes: usize,
    /// Maximum encoded footer bytes.
    pub footer_bytes: usize,
    /// Maximum encoded or decoded page bytes.
    pub page_bytes: usize,
    /// Maximum values declared by one data or dictionary page.
    pub page_values: usize,
    /// Maximum logical entries in one decoded output buffer.
    pub output_values: usize,
    /// Maximum bytes in one decoded output or scratch buffer.
    pub output_bytes: usize,
    /// Maximum nested schema depth before native recursive construction.
    pub schema_depth: usize,
    /// Maximum bytes in one fixed native decompression workspace.
    pub codec_bytes: usize,
}

struct PolicyOwner {
    admission: Arc<dyn ResourceAdmission>,
    limits: ReaderResourceLimits,
    receipts: Mutex<Vec<Arc<dyn ResourceReceipt>>>,
    _registry_receipt: Arc<dyn ResourceReceipt>,
    io_failure: Mutex<Option<ResourceExhausted>>,
}

/// Shared lifetime of admitted native work and retained reader backing.
#[derive(Clone)]
pub struct ReaderResourcePolicy(Arc<PolicyOwner>);
impl Debug for ReaderResourcePolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReaderResourcePolicy")
            .field("limits", &self.0.limits)
            .finish()
    }
}
impl PartialEq for ReaderResourcePolicy {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl ReaderResourcePolicy {
    /// Admit the finite reservation registry before allocating it.
    pub fn try_new(
        admission: Arc<dyn ResourceAdmission>,
        limits: ReaderResourceLimits,
    ) -> Result<Self, ResourceExhausted> {
        for (kind, value) in [
            ("reservation slots", limits.allocations),
            ("metadata collection limit", limits.collection_entries),
            ("metadata string limit", limits.string_bytes),
            ("footer byte limit", limits.footer_bytes),
            ("page byte limit", limits.page_bytes),
            ("page value limit", limits.page_values),
            ("output value limit", limits.output_values),
            ("output byte limit", limits.output_bytes),
            ("schema depth limit", limits.schema_depth),
            ("codec workspace limit", limits.codec_bytes),
        ] {
            if value == 0 || value > isize::MAX as usize {
                return Err(ResourceExhausted {
                    kind,
                    requested: value,
                    limit: isize::MAX as usize,
                });
            }
        }
        let bytes = limits
            .allocations
            .checked_mul(std::mem::size_of::<Arc<dyn ResourceReceipt>>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PolicyOwner>()))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or(ResourceExhausted {
                kind: "reservation registry",
                requested: usize::MAX,
                limit: usize::MAX - 1,
            })?;
        Self::check(bytes, isize::MAX as usize, "reservation registry layout")?;
        let receipt = admission.try_reserve(ResourceRequest {
            bytes,
            kind: "reservation registry",
        })?;
        if receipt.bytes() < bytes {
            return Err(ResourceExhausted {
                kind: "short reservation receipt",
                requested: bytes,
                limit: receipt.bytes(),
            });
        }
        let mut receipts = Vec::new();
        receipts
            .try_reserve_exact(limits.allocations)
            .map_err(|_| ResourceExhausted {
                kind: "reservation registry allocation",
                requested: bytes,
                limit: bytes,
            })?;
        Ok(Self(Arc::new(PolicyOwner {
            admission,
            limits,
            io_failure: Mutex::new(None),
            receipts: Mutex::new(receipts),
            _registry_receipt: receipt,
        })))
    }

    /// The immutable, finite structural limits.
    pub fn limits(&self) -> ReaderResourceLimits {
        self.0.limits
    }

    /// Reserve a complete new allocation; previous receipts remain held.
    pub(crate) fn reserve(
        &self,
        bytes: usize,
        kind: &'static str,
    ) -> Result<(), ResourceExhausted> {
        if bytes == 0 {
            return Ok(());
        }
        Self::check(bytes, isize::MAX as usize, "native allocation layout")?;
        let mut receipts = self.0.receipts.lock().map_err(|_| ResourceExhausted {
            kind: "reservation registry unavailable",
            requested: 1,
            limit: 0,
        })?;
        Self::check(
            receipts.len().saturating_add(1),
            self.0.limits.allocations,
            "reservation count",
        )?;
        let receipt = self
            .0
            .admission
            .try_reserve(ResourceRequest { bytes, kind })?;
        if receipt.bytes() < bytes {
            return Err(ResourceExhausted {
                kind: "short reservation receipt",
                requested: bytes,
                limit: receipt.bytes(),
            });
        }
        receipts.push(receipt);
        Ok(())
    }

    pub(crate) fn check(
        value: usize,
        limit: usize,
        kind: &'static str,
    ) -> Result<(), ResourceExhausted> {
        if value > limit {
            Err(ResourceExhausted {
                kind,
                requested: value,
                limit,
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn collection(
        &self,
        count: usize,
        width: usize,
        kind: &'static str,
    ) -> Result<(), ResourceExhausted> {
        Self::check(
            count,
            self.0.limits.collection_entries,
            "metadata collection entries",
        )?;
        let bytes = count.checked_mul(width).ok_or(ResourceExhausted {
            kind,
            requested: usize::MAX,
            limit: usize::MAX - 1,
        })?;
        self.reserve(bytes, kind)
    }

    pub(crate) fn string(&self, bytes: usize) -> Result<(), ResourceExhausted> {
        Self::check(bytes, self.0.limits.string_bytes, "metadata string bytes")?;
        self.reserve(bytes, "metadata string backing")
    }

    pub(crate) fn admit_bytes_owner(&self) -> Result<(), ResourceExhausted> {
        // bytes 1.12.1 Owned<T> is repr(C) { AtomicUsize, T }; this mirrors that
        // exact allocation layout, not an arbitrary overhead multiplier.
        #[repr(C)]
        struct NativeOwnedLayout {
            ref_count: std::sync::atomic::AtomicUsize,
            owner: OwnedBytes,
        }
        self.reserve(
            std::mem::size_of::<NativeOwnedLayout>(),
            "page backing owner",
        )
    }

    pub(crate) fn own_bytes(&self, bytes: bytes::Bytes) -> bytes::Bytes {
        bytes::Bytes::from_owner(OwnedBytes {
            bytes,
            _policy: self.clone(),
        })
    }
}

struct OwnedBytes {
    bytes: bytes::Bytes,
    _policy: ReaderResourcePolicy,
}
impl AsRef<[u8]> for OwnedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

thread_local! {
    static REQUIRED_THREAD_POLICY: std::cell::RefCell<Option<ReaderResourcePolicy>> = const { std::cell::RefCell::new(None) };
    static CURRENT_POLICY: std::cell::RefCell<Option<ReaderResourcePolicy>> = const { std::cell::RefCell::new(None) };
}

/// Required policy for one dedicated native execution thread. This guard is
/// deliberately !Send: install and drop it on the same worker/blocking thread.
/// It must cover that thread's complete owned operation lifetime. Caller options
/// (including None) cannot disable it. Do not install it on a shared async pool.
pub struct ReaderThreadGuard {
    previous: Option<ReaderResourcePolicy>,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for ReaderThreadGuard {
    fn drop(&mut self) {
        REQUIRED_THREAD_POLICY.with(|p| *p.borrow_mut() = self.previous.take());
    }
}
impl ReaderResourcePolicy {
    /// Clone the effective current owner for a native retained EngineData/container.
    pub fn current() -> Option<Self> {
        current()
    }

    /// Whether two handles refer to the same admitted allocation scope.
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Require this policy until the returned dedicated-thread guard is dropped.
    pub fn enter_thread(&self) -> ReaderThreadGuard {
        let previous = REQUIRED_THREAD_POLICY.with(|p| p.replace(Some(self.clone())));
        ReaderThreadGuard {
            previous,
            _not_send: std::marker::PhantomData,
        }
    }
}
/// Required lane ownership wins over optional caller configuration.
pub(crate) fn effective(explicit: Option<ReaderResourcePolicy>) -> Option<ReaderResourcePolicy> {
    REQUIRED_THREAD_POLICY
        .with(|p| p.borrow().clone())
        .or(explicit)
        .or_else(|| CURRENT_POLICY.with(|p| p.borrow().clone()))
}

/// Reject a previously built decoder instead of relabeling allocations made
/// outside this dedicated operation. Fresh decoders inherit the required owner.
pub(crate) fn validate_thread_owner(
    owner: Option<&ReaderResourcePolicy>,
) -> crate::errors::Result<()> {
    let required = REQUIRED_THREAD_POLICY.with(|p| p.borrow().clone());
    if let Some(required) = required {
        if !owner.is_some_and(|owner| Arc::ptr_eq(&required.0, &owner.0)) {
            return Err(ResourceExhausted {
                kind: "reader belongs to another resource scope",
                requested: 1,
                limit: 0,
            }
            .into());
        }
    }
    Ok(())
}
#[cfg(feature = "arrow")]
pub(crate) fn require_owned_metadata(
    owner: Option<&ReaderResourcePolicy>,
) -> crate::errors::Result<()> {
    if REQUIRED_THREAD_POLICY.with(|p| p.borrow().is_some()) && owner.is_none() {
        return Err(ResourceExhausted {
            kind: "metadata has no resource owner",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    Ok(())
}

/// A synchronous decode scope. This guard is deliberately not Send and must
/// never be retained across an await. Nesting restores the prior owner.
pub(crate) struct DecodeScope {
    previous: Option<ReaderResourcePolicy>,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for DecodeScope {
    fn drop(&mut self) {
        CURRENT_POLICY.with(|p| *p.borrow_mut() = self.previous.take());
    }
}
pub(crate) fn enter(policy: Option<ReaderResourcePolicy>) -> DecodeScope {
    let previous = CURRENT_POLICY.with(|p| p.replace(policy));
    DecodeScope {
        previous,
        _not_send: std::marker::PhantomData,
    }
}
pub(crate) fn current() -> Option<ReaderResourcePolicy> {
    effective(None)
}

/// Admit an exact new Vec capacity, retaining all old allocation receipts. The
/// guarded path uses reserve_exact so hidden geometric growth is not assumed.
pub(crate) fn reserve_vec<T>(
    buffer: &mut Vec<T>,
    additional: usize,
    kind: &'static str,
) -> crate::errors::Result<()> {
    let Some(policy) = current() else {
        buffer.reserve(additional);
        return Ok(());
    };
    let count = buffer
        .len()
        .checked_add(additional)
        .ok_or(ResourceExhausted {
            kind,
            requested: usize::MAX,
            limit: policy.limits().output_values,
        })?;
    ReaderResourcePolicy::check(
        count,
        policy.limits().output_values,
        "decoded buffer entries",
    )?;
    if count <= buffer.capacity() {
        return Ok(());
    }
    let width = std::mem::size_of::<T>();
    let maximum_capacity = if width == 0 {
        policy.limits().output_values
    } else {
        policy
            .limits()
            .output_values
            .min(policy.limits().output_bytes / width)
    };
    let capacity = count
        .max(buffer.capacity().saturating_mul(2))
        .min(maximum_capacity);
    let bytes = count.checked_mul(width).ok_or(ResourceExhausted {
        kind,
        requested: usize::MAX,
        limit: policy.limits().output_bytes,
    })?;
    ReaderResourcePolicy::check(bytes, policy.limits().output_bytes, "decoded buffer bytes")?;
    let bytes = capacity.checked_mul(width).ok_or(ResourceExhausted {
        kind,
        requested: usize::MAX,
        limit: policy.limits().output_bytes,
    })?;
    policy.reserve(bytes, kind)?;
    buffer
        .try_reserve_exact(capacity - buffer.len())
        .map_err(|_| ResourceExhausted {
            kind: "native allocation failed",
            requested: bytes,
            limit: bytes,
        })?;
    Ok(())
}

/// Append to a fresh native writer assembly buffer after fallible capacity admission.
pub(crate) fn extend_writer_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) -> crate::errors::Result<()> {
    reserve_vec(buffer, bytes.len(), "writer page assembly")?;
    buffer.extend_from_slice(bytes);
    Ok(())
}

/// Freeze an already admitted fresh writer Vec without first constructing an unowned
/// Bytes shared header. The original Vec and every escaped slice retain the same bank.
pub(crate) fn own_writer_vec(bytes: Vec<u8>) -> crate::errors::Result<bytes::Bytes> {
    let Some(policy) = current() else {
        return Ok(bytes::Bytes::from(bytes));
    };
    #[repr(C)]
    struct NativeOwnedLayout {
        ref_count: std::sync::atomic::AtomicUsize,
        owner: OwnedWriterVec,
    }
    policy.reserve(
        std::mem::size_of::<NativeOwnedLayout>(),
        "writer page backing owner",
    )?;
    Ok(bytes::Bytes::from_owner(OwnedWriterVec {
        bytes,
        _policy: policy,
    }))
}
struct OwnedWriterVec {
    bytes: Vec<u8>,
    _policy: ReaderResourcePolicy,
}
impl AsRef<[u8]> for OwnedWriterVec {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

pub(crate) fn vec_with_capacity<T>(
    capacity: usize,
    kind: &'static str,
) -> crate::errors::Result<Vec<T>> {
    let mut buffer = Vec::new();
    reserve_vec(&mut buffer, capacity, kind)?;
    Ok(buffer)
}

#[cfg(feature = "arrow")]
pub(crate) fn admit_arrow_owners(count: usize) -> crate::errors::Result<()> {
    if let Some(policy) = current() {
        let bytes = count
            .checked_mul(arrow_buffer::allocation_owner_size())
            .ok_or(ResourceExhausted {
                kind: "arrow allocation owners",
                requested: usize::MAX,
                limit: policy.limits().output_bytes,
            })?;
        policy.reserve(bytes, "arrow allocation owners")?;
    }
    Ok(())
}

#[cfg(feature = "arrow")]
pub(crate) fn reserve_bitmap(
    buffer: &mut arrow_buffer::BooleanBufferBuilder,
    additional: usize,
    kind: &'static str,
) -> crate::errors::Result<()> {
    if let Some(policy) = current() {
        // The final bitmap conversion creates one native shared buffer owner.
        // Repeated reservations are conservative when this builder spans pages.
        policy.reserve(arrow_buffer::allocation_owner_size(), "arrow bitmap owner")?;
        let bits = buffer
            .len()
            .checked_add(additional)
            .ok_or(ResourceExhausted {
                kind,
                requested: usize::MAX,
                limit: policy.limits().output_values,
            })?;
        ReaderResourcePolicy::check(
            bits,
            policy.limits().output_values,
            "decoded bitmap entries",
        )?;
        if bits > buffer.capacity() {
            // Arrow 59.2 MutableBuffer::reserve rounds to 64 bytes, then takes
            // max(rounded_required, old_capacity * 2). Reserve that entire new
            // capacity before asking BooleanBufferBuilder to grow.
            let bytes = bits
                .checked_add(7)
                .map(|n| n / 8)
                .and_then(|n| n.checked_add(63))
                .map(|n| n & !63)
                .ok_or(ResourceExhausted {
                    kind,
                    requested: usize::MAX,
                    limit: policy.limits().output_bytes,
                })?;
            let doubled = (buffer.capacity() / 8)
                .checked_mul(2)
                .ok_or(ResourceExhausted {
                    kind,
                    requested: usize::MAX,
                    limit: policy.limits().output_bytes,
                })?;
            let bytes = bytes.max(doubled);
            ReaderResourcePolicy::check(
                bytes,
                policy.limits().output_bytes,
                "decoded bitmap bytes",
            )?;
            policy.reserve(bytes, kind)?;
        }
    }
    buffer.reserve(additional);
    Ok(())
}

/// Admit a complete new native output allocation before an Arrow builder.
#[cfg(feature = "arrow")]
pub(crate) fn admit_output_allocation(
    bytes: usize,
    kind: &'static str,
) -> crate::errors::Result<()> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(bytes, policy.limits().output_bytes, kind)?;
        policy.reserve(bytes, kind)?;
    }
    Ok(())
}

/// Reserve the fixed Arc owner of a concrete native Array implementation.
#[cfg(feature = "arrow")]
pub(crate) fn admit_shared_owner<T>() -> crate::errors::Result<()> {
    let (layout, _) = std::alloc::Layout::new::<[std::sync::atomic::AtomicUsize; 2]>()
        .extend(std::alloc::Layout::new::<T>())
        .map_err(|_| ResourceExhausted {
            kind: "arrow array owner",
            requested: usize::MAX,
            limit: isize::MAX as usize,
        })?;
    admit_output_allocation(layout.pad_to_align().size(), "arrow array owner")
}

/// Source-declared hashbrown0.17 table layouts for at most `count` distinct
/// values, including every old/new table replacement. Entries are >=4 bytes.
#[cfg(feature = "arrow")]
pub(crate) fn admit_dictionary_table<T>(count: usize) -> crate::errors::Result<()> {
    let Some(policy) = current() else {
        return Ok(());
    };
    ReaderResourcePolicy::check(
        count,
        policy.limits().output_values,
        "dictionary distinct values",
    )?;
    if count == 0 {
        return Ok(());
    }
    let entry = std::alloc::Layout::new::<T>();
    assert!(entry.size() >= 4);
    let align = entry.align().max(16);
    let mut buckets = 4_usize;
    loop {
        // Group::WIDTH <=16 on the exact hashbrown supported targets. Raw
        // layout is aligned bucket bytes + one control byte per bucket + one
        // extra control group. This is independent of encoded input bytes.
        let bytes = entry
            .size()
            .checked_mul(buckets)
            .and_then(|n| n.checked_add(align - 1))
            .map(|n| n & !(align - 1))
            .and_then(|n| n.checked_add(buckets))
            .and_then(|n| n.checked_add(16))
            .ok_or(ResourceExhausted {
                kind: "dictionary hash table",
                requested: usize::MAX,
                limit: policy.limits().output_bytes,
            })?;
        admit_output_allocation(bytes, "dictionary hash table")?;
        let capacity = if buckets < 8 {
            buckets - 1
        } else {
            buckets / 8 * 7
        };
        if capacity >= count {
            return Ok(());
        }
        buckets = buckets.checked_mul(2).ok_or(ResourceExhausted {
            kind: "dictionary hash table",
            requested: usize::MAX,
            limit: policy.limits().output_bytes,
        })?;
    }
}

/// Before native builders configured to the already-decoded input length,
/// admit the Vec payloads and the lazily created key validity bitmap. At most
/// input.len distinct values are inserted; value validity stays all-valid.
#[cfg(feature = "arrow")]
pub(crate) fn admit_dictionary_buffers<K>(
    count: usize,
    values_bytes: usize,
    offsets: Option<(usize, usize)>,
) -> crate::errors::Result<()> {
    let Some(policy) = current() else {
        return Ok(());
    };
    ReaderResourcePolicy::check(
        count,
        policy.limits().output_values,
        "dictionary output values",
    )?;
    let overflow = || ResourceExhausted {
        kind: "dictionary output bytes",
        requested: usize::MAX,
        limit: policy.limits().output_bytes,
    };
    admit_output_allocation(
        count
            .checked_mul(std::mem::size_of::<K>())
            .ok_or_else(overflow)?,
        "dictionary builder keys",
    )?;
    admit_output_allocation(values_bytes, "dictionary builder values")?;
    let bitmap = count
        .checked_add(7)
        .map(|bits| bits / 8)
        .and_then(|bytes| bytes.checked_add(63))
        .map(|bytes| bytes & !63)
        .ok_or_else(overflow)?;
    admit_output_allocation(bitmap, "dictionary builder validity")?;
    let mut owners = 3; // keys, value payload, and optional key validity
    if let Some((offsets_count, width)) = offsets {
        admit_output_allocation(
            offsets_count.checked_mul(width).ok_or_else(overflow)?,
            "dictionary builder offsets",
        )?;
        // GenericByteBuilder::finish resets its empty offsets Vec by pushing
        // one value: RawVec's initial four entries for i32/i64 offsets.
        admit_output_allocation(
            4_usize.checked_mul(width).ok_or_else(overflow)?,
            "dictionary reset offsets",
        )?;
        owners += 1;
    }
    admit_arrow_owners(owners)?;
    // Native finish uses ArrayDataBuilder buffer Vecs (initial capacity4),
    // vec![values.into_data()], and exactly two Dictionary DataType boxes.
    admit_output_allocation(
        4 * std::mem::size_of::<arrow_buffer::Buffer>(),
        "dictionary key buffer descriptors",
    )?;
    admit_output_allocation(
        4 * std::mem::size_of::<arrow_buffer::Buffer>(),
        "dictionary value buffer descriptors",
    )?;
    admit_output_allocation(
        std::mem::size_of::<arrow_data::ArrayData>(),
        "dictionary child descriptor",
    )?;
    admit_output_allocation(
        std::mem::size_of::<arrow_schema::DataType>(),
        "dictionary key type",
    )?;
    admit_output_allocation(
        std::mem::size_of::<arrow_schema::DataType>(),
        "dictionary value type",
    )?;
    Ok(())
}

#[cfg(feature = "arrow")]
#[derive(Debug)]
struct BackingLifetime {
    _policy: ReaderResourcePolicy,
    size: usize,
}
#[cfg(feature = "arrow")]
impl arrow_buffer::MemoryReservation for BackingLifetime {
    fn size(&self) -> usize {
        self.size
    }
    fn resize(&mut self, size: usize) {
        self.size = size;
    }
}
#[cfg(feature = "arrow")]
impl ReaderResourcePolicy {
    /// Retain this policy alongside all independently owned original buffer
    /// reservations. Payloads must have been admitted before construction.
    /// Wrapper admission and new-claim admission precede mutation; either
    /// refusal preserves the original claims. The finite native allocation
    /// count also bounds the number of reservations on one original buffer.
    pub fn retain_buffer(&self, buffer: &arrow_buffer::Buffer) -> crate::errors::Result<()> {
        buffer
            .try_add_claim(
                self.limits().allocations,
                |layout| self.reserve(layout.size(), "arrow combined backing owner"),
                |size| {
                    self.reserve(
                        std::mem::size_of::<BackingLifetime>(),
                        "arrow backing owner",
                    )?;
                    Ok(Box::new(BackingLifetime {
                        _policy: self.clone(),
                        size,
                    })
                        as Box<dyn arrow_buffer::MemoryReservation>)
                },
            )
            .map_err(|error| match error {
                arrow_buffer::BufferClaimError::Capacity { requested, limit } => {
                    ResourceExhausted {
                        kind: "arrow backing claim count",
                        requested,
                        limit,
                    }
                    .into()
                }
                arrow_buffer::BufferClaimError::Admission(error) => {
                    crate::errors::ParquetError::ResourceExhausted(error)
                }
            })
    }

    /// Retain this scope in the original immutable Arrow backing. This does not
    /// admit those buffers after allocation, nor cover mutable reallocation.
    /// The EngineData/container owner must also retain this policy for schema,
    /// bufferless arrays, and clients replacing a buffer's pool claim.
    pub fn retain_record_batch(
        &self,
        batch: &arrow_array::RecordBatch,
    ) -> crate::errors::Result<()> {
        for array in batch.columns() {
            array.try_visit_buffers(&mut |buffer| {
                self.retain_buffer(buffer)
                    .map_err(arrow_schema::ArrowError::from)
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    pub(crate) struct Admission {
        pub live: Arc<AtomicUsize>,
        pub requests: Mutex<Vec<ResourceRequest>>,
        pub deny: &'static str,
    }
    #[derive(Debug)]
    struct Receipt {
        bytes: usize,
        live: Arc<AtomicUsize>,
    }
    impl ResourceReceipt for Receipt {
        fn bytes(&self) -> usize {
            self.bytes
        }
    }
    impl Drop for Receipt {
        fn drop(&mut self) {
            self.live.fetch_sub(self.bytes, Ordering::AcqRel);
        }
    }
    impl ResourceAdmission for Admission {
        fn try_reserve(
            &self,
            request: ResourceRequest,
        ) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
            self.requests.lock().unwrap().push(request);
            if request.kind == self.deny {
                return Err(ResourceExhausted {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
            }
            self.live.fetch_add(request.bytes, Ordering::AcqRel);
            Ok(Arc::new(Receipt {
                bytes: request.bytes,
                live: self.live.clone(),
            }))
        }
    }
    pub(crate) fn policy(deny: &'static str) -> (ReaderResourcePolicy, Arc<Admission>) {
        let admission = Arc::new(Admission {
            live: Arc::new(AtomicUsize::new(0)),
            requests: Mutex::new(Vec::new()),
            deny,
        });
        let policy = ReaderResourcePolicy::try_new(
            admission.clone(),
            ReaderResourceLimits {
                allocations: 1024,
                collection_entries: 128,
                string_bytes: 4096,
                footer_bytes: 64 * 1024,
                page_bytes: 8192,
                page_values: 4096,
                output_values: 65536,
                output_bytes: 65536,
                schema_depth: 128,
                codec_bytes: 1024 * 1024,
            },
        )
        .unwrap();
        (policy, admission)
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_combined_buffer_keeps_independent_native_banks() {
        use arrow_buffer::{Buffer, TrackingMemoryPool};
        let original = TrackingMemoryPool::default();
        let buffer = Buffer::from_vec(vec![1u8; 17]);
        buffer.claim(&original);
        let (policy, admission) = policy("");
        policy
            .reserve(37, "independent decoded allocation")
            .unwrap();
        policy.retain_buffer(&buffer).unwrap();
        let clone = buffer.clone();
        drop(policy);
        drop(buffer);
        assert_eq!(original.allocated(), 17);
        assert!(admission.live.load(Ordering::Acquire) >= 37);
        drop(clone);
        assert_eq!(original.allocated(), 0);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_combined_buffer_denial_preserves_original_bank() {
        use arrow_buffer::{Buffer, TrackingMemoryPool};
        for kind in ["arrow combined backing owner", "arrow backing owner"] {
            let original = TrackingMemoryPool::default();
            let buffer = Buffer::from_vec(vec![1u8; 17]);
            buffer.claim(&original);
            let (policy, admission) = policy(kind);
            let error = policy.retain_buffer(&buffer).unwrap_err();
            assert!(
                matches!(error, crate::errors::ParquetError::ResourceExhausted(error) if error.kind == kind)
            );
            assert_eq!(original.allocated(), 17);
            drop(policy);
            assert_eq!(admission.live.load(Ordering::Acquire), 0);
            drop(buffer);
            assert_eq!(original.allocated(), 0);
        }
    }

    #[test]
    fn resource_reallocation_admits_complete_new_capacity_and_sharing_retains_receipts() {
        let (policy, admission) = policy("");
        policy.reserve(100, "initial buffer").unwrap();
        policy.reserve(200, "replacement buffer").unwrap();
        let baseline = admission.requests.lock().unwrap()[0].bytes;
        assert_eq!(admission.live.load(Ordering::Acquire), baseline + 300);
        let other = policy.clone();
        drop(policy);
        assert_eq!(admission.live.load(Ordering::Acquire), baseline + 300);
        drop(other);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[test]
    fn resource_native_allocation_geometry_rejects_before_approving_callback() {
        let (policy, admission) = policy("");
        let before = admission.requests.lock().unwrap().len();
        let error = policy
            .reserve(isize::MAX as usize + 1, "impossible allocation")
            .unwrap_err();
        assert_eq!(error.kind, "native allocation layout");
        assert_eq!(admission.requests.lock().unwrap().len(), before);
    }

    #[test]
    fn resource_native_offset_index_denial_does_not_retry_or_decode_missing_element() {
        let (policy, admission) = policy("offset index page locations");
        let _guard = policy.enter_thread();
        // Field one: list containing one struct. Its element bytes are absent.
        // The denial must win over EOF, and a fast-path retry must not swallow it.
        let error =
            crate::file::page_index::index_reader::decode_offset_index(&[0x19, 0x1c]).unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "offset index page locations",
                ..
            })
        ));
        assert_eq!(
            admission
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.kind == "offset index page locations")
                .count(),
            1
        );
    }

    #[test]
    fn resource_native_primitive_column_index_denies_before_value_parse() {
        let (policy, _) = policy("primitive column index minima");
        let _guard = policy.enter_thread();
        let error = crate::file::page_index::column_index::PrimitiveColumnIndex::<i32>::try_new(
            vec![false],
            crate::basic::BoundaryOrder::UNORDERED,
            None,
            None,
            None,
            vec![&[]],
            vec![&[]],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "primitive column index minima",
                ..
            })
        ));
    }

    #[test]
    fn resource_native_byte_column_index_admits_combined_backing_and_offsets() {
        let (policy, admission) = policy("");
        let _guard = policy.enter_thread();
        crate::file::page_index::column_index::ByteArrayColumnIndex::try_new(
            vec![false, false],
            crate::basic::BoundaryOrder::UNORDERED,
            None,
            None,
            None,
            vec![b"a", b"bc"],
            vec![b"def", b"ghij"],
        )
        .unwrap();
        let requests = admission.requests.lock().unwrap();
        for (kind, bytes) in [
            ("byte column index minima", 3),
            ("byte column index maxima", 7),
            (
                "byte column index minimum offsets",
                3 * std::mem::size_of::<usize>(),
            ),
            (
                "byte column index maximum offsets",
                3 * std::mem::size_of::<usize>(),
            ),
        ] {
            assert!(
                requests
                    .iter()
                    .any(|request| request.kind == kind && request.bytes == bytes)
            );
        }
    }

    #[test]
    fn resource_push_decoder_explicit_policy_admits_staging_without_thread_scope() {
        use crate::file::metadata::{ParquetMetaDataOptions, ParquetMetaDataPushDecoder};
        let (policy, _) = policy("pushed range descriptors");
        let options = Arc::new(ParquetMetaDataOptions::new().with_resource_policy(policy));
        let mut decoder = ParquetMetaDataPushDecoder::try_new(8)
            .unwrap()
            .with_metadata_options(Some(options));
        let error = decoder
            .push_range(0..8, bytes::Bytes::from_static(b"\0\0\0\0PAR1"))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "pushed range descriptors",
                ..
            })
        ));
        assert!(current().is_none());
    }

    #[test]
    fn resource_native_footer_rejects_declared_collection_before_vector_allocation() {
        // FileMetaData.version=1, followed by schema list<struct> with 1024 elements.
        // There are no element bytes: a post-decode check would produce EOF instead.
        let bytes = [0x15, 0x02, 0x19, 0xfc, 0x80, 0x08];
        let (policy, _) = policy("");
        let options =
            crate::file::metadata::ParquetMetaDataOptions::new().with_resource_policy(policy);
        let error = crate::file::metadata::ParquetMetaDataReader::decode_metadata_with_options(
            &bytes,
            Some(&options),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "thrift collection entries",
                requested: 1024,
                limit: 128
            })
        ));
    }

    #[test]
    fn resource_native_footer_budget_denial_precedes_element_decode() {
        let bytes = [0x15, 0x02, 0x19, 0x1c]; // one schema element, deliberately absent
        let (policy, _) = policy("thrift collection backing");
        let options =
            crate::file::metadata::ParquetMetaDataOptions::new().with_resource_policy(policy);
        let error = crate::file::metadata::ParquetMetaDataReader::decode_metadata_with_options(
            &bytes,
            Some(&options),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "thrift collection backing",
                ..
            })
        ));
        assert!(matches!(
            crate::file::metadata::ParquetMetaDataReader::decode_metadata(&bytes),
            Err(crate::errors::ParquetError::EOF(_))
        ));
    }
    #[cfg(feature = "arrow")]
    #[test]
    fn resource_real_record_reader_rejects_primitive_output_before_decode() {
        use crate::arrow::{
            ArrowWriter,
            arrow_reader::{ArrowReaderOptions, ParquetRecordBatchReaderBuilder},
        };
        use arrow_array::{Int32Array, RecordBatch};
        let batch = RecordBatch::try_from_iter([(
            "n",
            Arc::new(Int32Array::from(vec![1, 2, 3])) as arrow_array::ArrayRef,
        )])
        .unwrap();
        let mut file = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut file, batch.schema(), None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        let (policy, _) = policy("primitive values");
        let options = ArrowReaderOptions::new().with_resource_policy(policy);
        let mut reader = ParquetRecordBatchReaderBuilder::try_new_with_options(
            bytes::Bytes::from(file),
            options,
        )
        .unwrap()
        .build()
        .unwrap();
        let error = reader.next().unwrap().unwrap_err();
        let arrow_schema::ArrowError::ResourceOwnerError(resource) = error else {
            panic!("resource error lost its type")
        };
        assert_eq!(resource.kind, "primitive values");
        assert_eq!(resource.requested, 12);
        assert!(current().is_none());
    }

    #[test]
    fn resource_decode_scope_restores_owner_after_unwind_and_nested_owner() {
        let (outer, outer_admission) = policy("");
        let (inner, inner_admission) = policy("");
        let _outer = enter(Some(outer));
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = enter(Some(inner));
            let _buffer = vec_with_capacity::<u8>(3, "inner allocation").unwrap();
            panic!("decoder panic");
        }));
        assert!(caught.is_err());
        assert_eq!(inner_admission.live.load(Ordering::Acquire), 0);
        let _buffer = vec_with_capacity::<u8>(7, "outer allocation").unwrap();
        assert_eq!(
            outer_admission
                .requests
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .kind,
            "outer allocation"
        );
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_real_zstd_record_reader_retains_original_backing_after_drop() {
        use crate::arrow::{
            ArrowWriter,
            arrow_reader::{ArrowReaderOptions, ParquetRecordBatchReaderBuilder},
        };
        use arrow_array::{Int32Array, RecordBatch};
        let batch = RecordBatch::try_from_iter([(
            "n",
            Arc::new(Int32Array::from(vec![1, 2, 3])) as arrow_array::ArrayRef,
        )])
        .unwrap();
        let mut file = Vec::new();
        let properties = crate::file::properties::WriterProperties::builder()
            .set_compression(crate::basic::Compression::ZSTD(Default::default()))
            .build();
        let mut writer = ArrowWriter::try_new(&mut file, batch.schema(), Some(properties)).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        let (policy, admission) = policy("");
        let options = ArrowReaderOptions::new().with_resource_policy(policy);
        let mut reader = ParquetRecordBatchReaderBuilder::try_new_with_options(
            bytes::Bytes::from(file),
            options,
        )
        .unwrap()
        .build()
        .unwrap();
        let output = reader.next().unwrap().unwrap();
        assert_eq!(output, batch);
        let slice = output.slice(1, 1);
        drop(output);
        drop(reader);
        assert!(admission.live.load(Ordering::Acquire) > 0);
        assert!(
            admission
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r.kind == "zstd static workspace")
        );
        drop(slice);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_backing_owner_denial_rejects_batch_without_unwinding() {
        use arrow_array::{Int32Array, RecordBatch};
        let batch = RecordBatch::try_from_iter([(
            "n",
            Arc::new(Int32Array::from(vec![1, 2, 3])) as arrow_array::ArrayRef,
        )])
        .unwrap();
        let (policy, admission) = policy("arrow backing owner");
        let error = policy.retain_record_batch(&batch).unwrap_err();
        assert!(matches!(
            error,
            crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "arrow backing owner",
                ..
            })
        ));
        drop(policy);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }
    #[test]
    fn resource_registry_rejects_oversized_slots_before_callback_or_allocation() {
        let (ordinary, admission) = policy("");
        let mut limits = ordinary.limits();
        limits.allocations = isize::MAX as usize;
        let before = admission.requests.lock().unwrap().len();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ReaderResourcePolicy::try_new(admission.clone(), limits)
        }));
        assert!(matches!(result, Ok(Err(ResourceExhausted { .. }))));
        assert_eq!(admission.requests.lock().unwrap().len(), before);
        limits.allocations = 0;
        assert!(ReaderResourcePolicy::try_new(admission.clone(), limits).is_err());
        assert_eq!(admission.requests.lock().unwrap().len(), before);
    }

    #[test]
    fn resource_required_thread_cannot_be_bypassed_by_prebuilt_none_options() {
        let options = crate::file::metadata::ParquetMetaDataOptions::new();
        assert!(options.resource_policy().is_none());
        let (required, admission) = policy("thrift collection backing");
        let (other, other_admission) = policy("");
        let guard = required.enter_thread();
        let bytes = [0x15, 0x02, 0x19, 0x1c];
        for options in [
            options,
            crate::file::metadata::ParquetMetaDataOptions::new().with_resource_policy(other),
        ] {
            let error = crate::file::metadata::ParquetMetaDataReader::decode_metadata_with_options(
                &bytes,
                Some(&options),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                crate::errors::ParquetError::ResourceExhausted(ResourceExhausted {
                    kind: "thrift collection backing",
                    ..
                })
            ));
        }
        assert_eq!(other_admission.requests.lock().unwrap().len(), 1);
        assert_eq!(
            admission
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.kind == "thrift collection backing")
                .count(),
            2
        );
        drop(guard);
        assert!(current().is_none());
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_required_thread_rejects_prebuilt_unowned_record_reader() {
        use crate::arrow::{ArrowWriter, arrow_reader::ParquetRecordBatchReaderBuilder};
        use arrow_array::{Int32Array, RecordBatch};
        let batch = RecordBatch::try_from_iter([(
            "n",
            Arc::new(Int32Array::from(vec![1])) as arrow_array::ArrayRef,
        )])
        .unwrap();
        let mut file = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut file, batch.schema(), None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(file))
            .unwrap()
            .build()
            .unwrap();
        let (required, _) = policy("");
        let _guard = required.enter_thread();
        let error = reader.next().unwrap().unwrap_err();
        let arrow_schema::ArrowError::ResourceOwnerError(error) = error else {
            panic!("lost resource type")
        };
        assert_eq!(error.kind, "reader belongs to another resource scope");
    }
}

/// Serialize through the native Thrift implementation twice: the first pass
/// measures exact output length without allocating, the second uses a fixed
/// slice whose capacity was admitted before allocation. No header-size estimate.
pub(crate) fn writer_thrift_bytes<T: crate::parquet_thrift::WriteThrift>(value: &T) -> crate::errors::Result<bytes::Bytes> {
    use crate::parquet_thrift::ThriftCompactOutputProtocol;
    use std::io::Write;
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_add(bytes.len()).ok_or(std::io::ErrorKind::OutOfMemory)?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut count = Counter(0);
    value.write_thrift(&mut ThriftCompactOutputProtocol::new(&mut count))?;
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(count.0, policy.limits().footer_bytes, "writer thrift bytes")?;
    }
    let mut bytes = Vec::new();
    reserve_vec(&mut bytes, count.0, "writer thrift buffer")?;
    bytes.resize(count.0, 0);
    value.write_thrift(&mut ThriftCompactOutputProtocol::new(bytes.as_mut_slice()))?;
    own_writer_vec(bytes)
}

/// Immutable original native vector backing and its allocation owner. Cloning
/// shares this backing; fallible mutation preadmits an actual copy when shared.
/// No raw-Vec extraction or mutable capacity access is exposed.
pub struct OwnedVec<T>(Arc<OwnedVecInner<T>>);
struct OwnedVecInner<T> {
    values: Vec<T>,
    policy: Option<ReaderResourcePolicy>,
}
impl<T> Clone for OwnedVec<T> { fn clone(&self) -> Self { Self(self.0.clone()) } }
impl<T: Debug> Debug for OwnedVec<T> { fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.0.values.fmt(f) } }
impl<T: PartialEq> PartialEq for OwnedVec<T> { fn eq(&self, other: &Self) -> bool { self.0.values == other.0.values } }
impl<T: Eq> Eq for OwnedVec<T> {}
impl<T> std::ops::Deref for OwnedVec<T> { type Target = Vec<T>; fn deref(&self) -> &Vec<T> { &self.0.values } }
impl<'a, T> IntoIterator for &'a OwnedVec<T> { type Item = &'a T; type IntoIter = std::slice::Iter<'a, T>; fn into_iter(self) -> Self::IntoIter { self.0.values.iter() } }
impl<T> OwnedVec<T> {
    /// Retain already-admitted vector backing. This admits the exact new native
    /// Arc descriptor before allocation; it does not retroactively admit values.
    pub fn try_new(values: Vec<T>) -> crate::errors::Result<Self> {
        let policy = current();
        if let Some(policy) = &policy {
            let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
                .extend(std::alloc::Layout::new::<OwnedVecInner<T>>())
                .map_err(|_| ResourceExhausted { kind: "owned vector layout", requested: usize::MAX, limit: isize::MAX as usize })?;
            policy.reserve(layout.pad_to_align().size(), "owned vector descriptor")?;
        }
        Ok(Self(Arc::new(OwnedVecInner { values, policy })))
    }

    /// Mutably borrow values without exposing vector growth/removal. Shared
    /// backing is copied only after exact allocation admission; denial keeps
    /// both the original vector and its owner unchanged.
    pub fn try_values_mut(&mut self) -> crate::errors::Result<&mut [T]>
    where T: Copy {
        validate_thread_owner(self.0.policy.as_ref())?;
        let _scope = enter(self.0.policy.clone());
        if Arc::get_mut(&mut self.0).is_none() {
            let mut values = Vec::new();
            reserve_vec(&mut values, self.0.values.len(), "owned vector copy")?;
            values.extend(self.0.values.iter().copied());
            *self = Self::try_new(values)?;
        }
        Ok(Arc::get_mut(&mut self.0).unwrap().values.as_mut_slice())
    }
}

/// Allocation-free cursor retaining the complete original backing through its
/// last unread element. Copy elements own no independent destructor/backing.
pub(crate) struct OwnedVecCursor<T: Copy> { values: OwnedVec<T>, offset: usize }
impl<T: Copy> OwnedVec<T> {
    pub(crate) fn into_cursor(self) -> OwnedVecCursor<T> { OwnedVecCursor { values: self, offset: 0 } }
}
impl<T: Copy> OwnedVecCursor<T> {
    pub(crate) fn front(&self) -> Option<&T> { self.values.get(self.offset) }
    pub(crate) fn get(&self, index: usize) -> Option<&T> { self.offset.checked_add(index).and_then(|idx| self.values.get(idx)) }
    pub(crate) fn pop_front(&mut self) -> Option<T> { let value = self.front().copied()?; self.offset += 1; Some(value) }
}

/// An admitted native Parquet output sink. Its original Vec never escapes
/// without an owning Bytes receipt; each replacement starts empty.
#[derive(Debug)]
pub struct OwnedWriteBuffer {
    bytes: Vec<u8>,
    policy: Option<ReaderResourcePolicy>,
}
impl Default for OwnedWriteBuffer { fn default() -> Self { Self::new() } }
impl OwnedWriteBuffer {
    /// Capture the current policy before any sink backing is allocated.
    pub fn new() -> Self { Self { bytes: Vec::new(), policy: current() } }
    /// Current buffered byte length.
    pub fn len(&self) -> usize { self.bytes.len() }
    /// Whether the buffer contains no bytes.
    pub fn is_empty(&self) -> bool { self.bytes.is_empty() }
    /// Borrow the original written bytes without transferring ownership.
    pub fn as_slice(&self) -> &[u8] { &self.bytes }
    /// Freeze and take original bytes; no raw-Vec transfer is exposed.
    pub fn try_take_bytes(&mut self) -> crate::errors::Result<bytes::Bytes> {
        validate_thread_owner(self.policy.as_ref())?;
        let _scope = enter(self.policy.clone());
        // Admit the owner before removing bytes so denial leaves this sink intact.
        let Some(policy) = self.policy.clone() else { return Ok(bytes::Bytes::from(std::mem::take(&mut self.bytes))); };
        #[repr(C)] struct NativeOwnedLayout { refs: std::sync::atomic::AtomicUsize, owner: OwnedWriterVec }
        policy.reserve(std::mem::size_of::<NativeOwnedLayout>(), "writer sink backing owner")?;
        Ok(bytes::Bytes::from_owner(OwnedWriterVec { bytes: std::mem::take(&mut self.bytes), _policy: policy }))
    }
    /// Consume and freeze original sink bytes with their allocation owner.
    pub fn into_bytes(mut self) -> crate::errors::Result<bytes::Bytes> { self.try_take_bytes() }
    fn try_append(&mut self, bytes: &[u8]) -> crate::errors::Result<()> {
        validate_thread_owner(self.policy.as_ref())?;
        let _scope = enter(self.policy.clone());
        reserve_vec(&mut self.bytes, bytes.len(), "writer sink bytes")?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
impl std::io::Write for OwnedWriteBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.try_append(bytes).map(|()| bytes.len()).map_err(|error| {
            if let (Some(policy), crate::errors::ParquetError::ResourceExhausted(error)) = (current().or_else(|| self.policy.clone()), error) {
                if let Ok(mut failure) = policy.0.io_failure.lock() { failure.get_or_insert(error); }
            }
            // No boxed diagnostic is allocated on a denied allocation path.
            std::io::ErrorKind::OutOfMemory.into()
        })
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

pub(crate) fn io_resource_failure() -> Option<ResourceExhausted> {
    current()?.0.io_failure.lock().ok().and_then(|error| *error)
}

#[cfg(test)]
mod writer_backing_tests {
    use super::*;
    use super::tests::policy;
    use std::io::Write;
    use std::sync::atomic::Ordering;

    #[test]
    fn resource_writer_owned_sink_admits_before_write_and_keeps_last_slice() {
        let (policy, admission) = policy("");
        let scope = policy.enter_thread();
        let mut sink = OwnedWriteBuffer::new();
        sink.write_all(b"original bytes").unwrap();
        let bytes = sink.try_take_bytes().unwrap();
        assert!(sink.is_empty());
        let tail = bytes.slice(1..2);
        drop((sink, bytes, scope, policy));
        assert!(admission.live.load(Ordering::Acquire) > 0);
        drop(tail);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[test]
    fn resource_writer_owned_sink_refusal_is_inline_and_preserves_bytes() {
        for kind in ["writer sink bytes", "writer sink backing owner"] {
            let (policy, admission) = policy(kind);
            let scope = policy.enter_thread();
            let mut sink = OwnedWriteBuffer::new();
            if kind == "writer sink bytes" {
                let error = crate::errors::ParquetError::from(sink.write_all(b"denied").unwrap_err());
                assert!(matches!(error, crate::errors::ParquetError::ResourceExhausted(error) if error.kind == kind));
                assert!(sink.is_empty());
            } else {
                sink.write_all(b"kept").unwrap();
                assert!(matches!(sink.try_take_bytes(), Err(crate::errors::ParquetError::ResourceExhausted(error)) if error.kind == kind));
                assert_eq!(sink.as_slice(), b"kept");
            }
            drop((sink, scope, policy));
            assert_eq!(admission.live.load(Ordering::Acquire), 0);
        }
    }

    #[test]
    fn resource_writer_owned_vector_clone_and_cursor_retain_original() {
        let (policy, admission) = policy("");
        let scope = policy.enter_thread();
        let mut values = Vec::new();
        reserve_vec(&mut values, 3, "offset source vector").unwrap();
        values.extend([1i64, 2, 3]);
        let vector = OwnedVec::try_new(values).unwrap();
        let original = vector.0.values.as_ptr();
        let clone = vector.clone();
        assert_eq!(original, clone.0.values.as_ptr());
        let mut cursor = clone.into_cursor();
        assert_eq!(cursor.pop_front(), Some(1));
        drop((vector, scope, policy));
        assert!(admission.live.load(Ordering::Acquire) > 0);
        assert_eq!(cursor.pop_front(), Some(2));
        drop(cursor);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[test]
    fn resource_writer_owned_vector_copy_denial_keeps_original() {
        let (policy, admission) = policy("owned vector copy");
        let scope = policy.enter_thread();
        let mut values = Vec::new();
        reserve_vec(&mut values, 2, "offset source vector").unwrap();
        values.extend([1i64, 2]);
        let vector = OwnedVec::try_new(values).unwrap();
        let mut clone = vector.clone();
        assert!(matches!(clone.try_values_mut(), Err(crate::errors::ParquetError::ResourceExhausted(error)) if error.kind == "owned vector copy"));
        assert_eq!(vector.0.values.as_ptr(), clone.0.values.as_ptr());
        drop((vector, scope, policy));
        assert!(admission.live.load(Ordering::Acquire) > 0);
        drop(clone);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }
}

/// Admit a native collection that will be allocated by its original constructor.
/// The concrete caller supplies the inline element type; nested allocations are
/// admitted separately at their own source sites.
pub(crate) fn admit_writer_slots<T>(count: usize, kind: &'static str) -> crate::errors::Result<()> {
    let Some(policy) = current() else { return Ok(()); };
    ReaderResourcePolicy::check(count, policy.limits().output_values, kind)?;
    let bytes = count.checked_mul(std::mem::size_of::<T>()).ok_or(ResourceExhausted { kind, requested: usize::MAX, limit: isize::MAX as usize })?;
    ReaderResourcePolicy::check(bytes, policy.limits().output_bytes, kind)?;
    policy.reserve(bytes, kind)?;
    Ok(())
}

/// Original policy retained by a native payload. Equality intentionally ignores
/// accounting identity, including presence versus absence of an owner.
#[derive(Clone, Default, Debug)]
pub(crate) struct RetainedPolicy { _policy: Option<ReaderResourcePolicy> }
impl RetainedPolicy { pub(crate) fn capture() -> Self { Self { _policy: current() } } }
impl PartialEq for RetainedPolicy { fn eq(&self, _: &Self) -> bool { true } }
impl Eq for RetainedPolicy {}
