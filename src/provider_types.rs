//! Application-owned data transfer types shared by fact-provider adapters.

use std::sync::Arc;

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
