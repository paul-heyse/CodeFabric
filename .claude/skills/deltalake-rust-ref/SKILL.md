---
name: deltalake-rust-ref
description: "Reference navigator for CodeFabric's Delta state layer — Rust `deltalake` 1.0.0 at exact revision `43a0cf10` on DataFusion 55.0.0 and Arrow/Parquet 59.2.0, bound to the v2 data-fabric constitution. SKILL.md maps the exact delta-rs reference, `full_data_fabric_design_principles_v2.md` (staticness test, P1–P36), and the Delta alignment manual (its P1–P25 capability mapping remains library evidence; v2 P26–P36 apply directly). Use when Rust or Cargo touches `deltalake`, exact snapshots/versions, writes/DML/CDF/layout/maintenance, or maps durable table-state outcomes to the v2 principles and MOD-/STA-/TXN-style patterns. Non-Delta DataFusion and Arrow APIs → sibling `datafusion-pyarrow-rust-ref`; fact providers → `code-facts-lib-ref`; graph analytics → `petgraph-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# delta-rs 1.0.0 @ `43a0cf10` Reference Navigator

This skill routes the three documents that together govern Delta work: one API authority,
the design constitution, and the alignment manual that joins them. Its job is one
sentence: **for any durable table-state outcome, route outcome → alignment-manual flow →
utilization-pattern IDs → design principles → the exact reference sections documenting the
API surface.** This SKILL.md is the core map — version anchors, the document pack, the
mandatory outcome-mapping loop, scenario routing, invariants. The companion
[REFERENCE.md](REFERENCE.md) is the mechanical layer — the chapter index with verified
line numbers, a symbol index (`REFERENCE §1.2`), the pattern→section binding matrix
(`REFERENCE §2`), the P1–P25 binding table, decision trees, and operating rules.

**Out of scope.** `SessionContext`/`SessionState`/`RuntimeEnv` semantics, `Expr`
construction, logical and physical planning, the optimizer, UDFs, statistics contracts,
Arrow arrays/kernels/IPC, and the `object_store` crate itself → `datafusion-pyarrow-rust-ref`
(Delta-side provider, scan config, and write-from-plan integration stay here).
Fact-extraction providers → `code-facts-lib-ref`. Graph analytics → `petgraph-ref`.
Canonical JSON bytes and digests → `canonicalization-lib-ref`. The Python/FastMCP adapter
is presentation-only and never gains a Delta, Arrow, or DataFusion processing role.

## Version anchors

- `deltalake` / `deltalake-core` `1.0.0` at git revision
  `43a0cf10a313e5077c48637ad786a05359136bbb` (pre-release pin), DataFusion `=55.0.0`,
  Arrow/Parquet `=59.2.0`, `object_store` `=0.13.2`, Rust 1.95.0, edition 2024. A branch
  name, the declared crate version, or a nearby `main` is not interchangeable with that
  revision. Read the pins from `FAB §2.1` and the session context — never infer them from
  examples in any reference.
- **Rust floor.** Both the alignment manual's banner and `delta` §0.10's matrix state Rust
  `1.94.1`. That is delta-rs's own minimum. The CodeFabric floor is `1.95.0`, set by the
  Ruff 0.0.7 provider train (`FAB §2.1`, `Cargo.toml`). Do not copy `1.94.1` into a
  tracked file as this repository's pin.
- **`delta` §0.10's alignment matrix lists Arrow 58 and Parquet 58 — a known typo**, and
  both rows self-correct in place with "(resolves to `59.2.0`)". The target banner, the
  Cargo snippets in the same subsection, the exact source, and `FAB §2.1` are
  authoritative, and that subsection's own prose says application pins must be `=59.2.0`.
- **`DeltaOps` is deprecated at this revision** in favour of methods on `DeltaTable`
  (`DeltaTable::write`, `delta_table.create()`); `delta` §5's chapter preamble and §8.2 both
  say so, and `WriteBuilder` now awaits to `Result<DeltaTable, DeltaTableError>` rather than
  a `(table, metrics)` tuple. Older `DeltaOps(table).write(...)` examples are legacy style.
- **Two `sqlparser` versions in the resolved graph are expected**, not a defect:
  `deltalake-core` declares `0.61.0` and DataFusion 55 declares `0.62.0` (`delta` §0.10).
  This repository registers the exact skip in `deny.toml`; do not "fix" it in
  `cargo tree -d` output.

## The three documents

| Alias | Path (under `docs/library_ref/`) | Lines | Scope |
|---|---|---:|---|
| `delta` | `deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md` | 17,270 | the API authority: §0 baseline · §2 deployment/storage · §3 loading and time travel · §4 schema and Arrow mapping · §5 writes · §6 DataFusion reads · §7 integration track (§7.0–§7.13) · §8 create-table · §9 DML · §10 CDF · §11 constraints/protocol · §12 layout/pruning · §13 optimize/vacuum/restore. **There is no §1.** |
| `principles` | `full_data_fabric_design_principles_v2.md` | 2,189 | the current constitution: staticness test, P1–P36, design questions, anti-patterns, compact constitution, and v1→v2 delta |
| `delta-align` | `deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md` | 2,903 | the join: P1–P25 alignments, 156 pattern IDs in 16 families, 13 flows, 13 design artifacts, crosswalks, 11 review checklists, a 20-row anti-pattern table, App. A version leverage, **App. B Delta authority matrix**, App. C table-class defaults, **App. D release gate** |

**Superseded and historical.** `deltalake_rust_1.0.0_9f922319_advanced_reference_2026-08-20.md`
documents the predecessor revision. It is comparison material only and is never an API
authority; `delta` §0.16 is the reviewed net-change map between the two revisions. If a
routed document is missing from the tree, say so explicitly and degrade — never
reconstruct pattern IDs, principle numbers, or section claims from memory.

## Why these documents travel together

`delta` tells you what the library does at this exact revision. `principles` says what
shape a design must have. `delta-align` is the join — but it cites only its own IDs
(P-numbers and pattern IDs), no filenames and no `delta` section numbers, so
**REFERENCE §2 supplies the missing binding** from every pattern to the sections that
document its API surface. Reading any one of the three without the others is the failure
mode: valid Delta tables that violate the constitution, or principled design with no
resolvable API path.

`principles` is shared with `datafusion-pyarrow-rust-ref`: one constitution, two alignment
manuals. Their capability legends are **not** interchangeable — `delta-align` §0.1 has
eight statuses (adding NATIVE AUTHORITY, NATIVE STATE TRANSITION, NATIVE OBSERVABILITY,
INTEGRATION CONTRACT) where the DataFusion manual has six.

## The outcome-mapping loop

Before implementing any durable table-state outcome, run this loop — the manual labels its
review flow mandatory (`delta-align` §1.2); this skill makes it findable:

1. Classify the outcome into one or more of the thirteen `delta-align` Part III flows
   (REFERENCE §3b is the classifier). Most real outcomes hit 2–4 flows.
2. Walk each flow's decision tree and take its literal `### Required selections` pattern
   IDs; union them across flows. Answer its `### Agent questions` — they are where
   "latest", "atomic", and "supported" get pinned down.
3. Read each selected ID's `delta-align` Part II row: required leverage, primary
   principles, minimum evidence.
4. Read `delta-align` Part I for the named P-numbers; open `principles` for depth on any
   principle whose application is unclear (cite by principle number — REFERENCE §4
   rule 6).
5. Resolve the API surface through REFERENCE §2's binding matrix and read the cited
   `delta` sections — jump straight from a symbol with REFERENCE §1.2, or zoom with
   `just lib-outline <path> --view names` first.
6. Settle authority explicitly against `delta-align` App. B before writing anything that
   caches, derives, or republishes table state. §0.2–§0.4 draw the same line in prose.
7. Interpret every recommendation through the `delta-align` §0.1 eight-status legend — a
   NATIVE AUTHORITY claim needs different evidence than an APPLICATION OVERLAY
   (REFERENCE §3d).
8. Take the highest operation level that preserves the semantics (`delta-align` §2.3,
   bound to sections in REFERENCE §3c) and record why in an `OperationSelectionRecord`.
9. Check `delta-align` §1.3's twelve stop conditions; if any holds, stay at design.
10. For material subsystems, produce the `delta-align` Part IV artifacts — cite the
    thirteen §1.2 steps, do not restate them, and note that five §1.2 output names have no
    Part IV template (REFERENCE §1.4).
11. Check the proposed shape against `delta-align` Part VII (anti-pattern → Delta symptom
    → prescribed correction) and Appendix A for version-specific leverage of this pinned
    revision.
12. Derive the evidence plan from each pattern's minimum-evidence column
    (`delta-align` Part VI checklists §37–§47; TST family) and close with App. D's release
    gate (REFERENCE §3e).

## Reading paths by problem context

Each entry is `delta-align` flow → its literal `### Required selections` IDs → the `delta`
sections that document them. The ID sets below are the manual's, not a paraphrase; when a
flow qualifies a group ("as applicable", "when using DataFusion"), that qualification is
carried through.

### 1. Creating a table, or changing what it means
`delta-align` §8 → MOD-01, SCH-01–05, SCH-08–10; GOV-01–03 as applicable; INT-05 for any
advanced feature. APIs: `delta` §4.3, §4.29–§4.31 · §8.2–§8.9 · §11.3, §11.8–§11.9.
Evidence: TST-01, TST-03, TST-14.

### 2. Reading a table — snapshot, freshness, time travel
`delta-align` §9 → MOD-02, STA-01–10; QRY-01–05 when using DataFusion; OBS-03, OBS-08.
APIs: `delta` §3.3–§3.9, §3.15–§3.18 · §6.24–§6.25. Evidence: TST-02, TST-08.

### 3. Appending or replacing data
`delta-align` §10 → MOD-03, TXN-01–08, WRT-01–08; SCH-03, SCH-06, SCH-08; OBS-01,
OBS-04–06. APIs: `delta` §5.3–§5.13, §5.16–§5.18 · §7.7 for plan-backed writes.
Evidence: TST-04, TST-05.

### 4. Mutating rows: delete, update, merge
`delta-align` §11 → DML-01–08, TXN-01–07; GOV-01, GOV-02, GOV-05, GOV-06. APIs: `delta`
§9.4–§9.14, §9.19, §9.22–§9.24 · §7.5 for the expression seam. Evidence: TST-05, TST-06.

### 5. Migrating schema, properties, protocol, or features
`delta-align` §12 → SCH-06–12, GOV-05–10, INT-05–08. APIs: `delta` §4.12–§4.14,
§4.25–§4.27, §4.37 · §11.12–§11.14, §11.19–§11.20, §11.27. Evidence: TST-01, TST-03,
TST-14.

### 6. Consuming changes incrementally
`delta-align` §13 → CDF-01–10, GOV-03, GOV-08; OBS-07, OBS-10. APIs: `delta` §10.3–§10.10,
§10.13, §10.18–§10.20, §10.29. Evidence: TST-07, TST-12.

### 7. Serving queries through DataFusion
`delta-align` §14 → QRY-01–10, STA-03, MOD-07; LAY-01–07; OBS-01, OBS-03, OBS-08, OBS-09.
APIs: `delta` §6.5, §6.10–§6.18, §6.24–§6.25, §6.36 · §7.1–§7.2, §7.8. Non-Delta planning
and execution questions → `datafusion-pyarrow-rust-ref`. Evidence: TST-08.

### 8. Reading specific known files — repair, quality, targeted work
`delta-align` §15 → LAY-09, LAY-10, QRY-10, EXT-05. APIs: `delta` §6.35 · §3.12–§3.13 ·
§12.8, §12.20. Evidence: TST-08, TST-13.

### 9. Changing physical layout without changing meaning
`delta-align` §16 → LAY-03–07, MNT-01–03, MNT-10; QRY-06, OBS-05. APIs: `delta`
§13.4–§13.9, §13.12 · §12.17–§12.19, §12.24–§12.25. Evidence: TST-09, TST-13 — logical
equality is the gate.

### 10. Physically deleting old files
`delta-align` §17 → GOV-08, GOV-09, MNT-04–06; OBS-01, OBS-05, OBS-10. APIs: `delta`
§13.13–§13.18, §13.25 · §10.18 for the CDF boundary. Evidence: TST-09, TST-12. This is
destructive — see invariant 7.

### 11. Rolling back or repairing an incident
`delta-align` §18 → MNT-07, MNT-08, GOV-09; OBS-01, OBS-05, OBS-10. APIs: `delta`
§13.19–§13.21. Evidence: TST-09.

### 12. Changing storage backend, credentials, or deployment
`delta-align` §19 → STO-01–10, TXN-08. APIs: `delta` §2.1, §2.4–§2.6, §2.10,
§2.16–§2.18, §2.23 · §0.6–§0.7. Evidence: TST-10, TST-14.

### 13. Explaining, reproducing, or auditing what happened
`delta-align` §20 → OBS-01–10, MOD-06, MOD-07; STA-03, TXN-05, TXN-06. APIs: `delta`
§3.11, §3.15 · §5.13 · §9.18 · §13.17. Plan artifacts and query fingerprints →
`datafusion-pyarrow-rust-ref`. Evidence: TST-11, TST-12.

### 14. Dependency, pin, or feature change on this stack
Not a manual flow. `delta` §0.1–§0.4, §0.6–§0.10, §0.15–§0.16 · §2.15 ·
`delta-align` A.1, A.3, App. D. Run the repository's own gates
(`just stable-graph-check`, `deps-fast`, `features-each` per the `AGENTS.md` §8 risk
table).

### 15. Diagnosing behavior change after a revision bump
Not a manual flow. `delta` §0.16 is the reviewed net-change map; then the twelve
latest-pin chapter notes (REFERENCE §4 rule 5) and `delta-align` Appendix A's A.1–A.18.
Compare result meaning — versions, rows, schema, checksum — before file counts or plan
text.

## Key invariants

1. **Coordination is application-owned.** Keep
   `CommitProperties::default().with_max_retries(0)`: CodeFabric owns retries,
   application transactions, predecessor checks, and unknown-outcome reconciliation
   (TXN-02–04, TXN-07; `delta` §5.17, §9.22). Neither `with_max_retries` nor
   `with_application_transaction` is documented in `delta` — verify those two against the
   pinned source, never against a fabricated section.
2. **A query pins one immutable snapshot.** A `DeltaTable` handle or a provider is a
   pinned view, not "latest forever"; every query binds exact table versions and never
   mixes publications (STA-03/-10, QRY-03/-05, MOD-07; `delta-align` App. B).
3. **Never bypass the transaction log.** Read Delta through delta-rs schema and physical
   adaptation; raw Parquet reads and `object_store` listings are not table state
   (QRY-01, STO-02, SCH-11; `delta-align` Part VII row 1).
4. **Query-serving handles retain full statistics.** Metadata-only, `without_files`, and
   `with_skip_stats` profiles are separate deliberate constructions with their own cost
   model (STA-06/-07, LAY-07; `delta` §3.4, §3.28, §12.12).
5. **Reopen validation fails closed** on unapproved CDF, deletion vectors, type widening,
   protocol versions, or table features. Protocol recognition is not operation support
   (GOV-05/-06, SCH-08–09, INT-05; `delta` §11.20, §11.27).
6. **Local workstation authority excludes `deltalake-aws` and AWS SDK packages**; only the
   `s3-storage` feature activates that implementation. Kernel-forced latent `object_store`
   cloud features are reported, not mistaken for runtime authority (STO-08, INT-10;
   `scripts/stable_graph_check.sh`).
7. **Vacuum is destructive to retained versions.** Dry-run and reference/lease safety
   remain mandatory, and this skill does not authorize production vacuum orchestration
   (MNT-04–06, GOV-08/-09; `delta` §13.13, §13.18, §13.25).
8. **Capability claims are truthful.** Feature support, pruning, statistics, retention
   reach: exact/inexact/unsupported/absent, never optimistic (Principle 20; GOV-10,
   LAY-04, OBS-10; `delta-align` §0.1).
9. **One authority per concept.** Every derived form — provider, cache, file inventory,
   checkpoint, publication manifest — names the authority it derives from
   (Principle 3; `delta-align` §2.2 and App. B).

## Project context: CodeFabric

The source map (repo file → role → dominant pattern families), the repository constraints,
and the spec anchors live in REFERENCE §5. A publication maps to an exact Delta version
per table, and intermediate versions stay invisible through `cpg_serving` until the
pointer advances; the manual's Part IV artifacts are the expected design-record shape for
Delta work.
