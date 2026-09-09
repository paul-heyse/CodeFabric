//! Whole-process memory observations, separate from managed DataFusion reservations.
//!
//! RSS is sampled at data-work admission and execution checkpoints. This prevents
//! adding more work under observed pressure; it cannot prevent a native allocation
//! between samples or guarantee that the OS will not terminate the process.

use std::sync::{Mutex, OnceLock};

use crate::resource_budget::ResourceClass;

/// Conservative initial workstation thresholds, independent of the 2 GiB managed pool.
pub const RSS_PAUSE_BYTES: u64 = 3 * 1024 * 1024 * 1024;
pub const RSS_RESUME_BYTES: u64 = 5 * 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct ProcessMemoryObservation {
    /// None means this platform does not supply the current RSS probe.
    pub resident_bytes: Option<u64>,
    /// Maximum sampled RSS; this is not an allocator or OS lifetime peak.
    pub peak_sampled_resident_bytes: Option<u64>,
    pub data_paused: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMemoryError {
    #[error("PROCESS_MEMORY_OBSERVATION_UNAVAILABLE: {0}")]
    Observation(String),
    #[error(
        "PROCESS_MEMORY_BACKPRESSURE: rss={resident_bytes}, pause={pause_bytes}, resume={resume_bytes}; release retained work and retry"
    )]
    Backpressure {
        resident_bytes: u64,
        pause_bytes: u64,
        resume_bytes: u64,
    },
}

struct ProcessMemoryGuard {
    pause: u64,
    resume: u64,
    state: Mutex<ProcessMemoryObservation>,
}

impl ProcessMemoryGuard {
    fn observe(&self, resident: Option<u64>) -> ProcessMemoryObservation {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.resident_bytes = resident;
        if let Some(bytes) = resident {
            state.peak_sampled_resident_bytes =
                Some(state.peak_sampled_resident_bytes.unwrap_or(0).max(bytes));
            if bytes >= self.pause {
                state.data_paused = true;
            } else if bytes <= self.resume {
                state.data_paused = false;
            }
        }
        *state
    }

    fn check(
        &self,
        class: ResourceClass,
        observation: ProcessMemoryObservation,
    ) -> Result<(), ProcessMemoryError> {
        if class == ResourceClass::Data && observation.data_paused {
            return Err(ProcessMemoryError::Backpressure {
                resident_bytes: observation.resident_bytes.unwrap_or(0),
                pause_bytes: self.pause,
                resume_bytes: self.resume,
            });
        }
        Ok(())
    }
}

fn guard() -> &'static ProcessMemoryGuard {
    // Scheduling telemetry only: no graph state, workspace authority or allocation receipts.
    static GUARD: OnceLock<ProcessMemoryGuard> = OnceLock::new();
    GUARD.get_or_init(|| ProcessMemoryGuard {
        pause: RSS_PAUSE_BYTES,
        resume: RSS_RESUME_BYTES,
        state: Mutex::new(ProcessMemoryObservation::default()),
    })
}

/// Observe actual current RSS. Unsupported platforms explicitly report None.
pub fn sample() -> Result<ProcessMemoryObservation, ProcessMemoryError> {
    Ok(guard().observe(read_resident_bytes()?))
}

pub(crate) fn admit(class: ResourceClass) -> Result<(), ProcessMemoryError> {
    // Failure of a data probe must never prevent status, cancellation or cleanup.
    if class == ResourceClass::Control {
        return Ok(());
    }
    guard().check(class, sample()?)
}

#[cfg(target_os = "linux")]
fn read_resident_bytes() -> Result<Option<u64>, ProcessMemoryError> {
    use std::io::Read as _;
    const LIMIT: u64 = 64 * 1024;
    // Fixed kernel-owned telemetry, never workspace source or an arbitrary caller path.
    let file = std::fs::File::open("/proc/self/status")
        .map_err(|error| ProcessMemoryError::Observation(error.to_string()))?;
    let mut text = String::new();
    file.take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|error| ProcessMemoryError::Observation(error.to_string()))?;
    if text.len() as u64 > LIMIT {
        return Err(ProcessMemoryError::Observation(
            "proc status exceeded its byte bound".into(),
        ));
    }
    parse_resident_bytes(&text).map(Some)
}

#[cfg(not(target_os = "linux"))]
fn read_resident_bytes() -> Result<Option<u64>, ProcessMemoryError> {
    Ok(None)
}

#[cfg(any(target_os = "linux", test))]
fn parse_resident_bytes(status: &str) -> Result<u64, ProcessMemoryError> {
    let invalid = || ProcessMemoryError::Observation("proc status has no valid VmRSS in kB".into());
    let mut values = status
        .lines()
        .filter_map(|line| line.strip_prefix("VmRSS:"));
    let mut fields = values.next().ok_or_else(invalid)?.split_whitespace();
    let bytes = fields
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|value| value.checked_mul(1024))
        .ok_or_else(invalid)?;
    if fields.next() != Some("kB") || fields.next().is_some() || values.next().is_some() {
        return Err(invalid());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_backpressure_has_hysteresis_and_preserves_control() {
        let guard = ProcessMemoryGuard {
            pause: 100,
            resume: 80,
            state: Mutex::default(),
        };
        for (rss, paused) in [
            (90, false),
            (100, true),
            (90, true),
            (80, false),
            (95, false),
        ] {
            let observation = guard.observe(Some(rss));
            assert_eq!(observation.data_paused, paused);
            assert_eq!(
                guard.check(ResourceClass::Data, observation).is_err(),
                paused
            );
            assert!(guard.check(ResourceClass::Control, observation).is_ok());
        }
        assert_eq!(guard.observe(None).resident_bytes, None);
        assert_eq!(guard.observe(None).peak_sampled_resident_bytes, Some(100));
    }

    #[test]
    fn rss_observation_checks_units_presence_duplicates_and_overflow() {
        assert_eq!(
            parse_resident_bytes("Name: daemon\nVmRSS:\t123 kB\n").unwrap(),
            123 * 1024
        );
        for invalid in [
            "",
            "VmRSS: 3 MB",
            "VmRSS: 1 kB extra",
            "VmRSS: 1 kB\nVmRSS: 2 kB",
            "VmRSS: 18446744073709551615 kB",
        ] {
            assert!(parse_resident_bytes(invalid).is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_probe_observes_real_residency() {
        assert!(read_resident_bytes().unwrap().unwrap() > 0);
    }
}
