---
title: "Library capability research: codefabric-jcs-v1 canonical JSON"
status: accepted-for-wp06
date: 2026-08-20
plan: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md
packet: WP06
---

# Library capability research: `codefabric-jcs-v1`

## Decision

Adopt maintained RFC 8785 serializers and keep only CodeFabric-specific validation,
format semantics, checksum framing, and duplicate-key detection in repository code.

- Rust: exact `serde_json_canonicalizer` 0.3.2 for canonical bytes;
  `serde_json` 1.0.151 with `arbitrary_precision` so the validator can inspect the
  original number token before canonicalization; existing `blake3`; exact `base64`
  0.22.1 `URL_SAFE_NO_PAD` for `codefabric-bytes`.
- Python: exact `rfc8785` 0.1.4 for canonical bytes; stdlib `json.loads` with an
  `object_pairs_hook` and strict number hooks at the input boundary; exact `blake3`
  1.0.9 for the cross-language checksum contract.
- Registry YAML ingestion: exact `serde_yaml_ng` 0.10.0, which is Serde-native and
  declares Rust 1.64 compatibility, below CodeFabric's verified 1.94.1 floor.

Do not use `serde_jcs` 0.2.0: it exposes the expected serialization functions, but
the maintained canonicalizer documents known RFC divergences in that older crate.
Do not use `orjson` as the fingerprint serializer: it remains useful adapter
infrastructure, but sorting/minimal output is not the RFC 8785 contract.

## Requirements fit

Query AC-G-53 requires RFC 8785 string escaping, UTF-16 object-key ordering, and
ECMAScript-compatible number rendering, plus stricter CodeFabric rules: duplicate
rejection, safe-integer bounds, schema formats, lowercase IDs/digests, unchanged
Unicode, sorted record arrays for non-string keys, and `b3:` checksums.

`serde_json_canonicalizer` owns the difficult generic serialization mechanics. Its
formatter uses UTF-16 sorting keys, `ryu-js` finite-double rendering, and rejects
NaN/infinity. Its arbitrary-precision documentation also confirms that wide numeric
tokens are converted to doubles during JCS output, so CodeFabric must reject unsafe
integer tokens first rather than enable arbitrary precision as an output capability.
The upstream API is a drop-in `to_vec`/`to_string`/`to_writer` surface.

The Trail of Bits Python implementation owns the same RFC mechanics and rejects
unsafe Python integers, non-finite floats, invalid UTF-8 strings, and non-string map
keys. Python's stdlib decoder remains necessary because an already-materialized
`dict` cannot reveal duplicate source keys.

## Custom-code boundary

Repository code remains authoritative only for:

1. duplicate-key rejection before a JSON object becomes a map;
2. the inclusive safe-integer bound `[-9007199254740991, 9007199254740991]`;
3. canonical `codefabric-int64`, `codefabric-uint64`, and unpadded
   `codefabric-bytes` validation;
4. lowercase ASCII ID/digest validation;
5. canonical sorting of non-string-keyed maps represented as key/value records;
6. BLAKE3-256 `b3:<64 lowercase hex>` framing;
7. contract-tree walking, generated-source digests, drift detection, and profiles.

No repository-owned JSON escaping algorithm, UTF-16 comparator, or float formatter is
justified after adoption.

## Probes and evidence

- `cargo info serde_json_canonicalizer@0.3.2` resolved the exact crate and source.
  Source inspection confirmed UTF-16 ordering, `ryu-js`, explicit non-finite rejection,
  and RFC Appendix B number tests. The implementation and API are documented at
  [docs.rs](https://docs.rs/serde_json_canonicalizer/0.3.2/serde_json_canonicalizer/).
- `serde_json` 1.0.151 documents integer/floating discrimination and finite
  `Number::from_f64` behavior in its
  [Number API](https://docs.rs/serde_json/1.0.151/serde_json/struct.Number.html).
- An isolated CPython 3.14.7 probe imported `rfc8785==0.1.4` and emitted
  `{"a":2,"b":1}` from reversed input. The project documents bytes-only RFC 8785
  output and typed domain errors in its
  [official repository](https://github.com/trailofbits/rfc8785.py).
- `cargo info serde_yaml_ng@0.10.0` resolved the exact YAML crate and its Rust 1.64
  floor.

WP06's shared fixture suite is the executable adoption gate: both languages must emit
the same bytes and `b3:` values for positive vectors and the expected typed failures
for duplicate keys, non-finite tokens, unsafe integers, padding, malformed decimal
strings, and uppercase IDs/digests.

## Upgrade posture

All selected versions are exact pins. Any serializer, decoder, or hash-library update
must replay the shared fixture corpus byte-for-byte and the negative corpus without
changing `codefabric-jcs-v1`; a byte change requires a new canonicalization profile,
not an in-place dependency bump.
