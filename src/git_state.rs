//! Read-only Git implementation boundary.

/// Application-owned hash algorithms supported by the configured gix profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectHashAlgorithm {
    Sha1,
    Sha256,
}

pub(crate) const fn supported_hash_algorithms() -> [ObjectHashAlgorithm; 2] {
    let selected = [gix::hash::Kind::Sha1, gix::hash::Kind::Sha256];
    match selected {
        [gix::hash::Kind::Sha1, gix::hash::Kind::Sha256] => {
            [ObjectHashAlgorithm::Sha1, ObjectHashAlgorithm::Sha256]
        }
        _ => unreachable!(),
    }
}
