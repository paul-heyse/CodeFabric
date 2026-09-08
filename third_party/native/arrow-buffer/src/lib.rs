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

//! Low-level buffer abstractions for [Apache Arrow Rust](https://docs.rs/arrow)
//!
//! # Byte Storage abstractions
//! - [`MutableBuffer`]: Raw memory buffer that can be mutated and grown
//! - [`Buffer`]: Immutable buffer that is shared across threads
//!
//! # Typed Abstractions
//!
//! There are also several wrappers over [`Buffer`] with methods for
//! easier manipulation:
//!
//! - [`BooleanBuffer`][]: Bitmasks (buffer of packed bits)
//! - [`NullBuffer`][]: Arrow null (validity) bitmaps ([`BooleanBuffer`] with extra utilities)
//! - [`ScalarBuffer<T>`][]: Typed buffer for primitive types (e.g., `i32`, `f64`)
//! - [`OffsetBuffer<O>`][]: Offsets used in variable-length types (e.g., strings, lists)
//! - [`RunEndBuffer<E>`][]: Run-ends used in run-encoded encoded data

#![doc(
    html_logo_url = "https://arrow.apache.org/img/arrow-logo_chevrons_black-txt_white-bg.svg",
    html_favicon_url = "https://arrow.apache.org/img/arrow-logo_chevrons_black-txt_transparent-bg.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub mod alloc;
pub mod buffer;
pub use buffer::*;

pub mod builder;
pub use builder::*;

mod bigint;
pub use bigint::i256;

mod bytes;

mod native;
pub use native::*;

mod util;
pub use util::*;

mod interval;
pub use interval::*;

mod arith;

#[cfg(feature = "pool")]
mod pool;
#[cfg(feature = "pool")]
pub use pool::*;

/// Size of the original shared owner allocated when a native [`Buffer`] takes
/// ownership of an existing allocation, excluding that allocation's byte
/// capacity. This includes Arrow's private byte owner and both `Arc` counters,
/// and reflects the compiled `pool` feature (including its inline mutex).
///
/// Resource-governed decoders can reserve this amount before `Buffer::from_vec`
/// or the equivalent ownership constructor. The payload bytes must be admitted
/// separately before their allocation; a later pool claim is not admission.
/// A pool's boxed `MemoryReservation` is also a separate allocation.
///
/// This matches the `repr(C, align(2))` `ArcInner<T>` geometry of the pinned Rust
/// baseline. Native allocation probes defend it against toolchain/layout drift.
/// Returning this value does not reserve capacity or attach a resource owner.
pub fn allocation_owner_size() -> usize {
    allocation_owner_layout().size()
}

/// Exact layout corresponding to [`allocation_owner_size`], including native
/// alignment, for preallocation policies that distinguish layout classes.
pub fn allocation_owner_layout() -> std::alloc::Layout {
    #[repr(C, align(2))]
    struct SharedOwner {
        strong: std::sync::atomic::AtomicUsize,
        weak: std::sync::atomic::AtomicUsize,
        bytes: bytes::Bytes,
    }
    std::alloc::Layout::new::<SharedOwner>()
}

#[cfg(test)]
mod allocation_owner_tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static TRACK: Cell<bool> = const { Cell::new(false) };
        static BYTES: Cell<usize> = const { Cell::new(0) };
        static CALLS: Cell<usize> = const { Cell::new(0) };
    }
    struct OwnerProbeAllocator;
    #[global_allocator]
    static ALLOCATOR: OwnerProbeAllocator = OwnerProbeAllocator;
    // SAFETY: all allocation/deallocation delegates unchanged layouts/pointers
    // to System. Thread-local counters never allocate, and errors during TLS
    // teardown merely disable observation. No allocator result is modified.
    unsafe impl GlobalAlloc for OwnerProbeAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = TRACK.try_with(|track| {
                if track.get() {
                    let _ = BYTES.try_with(|bytes| bytes.set(bytes.get() + layout.size()));
                    let _ = CALLS.try_with(|calls| calls.set(calls.get() + 1));
                }
            });
            // SAFETY: caller supplied a valid allocation layout.
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: this allocator returned ptr for the same layout.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[test]
    fn allocation_owner_size_matches_actual_native_allocation_and_sharing() {
        let values = vec![1_u64, 2, 3, 4];
        let payload = values.as_ptr();
        TRACK.with(|track| track.set(true));
        let buffer = Buffer::from_vec(values);
        TRACK.with(|track| track.set(false));
        assert_eq!(buffer.as_ptr(), payload.cast());
        assert_eq!(CALLS.with(Cell::get), 1);
        assert_eq!(BYTES.with(Cell::get), allocation_owner_size());
        CALLS.with(|calls| calls.set(0));
        BYTES.with(|bytes| bytes.set(0));
        TRACK.with(|track| track.set(true));
        let shared = buffer.clone().slice(8);
        TRACK.with(|track| track.set(false));
        assert_eq!(CALLS.with(Cell::get), 0);
        assert_eq!(BYTES.with(Cell::get), 0);
        assert_eq!(shared.as_ptr(), payload.cast::<u8>().wrapping_add(8));
    }
}
