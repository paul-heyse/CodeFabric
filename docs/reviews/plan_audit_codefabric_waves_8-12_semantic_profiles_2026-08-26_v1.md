---
artifact: plan-audit
date: 2026-08-26
version: v1
status: complete
plan_path: docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v1_2026-08-26.md
verdict: needs-revision
---

# Plan Audit: CodeFabric Waves 8–12 semantic profiles

## Provenance and Scope

This audit independently reviewed the complete 4,091-line plan against the current
repository, its governing roadmap and 1.3 design suite, the authoritative semantic
and data-fabric design principles, and the version-pinned provider/data-fabric
library references.

Primary design evidence included `RM §§13–17, 27–28`, `GEN §§5, 17–25, 36–47,
59, 87–90`, `GEN AC-G-30`–`AC-G-35`/`AC-G-39`, `ONT` semantic-profile and
identity contracts, `FAB §§21–42, 51–59, 64–79A, 91`, `FAB AC-G-42`, `LIFE
§§7–8, 93–99`, and the relevant `QRY` completeness/context contracts. Doctrine
evidence included the full `semantic_design_principles_holistic.md`,
`full_data_fabric_design_principles.md`, and the DataFusion/Delta alignment
manuals. Library evidence included the exact Ruff, Pyrefly, Tree-sitter,
`rustc_public`/`rustc_private`, petgraph 0.8.3, DataFusion 55, Arrow 59, and
delta-rs `43a0cf10` references plus current manifests, lockfiles, and resolved
source where needed.

The plan's recorded baseline is `ea13ca41617dce93ea76349f34bfbd5739f7a5a2`.
Current `HEAD` is `66ab6e9fef7e12afcceba45a82ac026ca8f64d56`, four commits later;
the committed drift is 46 files, 5,660 insertions, and 604 deletions. The plan
correctly treats the current remediation plan as its predecessor and remains
inactive/draft, but current-tree facts still outrank its illustrative detail.

Validation evidence:

- direct artifact validation passed: 37 packets, 5 milestones, 3 decommission
  batches, unique IDs, resolvable paths, and matching declared-input digests;
- `RUSTC_WRAPPER= just stable-graph-check` passed on the current graph;
- the current `just ci-fast` baseline reached 352 passing Rust tests and passing
  doctests, then failed `deps-fast` on three unused sidecar dependencies
  (`hyper-util`, `prost`, `tonic-prost`); this is a measured predecessor-era
  baseline, not evidence that this inactive plan is green;
- the pre-existing dirty worktree was preserved. This audit adds only this report.

No earlier audit of this plan was found.

## Executive Summary

The plan is structurally strong and substantially faithful on provider isolation,
explicit unknowns, immutable source-digest fences, owner replacement, incremental
equivalence, and the core DataFusion/Arrow/Delta/petgraph choices. It is not ready
for approval or execution.

Two blockers require packet-graph revision. First, WP07 publishes reaching-
definition and def-use derivations before the registry that is the mandatory
single authority for those families is completed in WP35. Second, WP10 requires
Pyrefly module graph/export/xref facts, but the selected `Query` surface does not
provide them and the proposed type-table/callee “derivation” cannot reconstruct
them. The plan must select and compile-prove a Glean/LSP/internal adapter surface
before dependent packets are executable.

The remaining major findings are also material: the primary holistic doctrine is
absent; language-neutral foundations are assigned to the Python lane despite an
existing shared `TypeInterner`; the integrated-plan authorization and overlapping
write sets are not closed; sandbox implementation/status is under-specified;
mandatory fault, observability, and performance evidence is missing; the Ruff
dependency impact is incomplete; property authority can be repaired retroactively;
and the legacy zero-state proof is not a governed executable surface.

## Readiness Verdict

**Verdict: `needs-revision`.** The intended target architecture is not generally
invalidated, so `needs-redesign` is not warranted. Approval requires a new plan
version that resolves both blockers and all major findings. Localized minor fixes
can land in the same revision.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | design/sequence | WP07, WP35, M01 | open |
| F-002 | blocker | library/design | LD-PY-01, WP10–WP14, M02 | open |
| F-003 | major | doctrine | declared inputs, §2.2, all transformation packets | open |
| F-004 | major | design/sequence/factuality | WP02, WP11, WP15, WP18, §8 | open |
| F-005 | major | sequence/operations | integrated plan, WP04/WP05, §8 | open |
| F-006 | major | operations/library | WP36, WP08, WP16, M02–M04 | open |
| F-007 | major | operations/proof | Waves 8–12, final gates | open |
| F-008 | major | impact/library | LD-RF-01, WP03 | open |
| F-009 | major | design/proof | all table packets, WP30, M01–M04 | open |
| F-010 | major | legacy/proof/impact | DB01–DB03, WP08, WP16–WP19, M05 | open |
| F-011 | minor | impact/proof | WP03, WP19, final gate matrix | open |
| F-012 | minor | library/factuality | LD-PY-01, LD-TS-01 | open |

## Findings

### F-001 — Derived facts precede their mandatory single authority

**Severity:** blocker  
**Category:** design, sequence  
**Scope:** WP07, WP35, M01

**Finding:** WP07 required change 3 materializes `REACHING_DEFINITION`,
`REACHES`, `DEF_USE`, `DATA_DEP`, `VALUE_FLOWS_TO`, and `KILLS_DEFINITION` and
publishes them at M01, but it neither registers the family nor requires execution
through a registered derivation. WP35 defers completion and runtime enforcement of
the derivation registry until M05. This contradicts `GEN §87` (a generation
adapter shall not materialize a competing reaching-definition family), `FAB §79A`
and `AC-G-42` (every materialized derived family has exactly one registry entry),
and `LIFE §97` (the scheduler invokes registered entries). The current registry and
runtime contain only `SYNTAX_TREE_V1`, confirming that no inherited entry closes
the gap.

**Required resolution:** Move registration and registry-selected execution for
the Wave 8 reaching-definition/def-use family into WP07 or a predecessor foundation
packet. WP07 must stamp the selected profile/bundle and prove that no generation
adapter can publish the family independently. WP35 may audit and complete the
remaining matrix, but must not introduce authority retroactively.

**Revalidation:**

```bash
just model-repro-check && just packet-oracle-check WP07 && just wave8-integration-check
```

### F-002 — WP10 depends on a Pyrefly surface the selected API does not expose

**Severity:** blocker  
**Category:** library, design  
**Scope:** LD-PY-01, WP10–WP14, M02

**Finding:** `GEN §5.1`, `§18.4`, and `§19.2`–`§19.4` assign cross-module
definitions/references, import resolution, and exports to a Pyrefly Glean/LSP/
internal adapter. LD-PY-01 selects `Query::{add_files, change_files,
get_type_table_in_file, get_callees_with_location, get_attributes,
resolve_target_from_qualified_name, is_subtype}`. None returns the module graph,
bulk definitions/xrefs, or export index required by WP10. The plan acknowledges the
absence, then says the type-table/callee machinery plus application derivation is
the planned fallback; those observations do not contain enough information to
derive the missing facts. At exact revision
`1933169ad8ee9e4d4114112eb56ef0811fb0a094`, Glean conversion exists behind
Pyrefly's private `report` module, so using it requires a deliberate, compile-proved
internal surface, CLI path, or source change.

**Required resolution:** Before WP10, add a bounded capability packet/probe that
selects one exact Glean, LSP, or internal-adapter surface, records its version and
feature boundary, defines the sidecar protocol impact, and proves module/import/
export/xref fixtures at the pinned revision. Revise LD-PY-01 and WP10 to that
decision. If a Pyrefly source patch is required, version the plan accordingly before
execution rather than discovering it after WP09.

**Revalidation:**

```bash
just pyrefly-module-xref-surface-check && just packet-oracle-check WP10
```

### F-003 — The primary holistic doctrine and pass contracts are absent

**Severity:** major  
**Category:** doctrine  
**Scope:** declared inputs, §2.2, all transformation packets

**Finding:** The declared-input ledger and §2.2 trace only the data-fabric P1–P25
doctrine. They omit `docs/library_ref/semantic_design_principles_holistic.md`, the
authoritative cross-cutting doctrine for identities, transformations, adapters,
runtime ownership, security, and observability. Bare identifiers such as `P1` and
`P3` therefore also collide with different meanings in the holistic doctrine.
Most materially, holistic §6 requires explicit pass contracts, but the plan does
not allocate pass-contract artifacts for Ruff semantic traversal, CFG/dataflow,
type interning, reconciliation, or registered derivations.

**Required resolution:** Add the holistic doctrine and digest to declared inputs;
namespace principle identifiers; add a doctrine-conformance matrix; and require
explicit pass contracts covering inputs, outputs, entry/exit invariants, preserved/
generated identities, invalidation, diagnostics, determinism, and fixtures for each
load-bearing transformation.

**Revalidation:**

```bash
rg -q 'semantic_design_principles_holistic\.md' docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md && just design-principle-traceability-check
```

### F-004 — Language-neutral foundations are incorrectly owned by Python packets

**Severity:** major  
**Category:** design, sequence, factuality  
**Scope:** WP02, WP11, WP15, WP18, §8

**Finding:** The plan makes Rust context discovery depend on WP02 and Rust type
semantics depend on WP11 because Python “reaches” shared context/type authority
first. This serializes independent language lanes despite `RM §4.1` making Wave 12
their integration barrier. It is also stale against the current tree:
`src/identity.rs` already defines the language-neutral `TypeConstructor`,
`TypeTerm`, and shape-checked `TypeInterner`; WP11's “new type-interning module”
does not exist as a new requirement.

**Required resolution:** Put common analysis-context discovery interfaces,
language-neutral tables, canonical type algebra, and interner ownership in WP01/
WP36 or a new foundation packet. Make WP02/WP15 and WP11/WP18 independent
consumers, reuse/extend the existing interner, and remove false cross-lane
dependencies while preserving real shared-file serialization.

**Revalidation:**

```bash
! rg -n 'WP15 requires WP02|WP18 requires WP11|new type-interning module' docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md && just plan-dependency-check
```

### F-005 — The integrated form does not close RM §28's authorization and write-set rules

**Severity:** major  
**Category:** sequence, operations  
**Scope:** integrated plan, WP04/WP05, §8

**Finding:** The introduction states that RM §28 “authorizes” an integrated plan,
but RM §28 permits it only when explicitly authorized by the design owner/user.
No accountable authorization provenance is recorded. RM §28 also requires parallel
packets to have disjoint write sets or explicit serialized edges. The plan records
WP04 and WP05 as parallel while both touch `src/ruff_adapter.rs`; “coordinate
through the lead” is not a dependency edge or machine-checkable disposition. The
same risk applies to the listed shared registries/core files unless every unordered
overlap is encoded.

**Required resolution:** Record the accountable authorization principal, date,
and source. Enumerate all unordered known-touch overlaps; split the surface,
serialize packets, or add an explicit integration packet for each. Ensure the
target plan's dependency checker rejects undisposed overlaps.

**Revalidation:**

```bash
rg -q '^Integrated-plan authorization:' docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md && just plan-dependency-check
```

### F-006 — Sandbox implementation and capability status are not decision-closed

**Severity:** major  
**Category:** operations, library  
**Scope:** WP36, WP08, WP16, M02–M04

**Finding:** `GEN AC-G-35` specifies more than “namespaces”: Linux requires
user/mount/PID/network namespaces, read-only binds, `no_new_privs`, seccomp, and
Landlock or equivalent path restrictions; macOS requires an approved Seatbelt
profile or an explicit `TRUSTED_LOCAL` grant. The plan has no load-bearing platform
decision record naming how those controls are implemented, provisioned, featured,
and CI-proved, while §1.2 limits new dependencies to Ruff crates. Current root
`rustix` is optional with only `fs`, and no existing sandbox implementation closes
the gap. WP08 additionally permits completion with trusted-local fixtures when
containment is unavailable without consistently demoting the advertised production
host/profile capability.

**Required resolution:** Add an `LD-SB-*` platform decision with exact Darwin and
Linux mechanisms, dependency/tool/feature impact, profile construction and digest,
resource limits, child containment, and deployment prerequisites. Add a host
capability matrix: a platform lacking containment may complete only with the
untrusted profile explicitly unavailable/partial; it cannot certify
`UNTRUSTED_SANDBOXED`. Exercise network, write, credential, FD, process-tree,
resource, cancellation, and cleanup escapes on every advertised host.

**Revalidation:**

```bash
just semantic-sandbox-host-matrix-check && just packet-oracle-check WP08 && just packet-oracle-check WP16
```

### F-007 — Mandatory fault, observability, and performance evidence is unallocated

**Severity:** major  
**Category:** operations, proof  
**Scope:** Waves 8–12, final gates

**Finding:** RM §27.3 requires deterministic fault points at every new state,
process, write, pointer, artifact, and long-running boundary. RM §27.5 requires
each wave to define phase traces, counters, queue depths, memory, spill, cache,
cancellation, and failure metrics. RM §28 also requires performance measurements
even before the final performance gate. The plan has useful scattered failure
fixtures and journals, but no per-wave fault/telemetry contract, metric lifecycle/
cardinality, or final observability gate. Its risk section makes performance recipes
conditional on regression suspicion, which does not satisfy the roadmap.

**Required resolution:** Allocate fault-point registration and per-wave
observability/resource contracts to the owning packets; require failure oracles to
assert emitted telemetry as well as behavior; add bounded baseline measurements for
the new provider, semantic-stream, derivation, reconciliation, spill, and query
paths; and name aggregate recipes in each milestone/final gate.

**Revalidation:**

```bash
just semantic-fault-point-check && just semantic-observability-contract-check && just semantic-profile-bench
```

### F-008 — WP03 omits load-bearing Ruff dependency graph changes

**Severity:** major  
**Category:** impact, library  
**Scope:** LD-RF-01, WP03

**Finding:** WP03 adds `ruff_python_semantic` and conditionally unnamed helper
crates, but its Known Touch omits `Cargo.lock` and
`scripts/stable_graph_check.sh`. The current graph checker carries both an exact
Ruff crate census (lines 49–61) and an exact `fact-generation` feature allowlist
(lines 98–103); WP03's own `just stable-graph-check` must fail until both change.
An “if required” dependency branch is not a closed exact-version decision.

**Required resolution:** Name every required 0.0.7-train crate after a compile
probe; add `Cargo.lock` and `scripts/stable_graph_check.sh` to Known Touch; and
update the exact pin, feature-shape, default-tree, and narrow-graph assertions.

**Revalidation:**

```bash
RUSTC_WRAPPER= just stable-graph-check && just features-each && just root-check
```

### F-009 — Property authority may be repaired only after four milestones

**Severity:** major  
**Category:** design, proof  
**Scope:** all table packets, WP30, M01–M04

**Finding:** The group rule says every packet registers each introduced property
at landing and WP30 performs “never late registration.” WP30's legacy disposition
nevertheless permits any Wave 8–11 property that landed without a registry entry
to be “regularized here” at M05. That allows M01–M04 artifacts outside `ONT
AC-G-71`'s registry-generated schema/cardinality authority and treats a contract
violation as migration work.

**Required resolution:** Make missing property registration a packet/milestone
failure from the first table packet. Move the unregistered-property rejection into
the shared model gate used by every table packet. WP30 may audit predecessor
closure and generate enforcement, but must fail rather than repair a missing
authority record.

**Revalidation:**

```bash
just property-registry-closure-check && just model-repro-check && just wave8-integration-check
```

### F-010 — Legacy zero state is neither complete nor governed as an executable gate

**Severity:** major  
**Category:** legacy, proof, impact  
**Scope:** DB01–DB03, WP08, WP16–WP19, M05

**Finding:** DB01–DB03 describe exclusions in prose but define no verbatim recipe
with a reviewed candidate/allow set, and the final gate matrix names no
decommission zero-state command. Current code shows why the scope matters:
`src/fabric/serving.rs:2812`–`2831` calls the Gate B direct-spawn helpers but is
absent from WP08/WP16's named cutover surface; the sidecar and stable root retain
hand-authored observation-schema includes/opaque JSON columns; and the extractor/
ingest path retains MIR string summaries. A textual “outside production modules”
scan is not executable until those allowed paths are enumerated.

**Required resolution:** Add one governed semantic-provider legacy-zero-state
recipe covering direct provider spawning, hand-authored observation authority, and
opaque ingest inputs. Encode reviewed allowed paths; combine `rg`, structural
`ast-grep`, and clean build/domain checks; include `src/fabric/serving.rs`; and name
the recipe in DB01–DB03, every applicable milestone, and the final gate matrix.

**Revalidation:**

```bash
just semantic-provider-legacy-zero-state-check && just root-check && just sidecar-check && just extractor-check
```

### F-011 — Two preflights and one final gate are not executable as written

**Severity:** minor  
**Category:** impact, proof  
**Scope:** WP03, WP19, final gate matrix

**Finding:** WP03 and WP19 use `jq '.tables | keys'`; `.tables` is an array, so
the command emits numeric indices and cannot discover `scope`, `binding`,
`reference`, or `mir` names. The correct surface is `.tables[].name`. The final
gate `just packet-oracle-check <WP01…WP37>` is also a placeholder rather than a
valid recipe invocation or shell expansion.

**Required resolution:** Replace both `jq` commands with baseline-aware
`.tables[].name` queries. Add an aggregate recipe that deterministically enumerates
all 37 packet selectors, or spell out a valid loop.

**Revalidation:**

```bash
! rg -n "jq '\.tables \| keys'|packet-oracle-check <" docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md
```

### F-012 — Two library decisions do not record exact current identities

**Severity:** minor  
**Category:** library, factuality  
**Scope:** LD-PY-01, LD-TS-01

**Finding:** LD-PY-01 says “source/tag” and `1.2.0` instead of the exact Pyrefly
revision `1933169ad8ee9e4d4114112eb56ef0811fb0a094`. LD-TS-01 says only
“pinned grammars” rather than recording `tree-sitter 0.26.12`,
`tree-sitter-python 0.25.0`, and `tree-sitter-rust 0.24.2`. These are
load-bearing unstable/versioned boundaries, so shorthand is not sufficient plan
provenance.

**Required resolution:** Replace both version bases with exact manifest identities,
features, and the gate that proves them.

**Revalidation:**

```bash
RUSTC_WRAPPER= cargo metadata --manifest-path pyrefly-sidecar/Cargo.toml --locked --format-version 1 >/dev/null && RUSTC_WRAPPER= just stable-graph-check
```

## Target-Design Assessment

The clean-sheet target remains directionally correct: application-owned facts and
identity, provider adapters, immutable source/context inputs, typed observation
streams, a sole reconciler, owner-scoped replacement, immutable serving snapshots,
and explicit unknown/capability semantics are appropriate. The plan also correctly
keeps the FastMCP layer thin and compiler/Pyrefly types out of the stable root.

The preferred clean-sheet architecture differs in two places. Shared analysis-
context and canonical-type authority belongs in a language-neutral foundation, not
in whichever language lane happens to land first. Registered derivation authority
must precede every derived publication, not arrive as an integrated Wave 12 cleanup.

## Library Capability Assessment

The following choices were confirmed at their pinned versions:

- DataFusion 55 built-in joins, windows, `row_number`, left anti joins, bounded
  memory pools, reservations, spilling, and metrics support LD-DF-01 without a
  custom UDF/operator;
- delta-rs `43a0cf10` exposes the selected create, commit retry/application-
  transaction, owner-replacement, and exact-version snapshot surfaces already
  compile-bound in the current fabric;
- Arrow 59 typed builders, `RecordBatch` validation, and row conversion support
  the columnar/row-relation policy;
- petgraph 0.8.3 `DiGraph`, application-owned identity maps, `toposort`, and
  reachability are appropriate ephemeral mechanisms;
- Ruff 0.0.7 provides the semantic model, but construction is not analysis, so
  the plan correctly treats traversal reproduction as a pinned adapter risk;
- Pyrefly Query, `rustc_public`/the narrow private adapter, and Tree-sitter support
  the type/callee/member, MIR/compiler, and source-correspondence uses recorded in
  their decisions.

F-002 is narrower but load-bearing: the selected Pyrefly Query API does not cover
WP10's bulk cross-module/export/xref outcome. F-006 and F-008 concern unclosed
platform/dependency impacts, not rejection of their overall architectural roles.

## Work-Packet and Impact Assessment

All 37 packets satisfy the repository's structural schema and carry four named
oracle classes. Most outcomes are observable and the per-wave packet counts fit RM
§28. Structural validity does not close the authority/sequence defects in F-001,
F-004, F-005, and F-009, the unavailable WP10 surface in F-002, or the exact impact
misses in F-008/F-011.

The current tree already contains material seeds that revised preflights must use:
`TypeInterner`, `CanonicalReconciliationEngine`, `ProviderRuntime`, direct provider
verticals and their serving consumer, a one-entry derivation registry, opaque
sidecar/MIR ingest formats, and exact feature-graph governance.

## Legacy, Transition, and Decommission Assessment

The plan correctly names direct provider spawning, hand-authored observation
schemas, and opaque semantic evidence as decommission targets, uses split lane
prerequisites, and rejects permanent dual authority. It does not yet make their
zero state reproducible or complete. F-010 is required so the intended transition
cannot leave a hidden invocation or ingest path, and F-001/F-009 prevent Wave 12
from being used to retroactively legitimize earlier unregistered authority.

## Proof and Validation Assessment

The plan has unusually strong behavioral, structural, negative, operational, and
incremental-equivalence intent. The main proof weaknesses are architectural: a
named oracle cannot prove a library observation that the selected API cannot
produce; final broad gates cannot replace a derivation/property authority present
at first publication; prose scans are not zero-state gates; and RM §27's fault/
telemetry obligations are not allocated. The measured current `ci-fast` failure
must be re-derived and dispositioned in WP01 after the predecessor freezes.

## Doctrine and Anti-Principle Assessment

The plan advances information hiding, ports/adapters, stable identity, explicit
failure semantics, reproducibility, provenance, and single-source contract models.
F-003 is nevertheless material: omitting the authoritative holistic doctrine makes
its bare `P*` claims ambiguous and leaves required transformation pass contracts
unowned. F-001 and F-009 also risk the anti-principle of distributed authority by
allowing facts/properties to exist before their registry owner is enforced.

## Top Required Changes

1. Move derivation registration/enforcement ahead of WP07 publication.
2. Select and compile-prove a Pyrefly module/export/xref surface before WP10.
3. Extract shared context/type authority into a lane-neutral foundation.
4. Add holistic-doctrine traceability and explicit pass contracts.
5. Close RM §28 authorization/write-set requirements and AC-G-35 host decisions.
6. Add first-publication property closure, governed legacy zero-state, and the
   missing fault/observability/performance gates.
7. Correct exact Ruff graph impact, invalid commands, and version identities.

## Re-Audit Scope

Re-audit the complete successor plan version, not only changed paragraphs, because
F-001/F-002/F-004/F-005 alter packet dependencies and milestone closure. At minimum
re-run direct artifact validation, target-plan dependency/overlap validation, the
Pyrefly capability probe, declared-input freshness, exact feature-graph validation,
and the new derivation/property/sandbox/observability/legacy-zero-state recipes.
