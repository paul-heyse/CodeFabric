---
artifact: implementation-review
review_id: codefabric-waves-0-3-foundation-v4-library-leverage-v1
date: 2026-08-20
status: complete
plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md
state_path: docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json
baseline_commit: a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91
reviewed_packets:
  - WP01
  - WP02
  - WP03
  - WP04
  - WP05
  - WP06
next_packet: WP07
verdict: design-invalidated
---

# Implementation Review: CodeFabric waves 0-3 foundation plan v4

## Provenance and review scope

This is a read-only review of the implementation completed through WP06 and of the
load-bearing design that WP07 would extend. It does not assess unimplemented runtime
behavior as though it already existed. WP01-WP06 are complete in the execution state;
WP07 is `not_started`.

The review concentrates on the user's concern: whether the foundation is using its
selected libraries as model compilers and runtime authorities, or whether it is building
manual lists, validators, mirrors, and expected hashes that will create recurring churn.
It therefore reviews:

- the contract tree, JCS implementation, generator, verifier, fixtures, and generated
  indexes introduced by WP06;
- the Wave-0 FastMCP/Pydantic adapter shell and gRPC channel seam;
- the Protobuf generation and compatibility probe introduced by WP05;
- the Cargo feature/cache design and current aggregate command graph;
- the plan obligations that will populate these foundations in WP07-WP11 and later expose
  them through FastMCP.

The primary library authorities used for this review are:

- `README_canonicalization_library_reference_pack.md`,
  `serde_json_rust_advanced_reference_1.0.151.md`,
  `python_stdlib_json_3.14.7_advanced_reference.md`,
  `serde_json_canonicalizer_rust_advanced_reference_0.3.2.md`,
  `rfc8785_python_advanced_reference_0.1.4.md`, the Rust/Python BLAKE3 references,
  `base64_rust_advanced_reference_0.22.1.md`, and
  `serde_yaml_ng_rust_advanced_reference_0.10.0.md`;
- `grpcio_python_advanced_reference_1.83.0.md`,
  `protobuf_python_advanced_reference_7.36.0.md`, and
  `orjson_python_advanced_reference_3.12.0.md`;
- `fastmcp_python_advanced_reference_3.4.7.md` and
  `pydantic_python_advanced_reference_2.13.4.md`.

Repository searches excluded generated trees and virtual environments unless the generated
artifact itself was the subject. The Python structural-search envelope was the 18 files under
`codefabric-cpg-mcp/src` and `codefabric-cpg-mcp/tests`; text searches additionally covered
`src`, `tests`, `tooling/proto`, and the non-generated `contracts` tree.

## Executive summary

The completed foundation should not be discarded. Its strongest decisions are sound:

- Cargo capability features now prevent narrow contract, Protobuf, and fuzz work from
  compiling the DataFusion/Delta graph. This is a larger performance gain than serializer
  micro-optimization.
- Rust delegates RFC 8785 emission to `serde_json_canonicalizer`, Python delegates it to
  `rfc8785`, and both delegate BLAKE3 and base64url primitives to their selected libraries.
  The custom code is concentrated at CodeFabric's stricter ingress/profile boundary, where
  the libraries do not supply the full contract.
- The shared cross-language JCS corpus and production-path fuzz target are valuable. The
  fuzz-discovered fractional-number case demonstrates that the emitted-value validation is
  real proof, not ceremony.
- The FastMCP shell uses an explicit server identity, lifespan-owned settings,
  `strict_input_validation`, masked errors, protocol-silent STDIO behavior, and FastMCP's
  in-memory `Client` test path.
- The gRPC seam uses `grpc.aio` and symmetric message limits; the Protobuf generator emits
  committed Python type stubs, records its toolchains, and proves two-root regeneration.

The hesitation about churn is nevertheless justified. The principal issue is not that too
many libraries were ignored. It is that the contract compiler does not yet have one typed,
declarative model of the artifacts it owns. The 50-file artifact census, the 13 registry
sources, output paths, generated views, warning counts, and tests are duplicated in Rust code.
Metadata is verified by byte-substring scans, while traceability is traversed as generic
`serde_json::Value`. Every one of the 50 required machine sources embeds a zero digest, and
the current generated index hashes those zero-bearing sources. The governing AC-G-02 rule
requires machine artifacts to embed their digest but does not define the digest projection
that excludes that field, so a real embedded value would otherwise be self-referential.

The best correction is a typed contract-compiler architecture, not a universal mega-schema:

```text
ecosystem-native authorities
  artifact catalog + registry YAML + JSON Schema + .proto + EBNF
        |
        v
bounded, format-specific ingress
        |
        v
typed Contract IR + explicit digest projection + source provenance
        |
        +--> canonical JSON / BLAKE3 artifact index
        +--> generated Rust compatibility types
        +--> generated Python adapter types
        +--> Protobuf descriptor set + Rust/Python stubs
        +--> Pydantic validation/serialization schemas
        +--> FastMCP protocol manifest and fingerprints
```

One catalog should describe ownership and derivation; each native format should retain its
own semantic authority. In particular, `.proto` remains the RPC wire authority, Pydantic
remains the executable Python adapter-contract compiler, FastMCP remains the published MCP
component authority, and JCS libraries remain the canonical byte emitters. `orjson` should
not be inserted into any of those roles merely because it is fast.

## Verdict

**Design invalidated for continuation beyond WP06.**

This verdict is narrow. WP01-WP05 and most WP06 mechanics are acceptable and reusable. The
accepted design must be corrected before WP07 populates identity contracts because:

1. AC-G-02's embedded machine-artifact digest is undefined without an explicit
   non-self-referential digest projection; and
2. the current generator/verifier authority is a set of hard-coded Rust lists and byte scans,
   not the model-based contract compiler that the roadmap says Wave 1 establishes.

Continuing WP07-WP11 on the current substrate would multiply duplicated census data,
placeholder digest updates, renderer branches, and fixture churn. The required correction is
an evolution of the completed work, not a restart.

## Gate and evidence assessment

The pre-edit `just ci-fast` gate passed on 2026-08-20. It covered:

- stable-root format, default and featureless checks/Clippy, 21 Nextest tests, doctests,
  typos, dependency hygiene, and exact graph governance;
- rustc-extractor format/check/Clippy/tests/identity;
- Pyrefly sidecar format/check/Clippy and two tests;
- adapter Ruff, Pyrefly, 46 pytest cases, and the locked STDIO subset;
- ast-grep governance, duplicate-family and seed zero-state checks;
- Protobuf generator check and contract full-profile verification;
- contract negative fixtures and two-root byte-identical regeneration.

`codefabric-contracts verify --profile full` reported 50 source artifacts with 50 draft
warnings. This is appropriate for the current draft phase but does not validate released
content. The state correctly leaves M02 and WP07 onward incomplete.

Current global sccache telemetry reported 1,743 compile requests, 209 Rust hits, 790 Rust
misses, and a 20.92% Rust hit rate. These aggregate counters are not a controlled benchmark,
but they reinforce the accepted design's rule that dependency closure and command topology
matter more than assuming cache reuse. During this gate, the sidecar test profile still spent
about 62 seconds code-generating its graph after its separate check and Clippy steps had
completed.

## Finding index

| ID | Severity | Dimension | Required before |
|---|---|---|---|
| IR-001 | blocker | correctness / architecture | WP07 |
| IR-002 | major | architecture / maintenance | WP07 |
| IR-003 | major | correctness / security / performance | WP07 |
| IR-004 | major | library / compatibility | WP10 |
| IR-005 | major | architecture / library | WP09 and serving implementation |
| IR-006 | minor | library / dependency policy | real RPC or ordinary-JSON use |
| IR-007 | minor | architecture / maintenance | Wave-1 generator expansion |
| IR-008 | minor | performance / operations | next command-contract revision |
| IR-009 | observation | tests / maintenance | ongoing |

## Findings

### IR-001 — Machine-artifact digest authority is self-referential and currently zero-filled

**Severity:** blocker

**Dimension:** correctness / architecture

**Design/Plan refs:** SUITE AC-G-02, AC-G-05, AC-G-07; QRY AC-G-53; v4 WP06

**Evidence:** AC-G-02 requires machine artifacts to embed `canonical_digest`, but unlike
AC-G-07's bundle digest it does not define which field is omitted while calculating that
digest. All 50 paths in `REQUIRED_SOURCE_ARTIFACTS` currently contain the same
`b3:` plus 64-zero placeholder. `collect_artifact_records` in
`src/contracts/artifacts.rs:358` canonicalizes those bytes as they stand and hashes them;
it does not validate the embedded value against the result. The generated artifact index
therefore fingerprints a document containing a placeholder rather than proving the artifact's
declared identity. Tests only validate that generated index values have checksum syntax.

**Failure mode:** replacing a zero with the computed digest changes the content and therefore
changes the digest again. The repository can report a green full-profile verification while
every embedded machine-artifact digest is false. Bundle and compatibility decisions can then
bind to the wrong notion of artifact identity.

**Remediation:** revise AC-G-02 to define one explicit digest projection for every machine
format. The recommended rule is:

```text
artifact_digest = BLAKE3-256(
  canonical semantic artifact with canonical_digest omitted
)
```

For JSON/YAML, remove the root metadata field before JCS projection. For line-oriented JSON,
omit it from the metadata record. For `.proto` and EBNF metadata comments, exclude the one
machine-readable digest header record using a format-specific parser, not an arbitrary text
replacement. Then populate and verify the embedded value. Keep generated artifact-index and
bundle digests as sidecar authorities, but do not use a zero value as a permanent sentinel.
If the design instead chooses external-only digests, make that a deliberate AC-G-02 change and
use the same explicit external marker already defined for prose.

**Focused re-test:** for every supported format, generate a real embedded digest; verify it;
mutate one semantic field and prove verification fails; mutate only the digest and prove it
fails; regenerate twice and prove byte identity. Assert that no required source contains the
zero sentinel. Add a bundle fixture that proves member-digest and bundle-digest projections
are distinct and correct.

### IR-002 — The contract compiler has no single declarative artifact/derivation model

**Severity:** major

**Dimension:** architecture / maintenance

**Design/Plan refs:** RM §6; SUITE AC-G-05; v4 I-11, D-04, WP06-WP11; doctrine Principles 10,
25, 29, and 31

**Evidence:** `src/contracts/artifacts.rs` hard-codes 50 source paths in
`REQUIRED_SOURCE_ARTIFACTS`, separately hard-codes 13 registry names in `REGISTRY_SOURCES`,
separately names five index mirror paths, and hand-renders Rust, Python, `.pyi`, and Python
package initialization. Tests assert the literal values 50 and 13. The generator currently
emits 14 files under `contracts/generated` plus four per-domain mirrors, but no machine model
declares why each output exists or which source/model produces it.

**Failure mode:** adding one Wave-1 artifact or output requires synchronized edits across
lists, dispatch code, renderers, tests, packaging, and generated mirrors. A missed edit either
creates a false census failure or, worse, silently leaves an artifact outside compilation and
verification. The literal-count tests turn intended additions into churn rather than deriving
the new expected state.

**Remediation:** add one versioned, typed artifact catalog under `contracts/manifests/` as the
contract compiler's declarative input. Each record should identify at least:

```text
artifact_id, authority_path, artifact_kind, format/profile, owner,
status/version, schema or parser, digest projection, generated outputs,
consumer domains, compatibility family, and provenance requirements
```

Deserialize it into `ContractCatalog`/`ArtifactDescriptor` types and derive:

- the required source and output census;
- registry selection and canonical output locations;
- generator dispatch and headers;
- per-domain package-data/copy requirements;
- warning and release counts;
- artifact-index records and traceability joins;
- generated-file governance inputs.

The catalog must not attempt to replace `.proto`, JSON Schema, EBNF, or registry YAML with a
lowest-common-denominator schema. It describes their ownership and derivation graph. Their
native semantic models remain authoritative.

**Focused re-test:** add a synthetic artifact to a test catalog with one record and prove the
compiler derives its census, digest, output, provenance header, and consumer view without a
second hard-coded edit. Reject duplicate IDs/paths/outputs, cycles, unknown kinds, and outputs
with multiple authorities. Replace literal 50/13 assertions with catalog-derived and
AC-G-05-required-set equivalence assertions.

### IR-003 — Verification is lexical and unbounded where it should compile typed models

**Severity:** major

**Dimension:** correctness / security / performance

**Design/Plan refs:** SUITE AC-G-02, AC-G-04, AC-G-05; v4 WP06-WP11; doctrine Principles 1,
10, 25, and 30

**Evidence:** `has_metadata` and `is_draft` at `src/contracts/artifacts.rs:670` scan byte
windows for field-name/status substrings. Traceability records are generic
`serde_json::Value` objects with string-index lookups. `canonical_source_bytes` and YAML
projection operate on generic dynamic values. Artifact files are read wholly into memory,
and the contract compiler has no application-owned raw-byte, collection-cardinality,
aggregate-node, string-length, YAML-alias-expansion, or diagnostic-count budget. Adapter
settings declare some future query limits, but those do not govern contract compilation.

**Failure mode:** a comment or nested value can satisfy a metadata substring check; innocuous
formatting can evade the draft scan; unknown or misspelled fields can survive dynamic
traversal; and pathological source files can consume excessive memory/CPU before a useful
diagnostic is emitted. As registries become populated, every new rule will otherwise become
another manual `Value` lookup and branch.

**Remediation:** compile each artifact family into typed Serde models with closed records
(`deny_unknown_fields`) wherever the contract is closed. Use a common typed `ArtifactHeader`,
typed registry/requirement/traceability records, discriminated artifact-kind dispatch, and
path-aware errors. Keep `serde_json::Value`/`serde_yaml_ng::Value` only at the truly generic
JCS/YAML projection seam. Enforce limits before and after parsing:

- file bytes before allocation/read;
- nesting/recursion and total semantic nodes;
- mapping/sequence cardinality and string/token length;
- YAML document/tag/merge/alias policy;
- registry-specific record/edge limits;
- bounded accumulated diagnostics.

Where a JSON Schema is itself the authority, validate its declared dialect and metaschema as
part of compilation; do not infer completeness from metadata alone.

**Focused re-test:** prove comments/nested strings cannot impersonate headers; reject unknown
fields, malformed statuses, duplicate semantic IDs, over-budget bytes/nodes/depth/aliases, and
incomplete trace records with stable path locations. Retain one limit-edge success fixture and
one just-over-limit failure fixture per dimension.

### IR-004 — Protobuf generation proves bytes, not a compiled protocol model

**Severity:** major

**Dimension:** library / compatibility

**Design/Plan refs:** SUITE AC-G-03, AC-G-05; v4 LD-10, WP05, WP10; protobuf ref §§0.2-0.4,
16, 20-21, 26, 30, 37; gRPC ref §§2, 18-19, 26, 29

**Evidence:** WP05 correctly proves the Wave-0 probe and records generator identity, but
`tooling/proto/generate.py` compares generated source bytes and a deterministic message
fixture only. It does not emit a `FileDescriptorSet`. Python generation uses grpcio-tools
1.83.0 / libprotoc 35.1 while Rust uses protoc-bin-vendored 3.2.0 / libprotoc 31.1. The
identities are visible, but no semantic descriptor comparison proves that the two compiler
families interpreted a real schema identically. Protobuf's deterministic serialization is
explicitly not a cross-version canonicalization format.

**Failure mode:** once the four production packages arrive, a service, method, field number,
presence rule, oneof, reserved range/name, option, or import can drift while generated text
comparison remains toolchain-specific and hard to classify. Treating deterministic wire bytes
as a durable fingerprint can also create false compatibility promises across runtimes.

**Remediation:** make `.proto` the single wire authority and emit a committed descriptor set
(`--descriptor_set_out`, with the chosen import/source-info policy) as the compiled protocol
IR. Derive a service/message/field/enum census and compatibility report from descriptors.
Use generated `DESCRIPTOR` objects/`DescriptorPool` for Python-side assertions. Compare a
normalized descriptor model across the Rust and Python generation paths, or standardize both
on one exact protoc family if a compatibility probe shows that is practical. Codify permanent
field numbers and reservation of deleted numbers/names. Keep binary fixtures small and
representative; treat them as compatibility known answers, not canonical content hashes.

Also establish one `DaemonClient` facade before production RPC use: one long-lived
event-loop-owned `grpc.aio` channel/stub created in FastMCP lifespan, bounded deadlines on
every call, centralized metadata/status translation, explicit close, and streaming only where
the response semantics require it.

**Focused re-test:** regenerate descriptor and stubs twice; compare normalized descriptor
semantics across both compiler paths; fail on field-number reuse/removal without reservation,
service/method cardinality drift, presence/oneof drift, or runtime/compiler incompatibility.
Run cross-language binary round trips plus unknown-field preservation and deadline/status/
message-limit tests.

### IR-005 — The adapter should compile its contract model into Pydantic and FastMCP views

**Severity:** major

**Dimension:** architecture / library

**Design/Plan refs:** SRV §§19, 33-34, 70-71; v4 WP09 and later serving packets; FastMCP ref
§§3, 6, 10-11, 14-15, 30, 33-34; Pydantic ref §§3, 7, 9-10, 21, 26, 34-35, 40, 48-50

**Evidence:** the current Wave-0 shell is correctly empty and therefore is not missing tools.
It does, however, provide no executable bridge yet from future `contracts/adapter` sources to
Pydantic models, Pydantic validation/serialization schemas, FastMCP declarations, and the MCP
manifest. The current adapter contains no `TypeAdapter`, as expected before the serving
contract is implemented. The risk lies in implementing those later views independently.

**Failure mode:** handwritten JSON Schemas, Pydantic DTOs, tool signatures, output models, and
FastMCP fingerprints can describe subtly different accepted/serialized shapes. Rebuilding
dynamic models or `TypeAdapter`s per request would add avoidable schema-compilation cost.
A generated custom FastMCP Provider for only four stable tools would add indirection without
removing this authority problem.

**Remediation:** make the typed Contract IR from IR-002 emit the small adapter-owned model
family and its JSON Schemas. Generate or otherwise compile stable Pydantic request/response/
meta models with the SRV §19 strict base policy. Instantiate the recursive JSON-object
`TypeAdapter` exactly once. Use discriminated unions for closed variant families and
declarative `Annotated`/`Field` constraints before custom validators/CoreSchema.

For the four stable public tools, prefer explicit typed handler functions registered through
FastMCP's existing `LocalProvider`; this maximizes static typing and makes FastMCP's signature
inspection the executable publication path. Use FastMCP DI for `RuntimeState`/`DaemonClient`
and request `Context`, so infrastructure never appears in the MCP schema. Reserve a custom
Provider for a genuinely dynamic/large component catalog. Providers source components;
transforms reshape publication; neither should become authorization or domain logic.

At generation/check time, compare all independent views required by SRV §70:

1. contract-IR adapter schemas;
2. Pydantic validation-mode schemas;
3. Pydantic serialization-mode schemas;
4. `tool.to_mcp_tool()` / `fastmcp inspect --format mcp` protocol manifest;
5. selected canonical tool fingerprints, including descriptions/annotations when they affect
   routing or policy.

Compile/cache models and adapters once at import/startup. Use `defer_build` only if measured
cold-start savings outweigh first-use latency. Keep the complete semantic query schema daemon
owned and validate only its recursive JSON shape in Python, exactly as SRV §19.4-19.5 requires.

**Focused re-test:** change one Contract-IR field and prove all intended generated views change
or a semantic equivalence gate fails. Verify unknown fields and unintended coercions fail,
validation and serialization schemas are separately fingerprinted, injected runtime values
are absent from the public schema, in-memory `Client(mcp)` exercises the full protocol path,
and no model/adapter is constructed inside a handler.

### IR-006 — Python protocol dependency intent and orjson's role are not declared precisely

**Severity:** minor

**Dimension:** library / dependency policy

**Design/Plan refs:** v4 LD-11, WP04; gRPC ref §1.4; protobuf ref §§1, 30; orjson ref §§0,
13.5, 22-29, 31-33

**Evidence:** `codefabric-cpg-mcp/uv.lock` resolves `grpcio==1.83.0`,
`grpcio-tools==1.83.0`, `protobuf==7.36.0`, and `orjson==3.12.0`, but
`codefabric-cpg-mcp/pyproject.toml` declares all four without exact constraints. Frozen sync is
currently reproducible, but a deliberate re-lock has no manifest-level compatibility intent.
`orjson` has no first-party source use in the reviewed adapter, tooling, or tests.

**Failure mode:** a routine re-lock can silently select a new protocol/compiler family before
the intended compatibility probe is rerun. Carrying `orjson` without an owned boundary adds a
native runtime dependency and future upgrade work; using it reflexively would be wrong for
JCS, ProtoJSON, Protobuf binary RPC, and FastMCP structured tool results.

**Remediation:** express the accepted exact protocol/compiler versions in the adapter manifest
as well as the lock, or define an equally explicit tested compatibility interval. Keep
`grpcio-tools` build-only. Remove/defer `orjson` until an ordinary JSON/JSONL/log/cache boundary
has a measured need, or document that bounded role and its exact option set. If adopted, feed
it Pydantic `model_dump(mode="json")` data, retain bytes end-to-end, bound input size, and
never use `OPT_SORT_KEYS` as JCS or `Fragment` for untrusted bytes.

**Focused re-test:** a fresh re-lock retains the declared versions; the generator identity and
descriptor check pass; production imports do not require grpcio-tools; an adopted orjson role
has representative semantic and performance tests, while canonical/protobuf/MCP code has a
negative structural rule preventing orjson substitution.

### IR-007 — Simple artifact-index data is multiplied into hand-rendered language source

**Severity:** minor

**Dimension:** architecture / maintenance

**Design/Plan refs:** SUITE AC-G-05; v4 D-04, WP06; Pydantic ref §§21 and 40

**Evidence:** the same list of `(path, canonical_digest)` pairs is encoded in canonical
`contracts/generated/artifact-index.json`, Rust source, Python source, a Python stub, and a
Python package initializer. `render_rust_index` and `render_python_index` manually assemble
source text. These files carry no behavior or rich static enum/type structure; they mirror
data already present in the canonical JSON authority.

**Failure mode:** harmless formatting/import/stub changes create generated diffs and extra
hash churn. Each new language needs another custom renderer even though the payload is
language-neutral data.

**Remediation:** distinguish generated *types* from generated *data*. Continue generating
Rust/Python types where static exhaustiveness is valuable, but keep the artifact index as one
canonical JSON resource. Rust can package it with `include_bytes!` and deserialize it once
behind `OnceLock`; Python can package the same bytes and validate them once with a module-level
Pydantic `TypeAdapter`. If compile-time constants are proven necessary, drive every renderer
from the typed IR and a common structured emitter rather than ad hoc string concatenation.

**Focused re-test:** Rust and Python load the identical packaged artifact-index bytes, validate
the same digest, and expose equivalent typed views. Generated-output census no longer changes
for language-only wrappers, while true generated compatibility types still compile.

### IR-008 — Aggregate gates repeat compile/test work that independent recipes already expose

**Severity:** minor

**Dimension:** performance / operations

**Design/Plan refs:** accepted build-cache design §§5-7, 12-13; repo command-contract doctrine

**Evidence:** `root-ci-fast`, `extractor-ci-fast`, and `sidecar-ci-fast` run Cargo check, then
Clippy, then tests as separate aggregate dependencies. `adapter-test` already runs every pytest
case including `test_stdio.py`, after which `adapter-stdio-test` runs the same two cases again.
The current global Rust sccache hit rate is 20.92%; the latest sidecar test code-generation
phase took about 62 seconds despite earlier check/Clippy completion.

**Failure mode:** cache configuration cannot remove work caused by distinct Cargo modes,
profiles, features, compiler versions, or repeated test selection. A full local gate pays for
some proofs twice, which makes iteration slower and encourages developers to skip the gate.

**Remediation:** preserve the independent intent recipes, but benchmark a coverage-minimal
aggregate command graph. Candidate changes are:

- avoid a standalone `cargo check` in an aggregate where warning-denied
  `cargo clippy --all-targets` plus the exact test build proves the same surfaces;
- avoid rerunning the STDIO pytest subset after the full adapter suite, while retaining the
  explicit subset recipe for CI granularity and targeted use;
- derive an optional `ci-affected` selection from changed paths plus the artifact/consumer
  graph, while retaining `ci-fast` as the full local/milestone oracle;
- record per-domain/profile timings and sccache deltas so changes are evidence based.

Do not merge stable/nightly/sanitizer target roots or weaken feature/target coverage merely to
improve a warm timing.

**Focused re-test:** use Hyperfine or equivalent controlled before/after runs for cold and warm
aggregate gates; prove the same targets/features/tests and failure fixtures remain covered;
show a material wall-time or compile-request reduction.

### IR-009 — Known-answer hashes must remain independent even as routine digests become derived

**Severity:** observation

**Dimension:** tests / maintenance

**Design/Plan refs:** QRY AC-G-53; v4 WP06-WP10; protobuf ref §§16 and 37; Pydantic ref §48;
FastMCP ref §30

**Evidence:** the user observed repeated expected-hash updates. Some of that churn is caused by
the placeholder/manual authority issues above. However, the JCS corpus and future CBEF/identity
known-answer tests intentionally store independently reviewed expected bytes/digests. Making
the ordinary generator rewrite its own expected values would cause the implementation to
become its own oracle.

**Failure mode:** auto-accepting every changed golden value makes semantic drift green. At the
other extreme, storing exact hashes for every broad generated fixture creates noisy review
churn where structural/property assertions would be stronger.

**Remediation:** maintain two explicit test classes:

- a small, normative KAT corpus whose expected bytes/digests come from an independent
  implementation, specification example, or owner review and are never silently rewritten;
- a broad generated/property/differential corpus that asserts round trip, permutation
  invariance, cross-language equality, descriptor/schema equivalence, and stable failure
  classes without storing incidental hashes.

Routine artifact, provenance, manifest, and generated-tree digests should be computed from the
typed model and never hand-updated. KAT changes should require a contract-version/change record.

**Focused re-test:** mutation of an encoder must break at least one independent KAT; broad
generated cases regenerate without manual hash edits; a deliberate contract change produces a
reviewable KAT/version delta rather than a blanket snapshot acceptance.

## Outcome and invariant matrix

| Implemented outcome/invariant | Assessment | Evidence / required action |
|---|---|---|
| Four build domains and seed cutover | conforming | WP01-WP05 gates and zero-state checks pass. |
| Narrow feature closures | conforming | Contract/proto/fuzz selections omit the heavy data-fabric families; keep this design. |
| Stable target sharing and isolated incompatible roots | conforming | Root/sidecar share stable target; extractor/fuzz are isolated. |
| JCS emission and BLAKE3 form | substantially conforming | Library delegation and cross-language corpus are sound; close IR-001 and IR-003. |
| YAML-to-JSON projection | substantially conforming | Explicit tag/merge/duplicate policy is good; compile typed registry models after projection. |
| Artifact census and generation authority | non-conforming as a scalable design | Close IR-001-IR-003 and IR-007 before adding identity/registry/schema content. |
| Protobuf reproducibility | conforming for Wave-0 probe only | Add descriptor IR and compatibility governance before WP10. |
| FastMCP adapter shell | conforming for Wave 0 | Retain empty catalog; implement SRV's model/schema/fingerprint design through IR-005. |
| Released Gate A | not yet claimed | Correctly remains open; all 50 artifacts are draft and placeholder-filled. |

## Architecture and doctrine assessment

The feature-isolation correction advances information hiding, dependency direction,
reproducibility, and semantic incrementality. The existing split between Rust core, compiler
extractor, Pyrefly sidecar, and Python adapter is justified by toolchain and dependency
boundaries rather than conceptual folder preference.

The current contract compiler, however, violates the spirit of declarative knowledge
single-sourcing (doctrine Principle 10): its authority is spread across file layout, two Rust
lists, output constants, renderer loops, count assertions, package locations, and generated
copies. IR-002 restores one declared model without creating a new Cargo package or a new
runtime service.

The target should be a staged compiler with explicit information hiding:

1. **Ingress adapters** own JSON, YAML, JSONL, EBNF-header, and Protobuf parsing and limits.
2. **Typed Contract IR** owns artifact identity, ownership, derivation edges, compatibility,
   and normalized semantic values.
3. **Validators** own cross-record invariants, append-only rules, traceability, and release
   readiness.
4. **Emitters** own JCS/registry outputs, descriptor sets/stubs, JSON Schemas, and language
   types.
5. **Fingerprint adapters** own artifact, bundle, Pydantic, and FastMCP view comparison.

No emitter may reach back into arbitrary source bytes to discover business facts, and no
runtime adapter may become a second semantic query implementation.

## Library leverage assessment

### Canonicalization stack

The current responsibility split is mostly best-in-class:

| Responsibility | Correct authority | Assessment |
|---|---|---|
| strict Python JSON ingress | stdlib `json` hooks | Correct: `object_pairs_hook`, `parse_int`, `parse_float`, and `parse_constant` are used. |
| strict Rust JSON ingress | Serde visitor + `serde_json::Value` | Duplicate detection is necessarily custom; consider a one-pass value-building visitor only after profiling. |
| RFC 8785 bytes | `serde_json_canonicalizer` / `rfc8785` | Correct; do not replace with sorted ordinary JSON. |
| BLAKE3 | Rust/Python `blake3` | Correct one-shot use. |
| base64url | Rust `URL_SAFE_NO_PAD`, Python strict decode/re-encode | Correct canonical-form validation. |
| registry YAML | pinned `serde_yaml_ng` plus explicit projection | Correct boundary ownership; now add typed post-projection models and limits. |

The Rust JCS path currently performs duplicate-detection parse, value parse, and canonical-output
parse, plus a lexical non-finite-token scanner for cross-language error classification. The
last parse is justified by the fuzz-discovered binary64 rounding case. A one-pass strict
Visitor that constructs `Value` could remove one ingress parse, but only if it preserves
`arbitrary_precision`, duplicate diagnostics, depth behavior, and fixture parity. The lexical
scanner can be deleted if the contract intentionally collapses non-standard NaN/Infinity text
into the ordinary `invalid-json` class. Neither change should precede a representative
benchmark; contract generation is unlikely to be dominated by these small fixture parses.

### Protobuf and gRPC

Use Protobuf binary messages directly at the daemon boundary. Do not convert messages through
dict/JSON and do not use orjson on generated message internals. Use ProtoJSON only where a
separately declared JSON representation is required. Reuse one `grpc.aio` channel/stub for the
adapter lifespan, maintain event-loop ownership, apply per-call deadlines, and centralize
status/metadata conversion. The current 4 MiB symmetric limit is a good control-message
default; large semantic results should use the designed streaming/artifact path rather than
raising the limit reflexively.

Descriptor sets are the missing model leverage. They give the compiler a supported reflection
IR for service, message, field, enum, option, and compatibility checks. They do not replace
generated stubs in production.

### Pydantic and FastMCP

Pydantic V2 already is a schema compiler: Python annotations become CoreSchema, then reusable
Rust validators/serializers. The design should exploit this by compiling models/adapters once,
not by rebuilding validators in handlers or manually walking dictionaries. Use:

- `StrictWireModel` for public request/response/meta DTOs;
- reusable `Annotated` constraints for identifiers and bounded values;
- discriminated unions for closed variants;
- one module-scope `TypeAdapter(dict[str, JsonValue], ...)` for the opaque semantic request;
- explicit validation- and serialization-mode JSON Schemas;
- semantic schema assertions plus normalized snapshots/fingerprints;
- `model_validate_json`/`TypeAdapter.validate_json` when the actual ingress is raw JSON bytes.

FastMCP should own protocol publication. Explicit typed functions are the cleanest source for
four stable tools; a custom Provider becomes valuable only for a dynamic or large catalog.
Use lifespan for the channel/client/handshake snapshot and DI for runtime-only dependencies.
Use transforms for namespacing, visibility/version publication, or catalog search when those
needs arise—not for authorization. Continue using the in-memory Client and add manifest/tool
fingerprint checks at the protocol-facing representation.

### orjson

Maximal library leverage does not mean using every installed library. `orjson` is excellent for
a measured ordinary-JSON bytes boundary, JSONL diagnostics/export, or a validated metadata
cache. It is not JCS, not ProtoJSON, not Protobuf, and not the preferred FastMCP structured
result representation. It holds the GIL during encode/decode, `OPT_SORT_KEYS` is not canonical,
and `Fragment` bypasses validation. Until a bounded role exists, removing/defering it is less
maintenance than inventing a use.

## Legacy and decommission assessment

The WP01 seed/PyO3/root-Python cutover is structurally enforced and passed the current
zero-state gate. No recommendation in this review reintroduces Maturin, PyO3, the old
`python/codefabric` package, a Cargo workspace, or a second production crate.

The generated-index simplification in IR-007 applies only to language-neutral data. It does not
abolish the AC-G-05 generated Rust/Python compatibility types or committed generated artifacts.

## Test and operational assessment

The current tests are unusually strong for a foundation packet: cross-language fixtures,
negative committed fixtures, two-root generation, production-path fuzzing, exact dependency
graphs, protocol-silent subprocess checks, and FastMCP in-memory protocol tests all prove
different risks.

The next quality improvement is to make oracles model-derived without making them
self-fulfilling:

- catalog-derived census and output expectations;
- typed invalid-model fixtures rather than substring examples;
- descriptor compatibility checks rather than generated-text-only comparison;
- Pydantic validation/serialization schema and FastMCP manifest equivalence;
- small independent KATs plus broad property/differential corpora;
- explicit performance budgets for compiler ingest, model build, generation, adapter startup,
  and steady-state validation.

Do not introduce giant byte-for-byte snapshots where semantic assertions are clearer. Do not
auto-accept a KAT or public schema fingerprint merely because the current generator emitted a
new value.

## Plan deviations and diff hygiene

The working tree contains the expected broad uncommitted implementation of WP01-WP06 plus
pre-existing/user-owned changes. This review added only this report. It did not modify the plan,
state, design corpus, implementation, generated outputs, lockfiles, or fixtures.

The execution state accurately records failed compatibility approaches and the current WP07
boundary. The number of corrections in those records is not by itself evidence of poor
implementation: several are legitimate probes against compiler-private APIs, pinned
third-party packages, or fuzz-discovered behavior. The avoidable portion is the repeated
manual authority update exposed by IR-001-IR-003 and IR-007.

## Required remediation order

1. **Correct AC-G-02 and the plan** with an explicit non-self-referential digest projection.
2. **Design and implement the typed artifact catalog/Contract IR** and derive census,
   generation, warning, and consumer data from it.
3. **Replace lexical verification with typed models and resource budgets.** Migrate the
   existing WP06 sources/fixtures without changing JCS semantics.
4. **Regenerate and independently verify all 50 real source digests.** Remove zero sentinels.
5. **Resume WP07** by expressing CBEF/path/type-algebra recipes as typed IR inputs, deriving
   broad fixtures and retaining a small owner-reviewed KAT corpus.
6. **Before WP10**, add descriptor-set generation, normalized compatibility checks, and exact
   protocol compiler/runtime declarations.
7. **Before adapter contract/tool implementation**, wire generated Pydantic models, cached
   adapters, FastMCP DI/lifespan, and schema/manifest fingerprint gates.
8. **Separately benchmark aggregate command choreography** and remove only proven duplicate
   work.

## Focused re-review scope

A follow-up review should re-test IR-001 through IR-003 before WP07 execution resumes. It should
then verify IR-004 before production `.proto` contracts land and IR-005 before public FastMCP
tools are registered. IR-006-IR-008 may be closed in those same changes or tracked as bounded
maintenance work. IR-009 remains a standing test-oracle rule rather than a code defect.
