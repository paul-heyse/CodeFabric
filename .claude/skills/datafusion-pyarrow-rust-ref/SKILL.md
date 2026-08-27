---
name: datafusion-pyarrow-rust-ref
description: "Reference navigator for CodeFabric's Rust data fabric — DataFusion 55.0.0 + Arrow/Parquet 59.2.0 APIs bound to the data-fabric design constitution. SKILL.md maps four documents at `docs/library_ref/`: `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` (sessions, RecordBatch/schema, Expr, logical+physical planning, TableProvider/ScanArgs, statistics, Parquet, object_store, UDF/UDAF/UDTF; §0–§40A + S1–S15 + §41–§56 + C1–C13 + upgrade gates V1–V6), `arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md` (crate topology, buffers/zero-copy, arrays/builders, kernels, IPC/Feather, Parquet, object-store, Flight/ADBC, PyO3/PyCapsule, Substrait, extension types; §0–§28), `full_data_fabric_design_principles.md` (Principles 1–25 + agent constitution), and `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md` (P1–P25 alignment, 150 utilization patterns in 14 families MOD/ARR/SCH/CAT/EXP/LOG/PHY/SRC/RUN/INT/OBS/GOV/EXT/TST, 8 requirement flows, 11 design artifacts). REFERENCE.md (same folder) holds per-document section indexes with line numbers, the pattern→section binding matrix, the P1–P25 binding table, decision trees, and operating rules. Use when Rust or Cargo touches `datafusion`/`arrow`/`arrow_*`/`parquet`/`object_store`, authors `SessionContext`/`RecordBatch`/`SchemaRef`/`Expr`/`LogicalPlan`/`ExecutionPlan`/`TableProvider`/`ScanArgs`/`ScalarUDFImpl`/`GroupsAccumulator`/`RowConverter`, or when a data-fabric functional outcome must map to principles P1–P25 or MOD-/ARR-/SCH-style utilization patterns. Delta tables → sibling `deltalake-rust-ref`; fact providers → `code-facts-lib-ref`; graph analytics → `petgraph-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# DataFusion 55 + Arrow 59 Reference Navigator

This skill routes the four documents that together govern data-fabric work: two API
authorities, the design constitution, and the alignment manual that joins them. Its job
is one sentence: **for any functional outcome, route outcome → alignment-manual flow →
utilization-pattern IDs → design principles → the exact reference sections documenting
the API surface.** This SKILL.md is the core map — version anchors, the document pack,
the mandatory outcome-mapping loop, scenario routing, invariants. The companion
[REFERENCE.md](REFERENCE.md) is the mechanical layer — per-document section indexes with
verified line numbers, the pattern→section binding matrix (`REFERENCE §2`), the P1–P25
binding table, decision trees, and operating rules.

**Out of scope.** Delta table loading, exact versions, writes, transactions,
checkpoints, optimize, and vacuum → `deltalake-rust-ref` (provider-side integration
stays here). Fact-extraction providers → `code-facts-lib-ref`. Graph analytics →
`petgraph-ref`. Canonical JSON bytes and digests → `canonicalization-lib-ref`. The
Python/FastMCP adapter is presentation-only and never gains an Arrow/DataFusion
processing role.

## Version anchors

- DataFusion `=55.0.0`, Arrow/Parquet `=59.2.0`, `object_store` `=0.13.2`, Rust 1.95,
  edition 2024. Read the pins from FAB §2.1 and the session context — never infer them
  from examples in any reference.
- Both API documents import deep dives that predate the final source verification. The
  comprehensive reference's **§40A** (source-verified capability reconciliation) and the
  Part III upgrade gates **V1–V6** outrank any conflicting prose; note **V6 sits
  physically between V4 and V5**. The Arrow reference's own version matrix and refresh
  notes play the same role there.
- The Arrow reference's migration-ledger sections deliberately describe
  predecessor-release syntax for comparison; they are never current-API citations, and
  their version strings must not be copied into tracked files (REFERENCE §4 rule 4).

## The four documents

| Alias | Path (under `docs/library_ref/`) | Lines | Scope |
|---|---|---:|---|
| `df` / `df-schema` / `df-plan` / `df-calc` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | 115,587 | DataFusion authority: §0–§40 + §40A · S1–S15 · §41–§56 · C1–C13 · gates V1–V6 |
| `arrow` | `arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md` | 34,372 | Arrow-crate authority: §0–§28 — buffers, arrays, kernels, IPC, Parquet mechanics, Flight/ADBC, PyO3/PyCapsule, Substrait, extension types |
| `principles` | `full_data_fabric_design_principles.md` | 1,327 | the design constitution: Principles 1–25, §28 design questions, §29 anti-patterns, §30 compact constitution |
| `align` | `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md` | 2,294 | the join: P1–P25 alignments, 150 pattern IDs, 8 flows, 11 required design artifacts, crosswalks, App. A version leverage |

**Superseded and historical.** `datafusion_rust.md`, `datafusion_planning_rust.md`,
`datafusion_schemas_rust.md`, `datafusion_calculations_rust.md`, and `pyarrow_rust.md`
document the predecessor stack; a same-day DataFusion-only split of the comprehensive
reference was superseded by it and has been removed. None of these is ever an API
authority. If a routed document is missing from the tree, say so explicitly and degrade
— never reconstruct pattern IDs, principle numbers, or section claims from memory.

## Why these documents travel together

The two API authorities have **zero heading overlap**: `df` owns everything
DataFusion-integrated (sessions, planning, providers, UDFs, configuration); `arrow` owns
crate-level Arrow mechanics and every interoperability protocol (REFERENCE §3a arbitrates
the overlap topics: Parquet, UDFs, IPC, object stores). `principles` says what shape a
design must have. `align` is the join — but it cites only its own IDs (P-numbers and
pattern IDs), no filenames and no section numbers, so **REFERENCE §2 supplies the
missing binding** from every pattern family to the sections of `df` and `arrow` that
document its API surface. Reading any one of the four without the others is the failure
mode: working code that violates the constitution, or principled design with no
resolvable API path.

## The outcome-mapping loop

Before implementing any data-fabric functional outcome, run this loop — the manual
labels its review flow mandatory (`align` §1.2); this skill makes it findable:

1. Classify the outcome into one or more of the eight `align` Part III flows
   (REFERENCE §3b is the classifier). Most real outcomes hit 2–4 flows.
2. Walk each flow's decision tree and take its literal `### Required selections`
   pattern IDs; union them across flows.
3. Read each selected ID's `align` Part II row: required leverage, primary principles,
   minimum evidence.
4. Read `align` Part I for the named P-numbers; open `principles` for depth on any
   principle whose application is unclear (cite by principle number — REFERENCE §4
   rule 3).
5. Resolve the API surface through REFERENCE §2's binding matrix and read the cited
   `df`/`arrow` sections (zoom with `just lib-outline <path> --view names` first).
6. Interpret every recommendation through the `align` §0.1 capability-status legend —
   a NATIVE ENFORCEMENT claim needs different evidence than an APPLICATION OVERLAY
   (REFERENCE §3d).
7. Check `align` §1.3 stop conditions; if any holds, stay at design.
8. For material subsystems, produce the `align` §1.2 artifacts
   (`SemanticRequirement` → … → `ImplementationPacket`) — cite the twelve steps, do not
   restate them.
9. Check the proposed shape against `align` Part VII (anti-pattern → prescribed
   correction) and Appendix A for version-specific leverage of the pinned 55/59 stack.
10. Derive the evidence plan from each pattern's minimum-evidence column
    (`align` Part VI checklists; TST family).

## Reading paths by problem context

### 1. Defining or changing what a table or field means
`align` §4 (schema flow) → SCH-01–04 always; SCH-05–07 for metadata/constraints;
SCH-08/-11 for physical adaptation. APIs: `df-schema` S1–S8 · `df-plan` §44 ·
`arrow` §3. Evidence: TST-01, and the compatibility classification matrix.

### 2. Computing a new value, predicate, or aggregate
`align` §5 (calculation flow) → EXP-01/-02 before any custom function; then the
semantic-family ladder (EXP-03–EXP-10). APIs: `df` §11–§12, §24 · `df-calc` C1
(decision tree) then C5–C13 · `arrow` §7–§8. Evidence: TST-05 (+TST-10 if stateful).

### 3. Reading from or writing to a new source
`align` §6 (provider flow) → CAT-03–07 for every custom provider; SRC-01–10 as
applicable; CAT-08/GOV-09 for writes. APIs: `df` §14–§18 · `df-schema` S10 ·
`df-plan` §51 · `arrow` §13–§14. Evidence: TST-02–04, TST-10.

### 4. Changing what queries express or how they compile
`align` §7 (plan flow) → LOG-01–07 for governed planning; LOG-08 only for genuinely
new relational meaning. APIs: `df` §19, §22, §25–§26 · `df-plan` §41–§49. Evidence:
TST-06, TST-08, TST-11.

### 5. Changing how execution runs — speed, memory, partitioning
`align` §8 (physical flow) → PHY-01–09 for custom nodes; PHY-10–12 as applicable;
RUN-04/-05/-09/-10. APIs: `df` §20–§21, §26, §28–§29, §40A · `df-plan` §50–§54.
Evidence: TST-07, TST-10, TST-11.

### 6. Crossing a process, language, engine, or file boundary
`align` §9 (interop flow) → one or more of INT-01–09; INT-10 always for public or
durable boundaries. APIs: `arrow` §10–§12, §19–§21, §25–§26 · `df` §36. Evidence:
TST-08, TST-09.

### 7. Deciding who may see or do something
`align` §10 (governance flow) → GOV-01–10 as applicable; enforcement lives at the
owning authority (Principle 13). APIs: `df` §17–§18, §39 · `df-schema` S15 ·
`df-plan` §46 · `df-calc` C3. Evidence: TST-12 is mandatory for governed systems.

### 8. Explaining, reproducing, or auditing what happened
`align` §11 (provenance flow) → OBS-01–12 by criticality; SCH-10, MOD-06, RUN-10.
APIs: `df` §30 · `df-plan` §55–§56. Evidence: TST-11, TST-14. Delta commit metadata →
`deltalake-rust-ref`.

### 9. Dependency, pin, or feature change on this stack
Not a manual flow. `df` §1, §34, §40A and gates V1–V6 · `arrow` §1–§2 ·
`align` A.3. Run the repo's own gates (`just stable-graph-check`, `deps-fast`,
`features-each` per the AGENTS.md §8 risk table).

### 10. Diagnosing behavior change after an upgrade
Not a manual flow. `df` §33–§35, §40A, then the V1–V6 walk (REFERENCE §3e). Compare
result meaning — rows, schema, checksum — before optimizer/operator names.

## Key invariants

1. **One Arrow type universe.** Every crate exchanging Arrow types matches the exact
   59.2.0 family across the stable root and the extractor IPC boundary (INT-10, TST-14;
   `scripts/stable_graph_check.sh`).
2. **`RowConverter` bytes are durable.** They participate in application checksums;
   never refresh a checksum merely because a library version changed (MOD-06, OBS-09;
   gate V3).
3. **A query pins one snapshot.** One immutable snapshot, exact Delta versions; plan
   text and metrics are diagnostics, not semantic identity (OBS-08; Principle 11).
4. **Provider wrappers are explicit.** The application-owned `TableProvider` wrappers
   make DataFusion 55 structured scan and statistics behavior explicit — default trait
   methods are not evidence that wrapper semantics were preserved (CAT-06, CAT-07;
   `df` §40A).
5. **Highest viable extension level.** Climb down the `align` §2.3 ladder only when
   semantics require it, and record why (Principle 14; EXT-01–10; REFERENCE §3c).
6. **Capability claims are truthful.** Pushdown, statistics, ordering, volatility,
   strictness: exact/inexact/unsupported/absent, never optimistic (Principle 20;
   CAT-05, EXP-05, PHY-03; `align` §0.1).
7. **One authority per concept.** Every derived form — cache, projection, serialized
   plan, metadata tag — names the authority it derives from (Principle 3; `align` §2.2).

## Project context: CodeFabric

The source map (repo file → role → dominant pattern families) and the spec anchors live
in REFERENCE §5. Typed `Expr` construction only — never splice untrusted strings into
predicates. `DataFrame` is lazy: bound terminal materialization and memory. Distinguish
plan-side `DFSchema` from runtime Arrow `Schema` and preserve field metadata and
nullability at boundaries. For fabric design documents, the `align` Part IV artifacts
are the expected design-record shape.
