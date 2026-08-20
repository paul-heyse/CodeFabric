---
artifact: implementation-plan
plan_id: codefabric-model-based-foundation-remediation
version: v1
date: 2026-08-20
status: draft
design_path: docs/reviews/implementation_review_codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20_2026-08-20_v1.md
design_version: v1
baseline_commit: a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91
state_path: docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json
cutover: true
---

# CodeFabric Model-Based Foundation Remediation — Implementation Plan v1

This is the corrective successor overlay for the implementation completed through
WP06 of the CodeFabric Waves 0–3 foundation plan v4. It converts review findings
IR-001–IR-009 into dependency-closed implementation packets without copying or
renumbering the unaffected 3,174-line v4 plan.

The two plans compose as follows:

```text
v4 WP01–WP06 completed evidence
             |
             v
this plan WP01–WP09 + M01–M04 + DB01–DB04
             |
             v
v4 WP07 resumes under this plan's standing acceptance overlays
             |
             +--> v4 WP09 consumes the adapter-model compiler
             +--> v4 WP10 consumes the descriptor-set compiler
             +--> v4 WP11/M02 certifies the corrected Gate-A substrate
```

The v4 plan remains the authority for unaffected Wave 1–3 outcomes and packets.
Where this plan names an explicit v4 acceptance overlay, this plan takes precedence
for that behavior. It does not rewrite v4 history or treat the corrected tree as
proof that the original WP06 implementation was correct under the new contract.

---

## 1. Outcome and non-goals

### 1.1 Outcome

At completion:

1. The machine-contract digest rule is non-self-referential and distinguishes
   semantic identity from exact source-byte identity. All required source artifacts
   carry real semantic digests; no zero sentinel remains.
2. `contracts/manifests/suite-manifest.json` is the only contract-compiler bootstrap
   authority. A typed `ContractCatalog` derives the source census, output graph,
   ownership, parsers, resource profiles, provenance, consumers, release counts, and
   generated-file governance.
3. Format-specific ingress compiles native authorities into a typed Contract IR under
   explicit byte, depth, node, collection, string/token, alias, record/edge, and
   diagnostic budgets. Byte-substring metadata/status checks and unbounded generic
   traversal no longer decide conformance.
4. The language-neutral artifact index exists once as canonical JSON data. Rust and
   Python package and validate the same bytes once; generated language source remains
   only where static type exhaustiveness has value.
5. A single exact `grpcio-tools` compiler invocation emits the committed Protobuf
   `FileDescriptorSet` and Python bindings. Rust generation consumes that descriptor
   set through `prost-build`/`tonic-prost-build`; a second `protoc` interpretation is
   removed.
6. The adapter contract compiler emits statically typed Pydantic 2.13.4 models,
   validation and serialization schemas, cached adapters, and reusable FastMCP
   fingerprint machinery from the Contract IR. Later stable public tools use explicit
   typed handlers and FastMCP's native local-provider path.
7. Python protocol/compiler versions are exact in the manifest. `orjson` is removed
   until a measured, bounded ordinary-JSON role exists; JCS, ProtoJSON, Protobuf, and
   FastMCP structured results keep their native authorities.
8. Normative known-answer tests remain independent oracles. Broad routine coverage is
   generated/property/differential proof and does not require hand-maintained hashes.
9. Aggregate command changes are backed by cold/warm measurements and an equivalent
   proof-coverage manifest. Independent intent recipes and full `ci-fast` remain.
10. A focused independent re-review closes IR-001–IR-003 before v4 WP07 resumes and
    records bounded downstream obligations for Wave 17's `DaemonClient` and Wave 18's
    real four-tool FastMCP manifest.

### 1.2 Non-goals

- Reimplement RFC 8785 member sorting, string escaping, or number rendering.
- Replace `.proto`, JSON Schema, registry YAML, or EBNF with a universal schema.
- Add another Cargo package, workspace, runtime service, or Python implementation of
  daemon semantics.
- Implement the production daemon client, RPC handlers, semantic query engine, or the
  four public FastMCP tools before their roadmap waves.
- Treat deterministic Protobuf bytes as canonical or signing bytes.
- Auto-accept owner-reviewed KATs, public schemas, or tool fingerprints.
- Remove Rust `check`, weaken feature/target coverage, merge incompatible target roots,
  or make `ci-affected` the release oracle.
- Revisit the accepted cache/feature isolation design, license policy, or unaffected
  Wave 2/3 publication and snapshot sequencing.

### 1.3 Completion boundary

This plan completes when M04 is green and v4 WP07 is released from its interlock. The
actual four-tool manifest and production `DaemonClient` remain explicit downstream
obligations because roadmap §6 defers RPC behavior and public tools. Their design and
acceptance contracts are fixed here so later packets cannot recreate them ad hoc.

---

## 2. Source design and governing decisions

### 2.1 Inputs and staleness digests

| Input | SHA-256 at planning |
|---|---|
| implementation review v1 | `f48caf54239752324a979eb26a9c435888d6cf3b1974d32b762da5e783eede72` |
| Waves 0–3 plan v4 | `598b4971574c245cfd4f3f560ad52e2838eef884d21ea4c77c9233c70ad3d3db` |
| v4 execution state | `621bb157dfdae47876ae926c82909ab4e099cf102480e28ea6fb5906eaac2343` |
| SUITE governance manifest v1.3 | `b0054314f9c5e4360476b053f98bdaa43857e2611e23d4fd026c6c5e4c0b6985` |
| Serving specification v1.3 | `8ee8ec7dbc06b1a8ac1b03d882abe33fef5d3dbe70e35446e56dbfd399f25524` |
| implementation roadmap v1.0 | `749a032a21875589ea5e15eb850cb4008269856d6a3aeba9df3ef1ef6ee216fd` |
| cache/feature isolation design v1 | `460e2f36a8a61a9972c976adbd20e1a86b82a26cd7bd38840cb1565948475e8f` |

Also governing are Query AC-G-53, the repository specification, and
`docs/library_ref/semantic_design_principles_holistic.md`. The source review is treated
as accepted remediation intent because the user asked that every finding be actioned.
WP01 corrects the owning normative documents before production code consumes the new
rules.

### 2.2 Governing decisions

#### D-01 — Two identities, two questions

`canonical_digest` identifies the artifact's compiled semantic contract. It is
BLAKE3-256 over the artifact kind's versioned canonical projection with digest metadata
omitted. `source_digest` identifies the exact checked-in bytes and is emitted in the
generated artifact index/provenance view rather than embedded into a source it hashes.

Consequently:

- a semantic change changes both identities;
- a comment/formatting-only change changes `source_digest` but not necessarily
  `canonical_digest`;
- AC-G-02's current sentence that every editorial change changes “the digest” is
  corrected to name `source_digest`;
- AC-G-07 `bundle_digest` remains a third, distinct projection over a sorted bundle
  record with `bundle_digest` and `signature` omitted.

Each catalog record selects one closed projection profile:

| Profile | Canonical semantic projection |
|---|---|
| `json-jcs-v1` | strict JSON root model, digest fields omitted, RFC 8785 bytes |
| `yaml-ac-g-53-v1` | pinned YAML 1.1 projection, digest fields omitted, RFC 8785 bytes |
| `jsonl-jcs-v1` | typed metadata/records, digest field omitted, each record JCS plus LF framing |
| `proto-descriptor-v1` | normalized typed descriptor model derived from the single committed descriptor set |
| `ebnf-source-v1` | parsed/validated metadata header plus exact LF-normalized grammar payload; no arbitrary text replacement |
| `bundle-ac-g-07-v1` | the existing AC-G-07 bundle projection only |

The projection ID is part of the artifact contract. Changing a profile or its emitted
bytes is a contract-version event, never a silent generator refactor.

#### D-02 — One bootstrap authority, native semantic authorities

`contracts/manifests/suite-manifest.json` becomes the typed `ContractCatalog` and is the
only path hard-coded into the compiler. It describes artifacts and derivation edges; it
does not replace their native semantics. Its own record uses the same catalog/header
model and `json-jcs-v1` self-projection. Computed digests and release observations live
in the generated artifact index so the catalog does not contain a second self-referential
computed census.

#### D-03 — A staged compiler, not shared dynamic values

The contract compiler is divided into ingress, typed Contract IR, cross-record
validators, emitters, and fingerprint adapters. Closed records use typed Serde models
with unknown-field rejection. `serde_json::Value` and `serde_yaml_ng::Value` are limited
to the generic projection seam. No emitter rediscovers ownership by scanning arbitrary
source bytes.

#### D-04 — One Protobuf compilation

`.proto` remains the source authority. Exact `grpcio-tools==1.83.0` invokes its bundled
compiler once with `--descriptor_set_out` and `--include_imports`, without source info in
the semantic descriptor. Python stubs and the committed descriptor come from that run.
Rust decodes the same `FileDescriptorSet` and calls
`prost_build::Config::compile_fds` or
`tonic_prost_build::Builder::compile_fds_with_config`. The second vendored `protoc`
invocation and its production dependency are deleted after equivalence is proven.

#### D-05 — Pydantic and FastMCP compile their own boundary views

The Contract IR emits statically typed Python model source, not competing handwritten
adapter JSON Schemas. Imported Pydantic models compile CoreSchema once and emit separate
validation/serialization JSON Schemas. FastMCP publishes explicit typed handler
signatures. The protocol-facing fingerprint input is the selected, documented subset of
`tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)` and is
hashed with CodeFabric JCS+BLAKE3.

The current empty Wave-0 catalog remains correct. WP07 builds and proves the compiler and
fingerprint substrate with contract/test components; Wave 18 supplies the four real tools.

#### D-06 — Exact Python protocol intent; no speculative orjson role

The manifest declares `grpcio==1.83.0`, `protobuf==7.36.0`, and build-only
`grpcio-tools==1.83.0`. `pydantic-core` remains Pydantic-owned. `orjson` is removed from
the manifest and lock because its two current Serving-spec roles are not sound: sorted
ordinary JSON is not JCS, and schema export is not a measured performance boundary.
Re-adoption requires a named ordinary-JSON/JSONL/cache/log boundary, exact option mask,
size limit, semantic fixtures, and a benchmark.

#### D-07 — Tests separate immutable oracles from derived evidence

Normative KATs carry origin, contract version, owner approval, and expected bytes/digest
from an independent authority. The generator may stage candidate values for review but
cannot approve or overwrite them. Broad cases assert properties, cross-language equality,
schema/descriptor equivalence, and stable failure classes without incidental hashes.

#### D-08 — Command optimization is a proof-preserving transformation

Independent recipes remain the command API. Full `ci-fast` remains the milestone oracle.
Tier-A Rust `check` and Clippy both remain unless the repository specification itself is
changed through design review. The duplicate STDIO pytest aggregate and other candidates
change only after controlled cold/warm measurement and proof-coverage equivalence.

#### D-09 — Historical proof is preserved, not relabeled

V4 WP01–WP06 and DB01 remain completed historical evidence. WP06's affected surfaces are
revalidated by this plan; its original checks are not retroactively presented as proof of
the corrected design. The v4 state is interlocked at WP07 until M04.

### 2.3 Finding disposition

| Finding | Disposition in this plan | Closure boundary |
|---|---|---|
| IR-001 | applied-design WP01; implementation WP03/WP04 | M02 + focused re-review |
| IR-002 | applied-design WP01; implementation WP02/WP04 | M02 |
| IR-003 | implementation WP03; validator selection WP01/WP03 | M02 |
| IR-004 | descriptor/compiler closure WP05; runtime client obligation DO-01 | M03; Wave 17 final |
| IR-005 | compiler/fingerprint substrate WP07; real tool obligation DO-02 | M03; Wave 18 final |
| IR-006 | exact pins and orjson decommission WP01/WP05 | M03 |
| IR-007 | artifact-index cutover WP04 | M02 + DB02 |
| IR-008 | controlled command benchmark WP08 | M04 |
| IR-009 | standing oracle rule WP06 | M02 and every later contract packet |

### 2.4 Doctrine impact

This plan advances information hiding (typed format adapters), high cohesion/low
coupling (staged compiler), dependency direction (native authorities feed consumers),
declarative single-sourcing (one catalog), reproducibility and semantic incrementality
(distinct semantic/source identities), declared public contracts, testability, and
additive extensibility. The primary risk is centralizing too much in the catalog; D-02
limits it to ownership and derivation rather than replacing format-native schemas.

---

## 3. Current baseline and staleness boundary

- HEAD is `a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91` on `master`.
- The working tree is intentionally broad and dirty from v4 WP01–WP06. Its planning-time
  porcelain-status digest is
  `0f2d62a258b38ab7e27f7e70602f5eef93fd79f8024825e9c8011956bd3ad80a`.
  Execution must capture a new successor-adoption inventory and digest before WP01; it
  must not overwrite user or prior-packet work.
- V4 state is `executing`, `current_packet: WP07`; WP01–WP06 are complete. WP06 has no
  proving commit/working-tree digest, so this plan binds the current relevant tree before
  modifying it and preserves its evidence as historical only.
- V4 M01 is still `in_progress`: a retained-source clean macOS rebuild passed, while the
  GitHub Actions Ubuntu clean-checkout run is pending external authorization/state.
  WP07 re-entry requires that external clean-checkout result or an accepted plan revision;
  this plan does not silently waive it.
- Pre-edit `just ci-fast` passed on 2026-08-20 after the review was written: root stable
  and featureless checks/Clippy, 21 Nextest tests and doctests; extractor tests/identity;
  sidecar checks/tests; adapter Ruff/Pyrefly/46 pytest cases plus the repeated STDIO
  subset; and governance, Protobuf, full draft contract verification, negative fixtures,
  and two-root generation all passed.
- Non-failing baseline warnings are a future-incompatibility notice for
  `proc-macro-error2` and a macOS compact-unwind linker warning. They are recorded, not
  attributed to this plan.
- Current contract compilation reports 50 source artifacts and 50 draft warnings. Every
  required source uses the zero digest sentinel. `artifacts.rs` separately hard-codes 50
  sources, 13 registry sources, output paths, renderers, lexical metadata/status scans,
  and literal count assertions.
- Python's lock resolves the relevant exact versions today, but the manifest leaves
  gRPC/Protobuf/compiler intent open. Rust currently resolves BLAKE3 1.8.7; the 1.8.6
  reference is behavioral guidance, not authority to downgrade the lock.
- Current Protobuf generation invokes libprotoc 35.1 through grpcio-tools and libprotoc
  31.1 through `protoc-bin-vendored`. The generated Python probe records gencode 7.35.1;
  the Python runtime is 7.36.0.

### 3.1 Staleness triggers before execution

Reconcile or revise this plan before editing if any of these changed:

- review, v4 plan/state, SUITE AC-G-02/05/07, Query AC-G-53, roadmap §6, or Serving
  §§18/19/33/58/60/70;
- current v4 packet moved beyond WP07 or WP07 gained implementation changes;
- exact canonicalization, Protobuf/gRPC, Pydantic/FastMCP, or JSON-Schema pins;
- the catalog/source/output census, generated package paths, or command recipes;
- the one-package Cargo topology, feature-isolation design, or Python adapter root.

---

## 4. Global target invariants

- **I-01 — Dual identity.** Every required source has one real semantic digest under a
  declared projection and one exact source digest in the generated index. Neither is
  self-referential or silently interchangeable with a bundle digest.
- **I-02 — One catalog.** The suite manifest is the sole artifact/derivation bootstrap;
  all source/output censuses and consumer edges derive from it.
- **I-03 — Native authority.** JSON Schema, `.proto`, registry YAML, and EBNF retain
  their native semantics; Contract IR describes their common ownership and derivation.
- **I-04 — Typed closed records.** Headers, catalog descriptors, registries,
  requirements, traceability, provenance, and compatibility records reject unknown
  fields and produce path-aware diagnostics.
- **I-05 — Bounded compilation.** Limits apply before full allocation and after parse;
  no alias expansion, recursion, collection, token, graph, or diagnostic path is
  accidentally unbounded.
- **I-06 — Library-owned canonical bytes.** RFC 8785 emission remains solely
  `serde_json_canonicalizer`/`rfc8785`; application code owns domain checks and framing.
- **I-07 — Generated data once.** Language-neutral data is one canonical resource;
  generated source is reserved for statically useful types/behavior.
- **I-08 — One compiled Proto IR.** Both language binding paths consume one committed
  descriptor set from one exact compiler invocation.
- **I-09 — Schema modes are distinct.** Pydantic validation and serialization schemas
  are generated, compared, and fingerprinted separately.
- **I-10 — Compile once, reuse hot paths.** Pydantic models/TypeAdapters and gRPC
  channels/stubs are lifecycle-owned, never built per request.
- **I-11 — Thin adapter.** Pydantic validates adapter-owned DTOs; the daemon retains
  semantic-query validation and execution authority.
- **I-12 — Independent oracle.** No production generator is the sole source of its own
  normative KAT expected values.
- **I-13 — Exact protocol intent.** Manifest and lock agree on accepted
  gRPC/Protobuf/compiler families; build-only tools do not become runtime imports.
- **I-14 — Proof-preserving performance.** Command optimization retains the same target,
  feature, test, and negative-fixture coverage.
- **I-15 — Historical integrity.** V4 completed evidence remains attributable to its
  original design; corrected evidence is recorded under this plan.

---

## 5. Library decisions carried into execution

| ID | Decision | Exact basis and constraints |
|---|---|---|
| LD-01 | Rust canonical bytes use `serde_json_canonicalizer=0.3.2`; strict ingress uses `serde_json=1.0.151` with `arbitrary_precision` | Library owns UTF-16 member order, escaping, and `ryu-js` number rendering. Duplicate evidence and safe-domain checks precede it. |
| LD-02 | Python canonical bytes use `rfc8785==0.1.4`; strict ingress uses CPython 3.14.7 `json` hooks | Compare bytes/raw BLAKE3/framed digest; compare failure classes, not exception text. |
| LD-03 | BLAKE3/base64 stay library-owned | Rust lock resolves `blake3=1.8.7`; Python `blake3==1.0.9`; Rust `base64=0.22.1` `URL_SAFE_NO_PAD`. No keyed/XOF digest mode. |
| LD-04 | YAML uses `serde_yaml_ng=0.10.0` plus application projection | Reject tags/merge keys; aliases are accepted only if a proven pre-expansion bound exists, otherwise WP03 changes policy to reject them through design correction. |
| LD-05 | Typed Serde models are the Rust contract compiler | `deny_unknown_fields` for closed records; dynamic `Value` only at projection seams. A one-pass value-building visitor is benchmark-only, not required. |
| LD-06 | Draft 2020-12 metaschema checks use an explicit validator | Preflight promotes locked `jsonschema==4.26.0` to a direct build-only adapter/tooling dependency and proves `Draft202012Validator.check_schema`; if cross-domain invocation cannot be hermetic, perform a bounded library-capability decision for an exact Rust validator before implementation. |
| LD-07 | Protobuf/gRPC pins are exact | `grpcio==1.83.0`, `grpcio-tools==1.83.0`, `protobuf==7.36.0`; one descriptor-set compile; use generated `DESCRIPTOR`/`DescriptorPool` for Python assertions. |
| LD-08 | Rust generation consumes descriptor IR | Existing `prost-build=0.14.4` `Config::compile_fds` or `tonic-prost-build=0.14.6` `compile_fds_with_config`; no second production `protoc` run. |
| LD-09 | Pydantic is the adapter schema compiler | `pydantic==2.13.4`, `pydantic-settings==2.15.0`; no independent `pydantic-core` pin; module-scope models/adapters; discriminated unions and `Annotated` constraints before custom core schemas. |
| LD-10 | FastMCP owns publication | `fastmcp==3.4.7`; explicit typed functions for four stable tools; lifespan/DI for runtime-only state; custom Provider only for a genuinely dynamic catalog. |
| LD-11 | `orjson` is decommissioned | It is neither JCS nor ProtoJSON and is unnecessary for schema pretty-printing or MCP structured results. Re-adoption is a new measured LD decision. |
| LD-12 | Artifact index consumers use native caches | Rust `include_bytes!` + `OnceLock` typed decode; Python `importlib.resources` + one module-level `TypeAdapter`. Both validate identical bytes/digest. |
| LD-13 | Hyperfine measures command changes | Cold tests use fresh explicit target directories, never `cargo clean`; warm tests fix machine/load and report distributions plus sccache deltas. |

No LD decision authorizes a new crate/package. The root's narrow capability features and
separate incompatible target roots remain unchanged.

---

## 6. Work packets

## WP01 — Correct normative authority and interlock v4 execution

### Outcome

The owning design documents state D-01–D-08 unambiguously; derived indexes are
restamped; v4 execution is durably blocked at WP07 until M04; and no production code
consumes a packet-local invention.

### Dependencies

V4 WP06 complete. User acceptance of this plan is required before execution.

### Target Invariants

I-01–I-03, I-08–I-11, I-13, I-15.

### Design and Library References

Review IR-001–IR-009; SUITE AC-G-01/02/05/07; roadmap §6; Query AC-G-53;
Serving §§18, 19, 33, 58, 60, 70–71; LD-01–LD-12; doctrine Principles 1, 5,
10, 25, 29, 31.

### Change Surface

#### Must Touch — Verified

- `docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`
- `docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md`
- `docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md`
- derived `docs/spec_index/` files affected by changed ownership/routing/traceability
- v4 execution state interlock and this plan's future state record

#### Likely Touch — Impact Candidates

- Query wording if request-byte ownership is not already explicit at AC-G-53
- `CLAUDE.md`, `AGENTS.md`, and README contract/compiler maps

#### Discover at Packet Preflight

- Use `just spec-outline` for each owning section and enumerate every reference to
  “canonical digest”, “orjson”, “three independent schema families”, descriptor
  generation, and the WP07 dependency boundary.
- Recompute the source-design hashes and compare current v4 state/current packet.

### Required Changes

1. Revise AC-G-02 with semantic/source identity names, projection IDs, version rules,
   and the no-self-reference rule. Add worked JSON/YAML/JSONL/Proto/EBNF/bundle cases.
2. Revise AC-G-05 so `suite-manifest.json` is the typed catalog authority and the
   generated artifact index carries computed semantic/source digests.
3. Revise roadmap §6 to require the staged typed compiler, budgets, one descriptor IR,
   generated-data/type distinction, and independent KAT policy.
4. Remove `orjson` from Serving's dependency list and examples. Serialize semantic
   request bytes with shared RFC 8785; export human-readable schemas with the standard
   library/Pydantic and fingerprint canonical schemas with RFC 8785+BLAKE3.
5. Correct Serving §70 from “three” to four independent families and freeze the
   protocol-facing FastMCP fingerprint inclusion policy.
6. Record DO-01 and DO-02 in roadmap/traceability: Wave 17 owns the production
   `DaemonClient`; Wave 18 owns real tool manifests and typed public handlers.
7. At execution-state initialization, mark v4 WP07 blocked by this plan M04. Preserve
   prior packet records unchanged; do not mark WP06 stale until its exact corrected
   surfaces are enumerated.

### Legacy Disposition and Decommission

Old ambiguous AC-G-02 projection and incorrect Serving orjson examples are superseded.
No compatibility alias exists for digest meanings.

### Acceptance Evidence

#### Behavioral

- Worked vectors distinguish semantic, source, and bundle digests.
- A formatting-only example changes only source identity; a semantic edit changes both.

#### Structural

- One owner for catalog, digest profiles, Proto IR, Pydantic views, FastMCP publication,
  and downstream client lifecycle.
- All changed design digests and derived index references are restamped.

#### Negative / Zero-State

- No design text calls `OPT_SORT_KEYS` canonical.
- No “three families” wording precedes a four-item list.
- No plan/state path permits v4 WP07 before M04.

#### Operational

- Design-only diff passes scoped Typos and link/citation checks.

### Edit-Local Gates

`just spec-outline` checks; scoped `typos`; `git diff --check`.

### Packet-Local Gates

Design consistency search across all eight authoritative artifacts and derived indexes;
independent review of the projection examples.

### Integration Milestone

M01.

### Replan Triggers

- Owners reject dual identity or suite-manifest catalog authority: reopen design.
- Section changes invalidate AC-G ownership or v4 packet boundaries: revise plan.

### Rollback or Recovery

Design edits are atomic and revertible before code packets begin. Do not partially adopt a
new projection.

### Design-Bearing Contracts and Exemplars

```text
canonical_digest = "b3:" + hex(blake3(canonical_projection(profile_id, artifact)))
source_digest    = "b3:" + hex(blake3(exact_checked_in_bytes))
```

## WP02 — Compile the suite catalog into a typed derivation graph

### Outcome

The compiler has one bootstrap path and derives every source, output, consumer, warning,
provenance, packaging, and governance obligation from a validated typed graph.

### Dependencies

WP01.

### Target Invariants

I-02–I-05, I-07, I-15.

### Design and Library References

Review IR-002/003/007; SUITE AC-G-01/02/04/05; roadmap §6; LD-05; doctrine
Principles 1, 4, 10, 25, 29, 31.

### Change Surface

#### Must Touch — Verified

- `contracts/manifests/suite-manifest.json`
- `src/contracts/artifacts.rs` and new cohesive modules beneath `src/contracts/`
- contract generator/verifier tests and negative fixtures
- generated-file and packaging governance inputs

#### Likely Touch — Impact Candidates

- `src/bin/codefabric-contracts.rs`, `src/contracts/mod.rs`
- scripts/just/CI recipes that consume generated source/output lists
- `contracts/manifests/{requirements,traceability}.jsonl`

#### Discover at Packet Preflight

- Enumerate every consumer of `REQUIRED_SOURCE_ARTIFACTS`, `REGISTRY_SOURCES`, output
  constants, literal `50`/`13`, and generated artifact-index mirrors.
- Build a complete ID/path/output/consumer relation from the current tree and reconcile
  it with AC-G-05 before editing.

### Required Changes

1. Define closed `ContractCatalog`, `ArtifactDescriptor`, `GeneratedOutput`,
   `ConsumerDomain`, `CompatibilityFamily`, `ProvenanceRequirement`,
   `ResourceBudgetProfile`, and `DigestProjection` models.
2. Validate unique artifact IDs, authority paths, output paths, and owners; reject
   unknown kinds/profiles, missing sources, cycles, conflicting edges, path escape, and
   outputs with multiple authorities.
3. Derive required-source/output census, registries, generator dispatch, package data,
   warning/release counts, index records, traceability joins, and generated governance.
4. Make catalog ordering irrelevant to semantics while preserving deterministic
   diagnostic and output order.
5. Provide a synthetic in-memory/test catalog constructor; production loads only the
   one bootstrap path.

### Legacy Disposition and Decommission

Remove the hard-coded source/registry/output authorities and literal-count assertions in
DB02 once catalog equivalence is green. Do not remove generated compatibility types.

### Acceptance Evidence

#### Behavioral

- Adding one synthetic descriptor derives census, digest job, output, provenance,
  package-data, and consumer view without any second edit.
- Reordering catalog records does not change compiled output bytes.

#### Structural

- Exactly one production bootstrap path.
- Every current AC-G-05 source/output has one typed descriptor and one authority.

#### Negative / Zero-State

- Fixtures reject duplicate IDs/paths/outputs, cycles, unknown kinds/profiles, missing
  authorities, path escapes, and multi-authority outputs.
- No consumer directly walks directory layout to infer ownership.

#### Operational

- Catalog compile time and peak input size are recorded as a baseline.

### Edit-Local Gates

Narrow root format/check/Clippy and targeted catalog tests.

### Packet-Local Gates

`just contracts-verify`; contract negative suite; catalog property tests; two-root
catalog compilation.

### Integration Milestone

M02.

### Replan Triggers

- A native artifact cannot be described without duplicating its semantic schema: narrow
  the descriptor rather than widen Contract IR.
- Parallel v4 WP08b/WP09/WP10 would mutate shared catalog records: predeclare all
  descriptors now or serialize those packets in a plan revision.

### Rollback or Recovery

Retain the prior reader only inside the packet branch until equivalence is green. No
released dual authority is permitted.

## WP03 — Implement bounded typed ingress and digest profiles

### Outcome

Every artifact family compiles through a typed, resource-bounded adapter and produces a
verified semantic digest plus exact source digest with stable path-aware failures.

### Dependencies

WP02 and WP05's descriptor-IR core for `proto-descriptor-v1`. WP03 and the non-Proto
parts of WP05 may develop in parallel; their packet gates meet before WP04.

### Target Invariants

I-01, I-03–I-06, I-08, I-12.

### Design and Library References

Review IR-001/003/009; SUITE AC-G-02/05/07; Query AC-G-53;
canonicalization pack; LD-01–LD-06.

### Change Surface

#### Must Touch — Verified

- Rust strict JSON/YAML/artifact compiler modules
- JSONL, JSON Schema, Proto descriptor, and EBNF format adapters
- `contracts/fixtures/negative/` and new budget/projection fixtures
- Python cross-language canonicalization/digest tests

#### Likely Touch — Impact Candidates

- root feature definitions if schema-validator tooling is added
- adapter dev dependencies and locked environment for metaschema checks
- fuzz targets/corpora for production ingress adapters

#### Discover at Packet Preflight

- Measure current maximum source size/depth/nodes/records/strings and set named limits
  with explicit headroom; do not invent limits from intuition.
- Probe `jsonschema==4.26.0` metaschema API and hermetic invocation.
- Probe whether `serde_yaml_ng` can bound alias expansion before materialization. If it
  cannot, reject aliases in the accepted design or select a proven bounded parser.

### Required Changes

1. Define typed `ArtifactHeader`, requirement, traceability, provenance, registry, and
   format-specific metadata models with closed fields.
2. Read through an explicit byte cap (`limit + 1` sentinel) before full allocation;
   enforce parser depth/recursion where native controls exist.
3. Count post-parse semantic nodes, collection sizes, strings/tokens, records/edges, and
   accumulated diagnostics under named catalog profiles.
4. Implement closed digest-profile dispatch. Omit digest metadata structurally, never by
   arbitrary text replacement.
5. Validate every JSON Schema's `$schema`, stable `$id`, and Draft 2020-12 metaschema.
6. Preserve strict duplicate and numeric-token evidence before maps/numeric conversions;
   continue library-owned RFC 8785 emission and BLAKE3 framing.
7. Emit stable error class, artifact path, data path/record number, limit name, observed
   value, and configured maximum without dumping unbounded input.

### Legacy Disposition and Decommission

Remove `has_metadata`, `is_draft`, raw substring status checks, and ad hoc `Value` field
lookups in DB02 after typed equivalence.

### Acceptance Evidence

#### Behavioral

- Per format: valid digest, semantic mutation failure, digest mutation failure,
  formatting-only source-digest change, and repeatable regeneration.
- Rust/Python JCS assertions cover identical bytes, raw BLAKE3, and `b3:` framing.

#### Structural

- Dynamic values occur only at documented projection seams.
- Every format/profile selects one parser and one resource budget.

#### Negative / Zero-State

- Comments/nested strings cannot impersonate headers; unknown fields/statuses fail.
- At-limit succeeds and just-over-limit fails for bytes, depth, nodes, cardinality,
  string/token, records/edges, aliases, and diagnostics.
- Duplicate keys, unsafe numbers, tags/merge keys, malformed digest headers, invalid
  schemas, and extra JSONL data fail with bounded diagnostics.

#### Operational

- Representative ingress benchmark records wall time, allocation/peak RSS where
  practical, and diagnostic cap behavior.

### Edit-Local Gates

Narrow contract feature format/check/Clippy/tests; adapter Ruff/Pyrefly and digest tests.

### Packet-Local Gates

Contract negative suite; bounded fuzz replay for each untrusted parser; shared JCS
corpus; metaschema suite; two-root digest reproduction.

### Integration Milestone

M02.

### Replan Triggers

- No pre-expansion YAML alias bound: design correction to reject aliases or library
  research before continuation.
- EBNF validation requires a custom grammar parser larger than the artifact contract:
  perform bounded library capability research and revise the profile.
- Metaschema validation cannot remain build-only/hermetic: select a Rust validator via
  plan revision.

### Rollback or Recovery

Keep old verification only during packet-local differential tests; it is not a fallback
after cutover.

## WP04 — Migrate all artifacts and consolidate the artifact index

### Outcome

All required artifacts and outputs are catalog-derived, all embedded semantic digests are
real, the zero sentinel is absent, and Rust/Python consume one canonical artifact-index
resource.

### Dependencies

WP02, WP03, WP05 descriptor IR.

### Target Invariants

I-01–I-08, I-12, I-15.

### Design and Library References

Review IR-001/002/007; SUITE AC-G-02/05/07; LD-12.

### Change Surface

#### Must Touch — Verified

- all required source artifacts under `contracts/`
- `contracts/generated/artifact-index.json`
- Rust/Python artifact-index consumers and package configuration
- obsolete generated `_contract_index` Rust/Python source/stub/init mirrors
- generator, verifier, integration tests, and generated-file governance

#### Likely Touch — Impact Candidates

- bundle sources/fixtures whose member digests change
- documentation that quotes current artifact/output counts

#### Discover at Packet Preflight

- Catalog-derived exact list of affected sources/outputs.
- Complete reference search for generated index symbols and direct file paths.

### Required Changes

1. Compute and write all real embedded semantic digests through the generator; generate
   exact source digests into the index.
2. Verify embedded values on every `verify` run; mutation of semantic content or the
   embedded value must fail.
3. Emit one canonical JSON artifact index with projection ID, semantic digest,
   source digest, owner, version/status, provenance, and consumers.
4. Rust packages the bytes with `include_bytes!` and decodes once behind `OnceLock`.
   Python packages the same bytes via `importlib.resources` and validates once through a
   module-scope `TypeAdapter`.
5. Retain generated Rust/Python types only for enum/registry/protocol/model surfaces that
   gain static exhaustiveness.
6. Update bundle member digests using their distinct AC-G-07 projection.

### Legacy Disposition and Decommission

Execute DB02: hard-coded census/output authorities, lexical checks, zero digests, and
hand-rendered language-neutral index mirrors reach zero.

### Acceptance Evidence

#### Behavioral

- Rust and Python expose equivalent typed records from byte-identical packaged bytes.
- Semantic/source/bundle mutation cases fail at their correct layer.

#### Structural

- Catalog-derived source count equals the AC-G-05 required set without literal count
  assertions.
- One artifact-index data authority and zero language-source mirrors.

#### Negative / Zero-State

- Complete-source search finds no `b3:` zero sentinel.
- No `REQUIRED_SOURCE_ARTIFACTS`, `REGISTRY_SOURCES`, `render_rust_index`,
  `render_python_index`, `has_metadata`, `is_draft`, or equivalent authority pattern.

#### Operational

- Two isolated roots regenerate byte-identically; package installs/imports locate the
  same canonical resource; no stale generated file survives.

### Edit-Local Gates

Generator unit/integration tests; root and adapter focused checks.

### Packet-Local Gates

`just contracts-verify`; negative and reproduction recipes; root/adapter gates; package
resource import test; DB02 structural rules.

### Integration Milestone

M02.

### Replan Triggers

- A consumer requires compile-time constants rather than once-cached data: prove the
  need and generate from typed IR through a structured emitter; do not restore hand
  mirrors.

### Rollback or Recovery

Generate into isolated roots and perform one atomic source/output migration. Never commit
a mixed zero/real-digest tree.

## WP05 — Establish one Protobuf descriptor IR and exact Python dependency intent

### Outcome

One exact compiler produces the committed descriptor set and Python bindings; Rust
generation consumes that descriptor; manifest/lock intent is exact; and unused orjson and
the second protoc path are gone.

### Dependencies

WP01 for normative decisions. Descriptor-core work may proceed in parallel with WP02.

### Target Invariants

I-01, I-03, I-08, I-13, I-15.

### Design and Library References

Review IR-004/006/009; SUITE AC-G-03/05; protobuf ref §§0, 2, 4, 7, 16,
20–21, 26–30, 37–39, 42–44; gRPC ref §§1–3, 9–10, 13–19, 26–30, 35,
38–40; LD-07/08/11.

### Change Surface

#### Must Touch — Verified

- `codefabric-cpg-mcp/pyproject.toml` and `uv.lock`
- `tooling/proto/generate.py`, `tooling/proto/generate.rs`, toolchain identity
- Cargo proto-tooling features/dependencies and lockfile
- committed descriptor output and generated Rust/Python probe bindings
- Proto generation, descriptor, round-trip, and negative tests

#### Likely Touch — Impact Candidates

- v4 WP10 generated output paths and future four-package descriptor baseline
- stable graph/dependency hygiene rules

#### Discover at Packet Preflight

- Compile-probe `Config::compile_fds` and tonic service generation from the exact
  committed descriptor, including options/imports/oneofs.
- Confirm generated Python runtime version validation with protobuf 7.36.0.
- Enumerate all `protoc-bin-vendored` consumers before removal.

### Required Changes

1. Exact-pin runtime and compiler packages; keep grpcio-tools in the dev/build group.
2. Invoke grpcio-tools once with Python/stub/gRPC outputs and
   `--descriptor_set_out --include_imports`; omit source info from semantic IR.
3. Decode that descriptor in Rust and generate via `compile_fds`; delete the second
   compiler invocation and remove `protoc-bin-vendored` when no consumer remains.
4. Build a typed normalized descriptor census: files/packages/imports, services,
   methods/cardinality, messages, fields/numbers/types/presence, oneofs, enums/options,
   reserved names/ranges.
5. Compare with the last released descriptor when it exists. Before first release,
   establish a reviewed baseline and reject number reuse/removal without reservation.
6. Use generated `DESCRIPTOR`/`DescriptorPool` for Python semantic assertions.
7. Remove `orjson` and re-lock; add structural rules excluding it from JCS, ProtoJSON,
   Protobuf, schema fingerprints, and MCP structured results.
8. Amend v4 WP10 acceptance through this plan: four production packages must use this
   descriptor path; generated-text identity alone is insufficient.

### Legacy Disposition and Decommission

Execute DB03 for `protoc-bin-vendored`, the Rust-side protoc invocation, unbounded
Python dependency declarations, and orjson.

### Acceptance Evidence

#### Behavioral

- Cross-language representative binary round trips and unknown-field preservation.
- Runtime-version validation, status/deadline/message-limit probe behavior.

#### Structural

- One compiler invocation and one committed `FileDescriptorSet` authority.
- Python generated descriptors and Rust decoded IR equal the committed census.

#### Negative / Zero-State

- Fail field-number reuse, removal without reservation, presence/oneof/cardinality drift,
  incompatible enum change, compiler/runtime mismatch, and unknown required feature.
- No deterministic binary value is labeled canonical; no runtime import of grpcio-tools.
- No first-party or manifest/lock occurrence of orjson after decommission, except docs
  explaining its rejection.

#### Operational

- Two-root descriptor and binding generation is byte-identical and records exact
  compiler/runtime identities.

### Edit-Local Gates

`just proto-check`; narrow Rust proto feature check/Clippy/test; adapter Ruff/Pyrefly and
generated-import tests.

### Packet-Local Gates

Descriptor census/compatibility suite; cross-language loopback/round-trip; exact graph
governance; frozen adapter sync; dependency hygiene.

### Integration Milestone

M03.

### Replan Triggers

- `compile_fds` cannot generate equivalent tonic service code from the selected
  descriptor: investigate `skip_protoc_run`/descriptor-path APIs, then revise LD-08.
- grpcio-tools descriptor semantics cannot satisfy production proto options/imports:
  reopen the one-compiler choice rather than restoring an unchecked dual path.

### Rollback or Recovery

Retain the old generator only in a temporary differential test directory until the new
path proves equivalence; delete it in the same packet.

### Design-Bearing Contracts and Exemplars

```text
.proto sources --grpc_tools.protoc once--> FileDescriptorSet + Python bindings
                                             |
                                             +--> typed descriptor census
                                             +--> prost/tonic compile_fds --> Rust bindings
```

## WP06 — Separate normative KATs from generated and differential corpora

### Outcome

Every contract fixture is classified by oracle type; normative KATs cannot be silently
rewritten; routine generated/property coverage no longer causes expected-hash churn.

### Dependencies

WP03 and WP04.

### Target Invariants

I-06, I-12, I-15.

### Design and Library References

Review IR-009; Query AC-G-53; Protobuf §§16/37; Pydantic §48; FastMCP §30;
canonicalization fixture/parity guidance.

### Change Surface

#### Must Touch — Verified

- `contracts/fixtures/jcs/` metadata/layout and test harnesses
- future identity/type/path fixture contract consumed by v4 WP07
- fixture generator commands and governance rules

#### Likely Touch — Impact Candidates

- Protobuf probe known answers, snapshots, and mutation tests
- README/AGENTS contributor guidance

#### Discover at Packet Preflight

- Inventory every stored expected digest/byte string and classify its authority,
  stability value, and current update mechanism.

### Required Changes

1. Define `normative-kat`, `differential`, `property`, `negative-class`, and
   `generated-example` fixture classes with provenance metadata.
2. Keep a small normative JCS set and future CBEF/type/path set with independent origin,
   owner acceptance, expected bytes/digests, and contract version.
3. Move broad permutations/round trips/cross-language comparisons to derived assertions.
4. A candidate-KAT command may write to an isolated review directory; no gate or accept
   command may update the normative corpus.
5. Require a version/change record and reviewed KAT delta for intentional protocol change.

### Legacy Disposition and Decommission

Remove incidental stored hashes with no independent-oracle value. Preserve genuine KATs.

### Acceptance Evidence

#### Behavioral

- A seeded serializer mutation breaks at least one independent KAT.
- Broad generated cases reproduce without expected-hash edits.

#### Structural

- Every normative KAT names an independent origin/owner/version.
- Future v4 WP07 acceptance explicitly consumes this classification.

#### Negative / Zero-State

- Generator and snapshot-accept recipes cannot write normative KAT paths.
- Correlated Rust/Python wrong output cannot pass solely because both sides agree.

#### Operational

- Contract change produces a small, attributable KAT/version diff rather than blanket
  snapshot churn.

### Edit-Local Gates

Focused fixture harness tests and mutation probe.

### Packet-Local Gates

Shared Rust/Python corpus, negative failure classes, generator write-boundary rule,
targeted mutation campaign.

### Integration Milestone

M02.

### Replan Triggers

- A claimed independent value has no recoverable authority: downgrade it to differential
  evidence or obtain owner review before retaining it as a KAT.

### Rollback or Recovery

Fixture moves are additive until all harnesses consume the new metadata; remove old paths
only after parity.

## WP07 — Compile adapter Contract IR into Pydantic and FastMCP views

### Outcome

A reusable generator emits statically typed adapter models from Contract IR; Pydantic
compiles and caches the validation/serialization authorities; FastMCP fingerprint helpers
prove protocol-facing equivalence without registering production tools early.

### Dependencies

WP02, WP03, WP04, WP05.

### Target Invariants

I-03, I-07, I-09–I-13.

### Design and Library References

Review IR-005/006/007; Serving §§19, 33–34, 70–71; Pydantic §§3, 7,
9–10, 21, 26, 34–35, 40, 48–50; FastMCP §§3, 6, 10–11, 14–15, 30,
33–34; LD-09–LD-12.

### Change Surface

#### Must Touch — Verified

- Contract catalog/IR adapter model descriptors
- adapter contracts/model/schema/fingerprint modules and packaged resources
- generated Python type source and governance headers
- adapter schema, protocol-manifest, and in-memory Client tests

#### Likely Touch — Impact Candidates

- current placeholder `contracts/adapter/*.schema.json`
- server lifespan/DI shell for test-owned runtime state
- v4 WP09 generator/schema acceptance clauses

#### Discover at Packet Preflight

- Enumerate all future adapter model families and current placeholders from Serving §19
  and v4 WP09; classify daemon-owned semantic JSON separately.
- Probe exact FastMCP 3.4.7 `get_tool`, `to_mcp_tool`, `inspect --format mcp`, lifespan,
  and DI surfaces against the installed package.

### Required Changes

1. Generate `StrictWireModel`-based request/response/meta models with reusable
   `Annotated` constraints, closed records, explicit aliases, and discriminated unions.
2. Keep semantic request content as daemon-owned recursive JSON and validate only its
   adapter shape with one module-scope `TypeAdapter`.
3. Emit both `model_json_schema(mode="validation")` and
   `mode="serialization"` (plus TypeAdapter schemas), attach Draft 2020-12 `$schema`
   and stable `$id`, and preserve `$defs`/recursion.
4. Fingerprint normalized schema views with RFC 8785+BLAKE3. Never use sorted ordinary
   JSON.
5. Implement a FastMCP fingerprint policy over protocol-facing tool data. Exercise it
   with test-only typed tools and compare in-process `to_mcp_tool()`, CLI inspect output,
   and `Client(mcp)` behavior.
6. Prove runtime state, daemon client, and `Context` enter through lifespan/DI and never
   appear in public schemas. Do not add the real public catalog before Wave 18.
7. Add structural rules preventing model/TypeAdapter construction in handlers/hot loops.
8. Amend v4 WP09 acceptance: adapter schemas derive from these Pydantic models, not an
   independent renderer; every Contract-IR change must update all intended views or fail.

### Legacy Disposition and Decommission

Execute DB04 against independent handwritten adapter schemas/renderers and per-request
model construction. Placeholder schemas are replaced atomically when their typed model
descriptors exist.

### Acceptance Evidence

#### Behavioral

- One Contract-IR field mutation changes every intended model/schema/fingerprint or
  causes a semantic-equivalence failure.
- Test-only FastMCP tools initialize/list/call through the in-memory protocol path.

#### Structural

- Validation and serialization schemas have distinct named fingerprints.
- Models and TypeAdapters are module/startup scoped; runtime values are absent from MCP
  schemas.

#### Negative / Zero-State

- Unknown fields, unintended coercions, wrong union discriminators, alias drift, and
  subclass leakage fail.
- No orjson, dynamic per-handler model creation, custom Provider for the stable four-tool
  catalog, or Python semantic-query validator.

#### Operational

- Record model/schema build, import/startup, steady validation, and serialization timing;
  use `defer_build` only if the benchmark justifies its first-use tradeoff.

### Edit-Local Gates

Adapter Ruff, Pyrefly, focused pytest, schema exporter check.

### Packet-Local Gates

Adapter full suite; in-memory FastMCP Client; CLI manifest equivalence; contract
generation/reproduction; module-construction structural rule; startup/hot-path benchmark.

### Integration Milestone

M03.

### Replan Triggers

- Contract IR cannot express a Pydantic boundary without duplicating schema semantics:
  narrow or redesign the IR.
- FastMCP 3.4.7 installed API differs from the reference: installed signature wins and
  the plan/library decision is revised before code.

### Rollback or Recovery

Generated models/resources are additive until all adapter tests consume them. No dual
public schema authority may survive packet completion.

### Design-Bearing Contracts and Exemplars

```text
Contract IR -> generated Python annotations -> Pydantic CoreSchema
            -> validation JSON Schema -----+
            -> serialization JSON Schema --+-> JCS/BLAKE3 fingerprints
            -> typed FastMCP signature ----+-> MCP manifest/fingerprint
```

## WP08 — Benchmark and optimize the aggregate command graph

### Outcome

Aggregate recipes perform no proven duplicate work, and every change has controlled
cold/warm timing plus machine-readable proof-coverage equivalence.

### Dependencies

WP02 for the consumer graph. May run in parallel with WP03–WP07 after catalog APIs settle.

### Target Invariants

I-14, I-15.

### Design and Library References

Review IR-008; accepted cache/feature isolation design §§5–7, 12–13; repo-spec Tier A
and performance evidence order; LD-13.

### Change Surface

#### Must Touch — Verified

- `justfile` aggregate dependencies
- benchmark/proof-coverage tooling and recorded results
- adapter aggregate's repeated STDIO selection if equivalence is proven

#### Likely Touch — Impact Candidates

- CI workflow only when per-step granularity can retain the targeted STDIO step without
  duplicating local aggregate work
- optional `ci-affected` recipe derived from catalog consumer edges

#### Discover at Packet Preflight

- Expand every aggregate to exact commands/targets/features/tests; record per-domain
  time and sccache counters.
- Confirm whether Clippy/test builds cover—but do not assume they replace—each check.

### Required Changes

1. Create a proof-coverage manifest for root/extractor/sidecar/adapter/governance steps.
2. Benchmark baseline warm runs with Hyperfine warmup/repeats under fixed load.
3. Benchmark cold runs with fresh explicit temporary target directories; never clean the
   shared target or reset cache state destructively.
4. Stop rerunning `test_stdio.py` after full adapter pytest only if the aggregate coverage
   manifest proves the full suite selects it. Retain the independent targeted recipe and
   CI step where granular diagnostics are valuable.
5. Keep root/extractor/sidecar check and Clippy Tier-A obligations. Any proposal to remove
   one requires a repo-spec design revision, not a recipe tweak.
6. Optionally add `ci-affected` from changed paths plus catalog consumers. Ambiguous,
   catalog/governance, or tooling changes fall back to full `ci-fast`.
7. Accept changes only for a material wall-time or compile-request reduction with equal
   proof coverage; otherwise retain the existing graph and record the negative result.

### Legacy Disposition and Decommission

Only measured duplicate aggregate edges are removed. Independent recipes remain.

### Acceptance Evidence

#### Behavioral

- Injected representative failures are caught by before/after aggregate graphs.

#### Structural

- Coverage manifest shows identical toolchain, target, feature, test, fixture, and
  negative-proof coverage.

#### Negative / Zero-State

- No `cargo clean`, shared-cache deletion, weakened feature matrix, or replacement of
  full milestone `ci-fast` by `ci-affected`.

#### Operational

- Report distributions, medians, compile requests, sccache hit/miss deltas, environment,
  and retained/rejected candidate changes.

### Edit-Local Gates

Justfile parse/list; targeted aggregate smoke checks.

### Packet-Local Gates

Controlled benchmark; complete `just ci-fast`; proof-coverage comparator.

### Integration Milestone

M04.

### Replan Triggers

- Timing variance masks the candidate effect: increase samples/control load; do not claim
  improvement.
- A candidate changes proof coverage or repo Tier A: reject or reopen tooling design.

### Rollback or Recovery

Recipe changes are individually revertible. Keep benchmark artifacts under ignored
`target/` plus a concise committed result record without machine secrets.

## WP09 — Certify findings, decommission old authority, and release v4 WP07

### Outcome

All in-scope findings have traceable evidence, old authority is mechanically absent,
independent re-review closes IR-001–IR-003, v4 M01 is satisfied, and v4 WP07 becomes the
safe next action under standing WP09/WP10 overlays.

### Dependencies

WP01–WP08, DB02–DB04, and the pending v4 M01 Ubuntu clean-checkout evidence.

### Target Invariants

I-01–I-15.

### Design and Library References

Entire review and this plan; v4 WP07/WP09/WP10/WP11/M02; repository final-gate policy.

### Change Surface

#### Must Touch — Verified

- this plan's execution state
- v4 execution state interlock/current action
- focused re-review under `docs/reviews/`
- final evidence/result records

#### Likely Touch — Impact Candidates

- derived spec indexes and agent documentation after final path/name reconciliation

#### Discover at Packet Preflight

- Re-run complete structural searches from DB01–DB04 over all source/governance scopes,
  including generated and hidden config where safe.
- Reconcile v4 source/state drift and proving evidence since WP01.

### Required Changes

1. Obtain an independent implementation re-review focused on IR-001–IR-003 and record
   every IR disposition/evidence path.
2. Verify v4 M01's clean Ubuntu checkout. If external execution remains unavailable,
   stop; do not mark the interlock complete without user-approved plan revision.
3. Run final mixed-language, generation, descriptor, schema, package-resource,
   performance, and zero-state gates.
4. Update v4 state: record this plan/version/digest, M04 evidence, WP07 readiness, revised
   KAT rule, and standing v4 WP09/WP10 overlays. Preserve completed packet history.
5. Record DO-01/DO-02 as blocking inputs to the future Wave 17/18 plans, not as completed
   behavior.

### Legacy Disposition and Decommission

Certify DB01 remains green and DB02–DB04 are complete. No unresolved compatibility shim
or duplicate authority may be called “deferred”.

### Acceptance Evidence

#### Behavioral

- Representative end-to-end contract change flows catalog -> typed ingress -> digest ->
  index -> Rust/Python view -> Pydantic schemas/fingerprint.

#### Structural

- Finding disposition table has one terminal in-scope outcome per IR ID.
- V4 WP07 dependency record names this plan M04 and the KAT policy.

#### Negative / Zero-State

- All DB searches green; focused reviewer reports no blocker/major finding in IR-001–003.
- No state claims runtime DaemonClient/public tools were implemented.

#### Operational

- Clean Ubuntu checkout, local full gate, two-root generation, and controlled command
  benchmark evidence are retained with exact commands/tool versions.

### Edit-Local Gates

State/schema validation; scoped docs checks.

### Packet-Local Gates

All final matrix commands in §9; independent re-review; `git diff --check`.

### Integration Milestone

M04.

### Replan Triggers

- Independent re-review leaves IR-001–IR-003 blocker/major findings.
- V4 drift changes packet boundaries, catalog ownership, or consumer graph.
- External M01 cannot be produced and no accepted substitute exists.

### Rollback or Recovery

Keep v4 blocked and this plan active. Do not partially release WP07.

---

## 7. Integration milestones

### M01 — Corrected design authority accepted

**Packets:** WP01.

**Evidence:** accepted AC-G-02/05, roadmap, and Serving changes; worked dual-identity
vectors; restamped digests/indexes; v4 WP07 state interlock.

**Gate:** design consistency review, scoped docs checks, no unresolved owner decision.

### M02 — Model-based contract compiler cutover

**Packets:** WP02, WP03, WP04, WP06.

**Evidence:** one typed catalog, bounded format adapters, all real digests, single artifact
index resource, independent KAT architecture, DB02 green.

**Gate:** root/adapter contract suites, negative/budget/fuzz fixtures, two-root
regeneration, focused preliminary review of IR-001–IR-003.

### M03 — Protocol and adapter compiler substrate

**Packets:** WP05, WP07.

**Evidence:** one descriptor IR/compile, exact manifest pins, orjson removed, Pydantic
validation/serialization schemas and FastMCP fingerprint substrate, DB03/DB04 green.

**Gate:** descriptor/round-trip/compatibility suites, adapter full/in-memory/CLI tests,
schema reproduction, exact graph and lock checks.

### M04 — Foundation remediation certified; v4 WP07 released

**Packets:** WP08, WP09; requires M01–M03 and external v4 M01 completion.

**Evidence:** controlled command benchmark, full mixed-domain gate, all decommission
batches, independent IR-001–IR-003 re-review, state reconciliation.

**Gate:** §9 final matrix. The safe next action becomes v4 WP07 under the standing KAT,
catalog, adapter, and descriptor overlays.

---

## 8. Cross-packet decommission batches

### DB01 — Preserve the completed seed/PyO3/Maturin zero-state

Inherited complete from v4. Re-run the permanent seed zero-state rule at M04. No packet
may reintroduce root Python packaging, PyO3, Maturin, the old `_native` module, a Cargo
workspace, or a second production crate.

### DB02 — Manual contract authority zero-state

**Prerequisites:** WP02–WP04.

**Delete/prohibit:** zero digest sentinels; `REQUIRED_SOURCE_ARTIFACTS`;
`REGISTRY_SOURCES`; literal 50/13 assertions; separate output authorities; lexical
metadata/draft scans; generic closed-record field lookups; hand-rendered Rust/Python/stub
artifact-index mirrors.

**Exit invariant:** one catalog authority, one canonical index data resource, one typed
descriptor per ID/path/output, and complete structural search with no old pattern.

### DB03 — Dual Protobuf compiler and speculative JSON dependency zero-state

**Prerequisites:** WP05.

**Delete/prohibit:** Rust-side production protoc invocation,
`protoc-bin-vendored` when unused, open manifest ranges for protocol/compiler packages,
orjson dependency/lock/source use, deterministic-Protobuf-as-canonical claims.

**Exit invariant:** one descriptor-set compile and exact manifest/lock intent.

### DB04 — Independent adapter schema authority zero-state

**Prerequisites:** WP07.

**Delete/prohibit:** independently handwritten schemas for generated adapter models,
per-request model/TypeAdapter construction, custom Provider for the stable four-tool
catalog, runtime/Context fields in public schemas, sorted ordinary-JSON fingerprints.

**Exit invariant:** Contract IR -> Pydantic -> schema/FastMCP views is the sole adapter
publication pipeline.

---

## 9. Final gate matrix

| Risk/proof | Required command or evidence | Boundary |
|---|---|---|
| Repository baseline/full regression | `just ci-fast` | M04 |
| Root contract compiler | narrow contracts feature format/check/Clippy/Nextest plus doctests | WP02–WP04/M02 |
| Contract conformance | `just contracts-verify` and committed negative suite | WP02–WP07 |
| Reproducibility | `just contracts-repro-check` from two isolated roots | WP03–WP07/M04 |
| Parser resource safety | budget edge fixtures + bounded fuzz replay per parser | WP03/M02 |
| Canonical parity | shared Rust/Python bytes/raw digest/framed digest corpus | WP03/WP06 |
| Digest migration | complete zero-sentinel and mutation proofs | WP04/M02 |
| Artifact packaging | Rust/Python identical resource bytes and typed views | WP04/M02 |
| Proto IR | `just proto-check`, descriptor census/compatibility, two-root generation | WP05/M03 |
| RPC compatibility | cross-language round trip, unknown fields, runtime version, limits/status/deadline probes | WP05/M03 |
| Python quality | adapter Ruff format/lint, Pyrefly, full pytest | WP05/WP07/M03 |
| Pydantic/FastMCP | schema check, in-memory `Client`, CLI manifest equivalence, hot-path construction rule | WP07/M03 |
| Dependency policy | frozen sync, exact pin assertion, graph hygiene, no runtime grpcio-tools/orjson | WP05/M03 |
| Feature isolation | `just stable-graph-check`, `just features-each` when Cargo features change | WP03/WP05/M04 |
| Legacy zero-state | DB01–DB04 complete searches with generated/hidden scope handling | M04 |
| Performance | controlled Hyperfine cold/warm report and proof-coverage comparator | WP08/M04 |
| Clean environment | pending v4 M01 Ubuntu clean-checkout `just ci-fast` | M04 |
| Independent challenge | focused implementation re-review closes IR-001–IR-003 | WP09/M04 |
| Diff integrity | `git diff --check`; reconcile user/pre-existing changes | every packet/M04 |

No Nextest-only result is “all Rust tests”; doctests remain separate. No editable/import
success substitutes for package-resource proof. Tier-C tools run only for the parser,
mutation, or performance risks named above.

---

## 10. Execution sequence

```text
v4 WP06 complete
       |
       v
WP01 design correction + state interlock ---------------- M01
       |
       +----------------------+
       v                      v
WP02 typed catalog       WP05 descriptor compiler/pins
       |                      |
       +----------+-----------+
                  v
        WP03 bounded ingress/digests
                  |
                  v
        WP04 artifact/index cutover
                  |
          +-------+-------+
          v               v
   WP06 oracle split   WP07 adapter compiler
          |               |
          +-------+-------+---------------- M02/M03
                  |
          WP08 command benchmark
                  |
          WP09 final re-review/state
                  |
                  v
                 M04
                  |
                  v
          v4 WP07 -> WP08 -> {WP08b || WP09 || WP10} -> WP11/M02
```

Parallelism constraints:

- WP02 and WP05 may run in parallel after WP01 because their initial write sets are
  catalog/compiler versus Proto tooling; coordinate the catalog descriptor and digest
  profile before either closes.
- WP03 waits for the descriptor IR API before completing the Proto projection.
- WP08 may measure the baseline early but may not finalize an optimized graph until the
  compiler/consumer graph settles.
- V4 WP08b/WP09/WP10 are parallel only if WP02 predeclares their descriptors/outputs so
  they do not contend on catalog authority. Otherwise revise the v4 sequence to serialize
  catalog edits.

Standing downstream obligations:

- **DO-01 (Wave 17):** one event-loop-owned `grpc.aio` channel/stub in FastMCP lifespan,
  typed `DaemonClient`, deadline on every call, centralized metadata/status mapping,
  explicit close after serving stops, and streaming only for semantic need.
- **DO-02 (Wave 18):** four explicit typed handlers through FastMCP's local-provider
  path; real validation/serialization/semantic/MCP manifest equivalence and fingerprints;
  runtime state and `Context` only through DI; no custom Provider absent a dynamic catalog.

---

## 11. Completion checklist

- [ ] WP01 complete; corrected design accepted and v4 interlock recorded.
- [ ] WP02 complete; one typed catalog/derivation graph.
- [ ] WP03 complete; bounded typed ingress and all digest profiles.
- [ ] WP04 complete; all real digests and one artifact-index resource.
- [ ] WP05 complete; one descriptor compiler/IR, exact pins, orjson removed.
- [ ] WP06 complete; independent KAT and derived-corpus policy enforced.
- [ ] WP07 complete; Pydantic/FastMCP compiler substrate and schema fingerprints.
- [ ] WP08 complete; command graph benchmark and only proven optimizations retained.
- [ ] WP09 complete; independent re-review and v4 state release.
- [ ] M01–M04 green.
- [ ] DB01 revalidated; DB02–DB04 complete.
- [ ] IR-001–IR-003 closed by focused review.
- [ ] IR-004/IR-005 compiler portions closed; DO-01/DO-02 durably assigned.
- [ ] IR-006–IR-008 closed with evidence; IR-009 enforced as standing policy.
- [ ] V4 M01 clean Ubuntu checkout evidence present.
- [ ] V4 WP07 is the recorded safe next action; no later packet is falsely complete.
- [ ] No execution-state file was created during planning; initialize it only after plan
  acceptance and execution request.

---

## 12. Plan risks and replan policy

| ID | Risk | Response / trigger |
|---|---|---|
| R-01 | Dual identity is rejected or misunderstood as duplicate authority | Reopen design; do not implement a projection until AC-G-02 names both questions and version rules. |
| R-02 | The suite manifest becomes a universal schema | Reject semantic fields that belong to native authorities; keep only ownership/derivation metadata. |
| R-03 | Catalog self-bootstrap becomes self-referential | Computed digest census stays in generated index; catalog self record uses structural semantic omission. Any computed catalog member reopens D-02. |
| R-04 | YAML aliases expand before limits can act | Reject aliases or select a parser with proven bounds through design revision. |
| R-05 | Schema metaschema validator creates a cross-domain runtime dependency | Keep it build-only; if hermetic composition fails, select an exact Rust validator before code. |
| R-06 | One descriptor compiler cannot serve all production options | Probe exact APIs/options; revise compiler decision rather than accepting unchecked semantic drift. |
| R-07 | Generated Pydantic source duplicates JSON-Schema authority | Contract IR must own model semantics and Pydantic must emit adapter schemas; remove competing emitters. |
| R-08 | KAT automation makes the implementation its own oracle | Candidate output is review-only; governance prohibits generator writes to normative paths. |
| R-09 | Central catalog becomes a parallel-write bottleneck | Predeclare descriptors before branch packets or serialize those packets by plan revision. |
| R-10 | Performance optimization weakens Tier A | Reject the candidate; repository spec changes require design review. |
| R-11 | Dirty-tree overlap obscures ownership | Capture per-file preflight digests and preserve unrelated/user changes; stop on an irreconcilable overlap. |
| R-12 | External Ubuntu evidence remains unavailable | M04 remains blocked unless the user accepts an explicit substitute/replan. |
| R-13 | Runtime concerns are falsely claimed complete | DO-01/DO-02 stay open and blocking for Wave 17/18 plans; M04 certifies only their substrate/contract. |

### 12.1 Adaptation versus revision

- **Implementation adaptation:** helper/module layout, diagnostic wording, structured
  emitter choice, or benchmark sample count changes while invariants and packet boundaries
  remain intact. Record in execution state.
- **Plan revision:** packet order/write sets, selected schema validator, descriptor
  generation mechanism, catalog output ownership, or proof obligations change.
- **Design reopening:** digest meaning/profile bytes, suite-manifest authority, native
  authority boundaries, exact protocol family, or Pydantic/FastMCP ownership changes.

### 12.2 Stop conditions

Stop rather than improvise if a packet cannot leave one coherent authority, requires
unbounded dual operation, weakens an invariant, changes public contract meaning, or cannot
produce its negative/operational proof. Preserve the last green packet and record the
failed approach before replanning.
