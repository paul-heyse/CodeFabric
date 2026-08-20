---
name: canonicalization-lib-ref
description: "Reference navigator for the ten-file **canonicalization pack** in `docs/library_ref/` — the libraries that turn a value into RFC 8785 canonical bytes and then into a `b3:` digest, identically in Rust and Python. Covers `README_canonicalization_library_reference_pack.md` (the adopted dependency map, the responsibility split, the cross-language invariant, reading bundles, the dependency-change decision tree), `serde_json_rust_advanced_reference_1.0.151.md` (strict Rust ingress: `from_str`/`from_slice`/`from_reader`, `Value`, `Map`, `Number` and `arbitrary_precision`, `RawValue`, Visitor-based duplicate detection, Serde derive as wire schema, recursion limits; §1-§22), `python_stdlib_json_3.14.7_advanced_reference.md` (strict Python ingress: `object_pairs_hook`, `parse_int`, `parse_float`, `parse_constant`, `JSONDecoder`, `raw_decode`, `JSONDecodeError`, the strict-loader reference implementation; §1-§27), `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md` (`to_vec`/`to_string`/`to_writer`/`pipe`, object sorting, `ryu-js` number rendering, string escaping, non-finite rejection, differential conformance; §1-§23), `rfc8785_python_advanced_reference_0.1.4.md` (`dumps`/`dump`, accepted data model, integer- and float-domain failures, `CanonicalizationError`, UTF-16 member ordering, differential testing against Rust; §1-§23), `blake3_rust_advanced_reference_1.8.6.md` and `blake3_python_advanced_reference_1.0.9.md` (`hash`/`keyed_hash`/`derive_key`, `Hasher`, `digest`/`hexdigest`, XOF and `seek`, `max_threads`, `update_mmap`, `b3:` framing; §1-§23 / §1-§21), `base64_rust_advanced_reference_0.22.1.md` (`Engine`, `URL_SAFE_NO_PAD`, padding and trailing-bit policy, `DecodeError`, the `codefabric-bytes` validator; §1-§21), `serde_yaml_ng_rust_advanced_reference_0.10.0.md` (YAML 1.1 registry ingestion, `Value`/`Mapping`/`Number`, tags, anchors, merge keys, the YAML-to-JSON projection checklist; §1-§27), and `rejected_canonical_json_alternatives_serde_jcs_orjson.md` (why `serde_jcs` and `orjson` are not the fingerprint serializer). SKILL.md is narrative: the four-stage pipeline, the Rust/Python document pairing, and an ordered reading path for each problem — strict ingress, canonical bytes, digest framing, the bytes lexical contract, YAML registry ingestion, dependency substitution, fixture corpora, and diagnosing a byte divergence. REFERENCE.md (same folder) is the mechanical layer: two-namespace section maps for all ten files, a symbol index, decision trees, and the navigation hazards. Use when code touches `serde_json_canonicalizer`/`to_vec`/`pipe`, `rfc8785.dumps`, `arbitrary_precision`, `object_pairs_hook`/`parse_int`/`parse_float`/`parse_constant`, `blake3`/`Hasher`/`hexdigest`/`b3:`, `base64`/`URL_SAFE_NO_PAD`/`codefabric-bytes`, `serde_yaml_ng`, `codefabric-jcs-v1`/`codefabric-int64`/`codefabric-uint64`, canonical JSON, JCS, RFC 8785, digests, fingerprints, duplicate-key rejection, or the safe-integer bound — or when any of those pins is bumped. orjson's own API → sibling `grpcio-orjson-protobuf-ref`. Arrow/Delta/DataFusion storage and query → siblings `deltalake-rust-ref`, `datafusion-pyarrow-rust-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# Canonicalization Reference Navigator

Routes the ten-file canonicalization pack in `docs/library_ref/` — the libraries that take a value
and produce **canonical bytes**, then a **digest**, and must do so **identically in Rust and
Python**.

This is not a library topic. It is a **byte-level protocol with two implementations**, and almost
every rule the pack states exists because the two implementations can drift. A change that looks
like an implementation detail on one side — a Serde attribute, a `default=` fallback, a sort call,
a float format — is a protocol change on both. The pack's own framing (`pack` *Cross-language
invariant*): any divergence is a protocol failure, not a formatting preference.

This SKILL.md is the **narrative**: what each stage owns, how the Rust and Python documents pair
up, and — for the problem you actually have — which sections to read and in what order. The
companion **`REFERENCE.md`** (same folder) is the **mechanical layer**: section maps for all ten
files, a symbol → location index, decision trees, and the navigation hazards. Reach for it once
you know which document you need. Cross-references back here are written `SKILL §...`.

**These are pure library navigators.** They index what the pack says, nothing more — including its
`codefabric-jcs-v1` sections, which are part of these documents. Nothing outside
`docs/library_ref/` is cited: no design specs, no reviews, no roadmap waves.

---

## The pipeline, and who owns which stage

The pack separates four stages **on purpose**, and its architecture note is explicit that agents
"should not collapse these stages merely because several libraries can parse or serialize
JSON-like values" (`pack` *Reference-pack architecture*).

```text
  1. SOURCE DECODING          2. VALIDATION /          3. CANONICAL BYTES     4. DIGEST /
     parse, preserve             NORMALIZATION            RFC 8785              FRAMING
     evidence                    application-owned        library-owned         b3:<hex>

Rust JSON:    serde_json    ->  CodeFabric rules  ->  serde_json_canonicalizer  ->  blake3
Python JSON:  stdlib json   ->  CodeFabric rules  ->  rfc8785                   ->  blake3
Rust YAML:    serde_yaml_ng ->  CodeFabric rules + JSON projection
                                              ->  serde_json_canonicalizer  ->  blake3

Typed bytes field (a side format, not a stage):
              raw bytes  <->  base64 URL_SAFE_NO_PAD text
```

Stage 2 is the one the libraries deliberately **do not** own. The pack's responsibility split
(`pack` *CodeFabric responsibility split*) leaves exactly seven things in repository code:
duplicate JSON member rejection before objects collapse into maps; the safe integer bound;
`codefabric-int64` / `codefabric-uint64` / unpadded `codefabric-bytes` validation; lowercase ASCII
ID and digest validation; deterministic sorting of non-string-keyed semantic maps represented as
record arrays; `b3:<64 lowercase hex>` framing; and contract-tree walking, generated-source
digests, profiles and drift detection.

Everything else is library territory, and the pack is blunt about it: no CodeFabric-owned JSON
escaping routine, UTF-16 key comparator, or float formatter should be introduced while the adopted
JCS libraries remain the profile implementation (`pack` *CodeFabric responsibility split*). `sjc`
§23 states the same as an execution rule — never implement object sorting, string escaping or
float formatting in repository code.

| Stage | Owns | Explicitly does **not** own |
|---|---|---|
| **`serde_json` / stdlib `json`** | tokenizing, the JSON data model, duplicate *evidence*, lexical number provenance | canonical output (`serde_json` *Canonicalization-specific anti-patterns*; `pyjson` *Encoder surface and why it is not JCS*) |
| **`sjc` / `rfc8785`** | member ordering, number rendering, string escaping, non-finite rejection | schema validation, the accepted domain, duplicate detection (`sjc` §1; `rfc8785` §11) |
| **`blake3-rust` / `blake3-py`** | the 32-byte digest | the `b3:` prefix, case rules and length validation — framing is application code (`blake3-rust` §8, `blake3-py` *Digest framing*) |
| **`base64`** | the URL-safe unpadded lexical form of a bytes field | integrity, authenticity, confidentiality (`base64` §18) |
| **`serde_yaml_ng`** | YAML 1.1 parsing into a Rust model | the JSON projection, and any claim that YAML syntax is semantics (`serde_yaml_ng` §1, §21) |

---

## The pairing is the point

Six of the nine documents are **three matched pairs**. Read them as pairs, because a change on one
side is only correct if the other side agrees.

| Stage | Rust | Python |
|---|---|---|
| strict ingress | `serde_json` (§1-§22) | `pyjson` (§1-§27) |
| canonical bytes | `sjc` (§1-§23) | `rfc8785` (§1-§23) |
| digest | `blake3-rust` (§1-§23) | `blake3-py` (§1-§21) |
| bytes lexical form | `base64` (§1-§21) | — *(see the gap in SKILL §"Navigation hazards")* |
| YAML registry ingress | `serde_yaml_ng` (§1-§27) | — *(Rust-only by design)* |

The invariant those pairs exist to defend, from `pack` *Cross-language invariant*:

```text
Rust validated value   --serde_json_canonicalizer--> canonical bytes --blake3--> digest
Python validated value --rfc8785-------------------> same bytes       --blake3--> same digest
```

**The parity contract itself is split across two documents**, which is why parity work always
reads both:

- `rfc8785` §20 holds the **three assertions** — identical canonical bytes, identical raw BLAKE3
  digest, identical `b3:` text — plus the rule that negative fixtures compare *failure class*, not
  Python-vs-Rust exception strings.
- `sjc` §17 holds the **fixture-file schema** and the reason it exists: test both languages from
  the same fixture definitions so neither implementation becomes the unchallenged oracle.

`rfc8785` *Cross-language parity* adds the trap that makes this non-obvious: build fixtures around
semantic values and *expected bytes*, not merely "Rust output equals Python output" — otherwise a
correlated defect passes both sides.

---

## Version anchors

| Library | Documented | Where the repository declares it |
|---|---|---|
| `serde_json_canonicalizer` | 0.3.2 | `Cargo.toml` |
| `serde_json` | 1.0.151 + `arbitrary_precision` | `Cargo.toml` |
| `base64` | 0.22.1 | `Cargo.toml` |
| `serde_yaml_ng` | 0.10.0 | `Cargo.toml` |
| `blake3` (Rust) | **deliberately unpinned — see below** | `Cargo.toml` |
| `rfc8785` | 0.1.4 | `codefabric-cpg-mcp/pyproject.toml` |
| `blake3` (Python) | 1.0.9 | `codefabric-cpg-mcp/pyproject.toml` |
| stdlib `json` | CPython 3.14.7 | the interpreter, not a manifest |

**The two halves of `blake3` carry opposite pin instructions, on purpose.** The Rust document
states that it describes 1.8.6 but "does **not** authorize changing the repository's resolved
version", and prints its own pin block as `blake3 = "=YOUR_EXISTING_PIN"` (`blake3-rust` *Version
/ source anchors*, `blake3-rust` §13); its checklist asks only that the existing pin be
*preserved*. The Python document requires the opposite — exactly `1.0.9` (`blake3-py` *Agent
checklist*, §11, §21). Do not generalize either instruction to the other side.

Consequently the documented version is not always the resolved one. **Read the manifests and locks
rather than the documents** when a version matters — and note that `blake3` is the only
canonicalization dependency in `Cargo.toml` declared as a range rather than an exact `=` pin, so
its resolved version lives in `Cargo.lock`.

Two further pin facts stated by the documents themselves: `sjc` pulls `serde_json` with
**`float_roundtrip`** and depends on **`ryu-js`** for number rendering (`sjc` *Installation and
pinning*, *RFC 8785 mechanics that this crate should own*); and `serde_yaml_ng` sits on
**`unsafe-libyaml`**, so parser upgrades are security-relevant even when the Rust-facing API is
unchanged (`serde_yaml_ng` *Security / resource policy*).

---

## How to read this pack

**The pack is small — 4,330 lines across ten files — and its chapters are tiny.** Median chapter
length is 10 to 17.5 lines; the longest chapter in the entire pack is 54 lines. Reading a whole
chapter costs almost nothing. Do not grep for a fragment when you can read the chapter.

But the chapter is usually not where you should start, because of the pack's structure.

### Every numbered document has two parts

```text
lines 1 .. ~200-250   PART 1   UNNUMBERED  ## sections  — about 45% of the document
   ~200-250           # Extended capability catalog     — an H1 divider
        .. end        PART 2   NUMBERED  ## N) chapters — members are unnumbered ### headings
```

**Part 1 is not a summary of Part 2.** It is a complete, self-contained briefing *and* the only
home of a large number of facts. Every Rust signature in `sjc` appears in Part 1 (*Public API
surface*) and nowhere else — `sjc` §2-§5 discuss `to_vec`, `to_string`, `to_writer` and `pipe`
without ever restating their types. The same is true of `blake3-rust`'s `hazmat` module,
`Hash::from_hex` and `HexError`; `base64`'s `encoded_len`, `decoded_len_estimate`,
`GeneralPurpose::new`, `with_decode_allow_trailing_bits`, `DecodeSliceError` and
`EncodeSliceError`; `blake3-py`'s ~1 MB threading threshold and its 3.13t wheel note; and every
Cargo feature matrix in the pack. REFERENCE.md marks these rows explicitly.

**So the default move is: read Part 1 of the one relevant document, whole.** That is 200 to 250
lines, it answers most questions outright, and it is the only way to see the front-matter-only
material. Drop into Part 2 chapters afterwards for mechanism, worked pipelines and fixtures.

Two documents have no Part 2 and are simply read whole: `pack` (156 lines) and `rejected` (87
lines).

### The shared skeleton

Part 1 sections appear in near-identical order in every document, so you can predict where
something lives in a file you have never opened:

| Part 1 section | Holds |
|---|---|
| *Version / source anchors* | the pin, the release date, upstream URLs, platform floors |
| *CodeFabric role* / *Feature inventory* / *Capability inventory* | what the library is for here, and the negative list |
| *Installation* / *Installation and pinning* | the literal manifest block and transitive-dependency facts |
| API sections | **signatures** and accepted domains |
| semantics sections | ordering, numbers, strings, errors |
| *…anti-patterns* | the 7-8 named ways to get it wrong |
| *Agent checklist* | the preconditions before touching the code |
| *Testing matrix* + *Upgrade and compatibility policy* | **identical in all eight documents** |

That last row is literally one shared block — byte-for-byte identical across all eight numbered
documents. Read it once. It is also JSON-phrased everywhere, including inside `base64`, where it
talks about JSON escaping and duplicate member names; the document-specific matrices are the real
ones (`serde_yaml_ng` §25, `blake3-rust` §21, `base64` §17).

Each numbered document also **ends** with its own numbered *Agent execution playbook* — a short
fenced rule block (`sjc` §23, `rfc8785` §23, `serde_json` §22, `pyjson` §27, `blake3-rust` §23,
`blake3-py` §21, `base64` §21, `serde_yaml_ng` §27). Those are the natural last stop on any
reading path below.

---

## Reading paths by problem context

### 1. Implementing strict Rust JSON ingress

Start in `serde_json`, and start with the part most implementations get wrong: **you cannot detect
a duplicate member from a parsed map.** `serde_json` *Custom `Deserializer` and Visitor path*
states the reason — a map representation cannot prove whether the source contained a duplicate —
and `serde_json` §13 gives the `MapAccess` sketch that records names as they arrive. This has to
be decided before any other design choice, because it dictates whether you can use `Value` at all.

Then the numeric contract, which is the second thing that is decided by parse order rather than by
logic. `serde_json` §7 explains what `arbitrary_precision` is for here: it exists to **delay
information loss long enough to reject** out-of-domain tokens, not to permit wide integers. The
rule it states is that a successful `as_f64()` does not mean the source token was an allowed
integer token — so token classification precedes conversion, not the reverse. The safe-integer
bound itself is written out in `serde_json` *`Number` and the `arbitrary_precision` feature* and
again in `pack` *CodeFabric responsibility split*.

With those two settled, read the entry points (`serde_json` §2), the `Value` model (§5) and the
object-order policy (§6 — which warns that `Map` ordering is not RFC 8785 ordering). If you are
retaining sub-documents verbatim, §8 covers `RawValue`.

Then read two policy chapters that are easy to skip and expensive to skip: `serde_json` §14, which
treats Serde derive attributes (`rename`, `flatten`, `skip_serializing_if`, enum tagging,
`serialize_with`) as **wire-schema policy** — changing one on a fingerprinted type is a protocol
change even when the Rust types are unchanged — and §16 for resource limits. Finish at §22.

Part 1's *Canonicalization-specific anti-patterns* is the compressed version of everything above;
read it as a checklist afterwards.

### 2. Implementing strict Python JSON ingress

`pyjson` is the mirror, and it is more directly usable because it contains a **complete reference
implementation**: Part 1 *Recommended strict loader*, expanded at `pyjson` §20. Read the hook
chapters first so the implementation is not a black box — §3 (`object_pairs_hook`, which is where
duplicate detection must live), §4 (`parse_int`), §5 (`parse_float`), §6 (`parse_constant`, which
is how `NaN`/`Infinity` tokens get rejected rather than accepted).

`pyjson` §7 states the default behavior you are overriding: repeated member names silently
collapse. §8 explains why `object_hook` cannot substitute — it sees an already-created dict and
therefore cannot recover duplicates. That pairing (§7 + §8) is the Python form of the same rule
`serde_json` §13 states for Rust.

Then `pyjson` *Encoder surface and why it is not JCS* and `pyjson` §11, so it is clear that the
stdlib encoder is not in the canonical path at all — `sort_keys=True` is addressed directly at §11
and is not sufficient. §22 for limits, §19 for `JSONDecodeError`, §27 to finish.

### 3. Producing canonical bytes, and proving they match

Read the serializer for your language, then the *other* language's parity chapter — the contract
is split across the pair (SKILL §"The pairing is the point").

**Rust:** `sjc` Part 1 whole, because that is where the four signatures are. Then `sjc` §2 (`to_vec`
is the default protocol API — bytes, not text), §6 (object sorting: not UTF-8 byte order, not
scalar-value order, not locale collation, and explicitly not `BTreeMap<String, _>`), §7 (number
rendering via `ryu-js`), §9 (string escaping, and the rule that canonical bytes are final — no
post-serialization string operations), and §11 (**arrays are never sorted by JCS**; array order
is the semantic model's responsibility, not the canonicalizer's).

Two chapters are traps worth reading before you write anything: `sjc` §8, on the
`arbitrary_precision` interaction — the safe-integer check belongs *before* `to_vec` — and §5, on
`pipe`. `pipe` takes a JSON string and returns canonical text, which makes it look like the
convenient entry point; it is listed in *Common failure modes* precisely because calling it on
untrusted protocol input loses the duplicate-key evidence that stage 1 exists to capture.

**Python:** `rfc8785` Part 1 whole, then `rfc8785` §3 (the accepted input domain — non-string dict keys are
rejected, not coerced), §4 and §5 (integer- and float-domain failures), §7 (member ordering,
which is *not* equivalent to Python's normal string ordering in all cases), §8 and §9. §13
explains why there is no pretty-print mode and why that is deliberate. §14 bans a generic
`default=` fallback: the schema adapter is application code.

Then whichever parity chapter you did not read: `rfc8785` §20 and `sjc` §17. Finish at §23 on
either side.

### 4. Computing and framing the digest

The digest is the easy part; the framing is where the rules are. Both blake3 documents make the
same division: **the hash crate never knows about `b3:`** (`blake3-rust` §8, `blake3-py` *Digest
framing*).

Read Part 1 of your side, then the pipeline chapter — `blake3-rust` §18 or `blake3-py` §14 — which
shows the correct call sequence taking canonical bytes as input. Then the verification chapter
(§19, §15), which carries the rule that matters in review: validate exactly 64 lowercase hex
characters, and **do not silently lowercase** an uppercase digest. On the Rust side that trap is
sharper, because `Hash::from_hex` would accept uppercase — a fact that appears only in
`blake3-rust` *`Hash` representation*, in Part 1, where chapter navigation will not find it.

Read the mode chapters once so you can recognize them in review, and then keep them out of the
fingerprint path: keyed hashing (`blake3-rust` §9, `blake3-py` §7), key derivation (§10, §8), and
XOF (§11, §5). Both documents treat any non-32-byte output, `seek`, or keyed mode in checksum code
as a **protocol change requiring justification** — and the two APIs make that visible differently,
which changes what you grep for: Rust separates the modes into distinct functions, while Python
selects them with `key=` and `derive_key_context=` keyword arguments on one constructor (§2).

`blake3-rust` §4 has the invariant behind all of this: `hash("ab" || "c") == hash("a" || "bc")`.
Concatenating fields without framing is ambiguous, which is why the canonical bytes — not an ad
hoc concatenation — are the input.

Parallelism and mmap are controlled differently on the two sides and neither may change output:
Rust uses the `rayon` and `mmap` Cargo features (`blake3-rust` §6, §7, §13), Python uses the
`max_threads=` argument and `update_mmap` (`blake3-py` §9, §10). Both warn against reaching for
them on small records without benchmarks. Finish at `blake3-rust` §23 or `blake3-py` §21, and read
`blake3-py` §18 for parity regardless of which language you are in — it is the pair's only
dedicated parity chapter.

### 5. The `codefabric-bytes` lexical contract

`base64` is short and its rule is narrow: **`URL_SAFE_NO_PAD`, and nothing else**. Read Part 1
whole — it holds the error types, the panic/overflow note, and the sizing helpers that Part 2
never names — then `base64` §2, which frames the choice correctly: the engine is part of the
format specification, not a convenience.

The strictness lives in three chapters. `base64` §8 (padding: `URL_SAFE_NO_PAD` rejects input
containing `=`, and a padded spelling violates the format even though a permissive decoder would
recover the same bytes), §9 (trailing bits: keep the default strict behavior; leniency admits
multiple textual encodings for one byte sequence), and §16, the complete validator — reject `=`,
decode, then **re-encode and compare**, so a non-canonical spelling fails even if the decoder
later becomes lenient.

`base64` §15 places the leniency boundary: a compatibility decoder may exist at an adapter, but
must never leak into canonical source validation. §6 gives a review signal worth remembering — if
new canonicalization code calls `Alphabet::new(...)`, stop and ask why. §10 for errors, §21 to
finish.

For the Python side of this contract see the gap noted in SKILL §"Navigation hazards"; the pack's
only statement of it is `rfc8785` §15.

### 6. Ingesting the YAML registry

`serde_yaml_ng` is the longest chapter list in the pack (§1-§27) and the one with the most ways to
silently change meaning. Its framing chapter is `serde_yaml_ng` §1, and the property it exists to
protect is stated there: a formatting-only YAML edit that preserves the parsed model must not
change the fingerprint, while any semantic-model change must.

Read `serde_yaml_ng` §2 next. YAML 1.1 — not 1.2 — is the specification scope, and the chapter's
rule is to stop assuming "YAML is just JSON with comments": tags, block styles, aliases, richer
scalar syntax and non-string mapping keys are all in play. §15 is the practical consequence, where
plain scalars resolve to unintended booleans and nulls.

Then the four features with no JSON equivalent, each of which needs an explicit policy rather than
a default: tags (`serde_yaml_ng` §9, with three named policies), enum helpers (§10), anchors and
aliases (§11 — fingerprints must not depend on anchor names or on inline-versus-alias authoring),
and merge keys (§12 — support is parser-sensitive and needs an executable fixture against the
pinned parser).

The projection itself is Part 1 *Conversion into canonical JSON* plus `serde_yaml_ng` §21, a
ten-item checklist that is the densest invariant block in the document. §6 carries the rule that
catches people: the `serde_yaml_ng::Value` → `serde_json::Value` conversion is **not total**, so a
generic conversion is not a projection. §7 handles non-string mapping keys as deterministically
sorted record arrays; §8 states the three things a successful YAML number parse does not prove.

`serde_yaml_ng` §22 is the duplicate-key policy, and it is worth reading carefully because it is
**not** stated as flatly as the JSON-side rule: it recommends symmetric rejection but leaves the
decision explicitly open. Do not cite it as a settled invariant. §23 for resource limits (alias
expansion is one), §25 for the YAML-specific testing matrix, §27 to finish.

### 7. Someone proposes swapping a serializer or bumping a pin

Read `rejected` first — all 87 lines, it is short by design. It exists so an agent does not
rediscover an attractive alternative and silently replace a rejected dependency. It rules out
`serde_jcs` (familiar `to_vec`/`to_string`/`to_writer` API, but byte-level equivalence is the
requirement, not surface compatibility) and `orjson` (sorted keys plus compact output is a
superficial subset of canonical JSON; JCS additionally fixes Unicode property-name ordering,
string escaping, number rendering and the accepted numeric domain).

It then gives the **eight-question decision test** and the **nine-item substitution evidence
checklist**. Both are gates, not suggestions: if any answer is missing, it is not a drop-in
replacement.

For a version bump rather than a substitution, use `pack` *Dependency-change decision tree*, whose
pivot is whether the dependency participates before or at canonical bytes, and whose conclusion is
that a canonical-byte change means a **new profile**, never an in-place mutation of
`codefabric-jcs-v1`. `sjc` §21 is the nine-step investigation checklist for exactly that case.

The rule underneath all of it appears identically at the end of all eight documents: do not infer
compatibility from SemVer alone when a dependency participates in a byte-level protocol.

### 8. Writing the shared fixture corpus

Fixtures are the pack's actual correctness gate, and four documents each own a piece.

`sjc` §17 defines the **fixture-file schema** and the principle — drive both languages from the
same definitions so neither becomes the unchallenged oracle. §18 adds the RFC test vectors and the
generated properties, including the metamorphic property worth having. `rfc8785` §21 lists the RFC
appendix and edge vectors to retain, notably property names whose UTF-16 order differs from naive
code-point assumptions. §20 gives the three assertions, and the rule that negative fixtures
compare failure *class*, not exception strings.

Then the per-document matrices for whatever you are covering: `serde_json` *Testing matrix for
agent-authored changes*, `pyjson` §25, `blake3-rust` §21, `blake3-py` §20, `base64` §17,
`serde_yaml_ng` §25. Note that §25 supersedes the shared boilerplate matrix in that document's
Part 1.

The negative corpus is enumerated in the shared upgrade policy: duplicate names, unsafe integer
tokens, non-finite values, malformed typed-format strings, non-canonical base64, and uppercase IDs
and digests.

### 9. Diagnosing a cross-language byte divergence

Bisect by stage rather than by library, and use the error taxonomy that already exists for this:
`rfc8785` §10 routes failures into four lanes — ingress, validation, canonicalization, checksum —
precisely so you can tell whether a fixture failed *before* or *during* canonicalization.
`serde_yaml_ng` §17 gives the parallel code taxonomy for the YAML path.

Then go to the axis:

| Symptom | Read |
|---|---|
| numbers differ | `sjc` §7 (`ryu-js`, six fixture classes) + `rfc8785` §8. Check `-0.0`, exponent thresholds, subnormals, values near representability limits |
| key order differs | `sjc` §6 + `rfc8785` §7. UTF-16 code-unit order, supplementary-plane characters — not Python string order, not `BTreeMap` |
| one side rejects, the other accepts | integers: `sjc` §8 + `rfc8785` §4. Non-finite: `sjc` *RFC 8785 mechanics…* + `rfc8785` §5 |
| bytes match, digest differs | framing, not hashing: `blake3-rust` §8 / `blake3-py` §15. Check case, prefix and length validation |
| output is stable but wrong | `rfc8785` §17 — JCS cannot repair nondeterminism already encoded as array order or scalar values |

The divergence surface the pack itself flags as least specified: the Python float-formatting
algorithm is never named, while the Rust side names `ryu-js` (`rfc8785` §8). Treat number fixtures
as the highest-value ones.

---

## The seam

These rules span documents. Each is a real failure mode the pack names, and none is fully stated
in any single chapter.

1. **Duplicates must die before map materialization.** A map cannot prove a duplicate existed.
   Rust: `serde_json` §13. Python: `pyjson` §3 + §7 + §8. The canonicalizers cannot help —
   `rfc8785` *Anti-patterns* lists "canonicalizing after duplicate keys have collapsed into a
   dict", and `sjc` §5 lists `pipe` on untrusted input for the same reason.

2. **Token validation precedes conversion.** `as_f64()` / `float()` erases exactly the distinction
   that has to be rejected. `serde_json` §7, `sjc` §8, `rfc8785` §4.

3. **`arbitrary_precision` is not permission.** It delays loss long enough to reject; it does not
   widen the JCS numeric domain. `serde_json` §7, `sjc` §8.

4. **Sorted keys plus compact output is not JCS.** `rfc8785` §7 names
   `json.dumps(sort_keys=True, separators=…)`, `sorted(d)` and `OrderedDict` as non-substitutes;
   `rejected` names `orjson.dumps(..., OPT_SORT_KEYS)`; `sjc` §6 names `BTreeMap<String, _>`.

5. **JCS sorts object properties, never arrays.** `sjc` §11 states it as a chapter; `rfc8785` §16
   states it inside the non-string-keyed-maps chapter. Array order is the semantic model's job,
   which is why non-string-keyed maps become *deterministically sorted* record arrays
   (`pack` *CodeFabric responsibility split*, `serde_yaml_ng` §7).

6. **Hash the canonical byte vector, never a re-serialized string.** `sjc` §16 carries a labelled
   wrong-contract example — newline-translating canonical text before hashing. `blake3-rust` §18
   and `blake3-py` §14 show the correct sequence.

7. **Canonical bytes are final.** No post-serialization string operation, no Unicode
   normalization, no whitespace change. `sjc` §9, `rfc8785` §9, §6.

### Asymmetries the pack documents on only one side

These are not gaps in your reading — they are places where the pack states a two-sided rule once.

- **`URL_SAFE_NO_PAD` is named on the Python side.** `rfc8785` §15 is the pack's statement that the
  `codefabric-bytes` form matches Rust's `base64::URL_SAFE_NO_PAD`; `sjc` never mentions base64
  encoding at all.
- **The float algorithm is named on the Rust side.** `sjc` names `ryu-js`; §8 requires
  byte-identity with it without naming Python's own implementation.
- **The parity contract is split**, as above: assertions in §20, fixture schema in
  `sjc` §17.
- **The YAML seam is one-directional.** `serde_yaml_ng` hands off to `serde_json_canonicalizer`
  (*Conversion into canonical JSON*, `serde_yaml_ng` §1); `sjc` never mentions YAML.
- **The duplicate rule is flat for JSON, open for YAML.** §22 recommends symmetric
  rejection but leaves it an explicit decision.
- **Parity has a dedicated chapter only in `blake3-py`** (§18); on the Rust side the same
  obligation is distributed through the checklist and `blake3-rust` §21.

---

## Key invariants

Drawn from the documents' own checklists, anti-pattern lists and playbooks.

1. The four stages stay separate; several libraries being able to parse JSON is not a reason to
   collapse them (`pack` *Reference-pack architecture*).
2. No repository-owned JSON escaping routine, UTF-16 key comparator, or float formatter
   (`pack` *CodeFabric responsibility split*, `sjc` §23).
3. Duplicate member names are rejected before objects become maps (`serde_json` §13, `pyjson` §3).
4. Integer tokens outside the safe bound are rejected before canonicalization (§7,
   §8, `rfc8785` §4).
5. Non-finite floats fail; they are never stringified into application-defined tokens
   (`sjc` *RFC 8785 mechanics…*, §5).
6. The canonical byte vector is the hash input, unmodified (§16, `blake3-rust` §18).
7. The digest is unkeyed and exactly 32 bytes; keyed, derive-key and XOF modes stay out of the
   `b3:` path (§23, `blake3-py` §21).
8. `b3:` framing, lowercase enforcement and length validation are application code, not library
   behavior (§8, `blake3-py` *Digest framing*).
9. `codefabric-bytes` is `URL_SAFE_NO_PAD` only, validated by re-encode-and-compare
   (`base64` §8, §9, §16).
10. Serde attribute changes on a fingerprinted type are schema changes (§14,
    `serde_yaml_ng` §20).
11. Build-time choices — SIMD backend, thread count, feature flags — must never change output
    bytes (§12, `blake3-py` *Multithreading*, §13).
12. A canonical-byte change is a new profile, never an in-place edit of `codefabric-jcs-v1`
    (`pack` *Upgrade and compatibility policy*, §21).

---

## Navigation hazards

1. **`just lib-outline` does not work on this pack.** The chapters are `##`-level, so the
   h1-rooted outliner emits only the title and `# Extended capability catalog` — two headings for
   a 442-line document. Use REFERENCE.md §1 instead; that is what it is for.

2. **Never write a `§N` whose document is not established.** Eight documents each number from `§1`, and the numbers mean
   unrelated things: `§16` is *Hashing without accidental text transformations* in `sjc`,
   *Non-string-keyed logical maps* in `rfc8785`, *Key coercion and `skipkeys`* in `pyjson`,
   *Dynamic-to-typed conversion* in `serde_yaml_ng`, and *Resource limits and denial-of-service
   posture* in `serde_json`. Qualify the first citation in a paragraph; within a run of
   references to the same document a bare `§N` is fine, but any switch of document re-qualifies.

3. **There is no `§0` and no `§N.M`.** Numbering starts at `§1`, and members inside a chapter are
   unnumbered `###` headings. Part 1 sections have no number at all — cite them by title.

4. **Part 1 material is invisible to `§` navigation**, and it is where the signatures are. See
   SKILL §"How to read this pack".

5. **The trailing boilerplate is duplicated eight times.** *Testing matrix for agent-authored
   changes* and *Upgrade and compatibility policy* are byte-identical in every numbered document.
   Do not read it eight times, and do not mistake it for document-specific guidance — in `base64`
   it discusses JSON escaping and duplicate member names.

6. **Fenced code contains `#` comments that look like headings** — for example in `blake3-py` and
   `rfc8785`. Any extractor over these files must track fence state; `grep '^#'` will lie.

7. **One handoff leaves the pack.** `base64` §17 asks for cross-language fixtures against Python's
   URL-safe unpadded implementation, but there is **no Python base64 document** in the pack. The
   only in-pack statement of the Python side of that contract is `rfc8785` §15.

---

## Boundaries

**orjson.** `rejected` permits orjson for ordinary application JSON and performance-sensitive
adapters that do not participate in canonical fingerprints, and forbids its output as
`codefabric-jcs-v1` canonical bytes. Keep "fast ordinary JSON" and "canonical protocol JSON" as
separate dependency roles even when one library could technically serialize the same object. For
orjson's own API — `dumps`/`loads`, the `OPT_*` flags, `default`, `Fragment` — use the sibling
skill **`grpcio-orjson-protobuf-ref`**.

**Other siblings.** Arrow, Delta and DataFusion storage and query → `deltalake-rust-ref`,
`datafusion-pyarrow-rust-ref`. gRPC transport and protobuf schemas → `grpcio-orjson-protobuf-ref`.
The MCP serving surface and its models → `fastmcp-pydantic-ref`. Graph algorithms →
`petgraph-ref`. Parsers and code-fact providers → `code-facts-lib-ref`. Git and filesystem
watching → `gix-notify-ref`.