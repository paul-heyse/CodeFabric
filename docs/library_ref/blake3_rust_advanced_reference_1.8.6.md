# `blake3` in Rust — advanced technical reference / hashing, XOF, keyed mode, and deployment catalog for LLM coding agents

## Version / source anchors

**Library reference anchor:** `blake3 = 1.8.6`, the current Rust crate as of 2026-08-20. The CodeFabric design review says to use the repository's **existing `blake3`** dependency rather than introducing a new canonicalization-specific pin; therefore this document describes 1.8.6 but does **not** authorize changing the repository's resolved version.

Primary anchors:

- https://docs.rs/blake3/1.8.6/blake3/
- https://github.com/BLAKE3-team/BLAKE3
- https://github.com/BLAKE3-team/BLAKE3/blob/master/Cargo.toml
- https://blake3.io/

The official crate is the Rust implementation of BLAKE3. Default output length is 32 bytes (256 bits).

## CodeFabric role

After RFC 8785 canonical bytes are produced, hash **those bytes** in ordinary unkeyed BLAKE3 mode and frame the 32-byte digest as lowercase hexadecimal:

```text
b3:<64 lowercase hex characters>
```

The `b3:` prefix and lowercase-hex requirement are CodeFabric protocol semantics, not BLAKE3 library behavior.

## Core API inventory

Top-level functions:

- `blake3::hash(input)` — one-shot ordinary hash;
- `blake3::keyed_hash(&[u8; 32], input)` — one-shot keyed hash/MAC-like mode;
- `blake3::derive_key(context, key_material)` — BLAKE3 key-derivation mode.

Primary types:

- `Hasher` — incremental state;
- `Hash` — default 32-byte digest with constant-time equality;
- `OutputReader` — extendable-output reader;
- `HexError` — digest hex parsing error.

Constants include `OUT_LEN = 32`, `KEY_LEN = 32`, `BLOCK_LEN = 64`, and `CHUNK_LEN = 1024`.

## Cargo features

| Feature | Default | Capability / caution |
|---|---:|---|
| `std` | yes | `std::io` integrations, `update_reader`, `Write` for `Hasher`, `Read`/`Seek` for `OutputReader` |
| `rayon` | no | multithreaded `update_rayon`; all other APIs remain single-threaded |
| `mmap` | no | memory-mapped file hashing helpers |
| `zeroize` | no | `Zeroize` implementations for relevant state |
| `serde` | no | Serialize/Deserialize for `Hash` |
| `wasm32_simd` | no | assumes WASM SIMD availability |
| `neon` | target-sensitive | enables/assumes NEON on relevant ARM targets |
| `traits-preview` | no | RustCrypto digest-trait integration; explicitly no SemVer stability guarantee |

There are also implementation/testing feature switches that should not be used as application policy.

## Ordinary hashing

```rust
let digest: blake3::Hash = blake3::hash(canonical_bytes);
let hex = digest.to_hex().to_string();
let framed = format!("b3:{hex}");
```

`Hash` can expose its raw bytes and can be parsed/printed as hex. Keep the protocol representation lowercase even if an API accepts uppercase input.

## Incremental hashing

```rust
let mut h = blake3::Hasher::new();
h.update(part1);
h.update(part2);
let digest = h.finalize();
```

Use incremental hashing when canonical bytes arrive in chunks or when hashing non-canonical artifacts such as source trees. For CodeFabric JCS, the canonicalizer often produces a complete `Vec<u8>` first because JCS object sorting may require buffering; one-shot `hash` is therefore usually simplest.

`Hasher` can also be cloned to branch from a shared prefix and can be reset/reused where the API permits. Avoid state reuse if it makes protocol boundaries less obvious.

## Keyed hashing and key derivation

BLAKE3 has three distinct modes:

1. ordinary hash;
2. keyed hash using a 32-byte secret key;
3. derive-key mode using a globally unique, application-specific context string.

Do not substitute keyed mode for the CodeFabric `b3:` fingerprint unless the protocol is explicitly versioned to do so. Ordinary content fingerprints must be reproducible without secret state.

Key derivation context strings are domain-separation labels, not passwords or salts. Key material should be high-entropy material appropriate to the application.

## Extendable output (XOF)

`Hasher::finalize_xof()` returns an `OutputReader` that can produce arbitrary-length output. The first 32 bytes equal the default digest. With `std`, the reader integrates with `Read` and `Seek`.

XOF is powerful but **not** part of `b3:`: CodeFabric fixes output at 32 bytes. Never change digest length while retaining the same prefix/profile.

## Reader, mmap, and parallel paths

With `std`, `Hasher::update_reader` hashes an `io::Read` source. With `mmap`, file helpers can memory-map content. With `rayon`, sufficiently large inputs can be hashed in parallel; `update_mmap_rayon` combines the two optional features.

BLAKE3's tree design makes parallel hashing efficient, but parallel overhead can lose on small inputs. Canonical JSON records are often small enough that one-shot hashing is preferable. Benchmark before enabling `rayon` solely for fingerprints.

## CPU feature selection and portability

The crate ships optimized implementations and performs runtime CPU selection where supported. AArch64 has NEON baseline behavior; ARMv7 and WASM SIMD require more care because enabling features can imply hardware/runtime support. Build-system feature choices may change performance but must never change digest bytes.

Cross-platform fixture tests should prove this invariant over all supported deployment targets.

## `Hash` representation

Treat `Hash` as 32 opaque bytes. Formatting is a presentation layer. Recommended protocol adapter:

```rust
fn format_b3(hash: blake3::Hash) -> String {
    format!("b3:{}", hash.to_hex())
}
```

Recommended parser behavior for a canonical protocol string:

1. require exact `b3:` prefix;
2. require exactly 64 lowercase ASCII hex digits;
3. reject uppercase even if `Hash::from_hex` would accept it;
4. decode to 32 bytes;
5. compare digest bytes, using constant-time semantics when comparison secrecy matters.

## `hazmat` module

The crate exposes low-level tree primitives under `hazmat`. Do not use them for ordinary application hashing. They are for specialized protocol/tree integrations where the caller understands BLAKE3 internal domain flags and tree structure. Misuse can create constructions that are not equivalent to standard BLAKE3 modes.

## Security semantics

A cryptographic digest establishes byte identity, not trust, authorship, freshness, or authorization. If a hash is used as a cache key or integrity identifier, bind the correct semantic object to the canonical bytes first.

Ordinary BLAKE3 is not a password hash. Do not use it directly for password storage. Keyed BLAKE3 is not interchangeable with the unkeyed content fingerprint used by CodeFabric.

## Error model

Most ordinary hashing APIs are infallible once bytes are available. Errors arise around I/O (`update_reader`, mmap/file operations), digest parsing (`HexError`), allocation/resource boundaries, or application framing/validation.

Keep the distinction clear: a malformed `b3:` string is a CodeFabric format error; it is not a BLAKE3 algorithm failure.

## Performance guidance

- one-shot `hash` is ideal for canonical byte vectors;
- incremental `Hasher` avoids concatenation for independently produced chunks;
- parallelism is most useful on large inputs and can be slower on small payloads;
- memory mapping is a file-hashing optimization, not a canonical JSON requirement;
- avoid hex encoding until the protocol boundary; raw bytes are smaller and faster internally;
- avoid hashing the same canonical value repeatedly; memoize only if the semantic value is immutable and cache invalidation is exact.

## Canonicalization integration anti-patterns

- hashing pre-canonical JSON text;
- hashing a Rust debug representation;
- hashing UTF-16 or platform-native strings instead of canonical UTF-8 bytes;
- mixing keyed and unkeyed modes;
- using XOF length other than 32 while keeping `b3:`;
- accepting uppercase digest text in a protocol that declares lowercase canonical form;
- truncating the hash for storage without versioning the identifier format;
- using parallelism to complicate a tiny-record hot path without measurement.

## Agent checklist

- [ ] hash input is exactly RFC 8785 canonical bytes;
- [ ] ordinary unkeyed BLAKE3 mode is used for `b3:`;
- [ ] output is exactly 32 bytes / 64 lowercase hex characters;
- [ ] framing code, not the hash crate, owns the `b3:` prefix;
- [ ] repository's existing Rust `blake3` pin is preserved unless separately approved;
- [ ] cross-language fixtures compare raw digest bytes before string framing;
- [ ] `hazmat`, XOF, keyed mode, mmap, and rayon are used only for explicit needs.
## Testing matrix for agent-authored changes

At minimum, exercise:

- empty, scalar, nested-object, nested-array, and mixed-value cases;
- ASCII and non-ASCII object keys, including supplementary-plane code points;
- characters requiring JSON escaping and strings containing combining sequences;
- `0`, `-0.0`, integral floats, exponent forms, subnormals, largest finite doubles, and values adjacent to safe-integer boundaries;
- duplicate JSON member names at multiple nesting levels;
- malformed UTF-8 / malformed JSON where the API accepts raw bytes;
- deterministic repeated execution and cross-process execution;
- Rust↔Python byte equality for canonical bytes and BLAKE3 digest framing;
- intentionally malformed `codefabric-bytes`, `codefabric-int64`, `codefabric-uint64`, ID, and digest values;
- very deep / large inputs according to the caller's resource-limit policy.
## Upgrade and compatibility policy

For CodeFabric canonicalization dependencies, a dependency update is **not** an ordinary implementation-only change. Replay the complete shared positive and negative fixture corpus before accepting any upgrade. Positive fixtures must preserve canonical bytes and `b3:` digests byte-for-byte. Negative fixtures must preserve rejection of duplicate names, unsafe integer tokens, non-finite values, malformed typed-format strings, non-canonical base64, and uppercase IDs/digests. If a serializer upgrade changes canonical bytes for any previously accepted value, introduce a new canonicalization profile/version rather than silently changing `codefabric-jcs-v1`.

Agent rule: do not infer compatibility from SemVer alone when a dependency participates in a byte-level protocol.


---

# Extended capability catalog

## 1) BLAKE3 mental model

BLAKE3 exposes four related capabilities through one construction family:

1. ordinary unkeyed hashing;
2. keyed hashing / MAC-like use with a 32-byte secret key;
3. context-separated key derivation;
4. extendable output (XOF) beyond the default 32-byte digest.

These modes are deliberately distinct. Never substitute one because the output length “looks the same.” CodeFabric's checksum contract uses **ordinary unkeyed BLAKE3 with 32 output bytes** over already-canonical bytes.

## 2) Constants and protocol-relevant sizes

Important public constants include:

- `OUT_LEN = 32` — ordinary digest size;
- `KEY_LEN = 32` — keyed-hash key size;
- `BLOCK_LEN = 64` — internal block size;
- `CHUNK_LEN = 1024` — BLAKE3 chunk size.

Only `OUT_LEN` belongs directly in CodeFabric checksum framing. Do not expose internal block/chunk sizes as application protocol concepts.

## 3) One-shot ordinary hashing

```rust
let hash: blake3::Hash = blake3::hash(canonical_bytes);
let hex = hash.to_hex();
```

Use one-shot hashing when the complete canonical byte slice is already present. This is the clearest CodeFabric path after `serde_json_canonicalizer::to_vec`.

## 4) Incremental `Hasher`

```rust
let mut hasher = blake3::Hasher::new();
hasher.update(part1);
hasher.update(part2);
let hash = hasher.finalize();
```

Incremental hashing is mathematically equivalent to hashing the concatenation of the same byte chunks in the same order.

### Framing hazard

That equivalence does **not** mean arbitrary field concatenation is a safe structured-data hash:

```text
hash("ab" || "c") == hash("a" || "bc")
```

if no framing separates fields. CodeFabric avoids this ambiguity by hashing one complete JCS byte sequence rather than ad hoc field concatenations.

## 5) Reader and large-input paths

`Hasher` provides I/O-oriented update paths for reading data, plus optional memory-map/parallel support under the corresponding feature set. Use these for file hashing when the protocol hashes file bytes directly.

For canonical JSON, the canonicalizer may need object-level buffering/sorting, so do not assume a fully streaming `Read -> BLAKE3` pipeline is available merely because BLAKE3 itself streams.

## 6) Parallel hashing with `rayon`

Feature `rayon` exposes parallel update paths intended for sufficiently large buffers. Parallelism changes throughput, not digest semantics.

Agent rule: do not add `rayon` to a small-message checksum path without benchmarks. Thread-pool overhead can dominate typical registry records.

## 7) Memory-mapped input

Feature `mmap` adds file-backed memory-map helpers. This can reduce copy overhead for large immutable files, but introduces filesystem-specific concerns:

- file contents can change concurrently unless the caller controls them;
- mapping failures and I/O failures need normal error handling;
- mmap does not help in-memory canonical JSON records;
- do not mmap untrusted arbitrary paths simply to “optimize hashing.”

## 8) `Hash` type

`blake3::Hash` is the fixed-size ordinary result type. Prefer retaining it as bytes/typed hash internally until the boundary that requires text.

Typical operations:

```rust
let h = blake3::hash(data);
let bytes: &[u8; 32] = h.as_bytes();
let lower_hex = h.to_hex().to_string();
```

Text formatting is application representation, not hash computation.

### CodeFabric framing

```rust
fn frame_b3(hash: blake3::Hash) -> String {
    format!("b3:{}", hash.to_hex())
}
```

Validate exactly 64 lowercase hex characters after `b3:` when consuming a checksum string. The `blake3` crate should not be asked to own the prefix policy.

## 9) Keyed hashing

```rust
let key: [u8; blake3::KEY_LEN] = /* secret */;
let tag = blake3::keyed_hash(&key, message);
```

or incrementally:

```rust
let mut h = blake3::Hasher::new_keyed(&key);
h.update(message);
let tag = h.finalize();
```

Keyed mode is not CodeFabric's `b3:` checksum. If an authenticated protocol later uses keyed BLAKE3, give it a distinct field/type/profile so a plain checksum can never be confused with a keyed tag.

## 10) Key derivation mode

BLAKE3 provides a dedicated context-separated derivation mode. Use a stable, globally descriptive context string rather than using ordinary unkeyed hashing as a home-grown KDF.

Key derivation and `b3:` checksums are separate security domains even when both use BLAKE3 internals.

## 11) Extendable output (XOF)

`Hasher::finalize_xof()` yields an `OutputReader` that can fill arbitrary-length output and supports seeking within the deterministic output stream.

Use cases include deterministic expansion or protocols explicitly standardized around BLAKE3 XOF. It is **not** the CodeFabric checksum representation. The checksum always uses the normal 32-byte output.

A reviewer should treat any `finalize_xof`, non-32-byte digest, or seek operation in canonicalization code as a protocol change requiring justification.

## 12) CPU dispatch and portability

The crate chooses optimized implementations for supported CPU features at runtime/build time as appropriate. SIMD/backend choice must not change digest bytes.

Avoid forcing implementation-disabling features (`no_sse2`, `no_avx2`, etc.) in application code unless building for a constrained target or diagnosing a backend issue. Such knobs are performance/portability controls, not hash semantics.

## 13) Cargo feature policy

A production dependency should enable only capabilities it needs:

```toml
blake3 = "=YOUR_EXISTING_PIN"
```

Possible features include normal `std`, optional `rayon`, `mmap`, `serde`, `zeroize`, architecture-specific acceleration controls, and specialized traits/preview surfaces depending on release.

CodeFabric canonical checksums need no parallel/mmap feature merely to hash ordinary canonical records.

## 14) `serde` feature

When enabled, the `Hash` type can participate in Serde representation. Do not let this silently define the `b3:<hex>` protocol field. Protocol checksum strings should use explicit parsing/validation so the prefix and lowercase rule remain visible.

## 15) `zeroize` feature and secret state

`zeroize` matters principally to keyed/derivation use where secret material lives in memory. It is not necessary to make a public checksum “more secure.” If keyed mode is introduced, review lifetime/copying/logging of keys separately from hash API selection.

## 16) Error surface

One-shot in-memory hashing is infallible for a byte slice. I/O-oriented helpers can fail because the underlying reader/file/mapping fails. Text parsing of hash representations can fail if the input is malformed.

Keep these failure classes separate from:

- canonicalization failure;
- schema validation failure;
- checksum mismatch.

A mismatch is a normal verification result, not necessarily a `blake3` library error.

## 17) Constant-time considerations

For a public content digest, ordinary equality semantics are usually sufficient. For future secret authentication tags, use an equality/checking approach suitable for the threat model rather than assuming every generic string comparison is acceptable.

Never compare textual checksum strings before first validating canonical prefix/case/length if the text format itself is part of the contract.

## 18) Correct CodeFabric pipeline

```rust
fn canonical_b3<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    let bytes = serde_json_canonicalizer::to_vec(value)?;
    let digest = blake3::hash(&bytes);
    Ok(format!("b3:{}", digest.to_hex()))
}
```

Preconditions—duplicate keys, numeric domain, formats, deterministic record-array ordering—must already have been enforced before this function receives the semantic value.

## 19) Verification pattern

Prefer parsing/validating the textual frame into the expected 32 bytes or normalized typed hash representation, then compare with the newly computed hash.

```text
input checksum
 -> require `b3:`
 -> require exactly 64 lowercase hex chars
 -> decode/parse digest
 -> compute BLAKE3(canonical_bytes)
 -> compare typed bytes
```

Do not silently lowercase an uppercase digest; uppercase is invalid under the CodeFabric contract.

## 20) Performance decision table

| Input | Path |
|---|---|
| small in-memory canonical JSON | `blake3::hash` |
| chunked generated bytes | `Hasher::new` + `update` |
| large file | reader/mmap path after benchmark |
| very large memory buffer | optional Rayon path after benchmark |
| XOF protocol | `finalize_xof` only when protocol explicitly requires |

## 21) Testing matrix

Include known BLAKE3 vectors plus application framing vectors:

```text
[ ] empty canonical document where allowed by surrounding schema
[ ] canonical byte fixture -> expected 32 bytes
[ ] exact lowercase `b3:` text
[ ] uppercase text rejected at format layer
[ ] wrong prefix rejected
[ ] 63/65 hex chars rejected
[ ] one-bit payload change changes digest
[ ] same semantic JSON spellings canonicalize to same digest
[ ] semantically different canonical values do not share expected fixture
[ ] Rust digest == Python blake3 digest for every shared fixture
```

## 22) Deployment advisory

| Deployment | Recommended stance |
|---|---|
| canonical registry/compiler | one-shot hash of canonical `Vec<u8>` |
| file integrity tool | incremental/reader/mmap as appropriate |
| high-throughput large-blob service | benchmark Rayon/mmap features |
| secret-key protocol | keyed/KDF mode with separate key-management review |
| no-std target | verify exact feature graph and API availability |

## 23) Agent execution rules

```text
Do not hash pre-canonical JSON text.
Do not hash a Rust/Python object by ad hoc field concatenation.
Do not use keyed or XOF mode for `b3:`.
Keep the repository's exact Rust blake3 pin until the fixture gate approves a change.
Treat textual lowercase/prefix validation as CodeFabric logic.
Cross-check every checksum fixture in Rust and Python.
```
