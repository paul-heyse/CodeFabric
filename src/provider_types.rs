//! Application-owned data transfer types shared by fact-provider adapters.

use std::sync::Arc;

use thiserror::Error;

/// Provider-compatible UTF-8 text plus character-boundary offsets into the
/// immutable original source image.
///
/// `original_byte_offsets` has one entry for every Unicode scalar boundary in
/// `text`, plus the terminal boundary. It intentionally is not indexed by UTF-8
/// byte offset: adapters build a checked boundary map once per accepted image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderText {
    pub text: Arc<str>,
    pub original_byte_offsets: Arc<[u64]>,
}

/// Validated conversion from provider UTF-8 offsets to authoritative source bytes.
#[derive(Clone, Debug)]
pub(crate) struct ProviderBoundaryMap {
    provider_offsets: Arc<[usize]>,
    original_offsets: Arc<[u64]>,
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
        let provider_offsets = text
            .text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.text.len()))
            .collect::<Vec<_>>();
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
            provider_offsets: provider_offsets.into(),
            original_offsets: Arc::clone(&text.original_byte_offsets),
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
