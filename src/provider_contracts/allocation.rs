//! Pre-admitted provider allocations and Arrow's actual shared backing lifetime.
//!
//! This private boundary claims only newly produced buffers, once per production transaction.
//! Arrow 59 replaces prior claims, so reclaiming arbitrary previously published buffers is not a
//! supported ownership transfer. Ordinary batch/array/ArrayData cloning requires no claim at all.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow_array::RecordBatch;
use arrow_buffer::{Buffer, MemoryPool, MemoryReservation};

use crate::resource_budget::{
    ChargedSlice, ChargedValue, NativeMemoryAccount, ResourceAmounts, ResourceBudget,
    ResourceClass, ResourceReservation,
};

use super::{ProviderContractError, ProviderJob};

/// Declared capacity for opaque provider allocations, not measured allocator use or RSS.
/// The release byte envelope is retained by the actual native owner; profiles are calibrated
/// separately. Input/node/depth/work/deadline caps remain independently mandatory.
pub(crate) fn reserve_native_state(
    job: &ProviderJob,
) -> Result<ResourceReservation, ProviderContractError> {
    let bytes = job.ceilings().max_bytes();
    Ok(job.resource_budget().try_reserve(
        ResourceClass::Data,
        ResourceAmounts {
            memory_bytes: bytes,
            retained_bytes: bytes,
            retained_generations: 1,
            ..ResourceAmounts::default()
        },
    )?)
}

pub(crate) fn require_native_workspace(
    prior: &ResourceBudget,
    next: &ResourceBudget,
) -> Result<(), ProviderContractError> {
    use crate::resource_budget::ResourceScopeKind;
    if !prior.same_root(next)
        || prior.ancestor_owner(ResourceScopeKind::Workspace)
            != next.ancestor_owner(ResourceScopeKind::Workspace)
    {
        return Err(ProviderContractError::ResourceOwnerMismatch);
    }
    Ok(())
}

/// A finite working envelope reserved before parsing, decoding or building candidate output.
#[derive(Debug)]
pub(crate) struct ProviderAllocation {
    budget: ResourceBudget,
    remaining: Mutex<ResourceReservation>,
    exceeded_declared_envelope: AtomicBool,
    arrow_claimed: bool,
}

/// Measured backing ownership after one private claim transaction; cloning this does no admission.
#[derive(Clone, Debug)]
pub(crate) struct ProviderBufferOwnership {
    pub(crate) budget: ResourceBudget,
    pub(crate) measured_buffer_bytes: u64,
    pub(crate) unique_buffers: usize,
}

impl ProviderAllocation {
    pub(crate) fn try_new(
        budget: &ResourceBudget,
        maximum_memory_bytes: u64,
    ) -> Result<Self, ProviderContractError> {
        let remaining = budget.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: maximum_memory_bytes,
                ..ResourceAmounts::default()
            },
        )?;
        Ok(Self {
            budget: budget.clone(),
            remaining: Mutex::new(remaining),
            exceeded_declared_envelope: AtomicBool::new(false),
            arrow_claimed: false,
        })
    }

    pub(crate) fn retain_value<T>(
        &mut self,
        measured_memory_bytes: u64,
        value: T,
    ) -> Result<ChargedValue<T>, ProviderContractError> {
        Ok(self
            .take_memory(measured_memory_bytes)?
            .into_charged_value(value))
    }

    pub(crate) fn retain_vec<T>(
        &mut self,
        measured_memory_bytes: u64,
        values: Vec<T>,
    ) -> Result<ChargedSlice<T>, ProviderContractError> {
        Ok(self
            .take_memory(measured_memory_bytes)?
            .into_charged_vec(values)?)
    }

    pub(crate) fn retain_measured_vec<T>(
        &mut self,
        values: Vec<T>,
        nested_bytes: impl Fn(&T) -> usize,
    ) -> Result<ChargedSlice<T>, ProviderContractError> {
        let bytes = values.iter().try_fold(
            values
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(ProviderContractError::ResourceOverflow)?,
            |sum, value| {
                sum.checked_add(nested_bytes(value))
                    .ok_or(ProviderContractError::ResourceOverflow)
            },
        )?;
        self.retain_vec(
            u64::try_from(bytes).map_err(|_| ProviderContractError::ResourceOverflow)?,
            values,
        )
    }

    pub(crate) fn take_memory(
        &mut self,
        bytes: u64,
    ) -> Result<ResourceReservation, ProviderContractError> {
        Ok(self
            .remaining
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .split(ResourceAmounts {
                memory_bytes: bytes,
                ..ResourceAmounts::default()
            })?)
    }

    /// Claim each newly allocated backing exactly once, across every array and relation in a run.
    /// The temporary walk is explicitly bounded independently of data rows or logical slice size.
    pub(crate) fn claim_new_batches<'a>(
        &mut self,
        batches: impl IntoIterator<Item = &'a RecordBatch>,
        maximum_arrays: usize,
    ) -> Result<ProviderBufferOwnership, ProviderContractError> {
        if self.arrow_claimed || maximum_arrays == 0 {
            return Err(ProviderContractError::ResourceCeilingExceeded);
        }
        self.arrow_claimed = true;
        let mut buffers = BTreeMap::<usize, Buffer>::new();
        let mut visited = 0_usize;
        for batch in batches {
            for column in batch.columns() {
                let mut pending = vec![column.to_data()];
                while let Some(data) = pending.pop() {
                    visited = visited
                        .checked_add(1)
                        .ok_or(ProviderContractError::ResourceOverflow)?;
                    if visited > maximum_arrays
                        || pending.len().saturating_add(data.child_data().len()) > maximum_arrays
                    {
                        return Err(ProviderContractError::ResourceCeilingExceeded);
                    }
                    for buffer in data
                        .buffers()
                        .iter()
                        .chain(data.nulls().map(arrow_buffer::NullBuffer::buffer))
                    {
                        if buffer.capacity() == 0 {
                            continue;
                        }
                        // data_ptr is the allocation base; slice start and logical length are not
                        // backing identities. No unsafe access or durable pointer identity is used.
                        let key = buffer.data_ptr().as_ptr() as usize;
                        buffers.entry(key).or_insert_with(|| buffer.clone());
                        if buffers.len() > maximum_arrays.saturating_mul(4) {
                            return Err(ProviderContractError::ResourceCeilingExceeded);
                        }
                    }
                    pending.extend(data.child_data().iter().cloned());
                }
            }
        }
        let mut measured_buffer_bytes = 0_u64;
        for buffer in buffers.values() {
            measured_buffer_bytes = measured_buffer_bytes
                .checked_add(
                    u64::try_from(buffer.capacity())
                        .map_err(|_| ProviderContractError::ResourceOverflow)?,
                )
                .ok_or(ProviderContractError::ResourceOverflow)?;
            buffer.claim(self);
        }
        // Even an under-declared producer leaves an escaped raw clone accurately charged until
        // its actual backing dies. Reporting the violation must not manufacture released memory.
        if self.exceeded_declared_envelope.load(Ordering::Acquire) {
            return Err(ProviderContractError::ResourceCeilingExceeded);
        }
        Ok(ProviderBufferOwnership {
            budget: self.budget.clone(),
            measured_buffer_bytes,
            unique_buffers: buffers.len(),
        })
    }
}

impl MemoryPool for ProviderAllocation {
    fn reserve(&self, size: usize) -> Box<dyn MemoryReservation> {
        let mut remaining = self
            .remaining
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prepaid = remaining.amounts().memory_bytes.min(size as u64);
        let mut account = remaining
            .split(ResourceAmounts {
                memory_bytes: prepaid,
                ..ResourceAmounts::default()
            })
            .expect("prepaid amount is bounded by the remaining reservation")
            .into_native_memory_account()
            .expect("split is memory-only");
        if prepaid < size as u64 {
            self.exceeded_declared_envelope
                .store(true, Ordering::Release);
            account.grow_infallible(
                size - usize::try_from(prepaid).expect("prepaid is bounded by usize size"),
            );
        }
        Box::new(ProviderArrowMemory { account, size })
    }

    fn available(&self) -> isize {
        let observation = self.budget.observation();
        let data_limit = observation.policy.limits.memory_bytes
            - observation.policy.control_reserve.memory_bytes;
        let data_available = i128::from(data_limit)
            - i128::try_from(observation.data_used.memory_bytes).unwrap_or(i128::MAX);
        let total_available = i128::from(observation.policy.limits.memory_bytes)
            - i128::try_from(observation.used.memory_bytes).unwrap_or(i128::MAX);
        isize::try_from(data_available.min(total_available)).unwrap_or(isize::MIN)
    }

    fn used(&self) -> usize {
        usize::try_from(self.budget.observation().used.memory_bytes).unwrap_or(usize::MAX)
    }

    fn capacity(&self) -> usize {
        usize::try_from(self.budget.policy().limits.memory_bytes).unwrap_or(usize::MAX)
    }
}

#[derive(Debug)]
struct ProviderArrowMemory {
    account: NativeMemoryAccount,
    size: usize,
}

impl MemoryReservation for ProviderArrowMemory {
    fn size(&self) -> usize {
        self.size
    }

    fn resize(&mut self, new_size: usize) {
        if new_size > self.size {
            self.account.grow_infallible(new_size - self.size);
        } else {
            self.account.shrink(self.size - new_size);
        }
        self.size = new_size;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{Array, Int64Array};
    use arrow_schema::{DataType, Field, Schema};

    use super::*;

    fn batch() -> RecordBatch {
        let values = Arc::new(Int64Array::from(vec![1_i64, 2, 3, 4]));
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("a", DataType::Int64, false),
                Field::new("b", DataType::Int64, false),
            ])),
            vec![values.clone(), values],
        )
        .unwrap()
    }

    #[test]
    fn wp79_provider_arrow_prepaid_backing_survives_raw_array_data_and_slice_clones() {
        let budget = super::super::fixture_provider_budget([1; 16], [2; 16]);
        let mut allocation = ProviderAllocation::try_new(&budget, 1024).unwrap();
        let batch = batch();
        let data = batch.column(0).to_data();
        let slice = batch.slice(1, 1);
        let ownership = allocation.claim_new_batches([&batch, &slice], 8).unwrap();
        assert_eq!(ownership.unique_buffers, 1);
        assert_eq!(ownership.measured_buffer_bytes, 32);
        assert!(ownership.budget.same_scope(&budget));
        drop(allocation);
        assert_eq!(budget.observation().used.memory_bytes, 32);
        drop(batch);
        drop(slice);
        drop(ownership);
        assert_eq!(budget.observation().used.memory_bytes, 32);
        drop(data);
        assert_eq!(budget.observation().used.memory_bytes, 0);
        assert_eq!(
            budget.observation().peak.memory_bytes,
            1024,
            "transfer must not double-charge even transiently"
        );
    }

    #[test]
    fn wp79_provider_arrow_underdeclared_backing_is_rejected_but_stays_charged() {
        let budget = super::super::fixture_provider_budget([1; 16], [2; 16]);
        let mut allocation = ProviderAllocation::try_new(&budget, 8).unwrap();
        let batch = batch();
        let escaped = batch.column(0).slice(0, 1);
        assert_eq!(
            allocation.claim_new_batches([&batch], 8).unwrap_err(),
            ProviderContractError::ResourceCeilingExceeded
        );
        drop(allocation);
        drop(batch);
        assert_eq!(budget.observation().used.memory_bytes, 32);
        assert_eq!(budget.observation().peak.memory_bytes, 32);
        drop(escaped);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[test]
    fn wp79_provider_arrow_infallible_resize_keeps_actual_debt_until_shrink_and_drop() {
        let budget = super::super::fixture_provider_budget([1; 16], [2; 16]);
        let allocation = ProviderAllocation::try_new(&budget, 8).unwrap();
        let mut reservation = MemoryPool::reserve(&allocation, 8);
        drop(allocation);
        reservation.resize((1 << 30) + 16);
        assert_eq!(budget.observation().exceeded.memory_bytes, 16);
        assert!(
            budget
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: 1,
                        ..ResourceAmounts::default()
                    }
                )
                .is_err()
        );
        reservation.resize(4);
        assert_eq!(budget.observation().used.memory_bytes, 4);
        drop(reservation);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }
}
