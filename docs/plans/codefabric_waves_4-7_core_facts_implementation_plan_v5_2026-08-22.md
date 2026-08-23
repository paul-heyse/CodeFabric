---
artifact: implementation-plan
plan_id: codefabric-waves-4-7-core-facts
version: v5
date: 2026-08-22
status: approved
design_path: docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md
design_version: v1.0
baseline_commit: 3830acade129b2a63a1927ac5e2f4d3ac284f38c
state_path: docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json
supersedes_plan_path: docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md
activation_requires: codefabric-model-driven-artifact-and-assurance-control-plane/M05
cutover: true
---

# CodeFabric Waves 4–7 core facts — implementation plan v5

This successor preserves the product outcomes and stable identifiers of Waves 4–7 v4 while
adopting the completed model-driven artifact and assurance control plane. It is intentionally
inactive until the model-control-plane plan reaches its sealed M05 handoff and the repository
owner explicitly approves this document. The frozen v4 plan and state remain historical evidence;
they are not edited or made green by rebuilding superseded governance.

WP27–WP31 retain their trusted proving commits. WP32 remains incomplete: the remediation supplied
recipe-aware CBEF APIs, governed registry/flag accessors, corrected TableSpec projections, and
library-derived provider catalogs, but it did not prove WP32's complete canonical encoding,
reconciliation, and end-to-end fact-ingest outcome. No WP33+ product work is claimed.

## 1. Outcome, boundary, and current disposition

### 1.1 Outcome

At M08, CodeFabric has:

1. deterministic, canonical source/syntax facts for Python and Rust;
2. one accepted Wave-5 golden vertical slice from source through query and service result;
3. continuous update, invalidation, freshness, recovery, and clean-rebuild equivalence;
4. Git-aware acceleration whose disabled path produces identical semantic state; and
5. model-derived artifacts, provenance, and assurance without authored aggregate manifests,
   embedded digest edits, packet mutation campaigns, or a second generation authority.

### 1.2 Non-goals and accepted deferrals

- This plan does not recreate any legacy contracts compiler, generator script, proof manifest,
  package include list, shared compiler executable, or `mutants-wp*` recipe.
- `mutants-file` remains an optional human Tier-C diagnostic and is never a packet or profile gate.
- Ubuntu clean-host evidence and license work remain user-deferred and do not block a packet.
- Gate C, sidecar activation, notebooks, the lifecycle journal of LIFE §108.2, and performance SLOs
  retain their roadmap owners outside Waves 4–7.
- Wall-clock build timing, cache hit rate, coverage percentage, and mutation score are never
  correctness gates or execution-state fields.

### 1.3 Historical and active-program boundary

- Frozen predecessor: `codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md`.
- Frozen trusted history: WP27 `7d82ec80`, WP28 `fed29172`, WP29 `0e440ac6`, WP30
  `0b5cbdbb`, WP31 `74befce8` (full hashes live in schema-2 state).
- Resume point after activation: WP32 `in_progress`, without a proving commit.
- WP33–WP53, M05–M08, and DB07–DB09 remain unproved.
- Until the sealed handoff, the model-control-plane remediation is the only active program.

## 2. Declared inputs

| path | sha256 |
|---|---|
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 0cacf3cbb3e0abe2b5e3f358b59ac198066d721c56761219905208231e32e7c6 |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | 2824e43ba21d9a24013b84f9a10eef5b7b65a6db8da594afbd60c841a00bcfa8 |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | 93e0c0e559a9f95d427d8ce6c52aba24307df8ae85440050fb531099b482da12 |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 81a6ea7baa3eb4229802acfba0c538051de27bc9dfaa026f174ce0422cc6e3ff |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | 4533fc4e8e944ee77dea593c271faa0442a4e1268e8e780466f8404474df56ed |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | e88aeb7fad6cb3d6f76e31cb32632d4a8d133ba2ec623449fa8cf57595f6d26a |
| docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | a5affe90ab4ea5b15f2f7cb41fc6b4da54f98c3c73e4bf2a9025c57a0730bf06 |
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | a58c42d04d5fef01efce9d21228cdd7f01e528dc7b5c6a1828310105e389e250 |
| docs/designs/codefabric_model_driven_artifact_and_assurance_control_plane_design_v1_2026-08-22.md | bede807bff61d4242cea09871c14cac379bbae13ea7497ba54286a9ceed07873 |
| docs/plans/codefabric_model_driven_artifact_and_assurance_control_plane_implementation_plan_v1_2026-08-22.md | b62f50eafa9ef4b82554f2eb5fb67cd9d50ac94e4b19de4ea91aeb23ee414d3b |
| docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md | 5e4bc67347f3f3fbc89564210dd3894ed4c078e30ac618fe621755fe72d71795 |
| docs/rust_core_python_interface_repository_specification_2026-08-20.md | 42678a93d6c323d3c527255c2f266b2520bd13dca485c1f1d4af7991a9243848 |

Input freshness is derived by `artifacts-check`. This table is immutable after approval; a
behavior- or authority-changing input evolution requires an explicit state deviation owned by the
consuming packet or a successor plan.

## 3. Governing implementation decisions

1. **One compiled model.** The typed `RepositoryModel`, family drivers, `DesiredTree`, release
   census, and assurance graph are the sole artifact/provenance/proof control plane.
2. **Data-only extension.** New contract members are self-describing authorities or typed adjacent
   declarations. Aggregate indexes, bundles, package exports, paths, and proof closure are derived.
3. **Family-native validation.** Serde, JSON Schema, Arrow/DataFusion, Pydantic/FastMCP,
   Protobuf descriptors, Tree-sitter, Ruff, rustc, and gix validate their own semantic domains.
4. **Current bytes are truth.** Git and watcher data classify and accelerate; stable current bytes
   and canonical identities decide the model.
5. **One writer.** `model-check` and all gates are read-only. Confirmed `model-sync` is the sole
   routine generated-output writer and uses staged validation plus transactional recovery.
6. **Independent acceptance.** Released IDs/tombstones, KATs, allocations, compatibility records,
   signatures, and golden answers remain outside routine renderer write sets.
7. **Exact incrementality.** Action identity includes source and upstream identities, output spec,
   executable bytes, lock/toolchain, feature/profile/target, and relevant environment. Cache misses,
   corruption, unknown reads, and watcher overflow widen or recompute; they never relax proof.
8. **Model-derived assurance.** `edit`, `changed`, `tier-a`, and `release` are compiled from live
   model/Just/test/rule/requirement data and are differentially checked against full evidence.
9. **No legacy-gate obligation.** Once an invariant has an independently proven model-native owner,
   a superseded legacy gate may be deleted and need not pass. No uncovered invariant is waived.
10. **Provider inventories.** Tree-sitter raw kinds come from pinned library introspection and
    `NODE_TYPES`; Ruff's non-iterable enums use one compiler-exhaustive declaration in an isolated
    provider tool. Normalization is resolved from the typed registry into generated hot-path data.
11. **CBEF fidelity.** Production construction uses generated recipe-aware builders and governed
    allocation accessors. `parenthesized`, `parse_error`, and `missing` are `syntax_detail` or
    `source_annotation` facts, not duplicate relation kinds. The generic codec is private and
    validates the selected domain recipe.
12. **Acceleration is optional.** Wave-7 gix/cache paths never become correctness authorities and
    must remain semantically equivalent to the fallback path.

## 4. Packet contract

Every remaining packet follows this contract in addition to its packet-specific obligations:

- Preflight: `model-plan`, `model-explain`, current plan/status checks, and focused structural
  search; no mutating generator is a preflight.
- Implementation: update typed authority or adjacent declaration, family driver, generated
  consumer, and live assurance relationship together; do not edit derived aggregates by hand.
- Apply: invoke confirmed `model-sync` only when the packet changes a governed DesiredTree output,
  inspect the full diff, then require a zero-action read-only plan.
- Proof: focused behavioral, structural, negative, operational, and independent consumer oracles;
  `model-check profile="edit"`; packet integration recipe where named; `ci-fast` proportionate to
  cross-domain risk. No packet-specific mutation infrastructure.
- State: completion requires all packet acceptance at a proving commit that is an ancestor of HEAD
  and again at current HEAD. State contains judgments, not derived metrics or copied inventories.
- Replan: stop for a normative conflict, an unmodeled authority/write path, an inability to stage
  validation independently, a semantic difference between incremental and full execution, or a
  new dependency/package boundary.

## 5. Wave 4 — source and syntax fact generation

### WP27 — Fact-generation build capability and provider pins

**Status:** complete at `7d82ec80b8b3e0812e97b668058315da1aa73030`.

**Dependencies:** successor activation prerequisite. **Outcome preserved:** exact Tree-sitter,
Ruff, and Rayon pins behind `fact-generation`, parser-free narrow graphs, stable feature boundary,
and provider pins represented in model-derived toolchain provenance.

**Current-head proof:** `stable-graph-check`, `features-each`, provider-family model validation,
and `ci-fast`. Reopen only if a pin/feature boundary or toolchain provenance changes.

Executable oracle: `wp27_behavioral_acceptance`
Executable oracle: `wp27_structural_acceptance`
Executable oracle: `wp27_negative_zero_state`
Executable oracle: `wp27_operational_acceptance`

### WP28 — Source/syntax schema and registry contract extension

**Status:** complete at `fed2917249b1e791346b289e4195c658bb40a8d1`.

**Dependencies:** WP27. **Outcome preserved:** the four source/syntax TableSpecs, serving views,
entity/token/annotation/role/resource/normalization registries, provider-raw catalog derivation,
and typed `AnalysisContext` contract.

The model control plane now derives TableSpec, schema, registry, provider inventory, package, and
provenance outputs. Current-head proof is `model-family-check schema`, `model-family-check
registry-cbef`, `model-release-census-check`, staged Arrow/SQLite/JSON-Schema consumers, and
`ci-fast`. Reopen on an allocation or Contract-IR semantic change.

Executable oracle: `wp28_behavioral_acceptance`
Executable oracle: `wp28_structural_acceptance`
Executable oracle: `wp28_negative_zero_state`
Executable oracle: `wp28_operational_acceptance`

### WP29 — Provider job runtime (GEN AC-G-32)

**Status:** complete at `0e440ac69ea2d684fbefc50b3508d523c304c4e1`.

**Dependencies:** WP27, WP28. **Outcome preserved:** asynchronous provider submission, bounded
admission, deadline/cancellation/lease/resource-profile enforcement, immutable observations,
metrics, and deterministic fake-clock tests. Current-head proof is the provider runtime suite,
`model-check profile="edit"`, stable graph, and `ci-fast`.

Executable oracle: `wp29_behavioral_acceptance`
Executable oracle: `wp29_structural_acceptance`
Executable oracle: `wp29_negative_zero_state`
Executable oracle: `wp29_operational_acceptance`

### WP30 — Tree-sitter Python and Rust adapters

**Status:** complete at `0b5cbdbba286bb465126caffe458963cc4d9dc38`.

**Dependencies:** WP27, WP28, and WP29's accepted job contract. **Outcome preserved:** incremental
Tree-sitter adapters emitting application-owned observations, governed query/field roles, explicit
recovery kinds, normalized raw kinds, cancellation/resource limits, and edit correctness.

Provider raw catalogs and Rust lookup tables are now derived by the isolated library-introspection
tool and typed normalization authority. Current-head proof uses `model-family-check registry-cbef`,
adapter behavior/correspondence fixtures, and `ci-fast`.

Executable oracle: `wp30_behavioral_acceptance`
Executable oracle: `wp30_structural_acceptance`
Executable oracle: `wp30_negative_zero_state`
Executable oracle: `wp30_operational_acceptance`

### WP31 — Ruff syntax/lexical adapter

**Status:** complete at `74befce8d04cc1f5a53c9d2ae728e03f8a929457`.

**Dependencies:** WP27–WP30. **Outcome preserved:** one complete compiler-exhaustive Ruff
`NodeKind`/`TokenKind` census, registry-owned normalization, Python coordinates/trivia/docstring
ownership, bounded admission, and Tree-sitter correspondence.

Current-head proof uses the generated exhaustive matcher, provider-family validation, Ruff adapter
fixtures, and `ci-fast`. A newly added Ruff enum variant must fail the isolated provider-tool build
until the single exhaustive declaration and normalization authority are deliberately updated.

Executable oracle: `wp31_behavioral_acceptance`
Executable oracle: `wp31_structural_acceptance`
Executable oracle: `wp31_negative_zero_state`
Executable oracle: `wp31_operational_acceptance`

### WP32 — Canonical source/syntax encoders and source-range reconciliation

**Status:** in progress; no proving commit. **Dependencies:** WP28, WP30, WP31.

**Outcome:** production encoders for `source_file`, `source_token`, `source_annotation`, and
`syntax_detail`; canonical deterministic IDs and rows; exact source-range validation and
provider-to-canonical reconciliation; validated-batch ingestion; and cross-provider equivalence.

**Required changes:** finish the model-generated row encoders and runtime consumers over the
already-promoted recipe-aware CBEF builders and registry/flag accessors. Encode ENTITY and
RELATION_FACT only through their governed field recipes. Represent parentheses, parser errors,
and missing nodes through the governed detail/annotation fields; do not create relation codes for
them. Reject invalid UTF-8 boundaries, overlapping/inverted/out-of-range spans, recipe mismatch,
unknown allocations, and ambiguous reconciliation before persistence.

**Acceptance:** deterministic replay; Python/Rust multi-provider fixture parity; malformed/range/
allocation/recipe negatives; generated API compile-fail guard against positional CBEF; staged
TableSpec/Arrow/SQLite/JSON-Schema validation; `model-family-check registry-cbef`,
`model-family-check schema`, `model-check profile="edit"`, and `ci-fast`.

**Legacy/rollback:** no restoration of raw codes, positional codecs, or packet mutation recipes.
On failure, preserve the last model-derived generated tree and leave WP32 incomplete.

Executable oracle: `wp32_behavioral_acceptance`
Executable oracle: `wp32_structural_acceptance`
Executable oracle: `wp32_negative_zero_state`
Executable oracle: `wp32_operational_acceptance`

### WP33 — Classification, capability, unknown handling, and Wave-4 integration

**Dependencies:** WP29, WP32. **Outcome:** GEN AC-G-43 admission thresholds, binary/generated/
excluded handling, explicit capability records, unknown-language/kind behavior, and the complete
capture → provider → reconcile → ingest → pinned-query Wave-4 path.

**Acceptance:** add `wave4-integration-check` as a stable capability recipe; cover valid,
malformed, oversized, binary, excluded, cancellation, and unknown cases; repeat extraction for
identical identities/effective rows; run model release-census and changed/full proof. Replan if any
raw provider kind lacks one governed disposition.

Executable oracle: `wp33_behavioral_acceptance`
Executable oracle: `wp33_structural_acceptance`
Executable oracle: `wp33_negative_zero_state`
Executable oracle: `wp33_operational_acceptance`

## 6. Wave 5 — end-to-end vertical golden slice

### WP34 — Golden corpus v1 substrate (SUITE AC-G-78)

**Dependencies:** M05. **Outcome:** immutable `codefabric-golden-v1` candidate with Python, Rust,
and mixed-workspace groups; byte identities, manifests, expected outputs, negative cases, and owner
review kept as evidence/acceptance rather than renderer output.

**Acceptance:** exact-byte census, independent KAT validation, tamper/missing/extra-file negatives,
and `model-check profile="edit"`. Replan rather than auto-accept any changed golden answer.

Executable oracle: `wp34_behavioral_acceptance`
Executable oracle: `wp34_structural_acceptance`
Executable oracle: `wp34_negative_zero_state`
Executable oracle: `wp34_operational_acceptance`

### WP35 — Thin rustc extractor slice (GEN AC-G-31 subset)

**Dependencies:** WP29, M05, WP34. **Outcome:** real Cargo-native rustc protocol for the authorized
golden fixture, bounded typed request/response, exact toolchain identity, diagnostics, MIR/instance
subset, cancellation, and process isolation.

**Acceptance:** `extractor-ci-fast`, protocol malformed/deadline/toolchain negatives, deterministic
golden output, model provenance binding, and `ci-fast`. Replan on required compiler-private API
outside the accepted extractor boundary.

Executable oracle: `wp35_behavioral_acceptance`
Executable oracle: `wp35_structural_acceptance`
Executable oracle: `wp35_negative_zero_state`
Executable oracle: `wp35_operational_acceptance`

### WP36 — Minimal canonical reconciliation engine (FAB AC-G-37 core)

**Dependencies:** WP32, WP35. **Outcome:** production reconciliation replaces
`SyntheticCanonicalIngest`; source/syntax and thin semantic observations become validated,
deterministic canonical facts with provenance and conflict diagnostics.

**Acceptance:** cross-provider precedence/conflict/property fixtures, state-digest equivalence,
invalid-batch atomicity, `model-check profile="edit"`, and DB07 zero state.

Executable oracle: `wp36_behavioral_acceptance`
Executable oracle: `wp36_structural_acceptance`
Executable oracle: `wp36_negative_zero_state`
Executable oracle: `wp36_operational_acceptance`

### WP37 — Minimal registered derivation (SYNTAX_TREE_V1)

**Dependencies:** WP36. **Outcome:** typed derivation registry record, deterministic dependency
closure, executor, projection rows, provenance, and incremental invalidation for `SYNTAX_TREE_V1`.

**Acceptance:** full/incremental exact equality, missing/cycle/version negatives, independent query
of derived rows, and model graph explanation.

Executable oracle: `wp37_behavioral_acceptance`
Executable oracle: `wp37_structural_acceptance`
Executable oracle: `wp37_negative_zero_state`
Executable oracle: `wp37_operational_acceptance`

### WP38 — Minimal semantic query slice (QRY core subset)

**Dependencies:** M05, WP37. **Outcome:** bounded typed validation and execution for the three
Wave-5 query forms over pinned snapshots, canonical JSON responses, deterministic ordering,
pagination/error semantics, and registry-resolved phrase mappings.

**Acceptance:** independent request/response KATs, malformed/budget/unknown/stale-handle negatives,
state-digest equivalence, and model-derived query-contract assurance.

Executable oracle: `wp38_behavioral_acceptance`
Executable oracle: `wp38_structural_acceptance`
Executable oracle: `wp38_negative_zero_state`
Executable oracle: `wp38_operational_acceptance`

### WP39 — Minimal accepted-handle service and result artifacts

**Dependencies:** WP38. **Outcome:** private UDS daemon serves the accepted v1 RPC subset with
deadline/status/limit handling, immutable handles, result artifacts, and one FastMCP adapter path.

**Acceptance:** descriptor/runtime/client round trips, cancellation/deadline/message-limit
negatives, installed-wheel package-data validation, and canonical result identity.

Executable oracle: `wp39_behavioral_acceptance`
Executable oracle: `wp39_structural_acceptance`
Executable oracle: `wp39_negative_zero_state`
Executable oracle: `wp39_operational_acceptance`

### WP40 — Vertical golden slice integration and Gate B

**Dependencies:** WP34–WP39. **Outcome:** all eleven Gate-B artifacts execute end to end over the
accepted golden profile and match independently reviewed identities, rows, response bytes, and
checksums.

**Acceptance:** add `wave5-integration-check` and `gate-b-check`; cache-disabled and incremental
execution agree; `ci-fast`, `extractor-ci-fast`, `adapter-wheel-test`, and model release checks pass.

Executable oracle: `wp40_behavioral_acceptance`
Executable oracle: `wp40_structural_acceptance`
Executable oracle: `wp40_negative_zero_state`
Executable oracle: `wp40_operational_acceptance`

## 7. Wave 6 — continuous update, freshness, and core equivalence

### WP41 — Watcher and event facade

**Dependencies:** M06. **Outcome:** a bounded `notify-debouncer-full` facade normalizes hints,
rename/rescan/overflow, lifecycle, and backpressure without treating events as current-byte truth.

**Acceptance:** platform-order-independent final-state scenarios, overflow widening, shutdown and
resource cleanup, plus poll/fallback equivalence.

Executable oracle: `wp41_behavioral_acceptance`
Executable oracle: `wp41_structural_acceptance`
Executable oracle: `wp41_negative_zero_state`
Executable oracle: `wp41_operational_acceptance`

### WP42 — Dirty registry, update-wave scheduler, and source capture loop

**Dependencies:** WP41. **Outcome:** actor-owned dirty registry and scheduler coalesce hints,
capture stable current bytes, persist the accepted update-wave vocabulary, enforce budgets, and
retry/restart deterministically.

**Acceptance:** burst/coalescing/backpressure/restart/source-drift tests, no lost dirty owner, and
DB09 persisted-row migration plus zero emission of deprecated states.

Executable oracle: `wp42_behavioral_acceptance`
Executable oracle: `wp42_structural_acceptance`
Executable oracle: `wp42_negative_zero_state`
Executable oracle: `wp42_operational_acceptance`

### WP43 — Invalidation and operational dependency graph (LIFE AC-G-41)

**Dependencies:** WP42. **Outcome:** typed SQLite dependency graph drives conservative owner and
derivation invalidation with transactions, cycle/unknown handling, and explainable closure.

**Acceptance:** graph invariants, rollback/crash tests, stale-fact non-exposure, and full-rebuild
differential over the core edit corpus.

Executable oracle: `wp43_behavioral_acceptance`
Executable oracle: `wp43_structural_acceptance`
Executable oracle: `wp43_negative_zero_state`
Executable oracle: `wp43_operational_acceptance`

### WP44 — Fast syntax lane

**Dependencies:** WP42, WP43, WP30. **Outcome:** stable recapture, incremental Tree-sitter parsing,
canonical reconciliation, overlay publication, and safe fallback to full parse.

**Acceptance:** incremental/full parser and fact equality, edit-sequence corpus, error recovery,
overflow/source-drift widening, and strict-current query negatives.

Executable oracle: `wp44_behavioral_acceptance`
Executable oracle: `wp44_structural_acceptance`
Executable oracle: `wp44_negative_zero_state`
Executable oracle: `wp44_operational_acceptance`

### WP45 — Freshness and barrier state machine (LIFE AC-G-24/25)

**Dependencies:** WP42–WP44. **Outcome:** sole formal workspace freshness counters/barrier govern
admission, waiting, cancellation, degradation, and publication; the temporary shim is removed.

**Acceptance:** state-transition model tests, concurrent waiter/cancellation/restart cases,
strict-current stale-exposure negatives, and DB08 zero state.

Executable oracle: `wp45_behavioral_acceptance`
Executable oracle: `wp45_structural_acceptance`
Executable oracle: `wp45_negative_zero_state`
Executable oracle: `wp45_operational_acceptance`

### WP46 — Continuous overlay rebase and durable flush (FAB AC-G-22)

**Dependencies:** WP42, WP44, WP45. **Outcome:** threshold-driven rebase/flush preserves snapshot,
overlay, pin, and publication invariants with idempotent retry and atomic visibility.

**Acceptance:** injected failure/retry/crash tests, before-or-after visibility, pinned-reader
isolation, and clean-rebuild equality.

Executable oracle: `wp46_behavioral_acceptance`
Executable oracle: `wp46_structural_acceptance`
Executable oracle: `wp46_negative_zero_state`
Executable oracle: `wp46_operational_acceptance`

### WP47 — Startup and crash recovery

**Dependencies:** WP41–WP46. **Outcome:** cold/warm/restart scenarios recover unfinished waves,
overlays, dirty owners, barriers, and snapshots without Git acceleration.

**Acceptance:** kill-point corpus, corrupt/missing operational state, replay idempotency, no stale
strict-current result, and resource cleanup.

Executable oracle: `wp47_behavioral_acceptance`
Executable oracle: `wp47_structural_acceptance`
Executable oracle: `wp47_negative_zero_state`
Executable oracle: `wp47_operational_acceptance`

### WP48 — Clean-rebuild comparator, core edit corpus, and CORE_SOURCE_V1

**Dependencies:** WP41–WP47. **Outcome:** SUITE AC-G-79 comparator and core edit corpus prove
continuous state equivalent to a clean rebuild; `CORE_SOURCE_V1` is advertised complete only for
the exact accepted coverage.

**Acceptance:** add `rebuild-equivalence-check` and `wave6-integration-check`; cover save bursts,
add/delete/rename/move, parse break/fix, generated bursts, overflow, and restart; compare canonical
state and diagnostics, not timing.

Executable oracle: `wp48_behavioral_acceptance`
Executable oracle: `wp48_structural_acceptance`
Executable oracle: `wp48_negative_zero_state`
Executable oracle: `wp48_operational_acceptance`

## 8. Wave 7 — Git-aware lifecycle acceleration

### WP49 — Phase 1: repository correctness

**Dependencies:** M07. **Outcome:** complete byte-safe `GitStateAdapter` DTO surface for repository,
worktree/common-dir, head/index/attributes/ignore/submodule topology, with explicit unavailable and
degraded states. No gix mutation or control-plane transaction role is introduced.

**Acceptance:** linked/bare/detached/unborn/non-UTF8/corrupt cases, gix-disabled equivalence, and
no direct domain leakage beyond DTOs.

Executable oracle: `wp49_behavioral_acceptance`
Executable oracle: `wp49_structural_acceptance`
Executable oracle: `wp49_negative_zero_state`
Executable oracle: `wp49_operational_acceptance`

### WP50 — Phase 2: Git-native inventory

**Dependencies:** WP49, WP41. **Outcome:** gix inventory implements the eight-class inclusion
taxonomy and produces candidates fenced by current bytes, attributes, ignores, submodules, and
worktree topology.

**Acceptance:** compare to fallback across ignored/untracked/conflicted/sparse/linked fixtures;
unknown or incomplete topology widens to fallback.

Executable oracle: `wp50_behavioral_acceptance`
Executable oracle: `wp50_structural_acceptance`
Executable oracle: `wp50_negative_zero_state`
Executable oracle: `wp50_operational_acceptance`

### WP51 — Phase 3: status and index acceleration

**Dependencies:** WP50. **Outcome:** bounded status/index acceleration for warm startup, rescan,
overflow, and ordinary edits while final bytes remain authoritative.

**Acceptance:** `git-parity-check`, zero full-status scans for accepted isolated-save scenarios,
and semantic equality under forced fallback. Scan counts are behavioral signals, not performance
SLOs.

Executable oracle: `wp51_behavioral_acceptance`
Executable oracle: `wp51_structural_acceptance`
Executable oracle: `wp51_negative_zero_state`
Executable oracle: `wp51_operational_acceptance`

### WP52 — Phase 4: bulk HEAD-tree acceleration

**Dependencies:** WP51. **Outcome:** tracked baseline and HEAD-tree diff accelerate branch and bulk
transitions, fenced by a typed `GitStateVector`, current-byte confirmation, and safe fallback.

**Acceptance:** branch switch, index conflict, rename, submodule and topology corpus; stale vector
and concurrent-worktree-change negatives; exact fallback equality.

Executable oracle: `wp52_behavioral_acceptance`
Executable oracle: `wp52_structural_acceptance`
Executable oracle: `wp52_negative_zero_state`
Executable oracle: `wp52_operational_acceptance`

### WP53 — Phase 5: shared caches, topology, degradation, and Wave-7 equivalence

**Dependencies:** WP49–WP52. **Outcome:** bounded L1/L2 caches and topology-aware invalidation
accelerate without authority; corruption, budget pressure, unavailable Git, and topology change
degrade safely.

**Acceptance:** add `wave7-integration-check`; gix-disabled and cache-disabled states equal the
accelerated result across the WP48 comparator corpus; cache eviction/corruption/restart and linked
worktree cases pass.

Executable oracle: `wp53_behavioral_acceptance`
Executable oracle: `wp53_structural_acceptance`
Executable oracle: `wp53_negative_zero_state`
Executable oracle: `wp53_operational_acceptance`

## 9. Milestones and decommission batches

### M05 — Wave 4 exit

**Dependencies:** WP27–WP33. `wave4-integration-check`, `model-release-census-check`,
`model-check profile="tier-a"`, `ci-fast`, `governance`, and `features-each` pass. Every raw kind
has one governed disposition and repeated extraction yields identical canonical state.

### M06 — Wave 5 exit / Readiness Gate B

**Dependencies:** WP34–WP40. `gate-b-check`, `wave5-integration-check`, cache-disabled/full
equivalence, `ci-fast`, `extractor-ci-fast`, and adapter wheel proof pass over owner-accepted golden
answers. Missing Gate-B authority blocks; it is never generated into acceptance.

### M07 — Wave 6 exit / CORE_SOURCE_V1

**Dependencies:** WP41–WP48, DB07–DB09 as applicable. `wave6-integration-check` and
`rebuild-equivalence-check` pass every core edit scenario; strict-current queries expose no
invalidated facts. Gate C remains open.

### M08 — Wave 7 exit

**Dependencies:** WP49–WP53. `wave7-integration-check`, `git-parity-check`, forced gix-disabled,
cache-disabled, and full-rebuild comparisons are semantically identical.

### DB07 — Synthetic canonical-ingest zero state

**Depends on:** WP36. No production `SyntheticCanonicalIngest` reference or construction remains;
test fixtures may continue to feed the production reconciliation engine. Enforced structurally and
at M07/M08.

### DB08 — Wave-5 freshness-shim zero state

**Depends on:** WP45. Every query admission path uses the AC-G-24 barrier and the bounded temporary
freshness shim is absent. Enforced structurally and at M07/M08.

### DB09 — Deprecated update-wave vocabulary zero state

**Depends on:** WP42. Accepted codes remain stable; historical terminal code 50 is decode-only;
nonterminal historical codes 30/40 are diagnosed/restarted; no writer, SQL default, generated
transition, fixture, or RPC encoder emits deprecated `RUNNING`, `PUBLISHING`, or `COMPLETE`.

## 10. Execution DAG and concurrency

```text
activation -> WP32 -> WP33 -> M05
M05 -> WP34 -> WP35 -> WP36 -> WP37 -> WP38 -> WP39 -> WP40 -> M06
M06 -> WP41 -> WP42 -> WP43 -> WP44 -> WP45 -> WP46 -> WP47 -> WP48 -> M07
M07 -> WP49 -> WP50 -> WP51 -> WP52 -> WP53 -> M08

historical trusted ancestors: WP27 -> WP28 -> {WP29 || WP30} -> WP31
DB07 closes with WP36; DB09 with WP42; DB08 with WP45; all recheck at M07/M08.
```

The exact direct dependency edges are the `Dependencies` fields above and are normative. Product
packets serialize by default because they share model authorities, generated families, runtime
consumers, and integration fixtures. Read-only research or isolated fixture preparation may run in
parallel only when it cannot write a shared authority, acceptance, DesiredTree output, state, or
generated consumer.

## 11. Gate matrix

The permanent command contract uses capability names only:

- just artifacts-check
- just plan-status
- just model-bootstrap-check
- just model-inventory-check
- just model-plan-check
- just model-release-census-check
- just model-family-check
- just model-transaction-check
- just model-repro-check
- just model-incremental-check
- just model-assurance-check
- just model-zero-state-check
- just model-release-check
- just governance
- just ci-fast
- just ci-pr
- just root-check
- just root-clippy
- just root-test
- just extractor-ci-fast
- just sidecar-ci-fast
- just adapter-ci-fast
- just adapter-wheel-test
- just stable-graph-check
- just features-each
- just features-no-default
- just deps-fast
- just policy
- just tracked-target-zero-state-check
- just typos

Packets WP33, WP40, WP48, WP51, and WP53 add only the stable product-capability recipes named in
their acceptance sections. A targeted fuzz invocation may be used when a new untrusted parser or
protocol surface lands. No packet-named mutation, proof-manifest, family-count, or legacy generator
recipe may be added.

## 12. Completion and replan policy

- [ ] successor explicitly approved and activated only through the remediation M05 handoff
- [x] WP27–WP31 historical proving commits preserved
- [ ] WP32 completed without raw allocation or recipe drift
- [ ] WP33 / M05 complete with Wave-4 integration standing green
- [ ] WP34–WP40 / M06 complete with owner-accepted Gate-B evidence
- [ ] WP41–WP48 / M07 complete; DB07–DB09 green; CORE_SOURCE_V1 complete
- [ ] WP49–WP53 / M08 complete with gix/cache disabled equivalence
- [ ] current-head model release, package, feature, policy, artifact, and status gates green
- [ ] no superseded governance reintroduced merely to satisfy a historical check

Replan immediately if a packet needs a second authority/writer, a hand-maintained aggregate list,
an acceptance artifact generated by the renderer it judges, an unbounded or non-replayable proof,
semantic dependence on a cache/Git hint, a new Cargo package, or a product change outside the
normative suite. Ordinary discovered files, typed model members, and derived outputs do not require
replanning when the existing family declaration and driver already describe them.

## 13. Approval and activation record

The repository owner explicitly approved this successor in the implementation-plan execution thread
on 2026-08-23. Approval changes only the frontmatter status and records an
`explicit_user_approval` judgment in its state; the plan remains inactive until the separate
model-control-plane WP15 activation event. Handoff commit H changes only the active pointer and
successor activation fields; seal commit S records H in the completed remediation state. No product
packet may execute between H and S.
