//! Build-static identities shared by every relation-IPC process boundary.
//!
//! This module deliberately depends only on BLAKE3 and the Rust standard library so the stable
//! daemon, Pyrefly sidecar, and dated-nightly rustc extractor compile the exact same identity
//! projection without sharing provider-owned types.

/// Current relation-scoped control protocol version.
pub(crate) const RELATION_IPC_PROTOCOL_VERSION: u16 = 1;
/// Exact Arrow type and IPC universe admitted at every provider boundary.
pub(crate) const ARROW_TYPE_UNIVERSE: &str =
    "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5";
/// Exact Arrow IPC metadata version represented by the released protobuf enum.
pub(crate) const ARROW_IPC_METADATA_VERSION_V5: i32 = 5;
/// Only semantic encoding admitted by the relation protocol.
pub(crate) const TYPED_RELATION_ENCODING: &str = "typed-arrow-relation-stream";
/// One protobuf payload stays below the four-MiB control-frame ceiling after envelope overhead.
pub(crate) const RELATION_IPC_FRAGMENT_BYTES: usize = 3 * 1024 * 1024;

/// Fixed-width identity carried by the released protobuf control plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationWireIdentity {
    pub(crate) relation_id: [u8; 16],
    pub(crate) stream_id: [u8; 16],
    pub(crate) schema_fingerprint: [u8; 32],
    pub(crate) source_pin: [u8; 32],
    pub(crate) context_pin: [u8; 32],
}

/// Build one independently reproducible relation-stream identity.
///
/// `source_digest` and `context_digest` are canonical lowercase `b3:` digests. The stream ID
/// binds the provider run, semantic scope, relation, and both exact pins; callers cannot select a
/// stream ID independently from the evidence it identifies.
pub(crate) fn relation_wire_identity(
    relation_name: &str,
    schema_digest: &str,
    provider_run_id: &str,
    scope_id: &str,
    source_digest: &str,
    context_digest: &str,
) -> Result<RelationWireIdentity, &'static str> {
    if relation_name.is_empty() || provider_run_id.is_empty() || scope_id.is_empty() {
        return Err("relation, provider-run, and scope identities must be non-empty");
    }
    let schema_fingerprint = parse_b3(schema_digest)?;
    let source_pin = parse_b3(source_digest)?;
    let context_pin = parse_b3(context_digest)?;
    let relation_id = digest16(
        b"codefabric.relation-ipc.relation-id.v1\0",
        &[relation_name.as_bytes()],
    );
    let stream_id = digest16(
        b"codefabric.relation-ipc.stream-id.v1\0",
        &[
            provider_run_id.as_bytes(),
            scope_id.as_bytes(),
            relation_name.as_bytes(),
            source_pin.as_slice(),
            context_pin.as_slice(),
        ],
    );
    if relation_id == [0; 16]
        || stream_id == [0; 16]
        || schema_fingerprint == [0; 32]
        || source_pin == [0; 32]
        || context_pin == [0; 32]
    {
        return Err("relation stream identity contains a forbidden zero component");
    }
    Ok(RelationWireIdentity {
        relation_id,
        stream_id,
        schema_fingerprint,
        source_pin,
        context_pin,
    })
}

fn digest16(domain: &[u8], parts: &[&[u8]]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(part);
    }
    let mut output = [0_u8; 16];
    output.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    output
}

fn parse_b3(value: &str) -> Result<[u8; 32], &'static str> {
    let hexadecimal = value
        .strip_prefix("b3:")
        .filter(|value| value.len() == 64)
        .ok_or("relation pin is not a canonical b3-32 digest")?;
    if !hexadecimal
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("relation pin is not lowercase hexadecimal");
    }
    let mut output = [0_u8; 32];
    for (index, bytes) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(bytes[0]) << 4) | hex_nibble(bytes[1]);
    }
    Ok(output)
}

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_domain_separated_and_pin_bound() {
        let digest_a = format!("b3:{}", "11".repeat(32));
        let digest_b = format!("b3:{}", "22".repeat(32));
        let first = relation_wire_identity(
            "provider.test.relation.v1",
            &digest_a,
            "run-1",
            "scope-1",
            &digest_a,
            &digest_b,
        )
        .unwrap();
        let repeated = relation_wire_identity(
            "provider.test.relation.v1",
            &digest_a,
            "run-1",
            "scope-1",
            &digest_a,
            &digest_b,
        )
        .unwrap();
        let changed_scope = relation_wire_identity(
            "provider.test.relation.v1",
            &digest_a,
            "run-1",
            "scope-2",
            &digest_a,
            &digest_b,
        )
        .unwrap();

        assert_eq!(first, repeated);
        assert_eq!(first.relation_id, changed_scope.relation_id);
        assert_ne!(first.stream_id, changed_scope.stream_id);
        assert_eq!(first.schema_fingerprint, [0x11; 32]);
        assert_eq!(first.source_pin, [0x11; 32]);
        assert_eq!(first.context_pin, [0x22; 32]);
    }

    #[test]
    fn identity_rejects_noncanonical_or_zero_pins() {
        let valid = format!("b3:{}", "11".repeat(32));
        assert!(
            relation_wire_identity(
                "relation",
                &valid.to_uppercase(),
                "run",
                "scope",
                &valid,
                &valid
            )
            .is_err()
        );
        assert!(
            relation_wire_identity(
                "relation",
                &format!("b3:{}", "00".repeat(32)),
                "run",
                "scope",
                &valid,
                &valid,
            )
            .is_err()
        );
    }
}
