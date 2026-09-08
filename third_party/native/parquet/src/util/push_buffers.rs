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

use crate::errors::ParquetError;
use crate::file::reader::{ChunkReader, Length};
use bytes::Bytes;
use std::fmt::Display;
use std::ops::Range;

/// Holds multiple non-contiguous, caller-provided buffers of file data.
///
/// This is the in-memory buffer used by the push-based Parquet decoders
/// (`ParquetPushDecoder` and `ParquetMetaDataPushDecoder`). It can be
/// constructed up front and handed to a builder so the decoder reuses bytes
/// that have already been fetched.
///
/// Features:
/// 1. Zero copy
/// 2. non contiguous ranges of bytes
///
/// # Non Coalescing
///
/// This buffer does not coalesce  (merging adjacent ranges of bytes into a
/// single range). Coalescing at this level would require copying the data but
/// the caller may already have the needed data in a single buffer which would
/// require no copying.
///
/// Thus, the implementation defers to the caller to coalesce subsequent requests
/// if desired.
#[derive(Debug, Clone)]
pub struct PushBuffers {
    /// the virtual "offset" of this buffers (added to any request)
    offset: u64,
    /// The total length of the file being decoded
    file_len: u64,
    /// The ranges of data that are available for decoding (not adjusted for offset)
    ranges: Vec<Range<u64>>,
    /// The buffers of data that can be used to decode the Parquet file
    buffers: Vec<Bytes>,
    resource_policy: Option<crate::resource::ReaderResourcePolicy>,
}

impl Default for PushBuffers {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Display for PushBuffers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Buffers (offset: {}, file_len: {})",
            self.offset, self.file_len
        )?;
        writeln!(f, "Available Ranges (w/ offset):")?;
        for range in &self.ranges {
            writeln!(
                f,
                "  {}..{} ({}..{}): {} bytes",
                range.start,
                range.end,
                range.start + self.offset,
                range.end + self.offset,
                range.end - range.start
            )?;
        }

        Ok(())
    }
}

impl PushBuffers {
    /// Create a new, empty `PushBuffers` for a file of the given length.
    ///
    /// Use [`PushBuffers::default`] when the file length is unknown or
    /// irrelevant (e.g. the push decoder, which tracks ranges by absolute
    /// offset and never consults `file_len`).
    pub fn new(file_len: u64) -> Self {
        Self {
            offset: 0,
            file_len,
            ranges: Vec::new(),
            buffers: Vec::new(),
            resource_policy: crate::resource::current(),
        }
    }

    fn effective_policy(
        &self,
    ) -> Result<Option<crate::resource::ReaderResourcePolicy>, ParquetError> {
        let policy = crate::resource::current().or_else(|| self.resource_policy.clone());
        if let Some(current) = &policy {
            match &self.resource_policy {
                Some(original) if !original.same_owner(current) => {
                    return Err(crate::resource::ResourceExhausted {
                        kind: "foreign pushed buffer owner",
                        requested: 1,
                        limit: 0,
                    }
                    .into());
                }
                None if self.ranges.capacity() != 0 || self.buffers.capacity() != 0 => {
                    return Err(crate::resource::ResourceExhausted {
                        kind: "unowned pushed buffers",
                        requested: 1,
                        limit: 0,
                    }
                    .into());
                }
                _ => {}
            }
        }
        Ok(policy)
    }

    /// Adopt a policy only while no unowned descriptor allocation exists.
    pub(crate) fn try_set_resource_policy(
        &mut self,
        policy: Option<crate::resource::ReaderResourcePolicy>,
    ) -> Result<(), ParquetError> {
        let _scope = crate::resource::enter(policy);
        self.resource_policy = self.effective_policy()?;
        Ok(())
    }

    /// Fallible native descriptor admission before adding caller-owned bytes.
    /// The caller must separately own/admit the bytes before fetching them.
    pub fn try_push_ranges(
        &mut self,
        ranges: Vec<Range<u64>>,
        buffers: Vec<Bytes>,
    ) -> Result<(), ParquetError> {
        if ranges.len() != buffers.len() {
            return Err(general_err!("range and buffer counts differ"));
        }
        let policy = self.effective_policy()?;
        let _scope = crate::resource::enter(policy.clone());
        // A later reservation can fail after the first vector has grown. Keep
        // its receipts attached to self even on that partial-allocation path.
        self.resource_policy = policy;
        crate::resource::reserve_vec(&mut self.ranges, ranges.len(), "pushed range descriptors")?;
        crate::resource::reserve_vec(
            &mut self.buffers,
            buffers.len(),
            "pushed buffer descriptors",
        )?;
        for (range, buffer) in ranges.into_iter().zip(buffers) {
            self.push_range(range, buffer);
        }
        Ok(())
    }

    /// Fallible admission for one native descriptor pair, before Vec growth.
    pub fn try_push_range(&mut self, range: Range<u64>, buffer: Bytes) -> Result<(), ParquetError> {
        let policy = self.effective_policy()?;
        let _scope = crate::resource::enter(policy.clone());
        self.resource_policy = policy;
        crate::resource::reserve_vec(&mut self.ranges, 1, "pushed range descriptors")?;
        crate::resource::reserve_vec(&mut self.buffers, 1, "pushed buffer descriptors")?;
        self.push_range(range, buffer);
        Ok(())
    }

    /// Clone descriptor vectors only after admitting their complete new backing.
    fn try_clone(&self) -> Result<Self, ParquetError> {
        let policy = self.effective_policy()?;
        let _scope = crate::resource::enter(policy.clone());
        let mut ranges =
            crate::resource::vec_with_capacity(self.ranges.len(), "pushed range clone")?;
        let mut buffers =
            crate::resource::vec_with_capacity(self.buffers.len(), "pushed buffer clone")?;
        ranges.extend_from_slice(&self.ranges);
        buffers.extend_from_slice(&self.buffers);
        Ok(Self {
            offset: self.offset,
            file_len: self.file_len,
            ranges,
            buffers,
            resource_policy: policy,
        })
    }

    /// Push all the ranges and buffers
    pub fn push_ranges(&mut self, ranges: Vec<Range<u64>>, buffers: Vec<Bytes>) {
        assert_eq!(
            ranges.len(),
            buffers.len(),
            "Number of ranges must match number of buffers"
        );
        for (range, buffer) in ranges.into_iter().zip(buffers) {
            self.push_range(range, buffer);
        }
    }

    /// Push a new range and its associated buffer
    pub fn push_range(&mut self, range: Range<u64>, buffer: Bytes) {
        assert_eq!(
            (range.end - range.start) as usize,
            buffer.len(),
            "Range length must match buffer length"
        );
        self.ranges.push(range);
        self.buffers.push(buffer);
    }

    /// Returns true if the Buffers contains data for the given range
    pub(crate) fn has_range(&self, range: &Range<u64>) -> bool {
        self.ranges
            .iter()
            .any(|r| r.start <= range.start && r.end >= range.end)
    }

    fn iter(&self) -> impl Iterator<Item = (&Range<u64>, &Bytes)> {
        self.ranges.iter().zip(self.buffers.iter())
    }

    /// return the file length of the Parquet file being read
    pub(crate) fn file_len(&self) -> u64 {
        self.file_len
    }

    /// Specify a new offset
    fn with_offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    /// Return the total of all buffered ranges
    #[cfg(feature = "arrow")]
    pub(crate) fn buffered_bytes(&self) -> u64 {
        self.ranges.iter().map(|r| r.end - r.start).sum()
    }

    /// Clear any range and corresponding buffer that is exactly in the ranges_to_clear
    #[cfg(feature = "arrow")]
    pub(crate) fn clear_ranges(&mut self, ranges_to_clear: &[Range<u64>]) {
        // Retain paired entries in place, in their original order, avoiding
        // fresh vectors for every row group's cleanup.
        let ranges = &self.ranges;
        let mut index = 0;
        self.buffers.retain(|_| {
            let range = &ranges[index];
            index += 1;
            !ranges_to_clear.iter().any(|clear| clear == range)
        });
        self.ranges
            .retain(|range| !ranges_to_clear.iter().any(|clear| clear == range));
    }

    /// Clear all buffered ranges and their corresponding data
    pub(crate) fn clear_all_ranges(&mut self) {
        self.ranges.clear();
        self.buffers.clear();
    }
}

impl Length for PushBuffers {
    fn len(&self) -> u64 {
        self.file_len
    }
}

/// less efficient implementation of Read for Buffers
impl std::io::Read for PushBuffers {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Find the range that contains the start offset
        let mut found = false;
        for (range, data) in self.iter() {
            if range.start <= self.offset && range.end >= self.offset + buf.len() as u64 {
                // Found the range, figure out the starting offset in the buffer
                let start_offset = (self.offset - range.start) as usize;
                let end_offset = start_offset + buf.len();
                let slice = data.slice(start_offset..end_offset);
                buf.copy_from_slice(slice.as_ref());
                found = true;
                break;
            }
        }
        if found {
            // If we found the range, we can return the number of bytes read
            // advance our offset
            self.offset += buf.len() as u64;
            Ok(buf.len())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "No data available in Buffers",
            ))
        }
    }
}

impl ChunkReader for PushBuffers {
    type T = Self;

    fn get_read(&self, start: u64) -> Result<Self::T, ParquetError> {
        Ok(self.try_clone()?.with_offset(self.offset + start))
    }

    fn get_bytes(&self, start: u64, length: usize) -> Result<Bytes, ParquetError> {
        // find the range that contains the start offset
        for (range, data) in self.iter() {
            if range.start <= start && range.end >= start + length as u64 {
                // Found the range, figure out the starting offset in the buffer
                let start_offset = (start - range.start) as usize;
                return Ok(data.slice(start_offset..start_offset + length));
            }
        }
        // Signal that we need more data
        let requested_end = start + length as u64;
        Err(ParquetError::NeedMoreDataRange(start..requested_end))
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use crate::resource::{ResourceExhausted, tests::policy};
    use std::sync::atomic::Ordering;

    #[test]
    fn resource_pushed_descriptor_denial_precedes_vector_growth() {
        let mut buffers = PushBuffers::new(4);
        let (policy, _) = policy("pushed range descriptors");
        let _guard = policy.enter_thread();
        let error = buffers
            .try_push_range(0..4, Bytes::from_static(b"data"))
            .unwrap_err();
        assert!(matches!(
            error,
            ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "pushed range descriptors",
                ..
            })
        ));
        assert_eq!(buffers.ranges.capacity(), 0);
        assert_eq!(buffers.buffers.capacity(), 0);
    }

    #[test]
    fn resource_pushed_partial_allocation_failure_retains_receipt_until_buffer_drop() {
        let mut buffers = PushBuffers::new(4);
        let (policy, admission) = policy("pushed buffer descriptors");
        let guard = policy.enter_thread();
        let error = buffers
            .try_push_range(0..4, Bytes::from_static(b"data"))
            .unwrap_err();
        assert!(matches!(
            error,
            ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "pushed buffer descriptors",
                ..
            })
        ));
        assert!(buffers.ranges.capacity() > 0);
        assert_eq!(buffers.buffers.capacity(), 0);
        drop(guard);
        drop(policy);
        assert!(admission.live.load(Ordering::Acquire) > 0);
        drop(buffers);
        assert_eq!(admission.live.load(Ordering::Acquire), 0);
    }

    #[test]
    fn resource_pushed_native_chunk_clone_is_admitted_before_growth() {
        let (policy, _) = policy("pushed range clone");
        let _guard = policy.enter_thread();
        let mut buffers = PushBuffers::new(4);
        buffers
            .try_push_range(0..4, Bytes::from_static(b"data"))
            .unwrap();
        let error = buffers.get_read(0).unwrap_err();
        assert!(matches!(
            error,
            ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "pushed range clone",
                ..
            })
        ));
        assert_eq!(buffers.get_bytes(0, 4).unwrap(), b"data"[..]);
    }

    #[cfg(feature = "arrow")]
    #[test]
    fn resource_pushed_clear_keeps_native_capacity_and_pair_order() {
        let (policy, admission) = policy("");
        let _guard = policy.enter_thread();
        let mut buffers = PushBuffers::new(16);
        for (start, bytes) in [(0, b"zero"), (4, b"four"), (8, b"eigh"), (12, b"twel")] {
            buffers
                .try_push_range(start..start + 4, Bytes::from_static(bytes))
                .unwrap();
        }
        let range_ptr = buffers.ranges.as_ptr();
        let data_ptr = buffers.buffers.as_ptr();
        let requests = admission.requests.lock().unwrap().len();
        buffers.clear_ranges(&[4..8, 12..16]);
        assert_eq!(buffers.ranges.as_ptr(), range_ptr);
        assert_eq!(buffers.buffers.as_ptr(), data_ptr);
        assert_eq!(buffers.ranges, [0..4, 8..12]);
        assert_eq!(
            buffers.buffers,
            [Bytes::from_static(b"zero"), Bytes::from_static(b"eigh")]
        );
        assert_eq!(admission.requests.lock().unwrap().len(), requests);
    }

    #[test]
    fn resource_pushed_cleared_unowned_backing_cannot_be_relabelled() {
        let mut buffers = PushBuffers::new(4);
        buffers.push_range(0..4, Bytes::from_static(b"data"));
        buffers.clear_all_ranges();
        let (policy, _) = policy("");
        let _guard = policy.enter_thread();
        assert!(matches!(
            buffers.try_push_range(0..4, Bytes::from_static(b"data")),
            Err(ParquetError::ResourceExhausted(ResourceExhausted {
                kind: "unowned pushed buffers",
                ..
            }))
        ));
    }
}
