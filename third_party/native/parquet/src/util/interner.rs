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

use crate::data_type::AsBytes;
use hashbrown::HashTable;

const DEFAULT_DEDUP_CAPACITY: usize = 4096;

/// Storage trait for [`Interner`]
pub trait Storage {
    type Key: Copy;

    type Value: AsBytes + ?Sized;

    /// Gets an element by its key
    fn get(&self, idx: Self::Key) -> &Self::Value;

    /// Adds a new element, returning the key
    fn push(&mut self, value: &Self::Value) -> Self::Key;

    /// Fallible storage admission before a new unique value is inserted.
    fn try_push(&mut self, value: &Self::Value) -> crate::errors::Result<Self::Key> {
        Ok(self.push(value))
    }

    /// Return an estimate of the memory used in this storage, in bytes
    #[allow(dead_code)] // not used in parquet_derive, so is dead there
    fn estimated_memory_size(&self) -> usize;
}

/// A generic value interner supporting various different [`Storage`]
#[derive(Debug, Default)]
pub struct Interner<S: Storage> {
    state: ahash::RandomState,

    /// Used to provide a lookup from value to unique value
    dedup: HashTable<S::Key>,

    storage: S,
}

impl<S: Storage> Interner<S> {
    /// Create a new `Interner` with the provided storage
    pub fn new(storage: S) -> Self {
        Self {
            state: Default::default(),
            dedup: HashTable::with_capacity(DEFAULT_DEDUP_CAPACITY),
            storage,
        }
    }

    /// Construct the native interner without eager hash capacity under a policy.
    pub fn try_new(storage: S) -> crate::errors::Result<Self> {
        if crate::resource::current().is_none() { return Ok(Self::new(storage)); }
        Ok(Self { state: Default::default(), dedup: HashTable::new(), storage })
    }

    /// Native lookup/insertion with fallible admission before hash and storage growth.
    pub fn try_intern(&mut self, value: &S::Value) -> crate::errors::Result<S::Key> {
        if crate::resource::current().is_none() { return Ok(self.intern(value)); }
        let hash = self.state.hash_one(value.as_bytes());
        if let Some(key) = self.dedup.find(hash, |key| value.as_bytes() == self.storage.get(*key).as_bytes()) {
            return Ok(*key);
        }
        self.try_reserve(1)?;
        let key = self.storage.try_push(value)?;
        self.dedup.insert_unique(hash, key, |key| self.state.hash_one(self.storage.get(*key).as_bytes()));
        Ok(key)
    }

    fn try_reserve(&mut self, additional: usize) -> crate::errors::Result<()> {
        let Some(policy) = crate::resource::current() else { return Ok(()); };
        let count = self.dedup.len().checked_add(additional).ok_or(crate::resource::ResourceExhausted {
            kind: "writer dictionary entries", requested: usize::MAX, limit: policy.limits().output_values,
        })?;
        crate::resource::ReaderResourcePolicy::check(count, policy.limits().output_values, "writer dictionary entries")?;
        if count <= self.dedup.capacity() { return Ok(()); }
        let bytes = dictionary_table_layout::<S::Key>(count)?;
        policy.reserve(bytes, "writer dictionary hash table")?;
        self.dedup.try_reserve(additional, |key| self.state.hash_one(self.storage.get(*key).as_bytes()))
            .map_err(|_| crate::resource::ResourceExhausted { kind: "writer dictionary allocation", requested: bytes, limit: bytes })?;
        debug_assert!(self.dedup.allocation_size() <= bytes);
        Ok(())
    }

    /// Intern the value, returning the interned key, and if this was a new value
    pub fn intern(&mut self, value: &S::Value) -> S::Key {
        let hash = self.state.hash_one(value.as_bytes());

        *self
            .dedup
            .entry(
                hash,
                // Compare bytes rather than directly comparing values so NaNs can be interned
                |index| value.as_bytes() == self.storage.get(*index).as_bytes(),
                |key| self.state.hash_one(self.storage.get(*key).as_bytes()),
            )
            .or_insert_with(|| self.storage.push(value))
            .get()
    }

    /// Return estimate of the memory used, in bytes
    #[allow(dead_code)] // not used in parquet_derive, so is dead there
    pub fn estimated_memory_size(&self) -> usize {
        self.storage.estimated_memory_size() + self.dedup.allocation_size()
    }

    /// Returns the storage for this interner
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Unwraps the inner storage
    #[cfg(feature = "arrow")]
    pub fn into_inner(self) -> S {
        self.storage
    }
}

/// Source bound for pinned hashbrown 0.17.1 raw.rs capacity_to_buckets and
/// TableLayout::calculate_layout_for. Native control groups are at most 16
/// bytes (SSE2/NEON/LSX); the generic word path is no larger. This interner
/// never removes entries, so reserve uses count, without tombstone rehash.
fn dictionary_table_layout<T>(count: usize) -> crate::errors::Result<usize> {
    let width = std::mem::size_of::<T>();
    let align = std::mem::align_of::<T>().max(16);
    let result = (|| {
        let buckets = if count < 15 {
            let minimum = match width { 0..=1 => 14, 2..=3 => 7, _ => 3 };
            match count.max(minimum) { 0..=3 => 4, 4..=7 => 8, _ => 16 }
        } else { (count.checked_mul(8)? / 7).checked_next_power_of_two()? };
        let control_offset = width.checked_mul(buckets)?.checked_add(align - 1)? & !(align - 1);
        control_offset.checked_add(buckets)?.checked_add(16)
    })().ok_or(crate::resource::ResourceExhausted { kind: "writer dictionary hash layout", requested: usize::MAX, limit: isize::MAX as usize })?;
    crate::resource::ReaderResourcePolicy::check(result, isize::MAX as usize - (align - 1), "writer dictionary hash layout")?;
    Ok(result)
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    #[test]
    fn resource_writer_hash_geometry_covers_actual_native_allocations() {
        for count in [1, 3, 4, 7, 8, 14, 15, 28, 29, 4096, 4097, 16384] {
            assert!(HashTable::<u64>::with_capacity(count).allocation_size() <= dictionary_table_layout::<u64>(count).unwrap());
            assert!(HashTable::<u8>::with_capacity(count).allocation_size() <= dictionary_table_layout::<u8>(count).unwrap());
        }
        assert!(dictionary_table_layout::<u64>(usize::MAX).is_err());
    }
}
