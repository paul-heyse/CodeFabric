# Canonicalization pack — mechanical reference

Companion to `SKILL.md`, which carries the narrative and the reading paths. This file is the
lookup layer: section maps, a symbol index, task routing, decision trees, and navigation rules.
References back to the narrative are written `SKILL §...`.

**Read this file's §5 before citing anything from the pack.** The citation scheme has two
namespaces because the documents do, and `just lib-outline` cannot navigate them.

## Document aliases

| Alias | File | Lines |
|---|---|---:|
| `pack` | `README_canonicalization_library_reference_pack.md` | 156 |
| `serde_json` | `serde_json_rust_advanced_reference_1.0.151.md` | 678 |
| `pyjson` | `python_stdlib_json_3.14.7_advanced_reference.md` | 548 |
| `sjc` | `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md` | 539 |
| `rfc8785` | `rfc8785_python_advanced_reference_0.1.4.md` | 442 |
| `blake3-rust` | `blake3_rust_advanced_reference_1.8.6.md` | 444 |
| `blake3-py` | `blake3_python_advanced_reference_1.0.9.md` | 422 |
| `base64` | `base64_rust_advanced_reference_0.22.1.md` | 510 |
| `serde_yaml_ng` | `serde_yaml_ng_rust_advanced_reference_0.10.0.md` | 504 |
| `rejected` | `rejected_canonical_json_alternatives_serde_jcs_orjson.md` | 87 |

All ten live in `docs/library_ref/`. Total: 4,330 lines.

---

## 1. Section maps

Two namespaces per document. **Part 1** sections are unnumbered — cite them by title. **Part 2**
chapters are `§N` — cite them with the alias, never bare. `Span` is the chapter's line count.

### `pack` — `README_canonicalization_library_reference_pack.md`
Pack index. No Part 2 — read whole (156 lines).  
**156 lines.** Flat structure, unnumbered `##` sections only.

| Line | Part 1 section (cite by title) |
|---:|---|
| 6 | Adopted dependency map |
| 19 | CodeFabric responsibility split |
| 33 | Recommended agent reading order |
| 42 | Cross-language invariant |
| 53 | Included documents |
| 64 | Upgrade and compatibility policy |
| 73 | Reference-pack architecture |
| 117 | Suggested reading bundles by task |
| 142 | Dependency-change decision tree |

### `serde_json` — `serde_json_rust_advanced_reference_1.0.151.md`
Strict Rust JSON ingress. Longest document in the pack.  
**678 lines.** Part 1 = lines 1-246; divider `# Extended capability catalog` at line 247; Part 2 = §1-§22.

| Line | Part 1 section (cite by title) |
|---:|---|
| 5 | Version / source anchors |
| 17 | Capability inventory |
| 28 | Cargo features in 1.0.151 |
| 49 | Typed JSON APIs |
| 78 | `Value`, `Map`, and `json!` |
| 91 | `Number` and the `arbitrary_precision` feature |
| 108 | Custom `Deserializer` and Visitor path |
| 128 | Streaming and multiple values |
| 132 | Recursion limits |
| 138 | `RawValue` (`raw_value` feature) |
| 150 | Serialization model and Serde annotations |
| 163 | Error model |
| 178 | JSON compliance details relevant to strict systems |
| 188 | Performance guidance |
| 198 | Security and robustness |
| 204 | Canonicalization-specific anti-patterns |
| 215 | Agent checklist |
| 224 | Testing matrix for agent-authored changes |
| 238 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 249 | 24 | Data model, ownership, and where `serde_json` sits in a protocol stack |
| §2 | 273 | 43 | Deserialization entry points and lifetime behavior |
| §3 | 316 | 28 | `Deserializer`: custom ingress and parser control |
| §4 | 344 | 22 | `StreamDeserializer`: sequential JSON values |
| §5 | 366 | 54 | `Value`: complete dynamic navigation mental model |
| §6 | 420 | 14 | `Map<String, Value>` and object-order policy |
| §7 | 434 | 26 | `Number`: representability, inspection, and lexical provenance |
| §8 | 460 | 19 | `RawValue`: exact sub-document retention |
| §9 | 479 | 20 | Serialization entry points |
| §10 | 499 | 13 | `Serializer` and `Formatter` |
| §11 | 512 | 19 | `json!` macro |
| §12 | 531 | 26 | Error model and diagnostics |
| §13 | 557 | 24 | Strict duplicate-key detection with a Visitor |
| §14 | 581 | 14 | Serde derive policy as wire-schema policy |
| §15 | 595 | 6 | Object keys and non-string maps |
| §16 | 601 | 16 | Resource limits and denial-of-service posture |
| §17 | 617 | 6 | `no_std` / `alloc` posture |
| §18 | 623 | 13 | Performance decision table |
| §19 | 636 | 6 | Concurrency and state ownership |
| §20 | 642 | 16 | Fuzzing and property-based testing |
| §21 | 658 | 10 | Deployment advisory |
| §22 | 668 | 11 | Agent execution playbook |

### `pyjson` — `python_stdlib_json_3.14.7_advanced_reference.md`
Strict Python JSON ingress. Contains a complete reference implementation.  
**548 lines.** Part 1 = lines 1-247; divider `# Extended capability catalog` at line 248; Part 2 = §1-§27.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 13 | CodeFabric role |
| 19 | Top-level API inventory |
| 38 | `json.loads` hooks |
| 108 | Recommended strict loader |
| 158 | `object_hook` vs `object_pairs_hook` |
| 164 | Decoder compliance caveats |
| 176 | Input encodings |
| 180 | `JSONDecodeError` |
| 186 | `JSONDecoder.raw_decode` |
| 190 | Encoder surface and why it is not JCS |
| 200 | Resource limits |
| 206 | Security and correctness anti-patterns |
| 217 | Agent checklist |
| 225 | Testing matrix for agent-authored changes |
| 239 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 250 | 12 | Decoder/encoder architecture |
| §2 | 262 | 11 | `loads` and `load` |
| §3 | 273 | 16 | `object_pairs_hook` |
| §4 | 289 | 16 | `parse_int` |
| §5 | 305 | 12 | `parse_float` |
| §6 | 317 | 11 | `parse_constant` |
| §7 | 328 | 10 | Repeated member names: default behavior |
| §8 | 338 | 9 | `object_hook` |
| §9 | 347 | 15 | `JSONDecoder` |
| §10 | 362 | 6 | `raw_decode` |
| §11 | 368 | 20 | `dump` / `dumps` |
| §12 | 388 | 6 | `allow_nan` |
| §13 | 394 | 8 | `JSONEncoder` |
| §14 | 402 | 6 | `ensure_ascii` |
| §15 | 408 | 6 | Whitespace controls |
| §16 | 414 | 10 | Key coercion and `skipkeys` |
| §17 | 424 | 6 | Circular references |
| §18 | 430 | 6 | Input encoding behavior |
| §19 | 436 | 6 | `JSONDecodeError` |
| §20 | 442 | 41 | Strict loader reference implementation |
| §21 | 483 | 4 | Decoder extension policy |
| §22 | 487 | 16 | Resource limits |
| §23 | 503 | 4 | CLI tooling |
| §24 | 507 | 6 | Determinism and dict ordering |
| §25 | 513 | 15 | Testing matrix |
| §26 | 528 | 10 | Deployment advisory |
| §27 | 538 | 11 | Agent execution playbook |

### `sjc` — `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md`
Rust RFC 8785 serializer. **All four signatures are Part 1 only.**  
**539 lines.** Part 1 = lines 1-250; divider `# Extended capability catalog` at line 251; Part 2 = §1-§23.

| Line | Part 1 section (cite by title) |
|---:|---|
| 5 | Version / source anchors |
| 27 | Feature inventory |
| 43 | Installation and pinning |
| 54 | Public API surface |
| 102 | RFC 8785 mechanics that this crate should own |
| 122 | Serde data-model implications |
| 135 | CodeFabric canonicalization boundary |
| 152 | Correct patterns |
| 179 | Error model |
| 191 | Performance and allocation |
| 201 | Security / protocol guidance |
| 207 | Common failure modes |
| 217 | Agent checklist |
| 228 | Testing matrix for agent-authored changes |
| 242 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 253 | 12 | Design philosophy and trust boundary |
| §2 | 265 | 15 | `to_vec` as the default protocol API |
| §3 | 280 | 10 | `to_string` |
| §4 | 290 | 11 | `to_writer` |
| §5 | 301 | 11 | `pipe` |
| §6 | 312 | 12 | Object sorting mechanics |
| §7 | 324 | 13 | Number serialization mechanics |
| §8 | 337 | 13 | `arbitrary_precision` interaction |
| §9 | 350 | 12 | String serialization mechanics |
| §10 | 362 | 6 | Serde map keys |
| §11 | 368 | 11 | Arrays are never sorted by JCS |
| §12 | 379 | 13 | Serde custom serializer hazards |
| §13 | 392 | 4 | Writer failures |
| §14 | 396 | 17 | Deterministic typed-model pattern |
| §15 | 413 | 11 | Dynamic `Value` pattern |
| §16 | 424 | 17 | Hashing without accidental text transformations |
| §17 | 441 | 16 | Differential conformance harness |
| §18 | 457 | 17 | RFC test vectors and generated properties |
| §19 | 474 | 15 | Security posture |
| §20 | 489 | 11 | Performance profile |
| §21 | 500 | 18 | Upgrade investigation checklist |
| §22 | 518 | 10 | Deployment matrix |
| §23 | 528 | 12 | Agent execution playbook |

### `rfc8785` — `rfc8785_python_advanced_reference_0.1.4.md`
Python RFC 8785 serializer. Holds the three-assertion parity contract (§20).  
**442 lines.** Part 1 = lines 1-195; divider `# Extended capability catalog` at line 196; Part 2 = §1-§23.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 14 | CodeFabric role |
| 18 | Capability inventory |
| 30 | Installation |
| 40 | `dumps` |
| 56 | `dump` |
| 65 | Accepted Python data model |
| 79 | Number semantics |
| 87 | String and key semantics |
| 93 | Error model |
| 106 | Why stdlib `json` is still required |
| 124 | CodeFabric pipeline |
| 138 | Cross-language parity |
| 144 | Performance guidance |
| 154 | Anti-patterns |
| 164 | Agent checklist |
| 173 | Testing matrix for agent-authored changes |
| 187 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 198 | 6 | Why this package exists in the stack |
| §2 | 204 | 19 | Public serialization surfaces |
| §3 | 223 | 16 | Input domain |
| §4 | 239 | 10 | Integer-domain failures |
| §5 | 249 | 6 | Float-domain failures |
| §6 | 255 | 6 | Strings and UTF-8 |
| §7 | 261 | 14 | Object member ordering |
| §8 | 275 | 6 | Number rendering |
| §9 | 281 | 6 | String escaping |
| §10 | 287 | 15 | Error hierarchy |
| §11 | 302 | 21 | Why `rfc8785` does not replace `json.loads` |
| §12 | 323 | 12 | Direct bytes-to-checksum pipeline |
| §13 | 335 | 6 | No pretty-print mode by design |
| §14 | 341 | 6 | Schema-adapter boundary |
| §15 | 347 | 6 | `codefabric-bytes` |
| §16 | 353 | 6 | Non-string-keyed logical maps |
| §17 | 359 | 12 | Determinism boundary |
| §18 | 371 | 12 | Performance guidance |
| §19 | 383 | 11 | Error handling example |
| §20 | 394 | 15 | Differential testing against Rust |
| §21 | 409 | 13 | RFC appendix/edge vectors to retain |
| §22 | 422 | 10 | Deployment matrix |
| §23 | 432 | 11 | Agent execution playbook |

### `blake3-rust` — `blake3_rust_advanced_reference_1.8.6.md`
Rust digest. The one document that deliberately does not pin its own version.  
**444 lines.** Part 1 = lines 1-196; divider `# Extended capability catalog` at line 197; Part 2 = §1-§23.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 16 | CodeFabric role |
| 26 | Core API inventory |
| 43 | Cargo features |
| 58 | Ordinary hashing |
| 68 | Incremental hashing |
| 81 | Keyed hashing and key derivation |
| 93 | Extendable output (XOF) |
| 99 | Reader, mmap, and parallel paths |
| 105 | CPU feature selection and portability |
| 111 | `Hash` representation |
| 129 | `hazmat` module |
| 133 | Security semantics |
| 139 | Error model |
| 145 | Performance guidance |
| 154 | Canonicalization integration anti-patterns |
| 165 | Agent checklist |
| 174 | Testing matrix for agent-authored changes |
| 188 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 199 | 11 | BLAKE3 mental model |
| §2 | 210 | 11 | Constants and protocol-relevant sizes |
| §3 | 221 | 9 | One-shot ordinary hashing |
| §4 | 230 | 21 | Incremental `Hasher` |
| §5 | 251 | 6 | Reader and large-input paths |
| §6 | 257 | 6 | Parallel hashing with `rayon` |
| §7 | 263 | 9 | Memory-mapped input |
| §8 | 272 | 24 | `Hash` type |
| §9 | 296 | 17 | Keyed hashing |
| §10 | 313 | 6 | Key derivation mode |
| §11 | 319 | 8 | Extendable output (XOF) |
| §12 | 327 | 6 | CPU dispatch and portability |
| §13 | 333 | 12 | Cargo feature policy |
| §14 | 345 | 4 | `serde` feature |
| §15 | 349 | 4 | `zeroize` feature and secret state |
| §16 | 353 | 12 | Error surface |
| §17 | 365 | 6 | Constant-time considerations |
| §18 | 371 | 12 | Correct CodeFabric pipeline |
| §19 | 383 | 15 | Verification pattern |
| §20 | 398 | 10 | Performance decision table |
| §21 | 408 | 17 | Testing matrix |
| §22 | 425 | 10 | Deployment advisory |
| §23 | 435 | 10 | Agent execution rules |

### `blake3-py` — `blake3_python_advanced_reference_1.0.9.md`
Python digest. Holds the pair's only dedicated parity chapter (§18).  
**422 lines.** Part 1 = lines 1-208; divider `# Extended capability catalog` at line 209; Part 2 = §1-§21.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 14 | CodeFabric role |
| 25 | Capability inventory |
| 39 | Installation |
| 49 | One-shot hashing |
| 59 | Incremental hashing |
| 70 | Copying state |
| 80 | XOF / seekable output |
| 89 | Keyed mode |
| 97 | Key derivation mode |
| 108 | Multithreading |
| 119 | Memory-mapped files |
| 129 | Packaging / CPython considerations |
| 135 | Digest framing |
| 147 | Error / misuse boundaries |
| 153 | Security semantics |
| 159 | Performance guidance |
| 168 | Canonicalization anti-patterns |
| 178 | Agent checklist |
| 186 | Testing matrix for agent-authored changes |
| 200 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 211 | 13 | Binding model |
| §2 | 224 | 11 | Constructor and initial data |
| §3 | 235 | 13 | Incremental `update` |
| §4 | 248 | 13 | `digest` / `hexdigest` |
| §5 | 261 | 14 | XOF and `seek` |
| §6 | 275 | 12 | Copying state |
| §7 | 287 | 11 | Keyed mode |
| §8 | 298 | 6 | Key derivation mode |
| §9 | 304 | 6 | `max_threads` |
| §10 | 310 | 6 | `update_mmap` |
| §11 | 316 | 6 | Packaging and CPython support |
| §12 | 322 | 6 | Free-threaded / interpreter considerations |
| §13 | 328 | 6 | GIL and concurrency planning |
| §14 | 334 | 11 | CodeFabric checksum helper |
| §15 | 345 | 17 | Verification helper |
| §16 | 362 | 6 | Failure boundaries |
| §17 | 368 | 11 | Performance decision table |
| §18 | 379 | 11 | Cross-language parity |
| §19 | 390 | 9 | Security and secret-data rules |
| §20 | 399 | 14 | Testing matrix |
| §21 | 413 | 10 | Agent execution playbook |

### `base64` — `base64_rust_advanced_reference_0.22.1.md`
`codefabric-bytes` lexical form. `URL_SAFE_NO_PAD` only.  
**510 lines.** Part 1 = lines 1-215; divider `# Extended capability catalog` at line 216; Part 2 = §1-§21.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 14 | CodeFabric role |
| 24 | Mental model: `Engine` |
| 42 | Preconfigured general-purpose engines |
| 51 | Alphabet layer |
| 57 | Encoding APIs |
| 71 | Decoding APIs |
| 81 | Streaming I/O |
| 87 | `Base64Display` |
| 91 | Padding semantics |
| 103 | Trailing bits and canonical encodings |
| 118 | Custom engine configuration |
| 128 | Error types |
| 134 | Memory, overflow, and panics |
| 138 | no-std / features |
| 150 | Deprecated top-level API |
| 154 | CodeFabric canonical patterns |
| 175 | Anti-patterns |
| 185 | Agent checklist |
| 193 | Testing matrix for agent-authored changes |
| 207 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 218 | 17 | `Engine` architecture |
| §2 | 235 | 11 | Preconfigured engines as protocol choices |
| §3 | 246 | 22 | Encoding methods |
| §4 | 268 | 22 | Decoding methods |
| §5 | 290 | 6 | Output-size planning |
| §6 | 296 | 12 | Alphabet definitions |
| §7 | 308 | 12 | `GeneralPurposeConfig` |
| §8 | 320 | 22 | Padding policy |
| §9 | 342 | 13 | Trailing-bit policy |
| §10 | 355 | 14 | `DecodeError` and slice-specific errors |
| §11 | 369 | 12 | Streaming read/write adapters |
| §12 | 381 | 6 | `Base64Display` |
| §13 | 387 | 14 | `std`, `alloc`, and constrained targets |
| §14 | 401 | 15 | Buffer reuse and throughput |
| §15 | 416 | 14 | Decoder leniency belongs at adapters, not canonical storage |
| §16 | 430 | 23 | CodeFabric format validator |
| §17 | 453 | 16 | Property tests |
| §18 | 469 | 6 | Security posture |
| §19 | 475 | 16 | Migration from old APIs |
| §20 | 491 | 10 | Deployment matrix |
| §21 | 501 | 10 | Agent execution playbook |

### `serde_yaml_ng` — `serde_yaml_ng_rust_advanced_reference_0.10.0.md`
YAML 1.1 registry ingestion. Most chapters in the pack (§1-§27).  
**504 lines.** Part 1 = lines 1-201; divider `# Extended capability catalog` at line 202; Part 2 = §1-§27.

| Line | Part 1 section (cite by title) |
|---:|---|
| 3 | Version / source anchors |
| 16 | CodeFabric role |
| 22 | Public API inventory |
| 50 | Installation |
| 60 | Typed deserialization |
| 73 | Dynamic `Value` |
| 79 | `Mapping` |
| 85 | YAML 1.1 semantics |
| 91 | Enums and tags |
| 97 | Multi-document streams |
| 101 | Anchors, aliases, merges, tags, and application semantics |
| 107 | Numbers |
| 113 | Strings and Unicode |
| 117 | Error handling |
| 129 | Security / resource policy |
| 135 | Conversion into canonical JSON |
| 154 | Serialization back to YAML |
| 158 | Anti-patterns |
| 169 | Agent checklist |
| 179 | Testing matrix for agent-authored changes |
| 193 | Upgrade and compatibility policy |

| § | Line | Span | Chapter title |
|---|---:|---:|---|
| §1 | 204 | 16 | Role of YAML in the canonicalization architecture |
| §2 | 220 | 8 | Specification version: YAML 1.1 |
| §3 | 228 | 13 | Deserialization entry points |
| §4 | 241 | 10 | Serialization entry points |
| §5 | 251 | 15 | `Deserializer` and multiple YAML documents |
| §6 | 266 | 16 | `Value` data model |
| §7 | 282 | 10 | `Mapping` |
| §8 | 292 | 12 | `Number` |
| §9 | 304 | 14 | Tagged values and Rust enums |
| §10 | 318 | 6 | Enum compatibility helpers |
| §11 | 324 | 8 | Anchors and aliases |
| §12 | 332 | 6 | Merge-key semantics |
| §13 | 338 | 6 | Comments and presentation information |
| §14 | 344 | 6 | Strings and Unicode |
| §15 | 350 | 13 | YAML booleans/nulls and scalar surprises |
| §16 | 363 | 13 | Dynamic-to-typed conversion |
| §17 | 376 | 17 | Error model |
| §18 | 393 | 6 | Error-path enrichment |
| §19 | 399 | 11 | Serialization policy |
| §20 | 410 | 13 | Serde model attributes |
| §21 | 423 | 17 | YAML-to-JSON projection checklist |
| §22 | 440 | 6 | Duplicate-key policy |
| §23 | 446 | 14 | Resource limits |
| §24 | 460 | 4 | Threading and concurrency |
| §25 | 464 | 19 | Testing matrix |
| §26 | 483 | 10 | Deployment advisory |
| §27 | 493 | 12 | Agent execution playbook |

### `rejected` — `rejected_canonical_json_alternatives_serde_jcs_orjson.md`
Substitution guardrail. No Part 2 — read whole (87 lines).  
**87 lines.** Flat structure, unnumbered `##` sections only.

| Line | Part 1 section (cite by title) |
|---:|---|
| 5 | Decision context |
| 9 | `serde_jcs 0.2.0` — do not use |
| 23 | `orjson` — useful adapter, not JCS |
| 35 | Decision test for agents |
| 53 | Agent-facing substitution guardrail |
| 80 | Appropriate uses that remain available |


---

## 2. Symbol → location

Rows marked **P1-only** appear *only* in that document's Part 1; a `§N`-scoped search will not
find them. This is the single most common way to conclude wrongly that the pack does not document
something.

### 2.1 Rust — ingress (`serde_json`)

| Symbol | Location |
|---|---|
| `from_str` / `from_slice` / `from_reader` / `from_value` | `serde_json` §2 |
| `Deserializer`, `end()`, recursion control | `serde_json` §3 |
| `StreamDeserializer` | `serde_json` §4 |
| `Value`, type predicates, `get`/`get_mut`, JSON Pointer, `take` | `serde_json` §5 |
| `Map<String, Value>`, `preserve_order` | `serde_json` §6 |
| `Number`, `is_i64`/`as_i64`/`as_f64`/`from_f64`, `arbitrary_precision` | `serde_json` §7 |
| `RawValue` (`raw_value` feature) | `serde_json` §8 |
| `to_vec` / `to_string` / `to_writer` (ordinary JSON only) | `serde_json` §9 |
| `Serializer`, `Formatter` | `serde_json` §10 |
| `json!` | `serde_json` §11 |
| `serde_json::Error`, classification, failure taxonomy | `serde_json` §12 |
| `MapAccess` / `Visitor` duplicate detection | `serde_json` §13 |
| `rename`, `flatten`, `skip_serializing_if`, `tag`/`content`/`untagged`, `serialize_with` | `serde_json` §14 |
| `no_std` / `alloc` posture | `serde_json` §17 |
| Cargo feature list for 1.0.151 | `serde_json` *Cargo features in 1.0.151* — **P1-only** |

### 2.2 Python — ingress (`pyjson`)

| Symbol | Location |
|---|---|
| `json.loads` / `json.load` | `pyjson` §2 |
| `object_pairs_hook` | `pyjson` §3 |
| `parse_int` | `pyjson` §4 |
| `parse_float` | `pyjson` §5 |
| `parse_constant` | `pyjson` §6 |
| repeated-name default (last wins) | `pyjson` §7 |
| `object_hook` and why it cannot substitute | `pyjson` §8 |
| `JSONDecoder` | `pyjson` §9 |
| `raw_decode` | `pyjson` §10 |
| `dump` / `dumps`, `sort_keys` | `pyjson` §11 |
| `allow_nan` | `pyjson` §12 |
| `JSONEncoder` | `pyjson` §13 |
| `ensure_ascii` | `pyjson` §14 |
| `skipkeys`, key coercion | `pyjson` §16 |
| `JSONDecodeError` | `pyjson` §19 |
| strict-loader reference implementation | `pyjson` §20; short form in *Recommended strict loader* |

### 2.3 Rust — canonical bytes (`sjc`)

| Symbol | Location |
|---|---|
| `to_vec` **signature** | `sjc` *Public API surface* — **P1-only**; usage at `§2` |
| `to_string` **signature** | `sjc` *Public API surface* — **P1-only**; usage at `§3` |
| `to_writer` **signature** | `sjc` *Public API surface* — **P1-only**; usage at `§4` |
| `pipe` **signature** | `sjc` *Public API surface* — **P1-only**; usage and hazard at `§5` |
| object sorting mechanics | `sjc` §6 |
| number rendering, `ryu-js` | `sjc` §7; named in *RFC 8785 mechanics that this crate should own* |
| `arbitrary_precision` interaction | `sjc` §8 |
| string escaping | `sjc` §9 |
| Serde map keys | `sjc` §10 |
| arrays are never sorted | `sjc` §11 |
| custom `Serialize` / `serialize_with` hazards | `sjc` §12 |
| writer failures | `sjc` §13 |
| `float_roundtrip` (transitive) | `sjc` *Installation and pinning* — **P1-only** |
| non-finite float rejection | `sjc` *RFC 8785 mechanics that this crate should own* — **P1-only** |

### 2.4 Python — canonical bytes (`rfc8785`)

| Symbol | Location |
|---|---|
| `rfc8785.dumps` | `rfc8785` §2; also *`dumps`* in Part 1 |
| `rfc8785.dump` | `rfc8785` §2; also *`dump`* in Part 1 |
| accepted input domain | `rfc8785` §3 |
| integer-domain failures | `rfc8785` §4 |
| float-domain failures | `rfc8785` §5 |
| `CanonicalizationError` and subclasses | `rfc8785` §10; short form in *Error model* |
| member ordering (UTF-16) | `rfc8785` §7 |
| number rendering | `rfc8785` §8 |
| string escaping | `rfc8785` §9 |
| `codefabric-bytes` (Python side) | `rfc8785` §15 |
| non-string-keyed logical maps | `rfc8785` §16 |
| no pretty-print mode | `rfc8785` §13 |
| schema-adapter boundary, no generic `default=` | `rfc8785` §14 |

### 2.5 Digest (`blake3-rust` / `blake3-py`)

| Symbol | Rust | Python |
|---|---|---|
| one-shot hash | `blake3::hash` — `blake3-rust` §3 | `blake3(data)` — `blake3-py` §2 |
| incremental | `Hasher::new`/`update`/`finalize` — `blake3-rust` §4 | `update` — `blake3-py` §3 |
| digest output | `Hash` — `blake3-rust` §8 | `digest`/`hexdigest` — `blake3-py` §4 |
| keyed mode | `keyed_hash`, `Hasher::new_keyed` — `blake3-rust` §9 | `key=` — `blake3-py` §7 |
| key derivation | `derive_key` — `blake3-rust` §10 | `derive_key_context=` — `blake3-py` §8 |
| XOF | `finalize_xof`, `OutputReader` — `blake3-rust` §11 | `digest(length=…, seek=…)` — `blake3-py` §5 |
| parallelism | `rayon` feature — `blake3-rust` §6, `§13` | `max_threads=` — `blake3-py` §9 |
| mmap | `mmap` feature — `blake3-rust` §7 | `update_mmap` — `blake3-py` §10 |
| state copy | *(passing mention only)* | `copy()` — `blake3-py` §6 |
| constants `OUT_LEN`/`KEY_LEN`/`BLOCK_LEN`/`CHUNK_LEN` | `blake3-rust` §2 | *(not documented)* |
| `b3:` framing helper | `blake3-rust` §18 | `blake3-py` §14 |
| verification / strict parse | `blake3-rust` §19 | `blake3-py` §15 |
| cross-language parity | distributed: `blake3-rust` §21, checklist | `blake3-py` §18 (dedicated) |
| `hazmat`, `Hash::from_hex`, `HexError` | `blake3-rust` Part 1 — **P1-only** | — |
| `update_rayon`, `update_mmap_rayon` | `blake3-rust` *Reader, mmap, and parallel paths* / *Cargo features* — **P1-only** | — |
| `blake3.AUTO`, ~1 MB threshold, 3.13t wheel note | — | `blake3-py` Part 1 — **P1-only** |
| GIL / free-threaded considerations | — | `blake3-py` §12, `§13` |
| `serde` / `zeroize` features | `blake3-rust` §14, `§15` | — |

### 2.6 Bytes lexical form (`base64`)

| Symbol | Location |
|---|---|
| `Engine` trait | `base64` §1 |
| `URL_SAFE_NO_PAD`, `STANDARD`, `STANDARD_NO_PAD`, `URL_SAFE` | `base64` §2 |
| `encode` / `encode_string` / `encode_slice` | `base64` §3 |
| `decode` / `decode_vec` / `decode_slice` | `base64` §4 |
| `Alphabet`, `Alphabet::new` | `base64` §6 |
| `GeneralPurposeConfig` | `base64` §7 |
| padding policy | `base64` §8 |
| trailing-bit policy | `base64` §9 |
| `DecodeError` | `base64` §10 |
| `DecoderReader` / `EncoderWriter` | `base64` §11 |
| `Base64Display` | `base64` §12 |
| `std` / `alloc` features | `base64` §13 |
| `codefabric-bytes` validator | `base64` §16 |
| deprecated top-level API | `base64` §19 |
| `encoded_len`, `decoded_len_estimate` | `base64` *Encoding APIs* / *Decoding APIs* — **P1-only** |
| `GeneralPurpose::new` | `base64` *Custom engine configuration* — **P1-only** |
| `with_decode_allow_trailing_bits` | `base64` *Trailing bits and canonical encodings* — **P1-only** |
| `DecodeSliceError`, `EncodeSliceError` | `base64` *Error types* — **P1-only** |
| MSRV, `usize`-overflow panic note | `base64` *Version / source anchors*, *Memory, overflow, and panics* — **P1-only** |

### 2.7 YAML registry (`serde_yaml_ng`)

| Symbol | Location |
|---|---|
| `from_str` / `from_slice` / `from_reader` / `from_value` | `serde_yaml_ng` §3 |
| `to_string` / `to_writer` / `to_value` | `serde_yaml_ng` §4 |
| `Deserializer`, multi-document streams | `serde_yaml_ng` §5 |
| `Value` | `serde_yaml_ng` §6 |
| `Mapping`, ordering | `serde_yaml_ng` §7 |
| `Number` | `serde_yaml_ng` §8 |
| tags, `!Tag` | `serde_yaml_ng` §9 |
| `singleton_map`, `#[serde(with = …)]` | `serde_yaml_ng` §10 |
| anchors `&name` / aliases `*name` | `serde_yaml_ng` §11 |
| merge key `<<:` | `serde_yaml_ng` §12 |
| YAML 1.1 scalar surprises | `serde_yaml_ng` §15 |
| dynamic → typed conversion | `serde_yaml_ng` §16 |
| error taxonomy | `serde_yaml_ng` §17, `§18` |
| Serde model attributes | `serde_yaml_ng` §20 |
| YAML-to-JSON projection checklist | `serde_yaml_ng` §21 |
| duplicate-key policy *(explicitly open)* | `serde_yaml_ng` §22 |
| resource limits, alias expansion | `serde_yaml_ng` §23 |
| full API inventory, `Sequence`, `Index`, `with` module | `serde_yaml_ng` *Public API inventory* — **P1-only** |
| MSRV, YAML 1.1 scope, `unsafe-libyaml` | `serde_yaml_ng` *Version / source anchors*, *Security / resource policy* — **P1-only** |

### 2.8 Shared contract vocabulary

| Term | Where defined |
|---|---|
| `codefabric-jcs-v1` | `pack` *Adopted dependency map*, *Upgrade and compatibility policy* |
| the seven repository-owned semantics | `pack` *CodeFabric responsibility split* |
| safe integer bound (literal) | `serde_json` *`Number` and the `arbitrary_precision` feature* + `§7`; `pyjson` *Recommended strict loader* + `§20`; `pack` *CodeFabric responsibility split* |
| `codefabric-int64` / `codefabric-uint64` | `serde_yaml_ng` §8; `pack` *CodeFabric responsibility split* |
| `codefabric-bytes` | `rfc8785` §15; `base64` §16; `serde_yaml_ng` §21 |
| `b3:<64 lowercase hex>` | `blake3-rust` §8, `§18`; `blake3-py` *Digest framing*, `§14` |
| cross-language invariant | `pack` *Cross-language invariant*; `rfc8785` §20; `sjc` §17 |

---

## 3. Task → location

| Goal | Read |
|---|---|
| reject duplicate members (Rust) | `serde_json` §13 |
| reject duplicate members (Python) | `pyjson` §3, `§7`, `§8` |
| reject unsafe integer tokens | `serde_json` §7; `pyjson` §4; `sjc` §8; `rfc8785` §4 |
| reject non-finite floats | `pyjson` §6, `§12`; `sjc` *RFC 8785 mechanics…*; `rfc8785` §5 |
| produce canonical bytes (Rust) | `sjc` *Public API surface* → `§2` |
| produce canonical bytes (Python) | `rfc8785` §2 |
| understand key ordering | `sjc` §6; `rfc8785` §7 |
| understand number rendering | `sjc` §7; `rfc8785` §8 |
| compute a `b3:` digest | `blake3-rust` §18; `blake3-py` §14 |
| verify a `b3:` digest | `blake3-rust` §19; `blake3-py` §15 |
| validate `codefabric-bytes` | `base64` §16; `rfc8785` §15 |
| project YAML to JSON | `serde_yaml_ng` *Conversion into canonical JSON* → `§21` |
| write cross-language fixtures | `sjc` §17, `§18`; `rfc8785` §20, `§21` |
| bound resource use | `serde_json` §16; `pyjson` §22; `serde_yaml_ng` §23; `base64` *Memory, overflow, and panics* |
| evaluate a serializer substitution | `rejected` (whole) |
| evaluate a version bump | `pack` *Dependency-change decision tree*; `sjc` §21 |
| find a signature | Part 1 API sections — never `§N` (see §5 rule 3) |

---

## 4. Decision trees

### 4.1 Which document?

```text
What is the input?
  JSON text/bytes, Rust    -> serde_json
  JSON text/bytes, Python  -> pyjson
  YAML registry file       -> serde_yaml_ng
  an already-validated value -> sjc (Rust) | rfc8785 (Python)
  canonical bytes          -> blake3-rust | blake3-py
  a bytes field's text form -> base64
  a proposal to change a dependency -> rejected, then pack
```

### 4.2 Is this the canonicalizer's job or mine?

```text
Does the rule concern object member ORDER, string ESCAPING, or float FORMATTING?
  yes -> the library owns it. Do not implement it.        (pack, sjc §23)
Does it concern WHICH VALUES ARE ACCEPTED, duplicates, or typed formats?
  yes -> repository code owns it, BEFORE the serializer.  (pack responsibility split)
Does it concern array ORDER?
  yes -> the semantic model owns it. JCS never sorts arrays. (sjc §11, rfc8785 §16)
```

### 4.3 Dependency change (the pack's own tree)

From `pack` *Dependency-change decision tree*:

```text
Does the dependency participate before/at canonical bytes?
  no  -> normal compatibility review may be sufficient
  yes -> replay the shared fixture corpus

Do any previously accepted values change canonical bytes?
  no  -> version bump may proceed after negative-corpus parity
  yes -> do NOT mutate codefabric-jcs-v1 in place; evaluate a new profile

Does a decoder begin accepting a previously rejected ambiguous form?
  yes -> determine whether repository strict validation still rejects it;
         if not, treat as a contract regression
```

### 4.4 Cross-language byte divergence

```text
Did BOTH sides accept the value?
  no  -> ingress or validation. rfc8785 §10 lanes; serde_json §12; pyjson §19
  yes -> continue

Do the canonical BYTES differ?
  yes -> numbers?    sjc §7  + rfc8785 §8
         key order?  sjc §6  + rfc8785 §7
         escaping?   sjc §9  + rfc8785 §9
  no  -> the digest or the framing differs: blake3-rust §8/§19, blake3-py §15/§18
```

### 4.5 Digest mode

```text
Is this a codefabric-jcs-v1 fingerprint?
  yes -> unkeyed, exactly 32 bytes, no seek. Anything else is a protocol change.
         blake3-rust §23 | blake3-py §21
  no  -> keyed: blake3-rust §9  | blake3-py §7
         KDF:   blake3-rust §10 | blake3-py §8
         XOF:   blake3-rust §11 | blake3-py §5
```

---

## 5. Navigation rules

1. **`just lib-outline` is useless on this pack.** It is h1-rooted; these chapters are `##`. It
   emits two headings per document. Use §1 above.

2. **Never write a `§N` whose document is not established.** Eight documents number from `§1`
   and the numbers collide completely. Qualify the first citation in a paragraph as
   `` `<alias>` §N ``; within a run of references to the same document a bare `§N` is fine, but
   any switch of document re-qualifies.

3. **Signatures are in Part 1, not in `§N`.** `sjc`'s four function signatures exist only in
   *Public API surface*; `§2`-`§5` describe usage. The same pattern holds for `base64`'s sizing
   helpers and slice error types, `blake3-rust`'s `hazmat`/`from_hex`/`HexError`, and
   `serde_yaml_ng`'s full API inventory. §2 above marks every such row **P1-only**.

4. **Part 1 has no numbers.** Cite by heading title, exactly as §1 lists it.

5. **There is no `§0` and no `§N.M`.** Members inside a chapter are unnumbered `###` headings;
   there are only a handful in the whole pack.

6. **The trailing two Part 1 sections are one shared block.** *Testing matrix for agent-authored
   changes* and *Upgrade and compatibility policy* are byte-identical across all eight numbered
   documents. Cite it once, as a pack-level policy. Prefer the document-specific matrices where
   they exist: `pyjson` §25, `blake3-rust` §21, `blake3-py` §20, `base64` §17, `serde_yaml_ng` §25,
   `sjc` §17-§18, `rfc8785` §20-§21, `serde_json` *Testing matrix for agent-authored changes*.

7. **Any extractor must be fence-aware.** Fenced code in `blake3-py` and `rfc8785` contains `#`
   comments at column 0 that a naive `grep '^#'` reports as headings.

8. **Every numbered document ends with an Agent execution playbook**: `serde_json` §22,
   `pyjson` §27, `sjc` §23, `rfc8785` §23, `blake3-rust` §23, `blake3-py` §21, `base64` §21,
   `serde_yaml_ng` §27. Short fenced rule blocks; the natural last stop on any reading path.

9. **Versions come from manifests, not from these documents.** `blake3-rust` deliberately declines
   to pin (SKILL §"Version anchors"). Read `Cargo.toml`, `Cargo.lock`,
   `codefabric-cpg-mcp/pyproject.toml` and `codefabric-cpg-mcp/uv.lock`.

10. **One handoff leaves the pack.** `base64` §17 asks for parity against a Python URL-safe
    unpadded implementation that no pack document covers; `rfc8785` §15 is the only in-pack
    statement of that side.
