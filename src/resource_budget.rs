//! Application-owned hierarchical admission and allocation lifetime accounting.
//!
//! A process creates one root explicitly. Workspace and operation handles carry opaque lineage,
//! not merely an owner name. All admission is one transaction over that lineage. A reservation
//! must be retained by the operation owner until actual termination, or by an immutable allocation
//! until its final owner drops. This is accounted capacity, not an allocator or an RSS measurement.

use std::collections::BTreeMap;
use std::ops::{Deref, Range};
use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

#[cfg(feature = "daemon")]
pub(crate) mod native_cpu;

const DIMENSIONS: usize = 8;

/// Explicit finite quantities; zero is valid for an unused reservation dimension.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceAmounts {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub running_jobs: u64,
    pub queued_jobs: u64,
    pub retained_generations: u64,
    pub retained_bytes: u64,
    pub rows: u64,
    pub pages: u64,
}

impl ResourceAmounts {
    const fn values(self) -> [u64; DIMENSIONS] {
        [
            self.memory_bytes,
            self.disk_bytes,
            self.running_jobs,
            self.queued_jobs,
            self.retained_generations,
            self.retained_bytes,
            self.rows,
            self.pages,
        ]
    }

    const fn from_values(v: [u64; DIMENSIONS]) -> Self {
        Self {
            memory_bytes: v[0],
            disk_bytes: v[1],
            running_jobs: v[2],
            queued_jobs: v[3],
            retained_generations: v[4],
            retained_bytes: v[5],
            rows: v[6],
            pages: v[7],
        }
    }

    /// Intersect release-owned limits with authorized ceilings, dimension by dimension.
    #[must_use]
    pub fn intersection(self, ceiling: Self) -> Self {
        let left = self.values();
        let right = ceiling.values();
        Self::from_values(std::array::from_fn(|i| left[i].min(right[i])))
    }
}

/// Wide counters retain exact native infallible-growth debt beyond a finite u64 limit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    pub memory_bytes: u128,
    pub disk_bytes: u128,
    pub running_jobs: u128,
    pub queued_jobs: u128,
    pub retained_generations: u128,
    pub retained_bytes: u128,
    pub rows: u128,
    pub pages: u128,
}

impl ResourceUsage {
    const fn from_values(v: [u128; DIMENSIONS]) -> Self {
        Self {
            memory_bytes: v[0],
            disk_bytes: v[1],
            running_jobs: v[2],
            queued_jobs: v[3],
            retained_generations: v[4],
            retained_bytes: v[5],
            rows: v[6],
            pages: v[7],
        }
    }
}

/// Control headroom is not available to data work. There are no implicit numerical defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBudgetPolicy {
    pub limits: ResourceAmounts,
    pub control_reserve: ResourceAmounts,
}

impl ResourceBudgetPolicy {
    /// Validate finite positive limits and headroom strictly below each corresponding limit.
    ///
    /// # Errors
    /// Rejects a zero limit or headroom consuming the entire corresponding capacity.
    pub fn validate(self) -> Result<Self, ResourceBudgetError> {
        for (i, (limit, reserve)) in self
            .limits
            .values()
            .into_iter()
            .zip(self.control_reserve.values())
            .enumerate()
        {
            if limit == 0 || reserve >= limit {
                return Err(ResourceBudgetError::InvalidPolicy {
                    dimension: ResourceDimension::ALL[i],
                });
            }
        }
        Ok(self)
    }

    /// Apply authorized ceilings without silently discarding required control headroom.
    ///
    /// # Errors
    /// Rejects an intersection too small to preserve the declared headroom.
    pub fn intersect_ceiling(self, ceiling: ResourceAmounts) -> Result<Self, ResourceBudgetError> {
        Self {
            limits: self.limits.intersection(ceiling),
            ..self
        }
        .validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceDimension {
    MemoryBytes,
    DiskBytes,
    RunningJobs,
    QueuedJobs,
    RetainedGenerations,
    RetainedBytes,
    Rows,
    Pages,
}

impl ResourceDimension {
    const ALL: [Self; DIMENSIONS] = [
        Self::MemoryBytes,
        Self::DiskBytes,
        Self::RunningJobs,
        Self::QueuedJobs,
        Self::RetainedGenerations,
        Self::RetainedBytes,
        Self::Rows,
        Self::Pages,
    ];
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceScopeKind {
    Process,
    Workspace,
    Operation,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceOwner {
    pub kind: ResourceScopeKind,
    pub id: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceClass {
    Data,
    Control,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceBudgetError {
    #[error("resource owner identity must not be zero")]
    InvalidIdentity,
    #[error("invalid resource limit or control reserve for {dimension:?}")]
    InvalidPolicy { dimension: ResourceDimension },
    #[error("invalid resource owner hierarchy")]
    InvalidHierarchy,
    #[error("a live resource scope already owns this identity")]
    DuplicateOwner,
    #[error("resource scope belongs to another live owner or process root")]
    ForeignOwner,
    #[error("resource scope count exceeds the finite admitted job/retention envelope")]
    ScopeCapacity,
    #[error("resource quantity arithmetic overflow")]
    Overflow,
    #[error("{dimension:?} exhausted at {owner:?}: requested total {requested}, limit {limit}")]
    Exhausted {
        owner: ResourceOwner,
        dimension: ResourceDimension,
        requested: u128,
        limit: u64,
    },
    #[error("cannot release more than this reservation owns")]
    InvalidShrink,
    #[error("allocation backing requires {required} bytes, but only {reserved} are reserved")]
    UnchargedAllocation { required: u64, reserved: u64 },
    #[error("invalid retained slice range")]
    InvalidSlice,
    #[error("native memory transfer requires a memory-only reservation")]
    NonMemoryReservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceObservation {
    pub owner: ResourceOwner,
    pub policy: ResourceBudgetPolicy,
    pub used: ResourceUsage,
    pub data_used: ResourceUsage,
    pub peak: ResourceUsage,
    pub exceeded: ResourceUsage,
}

#[derive(Debug)]
struct NodeState {
    owner: ResourceOwner,
    parent: Option<u64>,
    policy: ResourceBudgetPolicy,
    used: [u128; DIMENSIONS],
    data_used: [u128; DIMENSIONS],
    peak: [u128; DIMENSIONS],
}

#[derive(Debug, Default)]
struct Ledger {
    next: u64,
    nodes: BTreeMap<u64, NodeState>,
    children: BTreeMap<(u64, ResourceOwner), u64>,
    child_counts: BTreeMap<u64, usize>,
}

#[derive(Debug)]
struct Scope {
    ledger: Arc<Mutex<Ledger>>,
    token: u64,
    owner: ResourceOwner,
    policy: ResourceBudgetPolicy,
    // Child and reservation lifetime retain their ancestors without permanent registry cycles.
    _parent: Option<ResourceBudget>,
    // A workspace may pre-admit its complete bounded index capacity at its process parent.
    // The parent-owned reservation cannot cycle back to this scope and survives every child.
    bookkeeping: Mutex<Option<ResourceReservation>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        let mut ledger = lock(&self.ledger);
        if let Some(node) = ledger.nodes.remove(&self.token)
            && let Some(parent) = node.parent
        {
            ledger.children.remove(&(parent, self.owner));
            if let Some(count) = ledger.child_counts.get_mut(&parent) {
                *count -= 1;
                if *count == 0 {
                    ledger.child_counts.remove(&parent);
                }
            }
        }
    }
}

/// A clone shares one live ledger node; matching textual identities cannot forge its lineage.
#[derive(Clone, Debug)]
pub struct ResourceBudget {
    scope: Arc<Scope>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // No user code runs under this lock. Preserve accounting during unrelated panic unwinding.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ResourceBudget {
    /// Construct the single explicit process root.
    ///
    /// # Errors
    /// Rejects an invalid identity or policy. Callers must not mint replacement roots per epoch.
    pub fn try_process(
        id: [u8; 16],
        policy: ResourceBudgetPolicy,
    ) -> Result<Self, ResourceBudgetError> {
        let policy = policy.validate()?;
        if id == [0; 16] {
            return Err(ResourceBudgetError::InvalidIdentity);
        }
        let owner = ResourceOwner {
            kind: ResourceScopeKind::Process,
            id,
        };
        let mut ledger = Ledger {
            next: 1,
            ..Ledger::default()
        };
        ledger.nodes.insert(
            0,
            NodeState {
                owner,
                parent: None,
                policy,
                used: [0; DIMENSIONS],
                data_used: [0; DIMENSIONS],
                peak: [0; DIMENSIONS],
            },
        );
        Ok(Self {
            scope: Arc::new(Scope {
                ledger: Arc::new(Mutex::new(ledger)),
                token: 0,
                owner,
                policy,
                _parent: None,
                bookkeeping: Mutex::new(None),
            }),
        })
    }

    /// Create one workspace below a process root; duplicate live identities are rejected.
    ///
    /// # Errors
    /// Rejects invalid lineage, duplicate identity, overlarge policy, or exhausted scope capacity.
    pub fn workspace(
        &self,
        id: [u8; 16],
        policy: ResourceBudgetPolicy,
    ) -> Result<Self, ResourceBudgetError> {
        self.child(
            ResourceScopeKind::Process,
            ResourceScopeKind::Workspace,
            id,
            policy,
        )
    }

    /// Create one operation below a workspace, with ceilings intersected explicitly by its caller.
    ///
    /// # Errors
    /// Rejects invalid lineage, duplicate identity, overlarge policy, or exhausted scope capacity.
    pub fn operation(
        &self,
        id: [u8; 16],
        policy: ResourceBudgetPolicy,
    ) -> Result<Self, ResourceBudgetError> {
        self.child(
            ResourceScopeKind::Workspace,
            ResourceScopeKind::Operation,
            id,
            policy,
        )
    }

    fn child(
        &self,
        required: ResourceScopeKind,
        kind: ResourceScopeKind,
        id: [u8; 16],
        policy: ResourceBudgetPolicy,
    ) -> Result<Self, ResourceBudgetError> {
        let policy = policy.validate()?;
        if self.owner().kind != required {
            return Err(ResourceBudgetError::InvalidHierarchy);
        }
        if id == [0; 16] {
            return Err(ResourceBudgetError::InvalidIdentity);
        }
        if policy.limits.intersection(self.policy().limits) != policy.limits {
            return Err(ResourceBudgetError::InvalidHierarchy);
        }
        let owner = ResourceOwner { kind, id };
        let mut ledger = lock(&self.scope.ledger);
        if ledger.children.contains_key(&(self.scope.token, owner)) {
            return Err(ResourceBudgetError::DuplicateOwner);
        }
        // Retained provider allocations can outlive their running job. The finite page/retention
        // owner capacity is independent of simultaneous admission; counting only running/queued
        // jobs would reject a complete 10k-file capture after a few hundred finished providers.
        let max_scopes = child_scope_capacity(self.policy());
        let count = ledger
            .child_counts
            .get(&self.scope.token)
            .copied()
            .unwrap_or(0);
        if count as u128 >= max_scopes {
            return Err(ResourceBudgetError::ScopeCapacity);
        }
        let token = ledger.next;
        ledger.next = token.checked_add(1).ok_or(ResourceBudgetError::Overflow)?;
        ledger.nodes.insert(
            token,
            NodeState {
                owner,
                parent: Some(self.scope.token),
                policy,
                used: [0; DIMENSIONS],
                data_used: [0; DIMENSIONS],
                peak: [0; DIMENSIONS],
            },
        );
        ledger.children.insert((self.scope.token, owner), token);
        *ledger.child_counts.entry(self.scope.token).or_default() += 1;
        drop(ledger);
        Ok(Self {
            scope: Arc::new(Scope {
                ledger: Arc::clone(&self.scope.ledger),
                token,
                owner,
                policy,
                _parent: Some(self.clone()),
                bookkeeping: Mutex::new(None),
            }),
        })
    }

    /// Pre-admit this workspace's entire finite scope-index capacity at the process ancestor.
    /// Production calls this before any operation scope is constructed. Capacity is not reported
    /// as live allocated bytes; it remains owned until the last workspace descendant disappears.
    pub(crate) fn reserve_scope_bookkeeping(&self) -> Result<(), ResourceBudgetError> {
        if self.owner().kind != ResourceScopeKind::Workspace {
            return Err(ResourceBudgetError::InvalidHierarchy);
        }
        let mut capacity = lock(&self.scope.bookkeeping);
        if capacity.is_none() {
            let bytes = child_scope_capacity(self.policy())
                .checked_add(1)
                .and_then(|value| value.checked_mul(4096))
                .and_then(|value| u64::try_from(value).ok())
                .ok_or(ResourceBudgetError::Overflow)?;
            *capacity = Some(
                self.scope
                    ._parent
                    .as_ref()
                    .ok_or(ResourceBudgetError::InvalidHierarchy)?
                    .try_reserve(
                        ResourceClass::Data,
                        ResourceAmounts {
                            memory_bytes: bytes,
                            ..ResourceAmounts::default()
                        },
                    )?,
            );
        }
        Ok(())
    }

    /// Observe the actual shared process ledger, including parent-owned bookkeeping capacity.
    #[must_use]
    pub fn process_observation(&self) -> ResourceObservation {
        let mut root = self;
        while let Some(parent) = root.scope._parent.as_ref() {
            root = parent;
        }
        root.observation()
    }

    #[must_use]
    pub fn owner(&self) -> ResourceOwner {
        self.scope.owner
    }

    #[must_use]
    pub fn policy(&self) -> ResourceBudgetPolicy {
        self.scope.policy
    }

    #[must_use]
    pub fn same_root(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.scope.ledger, &other.scope.ledger)
    }

    /// Exact live scope equality, not owner-ID equality.
    #[must_use]
    pub fn same_scope(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.scope, &other.scope)
    }

    /// Resolve an owner from this live lineage, including this scope. Matching names from a
    /// separately constructed root cannot manufacture an ancestor relationship.
    #[must_use]
    pub fn ancestor_owner(&self, kind: ResourceScopeKind) -> Option<ResourceOwner> {
        let ledger = lock(&self.scope.ledger);
        let mut cursor = Some(self.scope.token);
        while let Some(token) = cursor {
            let node = &ledger.nodes[&token];
            if node.owner.kind == kind {
                return Some(node.owner);
            }
            cursor = node.parent;
        }
        None
    }

    /// Includes the same scope. IDs from separately constructed roots are never equivalent.
    #[must_use]
    pub fn is_descendant_of(&self, other: &Self) -> bool {
        if !self.same_root(other) {
            return false;
        }
        let ledger = lock(&self.scope.ledger);
        let mut cursor = Some(self.scope.token);
        while let Some(token) = cursor {
            if token == other.scope.token {
                return true;
            }
            cursor = ledger.nodes.get(&token).and_then(|n| n.parent);
        }
        false
    }

    /// Validate that a reservation is owned by this exact scope or one of its descendants.
    ///
    /// # Errors
    /// Rejects foreign roots, siblings, and ancestor-owned reservations.
    pub fn validate_reservation(
        &self,
        reservation: &ResourceReservation,
    ) -> Result<(), ResourceBudgetError> {
        if reservation.owner.is_descendant_of(self) {
            Ok(())
        } else {
            Err(ResourceBudgetError::ForeignOwner)
        }
    }

    /// Reserve atomically at every ancestor before work or allocation begins.
    ///
    /// # Errors
    /// Returns the first limiting ancestor/dimension; failure changes no counter.
    pub fn try_reserve(
        &self,
        class: ResourceClass,
        amounts: ResourceAmounts,
    ) -> Result<ResourceReservation, ResourceBudgetError> {
        self.add(class, amounts.values().map(u128::from), true)?;
        Ok(ResourceReservation {
            owner: self.clone(),
            class,
            amounts,
        })
    }

    fn add(
        &self,
        class: ResourceClass,
        amounts: [u128; DIMENSIONS],
        checked: bool,
    ) -> Result<(), ResourceBudgetError> {
        let mut ledger = lock(&self.scope.ledger);
        let mut cursor = Some(self.scope.token);
        while let Some(token) = cursor {
            let node = &ledger.nodes[&token];
            for (i, amount) in amounts.into_iter().enumerate() {
                let total = node.used[i]
                    .checked_add(amount)
                    .ok_or(ResourceBudgetError::Overflow)?;
                if checked {
                    let limit = node.policy.limits.values()[i];
                    if total > u128::from(limit) {
                        return Err(ResourceBudgetError::Exhausted {
                            owner: node.owner,
                            dimension: ResourceDimension::ALL[i],
                            requested: total,
                            limit,
                        });
                    }
                    if class == ResourceClass::Data {
                        let limit = limit - node.policy.control_reserve.values()[i];
                        let requested = node.data_used[i] + amount;
                        if requested > u128::from(limit) {
                            return Err(ResourceBudgetError::Exhausted {
                                owner: node.owner,
                                dimension: ResourceDimension::ALL[i],
                                requested,
                                limit,
                            });
                        }
                    }
                }
            }
            cursor = node.parent;
        }
        cursor = Some(self.scope.token);
        while let Some(token) = cursor {
            let node = ledger
                .nodes
                .get_mut(&token)
                .expect("live scope retains lineage");
            for (i, amount) in amounts.into_iter().enumerate() {
                node.used[i] += amount;
                node.peak[i] = node.peak[i].max(node.used[i]);
                if class == ResourceClass::Data {
                    node.data_used[i] += amount;
                }
            }
            cursor = node.parent;
        }
        Ok(())
    }

    fn subtract(&self, class: ResourceClass, amounts: [u128; DIMENSIONS]) {
        let mut ledger = lock(&self.scope.ledger);
        let mut cursor = Some(self.scope.token);
        while let Some(token) = cursor {
            let node = ledger
                .nodes
                .get_mut(&token)
                .expect("live reservation retains lineage");
            for (i, amount) in amounts.into_iter().enumerate() {
                node.used[i] -= amount;
                if class == ResourceClass::Data {
                    node.data_used[i] -= amount;
                }
            }
            cursor = node.parent;
        }
    }

    #[must_use]
    pub fn observation(&self) -> ResourceObservation {
        let ledger = lock(&self.scope.ledger);
        let node = &ledger.nodes[&self.scope.token];
        ResourceObservation {
            owner: node.owner,
            policy: node.policy,
            used: ResourceUsage::from_values(node.used),
            data_used: ResourceUsage::from_values(node.data_used),
            peak: ResourceUsage::from_values(node.peak),
            exceeded: ResourceUsage::from_values(std::array::from_fn(|i| {
                node.used[i].saturating_sub(u128::from(node.policy.limits.values()[i]))
            })),
        }
    }

    /// Accounting bridge only: native APIs may require infallible growth, unlike application admission.
    pub(crate) fn native_memory_account(&self, class: ResourceClass) -> NativeMemoryAccount {
        NativeMemoryAccount {
            owner: self.clone(),
            class,
            bytes: 0,
        }
    }
}

/// A linear capacity owner. Dropping it means work has actually terminated or storage is gone.
#[derive(Debug)]
pub struct ResourceReservation {
    owner: ResourceBudget,
    class: ResourceClass,
    amounts: ResourceAmounts,
}

impl ResourceReservation {
    #[must_use]
    pub fn owner(&self) -> &ResourceBudget {
        &self.owner
    }

    #[must_use]
    pub const fn amounts(&self) -> ResourceAmounts {
        self.amounts
    }

    /// Add capacity atomically before additional work.
    ///
    /// # Errors
    /// Rejects arithmetic overflow or exhaustion without changing the reservation.
    pub fn try_grow(&mut self, additional: ResourceAmounts) -> Result<(), ResourceBudgetError> {
        let current = self.amounts.values();
        let extra = additional.values();
        let mut total = [0; DIMENSIONS];
        for i in 0..DIMENSIONS {
            total[i] = current[i]
                .checked_add(extra[i])
                .ok_or(ResourceBudgetError::Overflow)?;
        }
        self.owner.add(self.class, extra.map(u128::from), true)?;
        self.amounts = ResourceAmounts::from_values(total);
        Ok(())
    }

    /// Release known-freed capacity, never more than this owner holds.
    ///
    /// # Errors
    /// Rejects over-release without changing any counter.
    pub fn shrink(&mut self, freed: ResourceAmounts) -> Result<(), ResourceBudgetError> {
        let current = self.amounts.values();
        let freed = freed.values();
        let mut next = [0; DIMENSIONS];
        for i in 0..DIMENSIONS {
            next[i] = current[i]
                .checked_sub(freed[i])
                .ok_or(ResourceBudgetError::InvalidShrink)?;
        }
        self.owner.subtract(self.class, freed.map(u128::from));
        self.amounts = ResourceAmounts::from_values(next);
        Ok(())
    }

    /// Move part of this charge to an independently owned lifetime, without new admission.
    ///
    /// # Errors
    /// Rejects splitting more than the reservation owns.
    pub fn split(&mut self, amounts: ResourceAmounts) -> Result<Self, ResourceBudgetError> {
        let current = self.amounts.values();
        let split = amounts.values();
        let mut remainder = [0; DIMENSIONS];
        for i in 0..DIMENSIONS {
            remainder[i] = current[i]
                .checked_sub(split[i])
                .ok_or(ResourceBudgetError::InvalidShrink)?;
        }
        self.amounts = ResourceAmounts::from_values(remainder);
        Ok(Self {
            owner: self.owner.clone(),
            class: self.class,
            amounts,
        })
    }

    /// Transfer pre-admitted memory into an infallible native allocation owner without subtracting
    /// or charging it again. Mixed reservations must first split their memory portion explicitly.
    ///
    /// # Errors
    /// Rejects non-memory dimensions; a rejected consumed reservation releases its original charge.
    pub(crate) fn into_native_memory_account(
        mut self,
    ) -> Result<NativeMemoryAccount, ResourceBudgetError> {
        if self.amounts.values()[1..]
            .iter()
            .any(|quantity| *quantity != 0)
        {
            return Err(ResourceBudgetError::NonMemoryReservation);
        }
        let bytes = u128::from(self.amounts.memory_bytes);
        self.amounts.memory_bytes = 0;
        Ok(NativeMemoryAccount {
            owner: self.owner.clone(),
            class: self.class,
            bytes,
        })
    }

    /// Retain a caller-measured immutable object; nested heap costs must already be in the charge.
    #[must_use]
    pub fn into_charged_value<T>(self, value: T) -> ChargedValue<T> {
        ChargedValue {
            inner: Arc::new(ChargedBacking {
                value,
                reservation: self,
            }),
        }
    }

    /// Transfer a pre-admitted vector backing and keep its full charge across every retained slice.
    /// Nested allocations inside elements are the caller's separately measured responsibility.
    ///
    /// # Errors
    /// Rejects a vector whose backing capacity exceeds the memory reservation.
    pub fn into_charged_vec<T>(
        self,
        value: Vec<T>,
    ) -> Result<ChargedSlice<T>, ResourceBudgetError> {
        let required = vector_bytes::<T>(value.capacity())?;
        if required > self.amounts.memory_bytes {
            return Err(ResourceBudgetError::UnchargedAllocation {
                required,
                reserved: self.amounts.memory_bytes,
            });
        }
        let len = value.len();
        Ok(ChargedSlice {
            backing: self.into_charged_value(value),
            range: 0..len,
        })
    }
}

impl Drop for ResourceReservation {
    fn drop(&mut self) {
        self.owner
            .subtract(self.class, self.amounts.values().map(u128::from));
    }
}

#[derive(Debug)]
struct ChargedBacking<T> {
    // Rust drops fields in declaration order: release memory before its capacity permit.
    value: T,
    reservation: ResourceReservation,
}

/// Shared ownership of a value and its capacity. This wrapper never exports a bare `Arc<T>`.
#[derive(Debug)]
pub struct ChargedValue<T> {
    inner: Arc<ChargedBacking<T>>,
}

impl<T> Clone for ChargedValue<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
impl<T: PartialEq> PartialEq for ChargedValue<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl<T: Eq> Eq for ChargedValue<T> {}
impl<T> Deref for ChargedValue<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner.value
    }
}
impl<T> ChargedValue<T> {
    /// Explicitly ungoverned fixture construction; never compiled into production.
    #[cfg(test)]
    pub(crate) fn for_test(value: T) -> Self {
        test_backing_reservation(std::mem::size_of::<T>() as u64).into_charged_value(value)
    }
    #[must_use]
    pub fn reservation(&self) -> &ResourceReservation {
        &self.inner.reservation
    }
}

/// An immutable view retains its complete backing allocation, including bytes outside the view.
#[derive(Debug)]
pub struct ChargedSlice<T> {
    backing: ChargedValue<Vec<T>>,
    range: Range<usize>,
}

impl<T> Clone for ChargedSlice<T> {
    fn clone(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            range: self.range.clone(),
        }
    }
}
impl<T> Deref for ChargedSlice<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.backing[self.range.clone()]
    }
}
impl<T> AsRef<[T]> for ChargedSlice<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<T: PartialEq> PartialEq for ChargedSlice<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<T: Eq> Eq for ChargedSlice<T> {}

impl<'a, T> IntoIterator for &'a ChargedSlice<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> ChargedSlice<T> {
    /// Finite fixture backing only; callers with nested allocations use explicit test envelopes.
    #[cfg(test)]
    pub(crate) fn for_test(value: Vec<T>) -> Self {
        test_backing_reservation(vector_bytes::<T>(value.capacity()).unwrap())
            .into_charged_vec(value)
            .unwrap()
    }
    /// Execute an allocation only after reserving its declared backing capacity. The closure must
    /// respect that capacity for peak allocation; returned capacity is independently checked.
    ///
    /// # Errors
    /// Rejects overflow, exhaustion, or a returned backing larger than the pre-admitted bound.
    pub fn try_from_fn(
        budget: &ResourceBudget,
        class: ResourceClass,
        capacity: usize,
        allocate: impl FnOnce() -> Vec<T>,
    ) -> Result<Self, ResourceBudgetError> {
        let reservation = budget.try_reserve(
            class,
            ResourceAmounts {
                memory_bytes: vector_bytes::<T>(capacity)?,
                ..ResourceAmounts::default()
            },
        )?;
        reservation.into_charged_vec(allocate())
    }

    /// Retain another view without multiplying or losing the backing charge.
    ///
    /// # Errors
    /// Rejects reversed or out-of-bounds ranges relative to this view.
    pub fn slice(&self, range: Range<usize>) -> Result<Self, ResourceBudgetError> {
        if range.start > range.end || range.end > self.len() {
            return Err(ResourceBudgetError::InvalidSlice);
        }
        Ok(Self {
            backing: self.backing.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        })
    }

    #[must_use]
    pub fn reservation(&self) -> &ResourceReservation {
        self.backing.reservation()
    }
}

fn vector_bytes<T>(capacity: usize) -> Result<u64, ResourceBudgetError> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ResourceBudgetError::Overflow)
}

fn child_scope_capacity(policy: ResourceBudgetPolicy) -> u128 {
    u128::from(policy.limits.running_jobs)
        + u128::from(policy.limits.queued_jobs)
        + u128::from(policy.limits.retained_generations)
        + u128::from(policy.limits.pages)
}

#[cfg(test)]
fn test_backing_reservation(bytes: u64) -> ResourceReservation {
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: bytes.max(1),
            disk_bytes: 1,
            running_jobs: 1,
            queued_jobs: 1,
            retained_generations: 1,
            retained_bytes: bytes.max(1),
            rows: 1,
            pages: 1,
        },
        control_reserve: ResourceAmounts::default(),
    };
    ResourceBudget::try_process([1; 16], policy)
        .unwrap()
        .workspace([2; 16], policy)
        .unwrap()
        .try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: bytes,
                ..ResourceAmounts::default()
            },
        )
        .unwrap()
}

/// DataFusion's infallible `grow` is an accounting obligation, not permission to allocate. This
/// private adapter keeps overdraft visible, so later fallible work cannot ignore existing debt.
#[derive(Debug)]
pub(crate) struct NativeMemoryAccount {
    owner: ResourceBudget,
    class: ResourceClass,
    bytes: u128,
}

impl NativeMemoryAccount {
    pub(crate) const fn bytes(&self) -> u128 {
        self.bytes
    }

    pub(crate) fn try_grow(&mut self, bytes: usize) -> Result<(), ResourceBudgetError> {
        let mut demand = [0; DIMENSIONS];
        demand[0] = bytes as u128;
        self.owner.add(self.class, demand, true)?;
        self.bytes += bytes as u128;
        Ok(())
    }

    pub(crate) fn grow_infallible(&mut self, bytes: usize) {
        let mut demand = [0; DIMENSIONS];
        demand[0] = bytes as u128;
        // A live address space cannot contain u128::MAX bytes. No finite budget comparison occurs.
        self.owner
            .add(self.class, demand, false)
            .expect("address-space byte accounting fits u128");
        self.bytes += bytes as u128;
    }

    pub(crate) fn shrink(&mut self, bytes: usize) {
        let mut freed = [0; DIMENSIONS];
        freed[0] = bytes as u128;
        self.owner.subtract(self.class, freed);
        self.bytes -= bytes as u128;
    }
}

impl Drop for NativeMemoryAccount {
    fn drop(&mut self) {
        let mut freed = [0; DIMENSIONS];
        freed[0] = self.bytes;
        self.owner.subtract(self.class, freed);
    }
}

// Shared fixture lives in the narrow budget module so data-fabric tests do not
// depend on the daemon serving package. It is never compiled into production.
#[cfg(test)]
pub(crate) fn test_resource_budget() -> ResourceBudget {
    ResourceBudget::try_process(
        [79; 16],
        ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 512 * 1024 * 1024,
                running_jobs: 64,
                queued_jobs: 64,
                retained_generations: 64,
                retained_bytes: 512 * 1024 * 1024,
                rows: 1_000_000,
                pages: 1_000_000,
            },
            control_reserve: ResourceAmounts {
                memory_bytes: 64 * 1024 * 1024,
                running_jobs: 8,
                ..ResourceAmounts::default()
            },
        },
    )
    .expect("finite explicit shared resource fixture budget")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    fn policy(memory: u64) -> ResourceBudgetPolicy {
        ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: memory,
                disk_bytes: 1_000,
                running_jobs: 8,
                queued_jobs: 16,
                retained_generations: 4,
                retained_bytes: 1_000,
                rows: 1_000,
                pages: 16,
            },
            control_reserve: ResourceAmounts {
                memory_bytes: 10,
                running_jobs: 1,
                queued_jobs: 1,
                ..ResourceAmounts::default()
            },
        }
    }

    #[test]
    fn rt_cpg_wp79_retained_scope_census_is_not_limited_to_running_jobs() {
        let mut policy = policy(256 * 1024 * 1024);
        policy.limits.pages = 20_000;
        let process = ResourceBudget::try_process([51; 16], policy).unwrap();
        let workspace = process.workspace([52; 16], policy).unwrap();
        workspace.reserve_scope_bookkeeping().unwrap();
        let capacity = process.observation().used.memory_bytes;
        assert!(capacity > 0);
        let mut retained = Vec::new();
        for id in 1_u128..=10_000 {
            let operation = workspace.operation(id.to_be_bytes(), policy).unwrap();
            retained.push(
                operation
                    .try_reserve(
                        ResourceClass::Data,
                        ResourceAmounts {
                            memory_bytes: 1,
                            ..ResourceAmounts::default()
                        },
                    )
                    .unwrap(),
            );
        }
        assert_eq!(workspace.observation().used.running_jobs, 0);
        assert_eq!(workspace.observation().used.memory_bytes, 10_000);
        assert_eq!(process.observation().used.memory_bytes, capacity + 10_000);
        drop(workspace);
        assert_eq!(process.observation().used.memory_bytes, capacity + 10_000);
        drop(retained);
        assert_eq!(process.observation().used.memory_bytes, 0);
        assert!(lock(&process.scope.ledger).children.is_empty());
        assert!(lock(&process.scope.ledger).child_counts.is_empty());
    }

    fn hierarchy() -> (ResourceBudget, ResourceBudget, ResourceBudget) {
        let root = ResourceBudget::try_process([1; 16], policy(100)).unwrap();
        let workspace = root.workspace([2; 16], policy(100)).unwrap();
        let operation = workspace.operation([3; 16], policy(80)).unwrap();
        (root, workspace, operation)
    }

    #[test]
    fn rt_cpg_wp79_budget_hierarchy_exhaustion_and_control_reserve_are_causal() {
        let (root, workspace, operation) = hierarchy();
        let first = operation
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 70,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert!(matches!(
            operation.try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 1,
                    ..ResourceAmounts::default()
                }
            ),
            Err(ResourceBudgetError::Exhausted {
                owner: ResourceOwner {
                    kind: ResourceScopeKind::Operation,
                    ..
                },
                ..
            })
        ));
        let second_workspace = root.workspace([4; 16], policy(100)).unwrap();
        let second = second_workspace
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 20,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert!(
            second_workspace
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: 1,
                        ..ResourceAmounts::default()
                    }
                )
                .is_err()
        );
        let control = workspace
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    memory_bytes: 10,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert_eq!(root.observation().used.memory_bytes, 100);
        assert_eq!(root.observation().data_used.memory_bytes, 90);
        drop(first);
        drop(second);
        drop(control);
        assert_eq!(root.observation().used, ResourceUsage::default());
        assert_eq!(root.observation().peak.memory_bytes, 100);
    }

    #[test]
    fn rt_cpg_wp79_budget_all_dimensions_fail_atomically_and_release_once() {
        let (root, workspace, operation) = hierarchy();
        let amounts = ResourceAmounts {
            memory_bytes: 20,
            disk_bytes: 500,
            running_jobs: 2,
            queued_jobs: 3,
            retained_generations: 2,
            retained_bytes: 400,
            rows: 50,
            pages: 5,
        };
        let mut reservation = operation.try_reserve(ResourceClass::Data, amounts).unwrap();
        let before = root.observation();
        assert!(
            reservation
                .try_grow(ResourceAmounts {
                    memory_bytes: 1,
                    disk_bytes: 501,
                    ..ResourceAmounts::default()
                })
                .is_err()
        );
        assert_eq!(root.observation(), before);
        assert!(
            reservation
                .shrink(ResourceAmounts {
                    pages: 6,
                    ..ResourceAmounts::default()
                })
                .is_err()
        );
        assert_eq!(root.observation(), before);
        let split = reservation
            .split(ResourceAmounts {
                memory_bytes: 5,
                pages: 2,
                ..ResourceAmounts::default()
            })
            .unwrap();
        assert_eq!(root.observation(), before);
        workspace.validate_reservation(&split).unwrap();
        drop(reservation);
        assert_eq!(root.observation().used.memory_bytes, 5);
        assert_eq!(root.observation().used.pages, 2);
        drop(split);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }

    #[test]
    fn rt_cpg_wp79_budget_clone_slice_retains_exactly_one_backing_charge() {
        let (root, workspace, _) = hierarchy();
        let charged =
            ChargedSlice::try_from_fn(&workspace, ResourceClass::Data, 40, || vec![7_u8; 40])
                .unwrap();
        let cloned = charged.clone();
        let slice = cloned.slice(5..6).unwrap();
        assert_eq!(&*slice, &[7]);
        assert!(cloned.slice(41..42).is_err());
        drop(charged);
        drop(cloned);
        assert_eq!(root.observation().used.memory_bytes, 40);
        assert_eq!(slice.reservation().amounts().memory_bytes, 40);
        drop(slice);
        assert_eq!(root.observation().used.memory_bytes, 0);
        let reservation = workspace
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 1,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert!(matches!(
            reservation.into_charged_vec(vec![0_u64; 2]),
            Err(ResourceBudgetError::UnchargedAllocation { .. })
        ));
        assert_eq!(root.observation().used.memory_bytes, 0);
    }

    #[test]
    fn rt_cpg_wp79_budget_foreign_owner_duplicate_and_policy_rejected() {
        let (root, workspace, operation) = hierarchy();
        let foreign_root = ResourceBudget::try_process([1; 16], policy(100)).unwrap();
        let foreign_workspace = foreign_root.workspace([2; 16], policy(100)).unwrap();
        assert!(!foreign_workspace.same_root(&workspace));
        assert!(!foreign_workspace.is_descendant_of(&root));
        assert_eq!(
            operation.ancestor_owner(ResourceScopeKind::Workspace),
            Some(workspace.owner())
        );
        assert_eq!(
            operation.ancestor_owner(ResourceScopeKind::Process),
            Some(root.owner())
        );
        assert_eq!(root.ancestor_owner(ResourceScopeKind::Workspace), None);
        let reservation = operation
            .try_reserve(ResourceClass::Data, ResourceAmounts::default())
            .unwrap();
        assert_eq!(
            foreign_workspace.validate_reservation(&reservation),
            Err(ResourceBudgetError::ForeignOwner)
        );
        assert!(matches!(
            root.workspace([2; 16], policy(99)),
            Err(ResourceBudgetError::DuplicateOwner)
        ));
        assert!(matches!(
            root.operation([3; 16], policy(100)),
            Err(ResourceBudgetError::InvalidHierarchy)
        ));
        assert!(ResourceBudget::try_process([0; 16], policy(100)).is_err());
        let mut invalid = policy(100);
        invalid.limits.pages = 0;
        assert!(invalid.validate().is_err());
        assert!(
            policy(100)
                .intersect_ceiling(ResourceAmounts {
                    memory_bytes: 10,
                    ..policy(100).limits
                })
                .is_err()
        );
        drop(operation);
        drop(workspace);
        // The live reservation retains the ancestor scope and therefore its uniqueness claim.
        assert!(matches!(
            root.workspace([2; 16], policy(100)),
            Err(ResourceBudgetError::DuplicateOwner)
        ));
        drop(reservation);
        assert!(root.workspace([2; 16], policy(100)).is_ok());
    }

    /// Packet negative oracle: labels cannot mint a second authority, declared capacity cannot
    /// hide a retained backing, and rejected large values never run their allocation closure.
    #[test]
    fn rt_cpg_wp79_faults() {
        rt_cpg_wp79_budget_foreign_owner_duplicate_and_policy_rejected();
        rt_cpg_wp79_budget_clone_slice_retains_exactly_one_backing_charge();
        rt_cpg_wp79_budget_reserves_before_closure_and_drops_value_before_charge();
        rt_cpg_wp79_budget_all_dimensions_fail_atomically_and_release_once();
    }

    #[test]
    fn rt_cpg_wp79_budget_concurrent_reserve_release_has_one_atomic_parent() {
        let (root, workspace, _) = hierarchy();
        let admitted = Arc::new(Barrier::new(9));
        let release = Arc::new(Barrier::new(9));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let workspace = workspace.clone();
                let admitted = Arc::clone(&admitted);
                let release = Arc::clone(&release);
                std::thread::spawn(move || {
                    let charge = workspace
                        .try_reserve(
                            ResourceClass::Data,
                            ResourceAmounts {
                                memory_bytes: 10,
                                ..ResourceAmounts::default()
                            },
                        )
                        .unwrap();
                    admitted.wait();
                    release.wait();
                    drop(charge);
                })
            })
            .collect();
        admitted.wait();
        assert_eq!(root.observation().used.memory_bytes, 80);
        assert!(
            workspace
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: 11,
                        ..ResourceAmounts::default()
                    }
                )
                .is_err()
        );
        release.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(root.observation().used.memory_bytes, 0);
    }

    #[test]
    fn rt_cpg_wp79_budget_reserves_before_closure_and_drops_value_before_charge() {
        struct ObserveDrop(ResourceBudget);
        impl Drop for ObserveDrop {
            fn drop(&mut self) {
                assert_eq!(self.0.observation().used.memory_bytes, 20);
            }
        }
        let (root, workspace, _) = hierarchy();
        let called = std::sync::atomic::AtomicBool::new(false);
        assert!(
            ChargedSlice::<u8>::try_from_fn(&workspace, ResourceClass::Data, 91, || {
                called.store(true, std::sync::atomic::Ordering::Relaxed);
                vec![0; 91]
            })
            .is_err()
        );
        assert!(!called.load(std::sync::atomic::Ordering::Relaxed));
        let retained = workspace
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 20,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap()
            .into_charged_value(ObserveDrop(root.clone()));
        let clone = retained.clone();
        drop(retained);
        assert_eq!(root.observation().used.memory_bytes, 20);
        drop(clone);
        assert_eq!(root.observation().used.memory_bytes, 0);
    }

    #[test]
    fn rt_cpg_wp79_budget_prepaid_native_transfer_preserves_one_charge_and_debt() {
        let (root, workspace, _) = hierarchy();
        let prepaid = workspace
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 40,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        let before = root.observation();
        let mut native = prepaid.into_native_memory_account().unwrap();
        assert_eq!(native.bytes(), 40);
        assert_eq!(root.observation(), before);
        native.try_grow(20).unwrap();
        native.grow_infallible(80);
        assert_eq!(native.bytes(), 140);
        assert_eq!(root.observation().used.memory_bytes, 140);
        assert_eq!(root.observation().exceeded.memory_bytes, 40);
        assert!(native.try_grow(1).is_err());
        native.shrink(100);
        assert_eq!(native.bytes(), 40);
        drop(native);
        assert_eq!(root.observation().used, ResourceUsage::default());

        let mixed = workspace
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 8,
                    pages: 1,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        assert!(matches!(
            mixed.into_native_memory_account(),
            Err(ResourceBudgetError::NonMemoryReservation)
        ));
        assert_eq!(root.observation().used, ResourceUsage::default());
        let control = workspace
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    memory_bytes: 10,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap()
            .into_native_memory_account()
            .unwrap();
        assert_eq!(root.observation().used.memory_bytes, 10);
        assert_eq!(root.observation().data_used.memory_bytes, 0);
        drop(control);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }
}
