# `serde_json_canonicalizer` in Rust — advanced technical reference / RFC 8785 canonicalization catalog for LLM coding agents

Modeled after the exhaustive CodeFabric library-reference format: release-pinned source anchors first, then a capability map, exact API surface, semantic constraints, integration patterns, failure modes, performance considerations, tests, and agent execution rules.

## Version / source anchors

**Release anchor:** `serde_json_canonicalizer = 0.3.2` (released 2026-02-03; current release as of 2026-08-20).

**CodeFabric role:** authoritative Rust RFC 8785/JCS serializer for canonical bytes. It owns generic JSON string escaping, object-key ordering, and ECMAScript-compatible finite-double rendering. Repository code remains responsible for pre-canonicalization validation and CodeFabric-specific framing.

Source-of-truth hierarchy:

| Priority | Source | Use |
|---:|---|---|
| 1 | `docs.rs/serde_json_canonicalizer/0.3.2` | exact public API and release docs |
| 2 | released crate source / `Cargo.toml` | implementation and dependency behavior |
| 3 | RFC 8785 | canonicalization contract |
| 4 | CodeFabric `codefabric-jcs-v1` design review / fixture corpus | stricter application constraints |

Primary anchors:

- https://docs.rs/serde_json_canonicalizer/0.3.2/serde_json_canonicalizer/
- https://docs.rs/crate/serde_json_canonicalizer/0.3.2/source/
- https://www.rfc-editor.org/rfc/rfc8785
- https://github.com/evik42/serde-json-canonicalizer

## Feature inventory

The crate deliberately has a small surface. Its value is semantic rather than API breadth:

- serialize any Serde `Serialize` value as RFC 8785 JCS;
- emit canonical UTF-8 to `Vec<u8>`, `String`, or any `std::io::Write` sink;
- canonicalize an already-parsed JSON text through `pipe`;
- sort object names according to UTF-16 code-unit order required by JCS;
- render finite IEEE-754 doubles using ECMAScript-compatible `ryu-js` formatting;
- reject non-finite floating values through JSON/JCS serialization constraints;
- reject maps whose keys cannot be represented as JSON strings;
- reuse `serde_json::Result` / `serde_json::Error` rather than introducing a parallel error hierarchy;
- support Serde-derived application structs and standard containers directly.

It is **not** a schema validator, duplicate-key detector, arbitrary-precision canonicalizer, Unicode normalizer, cryptographic hash library, or CodeFabric format validator.

## Installation and pinning

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = { version = "=1.0.151", features = ["arbitrary_precision"] }
serde_json_canonicalizer = "=0.3.2"
```

Use an exact pin in the canonicalization component. The canonicalizer itself depends on Serde, `serde_json` with `float_roundtrip`, and `ryu-js`; the application may enable additional `serde_json` features such as `arbitrary_precision` for **input validation**, but that does not turn JCS into arbitrary-precision JSON.

## Public API surface

### `to_vec`

```rust
pub fn to_vec<S: serde::Serialize>(value: &S) -> serde_json::Result<Vec<u8>>
```

Canonical choice for hashing/signing/fingerprinting because the protocol contract is bytes. Internally it writes into a preallocated byte vector through the JCS serializer.

```rust
let canonical = serde_json_canonicalizer::to_vec(&value)?;
```

### `to_string`

```rust
pub fn to_string<S: serde::Serialize>(value: &S) -> serde_json::Result<String>
```

Useful for diagnostics and text APIs. It represents the same UTF-8 bytes as `to_vec`.

### `to_writer`

```rust
pub fn to_writer<S, W>(value: &S, writer: &mut W) -> serde_json::Result<()>
where
    S: serde::Serialize,
    W: std::io::Write,
```

Use when the downstream abstraction already owns a writer or when avoiding the final output allocation matters. The serializer guarantees valid UTF-8 sequences to the sink.

### `pipe`

```rust
pub fn pipe(json: &str) -> serde_json::Result<String>
```

Implementation shape in 0.3.2 is conceptually:

```rust
let value: serde_json::Value = serde_json::from_str(json)?;
serde_json_canonicalizer::to_string(&value)
```

This is convenient but **not** suitable as the CodeFabric canonicalization ingress path because ordinary `serde_json::Value` materialization cannot preserve evidence of duplicate object member names and can erase the exact provenance needed for strict token validation.

## RFC 8785 mechanics that this crate should own

### Object member ordering

RFC 8785 orders JSON object property names according to their UTF-16 code units, not Rust UTF-8 byte order, Unicode scalar-value order, locale collation, or serialized/escaped key bytes. Do not pre-sort with `BTreeMap<String, _>` and assume that is JCS-equivalent for all Unicode keys. Let the canonicalizer perform JCS sorting.

### Number rendering

JCS is constrained to the interoperable IEEE-754/ECMAScript number model. The crate uses `ryu-js` for finite-double formatting, including exponent formatting and shortest round-trippable output according to the JavaScript/JCS expectations.

The canonicalizer documents a critical interaction with `serde_json`'s `arbitrary_precision`: wide numeric tokens can be preserved by `serde_json::Number` for inspection, but JCS output converts them to doubles. Therefore `arbitrary_precision` is an **ingress validation tool**, not an output capability. Reject out-of-domain integer tokens before canonicalization.

### String escaping

Do not implement a repository-owned escaping routine. The canonicalizer owns JSON/JCS escaping. CodeFabric additionally requires unchanged Unicode semantics: do not NFC/NFD normalize input strings before or after JCS unless a schema explicitly defines that transformation outside the canonicalization profile.

### Non-finite floats

`NaN`, positive infinity, and negative infinity are outside JSON/JCS and must fail. Never stringify them as application-defined tokens in the canonicalization layer.

## Serde data-model implications

JCS is JSON, so Serde values must ultimately map to JSON-compatible structures. In particular:

- object/map keys must serialize as strings;
- byte arrays do not acquire an application-specific base64 representation automatically;
- enum representations follow the caller's Serde attributes and are protocol-significant;
- `Option::None` serializes as JSON `null` unless field attributes skip it;
- field renames, flattening, aliases, custom serializers, and skipped fields can change canonical bytes;
- map ordering supplied by the Rust container is irrelevant to final JCS object ordering, but array ordering remains significant.

Treat Serde annotations on canonicalized contract types as part of the wire protocol.

## CodeFabric canonicalization boundary

Canonical ingress should be modeled as:

```text
raw JSON bytes/text
  -> strict decoder / duplicate-name detection
  -> preserve and validate numeric token domain
  -> schema / format validation
  -> deterministic normalization defined by CodeFabric only
  -> serde_json_canonicalizer::to_vec
  -> BLAKE3-256
  -> lowercase `b3:<64 hex>` framing
```

Repository-owned checks remain authoritative for duplicate keys, safe-integer bounds, typed integer/byte formats, lowercase IDs/digests, sorting of record arrays that model non-string-keyed maps, and BLAKE3 framing.

## Correct patterns

### Canonical bytes then checksum

```rust
use blake3::Hasher;

fn canonical_bytes<T: serde::Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json_canonicalizer::to_vec(value)?)
}

fn b3_frame(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}
```

The framing function is CodeFabric logic; the canonicalizer should never know about `b3:`.

### Writer path

```rust
let mut buf = Vec::new();
serde_json_canonicalizer::to_writer(&value, &mut buf)?;
```

Prefer `to_vec` unless the surrounding code already needs a writer abstraction. A streaming sink does not make JCS object sorting fully streaming: object keys and associated serialized content may require buffering to produce sorted output.

## Error model

All top-level APIs return `serde_json::Result`. Failures include:

- a custom `Serialize` implementation returning an error;
- a map containing non-string JSON keys;
- invalid/non-finite numeric values encountered by serialization;
- I/O errors from `to_writer`;
- parse errors in `pipe` before canonical serialization.

Wrap errors with application context at the call boundary, but preserve the original source error. Do not convert all failures into “canonicalization mismatch”; distinguish invalid input, schema/domain rejection, serialization failure, and sink I/O failure.

## Performance and allocation

The implementation is small and intended as a drop-in serialization surface, but canonical object ordering necessarily costs more than insertion-order JSON emission. Practical rules:

- canonicalize once at the protocol boundary; do not repeatedly canonicalize unchanged values;
- hash `to_vec` output directly rather than round-tripping through `String`;
- avoid building both `Value` and typed model representations unless validation requires it;
- benchmark unusually large objects with many members because sorting dominates there;
- do not trade correctness for a custom “fast path” that bypasses the canonicalizer.

## Security / protocol guidance

Canonicalization prevents representation ambiguity; it does not validate semantic authorization. A canonical byte sequence can still encode malicious or nonsensical data. Validate domain rules first.

Never canonicalize attacker-controlled JSON after duplicate names have already collapsed into a map if duplicate rejection is part of the protocol. Never accept a digest calculated from pre-validation bytes as equivalent to the digest of post-validation canonical bytes.

## Common failure modes

- Using `serde_json::to_vec` instead of the canonicalizer and assuming `BTreeMap` order is sufficient.
- Calling `pipe` on untrusted protocol input and thereby losing duplicate-key evidence.
- Enabling `arbitrary_precision` and assuming wide integer values will remain exact through JCS output.
- Unicode-normalizing property names before sorting.
- Hashing a diagnostic `String` produced with extra framing/newlines instead of the exact canonical byte vector.
- Serializing a contract enum or optional field differently in Rust and Python.
- Allowing custom `Serialize` implementations to emit a representation that has not been covered by cross-language fixtures.

## Agent checklist

Before changing code that uses this crate, verify:

- [ ] input has passed duplicate-name and numeric-domain validation;
- [ ] the value being serialized is the exact semantic value covered by the cross-language contract;
- [ ] `to_vec` is used for fingerprint bytes unless a writer is specifically required;
- [ ] no Unicode normalization or alternate key sorting was added;
- [ ] no custom float/string formatter exists in repository code;
- [ ] the resulting bytes are tested against the Python `rfc8785` implementation;
- [ ] all dependency pins and fixture gates remain intact.
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

## 1) Design philosophy and trust boundary

The crate is a specialized **serializer**, not a canonical-document processing framework. Its strongest deployment pattern is to receive an already-validated deterministic semantic value and emit the RFC 8785 bytes.

```text
validation owns what values are allowed
canonicalizer owns how allowed JSON values become bytes
hash layer owns fingerprinting/framing
```

Keeping those responsibilities separate avoids turning serializer quirks into application validation rules.

## 2) `to_vec` as the default protocol API

For signatures, hashes, content addresses, cache keys, and fixtures, use:

```rust
let canonical: Vec<u8> = serde_json_canonicalizer::to_vec(&value)?;
```

Advantages:

- output type matches the protocol byte concept;
- no UTF-8 re-encoding step before hashing;
- easy exact fixture comparison;
- ownership/lifetime simple at API boundaries.

## 3) `to_string`

`to_string` is semantically equivalent to UTF-8 interpretation of canonical output. Prefer it for diagnostics and textual transports, not intermediate checksum calculation.

```rust
let text = serde_json_canonicalizer::to_string(&value)?;
```

Because JCS JSON is UTF-8, `to_vec` and `to_string` should encode the same canonical representation; tests can assert this as an implementation invariant.

## 4) `to_writer`

Writer output supports sinks such as files, buffered sockets, digest adapters, or preallocated buffers. It does not imply that internal object sorting is zero-buffering.

```rust
let mut out = std::io::BufWriter::new(file);
serde_json_canonicalizer::to_writer(&value, &mut out)?;
```

If the final operation is BLAKE3 and objects are moderate in size, `to_vec` then one-shot hash is simpler and easier to fixture-test.

## 5) `pipe`

`pipe` parses source JSON through `serde_json::Value` then canonicalizes. It is valuable for:

- trusted CLI transformations;
- debugging canonical output;
- tests where source duplicate semantics are irrelevant;
- simple one-off conversion.

It is not the strict CodeFabric ingress because duplicate names and token-level policy need to be enforced before `Value` collapse.

## 6) Object sorting mechanics

JCS property ordering is based on UTF-16 code units of the **unescaped property name**. Consequences:

- ASCII-only tests are insufficient;
- Rust `String` lexicographic ordering is not the specification;
- UTF-8 byte order is not the specification;
- locale/collation APIs are irrelevant;
- sorting serialized `"..."` key strings is wrong.

Retain explicit Unicode ordering fixtures, including supplementary-plane characters.

## 7) Number serialization mechanics

The crate uses `ryu-js` to reproduce ECMAScript-compatible finite-double rendering required by JCS. This is the core reason not to hand-code numeric formatting.

Behavior to fixture includes:

- `-0.0` canonical representation;
- fixed-vs-exponent thresholds;
- exponent sign/leading zeros where applicable;
- shortest round-trippable finite numbers;
- subnormals;
- large finite values.

## 8) `arbitrary_precision` interaction

An upstream `serde_json::Number` may preserve an arbitrary source token for validation. The JCS serializer still targets interoperable ECMAScript numeric semantics and can convert represented values into double-domain output.

Therefore:

```text
arbitrary_precision == inspect/reject early
arbitrary_precision != expand JCS numeric domain
```

The safe-integer check belongs before calling `to_vec`.

## 9) String serialization mechanics

The serializer owns JSON escapes required by RFC 8785. Application code must not:

- escape forward slashes/HTML characters after serialization;
- force ASCII-only output;
- normalize Unicode;
- rewrite escape case/forms;
- remove bytes that “look like whitespace” inside strings.

Canonical bytes are final.

## 10) Serde map keys

JSON objects require string property names. A Serde value whose map keys do not serialize into an allowed JSON object-key representation is outside the direct JCS model.

CodeFabric's schema must perform non-string map conversion before the serializer. Do not patch the canonicalizer to stringify arbitrary keys.

## 11) Arrays are never sorted by JCS

RFC 8785 preserves array order. This is crucial for CodeFabric:

```text
object property order -> canonicalizer responsibility
array element order   -> semantic model responsibility
```

If a schema says a collection is unordered but represents it as records in an array, the repository must sort that array deterministically before canonicalization.

## 12) Serde custom serializer hazards

A `Serialize` implementation can emit a JSON representation unrelated to the obvious Rust fields. Review:

- manual `Serialize` impls;
- `serialize_with`;
- newtype wrappers;
- enum representation attributes;
- omission/default behavior;
- custom byte/string formatting.

The canonicalizer guarantees canonical bytes for **the representation it receives**; it does not prove that representation matches the schema.

## 13) Writer failures

`to_writer` can fail because the output sink fails. Preserve I/O source errors distinctly from semantic serialization errors. For content hashes built in memory, avoiding an external writer removes this failure class.

## 14) Deterministic typed-model pattern

```rust
#[derive(serde::Serialize)]
struct CanonicalRecord {
    id: String,
    enabled: bool,
    items: Vec<Item>, // already sorted if schema says unordered collection
}

fn bytes(record: &CanonicalRecord) -> serde_json::Result<Vec<u8>> {
    serde_json_canonicalizer::to_vec(record)
}
```

Do normalization/sorting in constructors or a validation/normalization stage rather than inside ad hoc `Serialize` implementations, so semantic changes remain visible and testable.

## 15) Dynamic `Value` pattern

Dynamic values are appropriate for schema-generic canonicalization **after** strict source ingestion:

```rust
let value: serde_json::Value = strict_decode_and_validate(input)?;
let canonical = serde_json_canonicalizer::to_vec(&value)?;
```

The key phrase is “strict decode and validate.” Plain `serde_json::from_str::<Value>` is not the same contract.

## 16) Hashing without accidental text transformations

```rust
let canonical = serde_json_canonicalizer::to_vec(&value)?;
let digest = blake3::hash(&canonical);
```

Avoid:

```rust
let canonical = serde_json_canonicalizer::to_string(&value)?;
let platform_text = canonical.replace("\n", "\r\n");
let digest = blake3::hash(platform_text.as_bytes()); // wrong contract
```

Canonical output is protocol bytes, not a platform text file that should receive newline translation.

## 17) Differential conformance harness

Keep a fixture schema like:

```yaml
name: supplementary-key-order
input_semantic: ...
canonical_utf8_hex: ...
b3: b3:...
expected: accept
```

and negative fixtures with stable failure class.

Test both Rust and Python from the same fixture definitions so neither implementation becomes the unchallenged oracle.

## 18) RFC test vectors and generated properties

Combine:

- upstream RFC/JCS vectors;
- crate tests such as Appendix B number cases;
- CodeFabric-specific format cases;
- randomized maps/arrays/scalars;
- metamorphic tests that permute object insertion order.

High-value property:

```text
permuting object insertion order does not change canonical bytes
permuting an ordered array DOES change bytes unless semantic normalization re-sorts it first
```

## 19) Security posture

Canonicalization is often used before signatures/hashes, so byte ambiguity can become a security bug. Treat:

- dependency version;
- Serde representation;
- serializer configuration;
- Unicode policy;
- numeric-domain validation;
- pre-canonical sorting

as security/protocol-sensitive surfaces.

Do not add a “fast canonicalization” path that bypasses the reference serializer for common values unless differential tests prove exact equivalence over a broad corpus and the profile explicitly authorizes it.

## 20) Performance profile

Expected cost centers:

- serialization traversal;
- object member collection/sorting;
- number formatting;
- output allocation/copying.

For large records, reduce avoidable copies around the canonicalizer rather than replacing its semantics. Reuse validated typed models, hash returned bytes directly, and avoid duplicate ordinary-JSON serialization in the same request unless needed for a separate response.

## 21) Upgrade investigation checklist

Before changing `0.3.2`:

```text
[ ] inspect release notes/source diff
[ ] inspect serde_json / ryu-js dependency changes
[ ] compile with CodeFabric's exact feature graph
[ ] replay upstream JCS vectors
[ ] replay all positive CodeFabric fixtures byte-for-byte
[ ] replay all negative fixtures
[ ] compare Rust output with pinned Python rfc8785
[ ] benchmark representative large-object workloads
[ ] verify no accepted value changes bytes
```

A canonical byte change is a profile/version question, not a routine dependency-refresh decision.

## 22) Deployment matrix

| Deployment | Recommended API |
|---|---|
| content fingerprint | `to_vec` |
| canonical text diagnostic | `to_string` |
| canonical file/sink | `to_writer` |
| trusted one-off JSON canonicalizer | `pipe` |
| strict untrusted source | custom strict decoder -> `to_vec`, not `pipe` |

## 23) Agent execution playbook

```text
1. Verify the input is already valid under CodeFabric's stricter semantic rules.
2. Never implement object sorting/string escaping/float formatting in repository code.
3. Use to_vec for protocol hashes and fixtures.
4. Preserve arrays exactly unless schema normalization explicitly sorts them first.
5. Keep serde_json arbitrary_precision as ingress tooling, not an expanded JCS domain.
6. Keep canonical bytes immutable after serialization.
7. Hash those bytes with unkeyed 32-byte BLAKE3 and frame separately.
8. Treat dependency/Serialize-shape changes as protocol-sensitive.
```
