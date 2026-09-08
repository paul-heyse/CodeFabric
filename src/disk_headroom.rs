//! Descriptor-bound physical free-space admission, independent of semantic table authority.
//!
//! The durable byte ledgers own files. This owner only reserves growth during actual writes,
//! retaining a separate cleanup floor. Other processes can consume the same filesystem after a
//! sample, so successful admission is not a promise that later I/O cannot return ENOSPC.

use std::fmt;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::resource_budget::ResourceClass;
use crate::secure_path::open_absolute_directory_nofollow;

pub(crate) const DATA_HEADROOM_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const CLEANUP_HEADROOM_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DiskSpaceSample {
    pub available_bytes: u64,
    pub allocation_unit: u64,
}

trait SpaceProbe: fmt::Debug + Send + Sync {
    fn sample(&self) -> Result<DiskSpaceSample, DiskHeadroomError>;
    fn device(&self) -> u64;
}

#[derive(Debug)]
struct DescriptorSpaceProbe {
    directory: OwnedFd,
    device: u64,
}

impl SpaceProbe for DescriptorSpaceProbe {
    fn sample(&self) -> Result<DiskSpaceSample, DiskHeadroomError> {
        let status = rustix::fs::fstatvfs(&self.directory)
            .map_err(|error| DiskHeadroomError::Io(error.to_string()))?;
        let allocation_unit = if status.f_frsize == 0 {
            status.f_bsize
        } else {
            status.f_frsize
        };
        if allocation_unit == 0 {
            return Err(DiskHeadroomError::InvalidFilesystem);
        }
        Ok(DiskSpaceSample {
            available_bytes: status
                .f_bavail
                .checked_mul(allocation_unit)
                .ok_or(DiskHeadroomError::Overflow)?,
            allocation_unit,
        })
    }

    fn device(&self) -> u64 {
        self.device
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DiskHeadroomError {
    #[error("physical disk observation unavailable: {0}")]
    Io(String),
    #[error("physical disk reports an invalid allocation unit")]
    InvalidFilesystem,
    #[error("physical disk resource calculation overflowed")]
    Overflow,
    #[error("write root differs from the admitted filesystem")]
    ForeignFilesystem,
    #[error("RESOURCE_CAPACITY: physical disk cleanup headroom would be consumed")]
    Insufficient {
        available_bytes: u64,
        required_bytes: u64,
    },
}

#[derive(Debug)]
struct HeadroomInner {
    probe: Arc<dyn SpaceProbe>,
    in_flight: Mutex<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalDiskHeadroom {
    inner: Arc<HeadroomInner>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DiskHeadroomObservation {
    pub available_bytes: u64,
    pub allocation_unit: u64,
    pub in_flight_growth_bytes: u64,
    pub data_floor_bytes: u64,
    pub cleanup_floor_bytes: u64,
}

impl LocalDiskHeadroom {
    pub(crate) fn open(directory: &Path) -> Result<Self, DiskHeadroomError> {
        let directory = open_absolute_directory_nofollow(directory)
            .map_err(|error| DiskHeadroomError::Io(error.to_string()))?;
        let device = rustix::fs::fstat(&directory)
            .map_err(|error| DiskHeadroomError::Io(error.to_string()))?
            .st_dev;
        let owner = Self::from_probe(Arc::new(DescriptorSpaceProbe { directory, device }));
        owner.observe()?;
        Ok(owner)
    }

    fn from_probe(probe: Arc<dyn SpaceProbe>) -> Self {
        Self {
            inner: Arc::new(HeadroomInner {
                probe,
                in_flight: Mutex::new(0),
            }),
        }
    }

    /// Reject a different mount rather than treating a sample from one filesystem as another's.
    pub(crate) fn validate_directory(&self, directory: &Path) -> Result<(), DiskHeadroomError> {
        let directory = open_absolute_directory_nofollow(directory)
            .map_err(|error| DiskHeadroomError::Io(error.to_string()))?;
        let device = rustix::fs::fstat(&directory)
            .map_err(|error| DiskHeadroomError::Io(error.to_string()))?
            .st_dev;
        if device != self.inner.probe.device() {
            return Err(DiskHeadroomError::ForeignFilesystem);
        }
        Ok(())
    }

    pub(crate) fn try_reserve_growth(
        &self,
        bytes: u64,
        class: ResourceClass,
    ) -> Result<DiskGrowthPermit, DiskHeadroomError> {
        let mut in_flight = self.inner.in_flight.lock().expect("disk headroom lock");
        let sample = self.inner.probe.sample()?;
        let rounded = bytes
            .max(1)
            .checked_add(sample.allocation_unit - 1)
            .map(|bytes| bytes / sample.allocation_unit)
            .and_then(|units| units.checked_mul(sample.allocation_unit))
            .ok_or(DiskHeadroomError::Overflow)?;
        let new_in_flight = in_flight
            .checked_add(rounded)
            .ok_or(DiskHeadroomError::Overflow)?;
        let floor = match class {
            ResourceClass::Data => DATA_HEADROOM_BYTES,
            ResourceClass::Control => CLEANUP_HEADROOM_BYTES,
        };
        let required_bytes = new_in_flight
            .checked_add(floor)
            .ok_or(DiskHeadroomError::Overflow)?;
        if required_bytes > sample.available_bytes {
            return Err(DiskHeadroomError::Insufficient {
                available_bytes: sample.available_bytes,
                required_bytes,
            });
        }
        *in_flight = new_in_flight;
        Ok(DiskGrowthPermit {
            owner: self.clone(),
            bytes: rounded,
        })
    }

    pub(crate) fn observe(&self) -> Result<DiskHeadroomObservation, DiskHeadroomError> {
        let in_flight = self.inner.in_flight.lock().expect("disk headroom lock");
        let sample = self.inner.probe.sample()?;
        Ok(DiskHeadroomObservation {
            available_bytes: sample.available_bytes,
            allocation_unit: sample.allocation_unit,
            in_flight_growth_bytes: *in_flight,
            data_floor_bytes: DATA_HEADROOM_BYTES,
            cleanup_floor_bytes: CLEANUP_HEADROOM_BYTES,
        })
    }
}

/// Hold until the actual write/cleanup has terminated, not merely until an async wait is dropped.
#[derive(Debug)]
pub(crate) struct DiskGrowthPermit {
    owner: LocalDiskHeadroom,
    bytes: u64,
}

impl Drop for DiskGrowthPermit {
    fn drop(&mut self) {
        let mut in_flight = self
            .owner
            .inner
            .in_flight
            .lock()
            .expect("disk headroom lock");
        *in_flight = in_flight
            .checked_sub(self.bytes)
            .expect("exact disk permit release");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Debug)]
    struct Probe(AtomicU64);
    impl SpaceProbe for Probe {
        fn sample(&self) -> Result<DiskSpaceSample, DiskHeadroomError> {
            Ok(DiskSpaceSample {
                available_bytes: self.0.load(Ordering::SeqCst),
                allocation_unit: 4096,
            })
        }
        fn device(&self) -> u64 {
            1
        }
    }

    #[test]
    fn rt_cpg_wp79_disk_floor_is_shared_and_preserves_cleanup() {
        let probe = Arc::new(Probe(AtomicU64::new(DATA_HEADROOM_BYTES + 8192)));
        let owner = LocalDiskHeadroom::from_probe(probe.clone());
        let data = owner.try_reserve_growth(1, ResourceClass::Data).unwrap();
        let other = owner
            .clone()
            .try_reserve_growth(4096, ResourceClass::Data)
            .unwrap();
        assert!(matches!(
            owner.try_reserve_growth(1, ResourceClass::Data),
            Err(DiskHeadroomError::Insufficient { .. })
        ));
        let cleanup = owner
            .try_reserve_growth(4096, ResourceClass::Control)
            .unwrap();
        assert_eq!(owner.observe().unwrap().in_flight_growth_bytes, 12288);
        probe.0.store(CLEANUP_HEADROOM_BYTES, Ordering::SeqCst);
        assert!(owner.try_reserve_growth(1, ResourceClass::Control).is_err());
        drop((data, other, cleanup));
        assert_eq!(owner.observe().unwrap().in_flight_growth_bytes, 0);
    }

    #[test]
    fn rt_cpg_wp79_disk_sample_is_descriptor_bound() {
        let root = tempfile::tempdir().unwrap();
        let owner = LocalDiskHeadroom::open(root.path()).unwrap();
        owner.validate_directory(root.path()).unwrap();
        let observation = owner.observe().unwrap();
        assert!(observation.allocation_unit > 0);
        assert_eq!(observation.in_flight_growth_bytes, 0);
        assert!(
            owner
                .try_reserve_growth(u64::MAX, ResourceClass::Data)
                .is_err()
        );
    }
}
