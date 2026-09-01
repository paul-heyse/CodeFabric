//! Keyed security hashes, isolated from semantic identity and content integrity.

pub use crate::identity_recipes::SecurityMacDomain;

/// A keyed BLAKE3 MAC-like construction for local bearer material.
///
/// The API returns only the full authenticator and cannot mint semantic IDs.
pub struct KeyedAuthenticator(blake3::Hasher);

impl KeyedAuthenticator {
    /// Start a keyed authenticator with a purpose-specific domain separator.
    #[must_use]
    pub fn new(key: &[u8; 32], domain: SecurityMacDomain) -> Self {
        let mut hasher = blake3::Hasher::new_keyed(key);
        hasher.update(domain.bytes());
        Self(hasher)
    }

    /// Add exact bytes to the keyed preimage.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Finish as the full keyed authenticator.
    #[must_use]
    pub fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

/// Hash a local secret token for constant-time comparison without exposing an
/// identity constructor.
#[must_use]
pub fn local_token_digest(domain: SecurityMacDomain, token: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.bytes());
    hasher.update(token);
    *hasher.finalize().as_bytes()
}

/// Render a full authenticator as lowercase hexadecimal without changing its
/// security purpose or truncating it into an ID.
#[must_use]
pub fn authenticator_hex(authenticator: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in authenticator {
        write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wp55_keyed_authority_preserves_full_authenticator() {
        let key = [7; 32];
        let mut actual = KeyedAuthenticator::new(&key, SecurityMacDomain::ResultLease);
        actual.update(b"payload");
        let mut registered_expected = blake3::Hasher::new_keyed(&key);
        registered_expected.update(b"codefabric.result-lease.v1\0");
        registered_expected.update(b"payload");
        assert_eq!(
            actual.finalize(),
            *registered_expected.finalize().as_bytes()
        );
    }
}
