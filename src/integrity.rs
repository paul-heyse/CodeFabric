//! Purpose-specific BLAKE3 content-integrity and cache-key operations.
//!
//! This module cannot mint application identities. Semantic identity and
//! fingerprint construction belongs to [`crate::identity`]; keyed authentication
//! belongs to [`crate::security`].

use std::fmt::Write as _;

#[cfg(any(
    feature = "canonical-json",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub use crate::identity_recipes::{CacheKeyDomain, IntegrityDomain};

/// Compute an unframed BLAKE3-256 content digest.
#[must_use]
pub fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Compute the repository's lowercase `b3:` content-digest representation.
#[must_use]
pub fn framed_digest(bytes: &[u8]) -> String {
    let digest = digest_bytes(bytes);
    format!("b3:{}", digest_hex_bytes(digest))
}

/// Compute lowercase hexadecimal BLAKE3-256 without a framing prefix.
#[must_use]
pub fn digest_hex(bytes: &[u8]) -> String {
    digest_hex_bytes(digest_bytes(bytes))
}

/// Convert an already-computed BLAKE3-256 digest to its lowercase `b3:` form.
#[must_use]
pub fn frame_digest(digest: [u8; 32]) -> String {
    format!("b3:{}", digest_hex_bytes(digest))
}

fn digest_hex_bytes(digest: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
    }
    encoded
}

/// Streaming content-integrity construction. It deliberately exposes no ID
/// truncation operation.
pub struct IntegrityHasher(blake3::Hasher);

impl IntegrityHasher {
    /// Start an unkeyed content-integrity digest.
    #[must_use]
    pub fn new() -> Self {
        Self(blake3::Hasher::new())
    }

    /// Start a domain-separated content-integrity digest selected from the
    /// governed hash-purpose registry.
    #[cfg(any(
        feature = "canonical-json",
        feature = "daemon",
        feature = "data-fabric",
        feature = "fact-generation",
        feature = "repository-state"
    ))]
    #[must_use]
    pub fn for_domain(domain: IntegrityDomain) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.bytes());
        Self(hasher)
    }

    /// Add exact bytes to the integrity preimage.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Finish as the full BLAKE3-256 digest.
    #[must_use]
    pub fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl Default for IntegrityHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming ephemeral cache-key construction. It deliberately exposes no ID
/// truncation operation and is distinct from persisted integrity evidence.
pub struct CacheKeyHasher(blake3::Hasher);

impl CacheKeyHasher {
    /// Start an unkeyed cache-key digest.
    #[must_use]
    pub fn new() -> Self {
        Self(blake3::Hasher::new())
    }

    /// Start a domain-separated ephemeral cache key selected from the
    /// governed hash-purpose registry.
    #[cfg(any(
        feature = "canonical-json",
        feature = "daemon",
        feature = "data-fabric",
        feature = "fact-generation",
        feature = "repository-state"
    ))]
    #[must_use]
    pub fn for_domain(domain: CacheKeyDomain) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.bytes());
        Self(hasher)
    }

    /// Add exact bytes to the cache-key preimage.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Finish as the full BLAKE3-256 cache key.
    #[must_use]
    pub fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl Default for CacheKeyHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wp55_integrity_authority_matches_blake3_without_id_minting() {
        let bytes = b"purpose-separated";
        assert_eq!(digest_bytes(bytes), *blake3::hash(bytes).as_bytes());
        assert_eq!(
            framed_digest(bytes),
            format!("b3:{}", blake3::hash(bytes).to_hex())
        );

        let mut streaming = IntegrityHasher::new();
        streaming.update(b"purpose-");
        streaming.update(b"separated");
        assert_eq!(streaming.finalize(), digest_bytes(bytes));
    }
}
