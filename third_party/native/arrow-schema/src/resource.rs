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

//! Neutral lifetime roots for already-admitted native metadata allocations.
//!
//! Capturing an owner does not admit payloads, deep clones, or later mutations.
//! Applications must admit those allocations before native work. Required owner
//! scopes are thread-lifetime scopes for dedicated workers; a guard must never
//! span an await on a shared asynchronous worker pool.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

/// Allocation-free failure at an explicit retained-owner admission boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOwnerError {
    /// Static failure category.
    pub kind: &'static str,
    /// Requested bytes or slots according to the category.
    pub requested: usize,
    /// Admitted finite ceiling.
    pub limit: usize,
}
impl Display for ResourceOwnerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} requires {}, limit {}",
            self.kind, self.requested, self.limit
        )
    }
}
impl std::error::Error for ResourceOwnerError {}

/// Exact native allocation requested before constructing or growing backing.
///
/// Reallocation requests describe the complete new allocation, while the owner
/// continues retaining the original allocation's receipt. `bytes` includes
/// native capacity/alignment rounding and excludes allocations admitted by a
/// separate request. This is not a logical payload-size or RSS estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceAllocationRequest {
    /// Complete new allocation size, including native capacity rounding.
    pub bytes: usize,
    /// Required native allocation alignment.
    pub alignment: usize,
    /// Finite source-declared allocation category.
    pub kind: &'static str,
}

/// Application-owned lifetime of reservations admitted before native work.
///
/// Adoption must use a preadmitted bounded registry, retain the original owner
/// until this owner dies, and reject ownership cycles. It must not recursively
/// allocate an unbounded owner chain. No admission is implied by cloning an Arc.
pub trait RetainedResourceOwner: Debug + Send + Sync + std::any::Any {
    /// Admit an exact allocation and retain its reservation until this owner
    /// dies. Implementations must reserve before returning success and keep all
    /// old and replacement receipts simultaneously. The default rejects this
    /// capability; implementing lifetime adoption alone never authorizes work.
    fn try_reserve_allocation(
        &self,
        request: ResourceAllocationRequest,
    ) -> Result<(), ResourceOwnerError> {
        Err(ResourceOwnerError {
            kind: "native allocation admission unavailable",
            requested: request.bytes,
            limit: 0,
        })
    }
    /// Admit and retain an original input owner before rebinding its native root.
    fn try_adopt(&self, original: Arc<dyn RetainedResourceOwner>)
    -> Result<(), ResourceOwnerError>;
    /// Record a sticky failure for an infallible native constructor. Applications
    /// must check this latch before exporting its result; ignoring it is invalid.
    fn record_failure(&self, error: ResourceOwnerError);
}

thread_local! {
    static REQUIRED_OWNER: RefCell<Option<Arc<dyn RetainedResourceOwner>>> = const { RefCell::new(None) };
}

/// The required owner on the current dedicated native worker, if installed.
pub fn current_resource_owner() -> Option<Arc<dyn RetainedResourceOwner>> {
    REQUIRED_OWNER.with(|slot| slot.borrow().clone())
}

/// An owner scope bound to the calling thread. Drop restores the prior owner.
#[must_use]
pub struct ResourceOwnerThreadGuard {
    previous: Option<Arc<dyn RetainedResourceOwner>>,
    _not_send: PhantomData<Rc<()>>,
}
impl Drop for ResourceOwnerThreadGuard {
    fn drop(&mut self) {
        REQUIRED_OWNER.with(|slot| *slot.borrow_mut() = self.previous.take());
    }
}
/// Install an explicit required owner on a dedicated native worker thread.
pub fn enter_resource_owner(owner: Arc<dyn RetainedResourceOwner>) -> ResourceOwnerThreadGuard {
    let previous = REQUIRED_OWNER.with(|slot| slot.replace(Some(owner)));
    ResourceOwnerThreadGuard {
        previous,
        _not_send: PhantomData,
    }
}

/// A shared lifetime root. Semantic Eq/Ord/Hash deliberately ignore this handle.
/// Clone retains the original owner and does not admit deep-copy allocations.
#[derive(Clone)]
pub struct ResourceOwnerHandle(Option<Arc<dyn RetainedResourceOwner>>);
impl ResourceOwnerHandle {
    /// An empty handle, usable in const native builders before execution.
    pub const fn empty() -> Self {
        Self(None)
    }
    /// Capture the current required worker owner, without allocating.
    pub fn capture() -> Self {
        Self(current_resource_owner())
    }
    /// Construct a handle for an explicitly admitted application owner.
    pub fn new(owner: Arc<dyn RetainedResourceOwner>) -> Self {
        Self(Some(owner))
    }
    /// Borrow the original retained owner.
    pub fn owner(&self) -> Option<&Arc<dyn RetainedResourceOwner>> {
        self.0.as_ref()
    }
    /// Whether no owner is retained.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
    /// Test actual owner identity, independently of semantic metadata equality.
    pub fn same_owner(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }
    /// Fallibly adopt original ownership before capturing the required worker.
    /// No native owner-chain allocation occurs here. On failure `self` is unchanged.
    pub fn try_capture_current(&mut self) -> Result<(), ResourceOwnerError> {
        let Some(current) = current_resource_owner() else {
            return Ok(());
        };
        self.try_rebind(current)
    }
    /// Fallibly retain this original owner in an explicitly admitted successor.
    pub fn try_rebind(
        &mut self,
        current: Arc<dyn RetainedResourceOwner>,
    ) -> Result<(), ResourceOwnerError> {
        if let Some(original) = &self.0 {
            if Arc::ptr_eq(original, &current) {
                return Ok(());
            }
            current.try_adopt(original.clone())?;
        }
        self.0 = Some(current);
        Ok(())
    }
    /// Transfer an original handle into a newly built native value. If an owner
    /// is already attached, adoption is explicit and fallible; it is not replaced.
    pub fn try_inherit(&mut self, original: &Self) -> Result<(), ResourceOwnerError> {
        match (&self.0, &original.0) {
            (None, _) => {
                *self = original.clone();
                Ok(())
            }
            (Some(current), Some(old)) if !Arc::ptr_eq(current, old) => {
                current.try_adopt(old.clone())
            }
            _ => Ok(()),
        }
    }
    /// Capture at an infallible native boundary. A rejected adoption retains the
    /// original root and latches failure; the caller must reject the final result.
    pub fn capture_current_or_latch(&mut self) {
        if let Err(error) = self.try_capture_current() {
            if let Some(owner) = current_resource_owner() {
                owner.record_failure(error);
            }
        }
    }
}
impl Default for ResourceOwnerHandle {
    fn default() -> Self {
        Self::capture()
    }
}
impl Debug for ResourceOwnerHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResourceOwnerHandle")
    }
}
impl PartialEq for ResourceOwnerHandle {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for ResourceOwnerHandle {}
impl PartialOrd for ResourceOwnerHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ResourceOwnerHandle {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl Hash for ResourceOwnerHandle {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataType, Field, Schema, SchemaBuilder};
    use std::collections::{HashMap, hash_map::DefaultHasher};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering},
    };

    #[derive(Debug)]
    struct Owner {
        drops: Arc<AtomicUsize>,
        failed: Arc<AtomicBool>,
        adopted: Mutex<Vec<Arc<dyn RetainedResourceOwner>>>,
        reject: bool,
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            self.drops.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }
    impl RetainedResourceOwner for Owner {
        fn try_adopt(
            &self,
            original: Arc<dyn RetainedResourceOwner>,
        ) -> Result<(), ResourceOwnerError> {
            let mut owners = self.adopted.lock().unwrap();
            if self.reject || owners.len() == owners.capacity() {
                return Err(ResourceOwnerError {
                    kind: "owner slots",
                    requested: owners.len() + 1,
                    limit: owners.capacity(),
                });
            }
            owners.push(original);
            Ok(())
        }
        fn record_failure(&self, _: ResourceOwnerError) {
            self.failed.store(true, AtomicOrdering::SeqCst);
        }
    }
    fn owner(reject: bool) -> Arc<Owner> {
        Arc::new(Owner {
            drops: Arc::default(),
            failed: Arc::default(),
            adopted: Mutex::new(Vec::with_capacity(2)),
            reject,
        })
    }

    #[test]
    fn resource_field_and_empty_schema_retain_original_owner_through_clones() {
        let owner = owner(false);
        let drops = owner.drops.clone();
        let guard = enter_resource_owner(owner.clone());
        let field = Field::new("null", DataType::Null, true);
        let cloned = field.clone();
        let empty = Schema::new_with_metadata(
            Vec::<Field>::new(),
            HashMap::from([("k".into(), "v".into())]),
        );
        drop(guard);
        drop(owner);
        drop(field);
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 0);
        drop(cloned);
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 0);
        let projected = empty.project(&[]).unwrap();
        drop(empty);
        let rebuilt = SchemaBuilder::from(projected).finish();
        assert_eq!(rebuilt.metadata()["k"], "v");
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 0);
        drop(rebuilt);
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn resource_owner_does_not_change_schema_equality_hash_or_serde() {
        let plain = Schema::new(vec![Field::new("x", DataType::Null, true)]);
        let guard = enter_resource_owner(owner(false));
        let owned = Schema::new(vec![Field::new("x", DataType::Null, true)]);
        assert_eq!(plain, owned);
        let hash = |schema: &Schema| {
            let mut state = DefaultHasher::new();
            schema.hash(&mut state);
            state.finish()
        };
        assert_eq!(hash(&plain), hash(&owned));
        #[cfg(feature = "serde")]
        assert_eq!(
            serde_json::to_value(&plain).unwrap(),
            serde_json::to_value(&owned).unwrap()
        );
        drop(guard);
    }

    #[test]
    fn resource_schema_rebind_adopts_original_before_replacing_root() {
        let older = owner(false);
        let older_weak = Arc::downgrade(&older);
        let original = {
            let _scope = enter_resource_owner(older.clone());
            Schema::empty()
        };
        let next = owner(false);
        let next_handle = ResourceOwnerHandle::new(next.clone());
        let result = {
            let _scope = enter_resource_owner(next.clone());
            Schema::new(original.fields.clone())
        };
        assert!(result.fields.resource_owner().same_owner(&next_handle));
        drop(original);
        drop(older);
        drop(next_handle);
        drop(next);
        assert!(older_weak.upgrade().is_some());
        drop(result);
        assert!(older_weak.upgrade().is_none());
    }

    #[test]
    fn resource_rejected_schema_adoption_preserves_original_and_latches() {
        let older = owner(false);
        let original = {
            let _scope = enter_resource_owner(older.clone());
            Schema::empty()
        };
        let original_owner = original.fields.resource_owner().clone();
        let next = owner(true);
        let result = {
            let _scope = enter_resource_owner(next.clone());
            Schema::new(original.fields)
        };
        assert!(next.failed.load(AtomicOrdering::SeqCst));
        assert!(result.fields.resource_owner().same_owner(&original_owner));
    }

    #[test]
    fn resource_thread_scope_restores_owner_after_nested_unwind() {
        let outer = owner(false);
        let _scope = enter_resource_owner(outer.clone());
        let outer = ResourceOwnerHandle::new(outer);
        let _ = std::panic::catch_unwind(|| {
            let _inner = enter_resource_owner(owner(false));
            panic!("native test unwind");
        });
        assert!(ResourceOwnerHandle::capture().same_owner(&outer));
    }
}
