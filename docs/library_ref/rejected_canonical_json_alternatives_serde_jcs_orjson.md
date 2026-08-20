# Canonical JSON alternatives explicitly rejected for `codefabric-jcs-v1` — `serde_jcs` and `orjson`

This note is intentionally shorter than the adopted-library references. It exists so coding agents do not rediscover an attractive alternative and silently replace a dependency that was rejected by design review.

## Decision context

The accepted CodeFabric design selects maintained RFC 8785 implementations: Rust `serde_json_canonicalizer 0.3.2` and Python `rfc8785 0.1.4`. Two alternatives are explicitly not used as the fingerprint serializer.

## `serde_jcs 0.2.0` — do not use

The design review rejects `serde_jcs` because the maintained `serde_json_canonicalizer` project documents known RFC-divergence concerns in the older crate and exists specifically to provide a maintained, cross-language-compatible JCS implementation.

Agent rule:

```text
Do not replace serde_json_canonicalizer with serde_jcs based only on API similarity.
```

Even if `serde_jcs` exposes familiar `to_vec`/`to_string`/`to_writer` functions, byte-level equivalence is the requirement, not surface compatibility.

If this decision is ever revisited, the burden of proof is a complete RFC 8785 and CodeFabric fixture corpus plus source-level review of UTF-16 key ordering, number formatting, string escaping, arbitrary-precision interactions, error behavior, and maintenance/security posture.

## `orjson` — useful adapter, not JCS

`orjson` is a high-performance Python JSON serializer/deserializer with useful options such as key sorting and compact output. Those features do **not** constitute RFC 8785.

In particular, “sorted keys + no spaces” is only a superficial subset of canonical JSON concerns. JCS additionally fixes Unicode property-name ordering semantics, string escaping, number rendering, and accepted numeric domain.

Permitted posture:

- use `orjson` for ordinary application JSON where its behavior is suitable;
- use it for performance-sensitive adapters not participating in canonical fingerprints;
- do not use its output as `codefabric-jcs-v1` canonical bytes.

## Decision test for agents

Before proposing any serializer substitution, answer all of these with evidence:

- Does it implement RFC 8785 explicitly?
- Does it order property names by UTF-16 code units?
- Does it implement ECMAScript-compatible finite-double rendering?
- Does it preserve/validate the required input number domain?
- Does it reject non-finite values appropriately?
- Can duplicate source keys be rejected before map materialization?
- Does the shared cross-language fixture corpus pass byte-for-byte?
- Is the version exact-pinned and covered by the upgrade gate?

If any answer is missing, it is not a drop-in replacement for the canonicalization path.


---

## Agent-facing substitution guardrail

A proposal to replace the selected canonicalizer must demonstrate **RFC 8785 byte parity**, not merely:

- sorted keys;
- compact output;
- stable output on ASCII fixtures;
- high benchmark throughput;
- “canonical JSON” in a package description;
- a similar `dumps` / `to_vec` API.

### Required substitution evidence

```text
[ ] UTF-16 property ordering parity
[ ] ECMAScript finite-double rendering parity
[ ] canonical string escape parity
[ ] non-finite rejection
[ ] safe-integer/application-domain integration
[ ] duplicate-key strategy remains pre-map
[ ] byte-for-byte shared Rust/Python corpus parity
[ ] negative corpus parity
[ ] maintained/security posture acceptable
```

`serde_jcs 0.2.0` fails the design's maintenance/correctness-selection criterion; do not revive it because its API is familiar. `orjson` can remain an excellent fast adapter JSON library, but its key sorting/minimal output options define a different serialization surface from RFC 8785 and therefore cannot replace `rfc8785` for fingerprints.

## Appropriate uses that remain available

| Library | Appropriate use | Not appropriate |
|---|---|---|
| `serde_jcs` | none in `codefabric-jcs-v1` canonical path; historical comparison only | canonical bytes |
| `orjson` | high-throughput non-fingerprint API/adapters after independent evaluation | JCS fingerprint bytes |

Keep “fast ordinary JSON” and “canonical protocol JSON” as separate dependency roles even when one library could technically serialize the same application object.
