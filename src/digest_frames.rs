/// Application-owned framed digest primitive shared across the Rust process boundary.
pub(crate) fn digest_frames(domain: &[u8], fields: impl IntoIterator<Item = Vec<u8>>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for field in fields {
        hasher.update(&(field.len() as u64).to_be_bytes());
        hasher.update(&field);
    }
    format!("b3:{}", hasher.finalize().to_hex())
}
