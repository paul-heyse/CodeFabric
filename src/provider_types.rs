//! Application-owned data transfer types shared by fact-provider adapters.

use crate::resource_budget::{
    ChargedSlice, ChargedValue, ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass,
};
use thiserror::Error;

/// Provider-compatible UTF-8 text plus character-boundary offsets into the
/// immutable original source image.
///
/// `original_byte_offsets` has one entry for every Unicode scalar boundary in
/// `text`, plus the terminal boundary. It intentionally is not indexed by UTF-8
/// byte offset: adapters build a checked boundary map once per accepted image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderText {
    pub text: ChargedValue<String>,
    pub original_byte_offsets: ChargedSlice<u64>,
}

/// Validated conversion from provider UTF-8 offsets to authoritative source bytes.
#[derive(Clone, Debug)]
pub(crate) struct ProviderBoundaryMap {
    provider_offsets: ChargedSlice<usize>,
    original_offsets: ChargedSlice<u64>,
}

/// A provider text/boundary map cannot resolve an authoritative source byte.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProviderBoundaryError {
    #[error("provider boundary map is invalid: {0}")]
    InvalidMap(String),
    #[error("provider byte offset {0} is not a character boundary")]
    InvalidOffset(usize),
}

impl ProviderBoundaryMap {
    pub(crate) fn new(text: &ProviderText) -> Result<Self, ProviderBoundaryError> {
        let provider_offsets = ChargedSlice::try_from_fn(
            text.original_byte_offsets.reservation().owner(),
            ResourceClass::Data,
            text.text.chars().count() + 1,
            || {
                let mut offsets = Vec::with_capacity(text.text.chars().count() + 1);
                offsets.extend(
                    text.text
                        .char_indices()
                        .map(|(offset, _)| offset)
                        .chain(std::iter::once(text.text.len())),
                );
                offsets
            },
        )
        .map_err(|error| ProviderBoundaryError::InvalidMap(error.to_string()))?;
        if provider_offsets.len() != text.original_byte_offsets.len() {
            return Err(ProviderBoundaryError::InvalidMap(format!(
                "{} provider boundaries but {} original boundaries",
                provider_offsets.len(),
                text.original_byte_offsets.len()
            )));
        }
        if text
            .original_byte_offsets
            .windows(2)
            .any(|window| window[0] > window[1])
        {
            return Err(ProviderBoundaryError::InvalidMap(
                "original offsets are not monotonic".into(),
            ));
        }
        Ok(Self {
            provider_offsets,
            original_offsets: text.original_byte_offsets.clone(),
        })
    }

    pub(crate) fn original(&self, provider_offset: usize) -> Result<u64, ProviderBoundaryError> {
        self.provider_offsets
            .binary_search(&provider_offset)
            .ok()
            .and_then(|index| self.original_offsets.get(index).copied())
            .ok_or(ProviderBoundaryError::InvalidOffset(provider_offset))
    }
}

impl ProviderText {
    /// Copy validated UTF-8 and its exact scalar boundaries only after admission.
    ///
    /// # Errors
    /// Rejects overflow or exhausted parent/operation capacity before allocation.
    pub fn from_validated_utf8(
        text: &str,
        budget: &ResourceBudget,
    ) -> Result<Self, ResourceBudgetError> {
        Self::from_validated_utf8_with_offset(text, 0, budget)
    }

    pub(crate) fn from_validated_utf8_with_offset(
        text: &str,
        original_start: u64,
        budget: &ResourceBudget,
    ) -> Result<Self, ResourceBudgetError> {
        let reservation = budget.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: text.len() as u64 + std::mem::size_of::<String>() as u64,
                ..ResourceAmounts::default()
            },
        )?;
        let original_byte_offsets = ChargedSlice::try_from_fn(
            budget,
            ResourceClass::Data,
            text.chars().count() + 1,
            || {
                let mut offsets = Vec::with_capacity(text.chars().count() + 1);
                offsets.extend(
                    text.char_indices()
                        .map(|(offset, _)| offset as u64 + original_start)
                        .chain(std::iter::once(text.len() as u64 + original_start)),
                );
                offsets
            },
        )?;
        Ok(Self {
            text: reservation.into_charged_value(text.to_owned()),
            original_byte_offsets,
        })
    }

    #[cfg(test)]
    pub fn for_test(text: &str) -> Self {
        Self::from_validated_utf8(text, &source_fixture_budget([2; 16])).unwrap()
    }

    /// Fingerprint the exact provider text and authoritative boundary geometry.
    /// This binds independently executed syntax providers to one immutable image.
    pub(crate) fn provider_image_fingerprint(&self) -> String {
        let mut hasher = crate::integrity::IntegrityHasher::for_domain(
            crate::integrity::IntegrityDomain::ProviderTextImage,
        );
        hasher.update(
            &u64::try_from(self.text.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hasher.update(self.text.as_bytes());
        hasher.update(
            &u64::try_from(self.original_byte_offsets.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for offset in self.original_byte_offsets.iter() {
            hasher.update(&offset.to_le_bytes());
        }
        crate::integrity::frame_digest(hasher.finalize())
    }
}

/// Explicit finite fixture owner, never compiled into a production constructor.
#[cfg(test)]
pub(crate) fn source_fixture_budget(workspace: [u8; 16]) -> ResourceBudget {
    use crate::resource_budget::ResourceBudgetPolicy;
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: 1024 * 1024 * 1024,
            disk_bytes: 1024 * 1024 * 1024,
            running_jobs: 32,
            queued_jobs: 512,
            retained_generations: 4,
            retained_bytes: 1024 * 1024 * 1024,
            rows: 1_000_000,
            pages: 65_536,
        },
        control_reserve: ResourceAmounts::default(),
    };
    ResourceBudget::try_process([1; 16], policy)
        .unwrap()
        .workspace(workspace, policy)
        .unwrap()
}
