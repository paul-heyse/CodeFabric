# CodeFabric canonicalization library advanced-reference pack

**Generated:** 2026-08-20  
**Design basis:** `Library capability research: codefabric-jcs-v1 canonical JSON` (`accepted-for-wp06`).

## Adopted dependency map

| Layer | Language | Library / runtime | Reference target | Canonicalization responsibility |
|---|---|---|---|---|
| JCS serializer | Rust | `serde_json_canonicalizer` | 0.3.2 | RFC 8785 canonical bytes |
| strict JSON decoder/model | Rust | `serde_json` | 1.0.151 + `arbitrary_precision` | token/domain inspection + Serde JSON model |
| checksum | Rust | `blake3` | current API 1.8.6; retain repository pin | BLAKE3-256 bytes |
| bytes text format | Rust | `base64` | 0.22.1 | strict `URL_SAFE_NO_PAD` |
| JCS serializer | Python | `rfc8785` | 0.1.4 | RFC 8785 canonical bytes |
| strict JSON decoder | Python | stdlib `json` | CPython 3.14.7 | duplicate and number-token hooks |
| checksum | Python | `blake3` | 1.0.9 | BLAKE3-256 bytes |
| registry ingestion | Rust | `serde_yaml_ng` | 0.10.0 | YAML 1.1 -> validated Rust model |

## CodeFabric responsibility split

Libraries own generic, well-specified mechanics. Repository code owns only the application contract:

1. duplicate JSON member rejection before objects collapse into maps;
2. safe integer bound `[-9007199254740991, 9007199254740991]`;
3. canonical `codefabric-int64`, `codefabric-uint64`, and unpadded `codefabric-bytes` validation;
4. lowercase ASCII ID/digest validation;
5. deterministic sorting of non-string-keyed semantic maps represented as record arrays;
6. BLAKE3-256 framing as `b3:<64 lowercase hex>`;
7. contract-tree walking, generated-source digests, profiles, and drift detection.

No CodeFabric-owned JSON escaping routine, UTF-16 key comparator, or float formatter should be introduced while the adopted JCS libraries remain the profile implementation.

## Recommended agent reading order

1. `serde_json_canonicalizer...` and `rfc8785...` — canonical byte contract.
2. `serde_json...` and `python_stdlib_json...` — strict ingress and source-token preservation.
3. Rust/Python `blake3...` — checksum identity and framing boundary.
4. `base64...` — `codefabric-bytes` lexical contract.
5. `serde_yaml_ng...` — authoring/registry ingestion boundary.
6. rejected-alternatives note — prevents accidental serializer substitution.

## Cross-language invariant

For every accepted semantic fixture:

```text
Rust validated value   --serde_json_canonicalizer--> canonical bytes --blake3--> digest
Python validated value --rfc8785-------------------> same bytes       --blake3--> same digest
```

Any divergence is a protocol failure, not a formatting preference.

## Included documents

- `base64_rust_advanced_reference_0.22.1.md`
- `blake3_python_advanced_reference_1.0.9.md`
- `blake3_rust_advanced_reference_1.8.6.md`
- `python_stdlib_json_3.14.7_advanced_reference.md`
- `rejected_canonical_json_alternatives_serde_jcs_orjson.md`
- `rfc8785_python_advanced_reference_0.1.4.md`
- `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md`
- `serde_json_rust_advanced_reference_1.0.151.md`
- `serde_yaml_ng_rust_advanced_reference_0.10.0.md`
## Upgrade and compatibility policy

For CodeFabric canonicalization dependencies, a dependency update is **not** an ordinary implementation-only change. Replay the complete shared positive and negative fixture corpus before accepting any upgrade. Positive fixtures must preserve canonical bytes and `b3:` digests byte-for-byte. Negative fixtures must preserve rejection of duplicate names, unsafe integer tokens, non-finite values, malformed typed-format strings, non-canonical base64, and uppercase IDs/digests. If a serializer upgrade changes canonical bytes for any previously accepted value, introduce a new canonicalization profile/version rather than silently changing `codefabric-jcs-v1`.

Agent rule: do not infer compatibility from SemVer alone when a dependency participates in a byte-level protocol.


---

## Reference-pack architecture

The pack intentionally separates **source decoding**, **semantic validation/normalization**, **canonical byte serialization**, and **digest/format framing**. Coding agents should not collapse these stages merely because several libraries can parse or serialize JSON-like values.

```text
Rust JSON source:
serde_json(strict wrapper, arbitrary_precision)
  -> CodeFabric validation/normalization
  -> serde_json_canonicalizer
  -> blake3

Python JSON source:
stdlib json(strict hooks)
  -> CodeFabric validation/normalization
  -> rfc8785
  -> blake3

Rust YAML registry source:
serde_yaml_ng
  -> CodeFabric validation/normalization + JSON projection
  -> serde_json_canonicalizer
  -> blake3

Typed bytes field:
raw bytes <-> base64 URL_SAFE_NO_PAD text
```

### What is intentionally repository-owned

The design review leaves only the following semantics in CodeFabric code rather than generic serialization libraries: duplicate-key rejection; safe integer bounds; `codefabric-int64`, `codefabric-uint64`, and `codefabric-bytes`; lowercase IDs/digests; deterministic sorting of non-string-key record arrays; `b3:<64 lowercase hex>` framing; and contract-tree/digest/profile orchestration.

### Cross-language equivalence rule

A positive fixture is accepted only when Rust and Python produce:

```text
same semantic accepted value
same RFC 8785 bytes
same 32 BLAKE3 bytes
same `b3:` string
```

A negative fixture is successful when both implementations reject at the required contract boundary, even if their internal exception/error strings differ.

## Suggested reading bundles by task

### Implement strict Rust JSON ingress

1. `serde_json_rust_advanced_reference_1.0.151.md`
2. `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md`
3. `blake3_rust_advanced_reference_1.8.6.md`
4. `base64_rust_advanced_reference_0.22.1.md`

### Implement strict Python JSON ingress

1. `python_stdlib_json_3.14.7_advanced_reference.md`
2. `rfc8785_python_advanced_reference_0.1.4.md`
3. `blake3_python_advanced_reference_1.0.9.md`

### Implement registry YAML ingestion

1. `serde_yaml_ng_rust_advanced_reference_0.10.0.md`
2. `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md`
3. `blake3_rust_advanced_reference_1.8.6.md`

### Review dependency substitution proposals

Read `rejected_canonical_json_alternatives_serde_jcs_orjson.md` first, then the selected serializer reference for the corresponding language.

## Dependency-change decision tree

```text
Does the dependency participate before/at canonical bytes?
  no  -> normal compatibility review may be sufficient
  yes -> replay shared fixture corpus

Do any previously accepted values change canonical bytes?
  no  -> version bump may proceed after negative-corpus parity
  yes -> do NOT mutate codefabric-jcs-v1 in place; evaluate a new profile

Does a decoder begin accepting a previously rejected ambiguous form?
  yes -> determine whether repository strict validation still rejects it;
         if not, treat as a contract regression
```
