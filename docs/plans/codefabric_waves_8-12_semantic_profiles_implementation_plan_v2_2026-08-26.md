---
artifact: implementation-plan
plan_id: codefabric-waves-8-12-semantic-profiles
version: v2
date: 2026-08-26
status: draft
design_path: docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md
design_version: v1.0
baseline_commit: ea13ca41617dce93ea76349f34bfbd5739f7a5a2
working_tree_digest: 697327d9a5549549c216513e894764adf0f3bfdcf4227c55e292eabddc7266af
state_path: docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json
activation_requires: codefabric-design-principles-full-alignment-review-remediation/M05
cutover: true
---

# CodeFabric Waves 8–12 semantic profiles — implementation plan v2

This is the integrated program plan for roadmap Waves 8–12: the Python semantic lane
(W8 → W9), the Rust semantic lane (W10 → W11), and the Wave 12 integration barrier
(full reconciliation, completeness, contexts, and unknown remainder). `RM §28`
authorizes an integrated consecutive-wave plan when one cross-wave graph materially
improves dependency and cutover safety; the two language lanes share the provider
runtime, the observation-schema pipeline, the fact-table substrate, and the
reconciliation engine, so a single dependency graph is materially safer than five
disjoint plans. Execution remains wave-segmented: each wave group closes at its own
milestone, and no later wave certifies an incomplete predecessor gate.

Integrated-plan authorization: user/design owner Paul Heyse, 2026-08-26, by the
request to create this v2 integrated plan from the independent v1 plan audit. This
records the accountable authorization required by `RM §28`; it does not approve or
activate the draft plan.

## 1. Outcome and non-goals

### 1.1 Outcome

At M05, the CodeFabric daemon maintains a continuously updated, canonically
reconciled semantic present-state substrate for Python and Rust:

1. **Wave 8 (M01).** Python local semantics derived from Ruff and application-owned
   analysis are current: analysis contexts, scopes, bindings, references, imports,
   exports, callable contracts, call sites, argument binding, CFG, and owner-local
   direct def-use, with explicit unknowns for the dynamic remainder.
   `PYTHON_SEMANTIC_V1` is advertised `PARTIAL` with the Pyrefly-owned mandatory
   capabilities explicitly named as missing.
2. **Wave 9 (M02).** Project-aware Python semantics arrive through the production
   Pyrefly sidecar behind the `codefabric.pyrefly.v1` protocol: module and symbol
   resolution, canonical type enrichment, object model and member resolution, and
   call-target enrichment, reconciled against the Ruff lane. `PYTHON_SEMANTIC_V1`
   is `COMPLETE` for the selected corpus and context, and incremental Python
   scenarios compare equal to a clean rebuild.
3. **Wave 10 (M03).** The stable-daemon-to-nightly-extractor boundary is a
   production provider: Rust context discovery, sandboxed extractor execution, the
   full invocation-manifest acceptance protocol, semantic definitions and types,
   MIR bodies, CFG, and call facts under a pinned context. Compile failure yields
   current source and syntax plus explicit compiler capability gaps — never stale
   semantic facts. `RUST_SEMANTIC_V1` is advertised `PARTIAL`.
4. **Wave 11 (M04).** The Rust profile closes: places and access events, ownership
   and initialization state, the narrow `rustc_private` enrichment adapter,
   monomorphized instances and dynamic dispatch, macro/generated/lowered
   correspondence, and drop/unsafe/const/FFI facts. `RUST_SEMANTIC_V1` is
   `COMPLETE` for the selected corpus/context, and compile break/fix and
   signature/trait incremental scenarios compare equal to a clean rebuild.
5. **Wave 12 (M05).** All provider lanes integrate into one sound canonical state:
   the complete `AC-G-37` reconciliation pipeline as the single canonicalization
   authority, property cardinality and storage integrity, formal capability
   aggregation, explicit unknown remainder and negative facts, the completeness and
   negative-proof algebra (`PROVEN_EMPTY` only when the algebra permits), enforced
   multi-context partitioning with endpoint-only external dependencies, the Static
   FFI Linking Profile v1, and the completed derivation materialization registry.
   `CORE_SOURCE_V1`, `PYTHON_SEMANTIC_V1`, and `RUST_SEMANTIC_V1` are revalidated
   against their exact profile requirements.

Every canonical semantic fact flows through the data fabric exactly as the design
constitution requires: registered Arrow observation schemas at the provider
boundary, typed builders and validated `RecordBatch` streams, owner-scoped Delta
replacement writes with commit provenance, manifest-pinned snapshot serving through
DataFusion, and DataFusion-planned reconciliation inside the `ReconciliationEngine`
boundary. Petgraph remains an ephemeral in-memory construction and validation
instrument; canonical identity never derives from graph indices.

### 1.2 Non-goals

- **No advanced-flow or interprocedural work.** Alias/points-to, dominators, SCC
  condensation, liveness materialization, effects, resources, concurrency, and
  interprocedural summaries are Waves 13–14 (`RM §17` deferred list; `FAB §79A`
  places them in the derivation registry with their own owners). Wave 8/11 direct
  def-use is the owner-local reaching-definitions family `GEN §25.5`/`GEN §45`
  assign to those waves — nothing more.
- **No semantic-query compiler expansion.** The eight QRY forms and their executors
  are governed by the predecessor remediation plan; Wave 12 packets extend query
  *responses* (coverage, completeness, context tagging) only where `AC-G-48` and
  `AC-G-51` require it, and reopen no form contract.
- **No movement from the FAB §2.1 dependency baseline**: DataFusion 55.0.0,
  Arrow/Parquet 59.2.0, `object_store` 0.13.2, delta-rs `43a0cf10`, petgraph 0.8.3
  (`std` only), gix 0.86.0. New direct Rust dependencies are limited to the exact
  Ruff 0.0.7 semantic crate and `clap` 4.6.2 already resolved by the pinned Pyrefly
  graph (LD-RF-01/LD-PY-02). Linux production deployment additionally requires
  bubblewrap 0.11.2 with setuid mode disabled (LD-SB-01); this is a host tool, not
  a Cargo dependency.
- **No custom DataFusion UDF, `LogicalPlan::Extension`, `ExecutionPlan`, or query
  planner** for reconciliation or ingest. Built-in `Expr`/`LogicalPlanBuilder`
  joins, windows, and anti-joins are the ceiling; any exception requires a
  versioned `ExtensionDecisionRecord` first (`FAB §72`).
- **No new Cargo root, no root workspace, no second top-level Rust test target, no
  native Python extension, and no Python Arrow/DataFusion processing layer.** The
  three existing process domains absorb all work.
- **No relaxation of provider isolation**: no compiler-private or Pyrefly type
  enters the stable root; the `rustc_private` enrichment adapter lives only in
  `rustc-extractor/` (LD-RS-02).

### 1.3 Baseline, predecessor disposition, and activation constraint

Baseline is `ea13ca41617dce93ea76349f34bfbd5739f7a5a2` with a dirty working tree
(`working_tree_digest` = SHA-256 over `git diff HEAD` concatenated with the sorted
untracked-file list). Every dirty and untracked path at planning time belongs to
the **active** remediation plan
(`docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v1_2026-08-26.md`,
state `executing`: WP01–WP04 complete; WP05/WP06/WP08 in progress; WP07 gated on a
human acceptance checkpoint). Waves 0–7 are complete
(`docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json`, all packets
`complete` with trusted proving commits).

Per `RM §0`, exactly one plan and one schema-current execution state may be mutable
at a time. This plan is therefore `draft` and **inactive**: `activation_requires`
names the remediation plan's M05 (accountable release and independent
certification). Activation is the normal sealed handoff — `just plan-activate`
creates the schema-2 state file and swaps the active pointer only after the
predecessor freezes. WP01 of this plan revalidates the inherited surface before any
product work, because the remediation plan's WP05/WP06/WP08 land on exactly the
files this plan extends (`src/pyrefly_service.rs`, `src/gate_b_candidate/vertical.rs`,
`pyrefly-sidecar/src/server.rs`, `rustc-extractor/src/wrapper.rs`,
`contracts/schema/provider-observations/`, `src/fabric/serving.rs`,
`src/lifecycle.rs`). Illustrative symbol names taken from the dirty tree may drift
before activation; the current repository is higher authority than plan detail, and
WP01's preflight re-derives them.

The declared-inputs digests below include the four `docs/upfront_design/` documents
the remediation plan edited in place (flagged in its state as *stale pending
accountable acceptance*). If accountable review changes any section this plan
cites, `just plan-status` reports the drift and the replan policy in §9 applies.

## 2. Source design and declared inputs

The design authority is the synchronized 1.3 suite. `RM §§13–17` fixes the wave
boundaries and work packages; the domain specs own the normative semantics; the
wave-traceability index records the citation corrections this plan adopts (W10/W11
off-by-one on `GEN §40`/`§42`/`§51`; `GEN` Part VII/VIII added to W12). Citations
use `TAG §N` per `docs/spec_index/README.md §2`; section numbers were confirmed
against `just spec-outline`/`just lib-outline` during planning.

| Path | sha256 |
|---|---|
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | 2b97f278d112ab1d7b4d5f40746f86832720edb853d2a9be8576353475d77376 |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | 9c7780c8e23b61ce8791f7b9fdb9d82c5e4a6df2cb67d6337ded06dc74910b3e |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | d72a302255daff31fe8e3c85e639239dac3246408fd1d1f63a9b6fd7f2d2b502 |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 0af56701dbcea6d479b0c856b362e86773ec8bd40bd34088a1d9cba575549ec2 |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | f892b6a18fa07e914ff3829937bd6bdfcb7632b4abebfed2dec51c0fa7a09647 |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | 0bc1e7d13a138e54f10bbf4b3930d97491a80176d84ddf27568bb42edc477956 |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 4bb8d7b4e4998b7215beb60e63580f2ad5207a0346d03efd2665be6490431984 |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md | 794a4ecbb38cd90d7ca4506a33c5e8c4b32e209d9a2f9b9429290f96c9af9fc1 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/petgraph.md | 8f5b19b2d9fbb9dfe2caf974b2a1f4c55b9244cfd167eb48956d225a076cccd9 |
| docs/library_ref/tree_sitter_rust_python.md | 615ce801958a3e74bf8cbbd5d759bade62958b1e4297185ebe8b18aa87e1428b |
| docs/library_ref/ruff_python_crates_advanced_reference_2026-08-18.md | f42e0b5e3d63c66bde68e2c3b79cef04d288eec52ee64e424f3d95578fc386d6 |
| docs/library_ref/pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md | 208582927c109dde0d399a7277442417006c878dfd21403a09a5c0bc7b2819e1 |
| docs/library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md | 1584a4ca9c7a06a495cfedacf585717aaec61949546d0191b19df48451451ea5 |
| docs/spec_index/wave-traceability.md | fb3b6c5da4b531e2a84cb2c14e911edaf99052a270d287ded7d64b7626c1e194 |

`docs/spec_index/` rows are navigation evidence, never normative; the plan cites
the sections they point at. Library-reference aliases used in packet citations:
`df` = the DataFusion reference, `arrow` = the Arrow reference, `delta` = the
deltalake reference, `align`/`delta-align` = the two alignment manuals, `ruff` /
`pyrefly` / `mir` / `ts` = the four provider references, `pg` = petgraph.md.

### 2.1 Library decisions

### LD-DF-01 — DataFusion reconciliation and ingest plan families

**Decision:** adopt (extend current use)
**Version basis:** DataFusion 55.0.0 / Arrow 59.2.0 exactly as pinned by `FAB §2.1`.
**Displaces:** ad hoc growth of the row-loop reconciliation in
`src/fact_ingest.rs::reconcile_candidates` for the high-volume Wave 12 families.
The engine boundary and output schemas stay application-owned (`FAB §72`); the
source-range join, declaration join, type reconciliation, call-target
partitioning, and unknown anti-join become built-in DataFusion plans — joins,
window `row_number()` over an integer authority rank, hash aggregation — per
`FAB §73`–`§74`, `df §11`–`§12`, `df §23`, `df §43`. Workspace-scale joins get
memory reservations and spill per `df §28`/`§54` (`align` RUN-09).
**Risk:** memory pressure on workspace-wide joins; a plan family that cannot be
expressed with built-ins tempts a custom operator. Mitigation: RUN-09 bounds;
`FAB §72` requires an `ExtensionDecisionRecord` before any non-built-in mechanism —
that is a replan trigger, not an implementation choice.
**Validation:** `canonical_reconciliation_pipeline_conformance` and
`reconciliation_plan_family_operational_gate` (WP29); `just root-check` proves the
graph compiles against the pinned resolver.

### LD-DL-01 — Delta fact-table expansion under owner-scoped replacement

**Decision:** adopt (extend current use)
**Version basis:** deltalake 1.0.0 exact revision `43a0cf10`.
**Displaces:** nothing — extends the generated `TableSpec` catalog with the
semantic extension tables of `FAB §§21–42` and `§§51–59`. Creation via
`CreateBuilder` from registry `StructType` with partition columns, constraints,
and metadata (`delta §8.2`–`§8.8`; `delta-align` SCH-01–SCH-10, GOV-01–GOV-03);
writes stay delete+append owner replacement under the publication-manifest
visibility rule with `CommitProperties` metadata and application transactions
(`delta §5.13`, `§5.16`–`§5.17`, `§9.4`–`§9.6`; `FAB §69`–`§70`); every table
joins the exact-version snapshot catalog (`delta §6.5`, `§6.24`; `FAB §91`).
Advanced protocol features stay off (`FAB §67`).
**Risk:** `with_application_transaction`/`with_max_retries` have no section in the
delta reference (documented absence). They are already exercised in
`src/fabric/mutation.rs`; the binding evidence is the pinned-source compile probe,
not a doc citation.
**Validation:** the schema round-trip gate (`FAB §11.1`) inside each table
packet's structural oracle; `just data-fabric-upgrade-check` at milestones.

### LD-AR-01 — Arrow encoder policy for high-volume semantic facts

**Decision:** adopt (extend current use)
**Version basis:** Arrow/Parquet 59.2.0.
**Displaces:** any serde-row path in provider adapters. Typed builders with
capacity preallocation are the sole hot path; FAB §64 starting batch sizes
(16 384 / 65 536 / 131 072 by table family); durable schemas use `Utf8` never
`Utf8View`, dictionary encoding only in transient batches; `Struct`/`List`
confined to bounded cohesive payloads — CFG edges, type components, arguments,
access-path components stay **row-oriented relations** (`FAB §65`, `arrow §5`–
`§7`); batch validation with Arrow kernels before Delta entry (`FAB §66`).
**Risk:** nested-type temptation for access paths and CFG payloads; rejected by
`FAB §65.4` and checked structurally in the owning packets.
**Validation:** per-family encoder unit tests plus each table packet's structural
oracle; `just root-test`.

### LD-PG-01 — Petgraph as ephemeral construction and validation instrument

**Decision:** adopt (bounded)
**Version basis:** petgraph exact 0.8.3, feature set `std` only (unchanged).
**Displaces:** hand-rolled graph bookkeeping in the Python CFG builder (WP06) and
reconciliation/ingest dependency ordering (WP29): `DiGraph` with an application
`HashMap<DomainId, NodeIndex>` identity map (`pg §2.2`, `§4`, `§12.4`–`§12.6`,
`§11.10`–`§11.17`), `toposort`'s error-returning form for order-with-cycle-
diagnosis (`pg §16.10`, `§16.16`), `has_path_connecting` for CFG entry/exit
well-formedness (`pg §16.3`).
**Displaces (negative):** nothing derived — `tarjan_scc`, `condensation`,
`dominators::simple_fast`, and all reaching-definition materialization are Wave 13
families owned by the derivation registry (`FAB §79A`, `AC-G-42`); materializing
them here would create a second derivation authority.
**Risk:** `NodeIndex` leaking into identity or persisted rows. Canonical CFG facts
carry application `cfg_node_id`s; graph indices die with the builder.
**Validation:** `py_cfg_wellformedness_parity` (WP06);
`competing_derivation_authority_falsification` (WP35).

### LD-RF-01 — Ruff 0.0.7-train semantic crates for the Python local lane

**Decision:** adopt (new direct dependencies)
**Version basis:** the exact pinned Ruff 0.0.7 train already used by
`src/ruff_adapter.rs` (`ruff_python_ast`, `ruff_python_parser`, …); WP03 adds
exactly `ruff_python_semantic = 0.0.7`. A compile probe precedes manifest editing;
if reproducing the semantic traversal needs any additional direct crate, that is
a plan/library-decision revision rather than an unnamed implementation branch.
**Displaces:** any custom Python scope/binding resolver as primary authority
(`GEN §5.1` keeps a custom AST scope builder only as fallback).
**Risk:** **`SemanticModel::new` is construction, not analysis** (`ruff §8.3`):
the binding→traversal→cleanup pass is Ruff-internal, so WP03 builds a
version-pinned adapter traversal that reproduces it. This is the largest hidden
Wave 8 cost and carries its own replan trigger. All Ruff IDs are arena-local and
never persist (`ruff §8.1`). The Ruff `cfg` module is internal-use-only — the CFG
stays application-owned (`ruff §8.19`).
**Validation:** `py_scope_binding_fixture_conformance` against the `GEN §93.1`
fixture corpus; `just stable-graph-check` proves the dependency graph stays inside
the declared universe, including the exact Ruff census and narrow `fact-generation`
feature allowlist.

### LD-PY-01 — Pyrefly pinned-source long-lived query surface

**Decision:** retain-current (deepen use)
**Version basis:** Pyrefly 1.2.0 at exact Git revision
`1933169ad8ee9e4d4114112eb56ef0811fb0a094`, `default-features = false`, as
locked by `pyrefly-sidecar/Cargo.toml`/`Cargo.lock`.
**Displaces:** nothing — extends `pyrefly-sidecar/src/pyrefly_link.rs` beyond the
two wired queries. Available and load-bearing: `Query::add_files`/`change_files`,
`get_type_table_in_file` (structural type table + response-local indices),
`get_callees_with_location`, `get_attributes`, `resolve_target_from_qualified_name`,
`is_subtype` (`pyrefly §9`–`§13`). One long-lived `Query` per context;
`Require::Everything` only for extracted files (`pyrefly §30.1`, `§6.1`).
**Risk:** the query module is marked not-for-external-use with panic branches in
callee extraction (`pyrefly §11.6`) — the sidecar supervises, restarts, and fences
generations (WP08). Known no-surface gaps the plan sizes as application work:
declared/expected types (TSP or sidecar extension — WP11 decision), narrowing
provenance (derived from the WP06 CFG), effective MRO (application C3 — WP12),
member types as display strings (`pyrefly §12.2`), callee-string→symbol
reconciliation (`pyrefly §11.7`), argument/overload binding (WP05/WP13). Pyrefly
bundles Ruff 0.0.6 against the root's 0.0.7 — two type universes; the process
boundary is the isolation mechanism and is never crossed with shared types.
**Validation:** `pyrefly_protocol_conformance` (WP08) and the WP11–WP13 behavioral
oracles against `GEN §93.1` fixtures.

### LD-PY-02 — Pyrefly pinned Glean-report adapter for modules and xrefs

**Decision:** adopt (bounded unstable surface)
**Version basis:** the same Pyrefly 1.2.0 revision
`1933169ad8ee9e4d4114112eb56ef0811fb0a094`; its public-but-`#[doc(hidden)]`
`library::library::library::library::FullCheckArgs` parser/run surface and the
`--report-glean` option. Add exact direct `clap = 4.6.2` only to bind the derived
`Parser` API already present in the locked Pyrefly graph.
**Displaces:** the v1 plan's impossible derivation of module graph, definitions,
imports, exports/re-exports, and bulk xrefs from type-table/callee responses. WP38
invokes the pinned check surface in-process inside the sidecar against an immutable
`ProviderWorkspaceView`; Pyrefly writes per-module Glean JSON only into the private
quotaed output root. The adapter decodes it immediately into application-owned
module/declaration/import/xref DTOs, validates source paths/ranges/digests and size
limits, and deletes the report files after terminal staging. No Glean or Pyrefly
report type crosses the process boundary. The long-lived `Query` of LD-PY-01 remains
the authority for type, callee, attribute, and subtype observations.
**Risk:** Pyrefly explicitly marks this library surface unstable, and the one-shot
checker does not share the long-lived `Query` state. It is therefore isolated in
one adapter module and compile/fixture-probed before WP10. Removal, option drift,
missing export semantics, or inability to fence the report to the requested source
snapshot is a mandatory plan revision; no source patch or subprocess fallback is
authorized by this version.
**Validation:** `just pyrefly-module-xref-surface-check` (WP38) compiles the exact
surface and proves packages, relative imports, stubs, re-export chains, star
imports, declarations, and cross-module xrefs at the pinned revision.

### LD-RS-01 — `rustc_public` on the dated nightly for compiler facts

**Decision:** adopt (deepen from summary extraction to full MIR)
**Version basis:** `nightly-2026-08-18` pinned by `rustc-extractor/rust-toolchain.toml`
(SUITE AC-G-07 toolchain bundle).
**Displaces:** the current shallow `OwnedMirItem` summary in
`rustc-extractor/src/rustc_link.rs`. The public surface proves out for: item
discovery (`mir §7`), types/generics/traits (`mir §16`), full MIR bodies —
locals, blocks, statements, terminators, operands, places, rvalues (`mir §8`–
`§15`), CFG with distinct normal/unwind/drop/assert edges (`mir §10`, `§12.3`),
and instance resolution (`Instance::resolve`/`resolve_closure`/
`resolve_drop_in_place`, `mir §20`).
**Risk:** no public dataflow framework exists — ownership/init state and def-use
are application derivations over the normalized access-event stream (`mir §26`,
`§29`–`§30`); public `PlaceContext` is too coarse for access classification, so
WP22 classifies from statements/rvalues/terminators directly (`mir §18.2`).
**Validation:** `rust_mir_body_fixture_conformance` (WP19) against `GEN §93.2`
fixtures; `just extractor-ci-fast`.

### LD-RS-02 — Narrow `rustc_private` enrichment adapter

**Decision:** adopt (bounded, Wave 11 only)
**Version basis:** the same dated nightly; adapter digest pinned in the toolchain
bundle (SUITE AC-G-07) and in provider runs.
**Displaces:** nothing in the stable root — the adapter lives only inside
`rustc-extractor/` behind owned DTOs. Scope is exactly the facts `GEN §97.2`
routes there: stable identity (`DefPathHash` + `StableCrateId`), SourceMap/hygiene
byte-exact spans and macro provenance, borrowck loans/regions where required
(`GEN §44.3`), and vtable layout (`GEN §42.2`); `run_with_tcx!` contained per
`mir App. P.7` so a nightly bump breaks one crate first.
**Risk:** nightly API drift. Mitigation: exhaustive variant tests in the extractor
domain, graceful capability degradation (conservative/absent capability status
instead of silent omission), and SUITE §83.6's six-way differential on any nightly
upgrade — never "latest nightly".
**Validation:** `rust_private_adapter_containment_parity` and
`rust_private_unavailable_degradation_falsification` (WP24).

### LD-TS-01 — Tree-sitter as syntax anchor for semantic correspondence

**Decision:** retain-current
**Version basis:** `tree-sitter = 0.26.12`, `tree-sitter-python = 0.25.0`, and
`tree-sitter-rust = 0.24.2` exactly as resolved by the stable root and proved by
`just stable-graph-check`; generated raw-kind catalogs carry their grammar
fingerprints.
**Displaces:** nothing. Occurrence identity stays `(file_id, content_digest, byte
range, kind) + ordinal` (`ts §11.7`); Python semantic facts join Ruff/Pyrefly
output on byte ranges under one content digest (`pyrefly §24`); Rust facts accept
1:N/N:1 source↔MIR correspondence (`mir §47.1`–`§47.2`); during compile failure
Tree-sitter keeps structure and dirty-owner identification alive (`mir §47.3`).
**Risk:** none new.
**Validation:** existing Wave 4–6 gates plus each lane's range-reconciliation
behavioral oracles.

### LD-SB-01 — Fail-closed provider containment on Darwin and Linux

**Decision:** adopt (host-specific production mechanism)
**Version basis:** Darwin uses the OS-owned `/usr/bin/sandbox-exec -f` Seatbelt
surface after code-signature/path and behavioral capability probes; the profile is
an application-owned generated artifact whose canonical bytes and SHA-256 digest
pin every run. Linux uses non-setuid bubblewrap 0.11.2 exactly, provisioned by the
deployment image and verified by absolute path, version, owner/mode, and capability
probe. The Linux policy uses user, mount, PID, network, IPC, UTS, and cgroup
namespaces; read-only source/dependency binds; a private size-bounded tmpfs output;
`PR_SET_NO_NEW_PRIVS`; capability drop; a generated architecture-specific seccomp
BPF passed on an inherited descriptor; disabled nested user namespaces; a new
session; and die-with-parent containment. Mount-namespace path restriction is the
approved `GEN AC-G-35` equivalent to Landlock for this profile.
**Displaces:** vague "Seatbelt/namespaces" intent and all unsandboxed execution
under `UNTRUSTED_SANDBOXED`. A small application-owned launcher in the stable root
sets registry-selected CPU/address-space/file/process limits through the existing
`rustix` dependency with its `process` feature, closes non-protocol descriptors,
then `exec`s the lane binary. ProviderRuntime owns the process group, cancellation,
deadline, output quota, and cleanup ledger.
**Host capability contract:** a Darwin host passes only when the exact Seatbelt
profile denies network, live-workspace/credential reads, out-of-root writes, child
escapes, excess descriptors, and residual processes. A Linux host passes only when
the exact bubblewrap/seccomp profile proves the corresponding namespace, syscall,
path, resource, cancellation, and cleanup tests. If the mechanism or any probe is
unavailable, `UNTRUSTED_SANDBOXED` is `UNAVAILABLE_PROVIDER` for that host and the
Python/Rust semantic profile cannot be certified `COMPLETE` there. `TRUSTED_LOCAL`
requires an explicit workspace grant and is always reported as a distinct, weaker
capability; it never satisfies an untrusted production milestone.
**Risk:** `sandbox-exec` is deprecated and bubblewrap policy safety is argument-
dependent. Both surfaces are therefore behind one owned launcher contract and a
host matrix; disappearance or a bypassable profile is a plan revision, not a
silent downgrade.
**Validation:** `just semantic-sandbox-host-matrix-check` plus the lane-specific
escape/cancellation oracles in WP08 and WP16.

### 2.2 Design-principle posture

Principle identifiers are namespaced throughout this plan: `DF-P*` means
`full_data_fabric_design_principles.md`; `H-P*` means
`semantic_design_principles_holistic.md`. Bare `P*` identifiers are forbidden in
plan artifacts and generated traceability. The load-bearing data-fabric bindings,
via the two alignment manuals (`align §23`–`§24`, `delta-align §34`–`§35`), are:

- **DF-P1/DF-P2 model-first, executable models** — fact families, observation schemas,
  capability records, and the derivation registry are typed contract models
  compiled into schemas, encoders, and validators; no consumer re-encodes them.
- **DF-P3 one authoritative owner** — provider observations are never canonical; the
  `ReconciliationEngine` is the sole canonicalization authority (GI-11); the
  derivation registry names one owner per derived family (GI-14).
- **DF-P7/DF-P8 canonical fabric representations** — Arrow `RecordBatch` streams at every
  provider boundary; no pairwise DTO universes.
- **DF-P9/DF-P10/DF-P11 provenance, closure, immutable snapshots** — every observation
  carries run/generation/digest identity; every canonical row carries evidence;
  publication stays manifest-pinned MVCC.
- **DF-P12 schemas as executable contracts** — every new table passes the
  Arrow→Delta→provider round-trip gate; cardinality integrity is generated from
  the property registry (WP30).
- **DF-P14/DF-P15 highest-level extension, optimizer visibility** — built-in DataFusion
  plans for reconciliation; no opaque UDFs wrapping transparent predicates.
- **DF-P18/DF-P19 fingerprints and reproducibility** — context manifests, owner content
  digests, adapter digests, coverage fingerprints; incremental output equals clean
  rebuild (AC-G-79) in both language lanes.
- **DF-P20/DF-P21 conservative capability truth** — `PARTIAL`/`COMPLETE` only through the
  formal aggregation algebra; unknown is preferable to falsely known (GI-06).
- **DF-P23 explicit state ownership** — provider caches (last-known-good, safe-reuse)
  are declared operational caches, never present-state authority.
- **DF-P25 tests derive from contracts** — every packet's oracles instantiate the
  contract it implements; profile conformance is revalidated at M05.

The holistic doctrine adds the following cross-cutting bindings:

- **H-P1/H-P5/H-P6/H-P8 boundary discipline** — providers, Glean reports,
  sandbox helpers, storage, and query are adapters behind application-owned ports;
  dependency direction points inward and trust is least-privilege.
- **H-P10/H-P12/H-P13 declarative authority and identity** — registries are the
  single source for properties, derivations, capabilities, observations, and pass
  contracts; provider-local and response-local indices cannot become durable IDs.
- **H-P14/H-P15/H-P16/H-P17 transformation discipline** — staged provider,
  normalization, reconciliation, derivation, and publication passes have explicit
  contracts, canonicalize before optimization, and keep deterministic core logic
  separate from effectful launch/commit shells.
- **H-P22/H-P23/H-P25 state, failure, and reproducibility** — runtime resources
  have one lifecycle owner; every gap/failure is typed; equal immutable inputs and
  fingerprints produce equal clean/incremental output.
- **H-P27/H-P28/H-P29/H-P30/H-P31 governance** — provenance and observability are
  distinct structured contracts; stable interfaces are versioned; tests and
  executable governance derive from those contracts.

Every load-bearing transformation added or materially changed by a packet must
register a pass contract in
`contracts/registry/transformation-pass-registry.yaml` before its implementation
can pass `just design-principle-traceability-check`. Each record names the pass ID,
owner packet, input artifact/schema and preconditions, output artifact/schema and
postconditions, entry/exit invariants, preserved identities, generated identities,
invalidation closure, diagnostics/failure taxonomy, determinism/fingerprint inputs,
resource/telemetry contract, and behavioral/negative/incremental fixtures. At
minimum this plan registers contracts for Ruff semantic traversal, Python and Rust
CFG/dataflow, canonical type interning, Glean report decoding, provider
normalization, canonical reconciliation, and every materialized derivation.
Generated traceability must map each contract to `H-P14`/`H-P16` and its packet
oracles; code without a record or a stale digest fails the model gate.

Principles not named by a packet are `N/A` for that packet; the doctrine tables in
`docs/spec_index/invariants-and-doctrine.md` remain navigation evidence, never
normative authority.

## 3. Global target invariants

GI-01 through GI-10 are the suite-wide invariants of `RM §1`, restated here as the
plan's binding set; GI-11 through GI-15 are the cross-cutting invariants this
plan's packets prove repeatedly.

- **GI-01** `workspace_id` identifies exactly one authorized analyzed source
  instance.
- **GI-02** One immutable leased `ServingSnapshot` is the only query pin.
- **GI-03** Current stable filesystem bytes are present-state authority — never
  watcher events, Git objects, or prior provider output.
- **GI-04** Provider observations are not canonical facts until reconciled.
- **GI-05** Context-sensitive facts never cross analysis-context boundaries.
- **GI-06** Unknown remainder is explicit; missing data does not prove absence.
- **GI-07** The Rust daemon owns semantic interpretation, planning, execution,
  snapshots, and canonical result bytes.
- **GI-08** The Python FastMCP process remains a thin adapter.
- **GI-09** Every compatibility-sensitive artifact is versioned and fingerprinted.
- **GI-10** Incremental results converge to the clean-rebuild result for identical
  inputs.
- **GI-11** Generation ends at validated observation streams plus capability
  manifests; only the `ReconciliationEngine` writes canonical rows (`GEN §86`,
  `FAB §72`).
- **GI-12** Owner-capability, completeness, provider-run, certainty, resolution,
  and directness vocabularies are single generated registries, orthogonal and
  never collapsed into one score (`GEN §85`, `ONT §62`).
- **GI-13** Provider execution is sandboxed fail-closed: when containment cannot be
  established, semantic execution is unavailable, never silently unsandboxed
  (`GEN AC-G-35`).
- **GI-14** Every derived family has exactly one registered implementation and
  precision profile per snapshot (`GEN §87`, `FAB §79A`, `AC-G-42`).
- **GI-15** Every new state/process/write/pointer/artifact/long-running boundary
  has a deterministic fault point and bounded structured phase, count, queue,
  memory/spill/cache, cancellation, and failure telemetry; every wave records a
  reproducible performance baseline (`RM §27.3`, `§27.5`, `§28`; H-P28).

## Audit Integration Log

Audit: `docs/reviews/plan_audit_codefabric_waves_8-12_semantic_profiles_2026-08-26_v1.md`
(`plan-audit` v1, verdict `needs-revision`). Source design: `RM` v1.0 plus the
unchanged synchronized 1.3 domain suite. Source plan: v1. Revised design: none;
the audit required faithful plan/library/sequence closure, not a new normative
target. Revised plan: this v2. Revision reason: integrate all 12 audit findings,
including both blockers, without mutating v1 or execution state.

The revalidation results below are evidence at integration time. Commands that
name recipes created by this plan correctly remain non-zero before execution;
their absence is recorded rather than represented as implementation closure.

- `F-001` — `applied-plan`
  - Finding: reaching-definition/def-use publication preceded its mandatory
    single derivation authority.
  - Resolution: WP07 now registers and runtime-selects
    `PY_OWNER_REACHING_DEFS_V1` before staging; WP35 audits/completes the matrix
    without retroactive authority; M01 requires the registry/property gates.
  - Revalidation: `just model-repro-check && just packet-oracle-check WP07 && just
    wave8-integration-check` — exit 1. Model reproduction passed; the inactive
    successor's WP07 oracles/recipe are not implemented and the active
    predecessor selector lacks its definitions.
  - Rationale: the first publication and sole implementation selection now share
    one packet and milestone boundary.
- `F-002` — `added-packet`
  - Finding: WP10 required module/export/xref evidence unavailable from the
    selected long-lived Query methods.
  - Resolution: LD-PY-02 and new WP38 bind the exact pinned public-hidden
    `FullCheckArgs --report-glean` surface, owned DTO/protocol boundary, fixture
    matrix, and stop-before-WP10 replan trigger; WP10 depends on WP38.
  - Revalidation: `just pyrefly-module-xref-surface-check && just
    packet-oracle-check WP10` — exit 1 because WP38 creates the recipe. Additional
    bounded compile probe at revision
    `1933169ad8ee9e4d4114112eb56ef0811fb0a094` with `clap = 4.6.2` — exit 0,
    proving `FullCheckArgs` parsing of `--report-glean` and public async `run`
    binding; behavioral module/xref proof remains WP38's gate.
  - Rationale: a versioned observation surface now exists before any consumer;
    unrelated type/callee responses are no longer treated as derivation inputs.
- `F-003` — `applied-plan`
  - Finding: the holistic doctrine, principle namespaces, and pass contracts were
    absent.
  - Resolution: declared inputs now include the exact holistic-doctrine digest;
    §2.2 uses `DF-P*`/`H-P*`, defines the pass-contract schema, and WP01 owns its
    generated traceability/enforcement.
  - Revalidation: `rg -q 'semantic_design_principles_holistic\.md'
    docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md
    && just design-principle-traceability-check` — exit 1. The text check passed;
    the current active-plan traceability gate fails on predecessor packet/dirty-
    baseline state and does not yet implement the v2 holistic/pass extension.
  - Rationale: each load-bearing transformation now has an owned, executable
    contract and unambiguous doctrine identity.
- `F-004` — `applied-plan`
  - Finding: language-neutral context/type authority was incorrectly owned by
    Python packets.
  - Resolution: WP01 owns the shared discovery ports, common type tables, and the
    existing `TypeInterner`; WP15 depends on WP01, WP18 depends on WP17, and
    WP11/WP18 independently populate the shared authority.
  - Revalidation: `! rg -n 'WP15 requires WP0&#50;|WP18 requires WP1&#49;|new
    type-interning modu&#108;e'
    docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md
    && just plan-dependency-check` — exit 0. The recipe checked the active
    predecessor; direct v2 artifact/DAG validation separately passed for all 38
    packets.
  - Rationale: language lanes no longer depend on arrival order for shared
    semantic authority.
- `F-005` — `applied-plan`
  - Finding: integrated-plan authorization and unordered write-set disposition
    were not closed.
  - Resolution: the authorization principal/date/source is recorded above; WP01
    creates lane-owned composable fragments, WP36 freezes shared runtime
    consumers, and explicit edges serialize every within-lane shared seam.
  - Revalidation: `rg -q '^Integrated-plan authorization:'
    docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md
    && just plan-dependency-check` — exit 0 (active predecessor). Direct target
    analysis — exit 0: 38-packet acyclic graph and zero unordered literal-path
    overlaps in the revised Known Touch sets.
  - Rationale: accountability is explicit and no plan packet relies on informal
    coordination for a known shared write.
- `F-006` — `applied-plan`
  - Finding: sandbox mechanisms and host capability truth were not decision-
    closed.
  - Resolution: LD-SB-01 fixes the Darwin Seatbelt and Linux non-setuid
    bubblewrap 0.11.2/seccomp profiles, resource launcher, profile digests,
    deployment probes, escape matrix, and unavailable/`TRUSTED_LOCAL` semantics;
    WP36/WP08/WP16 own implementation and proof.
  - Revalidation: `just semantic-sandbox-host-matrix-check && just
    packet-oracle-check WP08 && just packet-oracle-check WP16` — exit 1; the new
    recipe is an explicit WP36 implementation output.
  - Rationale: a host can advertise `UNTRUSTED_SANDBOXED` only after proving the
    exact platform contract; no trusted fixture can certify it.
- `F-007` — `applied-plan`
  - Finding: roadmap fault, observability, and mandatory performance evidence was
    unallocated.
  - Resolution: GI-15 and the §4 group rule allocate generated fault/telemetry
    contracts and per-wave baselines; WP36 creates three aggregate recipes and
    every milestone/final gate invokes them.
  - Revalidation: `just semantic-fault-point-check && just
    semantic-observability-contract-check && just semantic-profile-bench` — exit
    1; WP36 creates the first recipe and contracts.
  - Rationale: operational proof is now an owner/milestone obligation rather than
    a regression-triggered suggestion.
- `F-008` — `applied-plan`
  - Finding: WP03 omitted exact Ruff graph/lock/checker impact and left helpers
    unnamed.
  - Resolution: LD-RF-01 authorizes only `ruff_python_semantic = 0.0.7`; WP03
    starts with a compile probe, names `Cargo.lock` and
    `scripts/stable_graph_check.sh`, updates exact census/feature assertions, and
    requires a plan revision for any additional direct crate.
  - Revalidation: `RUSTC_WRAPPER= just stable-graph-check && just features-each &&
    just root-check` — exit 0 on the current pre-WP03 graph (warnings only). WP03
    must rerun it after the declared manifest change.
  - Rationale: the new dependency cannot land without exact default/narrow graph
    proof and has no open-ended helper branch.
- `F-009` — `applied-plan`
  - Finding: property authority could be regularized after four milestones.
  - Resolution: WP01 introduces the first-publication/model guard, every table
    packet and milestone invokes it, and WP30 hard-fails and returns any missing
    record to its owning packet instead of repairing it.
  - Revalidation: `just property-registry-closure-check && just
    model-repro-check && just wave8-integration-check` — exit 1; WP01 creates the
    first recipe.
  - Rationale: property authority must exist at landing and cannot be legitimized
    retroactively.
- `F-010` — `applied-plan`
  - Finding: legacy zero state was incomplete and prose-only, omitting a direct
    serving consumer.
  - Resolution: §6 defines one governed candidate/allow registry and combined
    `rg`/`ast-grep`/build recipe for DB01–DB03; WP36 cuts over
    `src/fabric/serving.rs`; lane packets and every applicable milestone invoke
    the scoped/default command.
  - Revalidation: `just semantic-provider-legacy-zero-state-check && just
    root-check && just sidecar-check && just extractor-check` — exit 1; WP01
    creates the first recipe.
  - Rationale: candidate scope, exceptions, expiry, and clean-domain proof are
    machine-governed and complete across all three legacy classes.
- `F-011` — `applied-plan`
  - Finding: two table preflights and the final packet selector were not
    executable.
  - Resolution: WP03/WP19 use `.tables[].name`; WP35 creates
    `semantic-profile-packets-check` over all 38 exact selectors; generic
    placeholders are removed.
  - Revalidation: `! rg -n "jq '\.tables &#124; keys'|packet-oracle-check &lt;"
    docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_*.md`
    — exit 0.
  - Rationale: the preflights inspect names and the final gate is a valid,
    completeness-checked recipe.
- `F-012` — `applied-plan`
  - Finding: Pyrefly and Tree-sitter decisions omitted exact identities.
  - Resolution: LD-PY-01/02 record Pyrefly 1.2.0 revision
    `1933169ad8ee9e4d4114112eb56ef0811fb0a094` and feature boundary; LD-TS-01
    records tree-sitter 0.26.12, Python grammar 0.25.0, and Rust grammar 0.24.2.
  - Revalidation: `RUSTC_WRAPPER= cargo metadata --manifest-path
    pyrefly-sidecar/Cargo.toml --locked --format-version 1 >/dev/null &&
    RUSTC_WRAPPER= just stable-graph-check` — exit 0.
  - Rationale: every unstable/provider identity is exact and has a live graph
    gate.

## 4. Work packets

Packets are grouped by wave. A packet is complete only when its four oracles pass
at its proving commit and at HEAD (`just packet-oracle-check WPnn`); oracle names
are globally unique test identifiers to be defined in the owning domain's test
tree. `RM §2.1` sizing holds per wave group (six to eight packets each, plus the
two cross-cutting substrate packets WP01/WP36 and the Pyrefly capability packet
WP38). WP36–WP38 were added during plan development/audit; packet IDs are stable
labels — the §8 dependency graph, not ID order, governs sequence. The two language
lanes execute independently after WP01's lane-neutral context/type/table authority
and both provider integrations consume WP36. No Python packet is a prerequisite
merely because it happens to populate a common table first. Wave 12
(WP29–WP35, WP37) is the integration barrier.

Four group-wide rules apply:

1. Every packet that introduces a table registers its
property records in `contracts/registry/ontology-property-registry.yaml` at
landing. `just property-registry-closure-check` is part of every table packet and
every milestone from M01 onward; missing registration fails publication and can
never be repaired by WP30.
2. Every load-bearing transformation registers the §2.2 pass contract before code
lands; the contract's failure, determinism, invalidation, telemetry, and fixtures
are acceptance inputs, not retrospective documentation.
3. Every packet that adds a GI-15 boundary registers its fault points and telemetry
in the generated registries and makes at least one negative oracle assert both the
behavior and emitted fault/phase/terminal records. Each milestone runs
`semantic-fault-point-check`, `semantic-observability-contract-check`, and a
bounded wave workload through `semantic-profile-bench`.
4. Each `waveN-integration-check` recipe's oracle selection excludes the closure
packet's own operational meta-oracle, so no wave gate invokes itself.

### Wave 8 group — Python local semantic substrate (`RM §13`)

### WP01 — Successor intake, semantic-lane activation, and observation-schema regularization

**Outcome.** The inherited remediation surface is revalidated at the activation
commit, and every provider observation schema — including the Pyrefly family that
today bypasses the pipeline — is generated from
`contracts/schema/schema-contract-ir.json` with the same authority as the rustc
family. Provider-runtime integration and lane scheduling are WP36's outcome; this
packet establishes the lane-neutral contract substrate they build on: shared
analysis-context discovery ports, the already-existing canonical type algebra and
`TypeInterner`, common type tables, property/derivation first-publication guards,
and transformation-pass traceability.

**Dependencies.** None.

**Target invariants.** GI-03, GI-04, GI-09, GI-11, GI-12, GI-14, GI-15;
DF-P1–DF-P3, DF-P7, DF-P9; H-P10, H-P13, H-P14, H-P16.

**Design and library references.** `RM §0` successor activation rules; `RM §13`
entry dependencies; `GEN AC-G-32` common asynchronous provider execution
interface; `GEN §86` generation output boundary; `GEN §90` provider job
interfaces; `LIFE §95`–`§96` lane scheduling; `FAB §63` observation-stream
contract; `SUITE AC-G-05` contracts tree; LD-AR-01.

**Change surface / Preflight / Known Touch.** Run:

```bash
just plan-status && just artifacts-check
rg -n 'semantic-work-required|semantic_capabilities_required|semantic_lane_required' src/
rg -n 'ProviderAdapter|ProviderJobFactory|register' src/provider_runtime.rs
rg -n 'observation_family_code|OBSERVATION_FAMILY_CODE|pyrefly-module' src pyrefly-sidecar contracts -g '!contracts/generated/**'
rg -n 'TypeConstructor|TypeTerm|TypeInterner' src/identity.rs
jq '.provider_observation_schemas' contracts/schema/schema-contract-ir.json
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(86|90)\.'
```

Known current touch includes `src/continuous.rs` (the `Ok(None)`
semantic-work-required return), `src/provider_runtime.rs`, `src/core_facts.rs`,
`contracts/schema/schema-contract-ir.json`, the hand-authored
`contracts/schema/provider-observations/pyrefly-module-v1.json`,
`pyrefly-sidecar/src/pyrefly_link.rs` (embedded schema text), the model compiler's
schema driver, `src/identity.rs`, `src/analysis_context.rs`, the new
`transformation-pass-registry.yaml`, and the generated registries. Exact symbols are re-derived at
preflight — the remediation plan is still landing on these files.

**Required changes.**

1. Revalidate the inherited surface: run the wave-acceptance and Gate B gates at
   the activation commit and record any pre-existing failure as baseline, not as
   this plan's debt.
2. Move the Pyrefly observation family into the schema Contract IR as a governed
   `provider_observation_schemas` entry; regenerate `PROVIDER_OBSERVATION_SCHEMAS`
   and the provider registry so family 110 has the same generated authority as the
   rustc family 120; the sidecar embeds the generated schema, not an
   `include_str!` of a hand-authored file.
3. Add the wave-group gate recipe scaffold `just wave8-integration-check` (empty
   selection initially, populated by WP02–WP07) so every Wave 8 packet cites a
   registered gate rather than raw flags.
4. Establish the lane-neutral analysis-context discovery port and shared semantic
   tables before either language lane: define `type_detail`/`type_fact_detail` in
   the Contract IR, reuse and extend the current `TypeConstructor`/`TypeTerm`/
   `TypeInterner` in `src/identity.rs` as the only canonical type authority, and
   expose language adapters as consumers. WP02/WP15 discover lane-specific
   manifests; WP11/WP18 populate the common tables independently.
5. Add the governed `transformation-pass-registry.yaml` and generated
   principle/pass/oracle traceability described in §2.2. Extend
   `just design-principle-traceability-check` to reject bare principle IDs,
   missing pass contracts, and stale contract digests.
6. Add `just property-registry-closure-check` as a model/publication guard from
   the first semantic table and scaffold
   `just semantic-provider-legacy-zero-state-check` with reviewed candidate and
   allow sets. The latter is diagnostic until DB01–DB03 close, but its definition
   is governed and cannot be replaced with packet-specific prose scans.
7. Before lane packets may run concurrently, replace monolithic semantic
   extension edits with deterministically composed lane-owned model, registry,
   ingest, context, and invalidation fragments. The compiler sorts and validates
   those fragments into frozen shared projections consumed by
   `src/core_facts.rs`, `src/analysis_context.rs`, `src/operational_store.rs`, and
   the lifecycle. Later lane packets add only their fragment and adapter; editing
   a frozen shared file requires a new dependency or integration packet.

**Legacy Disposition and Decommission.** The hand-authored observation descriptor
and the `include_str!` bypass are defects, not compatibility surfaces; they are
cut over here with no alias (DB02 owns final zero-state). The hard-coded Gate B
provider spawning survives temporarily; WP36 builds the production path and DB01
retires direct spawning after both lanes' provider packets land.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `observation_schema_generation_conformance`; Executable oracle: `provider_observation_schema_projection_parity`; Executable oracle: `handwritten_observation_schema_falsification`; Executable oracle: `successor_intake_operational_gate`.

- **Behavioral — Executable oracle:**
  `observation_schema_generation_conformance` — the regenerated observation
  schemas decode real sidecar and extractor output streams produced at the
  activation commit; family codes 110 and 120 resolve identically through the
  generated registry.
- **Structural — Executable oracle:**
  `provider_observation_schema_projection_parity` — both observation families
  resolve from the generated registry; the sidecar and extractor embedded schemas
  byte-equal the generated projections.
- **Negative/Zero-State — Executable oracle:**
  `handwritten_observation_schema_falsification` — no observation schema exists
  outside the Contract IR pipeline; a drift fixture (edited generated schema)
  fails model compilation.
- **Operational — Executable oracle:** `successor_intake_operational_gate` — the
  inherited wave-acceptance and Gate B surfaces re-run green (or with recorded
  baseline failures) at the activation commit, and the `wave8-integration-check`
  scaffold, property-closure guard, pass traceability gate, and governed legacy
  zero-state recipe resolve as executable surfaces.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted
`provider_runtime`/`continuous` unit tests.

**Packet-Local Gates.** `just model-repro-check`, `just root-ci-fast`,
`just sidecar-ci-fast`, `just governance-scan`, `just packet-oracle-check WP01`.

**Integration Milestone.** M01.

**Replan Triggers.** The remediation plan's final surface diverges so far from the
planned substrate that provider-runtime integration needs a different boundary; or
regenerating the observation-schema registry would break the released Gate B
corpus contract, requiring an accountable corpus decision first.

**Rollback or Recovery.** Pre-activation this packet is inert. After cutover, roll
forward by regenerating from the Contract IR; the hand-authored descriptor is
never restored.

### WP36 — Provider execution and sandbox substrate

**Outcome.** Semantic providers are production citizens of the runtime: the
Pyrefly sidecar and rustc extractor have `ProviderAdapter` registrations
executing under `ProviderRuntime` (admission, supersession keys, journaling,
credit control, the `GEN AC-G-32` wire-to-application event mapping); the
continuous engine schedules semantic lane work instead of parking at
`semantic-work-required`; and the lane-neutral halves of the `GEN AC-G-33`
transport (immutable `ProviderWorkspaceView` construction, `DependencyInputBundle`
pinning) and the `GEN AC-G-35` sandbox substrate (trust profiles, containment
probes, sandbox-profile digests, fail-closed reporting) exist once, for both
lanes to consume, together with the GI-15 fault/telemetry/resource substrate used
by every later provider packet. (This packet was split out of WP01 by the plan challenge; its
ID is late, its position is early.)

**Dependencies.** WP01.

**Target invariants.** GI-03, GI-04, GI-11, GI-12, GI-13, GI-15; DF-P3, DF-P9,
DF-P23; H-P8, H-P22, H-P23, H-P28.

**Design and library references.** `GEN AC-G-32`, `AC-G-33`, `AC-G-35`;
`GEN §90` provider job interfaces; `LIFE §95`–`§96` lane scheduling; the
existing `ProviderRuntime`/`ProviderAdapter`/`OperationalProviderRunJournal`
machinery in `src/provider_runtime.rs` (seed).
LD-SB-01 supplies the exact platform mechanism and host capability contract.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'semantic-work-required|semantic_lane_required' src/continuous.rs src/lifecycle.rs
rg -n 'ProviderAdapter|ProviderJob|map_wire_event' src/provider_runtime.rs
rg -n 'ProviderWorkspaceView|DependencyInputBundle|sandbox_profile' src contracts -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^AC-G-(32|33|35)'
```

Known current touch includes `src/provider_runtime.rs`, `src/continuous.rs`
(the `Ok(None)` semantic-work-required return), `src/lifecycle.rs` (lane
scheduling hooks), `src/source_image.rs` (view/bundle construction),
`src/operational_store.rs` (run journaling), the owned sandbox launcher/profile
artifacts, `Cargo.toml`/`Cargo.lock` (existing `rustix` process feature), CI host
provisioning, and the provider, fault-point, telemetry, and resource-profile
registries.

**Required changes.**

1. Implement `ProviderAdapter` registrations for the Pyrefly sidecar and rustc
   extractor over `ProviderRuntime`: accepted-handle jobs, supersession key
   `(workspace, context, provider, scope, capability family)`, registry-resolved
   resource profiles, journaled terminal states, and the event-mapping table in
   `contracts/rpc/feature-registry.yaml`.
2. Replace the continuous engine's semantic dead-end with lane scheduling: when
   a wave's invalidation plan names semantic capabilities, the engine schedules
   provider runs and resumes the wave on their terminal events, honoring
   generation fences (`LIFE §95`–`§96`).
3. Build the lane-neutral `ProviderWorkspaceView` core (manifest of
   WorkspacePath → blob digest/mode, atomic publication after verification, no
   writable link to the live workspace or `.git`, separate writable output
   root) and `DependencyInputBundle` pinning in `src/source_image.rs`; WP09 and
   WP16 instantiate them per lane.
4. Implement LD-SB-01 exactly: trust-profile model
   (`UNTRUSTED_SANDBOXED`/`TRUSTED_LOCAL`/`PARSING_ONLY`), platform containment
   probes, generated Darwin Seatbelt and Linux bubblewrap/seccomp profiles,
   application-owned resource-limit/descriptor launcher, sandbox-profile digests
   pinned into runs and snapshots, and the fail-closed host capability matrix.
   Lane-specific escape enforcement lands in WP08/WP16; a host that cannot prove
   containment never advertises `UNTRUSTED_SANDBOXED`.
5. Convert current direct provider consumers, including
   `src/fabric/serving.rs` and the Gate B vertical, to the registry-selected
   `ProviderRuntime` dispatch port once; lane packets only register and verify
   their adapter and never edit the shared consumer again.
6. Register deterministic fault points at provider admission, child launch,
   handshake, stage creation, chunk write/accept/reject, terminal verification,
   cancellation, kill, cleanup, and journal transition. Generate bounded
   phase/counter/queue/memory/output/cache/cancellation/failure telemetry with
   declared units, lifecycle, and cardinality; add
   `semantic-fault-point-check` and `semantic-observability-contract-check`.
7. Add `semantic-profile-bench` with reproducible small/medium fixture workloads,
   machine/context fingerprints, warm/cold distinction, and non-normative baseline
   storage. Every wave supplies its own provider/stream/derivation/reconciliation/
   spill/query scenario; a baseline is mandatory even when no regression is
   suspected.

**Legacy Disposition and Decommission.** The Gate B vertical harness becomes a
consumer of the production path as each lane's packet lands; DB01 owns the
direct-spawn zero state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `semantic_lane_scheduling_conformance`; Executable oracle: `provider_adapter_registration_parity`; Executable oracle: `sandbox_unavailable_fail_closed_falsification`; Executable oracle: `semantic_lane_operational_journal_gate`.

- **Behavioral — Executable oracle:** `semantic_lane_scheduling_conformance` — a
  workspace with semantic capabilities required processes a source batch through
  scheduled provider runs to a published wave, with generation fences discarding
  a superseded run.
- **Structural — Executable oracle:** `provider_adapter_registration_parity` —
  both providers resolve from the provider registry to registered adapters; the
  wire-event mapping table covers every event kind; no invocation path outside
  `ProviderRuntime` exists in the stable root's production modules.
- **Negative/Zero-State — Executable oracle:**
  `sandbox_unavailable_fail_closed_falsification` — with the containment probe
  forced unavailable, semantic provider execution reports unavailable with the
  registered reason and terminal telemetry; no code path launches a provider
  unsandboxed under the untrusted profile.
- **Operational — Executable oracle:** `semantic_lane_operational_journal_gate`
  — provider-run journal rows exist for scheduled semantic runs with admission,
  supersession, and terminal states; crash-mid-run recovery leaves no orphaned
  accepted run.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted
`provider_runtime`/`continuous` unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just provider-protocol-check`,
`just governance-scan`, `just semantic-fault-point-check`,
`just semantic-observability-contract-check`, `just semantic-profile-bench`,
`just packet-oracle-check WP36`.

**Integration Milestone.** M01.

**Replan Triggers.** Provider-runtime integration needs a different boundary
than the accepted-handle model provides (would touch the AC-G-32 contract); the
exact LD-SB-01 mechanism is unavailable or fails an escape probe on an advertised
host; or the lane-neutral view/bundle split proves impossible without lane-specific
knowledge (collapse back into the lane packets as an implementation
adaptation, recorded).

**Rollback or Recovery.** Adapter registrations are capability-gated;
unregistering restores the pre-substrate posture without data migration.

### WP02 — Python analysis-context discovery and identity

**Outcome.** Python analysis contexts are immutable, explicitly discovered
manifests with deterministic identity: configuration discovery walks the
`GEN AC-G-34` precedence chain, emits a complete `ConfigurationDependencySet`,
and refuses to guess; `analysis_context_id`/`context_set_id` are CBEF-derived;
context-affecting changes invalidate exactly the dependent semantic families
while preserving source/syntax facts.

**Dependencies.** WP01.

**Target invariants.** GI-05, GI-09; DF-P1, DF-P3, DF-P18.

**Design and library references.** `GEN AC-G-14` (Python manifest fields,
language-version precedence, defaulting diagnostics); `GEN AC-G-34` (config-source
precedence, lock-system selection, no live-venv inference); `LIFE §7.5`
(import-root/config change invalidation); `ONT §64.2`/`§64.4` context separation;
existing `src/analysis_context.rs` fingerprinting.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'AnalysisContextKind|fingerprint_bytes|context_manifest' src/analysis_context.rs src/continuous.rs src/snapshot.rs
rg -n 'pyproject|uv.lock|poetry.lock|requires-python|typeshed' src/ contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^AC-G-(14|34)'
```

Known current touch is a Python-owned discovery adapter, context/diagnostic model
fragments, and their tests. It consumes the frozen WP01 context, invalidation,
and operational-writer ports; it does not edit their shared implementation files.

**Required changes.**

1. Implement Python context discovery: project roots, module/source/stub/
   dependency roots, PEP 420 namespace policy, ordered import precedence,
   language-version precedence (workspace profile → type-checker config → unique
   `requires-python` → deployment default with `CONTEXT_DEFAULTED` diagnostic),
   typeshed bundle digest, lockfile and tool-config digests.
2. Emit the `ConfigurationDependencySet` (file IDs, digests, reasons); multiple
   lock systems without a workspace-profile selection are a terminal context
   diagnostic, never a merge.
3. Extend the context manifest model so every compatibility-sensitive field enters
   the CBEF preimage; a changed field mints a new `analysis_context_id` and drives
   the `LIFE §7.5` invalidation closure (module resolution, cross-module refs,
   types, call targets) while source/syntax stays current.
4. Designate exactly one default Python context per snapshot
   (`SnapshotContexts.default_python_context_id` already exists — wire discovery
   to it).

**Legacy Disposition and Decommission.** None — the context model exists; this
packet fills the Python discovery half. No alias or dual identity: a manifest
computed by discovery replaces any hand-registered Python context in fixtures.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_context_discovery_conformance`; Executable oracle: `py_context_manifest_identity_parity`; Executable oracle: `py_context_guess_rejection_falsification`; Executable oracle: `py_context_invalidation_operational_gate`.

- **Behavioral — Executable oracle:** `py_context_discovery_conformance` — fixture
  projects (pyproject-only, pyrefly.toml, multi-lock, namespace packages) yield
  the exact expected manifests and `ConfigurationDependencySet`s.
- **Structural — Executable oracle:** `py_context_manifest_identity_parity` — every
  compatibility-sensitive manifest field is in the CBEF preimage; equal manifests
  hash equal across process restarts; display labels do not affect identity.
- **Negative/Zero-State — Executable oracle:**
  `py_context_guess_rejection_falsification` — conflicting lock systems and
  unknown configuration produce terminal context diagnostics, never a guessed or
  merged context; no code path infers semantics from the launching interpreter.
- **Operational — Executable oracle:** `py_context_invalidation_operational_gate`
  — a config-file edit invalidates exactly the dependent semantic families,
  preserves source/syntax currency, and republishes under the new context ID.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted
`analysis_context` unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check` (registry
additions), `just packet-oracle-check WP02`.

**Integration Milestone.** M01.

**Replan Triggers.** A required manifest input cannot be discovered without
executing user code (would violate `GEN AC-G-14`'s no-execution rule) — return
the gap to the owning spec rather than weakening the rule.

**Rollback or Recovery.** Context records are additive; recovery is re-running
discovery. No durable state migration.

### WP03 — Ruff semantic adapter: traversal, scopes, bindings, references

**Outcome.** A version-pinned adapter traversal populates Ruff's `SemanticModel`
per the checker contract and emits the application-owned `PythonFrontendBatch`:
all seven `ONT §33` scope kinds, all `ONT §34` binding kinds across the fifteen
`GEN §18.2` target forms, references classified per `GEN §18.3`, and the
shadow/rebind/capture edge set — persisted through new `scope_detail`,
`binding_detail`, and `reference_detail` Delta tables with FAB-conformant
encoders. Unresolved references materialize `UNKNOWN_SYMBOL` targets, never
absent edges.

**Dependencies.** WP02, WP36. This serializes the two root-manifest edits before
the Ruff dependency change while retaining lane-neutral ownership.

**Target invariants.** GI-04, GI-05, GI-06, GI-11, GI-15; DF-P1, DF-P3, DF-P7,
DF-P12; H-P6, H-P14, H-P16.

**Design and library references.** `GEN §17` Ruff semantic enrichment and
declaration candidates; `GEN §18` scope and binding generation; `ONT §8`, `§33`,
`§34`; `GEN §7.2` adapter isolation; `FAB §22`–`§24` table contracts; `FAB §63`–
`§66` encoder contract; LD-RF-01; LD-AR-01; `ruff §8.1`–`§8.17`, `§8.22`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'ruff_python_semantic' Cargo.toml Cargo.lock
ast-grep outline src/ruff_adapter.rs
jq -r '.tables[].name' contracts/generated/model/schema/table-specs.json | rg 'scope|binding|reference'
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^(22|23|24)\.'
```

Known current touch includes `Cargo.toml`/`Cargo.lock` (new exact
`ruff_python_semantic = 0.0.7`), `scripts/stable_graph_check.sh` (exact Ruff census
and `fact-generation` allowlist), `src/ruff_adapter.rs`,
`contracts/schema/schema-contract-ir.json` (three tables + observation schema),
the Python-owned ontology/pass/property/telemetry and ingest fragments, and
generated encoders. Shared ingest dispatch is frozen by WP01.

**Required changes.**

1. First compile-probe the exact `ruff_python_semantic = 0.0.7` traversal surface.
   Add only that direct dependency, update `Cargo.lock`, and update
   `scripts/stable_graph_check.sh` so its exact Ruff census, default graph, narrow
   `fact-generation` graph, and feature allowlist prove the new shape. If another
   direct Ruff helper is required, stop for the LD-RF-01 plan revision. Then build the adapter
   traversal that reproduces Ruff's binding→traversal→cleanup order (`ruff §8.3`);
   the adapter owns all Ruff types and emits only CPG DTOs (`GEN §7.2`).
2. Emit scopes (seven kinds, no generic-block approximation), binding events for
   all fifteen target forms including walrus/global/nonlocal/type-params, and
   references classified READ/WRITE/READ_WRITE/DELETE/TYPE_REFERENCE/
   CALL_REFERENCE/IMPORT_REFERENCE, with SHADOWS/REBINDS/GLOBAL_RESOLUTION/
   NONLOCAL_RESOLUTION/CAPTURES/CAPTURED_FROM edges.
3. Resolution policy per `GEN §18.4`: Ruff supplies local lexical edges;
   unresolved names emit `REFERS_TO → UNKNOWN_SYMBOL`; star-import candidates use
   `MAY_REFER_TO` when exports are known, else `UNKNOWN_SYMBOL`.
4. Define the `scope_detail`/`binding_detail`/`reference_detail` tables in the
   Contract IR (owner-scoped replacement, `owner_bucket` partitioning, `Utf8`
   only, nullable `unknown_reason_code` on references) and their typed builders
   with FAB §64 batch sizing; batches pass FAB §66 validation before Delta entry.
5. Route the batch through the registered observation schema and the
   reconciliation ingest so canonical rows carry evidence and provenance (GI-11);
   the `SCOPES_BINDINGS` capability reports per-owner state.
6. Register the Ruff traversal pass contract and its deterministic phase/failure
   telemetry; the traversal fixture proves equal output across repeated runs and
   asserts the registered terminal record on injected cleanup failure.

**Legacy Disposition and Decommission.** The syntax-only Ruff adapter surface
remains for the syntax lane; no scope/binding facts existed before, so there is
no old authority to retire. Arena-local Ruff IDs never persist (`ruff §8.1`).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_scope_binding_fixture_conformance`; Executable oracle: `ruff_semantic_isolation_parity`; Executable oracle: `py_unresolved_reference_unknown_falsification`; Executable oracle: `py_scope_binding_owner_replacement_gate`.

- **Behavioral — Executable oracle:** `py_scope_binding_fixture_conformance` —
  the `GEN §93.1` scope/binding fixture set (all scope kinds, global/nonlocal,
  shadowing, captures, comprehensions, type params) produces exact expected
  canonical rows.
- **Structural — Executable oracle:** `ruff_semantic_isolation_parity` — no
  `ruff_python_semantic` type appears in any public stable-root signature outside
  the adapter module; the three table schemas pass the FAB §11.1 round-trip gate.
- **Negative/Zero-State — Executable oracle:**
  `py_unresolved_reference_unknown_falsification` — undefined names, dynamic
  scope tricks, and unresolvable star imports yield explicit `UNKNOWN_SYMBOL`
  facts; asserting the absent-edge representation fails.
- **Operational — Executable oracle:** `py_scope_binding_owner_replacement_gate` —
  a body-local edit replaces exactly the owning module/callable's scope rows
  under owner-scoped replacement with journal provenance; unaffected owners'
  rows are byte-identical.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, adapter unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just stable-graph-check` (new
dependency), `just deps-fast`, `just model-repro-check`,
`just packet-oracle-check WP03`.

**Integration Milestone.** M01.

**Replan Triggers.** Reproducing the Ruff checker traversal at the pinned version
is infeasible or unsound for a required fact family (`ruff §8.3` option B fails)
— escalate to a design decision on vendoring a traversal layer versus demoting
Ruff to fallback; or the pinned train's `SemanticModel` lacks a binding form
`GEN §18.2` requires.

**Rollback or Recovery.** Tables are additive; capability stays `UNAVAILABLE`
until the packet's oracles pass, so a partial landing never advertises scope
facts.

### WP04 — Python module, import, and export facts

**Outcome.** Source-declared module identity, imports, aliases, exports,
re-exports, and unresolved modules are canonical facts: every import statement
yields distinct syntax, binding, resolved-module, and resolved-symbol facts;
literal `__all__` is statically evaluated; dynamic export construction yields an
explicit incomplete-export status; no runtime import execution exists anywhere.

**Dependencies.** WP03.

**Target invariants.** GI-03, GI-06; DF-P1, DF-P3.

**Design and library references.** `GEN §19` module/import/export generation;
`ONT §9.3` required distinctions; `FAB §25` `module_import_detail`; `ruff §8.14`
import model; `GEN §33` dynamic-import unknowns; LD-RF-01.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'IMPORT_DECLARATION|__all__|REEXPORT|UNKNOWN_MODULE' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^19\.'
```

Known current touch includes the WP03 adapter, the Contract IR
(`module_import_detail` table), ontology registries (module/import entity and
relation kinds), and the Python-owned ingest fragment.

**Required changes.**

1. Emit `IMPORT_DECLARATION` facts with relative level, module text, imported
   name, alias, and star flag; each imported local name becomes a `BINDING`
   linked to import syntax, resolved module, and imported symbol — one syntax
   fact may yield several semantic facts (`ONT §9.3`).
2. Statically evaluate literal `__all__` (including safe concatenation); classify
   module-scope import exposure as re-export candidates; emit
   `EXPORTS`/`REEXPORTS`; dynamic construction yields `UNKNOWN_MODULE` or an
   incomplete-export status, never a guess.
3. Wave 8 resolution uses Ruff qualified-name resolution as the local fallback
   authority; the Pyrefly upgrade path (WP10) is left explicitly open — the
   import facts carry resolution-class codes so WP10 upgrades without schema
   change.
4. Define `module_import_detail` in the Contract IR with the FAB §25 contract and
   route through the standard encoder/ingest path.

**Legacy Disposition and Decommission.** None — new fact family. The prohibition
on runtime import execution is proved, not assumed.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_import_export_fixture_conformance`; Executable oracle: `py_import_syntax_semantic_distinction_parity`; Executable oracle: `py_dynamic_export_unknown_falsification`; Executable oracle: `py_module_fact_replacement_gate`.

- **Behavioral — Executable oracle:** `py_import_export_fixture_conformance` —
  fixtures covering plain/from/relative/star/aliased imports and literal
  `__all__` variants produce exact canonical rows.
- **Structural — Executable oracle:**
  `py_import_syntax_semantic_distinction_parity` — import syntax, local binding,
  resolved module, and resolved symbol are distinct fact rows with distinct
  kinds; no path rewrites source text into canonical qualified names.
- **Negative/Zero-State — Executable oracle:**
  `py_dynamic_export_unknown_falsification` — computed `__all__`, `importlib`
  calls, and unresolvable modules produce explicit unknowns/incomplete statuses;
  no runtime import executes (structural scan: no `pyo3`/process-spawn/import
  machinery in the adapter path).
- **Operational — Executable oracle:** `py_module_fact_replacement_gate` — an
  import edit replaces only the owning module's import/export rows; module-graph
  dependents are invalidated per the declared dependency edges.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP04`.

**Integration Milestone.** M01.

**Replan Triggers.** Static `__all__` evaluation rules in `GEN §19` prove
insufficient for a required fixture class — return to the spec rather than
inventing evaluation semantics.

**Rollback or Recovery.** Additive family; capability-gated as WP03.

### WP05 — Python callable contracts, call sites, and argument binding

**Outcome.** Callable syntax contracts (parameter order and kinds, defaults,
decorators, annotations, async/generator, type parameters), first-class
`CALL_SITE` entities for every Ruff `Call`, source-declared member candidates,
and the application-owned argument binder mapping actuals to formals are
canonical facts in the `callable_detail`/`parameter_detail`/`call_site_detail`/
`call_argument_detail` tables. Dynamic splats bind to `UNKNOWN_ARGUMENT_SET`,
never silently ignored.

**Dependencies.** WP04. This serializes the shared `src/ruff_adapter.rs` write set;
there is no unordered overlap at that seam.

**Target invariants.** GI-04, GI-06; DF-P1, DF-P3, DF-P12; doctrine: call syntax ≠
callable; call site ≠ caller→callee edge.

**Design and library references.** `GEN §21.1` member candidates; `GEN §22.1`/
`§22.3` syntax contract and argument binding; `GEN §23.1` call-site facts;
`ONT §12`–`§13`; `FAB §29`–`§32`; LD-RF-01; LD-AR-01.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'CALL_SITE|HAS_ARGUMENT|ARGUMENT_BINDS_TO|DECLARES_MEMBER' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(21|22|23)\.'
```

Known current touch includes the WP03 adapter, the Contract IR (four tables),
ontology registries (`CALLABLE`, `CALL_SITE` kinds already allocated), and
ingest wiring.

**Required changes.**

1. Emit the callable syntax contract from Ruff: positional-only through kwargs
   parameter kinds, defaults, decorator list, return annotation, async/generator
   flags, type parameters (`GEN §22.1`).
2. Materialize every call expression as a first-class `CALL_SITE` with
   `HAS_CALLEE_EXPRESSION`/`HAS_RECEIVER`/`HAS_ARGUMENT`/`CONTAINS_CALL` and the
   `ONT §13` property set (span, call_syntax_kind, dispatch_kind,
   resolved_target_count, resolution_status — the latter two remain unresolved
   placeholders until WP13, and the M01 wave gate explicitly tolerates the
   placeholder state).
3. Implement the argument binder: positional order, positional-only rules,
   keyword matching, duplicate detection, statically-known splat expansion,
   defaults, bound-receiver insertion; emit `ARGUMENT_BINDS_TO`; dynamic splats
   bind to the `UNKNOWN_ARGUMENT_SET` sentinel.
4. Collect source-declared members from class bodies (`DECLARES_MEMBER`, METHOD/
   CLASS_VARIABLE/PROPERTY_CANDIDATE/NESTED_TYPE) and `self.x` assignment
   candidates with locations.
5. Define the four tables in the Contract IR; arguments are row-oriented relation
   rows, never nested lists (`FAB §65.4`).

**Legacy Disposition and Decommission.** None — new families.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_callable_call_site_fixture_conformance`; Executable oracle: `py_call_site_first_class_parity`; Executable oracle: `py_dynamic_splat_unknown_argument_falsification`; Executable oracle: `py_callable_contract_replacement_gate`.

- **Behavioral — Executable oracle:**
  `py_callable_call_site_fixture_conformance` — the `GEN §93.1` argument-form
  fixtures (every parameter kind, defaults, decorators, nested calls) produce
  exact callable, parameter, call-site, and binding rows.
- **Structural — Executable oracle:** `py_call_site_first_class_parity` — every
  syntactic call has exactly one `CALL_SITE` entity row; no caller→callee edge
  exists without its call-site anchor; argument rows are relation rows with
  ordinals.
- **Negative/Zero-State — Executable oracle:**
  `py_dynamic_splat_unknown_argument_falsification` — `*args`/`**kwargs` of
  unknown shape bind to `UNKNOWN_ARGUMENT_SET`; duplicate keywords are
  diagnostics, not dropped rows; no call expression is silently skipped.
- **Operational — Executable oracle:** `py_callable_contract_replacement_gate` —
  signature edits replace the owner's callable/parameter rows and invalidate
  dependent argument bindings per declared edges.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP05`.

**Integration Milestone.** M01.

**Replan Triggers.** The `GEN §22.3` binder rules cannot express a legal Python
call form in the fixture corpus — spec gap, return it.

**Rollback or Recovery.** Additive; capability-gated.

### WP06 — Python control-flow graphs

**Outcome.** The application-owned CFG builder produces one validated CFG per
module body, function, async function, and lambda: full `GEN §24.3` evaluation
order, the `ONT §15` node/edge vocabulary with normal and exceptional flow kept
distinct, exact and summarized exceptional edges, complete `try`/`finally`
continuation routing, and the `GEN §24.7` per-CFG validation suite — persisted
in `cfg_graph`/`cfg_node_detail`/`cfg_edge_detail` with CFG edges as relation
rows.

**Dependencies.** WP05.

**Target invariants.** GI-06, GI-14 (construction only — no derived analyses);
DF-P1, DF-P2, DF-P14.

**Design and library references.** `GEN §24` (ownership: neither Ruff nor Pyrefly
provides the durable CFG; `ruff §8.19`, `pyrefly App. C.6` concur); `ONT §15`;
`FAB §34`–`§36`; LD-PG-01 (`pg §2.2`, `§12.4`–`§12.6`, `§16.3`); `GEN §5.1`
authority order.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'CFG_BLOCK|CFG_NORMAL|cfg_graph|cfg_node|cfg_edge' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^24\.'
```

Known current touch includes a new CFG builder module (layout derived at
preflight per the scope boundary — no mandated file shape), the Contract IR
(three tables), ontology registries (CFG kinds allocated), and ingest wiring.

**Required changes.**

1. Build CFGs over the Ruff AST using petgraph `DiGraph` as the ephemeral
   construction structure with an application `cfg_node_id` identity map; graph
   indices never persist (LD-PG-01).
2. Model the fifteen `GEN §24.3` evaluation-order rules (callee-before-args,
   short circuit, chained comparisons, aug-assign read-before-write, decorator
   order, definition-time defaults, class-body execution, comprehension
   ordering) and the full `GEN §24.4` statement table including `match`,
   `try`/`try*`, `with`/`async with`, yield/await suspend-resume points; nested
   def/class evaluate definition-time expressions in the enclosing CFG with
   separate body CFGs.
3. Emit exceptional flow per `GEN §24.5`: exact explicit-raise edges, exact
   categories for call/attribute/subscript/iteration/context-manager, summarized
   block→handler/finally edges, handler type syntax preserved; `finally` routes
   every exit kind and resumes pending continuations (`GEN §24.6`).
4. Validate every CFG before encoding (`GEN §24.7`): one entry, one synthetic
   normal exit, explicit exceptional exit, successor totality, return
   non-fallthrough, valid break/continue targets, complete finally routing —
   `has_path_connecting` backs the reachability checks.
5. Define the three tables in the Contract IR; edge payloads (condition, case
   value, exception type) are columnar relation attributes.

**Legacy Disposition and Decommission.** None — new family. Derived CFG analyses
(dominators, SCCs, loops) are explicitly out of scope (GI-14, Wave 13).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_cfg_fixture_conformance`; Executable oracle: `py_cfg_wellformedness_parity`; Executable oracle: `py_cfg_exceptional_edge_falsification`; Executable oracle: `py_cfg_owner_invalidation_gate`.

- **Behavioral — Executable oracle:** `py_cfg_fixture_conformance` — the
  `GEN §93.1` control-flow fixtures (all statement forms, `except*`, match
  guards, comprehensions, async) produce exact node/edge rows.
- **Structural — Executable oracle:** `py_cfg_wellformedness_parity` — every
  encoded CFG passes the `GEN §24.7` validation suite; every node/edge row keys
  to an existing graph row; no petgraph index appears in any persisted column.
- **Negative/Zero-State — Executable oracle:**
  `py_cfg_exceptional_edge_falsification` — normal and exceptional flow never
  share an edge kind; a `finally` that swallows a return/break/continue
  continuation is detected by the validator fixture; removing an unwind path
  fails the oracle.
- **Operational — Executable oracle:** `py_cfg_owner_invalidation_gate` — a
  body-local edit rebuilds exactly the owning callable's CFG; unrelated CFGs are
  byte-identical across the wave.
**Edit-Local Gates.** `just root-fmt`, `just root-check`, builder unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP06`.

**Integration Milestone.** M01.

**Replan Triggers.** A Python construct's evaluation order cannot be modeled from
the Ruff AST without type information (would need Pyrefly) — record the explicit
capability gap and defer that construct's exact edges to a spec decision, never
guess.

**Rollback or Recovery.** Additive; capability-gated.

### WP07 — Python direct def-use, access events, and Wave 8 capability closure

**Outcome.** Owner-local forward reaching-definitions over Ruff binding domains
produce definition events, use events, normalized access paths, and
REACHING_DEFINITION/DEF_USE/DATA_DEP edges with recoverable merge provenance,
executed only through the registered `PY_OWNER_REACHING_DEFS_V1` derivation;
Wave 8's explicit-unknown families are complete (`GEN §33`); and
`PYTHON_SEMANTIC_V1` is advertised `PARTIAL` through the formal aggregation
algebra with the Pyrefly-owned mandatory capabilities explicitly missing. M01
closes on `just wave8-integration-check`.

**Dependencies.** WP04, WP05, WP06.

**Target invariants.** GI-04, GI-06, GI-10, GI-12, GI-14, GI-15; DF-P2, DF-P20,
DF-P21; H-P10, H-P14, H-P16, H-P28.

**Design and library references.** `GEN §25` value/dataflow generation (owner
scope, conservative kills, `MERGED_VALUE`, no SSA requirement); `GEN §33`
explicit unknowns; `GEN §85` capability reporting; `GEN AC-G-36` aggregation;
`ONT §18.5`, `§62`; `LIFE §7.1`–`§7.2` invalidation and parse-error scenarios;
`FAB §37`–`§41`; `RM §13` exit evidence.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'DEF_USE|REACHING|dataflow_event|value_detail|access_path' src contracts/registry -g '!**/generated/**'
rg -n 'aggregate_capability|CapabilityChild|PYTHON_SEMANTIC' src/core_facts.rs contracts/registry/capability-registry.yaml
rg -n 'SYNTAX_TREE_V1|derivation' contracts/registry src -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(25|33)\.'
```

Known current touch includes the dataflow builder (new module beside the CFG
builder), the Contract IR (`value_detail`, `operation_detail`,
`dataflow_event_detail`, `memory_location_detail`, `access_path_component`),
the Python-owned capability/ingest fragments, and the
`justfile` (`wave8-integration-check` population).

**Required changes.**

1. Emit value nodes, definition events for the thirteen `GEN §25.3` binding
   forms, and use events (reads, receivers, callees, args, conditions,
   return/yield, index, decorators, evaluated annotations).
2. Normalize access paths per `GEN §25.4` (LOCAL/GLOBAL/CELL, FIELD/
   INSTANCE_MEMBER, CLASS_MEMBER, INDEXED_LOCATION, MODULE) into
   `memory_location_detail` + row-oriented `access_path_component`.
3. Run owner-local forward reaching definitions over the WP06 CFG with Ruff
   binding identity as the variable domain and conservative kills for
   attribute/container locations; emit REACHING_DEFINITION/REACHES/DEF_USE/
   DATA_DEP/VALUE_FLOWS_TO/KILLS_DEFINITION; CFG-join merges get `MERGED_VALUE`
   with recoverable provenance.
4. Before any row in those families can stage or publish, register exactly one
   `PY_OWNER_REACHING_DEFS_V1` entry in the derivation registry with owner packet,
   input families, output families, algorithm/precision/bundle IDs, context and
   source fingerprints, invalidation closure, resource profile, pass contract,
   and implementation symbol. `ProviderRuntime`/generation adapters may emit only
   normalized value/access/CFG inputs; the lifecycle scheduler invokes the
   registry-selected implementation, and every output row stamps the selected
   profile and bundle. Publication rejects an absent, duplicate, mismatched, or
   directly invoked implementation.
5. Complete the `GEN §33` unknown table for Wave 8: `getattr`/`exec`/dynamic
   import emit both the observable syntax fact and the unknown remainder;
   every condition row in `GEN §84` applicable to local Python analysis has a
   producer.
6. Advertise `PYTHON_SEMANTIC_V1` as `PARTIAL` via `aggregate_capability`: the
   Pyrefly-owned capabilities (project types, cross-module resolution, members,
   call targets) are explicitly `UNAVAILABLE_PROVIDER` children with named
   reasons; parse-error scenarios publish source + error-tolerant CST + gaps
   (`LIFE §7.2`).
7. Populate `just wave8-integration-check` with the Wave 8 oracle selection plus
   the Python-scenario subset of `just rebuild-equivalence-check`.

**Legacy Disposition and Decommission.** None — new families. Liveness
materialization (`LIVE_AT`) stays disabled; it is a Wave 13 derivation decision.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_defuse_fixture_conformance`; Executable oracle: `py_semantic_profile_partial_parity`; Executable oracle: `py_parse_error_capability_gap_falsification`; Executable oracle: `wave8_integration_operational_gate`.

- **Behavioral — Executable oracle:** `py_defuse_fixture_conformance` — def-use
  fixtures (rebinding, branches, loops, try/finally, comprehensions,
  augmented assignment) produce exact reaching-definition and def-use rows with
  merge provenance.
- **Structural — Executable oracle:** `py_semantic_profile_partial_parity` — the
  advertised profile status derives from the aggregation algebra over per-owner
  capability rows; every missing mandatory capability is a named child with a
  reason code, and the profile can never report `COMPLETE` while one exists. The
  six derived output families resolve to exactly one registry entry and every row
  carries its selected precision/bundle IDs.
- **Negative/Zero-State — Executable oracle:**
  `py_parse_error_capability_gap_falsification` — a syntax-broken module
  publishes current source + CST + diagnostics, withdraws semantic families for
  affected owners, and never leaves a stale scope/def-use row visible; a
  dynamic construct without its unknown remainder fails. A generation-adapter or
  direct-function attempt to publish the registered derived families is rejected.
- **Operational — Executable oracle:** `wave8_integration_operational_gate` —
  `just wave8-integration-check` passes end-to-end: fixture corpus → continuous
  waves → published facts → body-local incremental scenarios equal to clean
  rebuild for every Wave 8 family.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, solver unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just wave8-integration-check`,
`just rebuild-equivalence-check`, `just packet-oracle-check WP07`.

**Integration Milestone.** M01.

**Replan Triggers.** Owner-local reaching definitions cannot converge within the
resource envelope on the fixture corpus — precision-profile decision goes back
to the derivation-registry design rather than ad hoc widening.

**Rollback or Recovery.** Additive; the profile advertisement is
aggregation-derived, so removing a family's rows automatically demotes it.

### Wave 9 group — Pyrefly project semantics and Python profile closure (`RM §14`)

### WP08 — Pyrefly sidecar production service and job integration

**Outcome.** The Pyrefly sidecar is a production provider: the complete
`codefabric.pyrefly.v1` protocol (handshake negotiation that fails on
protocol/schema/bundle mismatch before analysis, OpenContext sessions,
credit-controlled Arrow streaming in strict event order, staged output released
only on verified terminal records, idempotent cancellation, crash/restart/reopen
with exponential backoff, generation supersession), executing under
`ProviderRuntime` with the `GEN AC-G-35` sandbox, supervised long-lived `Query`
state per context, and last-good snapshot retention that is never present-state
truth.

**Dependencies.** WP07, WP36.

**Target invariants.** GI-04, GI-09, GI-11, GI-12, GI-13, GI-15; DF-P9, DF-P20,
DF-P23; H-P8, H-P22, H-P23, H-P28.

**Design and library references.** `GEN AC-G-30` (complete wire protocol);
`GEN AC-G-32` (accepted-handle model, 2 s sidecar cancel-ack); `GEN AC-G-35`
(trust profiles, fail-closed containment); `GEN §7.3` isolation and path
replacement; `GEN §90`; `LIFE §7.4` sidecar-unavailable semantics; LD-PY-01
(`pyrefly §6`, `§29.4`–`§29.6`, `§30.1`, `§30.4`).

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline pyrefly-sidecar/src/server.rs
rg -n 'Handshake|OpenContext|AnalyzeModules|CancelRun|credit|MAX_ARROW_CHUNK' pyrefly-sidecar/src src/pyrefly_service.rs contracts/rpc/pyrefly_sidecar.proto
rg -n 'sandbox|trust_profile|Seatbelt|landlock' src contracts/registry -g '!**/generated/**'
```

Known current touch includes `pyrefly-sidecar/src/{server.rs,pyrefly_link.rs,main.rs}`,
`src/pyrefly_service.rs`, `src/provider_runtime.rs` (the WP01 adapter),
`contracts/rpc/pyrefly_sidecar.proto` + `feature-registry.yaml` (event-mapping
table), the provider and resource-profile registries, and the operational store
(run journaling). The shared serving consumer was cut over once by WP36.

**Required changes.**

1. Close every `GEN AC-G-30` protocol obligation the current server does not yet
   prove: strict event ordering with monotonic sequences, staged output invalid
   without covering `ModuleEnd` + `RunTerminal`, digest echo on terminals,
   credit accounting (4 initial chunks, ≤16 MiB unacknowledged,
   `ChunkAccepted`/`ChunkRejected`), idempotent `CancelRun` with the 2 s ack
   deadline, and multi-context hosting under a negotiated memory profile with
   serialized context mutation.
2. Supervise the sidecar as a restartable child: crash rejects the whole active
   run, restart uses exponential backoff and requires context reopen; a newer
   generation supersedes queued and in-flight runs; completed stale output is
   discarded (`GEN §90` fences).
3. Apply LD-SB-01 through the WP36 sandbox substrate to the sidecar per
   `GEN AC-G-35` (default `UNTRUSTED_SANDBOXED`: no network, read-only leased
   blobs, quotaed private tmp, resource limits, process-group termination); when
   containment cannot be established, Python semantic execution reports
   unavailable — never unsandboxed (GI-13). Exercise network, credential/live-
   workspace read, out-of-root write, FD, child/process-tree, resource,
   cancellation, and cleanup escapes on every advertised host. Sandbox profile
   digest and trust profile pin into runs and snapshots.
4. Keep one long-lived `Query` per open context (`pyrefly §30.1`), commit state
   behind the content-digest snapshot barrier (`pyrefly §6.6`), and record the
   in-sidecar `rechecked_modules` impact surface as an application extension for
   dependency-driven invalidation (WP14).
5. Register the lane implementation behind the WP36-converted dispatch port and
   prove the Gate B vertical and `src/fabric/serving.rs` contain no Python
   direct-spawn call site (DB01 Python half); do not edit the shared consumer.

**Legacy Disposition and Decommission.** The hard-coded sidecar invocation in
`src/gate_b_candidate/vertical.rs` (fixed run IDs, fixed digests, fixed module
lists) is retired in favor of the production adapter path — negative proof in
DB01. Last-good sidecar state is a declared operational cache (DF-P23), never
serving-visible for invalidated owners.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `pyrefly_protocol_conformance`; Executable oracle: `pyrefly_provider_runtime_parity`; Executable oracle: `pyrefly_stale_generation_rejection_falsification`; Executable oracle: `pyrefly_crash_restart_operational_gate`.

- **Behavioral — Executable oracle:** `pyrefly_protocol_conformance` — a scripted
  session exercises handshake negotiation (including a version/schema mismatch
  that fails before analysis), OpenContext, a multi-module AnalyzeModules stream
  with credit stalls, cancellation, and clean shutdown, matching the AC-G-30
  event grammar exactly.
- **Structural — Executable oracle:** `pyrefly_provider_runtime_parity` — sidecar
  wire events map to the single application event taxonomy via the
  `feature-registry.yaml` table; every run flows through `ProviderRuntime`
  admission/journal; no second invocation path exists in the stable root.
- **Negative/Zero-State — Executable oracle:**
  `pyrefly_stale_generation_rejection_falsification` — output arriving for a
  superseded generation, a partial stream missing `ModuleEnd`, and a digest
  mismatch are all rejected with the registered reason; no staged row reaches
  ingest.
- **Operational — Executable oracle:** `pyrefly_crash_restart_operational_gate` —
  a mid-run sidecar kill produces run rejection, backoff restart, context
  reopen, and a successful rerun; invalidated prior semantics are never visible
  during the outage (`LIFE §7.4`).

**Edit-Local Gates.** `just sidecar-fmt`, `just sidecar-check`, targeted server
tests.

**Packet-Local Gates.** `just sidecar-ci-fast`, `just root-ci-fast`,
`just provider-protocol-check`, `just semantic-sandbox-host-matrix-check`,
`just semantic-provider-legacy-zero-state-check`, `just packet-oracle-check WP08`.

**Integration Milestone.** M02.

**Replan Triggers.** The pinned Pyrefly source cannot host multiple read-only
contexts in one process within the memory profile (`GEN AC-G-30`'s hosting
clause) — process-per-context is an implementation adaptation; a protocol change
is a plan revision. A host that cannot pass the exact LD-SB-01 matrix may retain
the explicit `TRUSTED_LOCAL` development grant, but WP08 cannot certify
`UNTRUSTED_SANDBOXED` or M02 `COMPLETE` for that host.

**Rollback or Recovery.** Provider path is capability-gated; disabling the
adapter registration restores the Wave 8 `PARTIAL` posture without data
migration.

### WP09 — Python immutable source and context transfer

**Outcome.** Every byte the sidecar analyzes arrives through the `GEN AC-G-33`
transport: content-addressed leased blobs, a daemon-built immutable
`ProviderWorkspaceView` when project-relative layout is required, a pinned
`DependencyInputBundle` for typeshed/stubs/locked third-party sources, BOM and
PEP 263 encoding maps back to authoritative bytes, size thresholds, and
end-to-end digest fencing — with module requests batched by dependency
neighborhood.

**Dependencies.** WP08.

**Target invariants.** GI-03, GI-09, GI-13; DF-P9, DF-P11.

**Design and library references.** `GEN AC-G-33` (blob transport, workspace view,
dependency bundles, thresholds, encoding, `SOURCE_SNAPSHOT_MISMATCH`);
`GEN AC-G-30` lease-only reads; `LIFE §95` neighborhood batching and digest
eligibility; existing `src/source_image.rs` lease machinery; LD-PY-01
(`pyrefly §0.5` ModulePath backings, `§4.3`–`§4.4` config identity).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'SourceSnapshotLease|acquire_serving_snapshot_lease|BlobStore|codefabric-source' src/source_image.rs src/pyrefly_service.rs pyrefly-sidecar/src
rg -n 'typeshed|DependencyInputBundle|ProviderWorkspaceView' src contracts -g '!**/generated/**'
```

Known current touch includes `src/source_image.rs` (view construction),
`src/pyrefly_service.rs` (request assembly), `pyrefly-sidecar/src/server.rs`
(view resolution), the context tables (typeshed/dependency digests from WP02),
and the operational store (lease serialization).

**Required changes.**

1. Instantiate the WP36 view/bundle substrate for Python: the immutable
   `ProviderWorkspaceView` (manifest of WorkspacePath →
   blob digest/mode, atomically published after verification, no writable link
   to the live workspace or `.git`, separate writable output root) and use it
   whenever Pyrefly needs project-relative layout; otherwise serve
   content-addressed blob references under the snapshot lease.
2. Deliver typeshed and locked third-party dependencies via the context-pinned
   `DependencyInputBundle`; mounting is not indexing authorization; no network
   resolution exists.
3. Implement encoding handling: BOM + PEP 263 detection, provider-compatible
   text with byte-offset maps back to original authoritative bytes; enforce the
   256 KiB inline / 16 MiB ordinary / 64 MiB maximum thresholds with explicit
   capability gaps beyond.
4. Fence digests end-to-end: every module request carries the expected digest,
   terminals echo it, mismatch rejects with `SOURCE_SNAPSHOT_MISMATCH`; sidecar
   responses are eligible only when digests match the wave (`LIFE §95`).
5. Batch AnalyzeModules requests by module dependency neighborhood using the
   WP04 local module graph; WP10 upgrades the batching input to the resolved
   project graph after it lands.

**Legacy Disposition and Decommission.** The Gate-B-era direct file paths into
the sidecar reach zero (covered by DB01's negative proof); provider-rendered
absolute paths are replaced with `codefabric-source://` locators at the boundary
(already begun; completed and proven here).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_source_transfer_conformance`; Executable oracle: `py_workspace_view_immutability_parity`; Executable oracle: `py_snapshot_digest_mismatch_falsification`; Executable oracle: `py_dependency_bundle_operational_gate`.

- **Behavioral — Executable oracle:** `py_source_transfer_conformance` — fixture
  workspaces (BOM/PEP 263 encodings, oversize files, namespace layouts) analyze
  correctly through the view with byte-exact offset maps back to authoritative
  bytes.
- **Structural — Executable oracle:** `py_workspace_view_immutability_parity` —
  the view manifest verifies digest/mode for every entry; no path in the view
  resolves into the live workspace or `.git`; the sidecar sandbox cannot open a
  non-view path (probe).
- **Negative/Zero-State — Executable oracle:**
  `py_snapshot_digest_mismatch_falsification` — a stale-digest request and a
  tampered blob both reject with `SOURCE_SNAPSHOT_MISMATCH` and no partial
  ingest; an oversize source yields an explicit capability gap, not truncation.
- **Operational — Executable oracle:** `py_dependency_bundle_operational_gate` —
  a typeshed/stub bundle change invalidates dependent type facts (with WP02's
  context identity), and lease acquisition/renewal/GC stays serialized through
  the operational writer with restart-orphan grace.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, `just sidecar-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just source-capture-race-check`, `just packet-oracle-check WP09`.

**Integration Milestone.** M02.

**Replan Triggers.** Pyrefly's in-memory `ModulePath` backing proves unusable at
the pinned source for view-free transfer (the reference marks it
present-in-principle, unproven) — the workspace-view path is the fallback and is
already in scope, so this is an implementation adaptation unless it forces a
protocol change.

**Rollback or Recovery.** Transfer machinery is provider-internal; reverting to
the prior packet's posture requires no data migration.

### WP38 — Pyrefly module/xref surface binding and owned report adapter

**Outcome.** The exact pinned Pyrefly Glean-report path is compile- and
behavior-proved before any module/xref consumer lands: an isolated sidecar adapter
invokes `FullCheckArgs --report-glean` in-process over the immutable WP09 view,
decodes bounded per-module reports into application-owned module/declaration/
import/export/xref observations, and exposes them through a versioned extension of
`codefabric.pyrefly.v1`. This packet is the F-002 capability gate; WP10 cannot
begin from type-table/callee speculation.

**Dependencies.** WP09.

**Target invariants.** GI-03, GI-04, GI-05, GI-09, GI-11, GI-13, GI-15;
DF-P3, DF-P9, DF-P18; H-P6, H-P8, H-P14, H-P16, H-P23.

**Design and library references.** `GEN §5.1`, `§18.4`, `§19.2`–`§19.4`
(Pyrefly Glean/LSP/internal adapter authority); `GEN AC-G-30`, `AC-G-33`;
LD-PY-01, LD-PY-02; `pyrefly §9`–`§13`, `§24`, `§26`, `§30.1`; exact pinned
source `pyrefly/lib/commands/check.rs` and `pyrefly/lib/report/glean/convert.rs`
are binding evidence for the unstable surface.

**Change surface / Preflight / Known Touch.** Run:

```bash
RUSTC_WRAPPER= cargo metadata --manifest-path pyrefly-sidecar/Cargo.toml --locked --format-version 1 >/dev/null
RUSTC_WRAPPER= cargo metadata --manifest-path pyrefly-sidecar/Cargo.toml --locked --format-version 1 | jq -r '.packages[] | select(.name == "pyrefly") | [.version, .source] | @tsv'
rg -n 'FullCheckArgs|report-glean|module_xref' pyrefly-sidecar/src pyrefly-sidecar/Cargo.toml
```

Known touch includes `pyrefly-sidecar/Cargo.toml`/`Cargo.lock` (exact direct
`clap = 4.6.2`), an owned report-adapter module beside
`pyrefly-sidecar/src/pyrefly_link.rs`, `pyrefly-sidecar/src/server.rs`, the
Pyrefly Protobuf/feature registry if new stream variants are required, the
provider-observation Contract IR/projection, fixed module/xref fixtures, and the
`justfile` recipe `pyrefly-module-xref-surface-check`.

**Required changes.**

1. Compile-bind only the LD-PY-02 surface: construct `FullCheckArgs` through
   `clap::Parser`, select `--report-glean`, and call its public async `run` in the
   sidecar. Keep the long-lived LD-PY-01 `Query` path separate; no Pyrefly patch,
   private-module import, shell, or Pyrefly subprocess is permitted.
2. Execute the report pass only against the immutable digest-verified WP09
   `ProviderWorkspaceView` and context bundle. The output directory is a private,
   quotaed sandbox root; reports are terminal-staged, bound to the requested
   source/context/adapter/profile digests, rejected as a unit on mismatch, and
   removed after decode or failure.
3. Decode the pinned Glean JSON defensively into application-owned DTOs for module
   identity/dependency, declaration/definition location, imports (including
   relative and star), exported/re-exported names where evidenced, and bulk xrefs.
   Enforce field/type/size/path/range limits, retain raw provider kind/provenance,
   and emit explicit `UNAVAILABLE_PROVIDER`/unknown reasons for absent predicates;
   no report-local ID becomes canonical identity.
4. Extend the observation/protocol contract only with those owned DTOs and add
   fixtures for packages, namespace packages, relative imports, stubs, re-export
   chains, literal/dynamic `__all__`, star imports, cross-module definitions/xrefs,
   source mismatch, corrupt/oversize reports, cancellation, and cleanup.
5. Add `just pyrefly-module-xref-surface-check`: exact metadata/revision/clap
   checks, compile binding, fixture behavior, DTO isolation scan, snapshot fence,
   and cleanup. Register the Glean decode pass contract, fault points, and bounded
   report count/bytes/decode-time/failure telemetry.

**Legacy Disposition and Decommission.** This packet removes v1's unsupported
type-table/callee-derived module/xref fallback from the plan. It does not replace
the long-lived Query or create a second canonical authority; its output is provider
observation evidence reconciled by WP10.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `pyrefly_glean_surface_compile_conformance`; Executable oracle: `pyrefly_glean_dto_isolation_parity`; Executable oracle: `pyrefly_glean_snapshot_mismatch_falsification`; Executable oracle: `pyrefly_module_xref_surface_operational_gate`.

- **Behavioral — Executable oracle:**
  `pyrefly_glean_surface_compile_conformance` — at the exact revision, the owned
  adapter compiles and the complete module/import/export/xref fixture matrix
  decodes to the expected owned observations.
- **Structural — Executable oracle:** `pyrefly_glean_dto_isolation_parity` — no
  Pyrefly/Glean report type or report-local ID crosses the adapter/protocol
  boundary; schema projections and DTO field mappings are exhaustive.
- **Negative/Zero-State — Executable oracle:**
  `pyrefly_glean_snapshot_mismatch_falsification` — digest/context mismatch,
  malformed/oversize report, output escape, cancellation, and partial report sets
  reject the entire staging set with typed diagnostics and terminal telemetry.
- **Operational — Executable oracle:**
  `pyrefly_module_xref_surface_operational_gate` —
  `just pyrefly-module-xref-surface-check` passes from a clean sidecar build,
  report directories are removed on success/failure/restart, and no unbounded
  output or orphan process remains.

**Edit-Local Gates.** `just sidecar-fmt`, `just sidecar-check`, targeted adapter
and decoder tests.

**Packet-Local Gates.** `just pyrefly-module-xref-surface-check`,
`just sidecar-ci-fast`, `just provider-protocol-check`, `just model-repro-check`,
`just packet-oracle-check WP38`.

**Integration Milestone.** M02.

**Replan Triggers.** The exact public-hidden parser/run surface disappears; the
Glean output cannot prove any mandatory module/export/xref fixture; snapshot
fencing requires live-workspace access; or the sidecar needs a Pyrefly source
patch. Stop before WP10 and version the plan/library decision—do not derive missing
facts from unrelated Query responses.

**Rollback or Recovery.** The adapter/protocol feature is capability-gated. A
failed probe leaves Wave 8's explicit `PARTIAL` posture and cannot activate WP10.

### WP10 — Pyrefly module and symbol resolution

**Outcome.** Pyrefly is the primary authority for project module resolution:
`IMPORTS_MODULE`/`IMPORTS_SYMBOL` edges, project module graph, cross-module
declarations and references, and export/index data upgrade the Wave 8 local
edges; Ruff qualified-name resolution demotes to fallback; unresolved and
external endpoints stay explicit.

**Dependencies.** WP38.

**Target invariants.** GI-04, GI-05, GI-06; DF-P3.

**Design and library references.** `GEN §19.2`/`§19.4` (authority upgrade,
export data); `GEN §18.4` (reference-resolution upgrade); `GEN §5.1` authority
order; `pyrefly §26` import machinery; `pyrefly §11.7` target-string
reconciliation; `FAB §25`; `GEN §84` unknown rules.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'IMPORTS_MODULE|IMPORTS_SYMBOL|resolution_class|MAY_REFER_TO' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(18|19)\.'
```

Known current touch includes the WP38 owned module/xref report adapter,
the observation schema (new columns/streams via the Contract IR),
the Python-owned reconciliation fragment, and the WP04 import facts (resolution-class
upgrade in place, no schema change).

**Required changes.**

1. Consume only WP38's application-owned, snapshot-fenced module-resolution and
   declaration/xref observations (module graph, import targets, exported names,
   star-import export sets); register their observation schema through the
   Contract IR. WP10 does not invoke or reinterpret Glean directly.
2. Reconcile per `GEN §5.3`/`§19.2`: Pyrefly becomes primary for cross-module
   resolution; Ruff lexical edges remain as fallback evidence with their
   original resolution class; conflicts retain both with a diagnostic.
3. Upgrade star-import handling: known export sets convert `MAY_REFER_TO`
   candidates to resolved bindings; unresolved modules remain `UNKNOWN_MODULE`
   endpoints.
4. Reconcile Pyrefly target strings to canonical symbols in the `GEN §23.3`
   order (exact internal declaration → internal stub → external symbol →
   unknown external) — shared infrastructure that WP13 reuses for call targets.

**Legacy Disposition and Decommission.** Ruff-primary resolution for
cross-module facts ends here — demotion is authority-table-driven, not deletion;
the fallback stays for sidecar-unavailable degradation (`LIFE §7.4`).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `pyrefly_module_resolution_conformance`; Executable oracle: `py_import_authority_upgrade_parity`; Executable oracle: `py_unresolved_module_endpoint_falsification`; Executable oracle: `py_module_graph_invalidation_gate`.

- **Behavioral — Executable oracle:** `pyrefly_module_resolution_conformance` —
  cross-module fixtures (packages, relative imports, stubs, re-export chains,
  star imports) match canonical resolved edges.
- **Structural — Executable oracle:** `py_import_authority_upgrade_parity` —
  canonical cross-module edges carry Pyrefly authority with Ruff evidence
  retained; the per-family authority table in the generated registry matches
  `GEN §5.1`.
- **Negative/Zero-State — Executable oracle:**
  `py_unresolved_module_endpoint_falsification` — unresolvable imports remain
  explicit `UNKNOWN_MODULE`/external endpoints; no silent absence; sidecar
  unavailability demotes (never deletes) to fallback-class edges.
- **Operational — Executable oracle:** `py_module_graph_invalidation_gate` — a
  module rename propagates through the dependency graph: exactly the dependent
  owners re-resolve, verified against the operational dependency edges.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, `just sidecar-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP10`.

**Integration Milestone.** M02.

**Replan Triggers.** WP38's proved owned surface loses a mandatory fixture or
cannot express an authority distinction required by `GEN §19`; a Pyrefly source
patch or a different LSP/internal surface requires a plan revision before WP10.

**Rollback or Recovery.** Authority demotion is table-driven and reversible;
canonical rows re-reconcile from retained evidence.

### WP11 — Canonical Python type enrichment

**Outcome.** Computed, declared, expected, and narrowed types are canonical
facts: per-file type tables intern into the context-scoped canonical type
algebra (`ONT AC-G-15`) with structured-shape identity; response-local indices
never persist; `NARROWS_TO` derives cause from the WP06 CFG; uncertainty stays
distinct (UNKNOWN_TYPE / explicit-vs-implicit ANY / UNBOUND / NEVER) and missing
type output is never `Any`. WP11 populates the lane-neutral
`type_detail`/`type_fact_detail` tables defined by WP01 and reuses the existing
shared `TypeInterner`; it creates no new table or interner authority.

**Dependencies.** WP10.

**Target invariants.** GI-04, GI-05, GI-06; DF-P1, DF-P3, DF-P18.

**Design and library references.** `GEN §20` type generation; `ONT AC-G-15`
canonical type algebra; `ONT §35`; `GEN §82` type reconciliation; `FAB §26`–
`§27`; LD-PY-01 (`pyrefly §10` type table, `§8.1` declared-vs-computed TSP
split, `§8.3` narrowing derivation, `§8.5`–`§8.6`).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'type_table_json|type_index|SEMANTIC_TYPE|HAS_TYPE|COMPUTED_TYPE' src pyrefly-sidecar contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md --match '^35\.'
```

Known current touch includes `pyrefly-sidecar/src/pyrefly_link.rs` (typed table
decoding replaces the opaque `type_table_json` blob), the observation schema,
the existing `TypeConstructor`/`TypeTerm`/`TypeInterner` in `src/identity.rs`,
the WP01 common type-table projections, ontology registries, and
the Python-owned type reconciliation fragment.

**Required changes.**

1. Decode the Pyrefly type table structurally in the sidecar and emit typed
   observation columns (normalized shape, occurrence ranges, response-local
   index) instead of the opaque JSON blob (DB03).
2. Extend and consume WP01's shared interner: identity from structured normalized
   shape (kind, qualified name, ordered args, callable shapes, bounds,
   TypedDict/tuple traits); Pyrefly structural hashes only accelerate and are
   verified against shape equality; `type_index` dies at the response boundary.
3. Emit `COMPUTED_TYPE` occurrences mapped to the best expression/reference node
   via the `GEN §80` range ladder; declared types from Ruff annotations (+ TSP
   `getDeclaredType` where the sidecar exposes it) as DECLARED_TYPE/
   PARAMETER_TYPE/RETURN_TYPE/FIELD_TYPE; expected types only from provider
   facts — no assignment-syntax inference unless exact and marked
   `STATIC_SEMANTIC`.
4. Emit type relationships (TYPE_ARGUMENT…UNION_MEMBER) as TYPE-family relation
   rows; `is_subtype` remains an on-demand oracle — no all-pairs closure.
5. Emit `NARROWS_TO` when an occurrence type strictly refines the declared type,
   with cause classified from the WP06 CFG or omitted; keep UNKNOWN_TYPE,
   explicit/implicit ANY, UNBOUND, NEVER distinct; missing output → explicit
   unknown, never `Any` (`GEN §20.7`).

**Legacy Disposition and Decommission.** The opaque `type_table_json` evidence
blob is superseded by typed observations (DB03 owns the zero state). Declared
and computed types coexist by design — no overwrite in either direction
(`GEN §82`).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_type_enrichment_conformance`; Executable oracle: `py_type_interning_identity_parity`; Executable oracle: `py_missing_type_not_any_falsification`; Executable oracle: `py_type_table_ingest_operational_gate`.

- **Behavioral — Executable oracle:** `py_type_enrichment_conformance` — the
  `GEN §93.1` typing fixtures (generics, protocols, overloads, TypedDict,
  unions, narrowing chains) produce exact canonical type entities and
  occurrence facts.
- **Structural — Executable oracle:** `py_type_interning_identity_parity` —
  equal normalized shapes intern to equal `type_id`s across modules, runs, and
  process restarts; no response-local index or Pyrefly hash appears in any
  persisted identity column.
- **Negative/Zero-State — Executable oracle:**
  `py_missing_type_not_any_falsification` — a module with suppressed/missing
  type output yields UNKNOWN_TYPE facts and a capability gap; asserting `Any`
  for it fails; implicit and explicit `Any` remain distinguishable.
- **Operational — Executable oracle:** `py_type_table_ingest_operational_gate` —
  type ingest at FAB §64 batch sizes passes validation kernels; a stub change
  invalidates exactly the dependent type facts (`LIFE §7.6`).

**Edit-Local Gates.** `just root-fmt`, `just root-check`, `just sidecar-check`,
interning unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP11`.

**Integration Milestone.** M02.

**Replan Triggers.** Declared/expected types prove unreachable at the pinned
source through both TSP and a sidecar extension — the profile decision (ship
`PYTHON_SEMANTIC_V1` with declared types from Ruff annotations only) goes back
to the ontology owner; interning identity that cannot satisfy `ONT AC-G-15`'s
algebra is a design reopening.

**Rollback or Recovery.** Type families are additive and capability-gated;
DB03's zero state only completes after this packet proves the typed path.

### WP12 — Python object model and member resolution

**Outcome.** Inheritance, MRO, attributes, descriptors, properties, overrides,
and resolved member access are canonical: `INHERITS` from resolved bases,
application-computed C3 linearization with ordered `MRO_PRECEDES` (unknown bases
→ `UNKNOWN_TYPE`), descriptor/property/classmethod/staticmethod edges,
deterministic `OVERRIDES`, and access-site member resolution through receiver
types with union receivers fanning to `MAY_RESOLVE_MEMBER` and dynamic fallback
including `UNKNOWN_MEMBER`. The `member_relation_detail` table lands.

**Dependencies.** WP11.

**Target invariants.** GI-04, GI-06; DF-P3, DF-P20.

**Design and library references.** `GEN §21` object/member generation; `ONT §36`;
`FAB §28`; LD-PY-01 (`pyrefly §12` `get_attributes` declared-class scope and
display-string caveat, `§27.2` MRO as derived); `GEN §83` candidate semantics.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'MRO|INHERITS|RESOLVES_MEMBER|DESCRIPTOR|OVERRIDES' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^21\.'
```

Known current touch includes the sidecar link (attribute extraction), a new
member-resolution module in the stable root, the Contract IR
(`member_relation_detail`), ontology registries, and ingest wiring.

**Required changes.**

1. Enrich WP05's source-declared member candidates with Pyrefly attribute data
   (member type via the WP11 interner, property/field kind, finality,
   synthesized members — which may have no source declaration to anchor and
   then use provider-only synthetic occurrence anchoring per `GEN §80`).
2. Compute MRO: `INHERITS` from resolved bases; C3 linearization
   application-side with ordered `MRO_PRECEDES`; unknown/dynamic bases inject
   `UNKNOWN_TYPE` and demote downstream member completeness.
3. Emit descriptor/property structure (PROPERTY_FOR/DESCRIPTOR_FOR/GETTER_FOR/
   SETTER_FOR/DELETER_FOR/CLASS_METHOD_OF/STATIC_METHOD_OF) and deterministic,
   non-evaluative `OVERRIDES`/`OVERRIDDEN_BY` via MRO-parent traversal.
4. Resolve member access sites: receiver computed type → member resolution →
   `RESOLVES_MEMBER`; union receivers emit per-candidate `MAY_RESOLVE_MEMBER`;
   `__getattr__`/descriptor/metaclass/dynamic writes include `UNKNOWN_MEMBER`
   in the candidate set (`ONT §36.1`).

**Legacy Disposition and Decommission.** None — new family. The Pyrefly
annotation display string is never machine identity; member types must resolve
through the WP11 interner or stay explicitly unresolved.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_member_resolution_conformance`; Executable oracle: `py_mro_derivation_parity`; Executable oracle: `py_dynamic_member_unknown_falsification`; Executable oracle: `py_member_fact_replacement_gate`.

- **Behavioral — Executable oracle:** `py_member_resolution_conformance` — the
  `GEN §93.1` object-model fixtures (multiple inheritance, descriptors,
  properties, class/static methods, overrides) match canonical rows.
- **Structural — Executable oracle:** `py_mro_derivation_parity` — application
  C3 output equals the fixture-expected linearization for every class; MRO facts
  carry derivation provenance; member types reference interned `type_id`s, not
  display strings.
- **Negative/Zero-State — Executable oracle:**
  `py_dynamic_member_unknown_falsification` — `__getattr__` classes, metaclass
  tricks, and monkey-patched members always include `UNKNOWN_MEMBER` in the
  candidate set; a closed-member claim on such a class fails.
- **Operational — Executable oracle:** `py_member_fact_replacement_gate` — a
  base-class edit invalidates dependent MRO/member facts across modules per the
  dependency graph; unrelated classes are untouched.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, C3 unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP12`.

**Integration Milestone.** M02.

**Replan Triggers.** Structured member types require extending the pinned
sidecar through the type-table machinery and that extension proves infeasible —
member types then remain unresolved-with-evidence (allowed by `GEN §21.2`), a
recorded implementation adaptation, not silent degradation.

**Rollback or Recovery.** Additive; capability-gated.

### WP13 — Python call-target enrichment

**Outcome.** Every Python call site carries its reconciled target partition:
Pyrefly callee kinds mapped to the `ONT §37` vocabulary, targets resolved to
canonical symbols, constructor calls split into `__new__`/`__init__` plus the
class contract edge, `__call__` dispatch, decorator applications resolved, and
the dynamic remainder explicit — exact targets never eliminate the unknown
remainder under dynamic dispatch. The `call_target_detail` table lands with one
row per candidate.

**Dependencies.** WP11, WP12.

**Target invariants.** GI-04, GI-06; DF-P3, DF-P20; doctrine: resolved target set ≠
unknown target.

**Design and library references.** `GEN §23` call-site/dispatch generation;
`GEN §22.2` semantic callable contract; `GEN §83` call-target reconciliation;
`ONT §14`, `§37`; `FAB §33` (`call_target_detail` PK, no `CALLS` collapse);
LD-PY-01 (`pyrefly §11` callees, `§11.4`–`§11.7`).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'callees_json|CALLS_EXACT_TARGET|MAY_CALL|CALLS_UNKNOWN|dispatch_kind' src pyrefly-sidecar contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^23\.'
```

Known current touch includes the sidecar link (typed callee observations
replacing `callees_json` — DB03), the observation schema, the Python-owned
reconciliation fragment, the Contract IR (`call_target_detail`), and the WP05 call-site
rows (resolution_status/resolved_target_count upgrade in place).

**Required changes.**

1. Decode `get_callees_with_location` structurally in the sidecar; map kinds to
   DIRECT_FUNCTION_CALL/BOUND_METHOD_CALL/CLASS_METHOD_CALL/STATIC_METHOD_CALL/
   CONSTRUCTOR_CALL/CALLABLE_OBJECT_CALL/DECORATOR_APPLICATION.
2. Resolve target strings through the WP10 resolution order; emit
   CALLS_EXACT_TARGET/CALLS_DECLARATION/MAY_CALL per candidate row with
   certainty codes; constructors split `__new__`/`__init__` targets plus the
   class constructor-contract edge; `__call__` receivers get callable-object
   dispatch; decorators get `DECORATED_BY` plus a resolved application call
   site.
3. Partition per `GEN §83`: exact / sound-may / modelled / unknown remainder;
   dynamic receivers (`Any`, getattr, registries, dynamic import, monkey
   patching) emit `CALLS_UNKNOWN → UNKNOWN_CALL_TARGET` with
   `dispatch_kind = UNKNOWN_DYNAMIC`, coexisting with any known candidates.
4. Enrich the semantic callable contract (resolved parameter/return types via
   WP11, overloads, bound receiver) and connect argument binding to resolved
   contracts where the target is exact.

**Legacy Disposition and Decommission.** The opaque `callees_json` evidence blob
is superseded (DB03). Candidate rows never collapse into an unqualified
`CALLS` edge (`FAB §73.4`).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_call_target_enrichment_conformance`; Executable oracle: `py_call_target_partition_parity`; Executable oracle: `py_dynamic_dispatch_unknown_remainder_falsification`; Executable oracle: `py_call_target_ingest_operational_gate`.

- **Behavioral — Executable oracle:** `py_call_target_enrichment_conformance` —
  the `GEN §93.1` dispatch fixtures (bound/class/static methods, constructors,
  callable objects, decorators, unions, overloads) match canonical target rows.
- **Structural — Executable oracle:** `py_call_target_partition_parity` — every
  call site's rows partition cleanly (exact/sound-may/modelled/unknown); the
  `call_target_detail` PK holds; `resolved_target_count`/`resolution_status`
  on the call site equal the row set.
- **Negative/Zero-State — Executable oracle:**
  `py_dynamic_dispatch_unknown_remainder_falsification` — a union receiver with
  one dynamic arm keeps `UNKNOWN_CALL_TARGET` alongside exact candidates;
  zero-callee responses yield explicit unknowns, never absent rows
  (`pyrefly §11.5`).
- **Operational — Executable oracle:** `py_call_target_ingest_operational_gate`
  — call-target ingest under owner replacement stays consistent with call-site
  rows across an incremental edit that changes a target's signature.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, `just sidecar-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP13`.

**Integration Milestone.** M02.

**Replan Triggers.** Pinned-source panic branches in callee extraction fire on
the fixture corpus beyond supervision tolerance (`pyrefly §11.6`) — sidecar
hardening is adaptation; a Pyrefly source patch or query redesign is a plan
revision.

**Rollback or Recovery.** Additive; capability-gated.

### WP14 — Python reconciliation closure and `PYTHON_SEMANTIC_V1` COMPLETE

**Outcome.** The Python lane is closed: Ruff/Pyrefly reconciliation follows the
authority tables with retained evidence and conflict diagnostics, the semantic
lane runs in the `LIFE §95` order with dependency-driven invalidation and the
sidecar-failure semantics of `LIFE §7.3`–`§7.6`, incremental Python scenarios
compare equal to a clean rebuild, multiple Python contexts stay partitioned,
and `PYTHON_SEMANTIC_V1` reports `COMPLETE` for the selected corpus/context
through the formal aggregation algebra. M02 closes on
`just wave9-integration-check`.

**Dependencies.** WP10, WP11, WP12, WP13.

**Target invariants.** GI-04, GI-05, GI-06, GI-10, GI-12; DF-P3, DF-P10, DF-P19, DF-P20,
DF-P25.

**Design and library references.** `GEN §5.1`/`§5.3` authority and conflict
policy; `GEN §80`–`§83` reconciliation algorithms (Python instantiation);
`GEN §86` output boundary; `LIFE §95`, `§7.1`–`§7.6`; `LIFE §137`/
`SUITE AC-G-79` comparator; `ONT AC-G-72` profile definition; `RM §14` exit
evidence; `GEN §13.1`–`§13.2` identity recipes.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'reconcile_pyrefly_run|reconcile_candidates|authority|precedence' src/core_facts.rs src/fact_ingest.rs
rg -n 'PYTHON_SEMANTIC' contracts/registry/capability-registry.yaml src/
just spec-outline docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md --match '^95\.'
```

Known current touch includes the Python-owned reconciliation, authority,
invalidation, capability, and comparator fragments compiled through WP01's frozen
ports, plus the `justfile` (`wave9-integration-check`). Shared
`src/core_facts.rs`, lifecycle/continuous, and serving files are not edited here.

**Required changes.**

1. Instantiate the `GEN §80` range ladder and `GEN §81` declaration merge for
   the full Python fact surface: semantic facts attach only through the
   five-step ladder; Ruff–Pyrefly declaration disagreements keep the Ruff source
   declaration plus Pyrefly evidence and a conflict diagnostic; no duplicate
   canonical declarations except distinct source/stub entities.
2. Enforce the semantic lane order (Ruff parse → local scopes/bindings →
   Pyrefly module refresh → type/member/call reconciliation → CFG/dataflow) with
   dependency-driven invalidation from the sidecar's rechecked-modules surface
   and the operational dependency graph; summary-hash propagation stays inert
   (no summaries yet).
3. Prove the failure matrix: sidecar unavailable retains local facts and
   withdraws project semantics without negative claims (`LIFE §7.4`); type
   errors with valid parse keep syntax plus valid Pyrefly facts (`LIFE §7.3`);
   stub/dependency changes invalidate dependents (`LIFE §7.6`).
4. Extend the sixteen-scenario comparator corpus with Python semantic scenarios
   (body edit, signature change, import change, stub change, sidecar outage,
   context change) and prove incremental-equals-rebuild for every Wave 8/9
   family under AC-G-79 bag equality.
5. Close `PYTHON_SEMANTIC_V1`: every mandatory capability `COMPLETE` for every
   applicable owner in the selected corpus/context; multi-context fixtures
   (two Python versions of one workspace) never merge exact facts.
6. Populate `just wave9-integration-check`.

**Legacy Disposition and Decommission.** DB03's typed-observation zero state is
proven here (no opaque semantic JSON blob remains an ingest input). The Wave 8
`PARTIAL` advertisement flips through aggregation, not through edited status
strings.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `py_incremental_rebuild_equivalence_conformance`; Executable oracle: `py_semantic_profile_complete_parity`; Executable oracle: `py_sidecar_failure_visibility_falsification`; Executable oracle: `wave9_integration_operational_gate`.

- **Behavioral — Executable oracle:**
  `py_incremental_rebuild_equivalence_conformance` — every Python incremental
  scenario's terminal checkpoint compares equal (schema-fingerprint-first,
  duplicate-sensitive bag equality) to the independent zero-generation rebuild.
- **Structural — Executable oracle:** `py_semantic_profile_complete_parity` —
  `PYTHON_SEMANTIC_V1 = COMPLETE` derives from the aggregation algebra with
  zero uncharacterized children; the per-family authority tables in generated
  registries match `GEN §5.1` exactly.
- **Negative/Zero-State — Executable oracle:**
  `py_sidecar_failure_visibility_falsification` — killing the sidecar
  mid-corpus never leaves an invalidated prior semantic row visible in any
  active snapshot, and never produces a negative claim; two Python contexts
  never share a context-dependent fact ID.
- **Operational — Executable oracle:** `wave9_integration_operational_gate` —
  `just wave9-integration-check` passes: corpus → continuous waves → sidecar
  enrichment → reconciliation → publication → serving, with the failure matrix
  and comparator scenarios green.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted lifecycle
tests.

**Packet-Local Gates.** `just root-ci-fast`, `just sidecar-ci-fast`,
`just wave9-integration-check`, `just rebuild-equivalence-check`,
`just packet-oracle-check WP14`.

**Integration Milestone.** M02.

**Replan Triggers.** Incremental-equals-rebuild fails structurally for a family
(not a bug but an identity/ordering design flaw) — design reopening; the
selected conformance corpus cannot reach `COMPLETE` because a mandatory
capability has no provider path — profile-scope decision returns to the
ontology owner.

**Rollback or Recovery.** The profile advertisement is aggregation-derived;
any regression demotes automatically. Canonical rows re-derive from retained
evidence plus re-run providers.

### Wave 10 group — Rust compiler/MIR semantic core (`RM §15`)

### WP15 — Rust context and build discovery

**Outcome.** Rust analysis contexts are discovered, immutable manifests: a
pinned `cargo metadata` capture (never executing build scripts or proc macros at
discovery) yields the workspace/package/target/feature graph, toolchain
identity, and configuration digests; the `GEN AC-G-14` Rust manifest fields
enter the CBEF preimage; the default local profile selects lib/bin/proc-macro
targets under default features; manifest/lock/toolchain changes invalidate
exactly the dependent compiler-semantic families.

**Dependencies.** WP01. Rust context discovery consumes the lane-neutral
discovery port and is independent of the Python context packet.

**Target invariants.** GI-05, GI-09; DF-P1, DF-P3, DF-P18.

**Design and library references.** `GEN AC-G-34` (Rust discovery, no execution,
`ConfigurationDependencySet`); `GEN AC-G-14` (Rust manifest fields:
package/target/kind/features/cfg/triple/toolchain + extractor digest);
`LIFE §8.8`–`§8.9` invalidation; SUITE AC-G-07 toolchain bundle;
`src/analysis_context.rs` (Rust kind exists, discovery absent).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'cargo metadata|cargo_metadata_digest|target_kind|feature_set' src rustc-extractor/src contracts -g '!**/generated/**'
rg -n 'AnalysisContextKind::Rust' src/
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^AC-G-(14|34)'
```

Known current touch includes a Rust-owned discovery adapter and
context/diagnostic model fragments, plus `rustc-extractor/src/wrapper.rs` (digest
consumers — today the wrapper echoes daemon-derived digests; discovery must
produce them). It consumes the frozen WP01 context/operational-writer ports.

**Required changes.**

1. Run the pinned `cargo metadata` command per context selection (manifest,
   triple, feature profile, lock policy); capture and hash Cargo.toml/lock/
   config, rust-toolchain files, workspace/package/target graph, feature graph,
   build.rs and proc-macro identities, OUT_DIR roots, and dependency edges;
   emit the `ConfigurationDependencySet`.
2. Populate the Rust context manifest (cargo_workspace_root, package_id,
   target_name/kind, sorted feature_set, no_default_features, sorted cfg_set,
   target_triple, exact rustc version+commit, `rustc_extractor_digest`,
   metadata/lock/config digests, optional build-output digests); every
   compatibility-sensitive field in the CBEF preimage; default Rust context
   designation per snapshot.
3. Wire `LIFE §8.8`–`§8.9`: manifest/lock/feature/toolchain change invalidates
   the affected crate graph and compiler-semantic capabilities while preserving
   source/syntax; topology changes recompute target/module ownership.
4. Unknown or conflicting configuration is a terminal context diagnostic —
   never a guess.

**Legacy Disposition and Decommission.** The Gate B vertical's fixed context
digests are replaced by discovery output (DB01 negative proof covers the
residue).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_context_discovery_conformance`; Executable oracle: `rust_context_manifest_identity_parity`; Executable oracle: `rust_config_conflict_rejection_falsification`; Executable oracle: `rust_context_invalidation_gate`.

- **Behavioral — Executable oracle:** `rust_context_discovery_conformance` —
  fixture workspaces (single package, workspace with features, proc-macro
  crate, custom cfg) yield exact manifests and dependency sets.
- **Structural — Executable oracle:** `rust_context_manifest_identity_parity` —
  every compatibility-sensitive field participates in the context ID; equal
  manifests hash equal; the extractor digest and toolchain identity match the
  SUITE AC-G-07 bundle.
- **Negative/Zero-State — Executable oracle:**
  `rust_config_conflict_rejection_falsification` — conflicting toolchain files
  and unresolvable configuration produce terminal diagnostics; discovery
  provably executes no build script or proc macro (sandbox/spawn probe).
- **Operational — Executable oracle:** `rust_context_invalidation_gate` — a
  Cargo.lock change invalidates compiler-semantic capabilities for affected
  units, preserves source/syntax currency, and reruns metadata into a new
  context ID.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, discovery unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP15`.

**Integration Milestone.** M03.

**Replan Triggers.** A required manifest field cannot be derived without
executing user build code — return to the spec; never weaken the no-execution
rule.

**Rollback or Recovery.** Additive; contexts are immutable records.

### WP16 — Extractor process, sandbox containment, and job integration

**Outcome.** The rustc extractor runs as a production sandboxed provider: the
`RUSTC_WORKSPACE_WRAPPER` launch path with sccache disabled and a
sandbox-private target dir, handshake negotiation that fails on
toolchain/schema/sandbox/resource mismatch before analysis, `AC-G-32` job
integration (admission, supersession, 10 s cancel-to-kill), all inputs resolved
from the immutable `ProviderWorkspaceView` and `DependencyInputBundle`, and —
decisively — the `GEN AC-G-35` platform containment contract proven, ending the
Wave 5 `TRUSTED_LOCAL` golden-fixture-only exception: arbitrary registered
repositories execute `UNTRUSTED_SANDBOXED` or not at all.

**Dependencies.** WP15, WP36.

**Target invariants.** GI-03, GI-09, GI-11, GI-13, GI-15; DF-P9, DF-P20, DF-P23;
H-P8, H-P22, H-P23, H-P28.

**Design and library references.** `GEN AC-G-31` (wrapper env contract, launch
path, handshake, cancellation authority); `GEN AC-G-32` (compiler process
group, resource profiles); `GEN AC-G-33` (workspace view for Cargo/rustc/build
scripts/proc macros); `GEN AC-G-35` (containment or unavailable); `RM §27.4`
security progression; `scripts/run_rustc_extractor.sh`;
`rustc-extractor/src/wrapper.rs`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'RUSTC_WORKSPACE_WRAPPER|RUSTC_WRAPPER|sccache|target_dir' src scripts rustc-extractor/src -g '!**/generated/**'
rg -n 'TRUSTED_LOCAL|UNTRUSTED_SANDBOXED|sandbox_profile' src contracts src/gate_b_candidate*
ast-grep outline rustc-extractor/src/wrapper.rs
```

Known current touch includes `rustc-extractor/src/wrapper.rs`,
`scripts/run_rustc_extractor.sh`, `src/rustc_service.rs`, the WP01
`ProviderAdapter` for rustc, `src/gate_b_candidate/vertical.rs` (cutover to the
production path — DB01 Rust half), the provider/resource-profile registries,
and the sandbox profile machinery shared with WP08.

**Required changes.**

1. Launch pinned Cargo with the CodeFabric wrapper via
   `RUSTC_WORKSPACE_WRAPPER`; disable the outer sccache wrapper inside provider
   analysis and use a sandbox-private target dir so cache hits cannot skip
   extraction; probe/version invocations pass through untouched; compiler
   stdout/stderr/exit status preserved.
2. Enforce the handshake: extractor build, exact rustc version/commit,
   toolchain-identity digest, resource profile against the daemon's accepted
   schema bundle, sandbox profile, limits, deadline — mismatch fails before
   compiler analysis; the daemon `CancelCompilationRequest` is the single
   cancellation authority with the 10 s process-group kill.
3. Resolve every compiler input from the immutable `ProviderWorkspaceView`
   (Cargo, rustc, build scripts, proc macros) with the context-pinned
   `DependencyInputBundle`; no network; separate writable output tree
   (consumed by WP26 for AC-G-40 capture).
4. Prove LD-SB-01 containment per `GEN AC-G-35` on every advertised Darwin/Linux
   host, including network, credential/live-workspace read, out-of-root write,
   FD, build-script/proc-macro child tree, resource, cancellation, and cleanup
   escapes. Pin sandbox profile digest + trust profile into runs and snapshots;
   when containment cannot be established, untrusted compiler execution is
   unavailable (GI-13).
5. Route execution through `ProviderRuntime` with supersession keys and
   registry-resolved resource profiles; prove the WP36-converted Gate B and
   `src/fabric/serving.rs` consumers contain no Rust direct-spawn path without
   editing those shared files (DB01).

**Legacy Disposition and Decommission.** The Wave 5 `TRUSTED_LOCAL`-only
security posture ends: `UNTRUSTED_SANDBOXED` becomes the default for arbitrary
registered repositories, and the golden-fixture grant remains only as the
explicit narrow exception it was designed to be. DB01 owns the
direct-invocation zero state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rustc_process_contract_conformance`; Executable oracle: `rustc_provider_runtime_parity`; Executable oracle: `rustc_unsandboxed_execution_falsification`; Executable oracle: `rustc_sandbox_containment_operational_gate`.

- **Behavioral — Executable oracle:** `rustc_process_contract_conformance` — a
  fixture crate compiles through the full launch path (wrapper env echo,
  handshake, observation stream, passthrough correctness: the crate's real
  build artifacts are byte-identical to a wrapperless build).
- **Structural — Executable oracle:** `rustc_provider_runtime_parity` — every
  extractor run flows through `ProviderRuntime` admission/journal with a
  supersession key; the wrapper only echoes daemon-derived env values (no
  rediscovery); sccache is provably disabled inside the sandbox.
- **Negative/Zero-State — Executable oracle:**
  `rustc_unsandboxed_execution_falsification` — with containment forced
  unavailable, an arbitrary-repository semantic run reports
  unavailable-not-unsandboxed; a handshake with a wrong toolchain digest fails
  before any compiler analysis; network and workspace-write probes from inside
  the sandbox fail.
- **Operational — Executable oracle:**
  `rustc_sandbox_containment_operational_gate` — cancel-to-kill within 10 s
  including process-group descendants (build scripts); deadline overrun
  terminates the group; the run journal records the terminal state.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`,
`just root-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just provider-protocol-check`, `just semantic-sandbox-host-matrix-check`,
`just semantic-provider-legacy-zero-state-check`, `just packet-oracle-check WP16`.

**Integration Milestone.** M03.

**Replan Triggers.** Seatbelt/namespace containment cannot express a required
compiler behavior (proc-macro execution needs) — the trust-profile decision
escalates to the spec's `PARSING_ONLY`/`TRUSTED_LOCAL` ladder rather than
weakening the sandbox; wrapper-based launch conflicts with a Cargo behavior at
the pinned toolchain — protocol change is a plan revision.

**Rollback or Recovery.** Capability-gated; disabling the adapter restores the
prior posture. The sandbox profile digest pins make a rollback visible in
provenance.

### WP17 — Invocation-manifest acceptance protocol

**Outcome.** Semantic compiler output is accepted only through the complete
`AC-G-31` manifest grammar: `CompilationAccepted → CompilationBegin →
(OwnerBegin → chunks → OwnerEnd)* → CompilationEnd` with every digest field;
unexpected owners, duplicate sequences, missing end records, count mismatches,
stale source/context digests, and protocol EOF reject the run as
`PROTOCOL_ERROR`; no owner facts are publishable without a valid manifest;
backpressure and deadlines hold; run states use only the shared
`ProviderRunState` registry.

**Dependencies.** WP16.

**Target invariants.** GI-03, GI-04, GI-09, GI-12; DF-P9, DF-P16.

**Design and library references.** `GEN AC-G-31` acceptance rules 2/3/5 and
event fields; `LIFE §96.1` compiler-manifest rule; `LIFE §8.10` protocol
failure; `GEN §85` run-state vocabulary; `src/rustc_service.rs` (partial
implementation present: digests, credits, accepted compilation).

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/rustc_service.rs
rg -n 'PROTOCOL_ERROR|OwnerEnd|CompilationEnd|closed_owner|stale' src/rustc_service.rs contracts/rpc/rustc_extractor.proto
```

Known current touch includes `src/rustc_service.rs` (acceptance completion),
`rustc-extractor/src/wrapper.rs` (event emission), the proto/feature registry
if any field is missing, and the Rust-owned reconciliation fragment (accepts only
closed manifests through WP01's frozen dispatch).

**Required changes.**

1. Complete the acceptance taxonomy: semantic output accepted only when
   `CompilationEnd` reports success and every referenced owner is closed;
   enumerate and test every rejection class (unexpected owner, duplicate
   sequence, missing end record, family-count mismatch, stale
   source/context/generation digest, EOF, deadline).
2. Enforce backpressure (max 4 outstanding chunks / 16 MiB unacknowledged per
   compilation) symmetrically in wrapper and daemon; deadline overrun
   terminates the process group and rejects the run.
3. Bind owner keys, owner content digests, closed-owner-set digest, and stream
   digest into the acceptance decision; `compilation_unit_id` and invocation
   digests remain run/evidence identity, never fact identity.
4. Partial-compilation acceptance stays disabled (a future protocol feature per
   AC-G-31 rule 4); assert its absence.

**Legacy Disposition and Decommission.** None — completes a live surface. Any
Gate-B-era acceptance shortcut in the vertical harness routes through this
service (DB01).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rustc_manifest_acceptance_conformance`; Executable oracle: `rustc_owner_closure_parity`; Executable oracle: `rustc_partial_output_rejection_falsification`; Executable oracle: `rustc_protocol_failure_operational_gate`.

- **Behavioral — Executable oracle:** `rustc_manifest_acceptance_conformance` —
  a well-formed multi-owner compilation is accepted with exact owner digests;
  every event field of AC-G-31 round-trips.
- **Structural — Executable oracle:** `rustc_owner_closure_parity` — accepted
  runs have bijective OwnerBegin/OwnerEnd pairs, matching family counts, and a
  closed-owner-set digest equal to the recomputed set.
- **Negative/Zero-State — Executable oracle:**
  `rustc_partial_output_rejection_falsification` — each rejection class fires
  on its crafted stream (including kill-mid-owner and stale-generation) and no
  staged row reaches ingest; partial-compilation acceptance is provably
  disabled.
- **Operational — Executable oracle:** `rustc_protocol_failure_operational_gate`
  — after a rejected run the provider journal shows the registered
  `ProviderRunState`, the prior snapshot stays active, and a rerun succeeds
  cleanly.

**Edit-Local Gates.** `just root-fmt`, `just root-check`,
`just extractor-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just extractor-ci-fast`,
`just provider-protocol-check`, `just packet-oracle-check WP17`.

**Integration Milestone.** M03.

**Replan Triggers.** A required manifest field cannot be computed at the
wrapper (compiler gives no access) — spec gap on AC-G-31, return it.

**Rollback or Recovery.** Acceptance hardening is monotone; a stricter daemon
rejects, never corrupts.

### WP18 — Rust definitions and type semantics

**Outcome.** Valid crates yield exact semantic definitions and type facts:
local and referenced-external items in the `ONT §44` entity vocabulary with
Tree-sitter correspondence (one macro invocation ↔ many definitions),
recursive `Ty/TyKind` normalization into the canonical algebra, type edges,
the trait/impl graph, and coercions/adjustments only from compiler facts —
absent compiler exposure yields capability status, never source-text
reconstruction. The canonical type interner and language-neutral fact tables
come from WP01; Rust and Python populate that single authority independently.

**Dependencies.** WP17.

**Target invariants.** GI-04, GI-06; DF-P1, DF-P3; identity: application-owned keys,
no session-local `DefId` persistence.

**Design and library references.** `GEN §36` semantic definitions; `GEN §37`
types/generics/coercions; `ONT §44`–`§47`; `GEN §13.4` identity; `FAB §21`,
`§26`–`§28` (shared type tables); LD-RS-01 (`mir §7`, `§16`); `GEN §94`
differential checks.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline rustc-extractor/src/rustc_link.rs
rg -n 'all_local_items|TyKind|IMPLEMENTS_TRAIT' rustc-extractor/src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(36|37)\.'
```

Known current touch includes `rustc-extractor/src/rustc_link.rs` (from summary
to full extraction), the observation schema family (new streams via Contract
IR), the Rust-owned reconciliation/ontology fragments, and the shared
`TypeInterner`/common type tables from WP01 (language-neutral algebra).

**Required changes.**

1. Enumerate local + referenced-external items inside `rustc_public::run!`,
   copy to owned DTO records (no compiler type escapes), and emit the
   CRATE…FOREIGN_FUNCTION entity kinds with declaration properties
   (`ONT §45`).
2. Map rustc spans to Tree-sitter declarations
   (`SOURCE_SYNTAX —CORRESPONDS_TO→ SEMANTIC_DEFINITION`), accepting 1:N for
   macro-generated definitions.
3. Normalize `Ty/TyKind` recursively into the canonical type algebra (shared
   interner, Rust profile extensions `ONT §47`); emit TYPE_OF…OUTLIVES edges
   and the trait/impl graph (IMPLEMENTS_TRAIT, IMPLEMENTS_METHOD, SUPERTRAIT,
   ASSOCIATED_WITH).
4. Emit coercions/adjustments (AUTO_DEREF_TO…REIFIES_FN_POINTER) only where
   the compiler exposes them; otherwise report the capability gap (WP24 may
   later fill via the private adapter).
5. Identity: application-owned canonical keys (admitted package/target/crate +
   semantic name + context) until WP24 upgrades to `DefPathHash`; run the
   `GEN §94` Tree-sitter-items-vs-rustc-definitions differential.

**Legacy Disposition and Decommission.** The shallow `OwnedMirItem`
name/summary extraction is superseded by the structured definition/type
streams; the Gate-B-era `CALLABLE`+`NAME`-only canonicalization in
`reconcile_rustc_compilation` is replaced by full-family reconciliation (no
alias retains the old shape — DB03 covers the evidence-blob analogue).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_definition_type_fixture_conformance`; Executable oracle: `rust_semantic_identity_parity`; Executable oracle: `rust_adjustment_capability_gap_falsification`; Executable oracle: `rust_definition_replacement_gate`.

- **Behavioral — Executable oracle:**
  `rust_definition_type_fixture_conformance` — the `GEN §93.2` item/trait/
  impl/generic fixtures produce exact definition and type rows including the
  Tree-sitter correspondence set.
- **Structural — Executable oracle:** `rust_semantic_identity_parity` —
  equivalent source in independent sandboxes yields equal canonical owners and
  fact IDs; no session-local `DefId`/`CrateNum`/pointer value appears in any
  persisted column; the type interner produces equal `type_id`s across runs.
- **Negative/Zero-State — Executable oracle:**
  `rust_adjustment_capability_gap_falsification` — a coercion family the
  public API does not expose reports an explicit capability gap; no code path
  reconstructs adjustments from source text.
- **Operational — Executable oracle:** `rust_definition_replacement_gate` — an
  item edit replaces exactly the owning crate/module owner's definition/type
  rows under owner replacement.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`,
`just root-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP18`.

**Integration Milestone.** M03.

**Replan Triggers.** The pinned nightly's public API lacks a required
type/generic surface the reference asserts (drift between reference and
toolchain) — probe first, then either the WP24 private adapter absorbs it or
the gap returns to the spec.

**Rollback or Recovery.** Additive; capability-gated.

### WP19 — Rust MIR core

**Outcome.** Full MIR bodies are canonical facts: MIR_BODY/LOCAL/BASIC_BLOCK/
STATEMENT/TERMINATOR/OPERAND/RVALUE/PLACE/PLACE_PROJECTION with
`LOWERS_TO` edges from semantic callables/consts/statics, classified locals
(return/argument/user/temporary/capture) with debug-name enrichment, raw
provider-native variants retained alongside normalized meaning, and structured
places (base local + projection chain) — never serialized strings. The
`rust_mir_body`/`rust_mir_local` tables land with `mir_fingerprint`.

**Dependencies.** WP17, WP18.

**Target invariants.** GI-04; DF-P1, DF-P7; `ONT §48.3` raw+normalized coexistence.

**Design and library references.** `GEN §38` MIR-body generation; `ONT §48`–
`§49`; `FAB §51`–`§52`; LD-RS-01 (`mir §8`, `§11`, `§13`–`§15`, `§18.0`);
LD-AR-01 encoder policy.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'basic_block_count|statement_kinds|OwnedMirItem' rustc-extractor/src
jq -r '.tables[].name' contracts/generated/model/schema/table-specs.json | rg 'mir'
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^(51|52)\.'
```

Known current touch includes `rustc-extractor/src/rustc_link.rs` (full
`MirVisitor` extraction), the MIR observation schemas (Contract IR), the
Contract IR tables (`rust_mir_body`, `rust_mir_local`), ontology registries,
and ingest.

**Required changes.**

1. Extract full bodies with `MirVisitor`: ordered locals with classification
   and debug-variable naming (compiler-local ordinals retained only under
   owner scope), blocks with statements/terminators (raw kind + normalized
   kind + span + cleanup flags + successor ordinals), operands with
   move/copy/constant distinction, rvalues over the full surface, and places
   as base_local + projection vectors (`DEREF`/`FIELD`/`INDEX`/
   `CONSTANT_INDEX`/`SUBSLICE`/`DOWNCAST`/`OPAQUE_CAST`).
2. Emit `semantic callable/const/static —LOWERS_TO→ MIR_BODY` and per-body
   `mir_fingerprint` for WP28's owner-fingerprint comparison.
3. Preserve raw variants alongside normalized kinds (`ONT §48.3`); source
   correspondence per span where available; every MIR entity attributable to
   its body and source-level owner.
4. Encode at FAB §64 narrow-table batch sizes; projections and operands are
   row-oriented relation rows.

**Legacy Disposition and Decommission.** The stringified
`statement_kinds`/`terminator_kinds` summary lists are superseded; the MIR
cold-evidence blob shape retires with DB03.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_mir_body_fixture_conformance`; Executable oracle: `rust_mir_raw_normalized_parity`; Executable oracle: `rust_mir_index_identity_falsification`; Executable oracle: `rust_mir_ingest_operational_gate`.

- **Behavioral — Executable oracle:** `rust_mir_body_fixture_conformance` — the
  `GEN §93.2` MIR fixtures (every statement/terminator/rvalue variant, all
  projection kinds) produce exact body/local/statement/terminator/place rows.
- **Structural — Executable oracle:** `rust_mir_raw_normalized_parity` — every
  normalized row retains its raw provider-native kind; place rows are
  structured components with ordinals, never strings; every MIR row keys to an
  existing body row.
- **Negative/Zero-State — Executable oracle:**
  `rust_mir_index_identity_falsification` — MIR local/block ordinals appear
  only under owner scope; no canonical fact ID preimage contains a bare MIR
  index; an unsupported MIR variant yields an explicit diagnostic plus
  `UNKNOWN_*` fact, never silent omission.
- **Operational — Executable oracle:** `rust_mir_ingest_operational_gate` —
  MIR ingest for a mid-size crate stays within the resource profile at FAB §64
  batch sizes and replaces owners atomically.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just model-repro-check`, `just packet-oracle-check WP19`.

**Integration Milestone.** M03.

**Replan Triggers.** Observation volume for full MIR exceeds the transport or
storage envelope on the reference corpus — batch/family repartitioning is
adaptation; dropping a required MIR family is a spec decision.

**Rollback or Recovery.** Additive; capability-gated.

### WP20 — Rust CFG and call facts

**Outcome.** MIR is the authoritative CFG: the `GEN §39` terminator→edge table
with normal/unwind/drop/assert edges distinct; every call terminator yields a
first-class call site with callable operand, arguments, destination, declared
FnDef target, `Instance::resolve` outcome, and normal+unwind successors; edges
CALLS_DECLARATION/CALLS_EXACT_TARGET/CALLS_INSTANCE/MAY_CALL/CALLS_UNKNOWN;
function references and closures resolve; unknown-origin function pointers emit
`UNKNOWN_CALL_TARGET`.

**Dependencies.** WP19.

**Target invariants.** GI-04, GI-06; DF-P20; doctrine: unwind never collapses into
normal flow.

**Design and library references.** `GEN §39` CFG; `GEN §41.1`–`§41.4` calls,
references, fn-pointer locals, closures; `ONT §15`, `§52.1`; `FAB §34`–`§36`
(shared CFG tables), `§31`–`§33` (shared call tables); LD-RS-01 (`mir §10`,
`§12.3`, `§20`–`§21`); `GEN §94` MIR-CFG differential.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'TerminatorKind|UnwindAction|Instance::resolve' rustc-extractor/src
rg -n 'CFG_CALL_RETURN|CALLS_INSTANCE|MAY_CALL' contracts/registry src -g '!**/generated/**'
```

Known current touch includes `rustc-extractor/src/rustc_link.rs`, the CFG/call
observation schemas, ingest, and the shared `cfg_*`/`call_*` tables from
WP05/WP06 (language-neutral, Rust rows added).

**Required changes.**

1. Emit CFG edges from the terminator table: Goto→CFG_NEXT,
   SwitchInt→CFG_CASE/TRUE/FALSE, Call→CFG_CALL_RETURN + unwind, Drop/Assert
   with unwind, Resume/Abort/Unreachable, InlineAsm; statement-level expansion
   stays derived/query-time.
2. Per call terminator: call-site node, callable operand, arguments,
   destination, declared FnDef, `Instance::resolve` (exact instance when
   resolvable), normal and unwind successors; emit the five-edge taxonomy with
   certainty codes.
3. Function references (REFERENCES_CALLABLE…), closure resolution via
   `Instance::resolve_closure`, and the bounded intraprocedural fn-pointer
   propagation of `GEN §41.4` — unknown origins produce `UNKNOWN_CALL_TARGET`.
4. Run the `GEN §94` differentials: source calls vs MIR call-site
   correspondence; MIR CFG vs emitted CFG projection.

**Legacy Disposition and Decommission.** None — new families on shared tables.
Monomorphized instance completion is WP25 (`GEN §41.5` explicitly deferred).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_cfg_call_fixture_conformance`; Executable oracle: `rust_unwind_edge_distinction_parity`; Executable oracle: `rust_fn_pointer_unknown_falsification`; Executable oracle: `rust_call_ingest_operational_gate`.

- **Behavioral — Executable oracle:** `rust_cfg_call_fixture_conformance` — the
  `GEN §93.2` CFG/call fixtures (switches, drops, asserts, panics, closures,
  fn pointers) produce exact edge and call rows including both differentials.
- **Structural — Executable oracle:** `rust_unwind_edge_distinction_parity` —
  no unwind edge shares a kind with normal flow; every call site anchors its
  target rows; CALLS_DECLARATION and CALLS_INSTANCE are never conflated.
- **Negative/Zero-State — Executable oracle:**
  `rust_fn_pointer_unknown_falsification` — an unknown-origin fn pointer call
  yields `UNKNOWN_CALL_TARGET` alongside any propagated candidates; deleting
  the unknown row fails the check.
- **Operational — Executable oracle:** `rust_call_ingest_operational_gate` —
  CFG/call ingest replaces owners atomically and the emitted CFG projection
  matches MIR topology on re-extraction (idempotence).

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just packet-oracle-check WP20`.

**Integration Milestone.** M03.

**Replan Triggers.** `Instance::resolve` coverage at the pinned nightly
diverges from the reference's claims on the fixture corpus — degrade to
declared targets with explicit unknowns (adaptation) unless a required exact
family is lost (spec decision).

**Rollback or Recovery.** Additive; capability-gated.

### WP21 — Rust compile-failure semantics and `RUST_SEMANTIC_V1` PARTIAL

**Outcome.** Compiler failure never corrupts present state: syntax/type/
borrow/crate failures publish current source and syntax plus explicit compiler
capability gaps; invalidated owners become semantically unavailable with no
fresh compiler generation; unchanged owners stay current only under proven
dependency validity; the hidden last-known-good cache is never present-state
truth; query responses expose the `LIFE §96.3` degradation fields; the Rust
explicit-unknown table (`GEN §51`) is complete; and `RUST_SEMANTIC_V1`
advertises `PARTIAL` with ownership/lowering capabilities identified. M03
closes on `just wave10-integration-check`.

**Dependencies.** WP18, WP20.

**Target invariants.** GI-03, GI-06, GI-12; DF-P20, DF-P23.

**Design and library references.** `LIFE §96.2` partial-compilation policy;
`LIFE §8.2`–`§8.5`, `§8.10`; `LIFE §96.3` query fallback; `GEN §51` explicit
unknowns (traceability-corrected into this wave); `GEN §85`; `ONT AC-G-72`;
`RM §15` exit evidence.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'UNAVAILABLE_COMPILE|last-known-good|last_known_good' src contracts/registry -g '!**/generated/**'
rg -n 'RUST_SEMANTIC' contracts/registry/capability-registry.yaml src/
just spec-outline docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md --match '^96\.'
```

Known current touch includes Rust-owned failure/invalidation/capability fragments,
`src/semantic_query.rs` (degradation fields), and the `justfile`; the frozen
continuous/lifecycle/core dispatch is not edited here.

**Required changes.**

1. Implement the failure matrix: syntax error → source/CST current, compiler
   families withdrawn for invalidated owners; type/borrow error →
   source+syntax+diagnostics current, semantic/MIR unavailable; crate failure
   → never combine current syntax with old semantic facts for invalidated
   owners; build-script/proc-macro failure → units unavailable, generated
   facts withdrawn unless provably current.
2. Constrain retention: unchanged owners stay current only when dependency
   validity is established via the operational dependency graph; any
   last-known-good compiler rows live in a declared operational cache
   invisible to serving snapshots.
3. Surface `LIFE §96.3` in query responses: availability/completeness
   `UNAVAILABLE`, freshness `CURRENT`, owner_capability_state
   `UNAVAILABLE_COMPILE`/`UNAVAILABLE_PROVIDER`/`PENDING`, reason code, current
   source location, compiler diagnostics.
4. Complete `GEN §51` unknowns (indirect fn-pointer targets, dyn remainder,
   bodiless externals, opaque FFI, unmapped macro spans, unsupported variants,
   unavailable borrowck) — each with a producer and fixture.
5. Advertise `RUST_SEMANTIC_V1 = PARTIAL` through aggregation with the Wave 11
   mandatory capabilities (ownership, lowering correspondence, instances)
   explicitly missing; populate `just wave10-integration-check` including
   crash/timeout/stale-generation rejection scenarios.

**Legacy Disposition and Decommission.** None new; asserts the negative
posture that no failure path resurrects Gate-B-era shortcuts.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_compile_failure_semantics_conformance`; Executable oracle: `rust_semantic_profile_partial_parity`; Executable oracle: `rust_stale_semantic_visibility_falsification`; Executable oracle: `wave10_integration_operational_gate`.

- **Behavioral — Executable oracle:**
  `rust_compile_failure_semantics_conformance` — the four failure classes each
  produce the exact prescribed capability/withdrawal/diagnostic posture on the
  fixture corpus.
- **Structural — Executable oracle:** `rust_semantic_profile_partial_parity` —
  the `PARTIAL` advertisement derives from aggregation; the missing mandatory
  capabilities are named children; per-query owner/capability coverage is
  exposed as `ONT AC-G-72` requires.
- **Negative/Zero-State — Executable oracle:**
  `rust_stale_semantic_visibility_falsification` — after a compile break, no
  invalidated owner's prior semantic row is reachable through any active
  snapshot; the operational cache is provably outside the serving read path;
  break-then-fix converges to the clean rebuild.
- **Operational — Executable oracle:** `wave10_integration_operational_gate` —
  `just wave10-integration-check` passes: valid-crate extraction, all failure
  scenarios, crash/timeout/stale rejection, and the Rust subset of
  `just rebuild-equivalence-check`.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted lifecycle
tests.

**Packet-Local Gates.** `just root-ci-fast`, `just extractor-ci-fast`,
`just wave10-integration-check`, `just rebuild-equivalence-check`,
`just packet-oracle-check WP21`.

**Integration Milestone.** M03.

**Replan Triggers.** Dependency-validity proof for unchanged-owner retention
cannot be established from the operational graph (would force whole-context
invalidation on every failure) — precision escalation returns to `LIFE §96.2`'s
policy owner.

**Rollback or Recovery.** The failure semantics are themselves the recovery
contract; a defective packet build falls back to whole-owner invalidation
(sound, coarser).

### Wave 11 group — Rust ownership, lowering, and profile closure (`RM §16`)

### WP22 — Places, projections, and access events

**Outcome.** Every MIR construct normalizes to the canonical
`AccessEvent{owner, location, place, kind, type, span}` stream over structured
`PlaceKey`s: the full `GEN §40` mapping table (Copy→READ+COPY, Move→READ+MOVE,
Assign, Ref/Reborrow/AddressOf, call-destination writes, Drop,
SetDiscriminant, StorageLive/Dead, ThreadLocalRef), with MOVE and COPY never
collapsed and access classification derived from statements/rvalues/terminators
directly. The `memory_location_detail`/`access_path_component`/
`memory_access_detail` tables carry Rust rows; the event stream becomes the
canonical input for every later state analysis.

**Dependencies.** WP21.

**Target invariants.** GI-04; DF-P1, DF-P7; `ONT §50` distinctions.

**Design and library references.** `GEN §40` (traceability-corrected into this
wave); `ONT §49`–`§50`; `FAB §40`–`§42` (row-oriented access paths per
`FAB §65.4`); LD-RS-01 (`mir §26.0`–`§26.2`, `§18.2` PlaceContext coarseness,
`§14.1` structured keys).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'AccessEvent|PlaceKey|memory_access' src rustc-extractor/src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^40\.'
```

Known current touch includes `rustc-extractor/src/rustc_link.rs` (event
classification at extraction), the access-event observation schema, ingest, and
the shared memory/access tables from WP07 (Rust rows added).

**Required changes.**

1. Canonicalize `PlaceKey{owner, base_local, projections}` with the seven
   projection kinds as structured components — never printer strings.
2. Classify every MIR statement/rvalue/terminator into access events per the
   `GEN §40` table, deriving categories from the constructs directly (public
   `PlaceContext` is intentionally too coarse); keep READ/WRITE, COPY/MOVE,
   BORROW_SHARED/BORROW_MUT/REBORROW, RAW_ADDRESS_OF, STORAGE_LIVE/DEAD,
   INIT/DEINIT, DROP semantically distinct.
3. Emit the ordered event stream per owner with program-point locations
   (block/statement ordinals under owner scope) and place types.
4. Encode into the shared tables; access-path components stay row-oriented.

**Legacy Disposition and Decommission.** None — new family.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_access_event_fixture_conformance`; Executable oracle: `rust_place_key_structure_parity`; Executable oracle: `rust_move_copy_collapse_falsification`; Executable oracle: `rust_access_event_ingest_gate`.

- **Behavioral — Executable oracle:** `rust_access_event_fixture_conformance` —
  the `GEN §93.2` move/copy/borrow fixtures produce the exact ordered event
  stream per owner, including storage markers and discriminant writes.
- **Structural — Executable oracle:** `rust_place_key_structure_parity` — every
  place reference resolves to a structured key with ordinal projection
  components; identical places canonicalize identically across bodies.
- **Negative/Zero-State — Executable oracle:**
  `rust_move_copy_collapse_falsification` — a Copy-type read and a move of the
  same shape produce distinct event kinds; collapsing them (or dropping a
  storage event) fails the check.
- **Operational — Executable oracle:** `rust_access_event_ingest_gate` —
  event-stream ingest for the reference corpus stays in the resource profile,
  replaces owners atomically, and the per-owner event ordering is byte-stable
  across independent re-extractions.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just packet-oracle-check WP22`.

**Integration Milestone.** M04.

**Replan Triggers.** A MIR construct class resists sound event classification
at the pinned nightly — explicit `UNKNOWN_MEMORY`/unknown-effect posture is the
fallback; losing a required event kind is a spec decision.

**Rollback or Recovery.** Additive; capability-gated.

### WP23 — Ownership and initialization state

**Outcome.** Ownership-state facts derive from the access-event stream: base
facts (MOVED_TO/COPIED_TO/BORROWS_SHARED/BORROWS_MUTABLY/REBORROWS/DROPS), a
forward per-place lattice (UNINITIALIZED/INITIALIZED/MOVED/MAYBE_*) with the
`GEN §44.2` transfer rules, program-point facts (OWNED_AT/MOVED_AT/
UNINITIALIZED_AT), and the `rust_move_path` parent tree — with the `mir §29.2`
exactness boundary explicit: this is never a claim of compiler-equivalent
borrow safety.

**Dependencies.** WP22.

**Target invariants.** GI-04, GI-06; DF-P20; `ONT §51.1` program-point identity.

**Design and library references.** `GEN §44` move/init/ownership generation;
`ONT §21`, `§51`; `FAB §56` move-path tree; LD-RS-01 (`mir §29` — application
derivation, no public dataflow surface); exactness boundary `mir §29.2`,
`GEN §44.3`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'move_path|OWNED_AT|MAYBE_INITIALIZED' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^44\.'
```

Known current touch includes a new ownership-state solver in the stable root
(deriving from canonical access events — daemon-side, not extractor-side), the
Contract IR (`rust_move_path`), ontology registries, and ingest.

**Required changes.**

1. Derive base ownership facts from access events; build the move-path parent
   tree (`FAB §56`).
2. Run the forward per-place lattice with the `GEN §44.2` transfer rules
   (assignment initializes, move invalidates, storage transitions, drop
   consumes, call-destination init on normal return, joins form MAYBE_*) over
   the WP20 CFG.
3. Emit program-point facts preserving program-point identity; field-sensitive
   kill semantics per `GEN §45.2` (exact field kills subpaths, whole-base
   kills all, deref/index conservative).
4. State the exactness boundary in capability terms: without WP24 borrowck
   integration, loan liveness is conservative/absent and the capability record
   says so.

**Legacy Disposition and Decommission.** None — new family. Rust def-use/
liveness materialization and alias/points-to stay Wave 13 derivations
(GI-14).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_ownership_state_fixture_conformance`; Executable oracle: `rust_move_path_tree_parity`; Executable oracle: `rust_ownership_exactness_boundary_falsification`; Executable oracle: `rust_ownership_ingest_operational_gate`.

- **Behavioral — Executable oracle:**
  `rust_ownership_state_fixture_conformance` — the `GEN §93.2` ownership
  fixtures (moves in branches, partial moves, reinitialization, drops in
  loops) produce exact lattice states at every program point.
- **Structural — Executable oracle:** `rust_move_path_tree_parity` — the
  move-path tree is a well-formed parent forest; every program-point fact
  keys to a real CFG location; MAYBE_* states appear exactly at joins with
  divergent inputs.
- **Negative/Zero-State — Executable oracle:**
  `rust_ownership_exactness_boundary_falsification` — no fact or capability
  record claims exact borrow/loan liveness without the WP24 adapter; a
  crafted assertion of compiler-equivalence fails.
- **Operational — Executable oracle:**
  `rust_ownership_ingest_operational_gate` — ownership derivation is
  registered as the owner-local direct family it is (not a Wave 13
  derivation-registry family), re-runs deterministically, and replaces owners
  atomically.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, solver unit tests.

**Packet-Local Gates.** `just root-ci-fast`, `just packet-oracle-check WP23`.

**Integration Milestone.** M04.

**Replan Triggers.** The lattice cannot converge within the resource envelope
on real corpora — widening policy returns to the precision-profile design.

**Rollback or Recovery.** Additive; capability-gated.

### WP24 — Narrow `rustc_private` enrichment adapter

**Outcome.** The contained private adapter (extractor domain only) upgrades
exactly the facts `GEN §97.2` routes to it: stable identity
(`DefPathHash` + `StableCrateId` becoming the preferred canonical Rust
identity, with the application-key fallback retained), SourceMap/hygiene
byte-exact spans, borrowck loans/regions where required
(LOAN/LOAN_CREATED_AT/LOAN_LIVE_AT/REGION/OUTLIVES into
`rust_loan`/`rust_region`), and vtable layout — each with graceful capability
degradation when unavailable, an adapter digest pinned in the toolchain
bundle, and exhaustive variant tests so nightly drift breaks loudly in one
crate.

**Dependencies.** WP23. This continues the explicit serialization of Wave 11's
shared extractor/Contract-IR surface.

**Target invariants.** GI-06, GI-09, GI-13; DF-P18, DF-P20; repo invariant: no
compiler-private dependency enters the stable root.

**Design and library references.** LD-RS-02; `GEN §36.2`, `§37.4`, `§42.2`,
`§44.3`, `§97.2`; `ONT §64.5` identity; `FAB §54`–`§55`; `mir §37`, App. P;
SUITE AC-G-07, `§83.6` upgrade differential.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'rustc_private|run_with_tcx|DefPathHash' rustc-extractor/src
rg -n 'toolchain_identity|extractor_digest' contracts src -g '!**/generated/**'
```

Known current touch includes `rustc-extractor/src/` (new private-adapter
module beside `rustc_link.rs`), the observation schemas (identity/loan/region
streams), the Contract IR (`rust_loan`, `rust_region`), the toolchain bundle
(adapter digest), and reconciliation (identity upgrade).

**Required changes.**

1. Contain `run_with_tcx!` usage in one adapter module per `mir App. P.7`;
   no private type crosses the DTO boundary; the stable root never links it.
2. Emit stable identity: `DefPathHash` + stable crate identity become the
   preferred canonical key preimage for Rust named entities; the WP18
   application key remains the documented fallback; both recipes are
   registry-recorded so IDs are reproducible either way.
3. Emit byte-exact span/file identity and hygiene/expansion provenance for
   WP26's macro correspondence.
4. Emit borrowck loan/region facts where required and available; when absent,
   the capability record states loan liveness is conservative/absent
   (`GEN §44.3`) — never silent.
5. Emit vtable layout for WP25; pin the adapter digest into the toolchain
   bundle and provider runs; add exhaustive variant tests over the private
   surfaces used.

**Legacy Disposition and Decommission.** Identity upgrade is a governed
transition: WP18-keyed facts and DefPathHash-keyed facts never coexist in one
snapshot; the switch happens per-context at reconciliation with both recipes
versioned (GI-09). No alias identity survives a snapshot boundary.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_private_enrichment_conformance`; Executable oracle: `rust_private_adapter_containment_parity`; Executable oracle: `rust_private_unavailable_degradation_falsification`; Executable oracle: `rust_private_digest_pinning_gate`.

- **Behavioral — Executable oracle:** `rust_private_enrichment_conformance` —
  fixtures verify DefPathHash stability across builds, byte-exact spans for
  macro-heavy code, and loan/region facts on the borrowck fixture set.
- **Structural — Executable oracle:**
  `rust_private_adapter_containment_parity` — `rustc_private` symbols appear
  only in the adapter module (structural scan of the extractor crate); the
  stable root's dependency graph is unchanged (`just stable-graph-check`).
- **Negative/Zero-State — Executable oracle:**
  `rust_private_unavailable_degradation_falsification` — with the adapter
  compiled out (or a surface stubbed unavailable), every dependent family
  degrades to its documented conservative posture with explicit capability
  records; no fact silently disappears.
- **Operational — Executable oracle:** `rust_private_digest_pinning_gate` —
  an adapter-digest mismatch between bundle and handshake fails before
  activation; the SUITE §83.6 differential harness runs on a simulated
  nightly bump.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just stable-graph-check`,
`just root-ci-fast`, `just packet-oracle-check WP24`.

**Integration Milestone.** M04.

**Replan Triggers.** A required private surface is absent at the pinned
nightly, or containment per App. P.7 fails — the affected capability ships
conservative/absent (adaptation) unless `RUST_SEMANTIC_V1` mandates it, in
which case the profile decision returns to the ontology owner.

**Rollback or Recovery.** The adapter is feature-contained in the extractor;
disabling it restores the WP18 fallback identity path deterministically.

### WP25 — Executable instances and dynamic dispatch

**Outcome.** Monomorphized instances are first-class: `MONO_INSTANCE` keyed by
(definition, canonical generic args, instance kind) with MONOMORPHIZES/
CALLS_INSTANCE edges and ABI/name metadata; static dispatch carries exact
compiler targets; dynamic dispatch carries candidate sets from trait contract
+ impl inventory + vtable evidence with INVOKES_TRAIT_CONTRACT/USES_VTABLE/
MAY_DISPATCH_TO edges that are `SOUND_MAY`/`POSSIBLE` — never exact; open
trait worlds yield `UNKNOWN_EXTERNAL_IMPLEMENTATION`; generic MIR stays one
source-level body. `rust_instance`/`rust_vtable_entry` tables land.

**Dependencies.** WP24.

**Target invariants.** GI-04, GI-06; DF-P20; `ONT §52` definition-vs-instance
separation.

**Design and library references.** `GEN §41.5`; `GEN §42`
(traceability-corrected into this wave); `ONT §52`–`§53`; `FAB §53`, `§57`;
LD-RS-01 (`mir §20`–`§23`, App. L); LD-RS-02 (vtables).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'MONO_INSTANCE|USES_VTABLE|MAY_DISPATCH' contracts/registry src -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(41|42)\.'
```

Known current touch includes `rustc-extractor/src/rustc_link.rs` (instance
enumeration), the private adapter (vtable evidence), the Contract IR
(`rust_instance`, `rust_vtable_entry`), ontology registries, and ingest.

**Required changes.**

1. Enumerate mono instances with canonical generic-argument identity (through
   the shared type interner); emit MONOMORPHIZES, argument relation rows with
   ordinals, ABI/mangled-name metadata, and upgrade WP20 call rows with
   CALLS_INSTANCE where resolution is exact.
2. Model dynamic dispatch: trait contract edges, impl inventory, unsize/vtable
   sites (private adapter or impl-inventory overapproximation), receiver type
   flow; every dynamic edge carries `SOUND_MAY`/`POSSIBLE` certainty; open
   worlds add `UNKNOWN_EXTERNAL_IMPLEMENTATION`.
3. Closures and drop glue resolve via `Instance::resolve_closure`/
   `resolve_drop_in_place` (drop-glue facts consumed by WP27).
4. Generic source bodies stay single entities; concrete materialization only
   if explicitly enabled.

**Legacy Disposition and Decommission.** None — new families.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_instance_dispatch_fixture_conformance`; Executable oracle: `rust_declared_instance_distinction_parity`; Executable oracle: `rust_dyn_dispatch_open_world_falsification`; Executable oracle: `rust_instance_ingest_operational_gate`.

- **Behavioral — Executable oracle:**
  `rust_instance_dispatch_fixture_conformance` — the `GEN §93.2`
  monomorphization/dispatch fixtures (generics, trait objects, closures, fn
  pointers) produce exact instance and dispatch rows.
- **Structural — Executable oracle:**
  `rust_declared_instance_distinction_parity` — DECLARED_FUNCTION and
  MONO_INSTANCE are distinct entities; CALLS_DECLARATION and CALLS_INSTANCE
  never merge; instance identity uses interned generic args, never mangled
  strings.
- **Negative/Zero-State — Executable oracle:**
  `rust_dyn_dispatch_open_world_falsification` — a public trait with external
  implementors keeps `UNKNOWN_EXTERNAL_IMPLEMENTATION` in every candidate
  set; marking a dyn edge exact fails.
- **Operational — Executable oracle:**
  `rust_instance_ingest_operational_gate` — instance ingest is deterministic
  across re-extraction and owner replacement keeps instance/call rows
  consistent.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just packet-oracle-check WP25`.

**Integration Milestone.** M04.

**Replan Triggers.** Vtable evidence is unavailable and the impl-inventory
overapproximation produces unbounded candidate sets on real corpora — bounded
candidate policy returns to the spec.

**Rollback or Recovery.** Additive; capability-gated.

### WP26 — Macros, generated code, and lowered correspondence

**Outcome.** Generated and lowered code is a separate identity domain per
`GEN AC-G-40`: expansion facts (EXPANSION/EXPANDED_ITEM/EXPANDS_TO/
GENERATED_FROM/SOURCE_CORRESPONDENCE) with no one-to-one source/MIR
assumption; async/coroutine lowering entities with best-effort await
correlation; generated identity as
workspace+context+generator+role+logical-name+content-digest with
`codefabric-generated://` URIs; build-script/proc-macro outputs read only from
the sandbox output tree; source-authored and generated spans never compared in
one coordinate space; invalidation edges to generator inputs. The
`rust_macro_expansion` table lands.

**Dependencies.** WP25.

**Target invariants.** GI-03, GI-09; DF-P3, DF-P18; `ONT §27` representation
classes.

**Design and library references.** `GEN §43`, `§48`; `GEN AC-G-40` (ten
representation classes, identity, retention); `ONT §27`, `§54`, `§56.2`;
`FAB §58`; LD-RS-02 (SourceMap/hygiene); LD-TS-01 (invocation syntax);
`mir §17`, `§47.2`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'codefabric-generated|EXPANDS_TO|representation_class' src rustc-extractor/src contracts -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(43|48)\.'
```

Known current touch includes the extractor (expansion/lowering extraction via
the private adapter's hygiene data), the WP16 sandbox output tree (capture),
the Contract IR (`rust_macro_expansion`), ontology registries, and ingest.

**Required changes.**

1. Emit expansion facts joining Tree-sitter invocation syntax to compiler
   expansion data; hygiene retained where exposed; explicitly represent 1:N
   and N:1 correspondence; unmapped macro-generated spans yield the `GEN §51`
   unknown, never a fake mapping.
2. Emit async/coroutine lowering entities (ASYNC_FUNCTION…RESUME_POINT,
   LOWERS_TO_COROUTINE); calling an async fn is never represented as
   executing its body.
3. Implement AC-G-40 identity: representation classes as separate domains;
   generated identity URIs; capture build-script/proc-macro outputs from the
   WP16 sandbox output tree tied to exact run/context digests; shims/drop
   glue may have no text yet expose structured lowered facts.
4. Wire invalidation edges from generated owners to generator inputs, build
   unit, context, toolchain, and bundle; retention is snapshot-lease-only for
   captured text (durable facts keep digests + provenance).

**Legacy Disposition and Decommission.** None — new families.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_macro_lowering_fixture_conformance`; Executable oracle: `rust_representation_class_parity`; Executable oracle: `rust_generated_span_conflation_falsification`; Executable oracle: `rust_generated_capture_operational_gate`.

- **Behavioral — Executable oracle:**
  `rust_macro_lowering_fixture_conformance` — the `GEN §93.2` macro and
  async fixtures (derive macros, proc macros, nested expansion, async fns)
  produce exact expansion/lowering rows with correct multiplicity.
- **Structural — Executable oracle:** `rust_representation_class_parity` —
  every generated/lowered fact carries its representation class; generated
  identities use the URI scheme, never workspace paths.
- **Negative/Zero-State — Executable oracle:**
  `rust_generated_span_conflation_falsification` — a query mixing
  source-authored and generated spans in one coordinate space fails
  structurally; an unmapped expansion yields the explicit unknown.
- **Operational — Executable oracle:**
  `rust_generated_capture_operational_gate` — a proc-macro change invalidates
  all reachable generated owners (`LIFE §8.6` broad invalidation); captured
  outputs are lease-retained and GC'd with the snapshot.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just packet-oracle-check WP26`.

**Integration Milestone.** M04.

**Replan Triggers.** Hygiene/expansion provenance at the pinned nightly cannot
distinguish a required correspondence class — the class ships as explicit
unknown (adaptation) unless AC-G-40 mandates it (spec return).

**Rollback or Recovery.** Additive; capability-gated.

### WP27 — Drop, unsafe, constants, and FFI facts

**Outcome.** Compiler-generated destruction, unsafe operations, constant
evaluation, and FFI boundaries are objective canonical facts: DROP_SITE per
MIR Drop terminator with DROPS/DROPS_FIELD/INVOKES_DROP_GLUE/
INVOKES_DROP_IMPL (recursive drop glue represented, never omitted for lacking
source text); CONTAINS_UNSAFE_OPERATION/CALLS_FOREIGN/CROSSES_FFI/
USES_INLINE_ASSEMBLY from MIR with syntax facts from Tree-sitter; normalized
constant value forms with no compiler allocation handles persisted; opaque
FFI boundaries contribute UNKNOWN_EFFECT/UNKNOWN_MEMORY.

**Dependencies.** WP26.

**Target invariants.** GI-04, GI-06; DF-P20; `ONT §55.2`, `§57`–`§58`.

**Design and library references.** `GEN §47`, `§49`, `§50`; `ONT §55`, `§57`,
`§58`; LD-RS-01 (`resolve_drop_in_place`); effect/resource modeling stays
Wave 14 (`GEN §§47–50` shared ownership per the traceability index).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'DROP_SITE|INVOKES_DROP_GLUE|CALLS_FOREIGN|USES_INLINE_ASSEMBLY' contracts/registry src -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md --match '^(47|49|50)\.'
```

Known current touch includes the extractor (drop/unsafe/const/FFI extraction),
ontology registries, ingest, and the WP22 access-event stream (drop events).

**Required changes.**

1. Emit DROP_SITE per Drop terminator; resolve drop glue via
   `Instance::resolve_drop_in_place`; represent recursive glue; RAII resource
   classification stays model-pack territory — the MIR drop facts are factual
   regardless.
2. Emit unsafe/FFI operational facts from MIR (unsafe operations, foreign
   calls, FFI crossings, inline assembly) with syntax facts from Tree-sitter;
   opaque boundaries contribute UNKNOWN_EFFECT/UNKNOWN_MEMORY.
3. Normalize constant/static/CTFE value forms; internal allocation handles
   never persist (`ONT §58`).
4. Effect and resource *models* over these facts are explicitly deferred to
   Wave 14 — assert the boundary.

**Legacy Disposition and Decommission.** None — new families.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_drop_unsafe_ffi_fixture_conformance`; Executable oracle: `rust_drop_glue_representation_parity`; Executable oracle: `rust_ffi_opaque_unknown_falsification`; Executable oracle: `rust_resource_fact_ingest_gate`.

- **Behavioral — Executable oracle:**
  `rust_drop_unsafe_ffi_fixture_conformance` — the `GEN §93.2`
  drop/unsafe/const/FFI fixtures produce exact rows including recursive drop
  glue and TLS/const distinctions.
- **Structural — Executable oracle:** `rust_drop_glue_representation_parity` —
  every compiler-generated destructor site exists as a fact even with no
  source `drop()`; glue instances link through WP25 instance identity.
- **Negative/Zero-State — Executable oracle:**
  `rust_ffi_opaque_unknown_falsification` — an extern call with an opaque
  body carries UNKNOWN_EFFECT/UNKNOWN_MEMORY; a persisted compiler allocation
  handle anywhere fails the scan; no Wave 14 effect model exists yet
  (boundary assertion).
- **Operational — Executable oracle:** `rust_resource_fact_ingest_gate` —
  drop/unsafe/FFI ingest replaces owners atomically and re-extraction is
  deterministic.

**Edit-Local Gates.** `just extractor-fmt`, `just extractor-check`.

**Packet-Local Gates.** `just extractor-ci-fast`, `just root-ci-fast`,
`just packet-oracle-check WP27`.

**Integration Milestone.** M04.

**Replan Triggers.** None specific beyond the shared nightly-drift trigger
(LD-RS-02).

**Rollback or Recovery.** Additive; capability-gated.

### WP28 — Rust lifecycle, incremental fingerprints, and `RUST_SEMANTIC_V1` COMPLETE

**Outcome.** The Rust lane is closed: the `LIFE §96` pipeline (metadata →
incremental invocation → owned records → complete manifest → owner-fingerprint
comparison → changed-owner replacement → MIR-derived facts) runs continuously
with `LIFE §8.6`–`§8.7` invalidation (macro changes invalidate reachable
expansions broadly; trait/impl/signature changes propagate through the
reverse semantic dependency graph), the `LIFE §17` safe-reuse fast path only
under conservative proof, compiler-version and adapter-digest mismatches fail
before activation, and compile break/fix plus signature/trait incremental
scenarios compare equal to clean rebuild. `RUST_SEMANTIC_V1` reports
`COMPLETE` for the selected corpus/context. M04 closes on
`just wave11-integration-check`.

**Dependencies.** WP27.

**Target invariants.** GI-05, GI-09, GI-10, GI-12; DF-P18, DF-P19, DF-P25.

**Design and library references.** `LIFE §96`, `§8.6`–`§8.7`, `§17`;
`LIFE §137`/`SUITE AC-G-79` comparator; SUITE AC-G-07, `§83.6`;
`ONT AC-G-72`/`§84` profile conformance; `RM §16` exit evidence; `mir §35`,
`§38`, `§44` owner fingerprints and replacement.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'mir_fingerprint|REANCHORED_UNCHANGED|owner_fingerprint' src contracts -g '!**/generated/**'
rg -n 'RUST_SEMANTIC' contracts/registry/capability-registry.yaml src/
just spec-outline docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md --match '^(17|96)\.'
```

Known current touch includes Rust-owned lifecycle/invalidation/reconciliation/
capability fragments, the comparator corpus, and the `justfile`; WP01's frozen
shared dispatch files are not edited here.

**Required changes.**

1. Implement owner-fingerprint comparison (source-sensitive and
   semantic-sensitive fingerprints per `mir §38`) driving changed-owner
   replacement; unchanged-owner reuse only under the `LIFE §17` conservative
   proof, labeled `REANCHORED_UNCHANGED_SEMANTICS`.
2. Wire the invalidation matrix: macro-definition change → all reachable
   invocations/generated owners (broad when uncertain); trait/impl/signature/
   bound change → call-target resolution, candidate sets, mono instances via
   the reverse semantic dependency graph.
3. Enforce pre-activation failure on compiler-version and adapter-digest
   mismatch (handshake + bundle checks); the §83.6 upgrade differential
   harness stays wired from WP24.
4. Extend the comparator corpus with the Rust incremental scenarios (body
   edit, signature change, trait change, macro change, compile break/fix,
   context change) and prove equal-to-rebuild for every Wave 10/11 family.
5. Close `RUST_SEMANTIC_V1` via aggregation over the `ONT §84` conformance
   surface; populate `just wave11-integration-check`.

**Legacy Disposition and Decommission.** The Rust halves of DB01 (direct
extractor invocation) and DB03 (MIR summary blobs) must be at zero before
this packet completes — their negative checks are prerequisites of the wave
gate. The Python halves close at M02; full zero state is re-verified at M05.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `rust_incremental_rebuild_equivalence_conformance`; Executable oracle: `rust_semantic_profile_complete_parity`; Executable oracle: `rust_toolchain_mismatch_activation_falsification`; Executable oracle: `wave11_integration_operational_gate`.

- **Behavioral — Executable oracle:**
  `rust_incremental_rebuild_equivalence_conformance` — every Rust incremental
  scenario (including compile break/fix and signature/trait changes) compares
  equal to the independent clean rebuild under AC-G-79 bag equality.
- **Structural — Executable oracle:**
  `rust_semantic_profile_complete_parity` — `RUST_SEMANTIC_V1 = COMPLETE`
  derives from aggregation over every applicable build unit/context with zero
  uncharacterized children; safe-reuse labels appear exactly where the
  conservative proof held.
- **Negative/Zero-State — Executable oracle:**
  `rust_toolchain_mismatch_activation_falsification` — a compiler-version or
  adapter-digest mismatch fails before snapshot activation; a stale-toolchain
  observation batch is rejected; no snapshot ever mixes toolchain
  generations.
- **Operational — Executable oracle:**
  `wave11_integration_operational_gate` — `just wave11-integration-check`
  passes end-to-end including the invalidation matrix and the comparator
  scenarios.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted lifecycle
tests.

**Packet-Local Gates.** `just root-ci-fast`, `just extractor-ci-fast`,
`just wave11-integration-check`, `just rebuild-equivalence-check`,
`just packet-oracle-check WP28`.

**Integration Milestone.** M04.

**Replan Triggers.** Equal-to-rebuild fails structurally for a Rust family
(identity/ordering design flaw) — design reopening; fingerprint-driven reuse
proves unsound for a scenario class — fall back to full-owner rerun
(adaptation) and record the precision loss.

**Rollback or Recovery.** Aggregation-derived advertisement demotes
automatically on regression; the comparator harness is the recovery oracle.

### Wave 12 group — Full reconciliation, completeness, contexts, and unknown remainder (`RM §17`)

### WP29 — Complete canonical reconciliation engine

**Outcome.** The `ReconciliationEngine` implements the full `FAB AC-G-37`
eight-step deterministic pipeline over all provider lanes: schema/version/
generation validation, DTO normalization, canonical-key sorting, the `GEN §80`
range ladder, preimage grouping, authority application, emission of fact +
evidence + conflicts + unknowns + capability outcomes, and owner content
fingerprints — with the `FAB §73` plan families (source-range join,
declaration join, type reconciliation, call-target distinctness, unknown
anti-join) expressed as built-in DataFusion plans and `FAB §74` dedup via
`row_number()` over integer authority rank. Equal-authority conflicting exact
observations yield `CONFLICTING_EXACT_EVIDENCE` with no arbitrary winner; no
component outside the engine canonicalizes.

**Dependencies.** WP14, WP28.

**Target invariants.** GI-04, GI-11; DF-P2, DF-P3, DF-P14, DF-P15; `RM §17` exit: no
canonical fact produced twice by competing authorities.

**Design and library references.** `FAB AC-G-37` (pipeline, sort key,
source-correspondence table, rejection rules); `GEN §5`, `§80`–`§83`, `§86`;
`FAB §72`–`§75`; `FAB §16.2` evidence normalization; LD-DF-01 (`df §11`–`§12`,
`§23`, `§28`, `§43`; `align` LOG-01–LOG-07, EXP-01/02, RUN-09).

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/fact_ingest.rs
rg -n 'reconcile_candidates|precedence|authority_rank|ConflictRecord' src/fact_ingest.rs src/core_facts.rs
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^(72|73|74|75)\.'
```

Known current touch includes `src/fact_ingest.rs` (`CanonicalReconciliationEngine`
generalization — the existing precedence engine is the seed, not a rewrite
target), `src/core_facts.rs` (per-provider reconcile entry points converge on
the engine), the provider-normalization registry (authority ranks), and the
integrity-validation query set.

**Required changes.**

1. Generalize the engine to the AC-G-37 pipeline with the canonical sort key
   (`workspace, context, owner, fact_form, fact_kind, subject, role/ordinal,
   object/value, provider precedence, span, observation digest`); duplicate
   preimages with unequal payloads are conflicts, equal payloads coalesce with
   all evidence IDs; provider-version skew rejects the batch.
2. Implement the source-correspondence table: identifiers need exact byte
   spans; declarations exact name span + compatible kind/owner; call sites
   exact callee span or ≥0.80 overlap with a unique candidate;
   generated/lowered items require the explicit correspondence key — no fuzzy
   source-only match.
3. Express the five `FAB §73` plan families as built-in DataFusion plans over
   observation batches inside the engine boundary; memory reservations and
   spill for workspace-scale joins; `row_number()` over stable integer
   authority ranks for dedup; display names and provider-local IDs never break
   ties.
4. Normalize evidence content (`FAB §16.2`): run-local locators replaced for
   clean-rebuild comparison; cold payloads use admitted locators, never
   sandbox paths.
5. Run the `FAB §75` integrity queries pre-publication (PK uniqueness,
   referential existence, span bounds, call-target/unknown-kind matching) as
   application-enforced DataFusion checks.

**Legacy Disposition and Decommission.** Per-provider ad hoc canonicalization
in `reconcile_rustc_compilation`/`reconcile_pyrefly_run` converges on the one
engine — those entry points become staging adapters; any residual
canonical-row construction outside the engine reaches zero (structural
negative below). No `ExtensionDecisionRecord` exists because no non-built-in
mechanism is used.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `canonical_reconciliation_pipeline_conformance`; Executable oracle: `reconciliation_single_authority_parity`; Executable oracle: `conflicting_exact_evidence_falsification`; Executable oracle: `reconciliation_plan_family_operational_gate`.

- **Behavioral — Executable oracle:**
  `canonical_reconciliation_pipeline_conformance` — the multi-provider fixture
  corpus (Ruff+Pyrefly+Tree-sitter+rustc overlapping observations) reconciles
  to exact canonical rows with correct authority selection, evidence
  retention, and owner fingerprints.
- **Structural — Executable oracle:**
  `reconciliation_single_authority_parity` — every canonical-row write path in
  the stable root flows through the engine (structural scan + the existing
  ingest boundary types); the authority ranks in generated registries match
  `GEN §5` exactly.
- **Negative/Zero-State — Executable oracle:**
  `conflicting_exact_evidence_falsification` — equal-authority conflicting
  exact observations produce `CONFLICTING_EXACT_EVIDENCE` and an
  unresolved/multi-candidate representation; a provider-version-skewed batch
  is rejected whole; no arbitrary winner exists on any fixture.
- **Operational — Executable oracle:**
  `reconciliation_plan_family_operational_gate` — the five plan families
  execute within memory reservations on the workspace-scale corpus with spill
  verified, and `FAB §75` integrity queries gate publication.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, engine unit tests.

**Packet-Local Gates.** `just root-ci-fast`,
`just publication-referential-integrity-check`, `just model-repro-check`,
`just packet-oracle-check WP29`.

**Integration Milestone.** M05.

**Replan Triggers.** A plan family cannot be expressed with built-in
DataFusion plans at required scale — an `ExtensionDecisionRecord` decision
(design reopening, `FAB §72`); the AC-G-37 sort key cannot be made
deterministic for a fact family — spec return.

**Rollback or Recovery.** The engine versions its pipeline; reconciliation is
re-runnable from retained observations, so recovery is re-ingest under the
prior engine version.

### WP30 — Property cardinality and storage integrity

**Outcome.** Every canonical property is a first-class registered property
fact with enforced cardinality: the ontology property registry is populated
for all Wave 8–11 families; `EXACTLY_ONE`/`ZERO_OR_ONE`/`ZERO_OR_MORE`/
`ONE_OR_MORE` integrity holds (missing exactly-one is a capability gap, never
a null; two active zero-or-one values become retained-conflict + unresolved);
null cells never mean unknown; denormalized entity columns are provably
projections of one selected property fact; extension-table columns carry
round-trip mappings or are marked payload-only. Cardinality integrity queries
are generated from the registry and gate publication.

**Dependencies.** WP29.

**Target invariants.** GI-06, GI-12; DF-P12; `ONT AC-G-71` rules 1–8.

**Design and library references.** `ONT AC-G-71`; `FAB §16.1`, `§10`, `§11.1`;
`FAB §75`; the registry baseline (10 core property records exist —
NAME…CATEGORICAL_KIND; every Wave 8–11 property is absent and registers at its
owning packet per the §4 group rule); LD-DL-01 schema round-trip.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'property_code|property_slug|cardinality' contracts/registry/ontology-property-registry.yaml | head -40
rg -n 'PropertyValue|value_kind' src/fact_ingest.rs src/generated/fact_row_encoders.rs
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^16\.'
```

Known current touch includes `contracts/registry/ontology-property-registry.yaml`
(coverage verification), the schema drivers (generated cardinality validators +
Arrow/Delta/Pydantic projections), `src/fact_ingest.rs` (integrity enforcement),
and every extension table's denormalization declarations in the Contract IR.

**Required changes.**

1. Verify registry coverage: every property emitted by Waves 8–11 was
   registered at its owning packet (value kind, cardinality class,
   occurrence-specificity, owner family — the §4 group rule). Any gap fails WP30
   and M05 and requires returning to the offending packet; WP30 must not add or
   regularize its record. Then have the schema generator emit Arrow fields, Delta constraints,
   ingestion validators, and cardinality integrity queries from the registry.
2. Enforce the tagged-union value contract (exactly one value column per
   `value_kind_code`; complex structures via entity refs/typed extension
   tables, never opaque JSON).
3. Enforce cardinality at ingest and publication: missing `EXACTLY_ONE` →
   capability/validation gap; double `ZERO_OR_ONE` → conflict retained,
   canonical unresolved; multi-valued → one row per value unless an ordered
   list is declared.
4. Declare and prove denormalized projections: every denormalized entity
   column names its source property fact; round-trip equality is generated
   and tested; extension columns without round-trips are marked
   payload-only/non-query-visible.

**Legacy Disposition and Decommission.** Predecessor property records may be
audited here, but any Wave 8–11 property without its owning-packet registry entry
is a prior-packet contract violation and hard failure, never migration work. The
first-publication guard from WP01 remains the closed authority.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `property_cardinality_integrity_conformance`; Executable oracle: `property_registry_projection_parity`; Executable oracle: `null_as_unknown_falsification`; Executable oracle: `denormalized_round_trip_operational_gate`.

- **Behavioral — Executable oracle:**
  `property_cardinality_integrity_conformance` — fixtures for each cardinality
  class (missing exactly-one, duplicate zero-or-one, multi-valued ordering)
  produce the exact prescribed gap/conflict/row outcomes.
- **Structural — Executable oracle:** `property_registry_projection_parity` —
  every persisted property ID resolves in the populated registry; generated
  validators/queries cover every class; an unregistered property fixture
  fails model compilation.
- **Negative/Zero-State — Executable oracle:** `null_as_unknown_falsification`
  — no ingest path writes a null to mean unknown/not-populated; the FAB §65.1
  scan over encoders finds zero violations; a null-as-unknown fixture is
  rejected.
- **Operational — Executable oracle:**
  `denormalized_round_trip_operational_gate` — every declared denormalized
  column round-trips against its property fact on the full fixture
  publication; integrity queries run in the publication gate.

**Edit-Local Gates.** `just root-fmt`, `just root-check`,
`just model-family-check schemas`.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just property-registry-closure-check`,
`just publication-referential-integrity-check`,
`just packet-oracle-check WP30`.

**Integration Milestone.** M05.

**Replan Triggers.** A required property cannot satisfy the tagged-union
contract (needs an open structure) — spec return on AC-G-71 rule 1.

**Rollback or Recovery.** Registry population is additive and generated
artifacts regenerate; enforcement gates are monotone.

### WP31 — Capability aggregation completion

**Outcome.** Capability coverage is a formal aggregation, never a provider
status string: `CapabilityEvidence` at the smallest registered scope
(workspace → callable) with both states, coverage fingerprints, and remainder
flags; the `GEN AC-G-36` rules hold (`COMPLETE` only when every applicable
child is complete, remainder characterized, contexts covered, closure policy
permits; one `INDETERMINATE` child blocks parent `COMPLETE`; exclusions are
explicit); the six `ONT §62` registries stay orthogonal; profile advertisement
flows only through `ONT AC-G-72` named profiles with per-query coverage when
`PARTIAL`.

**Dependencies.** WP30.

**Target invariants.** GI-06, GI-12; DF-P20, DF-P21.

**Design and library references.** `GEN AC-G-36`, `§85`; `ONT §62`–`§63`,
`AC-G-72`; `LIFE §23`; the existing `aggregate_capability`/`CapabilityChild`
machinery in `src/core_facts.rs` (seed).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'aggregate_capability|CapabilityEvidence|coverage_scope_fingerprint' src/core_facts.rs contracts/registry/capability-registry.yaml
just spec-outline docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md --match '^62\.'
```

Known current touch includes `src/core_facts.rs` (aggregation generalization
over owner/scope/context/profile dimensions), the capability registry
(scope-kind and prerequisite completion for all Wave 8–11 capabilities), the
`capability_status` operational projection, and `src/semantic_query.rs`
(availability exposure).

**Required changes.**

1. Complete `CapabilityEvidence` records: scope, context, generation, both
   states, provider run, reason, coverage fingerprint, external/unknown
   remainder flags, supporting-owner-set digest — for every Wave 8–11
   capability at its registered scope kind.
2. Implement the aggregation algebra across owner → file → module/crate →
   build unit → context → workspace and per-profile roll-ups; exclusions are
   explicit scope exclusions.
3. Keep the vocabularies orthogonal (owner-capability, completeness,
   provider-run, certainty, resolution, directness, query-side states) with
   the generated registries as the single source; structural checks reject
   any second vocabulary.
4. Expose per-query owner/capability coverage when a profile is `PARTIAL`
   (`ONT AC-G-72`), feeding WP33's coverage proofs.

**Legacy Disposition and Decommission.** Blanket per-workspace capability
flags (Gate-B-era `semantic_capabilities_required` booleans) are replaced by
registry-scoped capability records; the boolean survives only as a scheduling
hint, never as advertised coverage.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `capability_aggregation_conformance`; Executable oracle: `capability_vocabulary_orthogonality_parity`; Executable oracle: `indeterminate_child_complete_falsification`; Executable oracle: `capability_aggregation_operational_gate`.

- **Behavioral — Executable oracle:** `capability_aggregation_conformance` —
  aggregation fixtures (mixed owner states across scopes/contexts) produce
  the exact parent states the algebra prescribes.
- **Structural — Executable oracle:**
  `capability_vocabulary_orthogonality_parity` — every state value in
  persisted rows and responses resolves to its one generated registry; no
  collapsed confidence score exists anywhere (structural scan).
- **Negative/Zero-State — Executable oracle:**
  `indeterminate_child_complete_falsification` — one `INDETERMINATE` child
  provably blocks every ancestor's `COMPLETE`; a silently-excluded owner
  fixture fails; a profile advertised outside AC-G-72 fails.
- **Operational — Executable oracle:**
  `capability_aggregation_operational_gate` — capability recomputation after
  a provider outage converges (outage → PARTIAL → recovery → COMPLETE) with
  the operational projection consistent at every step.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, aggregation unit
tests.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP31`.

**Integration Milestone.** M05.

**Replan Triggers.** A capability's registered scope kind cannot be computed
from available owner data — registry design return.

**Rollback or Recovery.** Aggregation is derived state; recovery is
recomputation.

### WP32 — Unknown remainder and explicit negative facts

**Outcome.** The unknown ontology is complete and identity-stable: all twelve
`ONT AC-G-73` unknown kinds with deterministic scoped BLAKE3/CBEF identity
(workspace, context, owner/proof scope, kind, originating role, reason,
candidate-set digest, program point where relevant), nine reason classes, the
remainder-in-candidate-set semantics (one unknown-remainder entity meaning
"additional targets may exist"), `AUTHORIZATION_EXCLUDED` never persisted as a
semantic unknown, and the four registered negative-fact families created on
demand with proof-universe fingerprints — invalid across any change of
projection, context, authorization scope, generation, or precision profile.

**Dependencies.** WP31.

**Target invariants.** GI-06; DF-P20; `ONT §65` separations.

**Design and library references.** `ONT AC-G-73`; `ONT §32`, `§66`; `GEN §84`;
`FAB §59` (`unknown_detail`). Registry baseline: the 25-record
`unknown-registry.yaml` already carries all twelve AC-G-73 kinds, all nine
reason classes, and the four negative-fact families, and has settled on
`UNKNOWN_MEMORY_LOCATION` — the remaining divergence is `UNKNOWN_MEMORY` in
GEN/ONT prose, a spec-reconciliation note, not registry work.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'UNKNOWN_MEMORY|UNKNOWN_FFI|UNKNOWN_RESOURCE' contracts/registry/unknown-registry.yaml src -g '!**/generated/**'
just spec-outline docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md --match '^(32|66)\.'
```

Known current touch includes the unknown-identity builders,
`src/fact_ingest.rs` (unknown anti-join emission), the negative-fact
*machinery* (new — the registry records exist, the on-demand
creation/invalidation runtime does not), and the spec-prose reconciliation
note for the `UNKNOWN_MEMORY` naming.

**Required changes.**

1. Wire producers for every registered unknown kind so each of the twelve has
   at least one emitting path and fixture; record the
   `UNKNOWN_MEMORY`→`UNKNOWN_MEMORY_LOCATION` prose divergence as a
   design-corpus reconciliation item (the registry is already the settled
   authority).
2. Implement deterministic unknown identity via the CBEF builders — no global
   unknown node; candidate sets hold exact/possible entities plus at most one
   unknown-remainder entity whose edge is distinct from ordinary candidates.
3. Enforce that `AUTHORIZATION_EXCLUDED` is only a query-coverage gap: no
   persisted semantic unknown may reveal denied source (structural + fixture
   proof).
4. Implement the four negative-fact families (`PROVEN_DOES_NOT_ALIAS…`,
   `PROVEN_NO_PATH…`, `PROVEN_NOT_SUBTYPE…`, `PROVEN_NO_RESOLVED_MEMBER…`)
   with negated positive kind, proof-universe fingerprint, derivation/profile,
   coverage proof, and supporting facts; created on demand, never
   exhaustively materialized; invalidation on any universe change.

**Legacy Disposition and Decommission.** No unknown emission bypasses the
identity builders; the registry remains the closed authority.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `unknown_remainder_conformance`; Executable oracle: `unknown_identity_registry_parity`; Executable oracle: `negative_fact_scope_invalidation_falsification`; Executable oracle: `unknown_registry_operational_gate`.

- **Behavioral — Executable oracle:** `unknown_remainder_conformance` — the
  cross-language unknown fixtures (dynamic Python dispatch, Rust dyn open
  worlds, opaque FFI, provider failures) produce exactly the prescribed
  unknown kinds, reasons, and candidate-set structure.
- **Structural — Executable oracle:** `unknown_identity_registry_parity` —
  every persisted unknown resolves to a registry kind and reason; identical
  unknown propositions hash to identical IDs across runs; exactly one
  remainder entity per candidate set.
- **Negative/Zero-State — Executable oracle:**
  `negative_fact_scope_invalidation_falsification` — a negative fact is
  provably unreachable after a projection/context/generation/profile change
  (fixture flips each dimension); an authorization exclusion persisted as a
  semantic unknown fails.
- **Operational — Executable oracle:** `unknown_registry_operational_gate` —
  on-demand negative creation and invalidation operate through publication
  without exhaustive materialization; the registry regenerates
  deterministically.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP32`.

**Integration Milestone.** M05.

**Replan Triggers.** A negative family's proof universe cannot be
fingerprinted deterministically — spec return on AC-G-73.

**Rollback or Recovery.** Negative facts are on-demand derived state; recovery
is invalidation plus re-derivation.

### WP33 — Completeness and negative-proof algebra

**Outcome.** Completeness is a proof over an explicit universe: `CoverageProof`
records (universe fingerprint, required capabilities, covered/excluded/
unavailable owner sets with reasons, unknown/external remainder, widening,
limit state, freshness, proof witnesses) computed by gap accumulation;
`COMPLETE` iff the gap set is empty, `PARTIAL` iff every remainder is
characterized, `INDETERMINATE` otherwise; the seven empty-result categories
are distinct; a negative statement requires all seven `AC-G-48` conditions;
transitive negatives require frontier closure; completeness is orthogonal to
freshness; query responses expose the coverage record whenever absence,
uncertainty, availability, contexts, limits, or negatives matter.

**Dependencies.** WP32.

**Target invariants.** GI-02, GI-06; DF-P20, DF-P24; `RM §17` exit: `PROVEN_EMPTY`
only when the algebra permits.

**Design and library references.** `QRY AC-G-48`; `QRY §30`, `§46`, `§115`;
`ONT §62.7` query-side registries; WP31 coverage inputs; the existing
capability summaries in `src/semantic_query.rs` (seed).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'PROVEN_EMPTY|CoverageProof|coverage|completeness' src/semantic_query.rs src/query_service.rs -g '!**/generated/**'
just spec-outline docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md --match '^(30|46)\.'
```

Known current touch includes `src/semantic_query.rs` (coverage computation in
execution), `src/query_service.rs` (response surfacing), the query-form
response schemas only where AC-G-48 fields are missing (no form reopening),
and the QRY §115 conformance fixtures.

**Required changes.**

1. Implement `CoverageProof` computation during query execution from WP31
   capability evidence, WP32 unknowns, context partitions, authorization
   scope, limits, and freshness barriers; gap accumulation exactly per
   AC-G-48.
2. Classify every empty result into the seven categories; prose/`none`
   summaries derive only from `PROVEN_EMPTY`.
3. Gate negative statements on the seven conditions including frontier
   closure for transitive queries (one unknown frontier edge →
   indeterminate); unsupported fact families return unavailable, never a
   substituted lookalike.
4. Keep completeness ⊥ freshness: a stale snapshot may be internally
   complete but never supports a present-current negative proof.
5. Extend the QRY §115 conformance suite to cover all seven distinct
   responses across both language lanes.

**Legacy Disposition and Decommission.** Any earlier empty-result shape that
conflated categories is superseded; the conformance suite carries the
negative proof that only the seven registered categories exist.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `coverage_proof_algebra_conformance`; Executable oracle: `empty_result_category_parity`; Executable oracle: `proven_empty_gate_falsification`; Executable oracle: `completeness_response_operational_gate`.

- **Behavioral — Executable oracle:** `coverage_proof_algebra_conformance` —
  fixtures spanning each gap source (context gap, provider failure,
  authorization exclusion, external remainder, widening, limits) produce the
  exact prescribed proof records and states.
- **Structural — Executable oracle:** `empty_result_category_parity` — every
  empty response carries exactly one registered category; the query-side
  state registries stay orthogonal in the response schema.
- **Negative/Zero-State — Executable oracle:**
  `proven_empty_gate_falsification` — each of the seven conditions is
  individually violated in a fixture and each violation demotes
  `PROVEN_EMPTY`; a transitive query with one unknown frontier edge is
  indeterminate; a stale snapshot never yields a present-current negative.
- **Operational — Executable oracle:**
  `completeness_response_operational_gate` — coverage computation stays
  within the serving latency envelope on the reference corpus and the QRY
  §115 suite passes end-to-end through the production query service.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, targeted query
tests.

**Packet-Local Gates.** `just root-ci-fast`,
`just semantic-query-conformance-check`, `just packet-oracle-check WP33`.

**Integration Milestone.** M05.

**Replan Triggers.** AC-G-48 response fields cannot be added without reopening
the governed form contract — coordinate with the query-authority owner (plan
revision, since the form contract belongs to the predecessor plan's surface).

**Rollback or Recovery.** Coverage is response-derived state; the conformance
suite is the recovery oracle.

### WP34 — Multi-context and external-dependency policy

**Outcome.** Contexts are enforced independent semantic universes: the three
selection forms, one subplan per partition, traversal/pattern/closure
operators confined within partitions, tagged `ContextUnion` results, missing
defaults → `DEFAULT_CONTEXT_UNAVAILABLE`; and external dependencies are
endpoint-only under the six `ONT AC-G-16` policy classes with
`external_unknown_remainder` propagation and no cross-workspace traversal.
(FFI linking was split into WP37 by the plan challenge.)

**Dependencies.** WP37.

**Target invariants.** GI-05, GI-06; DF-P3, DF-P13; `RM §17` exit: multi-context
fixtures partitioned, external workspaces opaque.

**Design and library references.** `QRY AC-G-51`; `ONT AC-G-16`;
`GEN AC-G-14` context sets, `§88` activation pinning; `ONT §64.4`;
`src/analysis_context.rs`/`SnapshotContexts` (seed).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'ContextUnion|within_each_context|DEFAULT_CONTEXT' src contracts -g '!**/generated/**'
just spec-outline docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md --match '^64\.'
```

Known current touch includes `src/semantic_query.rs` (partition-scoped
subplans and result tagging) and the external-identity machinery in
reconciliation.

**Required changes.**

1. Enforce partition semantics in query execution: selection forms, one
   subplan per context, within-partition-only traversal and joins, explicit
   `within_each_context` for cross-partition references, tagged unions,
   deterministic default-context failure.
2. Implement the six external-dependency policy classes: endpoint-only
   identity for third-party code; bodies indexed only when version-locked,
   available, provider-supported, and explicitly authorized; upgrades mint
   new external identities; `external_unknown_remainder=true` propagates into
   coverage proofs when bodies are absent.
3. Pin context sets in snapshot activation (`GEN §88`) — already modeled;
   prove the query path respects the pinned set.

**Legacy Disposition and Decommission.** None — enforcement completion over
existing context modeling. Response completeness for all-context queries is
the product of per-context proofs (WP33 integration).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `multi_context_partition_conformance`; Executable oracle: `external_endpoint_policy_parity`; Executable oracle: `cross_context_traversal_falsification`; Executable oracle: `multi_context_activation_operational_gate`.

- **Behavioral — Executable oracle:** `multi_context_partition_conformance` —
  multi-context fixtures (two Python versions, two Rust feature sets) execute
  per-partition subplans and return tagged unions with per-context proofs.
- **Structural — Executable oracle:** `external_endpoint_policy_parity` —
  every external dependency resolves to one of the six policy classes;
  external entities are endpoint-only; upgraded versions have distinct
  identities.
- **Negative/Zero-State — Executable oracle:**
  `cross_context_traversal_falsification` — a path/pattern/closure crossing a
  context partition or into a separately indexed workspace is rejected;
  entities with identical display/span in two contexts never merge; an
  unqualified all-context negative with one incomplete context is blocked.
- **Operational — Executable oracle:**
  `multi_context_activation_operational_gate` — snapshot activation pins the
  context set; a context-set change activates a new snapshot with the prior
  one intact; a query against a missing default context fails with
  `DEFAULT_CONTEXT_UNAVAILABLE` and no arbitrary selection.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-ci-fast`,
`just semantic-query-conformance-check`, `just packet-oracle-check WP34`.

**Integration Milestone.** M05.

**Replan Triggers.** Partition enforcement requires response-schema changes
beyond AC-G-51's fields — coordinate with the query-form authority (plan
revision).

**Rollback or Recovery.** Enforcement gates are monotone.

### WP37 — Static FFI Linking Profile v1

**Outcome.** Python and Rust link through explicit bridge identities under
`ONT AC-G-17`: recognized declarative evidence (Rust
`extern`/`#[no_mangle]`/`#[export_name]`, PyO3 macro expansions where
compiler evidence exists, Maturin binding manifests, Python imports resolving
to registered extension exports), the seven FFI relations
(`FFI_EXPORTS`…`FOREIGN_SIGNATURE_OF`), exact linkage only under all six
conditions, `SOUND_POSSIBLE`/`POSSIBLE`/`UNKNOWN_FFI_TARGET` otherwise, and
traversal never pretending the two languages share a context. (Split from
WP34 by the plan challenge; its ID is late, its position is with the Wave 12
group.)

**Dependencies.** WP33.

**Target invariants.** GI-05, GI-06; DF-P3, DF-P20.

**Design and library references.** `ONT AC-G-17` (Static FFI Linking Profile
v1: evidence sources, seven relations, six exact-link conditions, manifest
precedence); `ONT AC-G-73` (`UNKNOWN_FFI_TARGET`); WP27 extern/FFI facts;
WP04/WP10 Python import facts.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'FFI_EXPORTS|BINDS_FOREIGN|extern "C"|no_mangle' src contracts/registry -g '!**/generated/**'
just spec-outline docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md --match '^AC-G-17'
```

Known current touch includes a new FFI-linking module consuming WP27 extern
facts and Python import facts, the ontology registries (FFI relations), and
reconciliation (bridge-identity emission).

**Required changes.**

1. Recognize the declarative evidence sources and emit the seven FFI
   relations with per-edge evidence classes; exact linkage only when module
   name, symbol name, ABI, build/context manifest, normalized
   parameter/return contract, and no-conflicting-candidate all hold.
2. Manifests may supply exact mappings but never override contradictory
   compiler evidence; ambiguity yields `UNKNOWN_FFI_TARGET` (WP32 identity).
3. Bridge identity keeps both originating language contexts explicit;
   traversal across the bridge never merges the contexts (WP34 partition
   enforcement applies).

**Legacy Disposition and Decommission.** None — new fact family.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `ffi_linking_fixture_conformance`; Executable oracle: `ffi_bridge_identity_parity`; Executable oracle: `ffi_ambiguous_link_unknown_falsification`; Executable oracle: `ffi_linking_profile_operational_gate`.

- **Behavioral — Executable oracle:** `ffi_linking_fixture_conformance` — the
  FFI fixture set (extern "C" exports, PyO3 module, Maturin manifest,
  generated header) produces the exact expected relations and evidence
  classes.
- **Structural — Executable oracle:** `ffi_bridge_identity_parity` — every
  FFI relation carries an explicit bridge identity naming both language
  contexts; no FFI edge merges contexts; exact edges satisfy all six
  conditions by construction.
- **Negative/Zero-State — Executable oracle:**
  `ffi_ambiguous_link_unknown_falsification` — an ambiguous symbol (two
  candidate exports) and a manifest contradicting compiler evidence both
  yield `UNKNOWN_FFI_TARGET`/possible-class edges, never a false exact link.
- **Operational — Executable oracle:** `ffi_linking_profile_operational_gate`
  — the PyO3 fixture workspace links Python callers to Rust exports with
  correct evidence classes end-to-end through serving.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just packet-oracle-check WP37`.

**Integration Milestone.** M05.

**Replan Triggers.** Exact-linkage conditions cannot be established from
declarative evidence on the fixture corpus — the profile ships
possible/unknown-heavy (allowed) unless AC-G-17 mandates exactness for a
case class (spec return).

**Rollback or Recovery.** Additive; capability-gated.

### WP35 — Derivation registry completion, profile revalidation, and integrated closure

**Outcome.** The derivation materialization registry is complete per
`FAB AC-G-42`: every derived family has exactly one owner implementation and
one precision profile per snapshot, with placement (publication+overlay,
full-table replacement, or query-time) and `(family, profile, bundle)`
selection; query-time results are response facts, never durable relations;
generation adapters provably materialize no competing derivation. The three
profiles — `CORE_SOURCE_V1`, `PYTHON_SEMANTIC_V1`, `RUST_SEMANTIC_V1` — are
revalidated against their exact `ONT AC-G-72` requirements on the integrated
substrate. M05 closes on `just wave12-integration-check`.

**Dependencies.** WP34.

**Target invariants.** GI-10, GI-12, GI-14; DF-P3, DF-P25; `RM §17` exit evidence
complete.

**Design and library references.** `FAB AC-G-42`, `§79A`; `GEN §87`,
`AC-G-39` precision profiles; `ONT AC-G-72`; `RM §17`; the single-entry
`derivation-registry.yaml` seed; `RM §25` integrated-substrate checkpoint.

**Change surface / Preflight / Known Touch.** Run:

```bash
cat contracts/registry/derivation-registry.yaml
rg -n 'DerivationRegistry|SYNTAX_TREE_V1|precision_profile' src contracts -g '!**/generated/**'
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^79A'
```

Known current touch includes `contracts/registry/derivation-registry.yaml`
(the full AC-G-42 matrix — Wave 13/14 families registered with owners and
placements but implementations deferred), `src/derivation.rs` (registry
enforcement), the capability registry (profile definitions), and the
`justfile` (`wave12-integration-check`).

**Required changes.**

1. Complete the registry to the AC-G-42 matrix: every derived family row with
   default precision profile, placement, canonical authority, input families
   and required completeness, unknown-propagation policy; Wave 13/14 families
   are registered-but-unimplemented (their implementations remain deferred —
   registration prevents a second authority from forming in the interim).
2. Enforce registry selection at runtime: materialization only through the
   registered `(family, profile, bundle)` implementation; query-time
   derivations produce response facts only; profile ID + bundle digest stamp
   every materialized derived fact (`GEN AC-G-39`).
3. Prove no generation adapter materializes a competing SCC/dominance/
   points-to/reaching-def/summary family (`GEN §87` boundary).
4. Revalidate the three capability profiles on the integrated substrate:
   every mandatory capability of each profile checked against the generated
   profile definition, with the full comparator corpus green across both
   lanes plus reconciliation.
5. Populate `just wave12-integration-check` (reconciliation, cardinality,
   aggregation, unknowns, algebra, contexts, FFI, derivation-registry,
   profile revalidation) and add the five wave gates to
   `just wave-acceptance-check`'s successor surface. Add
   `just semantic-profile-packets-check` with an explicit machine-checked list of
   all 38 unique packet selectors (WP01–WP38, including non-contiguous narrative
   placement); it loops valid `just packet-oracle-check WPnn` invocations and
   rejects a duplicate, omission, or unknown ID.

**Legacy Disposition and Decommission.** DB01–DB03 must be at zero (their
negative checks are prerequisites of the wave gate). The single-entry
registry becomes the complete matrix; unregistered derivation code paths are
structurally prohibited.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `profile_revalidation_conformance`; Executable oracle: `derivation_single_owner_parity`; Executable oracle: `competing_derivation_authority_falsification`; Executable oracle: `wave12_integration_operational_gate`.

- **Behavioral — Executable oracle:** `profile_revalidation_conformance` —
  `CORE_SOURCE_V1`, `PYTHON_SEMANTIC_V1`, and `RUST_SEMANTIC_V1` each
  revalidate against their exact generated profile requirements on the
  integrated fixture corpus.
- **Structural — Executable oracle:** `derivation_single_owner_parity` — the
  registry covers the full AC-G-42 matrix; every materialized derived fact
  carries its profile ID and bundle digest; exactly one active authority per
  family per snapshot.
- **Negative/Zero-State — Executable oracle:**
  `competing_derivation_authority_falsification` — a fixture implementation
  attempting to publish an unregistered derivation (or a second profile for
  a registered family) is rejected; petgraph/DataFusion intermediate
  positions provably never persist as canonical identity.
- **Operational — Executable oracle:** `wave12_integration_operational_gate`
  — `just wave12-integration-check` passes end-to-end on the integrated
  substrate, and the M05 closure set (all five wave gates + rebuild
  equivalence + Gate B) is green at one commit.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-ci-fast`, `just model-repro-check`,
`just wave12-integration-check`, `just semantic-profile-packets-check`,
`just semantic-provider-legacy-zero-state-check`,
`just packet-oracle-check WP35`.

**Integration Milestone.** M05.

**Replan Triggers.** The AC-G-42 matrix names a family whose registration
cannot be expressed in the current registry schema — registry schema
evolution is a plan revision (it touches Wave 1 artifacts).

**Rollback or Recovery.** Registry completion is additive; enforcement is
monotone. Profile revalidation failures demote the profile automatically.

## 5. Integration milestones

### M01 — Python local substrate current (`RM §13` exit)

**Packets.** WP01, WP36, WP02–WP07.
**Evidence.** `just wave8-integration-check — exit 0`;
`just rebuild-equivalence-check — exit 0` (Python local scenarios);
`just root-ci-fast — exit 0`; `just sidecar-ci-fast — exit 0` (WP01 changes
the embedded schema); `just model-repro-check — exit 0` (registry additions);
`just property-registry-closure-check`, `just semantic-fault-point-check`,
`just semantic-observability-contract-check`, and the Wave 8
`just semantic-profile-bench` workload green; DB02 negative checks green;
`PYTHON_SEMANTIC_V1 = PARTIAL` with named
missing capabilities (`py_semantic_profile_partial_parity`).
**Gate.** Python declarations, scopes, bindings, imports, CFG, and direct
def-use are continuously current; body-local edits invalidate only sound
owners; parse errors yield precise capability gaps.

### M02 — `PYTHON_SEMANTIC_V1` complete (`RM §14` exit)

**Packets.** WP08–WP14, WP38.
**Evidence.** `just wave9-integration-check — exit 0`;
`just sidecar-ci-fast — exit 0`; `just provider-protocol-check — exit 0`;
`just pyrefly-module-xref-surface-check — exit 0`;
`just semantic-sandbox-host-matrix-check — exit 0` on every advertised host;
the property/fault/observability gates and Wave 9 benchmark green;
`py_incremental_rebuild_equivalence_conformance` green;
`just semantic-provider-legacy-zero-state-check python — exit 0`; DB01 and DB03
**Python-half** negative checks green.
**Gate.** Cross-module definitions, types, members, and call targets match
canonical fixtures; sidecar failure never exposes invalidated semantics;
multiple Python contexts stay partitioned.

### M03 — Rust compiler/MIR core current (`RM §15` exit)

**Packets.** WP15–WP21.
**Evidence.** `just wave10-integration-check — exit 0`;
`just extractor-ci-fast — exit 0`; crash/timeout/stale-rejection scenarios
green; `RUST_SEMANTIC_V1 = PARTIAL`; the `TRUSTED_LOCAL`-exception
retirement proven (`rustc_unsandboxed_execution_falsification`); the
sandbox/property/fault/observability gates and Wave 10 benchmark green; DB03
**Rust-half** negative checks green (MIR summary blobs superseded at WP19).
**Gate.** Valid crates yield exact definitions/types/MIR/CFG/calls; compile
failure yields current source plus explicit gaps, never stale semantics.

### M04 — `RUST_SEMANTIC_V1` complete (`RM §16` exit)

**Packets.** WP22–WP28.
**Evidence.** `just wave11-integration-check — exit 0`;
`rust_incremental_rebuild_equivalence_conformance` green;
`rust_toolchain_mismatch_activation_falsification` green; the
property/fault/observability gates and Wave 11 benchmark green;
`just semantic-provider-legacy-zero-state-check rust — exit 0`; DB01
**Rust-half** negative checks green.
**Gate.** Move/copy/borrow/drop, macro/lowered, instance, and unwind fixtures
match; compile break/fix and signature/trait scenarios equal clean rebuild.

### M05 — Integrated semantic substrate (`RM §17` exit; plan completion)

**Packets.** WP29–WP35, WP37.
**Evidence.** `just wave12-integration-check — exit 0`; all five wave gates +
`just rebuild-equivalence-check` + `just gate-b-check` green at one commit;
the three profile revalidations green; DB01–DB03 **full** zero states green;
the property/fault/observability/sandbox/legacy gates and Wave 12 benchmark
green; `just semantic-profile-packets-check — exit 0`;
the §7 final gate matrix green.
**Gate.** One sound canonical state across all provider lanes; negative
statements only through the algebra; contexts partitioned; externals opaque;
derivation authority single.

## 6. Cross-packet decommission batches

All three batches are one governed executable surface, not three prose searches.
WP01 adds `scripts/semantic_provider_legacy_zero_state.sh`, the
`semantic-provider-legacy-zero-state-check` recipe, dedicated structural rules,
and `contracts/governance/semantic-provider-legacy-candidates.yaml`. The registry
enumerates candidate symbols/patterns and reviewed allow records with path, scope,
rationale, owner, expiry packet, and replacement. The recipe accepts `python`,
`rust`, or default `all`; it combines `rg --hidden -g '!.git/**'
-g '!docs/library_ref/**'`, structural `ast-grep run`, generated-model checks, and
the applicable clean root/sidecar/extractor build. Unexpected candidates, stale
allows, expired transition entries, hand-authored observation authority, direct
spawn consumers (including `src/fabric/serving.rs`), or opaque semantic ingest
cause non-zero exit. DB01–DB03 may add reviewed candidates during their transition,
but may not replace or weaken this recipe.

### DB01 — Direct provider invocation outside `ProviderRuntime`

**Names.** The hard-coded sidecar/extractor spawning in
`src/gate_b_candidate/vertical.rs` (`run_pyrefly`, `run_rustc` with fixed run
IDs, fixed digests, fixed module lists) and any sibling direct-spawn path.
`tooling/gate_b_adapter_probe.py` (invoked from the vertical via FastMCP
STDIO) is **retained** as the adapter-consumer probe of the production path —
its disposition is conversion, not deletion: after cutover it asserts
production payloads, and WP33's AC-G-48 response fields update its assertions.
**Prerequisites.** WP36 (adapters exist), WP08 (Python production path),
WP16–WP17 (Rust production path).
**Exit invariant.** Every semantic provider execution flows through
`ProviderRuntime` admission/journal; the Gate B vertical consumes the
production path.
**Negative proof.** Python half:
`just semantic-provider-legacy-zero-state-check python` plus
`pyrefly_provider_runtime_parity`. Rust half:
`just semantic-provider-legacy-zero-state-check rust` plus
`rustc_provider_runtime_parity`. Default `all` includes the reviewed structural
`Command::new` scan, `src/fabric/serving.rs`, stale-allow rejection, and clean
root/sidecar/extractor builds; it is re-run at M05.
**Deletion safe.** Python half after WP08 (M02); Rust half after WP16/WP17
(M04); full zero state at M05.

### DB02 — Hand-authored provider-observation schema authority

**Names.** `contracts/schema/provider-observations/pyrefly-module-v1.json` as
a hand-authored authority and its `include_str!` consumption; any observation
family absent from `schema-contract-ir.json`.
**Prerequisites.** WP01 (both observation families regularized there).
**Exit invariant.** Every observation schema is generated from the Contract IR
with a registry family code; sidecar/extractor embed generated schemas only.
**Negative proof.** `just semantic-provider-legacy-zero-state-check` and
`handwritten_observation_schema_falsification` green; the governed candidate set
covers schema `include_str!` consumers outside generated projections; `just
model-repro-check` green.
**Deletion safe.** At WP01 completion; re-verified at every milestone.

### DB03 — Opaque JSON semantic evidence as ingest input

**Names.** `type_table_json`, `callees_json`, `diagnostics_json` opaque-blob
columns as the semantic observation payload, and the MIR summary cold-blob
shape (`statement_kinds`/`terminator_kinds` string lists) as canonical
evidence.
**Prerequisites.** WP11, WP13 (typed Python observations), WP18–WP19 (typed
Rust observations).
**Exit invariant.** Every canonical semantic fact reconciles from typed,
registered observation columns; opaque JSON survives only as bounded cold
diagnostic payload where `FAB §65.4` permits, never as the reconciliation
input for a fact family.
**Negative proof.** Python half:
`just semantic-provider-legacy-zero-state-check python` plus
`py_type_enrichment_conformance`. Rust half:
`just semantic-provider-legacy-zero-state-check rust` plus
`rust_mir_body_fixture_conformance`. Default `all` structurally proves that no
reconciliation path parses free-form JSON into canonical rows and rejects stale
allow records; it is re-run at M05.
**Deletion safe.** Python half after WP13 (M02); Rust half after WP19 (M03);
full zero state at M05.

## 7. Final gate matrix

All recipes; run at M05 on one candidate commit (baseline failures recorded
per `validation-policy.md §3`):

- `just ci-fast` — all four build domains + governance
- `just root-ci-fast` · `just root-test` (nextest + doctests)
- `just extractor-ci-fast` · `just sidecar-ci-fast` · `just adapter-ci-fast`
- `just stable-graph-check` — pins, features, new Ruff-semantic dependency
- `just features-each` — feature isolation after dependency additions
- `just governance` · `just governance-scan`
- `just model-repro-check` · `just model-check`
- `just artifacts-check` · `just plan-status` · `just plan-dependency-check`
- `just design-principle-traceability-check` — DF/H namespaces and pass contracts
- `just property-registry-closure-check` — no unregistered property at publication
- `just semantic-fault-point-check` · `just semantic-observability-contract-check`
- `just semantic-profile-bench` — mandatory reproducible per-wave baselines
- `just semantic-sandbox-host-matrix-check` — every advertised production host
- `just pyrefly-module-xref-surface-check` — exact pinned Glean adapter
- `just semantic-provider-legacy-zero-state-check` — DB01–DB03 governed zero state
- `just provider-protocol-check` · `just provider-statistics-contract-check`
- `just publication-referential-integrity-check`
- `just rebuild-equivalence-check` (full corpus, both lanes)
- `just wave-acceptance-check` (W2–W7 regression)
- `just wave8-integration-check` · `just wave9-integration-check` ·
  `just wave10-integration-check` · `just wave11-integration-check` ·
  `just wave12-integration-check` — **new recipes this plan adds** (WP07,
  WP14, WP21, WP28, WP35); each gate's selection excludes its closure
  packet's operational meta-oracle (§4 group rule)
- `just gate-b-check` — released corpus regression
- `just semantic-query-conformance-check` ·
  `just semantic-query-relational-conformance-check`
- `just query-daemon-activation-check` · `just query-determinism-check`
- `just policy` · `just deps-fast` · `just typos`
- `just semantic-profile-packets-check` — deterministic aggregate over the exact
  selector set WP01–WP38 (38 unique packet IDs, including WP36–WP38), implemented
  by WP35; each packet remains checked at its proving commit and re-derived at HEAD

## 8. Execution sequence

Dependency graph (packets advance when their predecessors and milestone
gates allow):

```text
WP01 → WP36 (substrate; both lanes consume it)
Python lane: WP02 → WP03 → WP04 → WP05 → WP06 → WP07  [M01]
             WP08 → WP09 → WP38 → WP10 → WP11 → WP12 → WP13 → WP14  [M02]
             (WP08 requires WP07 + WP36)
Rust lane:   WP15 → WP16 → WP17 → WP18 → WP19 → WP20 → WP21  [M03]
             (WP15 and WP02 independently consume WP01; WP16 also requires WP36)
             WP22 → WP23 → WP24 → WP25 → WP26 → WP27 → WP28  [M04]
Integration barrier (requires M02 + M04; W7 already complete):
  WP29 → WP30 → WP31 → WP32 → WP33 → WP37 → WP34 → WP35  [M05]
DB01: Python half after WP08, Rust half after WP16/WP17, full at M05
DB02: after WP01 · DB03: Python half after WP13, Rust half after WP19
```

After WP01 freezes the lane-neutral ports, common type tables/interner,
composable generated-model inputs, and registry ownership, the Python and Rust
lanes have no semantic dependency and may overlap in wall-clock terms through
M02/M04. Within each lane and throughout the Wave 12 barrier, shared current-tree
write sets are serialized by the explicit edges above. WP38 is an early Wave 9
capability gate, not a late fallback.

Execution is wave-segmented: only the current wave group and accepted
predecessor interfaces load as active context. WP01 must split current monolithic
model inputs into deterministically composed lane-owned fragments (or equivalently
generated projections) before parallel lane work: shared top-level Contract IR,
registry catalogs, `src/analysis_context.rs`, `src/operational_store.rs`, and
`src/core_facts.rs` are then frozen interfaces owned by WP01/WP36, while Python
and Rust packets edit only their lane fragments/adapters. If activation preflight
shows an unavoidable cross-lane write to a frozen resource, add an explicit
dependency or integration packet before both packets begin; informal coordination
is not an overlap disposition. WP04→WP05, WP22→…→WP28, and
WP29→…→WP35 already serialize every known within-lane shared seam named by v1.

## 9. Plan risks and replan policy

### 9.1 Replan classification

- **Implementation adaptation** (recorded in execution state): different
  module layout than sketched; batch-size or resource-profile tuning;
  degraded capability postures the specs explicitly permit
  (conservative loans, possible-not-exact FFI, unavailable declared types
  with the documented fallback); process-per-context sidecar hosting.
- **Plan revision** (new plan version): packet boundary or sequence changes;
  a protocol/proto change beyond field completion; the Pyrefly source needs
  patching; registry schema evolution; AC-G-48 response fields requiring
  coordination with the query-form authority.
- **Design reopening** (return to the owning 1.3 spec): any `RM §28` rule —
  a discovered ambiguity or infeasibility in a normative contract
  (AC-G-14/30/31/33/35/37/40/42/48/51/71/73 shapes, the reconciliation sort
  key, the completeness algebra, profile mandatory-capability sets); any
  need for a non-built-in DataFusion mechanism (`ExtensionDecisionRecord`);
  incremental-equals-rebuild failing for structural identity reasons.

### 9.2 Named risks

1. **Predecessor drift (high likelihood, bounded impact).** The remediation
   plan is still executing on the exact files this plan extends, and its
   WP08 edits the specs this plan cites. Mitigation: `activation_requires`,
   the WP01 revalidation packet, `just plan-status` freshness derivation at
   activation, and the declared-inputs table. If accountable review changes
   a cited section, the affected packets are re-audited before execution —
   plan revision if boundaries move.
2. **Ruff traversal reproduction (the largest Wave 8 unknown).**
   `SemanticModel` population is an internal Ruff contract. Mitigation:
   WP03's replan trigger fires early (the packet starts with a bounded
   traversal spike against the fixture corpus before table work).
3. **Pyrefly unstable surfaces.** Declared/expected types and structured member
   types remain Query/TSP risks governed by WP11/WP12. Module/export/xref facts
   are not derivable from them: WP38 binds the exact public-hidden Glean report
   surface before WP10, with a mandatory plan revision if its fixture contract
   fails. The experimental query/report paths may panic; isolation and
   supervision are WP08/WP38 contracts.
4. **Nightly drift at the extractor.** The dated nightly is pinned, but the
   reference and toolchain may disagree on public-surface details.
   Mitigation: compile probes early in WP18/WP19; LD-RS-02 containment and
   the §83.6 differential for any deliberate upgrade (never mid-plan).
5. **Observation volume.** Full MIR and Python semantic streams are orders
   of magnitude beyond current Gate B fixtures. Mitigation: FAB §64 batch
   sizing with mandatory per-wave `semantic-profile-bench` baselines for provider,
   stream, derivation, reconciliation/spill, and query paths; deeper profiler/
   performance work remains risk-triggered.
6. **Sandbox platform variance.** LD-SB-01 freezes Darwin Seatbelt and Linux
   non-setuid bubblewrap 0.11.2/seccomp mechanisms. Mitigation: GI-13 fail-closed
   posture plus the host matrix; a host that fails any escape probe advertises no
   `UNTRUSTED_SANDBOXED` capability and cannot certify the complete profile.
7. **Registry/contract regeneration cadence.** Nearly every packet adds a
   lane-owned Contract IR/registry fragment; aggregate regeneration can still
   bottleneck the lanes even though their write sets are disjoint. Mitigation:
   compile fragments deterministically at packet boundaries;
   `just model-repro-check` in every packet-local gate keeps the two-root
   reproduction honest.
8. **Scope pressure toward Wave 13.** CFG work invites dominators; access
   events invite alias analysis. GI-14 and the WP35 structural check are the
   hard boundary; any materialized derived family beyond the registry is a
   defect, not initiative.

### 9.3 State contract

Execution state lives at
`docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json`
(schema version 2, judgment fields only), created exclusively by
`just plan-activate` after this plan reaches `approved` and the predecessor
freezes. Proving commits are mandatory for `complete`; check outcomes are
never stored — they re-derive via the named oracles and
`just packet-oracle-check`. Deviations, failed approaches, discovered
obligations, and blockers are recorded per packet as they occur.
