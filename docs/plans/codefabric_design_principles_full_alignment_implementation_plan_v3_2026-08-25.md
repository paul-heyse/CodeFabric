---
artifact: implementation-plan
plan_id: codefabric-design-principles-full-alignment
version: v3
date: 2026-08-25
status: approved
design_path: docs/reviews/design_principles_remediation_proposal_2026-08-25_v2.md
design_version: v2
baseline_commit: dd3c0056ce2c01d04c28605b043a9316a6c26383
working_tree_digest: 7bb3c41c7b77cb6d4e98ed0b0e283286d1153567775d06210b9d59a21c26b3eb
state_path: docs/plans/state/codefabric-design-principles-full-alignment_v3_state.json
cutover: true
---

# CodeFabric design-principles full alignment — implementation plan v3

This plan executes remediation moves R1–R11 of the audited design v2. It
supersedes plan v2 solely because WP73's accepted normative amendments changed
five declared-input digests and added the detector-backed current register.
Packet IDs, dependencies, target design, library decisions, and proof
obligations are unchanged. WP73 is already proved at `c133c9e`; full alignment
is certified only after the resulting active worklist is implemented and
proved.

Citation tags follow the proposal: `PRIN P-n` (data-fabric principles),
`CONF DP-nnn` (conformance register), `ALIGN` (alignment manual and its
pattern IDs), `DFREF §n` (DataFusion 55 comprehensive reference), `ARROW §n`
(Arrow 59 reference), `HOL P-n` (holistic doctrine), plus the design-corpus
tags (`SUITE`, `GEN`, `FAB`, `QRY`, `LIFE`, `SRV`, `RM`).

**Doctrine precedence (recorded decision).** The data-fabric design
principles (`PRIN P1–P25`) are the governing doctrine for this program, by
user direction at planning time. `semantic_design_principles_holistic.md`
remains a declared input and is cited only where it adds an obligation PRIN
does not already own — pass contracts (`HOL §6`), the failure taxonomy
(`HOL P23`), and lifecycle ownership (`HOL P22`). Where the two could be read
to differ, PRIN wins.

## 1. Outcome and non-goals

### 1.1 Outcome

At M14, the CodeFabric tree satisfies the proposal's §5 end-state matrix:

1. the P2/P10/P14/P19/P21 and DP-051 decisions in design v2 §6 are accepted
   into their normative homes, and a detector-backed superseding conformance
   register owns the current DP worklist (R11);
2. every registry, semantic fingerprint domain, identity recipe, and enum
   vocabulary has exactly one authority in `contracts/`; integrity, cache, and
   keyed security hashes have separate narrow authorities (R1);
3. the semantic query plane compiles all eight request forms and arbitrary
   composition into a typed execution DAG containing DataFusion relational
   plans and application graph plans — no SQL text, hidden graph semantics, or
   compile-time-constant result states — and is reachable from
   `daemon::serve` (R2);
4. canonical delivered order precedes pagination; `ResultChecksumV1`, plan
   template identity, semantic query identity, and execution/request identity
   are distinct versioned contracts, and reproducibility is modeled (R3);
5. every governed execution persists a `QueryPlanArtifact` bundle, carries an
   execution identity allocated before planning, and resolves provenance
   closure from any durable result; metrics come from the one served physical
   plan without `EXPLAIN ANALYZE` re-execution (R4);
6. providers share one validated Arrow batch contract: bounded direct streams
   in process and validated IPC across subprocess boundaries, converging on
   one ingest pipeline (R5);
7. serving `TableProvider`s advertise only truthful effective-relation
   statistics, pushdown, and constraints; cross-table references are enforced
   over candidate publication state; `codefabric.id16` is an
   application-enforced Arrow extension metadata contract (R6);
8. every fabric error carries a lifecycle phase and a registry-closed public
   code; state-machine guards are evaluated, not merely legal (R7);
9. the daemon boundary is one proto-authoritative contract family behind
   keyed tokens and enforced leases, presented by a strictly pass-through
   adapter (R8);
10. Gate B executes end-to-end against independently reviewed,
    owner-accepted versioned golden answers; AC-G-79 uses exact schema and bag
    equality; oracle substance is governed before the first proving commit
    (R9); and
11. the model-compiler plane's checks can fail, its derived artifacts are
    produced by something, and its vocabularies have one registration (R10).

### 1.2 Non-goals

- No tenancy model, masking/classification metadata, advisory display
  channel, or user-facing expression surface — the register's divergence
  ledger stays closed (proposal §9).
- No UDF, custom `ExecutionPlan`, `PhysicalExpr`, `LogicalPlan::Extension`,
  or custom planner (`ALIGN EXT-04`–`EXT-10` unselected; `PRIN P14`).
- No Substrait, Flight, or ADBC adoption; subprocess boundaries remain UDS
  gRPC + Arrow IPC while in-process providers use Arrow batches.
- No new Cargo roots, no root `[workspace]`, no second top-level test target,
  no native Python extension, no Python Arrow/DataFusion processing layer.
- No Delta protocol/table feature enablement (CDF, deletion vectors, type
  widening, column mapping); the evolution-policy contract in WP57 *declares*
  the pin, it does not relax it.
- No version movement: DataFusion =55.0.0, Arrow/Parquet =59.2.0,
  `object_store` =0.13.2, delta-rs `43a0cf10` are fixed inputs; any movement
  is a replan event.
- No rewriting of completed plans, state files, or historical reviews; the
  stale status artifact named by CONF DP-107 is superseded, not edited.

### 1.3 Baseline disposition

Baseline commit `dd3c005` records the accepted and trusted WP73 checkpoint.
The recorded `working_tree_digest` is derived by
`git diff HEAD | shasum -a 256` at v3 planning time and covers the five
preserved repository-owner paths recorded in
`contracts/governance/design-principle-baseline.yaml`: two DataFusion skill
updates, the legacy `datafusion_rust.md` deletion, library routing, and the
seed zero-state edit. They remain separately owned until their planned WP54
disposition; v3 neither reconstructs nor silently absorbs them.

The pass-3 register remains a declared historical input. The current worklist
is the accepted detector-backed v2 conformance review and its two machine
registries. WP73 proved all 25 principle ownership rows, executed all 124
detectors, attributed the complete dirty/deleted/untracked path set, and made
Typos green without rewriting immutable v1 history.

F-003's atomic activation contract is already landed. V3 activation creates
and validates its schema-2 state before switching `active-plan.json`; the v2
state remains immutable execution history after it is marked superseded.

## 2. Source design and declared inputs

The source design is remediation proposal v2, created because the audit
verdict was `needs-redesign`. Its §4 table preserves the v1 mapping only as
provenance; WP73's superseding conformance register becomes the executable
finding-to-move map. This plan's packet **Design references** cite stable moves
and historical findings, but execution scope is conditioned on that re-derived
classification.

| path | sha256 |
|---|---|
| docs/reviews/design_principles_remediation_proposal_2026-08-25_v2.md | 9c0fc5067fc6f845082e6425c9eca0baff39f7883c9e4e4e21779cedda760674 |
| docs/reviews/plan_audit_codefabric_design_principles_full_alignment_implementation_plan_v1_2026-08-24_2026-08-25_v1.md | 894c920cbdf2587bd9a162a21ba8e80877bbfa82812b688620d89ee140f5f413 |
| docs/reviews/design_principles_conformance_2026-08-23_v1.md | 9d3ec5bcd8569a8acc8900162f8859546dea4778951f932b751ec99a6c832fe5 |
| docs/reviews/design_principles_conformance_2026-08-25_v2.md | 60eb552ef68bfbc012cf71ca676f9402dcb730f8e63236aa0bbdf6a0963b8258 |
| contracts/registry/design-principle-registry.yaml | 58cac43c481cd4bc109dd94fe909aa7906d0b0d090264c660c04e6773c1b3359 |
| contracts/registry/design-principle-detector-registry.yaml | 4509f4973245224f83adc3165c6256e742f24d4844156a06755cd2753cf5d476 |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 992b2e074dd24fd7725f22ab4242d46fbf8517c5bb23d78da792dc4990fe8ed8 |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 83c7f0ecc6ab81ef97cdc21f7087a56b870cf4e18e16902870d045f23f747b45 |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | f892b6a18fa07e914ff3829937bd6bdfcb7632b4abebfed2dec51c0fa7a09647 |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | 7580539f8699dce350fa2437fe3784d433fbcf6bc4f3b6a690144f825dc0d194 |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | 5a19a908db15dbf72fa6454f9a712944efc497c1e7d9f166ffbd9023558f1d3a |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/petgraph.md | 8f5b19b2d9fbb9dfe2caf974b2a1f4c55b9244cfd167eb48956d225a076cccd9 |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |

### 2.1 Library decisions

The proposal settles the library approach; this plan records the load-bearing
decisions with their evidence status. **Verified** = confirmed in-tree or in
the pinned reference this session; **probe** = a compile/API probe is a
preflight obligation of the owning packet.

| ID | Decision | Status | Authority |
|---|---|---|---|
| LD-01 | `QuerySpec` compiles to a typed semantic DAG: relational nodes use `LogicalPlanBuilder`/`Expr`/`DFSchema`; graph/path/pattern nodes use an application `GraphOperatorPlan`; no SQL text or opaque DataFusion extension | verified as the `QRY`-conformant target; packet proof required | QRY §§4, 15–17, 21, 106–107; DFREF §43, §46; ALIGN MOD-02/03, LOG-01–07 |
| LD-02 | Provider boundary is validated Arrow batches: direct bounded streams in process; `StreamDecoder` IPC across subprocesses with validation on, arbitrary chunks, `finish()`, and `require_alignment=false` | verified in Arrow 59.2 source; owner WP59/WP60 | ARROW §10.8–10.12; ALIGN INT-01/08 |
| LD-03 | `ResultChecksumV1` hashes canonical schema bytes, row count, and length-framed sorted multiset row encodings under an exact Arrow/`arrow-row` version domain | design-defined; adversarial proof owner WP64 | ARROW §7.9; DFREF §21; design v2 R3 |
| LD-04 | `codefabric.id16` is an application-enforced Arrow `ExtensionType` metadata contract over `FixedSizeBinary(16)`; optional DataFusion registration is formatter-only and never cited as cast/planner enforcement | verified limitation in DataFusion 55 source; owner WP58 | ARROW §26; DFREF S7 |
| LD-05 | Truthful `scan_with_args`, per-predicate pushdown, and effective-relation `Statistics`; `StatisticsRequest` is declined unless a request rule, returned-plan consumer, zero-I/O and staleness policy are implemented end to end | verified APIs; behavior proof owner WP58 | DFREF §18, §47, §51; ALIGN CAT-05–07 |
| LD-06 | Artifact captures ordinary `EXPLAIN` plus metrics/PG-JSON rendered from the exact served `ExecutionPlan`; governed serving never reruns `EXPLAIN ANALYZE` | verified in DataFusion 55 `AnalyzeExec` and `DisplayableExecutionPlan`; owner WP65 | DFREF §30, §55; ALIGN OBS-01–04 |
| LD-07 | Provenance at the table-transition boundary via delta-rs commit properties and `history()`; constraint presence verified at open | verified | DELTA43 reference; ALIGN P9 |
| LD-08 | IPC protocol profile: configurable writer compression; sans-IO stream encoding available where the transport owns backpressure; codec recorded in the protocol contract | verified (Arrow 59.1/59.2 ledger) | ARROW migration ledger, §10.6 |
| LD-09 | Versioned application canonicalization names the semantic-DAG/plan phase and serializes nodes, expressions, bound parameters, providers/functions and versions; plan-template, semantic-query, request and execution identities stay distinct | design-defined; golden proof owner WP64 | DFREF §56; ALIGN MOD-06, P18 |
| LD-10 | Boundary shape validation via fallible Arrow construction (`FixedSizeBinaryArray` `TryFrom`, `RecordBatch::try_new`, batch-vs-schema checks) | verified (Arrow 59.0 ledger) | ARROW migration ledger, §6.3; ALIGN SCH-09 |
| LD-11 | Graph traversal/path/pattern semantics execute through the existing application graph substrate and exchange typed Arrow batches with DataFusion relational nodes; no custom DataFusion planner or physical node | design decision; eight-form proof owner WP75 | QRY §§15–17, 21; PETGRAPH; PRIN P14/P15 |

## 3. Global target invariants

Every packet inherits these; packet-level invariants add to them.

- **GI-1 (PRIN P3/P18).** One authority per concept; every generated
  projection carries a digest link to its authority; no consumer re-encodes a
  registered vocabulary.
- **GI-2 (PRIN P6/P14).** No SQL text is constructed or executed on the
  semantic query path. A typed semantic DAG is the authority; relational nodes
  contain DataFusion `LogicalPlan`s and graph nodes contain inspectable
  application `GraphOperatorPlan`s.
- **GI-3 (PRIN P8/P22; HOL P8).** Validated Arrow batches are the canonical
  provider fact boundary. In-process providers use bounded direct streams;
  subprocess providers use validated Arrow IPC and converge before ingest.
- **GI-4 (PRIN P20).** Every advertised capability, state, statistic, or
  digest is computed from runtime facts or reported absent/unknown — never a
  constant standing in for a measurement.
- **GI-5 (PRIN P16; HOL P23).** Every fabric error carries a lifecycle phase;
  every public error code is a member of `PUBLIC_ERROR_IDS`.
- **GI-6 (PRIN P9/P10).** Every governed execution and publication resolves
  its provenance chain through stored references; missing links are explicit.
- **GI-7 (PRIN P25).** Acceptance is executable from the first proving commit:
  every oracle maps to a governed criterion, is selected by a
  `--no-tests=fail` gate, is not an alias, and avoids source-text checks where
  a structural or decoded-artifact oracle exists.
- **GI-8 (PRIN P14/P15).** Extension-ladder discipline: built-in relational
  nodes and transparent application graph plans only; any custom DataFusion
  logical/physical extension is a design-reopening trigger.
- **GI-9 (RM §1 inv. 8; SRV §6).** Python remains presentation-only; no
  adapter re-derivation of daemon state.
- **GI-10.** The divergence ledger stays closed; M14 requires accepted
  normative ownership for every selected principle and may not certify a
  merely routed clause.
- **GI-11 (ALIGN A.3).** One Arrow/DataFusion type universe at the pins;
  `just stable-graph-check` green at every proving commit.
- **GI-12 (PRIN P18/P19).** Delivered order, checksums, plan-template identity,
  semantic query identity, request identity, and execution identity are
  separate versioned contracts; duplicate multiplicity and bound parameters
  cannot disappear.
- **GI-13 (PRIN P12/P21).** Cross-table references are enforced over the
  complete candidate effective snapshot before publication activation;
  partial ingest batches never claim global referential truth.

## Audit Integration Log

Audit:
`docs/reviews/plan_audit_codefabric_design_principles_full_alignment_implementation_plan_v1_2026-08-24_2026-08-25_v1.md`
(v1, `needs-redesign`). Source design/plan: remediation proposal v1 and
implementation plan v1. Revised design/plan: remediation proposal v2 and
implementation plan v2, carried forward unchanged by this input-refresh v3.
Revision reason: replace invalid query, provider,
publication, identity, library, proof, and sequencing decisions while
preserving stable R/WP/M/DB identifiers.

- `F-001` — `applied-design`
  - Finding: full alignment omitted required normative amendments.
  - Resolution: design v2 R11/§6; plan WP73 and M14 require accepted amendments.
  - Revalidation: `just design-principle-traceability-check` — exit 1, recipe absent; WP73 adds it and it gates every downstream packet.
  - Rationale: certification now depends on accepted normative ownership, not routing prose.
- `F-002` — `applied-design`
  - Finding: LogicalPlan-only compilation could not implement `QRY`.
  - Resolution: design v2 R2/LD-01/LD-11; WP62 and new WP75 split typed IR, relational, graph, and eight-form work.
  - Revalidation: `just semantic-query-conformance-check` — exit 1, recipe absent; WP75 owns it.
  - Rationale: the target directly represents both relational and graph semantics.
- `F-003` — `applied-plan`
  - Finding: inactive draft validation fails on its absent state path.
  - Resolution: §1.3 and WP54 define inactive-draft validation plus atomic state-before-pointer activation; activation remains blocked until that governance change is separately authorized and landed.
  - Revalidation: `python3 -c 'from pathlib import Path; from tooling.ci.artifact_contracts import ROOT, validate_plan; validate_plan(ROOT, Path("docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v1_2026-08-24.md"), verify_declared_inputs=True)'` — exit 1, unresolved v1 state path, confirming the defect.
  - Rationale: no placeholder or unrelated state file is used to evade the contract.
- `F-004` — `added-packet`
  - Finding: the 124-row premise was stale and DP-022 was false at its baseline.
  - Resolution: WP73 re-runs every detector and publishes the superseding register before finding-driven work.
  - Revalidation: `just alignment-detector-check` — exit 1, recipe absent; WP73 adds it.
  - Rationale: executable current truth replaces five-anchor extrapolation.
- `F-005` — `applied-design`
  - Finding: universal IPC was the wrong in-process boundary and misstated alignment.
  - Resolution: design v2 R5/LD-02; WP59/WP60 use direct batches in process and validated IPC across processes with `finish()` and automatic alignment repair.
  - Revalidation: `just provider-protocol-check` — exit 1, recipe absent; WP59 adds it.
  - Rationale: one Arrow contract remains without unnecessary serialization.
- `F-006` — `added-packet`
  - Finding: partial ingest cannot enforce cross-table referential integrity.
  - Resolution: new WP74 enforces generated FK contracts over candidate effective publication state; WP57/WP59 retain local checks only.
  - Revalidation: `just publication-referential-integrity-check` — exit 1, recipe absent; WP74 adds it.
  - Rationale: the validator sees unchanged, co-arriving, replaced, and tombstoned rows.
- `F-007` — `applied-design`
  - Finding: DataFusion 55 extension registration does not enforce casts/planning.
  - Resolution: design v2 R6/LD-04; WP58 makes Id16 application-enforced Arrow metadata and limits DF registration to supported formatting.
  - Revalidation: `just id16-extension-contract-check` — exit 1, recipe absent; WP58 adds it.
  - Rationale: enforcement is assigned to an application boundary that can prove it.
- `F-008` — `applied-design`
  - Finding: ordering and result/plan/query identity contracts were under-defined.
  - Resolution: design v2 R2/R3; WP64 specifies delivered order, `ResultChecksumV1`, plan-template and bound-query identities.
  - Revalidation: `just query-determinism-check` — exit 1, recipe absent; WP64 adds it.
  - Rationale: duplicates, parameters, schema, pagination, and version domains remain semantic inputs.
- `F-009` — `applied-design`
  - Finding: `EXPLAIN ANALYZE` could record a second execution.
  - Resolution: design v2 R4/LD-06; WP65 renders metrics from the served physical-plan instance and prohibits diagnostic re-execution.
  - Revalidation: `just query-artifact-single-execution-check` — exit 1, recipe absent; WP65 adds it.
  - Rationale: one execution ID now joins exactly one result and metric source.
- `F-010` — `applied-design`
  - Finding: requested-statistics leverage lacked a producer/consumer path.
  - Resolution: design v2 R6/LD-05; WP58 defaults to truthful table statistics and explicitly declines CAT-07 unless its whole path is implemented.
  - Revalidation: `just provider-statistics-contract-check` — exit 1, recipe absent; WP58 adds it.
  - Rationale: API vocabulary is not mistaken for an automatic capability.
- `F-011` — `applied-plan`
  - Finding: oracle governance arrived after and invalidated its own oracles.
  - Resolution: WP54 installs criterion mappings, alias detection, selector reachability, and `--no-tests=fail`; every later packet depends on WP54. WP70 retains only late rule/fixture integration.
  - Revalidation: `just oracle-substance-check` — exit 1, recipe absent; WP54 adds it.
  - Rationale: proof cannot close before the proof contract exists.
- `F-012` — `applied-plan`
  - Finding: normative edges permitted conflicting shared-contract work.
  - Resolution: §8 serializes WP56→WP69, WP62/WP75/WP63/WP64/WP65→WP67, and WP63→WP68; new packet edges are explicit.
  - Revalidation: `just plan-dependency-check` — exit 1, recipe absent; WP54 adds a plan-graph/known-touch overlap oracle.
  - Rationale: normative dependencies, not the illustrative linear order, enforce ownership.
- `F-013` — `applied-design`
  - Finding: WP55 conflated semantic identity with integrity/security hashing.
  - Resolution: design v2 R1; WP55 performs a purpose-aware structural census and establishes separate authorities.
  - Revalidation: `just digest-domain-contract-check` — exit 1, recipe absent; WP55 adds it.
  - Rationale: each hash purpose has one correct owner and threat model.
- `F-014` — `applied-plan`
  - Finding: set difference did not prove exact effective-state equality.
  - Resolution: WP72 requires schema equality plus duplicate-sensitive bag equality and selects all four WP72 oracles.
  - Revalidation: `just rebuild-equivalence-check` — exit 0, but currently runs five legacy WP48 tests; WP72 replaces the body before it can close this finding.
  - Rationale: a currently green legacy selector is recorded as insufficient, not accepted as closure.
- `F-015` — `added-packet`
  - Finding: Gate B omitted accountable owner acceptance.
  - Resolution: WP71 generates review candidates; new WP76 is an explicit owner checkpoint and versioned release packet.
  - Revalidation: `just gate-b-owner-acceptance-check` — exit 1, recipe absent; WP76 adds it.
  - Rationale: the implementation cannot approve its own golden outputs.
- `F-016` — `added-packet`
  - Finding: the dirty baseline and untracked-deletion rollback were untrustworthy.
  - Resolution: WP73 owns complete path disposition, fresh baseline evidence, and recoverable handling; WP54 no longer claims Git can restore untracked bytes.
  - Revalidation: `just audit-baseline-check` — exit 1, recipe absent; WP73 adds it.
  - Rationale: ownership judgment is explicit while derived status/digests remain recomputable.

## 4. Work packets

Numbering continues the global sequence (prior plans end at WP53/M08/DB09).
WP54–WP72 retain their v1 identities; audit-added work uses WP73–WP76.

### WP73 — Normative alignment authority, current register, and owned baseline

**Outcome.** The five previously unowned principles and DP-051 have accepted
normative homes; every DP-001–DP-124 detector has a current-tree disposition;
and every pre-existing path at the v2 baseline has an owner decision before
implementation commits begin.

**Dependencies.** None. This is a design-acceptance prerequisite; no
finding-driven code packet is ready until it completes.

**Target invariants.** GI-10, GI-7; `PRIN P2/P10/P14/P19/P21/P25`;
`HOL P29/P31`. Design references: R11 and design v2 §6; audit F-001, F-004,
F-016.

**Change surface.**
- Preflight query: `git status --porcelain=v1`; `git diff HEAD | shasum -a 256`;
  `git show d89cc90:src/lifecycle.rs | rg -n 'INSERT INTO update_wave'`;
  enumerate every `DP-[0-9]{3}` detector from the conformance artifact and
  execute it with its stated coverage envelope; `just ci-fast`.
- Known touch: the owning `SUITE`, `FAB`, `QRY`, `LIFE`, and `GEN`
  specifications; a new superseding design-principles conformance review;
  generated navigation/traceability projections; `justfile` and detector
  scripts for the three packet recipes.

**Required changes.**
1. Apply design v2 §6's exact P2/P10/P14/P19/P21 and DP-051 decisions to the
   normative specifications. Produce a reviewable diff and stop for the
   repository owner's accountable acceptance; accepted spec digests become
   declared inputs of the next plan version before implementation continues.
2. Re-run all 124 detectors against HEAD. Publish a superseding conformance
   review with `open|partial|closed|invalid|changed` judgment per row and
   reproducible command/coverage. Regenerate, rather than hand-edit, the
   finding-to-move/packet projection. Any newly discovered work is a plan
   revision trigger.
3. Derive the complete dirty/untracked/deleted path set and obtain an owner
   disposition for each path. Stage plan-owned artifacts explicitly; preserve
   separately owned work. Any untracked removal requires a recoverable archive
   or byte manifest plus owner approval.
4. Add `design-principle-traceability-check`, `alignment-detector-check`, and
   `audit-baseline-check`. Clear or explicitly register the fresh `ci-fast`
   Typos baseline; never relabel the failed run green.

**Legacy disposition.** V1's routed-only principle rows and five-anchor claim
retire. The pass-3 register remains immutable history, not execution authority.

**Acceptance checks.**
- Behavioral: all 25 principles and active DP rows resolve to an accepted
  normative clause, owner, and executable proof.
  Executable oracle: `wp73_behavioral_acceptance`
  Governed criterion: `PC-WP73-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: the superseding register has exactly one disposition and one
  reproducible detector record per DP-001–DP-124.
  Executable oracle: `wp73_structural_acceptance`
  Governed criterion: `PC-WP73-STR` — mapped through this packet's Target invariants and Design references.
- Negative: a routed-only clause, missing detector, or unattributed dirty path
  fails certification.
  Executable oracle: `wp73_negative_zero_state`
  Governed criterion: `PC-WP73-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: owner acceptance and baseline disposition artifacts resolve;
  the three named recipes are green.
  Executable oracle: `wp73_operational_acceptance`
  Governed criterion: `PC-WP73-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just design-principle-traceability-check`,
`just alignment-detector-check`, `just audit-baseline-check`, `just typos`.
**Milestone.** M09 prerequisite. **Replan triggers.** Any normative decision
is rejected; any detector changes a packet's architecture, boundaries, or
scope; baseline ownership is unresolved. **Rollback/recovery.** Normative
artifacts are versioned; rejected candidates remain unreleased. Untracked
bytes are restored from the owner-approved archive/manifest, never Git.

### WP54 — Baseline, input canonicalization, and register-hygiene scaffolding

**Outcome.** The plan's inputs are reproducible repository artifacts; inactive
draft validation and activation are non-circular; and non-vacuous oracle,
selector, dependency, registration, and hygiene governance exists before any
implementation packet can prove completion.

**Dependencies.** WP73.

**Target invariants.** GI-1, GI-7, GI-10. Design references: R10/R11
(CONF DP-057, DP-058, DP-061, DP-074, DP-094, DP-104, DP-108, DP-124);
audit F-003, F-011, F-012; `artifact-schemas.md §7/§8`.

**Change surface.**
- Preflight query: `git status --porcelain=v1`; direct `validate_plan` on an
  inactive draft with absent state; inspect every `wpNN_*` declaration and
  `just --show` selector; `ast-grep` alias-test census; known-touch overlap
  graph for all packets; `rg -l 'docs/reviews' scripts/ tooling/`.
- Known touch (verified this session):
  `.claude/skills/_shared/artifact-schemas.md`,
  `tooling/ci/artifact_contracts.py` (both already edited: the
  `design-principles-remediation-proposal` row exists in both authorities),
  `scripts/seed_zero_state_check.sh`.

**Required changes.**
1. Land the pre-activation governance rule through its separately authorized
   change: inactive `draft|audited` plans may declare a future state path;
   approval/activation atomically creates and validates schema-2 state before
   switching `active-plan.json`. Add a failure-injection test proving the
   pointer never references absent or malformed state.
2. Commit only WP73-dispositioned plan-owned inputs and registration edits by
   explicit path. Preserve separately owned changes. Apply the accepted
   `skills/` disposition; deletion is permitted only with WP73 recovery
   evidence, otherwise use an owner-approved symlink or leave it untouched.
3. Add oracle-substance governance: every oracle has a unique governed
   `PC-WPNN-{BEH|STR|NEG|OPS}` criterion mapped through the packet's design
   references; aliases and literal-only placeholder occurrences fail; every
   selector uses `--no-tests=fail` and demonstrably selects all four packet
   oracles. Add `oracle-substance-check` before any packet selector.
4. Add `plan-dependency-check`: validate the declared DAG is acyclic and reject
   unordered known-touch/contract-owner overlaps unless the packets declare
   disjoint phases.
5. Add the artifact-vocabulary comparison oracle: `artifact-schemas.md §7`
   table keys == `REVIEW_REQUIREMENTS` keys (CONF DP-074), wired into
   `artifacts-check`'s pytest.
6. Add the detector-hygiene convention: whole-repo governance detectors carry
   `--glob '!docs/reviews/**'` (CONF DP-124), applied to the scripts the
   preflight query finds.
7. Re-run `just ci-fast`; record baseline failures in execution state at
   execution start.

**Legacy disposition.** Presence-only oracle discovery, alias tests, zero-match
selectors, non-atomic activation, and unowned untracked deletion retire.

**Acceptance checks.**
- Behavioral: inactive-draft validation and atomic activation failure-injection
  tests pass; vocabulary comparison is green; plan-owned inputs are tracked.
  Executable oracle: `wp54_behavioral_acceptance`
  Governed criterion: `PC-WP54-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: all plan oracles have unique criterion mappings and reachable
  selectors; dependency graph/known-touch overlaps are ordered.
  Executable oracle: `wp54_structural_acceptance`
  Governed criterion: `PC-WP54-STR` — mapped through this packet's Target invariants and Design references.
- Negative: alias, placeholder-only occurrence, zero-match selector,
  unresolved active state, unordered overlap, and unowned `skills/` deletion
  fixtures all fail.
  Executable oracle: `wp54_negative_zero_state`
  Governed criterion: `PC-WP54-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just oracle-substance-check`, `just plan-dependency-check`,
  and `just governance` are green before WP55 becomes ready.
  Executable oracle: `wp54_operational_acceptance`
  Governed criterion: `PC-WP54-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** Edit-local: `just artifacts-check`. Packet:
`just oracle-substance-check`, `just plan-dependency-check`, `just governance`,
`just ci-fast`. **Milestone.** M09. **Replan triggers.** `ci-fast` baseline
reveals failures attributable to the data-fabric upgrade that block later
packets; inactive-plan semantics cannot be made atomic. **Rollback/recovery.**
Governance changes revert by commit; owner-owned/untracked bytes follow WP73's
recovery record, never an unsupported Git revert claim.

### WP55 — Fingerprint-domain registry and identity consolidation

**Outcome.** Every hash use has exactly one purpose-appropriate authority;
semantic identity/fingerprint domains are registered and constructed through
`crate::identity`, while integrity, cache, and keyed security hashes remain
separate narrow APIs; generated CBEF recipes carry declared normalization.

**Dependencies.** WP54.

**Target invariants.** GI-1; `PRIN P3/P18`; `GEN §13` (application-owned
identity). Design references: R1 (CONF DP-005, DP-031, DP-044-digest,
DP-086, DP-119, DP-120); ALIGN MOD-06, SCH-10, OBS-09.

**Change surface.**
- Preflight query: `git grep -n 'b"codefabric' -- src/ rustc-extractor/`;
  `rg -n 'fn digest_bytes' src/ --glob '!src/generated/**'`;
  `rg -n 'blake3::(Hasher|hash)|new_keyed' src rustc-extractor/src tooling codefabric-cpg-mcp
  --glob '!src/generated/**'`; structural calls/imports including renamed
  bindings; candidate-file census with hidden paths included and library refs
  excluded;
  `rg -n 'normalization' contracts/identity/cbef-v1.yaml src/generated/model_identity_recipes.rs`
- Known touch: `src/identity.rs`, `contracts/identity/` (new registry),
  `src/core_facts.rs` (duplicate scope fingerprints), `src/source_syntax.rs`
  (unguarded twin), the model driver emitting recipes.

**Required changes.**
1. Classify every discovered use as semantic identity/fingerprint, integrity,
   cache, or security/MAC. Record the semantic distinction and owner; do not
   migrate a use merely because it calls BLAKE3.
2. Author the semantic fingerprint-domain registry: domain string, framing,
   field set/order, normalization, and compatibility policy. `crate::identity`
   compiles it into the only semantic identity constructors; collapse the twin
   `capability-scope` implementations.
3. Give integrity and keyed security operations separate named APIs and
   governance boundaries. Unify only `digest_bytes` definitions with the same
   semantics; differently purposed functions remain distinct.
4. Emit `normalization: ASCII_LOWER` in generated recipes and honor it in
   the generated-recipe evaluation path (closes the latent CONF DP-005
   before governance pushes production onto that path).
5. Add purpose-aware governance covering constructors, one-shot hashes, keyed
   forms, imports, and renamed calls. Every semantic fingerprint use must route
   through identity; security/integrity exceptions name their own authority.

**Legacy disposition.** Ad-hoc semantic domain literals are deleted at their
call sites; same-purpose `digest_bytes` copies collapse without merging
unrelated security or integrity semantics.

**Acceptance checks.**
- Behavioral: registry-driven digests reproduce the pre-change values for
  every migrated domain (golden equivalence corpus).
  Executable oracle: `wp55_behavioral_acceptance`
  Governed criterion: `PC-WP55-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: purpose-aware census — every hash use has exactly one authority
  and semantic domains in use are a subset of the identity registry.
  Executable oracle: `wp55_structural_acceptance`
  Governed criterion: `PC-WP55-STR` — mapped through this packet's Target invariants and Design references.
- Negative: direct semantic hashing through constructor, one-shot, keyed,
  imported, or renamed forms outside identity fails; integrity/security APIs
  cannot mint semantic IDs.
  Executable oracle: `wp55_negative_zero_state`
  Governed criterion: `PC-WP55-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just model-repro-check` green (recipes regenerate
  identically twice).
  Executable oracle: `wp55_operational_acceptance`
  Governed criterion: `PC-WP55-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just digest-domain-contract-check`, `just root-check`,
`just root-test`, `just governance-scan`, `just model-repro-check`.
**Milestone.** M09. **Replan triggers.** A domain
migration changes a persisted digest value — persisted-identity
compatibility then needs an explicit migration design (plan revision).
**Rollback.** Registry additive until call-site migration lands; revert per
sub-change.

### WP56 — One registry authority across languages and the wire

**Outcome.** One generated Rust registry module, the governed Python module
as the imported one, registry-emitted proto enums, and authority reads at
startup — the CONF DP-001/002/003 drift class is structurally closed.

**Dependencies.** WP55.

**Target invariants.** GI-1; `PRIN P3`. Design references: R1 (CONF DP-001,
DP-002, DP-003, DP-039, DP-040, DP-076, DP-083, DP-085, DP-118, DP-122);
ALIGN MOD-04.

**Change surface.**
- Preflight query: `rg -n 'model_generated::registries|crate::registries' src/`;
  `rg -n 'class IdentityDomain' codefabric-cpg-mcp/`; `sed -n '820,870p'
  src/bin/codefabric_model/repository_model.rs` (role matching);
  `rg -n 'QUERY_EXECUTION_STATE' contracts/`; `rg -n 'fn digest_frames'
  src/ rustc-extractor/`
- Known touch: `src/generated/registries.rs`,
  `src/generated/model_registries.rs`, `repository_model.rs` role matcher,
  `contracts/rpc/cpg_query_service.proto`, `src/rustc_service.rs`,
  `rustc-extractor/src/wrapper.rs`, `src/query_service.rs` (bundle digest).

**Required changes.**
1. Collapse the twin generated Rust registry modules to one; migrate the
   dual imports; delete the hand-written `NewlineKind` and `FreshnessState`
   re-declarations and their crosswalks.
2. Fix `ArtifactRole` matching so the governed Python registry and identity
   modules are the imported ones; delete the orphan `registries.py`; restore
   the missing `ROOT_AUTHORIZATION` domain in governed Python output.
3. Proto driver emits wire enums from `enum-registry.yaml` for domains both
   declare; cross-check oracle fails on set/value divergence. (`proto-gen`
   equivalent regeneration is a mutating model action — run deliberately,
   diff reviewed, per the command contract.)
4. Wire codes read from registry records; delete the array-position
   arithmetic in the extractor wrapper.
5. `digest_frames` emitted into both Cargo roots from one model template;
   byte-equality oracle.
6. Advertised bundle digest read from
   `contracts/bundles/query-language-bundle.json` at build/bootstrap.
7. Register the query-form vocabulary as a registry domain; serde renames
   derive from it.

**Legacy disposition.** Orphan Python registry deleted; twin Rust module
deleted; literal bundle digest deleted — covered by DB12's exit invariants.

**Acceptance checks.**
- Behavioral: cross-language registry KATs — Rust and Python decode shared
  fixtures to identical vocabularies, including identity domains.
  Executable oracle: `wp56_behavioral_acceptance`
  Governed criterion: `PC-WP56-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: registry↔proto cross-check green; `digest_frames`
  byte-equality green.
  Executable oracle: `wp56_structural_acceptance`
  Governed criterion: `PC-WP56-STR` — mapped through this packet's Target invariants and Design references.
- Negative: zero imports of the deleted module paths; zero hand-written
  registry-domain enums outside generated code (rule + rg zero-hit).
  Executable oracle: `wp56_negative_zero_state`
  Governed criterion: `PC-WP56-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just adapter-ci-fast` and `just model-repro-check` green.
  Executable oracle: `wp56_operational_acceptance`
  Governed criterion: `PC-WP56-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-check`, `just root-test`, `just adapter-ci-fast`,
`just extractor-ci-fast`, `just model-check`, `just model-repro-check`.
**Milestone.** M09. **Replan triggers.** Proto enum renumbering would break
the wire — the registry↔proto emission must preserve current wire values or
the boundary needs a versioned migration (plan revision). **Rollback.**
Generated outputs revert with the model; the role-matcher fix is isolated.

### WP57 — Schema contracts enforced: metadata classes, evolution policy, generated encoders

**Outcome.** The schema IR's declared semantics are classified into the six
P21 classes with named consumers; FK contracts and row-local validators are
generated for WP74's candidate-state enforcement; evolution policy is
versioned; row encoders derive from the same IR.

**Dependencies.** WP56.

**Target invariants.** GI-1, GI-4, GI-13; `PRIN P12/P21`; `FAB` App. C inv. 11.
Design references: R6 (CONF DP-020, DP-021, DP-025, DP-034, DP-037, DP-038,
DP-043); ALIGN SCH-01–SCH-09, GOV-07; DFREF S5–S8, S14.

**Change surface.**
- Preflight query: `rg -n 'semantic_type: "enum:' src/generated/table_specs.rs
  | wc -l` vs registry domains; `rg -n 'foreign_key' src/generated/ src/fact_ingest.rs`;
  `rg -n 'enableTypeWidening|SCHEMA_DIGEST_MISMATCH' src/fabric.rs`;
  `rg -n 'install_constraints|validate_open_table' src/fabric.rs`;
  `rg -c '"[a-z_]+", ' src/fact_ingest.rs` (hand-written encoder tuples)
- Known touch: `src/bin/codefabric_model/schema_driver.rs`,
  `src/schema_registry.rs`, `src/fact_ingest.rs`, `src/fabric.rs`,
  `contracts/schema/`.

**Required changes.**
1. Metadata dictionary: classify every schema-IR annotation as enforced,
   planner-consumed, contractual, governance, lineage, or advisory, with a
   named consumer; oracle asserts each non-advisory consumer exists.
2. Generate typed foreign-key contracts and row-local key/type/shape checks.
   Do not claim cross-table enforcement in this packet or partial ingest.
   Record the SQLite `REFERENCES` decision (emit clauses or drop the pragma);
   WP74 owns effective-state enforcement.
3. Validate `semantic_type` strings against the enum registry with a digest
   link; unresolvable references become build failures. Bind
   `fact_evidence.fact_form_code` to its domain; one sourcing rule.
4. Emit `ontology_version` and `compatibility_mode` from the IR; delete the
   hand-coded literals.
5. Generate the row-encoder family from the schema IR, replacing the
   hand-written `encode_*` column tuples; runtime name/arity guard retired
   in favor of generation.
6. Author the schema-evolution policy contract (current class: exact-pin;
   migration route; acceptance suite), and generate the compatibility
   acceptance suite `schema-validation.json` currently stubs
   (`compatibility_acceptance_generated: false` flipped by real generation).
7. `validate_open_table` verifies `delta.constraints.*` against the spec on
   every serving open; constraint installation moves after table
   authentication.

**Legacy disposition.** Hand-written encoders and IR-shadow literals
deleted; the two unconsumed planner annotations (`TableSpec::dependencies`,
`zorder_columns`) gain their consumers or are removed from the IR (decision
recorded in the metadata dictionary).

**Acceptance checks.**
- Behavioral: generated encoders byte-reproduce the prior encoding over the
  fact-table golden corpus; constraint-drop is detected at open; generated FK
  contracts match the IR.
  Executable oracle: `wp57_behavioral_acceptance`
  Governed criterion: `PC-WP57-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: metadata-dictionary oracle; semantic-type resolution oracle;
  evolution-policy artifact validated.
  Executable oracle: `wp57_structural_acceptance`
  Governed criterion: `PC-WP57-STR` — mapped through this packet's Target invariants and Design references.
- Negative: mutated constraints/schema rejected with the registered code;
  unresolvable `semantic_type` fails the build (expected-failure fixture).
  Executable oracle: `wp57_negative_zero_state`
  Governed criterion: `PC-WP57-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just model-repro-check` and `just stable-graph-check` green.
  Executable oracle: `wp57_operational_acceptance`
  Governed criterion: `PC-WP57-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-check`, `just root-test`, `just model-check`,
`just model-repro-check`. **Milestone.** M10. **Replan triggers.** Generated FK
contracts cannot express the IR or conflict with accepted schema evolution.
**Rollback.** Encoder generation lands behind equivalence proof; revertible
per change.

### WP74 — Candidate-publication referential integrity

**Outcome.** Cross-table foreign keys are enforced over the complete candidate
effective snapshot after owner replacements and tombstones and before Delta
publication CAS/activation.

**Dependencies.** WP57, WP59.

**Target invariants.** GI-13, GI-6; `PRIN P11/P12/P21`. Design references:
R6 (CONF DP-021); audit F-006; `FAB §§66, 71.1`.

**Change surface.**
- Preflight query: trace `ValidatedFactBatch::validate`, publication candidate
  construction, owner replacement, tombstone, `validate_references`, Delta
  write/CAS, and activation callers structurally; inspect all generated FK
  consumers and negative fixtures.
- Known touch: generated FK projection, `src/fabric/publication.rs`, candidate
  effective-state construction, publication tests, and the owning gate recipe.

**Required changes.**
1. Build the exact candidate relation for every referenced table: authenticated
   durable base minus tombstones plus owner replacements/overlay rows at the
   versions proposed for the same publication.
2. Evaluate generated FK contracts against that candidate set after all
   co-arriving tables are available and before commit/CAS/activation. Preserve
   row-local ingest failures as a distinct earlier phase.
3. Emit a registered publication-validation error with source/target table,
   key, owner scope, and coverage; failed validation publishes nothing.
4. Add adversarial cases for an unchanged base target, co-arriving target,
   replaced target, tombstoned target, genuinely missing target, and a
   multi-table candidate that fails after partial staging.

**Legacy disposition.** Decorative FK metadata and ingest-time global-FK
claims retire; one candidate-publication validator is authoritative.

**Acceptance checks.**
- Behavioral: all valid base/co-arriving/replacement references publish and
  remain queryable at the activated pointer.
  Executable oracle: `wp74_behavioral_acceptance`
  Governed criterion: `PC-WP74-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: every generated FK has exactly one candidate-state enforcement
  path before CAS/activation.
  Executable oracle: `wp74_structural_acceptance`
  Governed criterion: `PC-WP74-STR` — mapped through this packet's Target invariants and Design references.
- Negative: tombstoned/missing targets abort atomically with no visible Delta
  or serving-pointer transition.
  Executable oracle: `wp74_negative_zero_state`
  Governed criterion: `PC-WP74-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: recovery after failed validation leaves staging reclaimable and
  the prior snapshot current.
  Executable oracle: `wp74_operational_acceptance`
  Governed criterion: `PC-WP74-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just publication-referential-integrity-check`, `just root-test`,
`just wave4-integration-check`. **Milestone.** M10. **Replan triggers.** The
candidate state cannot be constructed without new durable metadata or a real
existing-data violation requires migration policy. **Rollback/recovery.** The
validator lands before activation cutover; failed candidates are discarded and
the prior pointer remains authoritative.

### WP58 — Truthful providers and the `codefabric.id16` metadata contract

**Outcome.** Serving providers advertise effective-relation statistics with
honest precision and per-predicate pushdown; requested statistics are either
implemented end to end or explicitly declined; Id16 columns carry an
application-enforced Arrow extension metadata contract.

**Dependencies.** WP57.

**Target invariants.** GI-4; `PRIN P15/P20/P21`. Design references: R6
(CONF DP-019, DP-063-instance-validation); ALIGN CAT-05–CAT-07, SCH-06,
INT-09; ARROW §26; DFREF §18, §47, §51, S7, S10.

**Change surface.**
- Preflight query: `rg -n 'fn statistics|supports_filters_pushdown|scan_with_args'
  src/fabric/`; `rg -n 'parquet_pruning|repartition_' src/fabric/serving.rs`;
  `rg -n 'ExtensionType|ARROW:extension' src/`; inspect DataFusion 55
  `DFExtensionType`, registry consumers, and `cast_to`; trace every
  `StatisticsRequest` producer and returned-plan consumer.
- Known touch: `src/fabric/overlay.rs`, `src/fabric/serving.rs`,
  `src/schema_registry.rs`, `tooling/model/validate_staged_schemas.py`.

**Required changes.**
1. Overlay/serving providers return `Statistics` with per-column
   `Precision::{Exact,Inexact,Absent}` for the complete effective relation.
   Replacement-batch counts cannot claim exact effective counts; Delta stats
   are used only at the authenticated snapshot scope. Adopt
   `scan_with_args`/`ScanArgs` and declare per-predicate pushdown truthfully,
   with one `ColumnStatistics` entry per schema field.
2. `ServingRuntimeEvidence` records observed pruning/repartition facts from
   the served physical plan's metrics, not configuration read-back or a second
   `EXPLAIN ANALYZE` execution.
3. Define the `codefabric.id16` `ExtensionType` (storage
   `FixedSizeBinary(16)`, versioned metadata), attach it to Id16 fields in
   generated Arrow schemas, and enforce metadata preservation/reattachment or
   deliberate rejection in application schema validation through projection,
   cast, IPC, Parquet, and evolution. Unknown consumers degrade to storage.
   A separate `DFExtensionType`/registration may provide formatting only; do
   not claim planner/cast enforcement from the registry.
4. Public JSON schemas gain instance validation: golden envelopes validated
   against `planspec.schema.json` and siblings in the staged-schema check.
5. Either explicitly decline `StatisticsRequest`/CAT-07 with no capability
   claim, or implement the whole path: request-producing optimizer rule,
   returned-plan statistics representation and consumer, cache/staleness
   policy, and zero planning-time I/O.

**Legacy disposition.** The tautological evidence assertions deleted;
`statistics() -> None` replaced.

**Acceptance checks.**
- Behavioral: adversarial pushdown-truth tests — claimed-exact predicates
  falsified with boundary rows; statistics precision matches measured data.
  Executable oracle: `wp58_behavioral_acceptance`
  Governed criterion: `PC-WP58-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: Id16 field metadata inspected through plan/project/cast, IPC,
  Parquet, and unknown-consumer fallback; registry factory tested only for
  supported behavior; instance validation wired into the staged-schema gate.
  Executable oracle: `wp58_structural_acceptance`
  Governed criterion: `PC-WP58-STR` — mapped through this packet's Target invariants and Design references.
- Negative: false effective-row exactness, false pushdown, missing extension
  metadata, or an unconsumed requested-statistics claim fails the suite.
  Executable oracle: `wp58_negative_zero_state`
  Governed criterion: `PC-WP58-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just root-test` and `just data-fabric-upgrade-check` green.
  Executable oracle: `wp58_operational_acceptance`
  Governed criterion: `PC-WP58-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just id16-extension-contract-check`,
`just provider-statistics-contract-check`, `just root-check`, `just root-test`,
`just data-fabric-upgrade-check`. **Milestone.** M10. **Replan triggers.**
Application validation cannot preserve the required Id16 contract, or
requested statistics require a new optimizer/plan contract not accepted in
design. Metadata-only Arrow behavior is the selected design, not a fallback
deviation. **Rollback.** Extension attachment and statistics changes remain
behind their behavioral contract gates until cutover.

### WP59 — One Arrow fact contract and merged ingest pipeline

**Outcome.** Direct in-process batch streams and validated subprocess IPC
converge on one schema-driven ingest pipeline covering every fact table;
provider-computed facts and diagnostics survive to their tables.

**Dependencies.** WP57.

**Target invariants.** GI-3, GI-4; `PRIN P7/P8/P22`; doctrine "raw and
normalized coexist". Design references: R5 (CONF DP-024, DP-026, DP-028,
DP-029, DP-030, DP-032, DP-050); R4 attribution; ALIGN ARR-03/08/10,
INT-01/08, SCH-09; ARROW §10.8–10.9, §5–§6; LD-02, LD-08, LD-10.

**Change surface.**
- Preflight query: `rg -n 'ArrowIpcChunk|ObservationMessage|CanonicalFact|encode_selected'
  src/`; `rg -n 'StreamReader|StreamDecoder|arrow::ipc' src/` (LD-02 is
  verified — the facade `ipc` feature is live; no probe needed);
  `rg -n 'evaluation_ordinal|source_ordinal' src/ruff_adapter.rs src/source_syntax.rs`
- Known touch: `src/provider_runtime.rs`, `src/fact_ingest.rs`,
  `src/source_syntax.rs`, `src/tree_sitter_adapter.rs`, `src/ruff_adapter.rs`.

**Required changes.**
1. Define the provider output port as bounded validated `RecordBatch` streams.
   In-process adapters implement it directly. External IPC adapters use
   `StreamDecoder` with validation enabled, arbitrary partial chunks,
   `finish()` termination, and `require_alignment=false` automatic copy for
   unaligned buffers; `with_skip_validation` is prohibited.
2. Retire the `ObservationMessage`/`CanonicalFact` channel; coverage becomes
   schema-driven (any generated table spec is representable).
3. Merge the projection and observation paths above
   `ValidatedFactBatch::validate`: one row-local shape/key validator, one
   row-budget mechanism, one precedence table, one conflict/evidence encoder,
   and fingerprint fencing. WP74 owns cross-table candidate-state validation.
4. Carry provider-computed facts as batch columns (`evaluation_ordinal`,
   `source_ordinal`, positions, `depth`, provider-parsed names); attribution
   columns carry the true producer and `derivation_code`; the
   `RuffTokenClass` narrowing becomes a declared registry mapping or is
   dropped for the raw+normalized pair.
5. Derived relations gain evidence rows via the common accumulator;
   `IngestDiagnostic`/`ConflictRecord` are written to the `diagnostic`
   table.
6. Record external IPC codec, compression, schema/profile and resource limits
   as a versioned contract. Memory alignment is local decoder policy, not a
   wire field.

**Legacy disposition.** `ObservationMessage`, `CanonicalFact`,
`encode_selected`, and the duplicated above-validator logic deleted — DB11
carries the exit invariants.

**Acceptance checks.**
- Behavioral: the same substitution corpus passes through direct-batch and IPC
  adapters; the merged pipeline reproduces both former paths.
  Executable oracle: `wp59_behavioral_acceptance`
  Governed criterion: `PC-WP59-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: schema-driven coverage — every generated fact table has an
  ingest path (census oracle); evidence and diagnostic rows appear for the
  golden corpus.
  Executable oracle: `wp59_structural_acceptance`
  Governed criterion: `PC-WP59-STR` — mapped through this packet's Target invariants and Design references.
- Negative: malformed/truncated IPC and schema/resource violations are
  rejected; arbitrary chunk splits and valid unaligned buffers succeed;
  `with_skip_validation` remains zero-hit.
  Executable oracle: `wp59_negative_zero_state`
  Governed criterion: `PC-WP59-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just wave4-integration-check` green.
  Executable oracle: `wp59_operational_acceptance`
  Governed criterion: `PC-WP59-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just provider-protocol-check`, `just root-check`, `just root-test`,
`just wave4-integration-check`, `just governance-scan`. **Milestone.** M10.
**Replan triggers.** The merged pipeline cannot preserve both former paths'
semantics without a behavioral choice the proposal did not settle.
**Rollback.** Direct and IPC adapters land behind the common port beside the
old channel until the differential proof is green; deletion is DB11's.

### WP60 — Provider hierarchy, one cancellation, one extractor seam

**Outcome.** Real adapters implement `ProviderAdapter`; the provider set is
registry-driven; one cancellation type is threaded end-to-end; the extractor
has exactly one wire protocol.

**Dependencies.** WP59.

**Target invariants.** GI-3; `PRIN P4/P5`; `SRV §6` inv. 10. Design
references: R5 (CONF DP-009, DP-010, DP-011, DP-033, DP-047, DP-049,
DP-084); ALIGN CAT-10, TST-02.

**Change surface.**
- Preflight query: `rg -n 'impl ProviderAdapter' src/`; `rg -n
  '(trait|struct|enum) \w*Cancellation' src/`; `rg -n 'tree_field_role|ruff_field_role'
  src/source_syntax.rs`; `rg -n 'extract-json|ExtractRequest' rustc-extractor/`;
  `rg -n 'ProviderJobSpec' src/`
- Known touch: `src/provider_runtime.rs`, `src/tree_sitter_adapter.rs`,
  `src/ruff_adapter.rs`, `src/source_syntax.rs`, `rustc-extractor/src/main.rs`,
  `rustc-extractor/src/wrapper.rs`.

**Required changes.**
1. `TreeSitterAdapter` and `RuffAdapter` implement `ProviderAdapter`,
   emitting direct bounded batch streams through WP59's Arrow port; external
   rustc/Pyrefly adapters own IPC decoding. The ingest entry point takes a
   registry-driven adapter-output collection (the fixed `(tree, ruff)`
   signature retired).
2. The two field-role tables become one generated crosswalk registry;
   current semantic coercions become records or are removed.
3. One `Cancellation` handle threaded RPC → provider execution → stream
   polling; the five bespoke encodings deleted.
4. A domain DTO owns the provider seam; prost types confined to the rpc
   adapter; the lossy scope→progress string collapse removed.
5. Delete `--extract-json` and its bespoke protocol; the extractor's
   determinism oracle runs against the gRPC + IPC path.
6. `AdmissionController` maps gain lifecycle eviction; `SourceImageStore`
   ownership doc made true or corrected.

**Legacy disposition.** `--extract-json`, bespoke cancellation types, dual
role tables deleted — DB11 exit invariants.

**Acceptance checks.**
- Behavioral: the provider contract suite runs identically against every
  direct/IPC adapter (substitution test as oracle); extractor determinism
  proved on the gRPC + IPC path.
  Executable oracle: `wp60_behavioral_acceptance`
  Governed criterion: `PC-WP60-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: crosswalk registry is the only field-role source; provider
  registration is the only provider-set authority.
  Executable oracle: `wp60_structural_acceptance`
  Governed criterion: `PC-WP60-STR` — mapped through this packet's Target invariants and Design references.
- Negative: cancellation-type census == 1 (rule); `--extract-json` zero-hit;
  `ProviderJobSpec` absent from domain signatures.
  Executable oracle: `wp60_negative_zero_state`
  Governed criterion: `PC-WP60-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: cancellation propagation test RPC→stream-drop under load;
  `just extractor-ci-fast` green.
  Executable oracle: `wp60_operational_acceptance`
  Governed criterion: `PC-WP60-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-test`, `just extractor-ci-fast`,
`just wave4-integration-check`, `just governance-scan`. **Milestone.** M10.
**Replan triggers.** The adapter trait cannot express an existing provider
behavior without widening — trait change is a contract change (plan
revision). **Rollback.** Adapters implement the trait additively before the
entry-point cutover.

### WP61 — Lifecycle phases, one error vocabulary, guard truth

**Outcome.** Every fabric error carries a phase; public error identity is
registry membership with generated projections consumed at the boundary;
stage traces survive failure; shutdown reports observed outcomes; wave
guards are evaluated.

**Dependencies.** WP56.

**Target invariants.** GI-4, GI-5; `PRIN P16/P20`; `HOL P23`; `RM §1`
inv. 6. Design references: R7 (CONF DP-014, DP-015, DP-016, DP-017, DP-018,
DP-079, DP-096, DP-117); ALIGN §17 lifecycle failure codes; DFREF §33, §41.

**Change surface.**
- Preflight query: `rg -c '#\[error' src/`; `rg -n 'PUBLIC_ERROR_IDS' src/
  tests/`; `rg -n '"CODE:' src/ | head`; `rg -n 'semantic-work-not-applicable'
  src/continuous.rs`; `rg -n 'shutdown_steps' src/daemon.rs`
- Known touch: `src/daemon.rs`, `src/fabric/snapshot_catalog.rs`,
  `src/snapshot_runtime.rs`, `src/continuous.rs`, `src/lifecycle.rs`,
  `src/semantic_query.rs`, `rustc-extractor/src/wrapper.rs`, error enums
  across fabric modules.

**Required changes.**
1. Introduce the phase-carrying error envelope (generated `Phase` enum from
   the lifecycle/state registries) as the boundary error type of the fabric
   subsystems; the extractor wrapper gains a real error type.
2. Enforce registry closure: every `CODE:`-prefixed public error ∈
   `PUBLIC_ERROR_IDS` (the CONF DP-117 detector as a gate); register or
   rename the shadow vocabularies; raise the registered codes at the exact
   conditions they name.
3. Generate and consume the error-registry projections (`grpc_status`,
   `severity`, `retryability`, `mcp_mapping`) at the RPC boundary.
4. Build stage traces incrementally so failures name their phase (both
   snapshot pipelines).
5. Shutdown logs each step after completion; `DaemonExit` reports only
   observed steps.
6. The continuous engine evaluates transition guards before asserting them;
   a semantic-capability-requiring wave cannot reach
   `required-capabilities-terminal` without the semantic lane.

**Legacy disposition.** Shadow error vocabularies and phase-less envelope
paths deleted at cutover within the packet.

**Acceptance checks.**
- Behavioral: phase-injection tests fail each lifecycle phase and assert the
  reported phase and registered code.
  Executable oracle: `wp61_behavioral_acceptance`
  Governed criterion: `PC-WP61-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: error-code closure oracle green repo-wide.
  Executable oracle: `wp61_structural_acceptance`
  Governed criterion: `PC-WP61-STR` — mapped through this packet's Target invariants and Design references.
- Negative: guard-falsification — a Rust-bearing wave parks in the
  explicit non-terminal state (no silent terminal declaration).
  Executable oracle: `wp61_negative_zero_state`
  Governed criterion: `PC-WP61-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: forced-failure shutdown reports the true completed-step set.
  Executable oracle: `wp61_operational_acceptance`
  Governed criterion: `PC-WP61-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-check`, `just root-test`, `just governance-scan`.
**Milestone.** M11. **Replan triggers.** Envelope adoption forces a breaking
change on a wire error contract — boundary versioning decision (plan
revision). **Rollback.** Envelope adoption is per-subsystem; revert per
module.

### WP62 — Typed semantic request IR and relational lowering

**Outcome.** The semantic query plane parses and validates one typed,
snapshot-bound DAG IR; relational nodes lower through DataFusion built-ins;
semantic policy is enforced before lowering and structural policy after it;
advertised relational filters/projections and runtime states are truthful.

**Dependencies.** WP56, WP57, WP61.

**Target invariants.** GI-2, GI-4, GI-8; `PRIN P1/P2/P6/P14/P20`;
`SRV §6` inv. 5/6.
Design references: R2 (CONF DP-077, DP-080, DP-095, DP-098, DP-109, DP-110,
DP-111, DP-112, DP-123); ALIGN MOD-02/03/05/07, LOG-01–LOG-07, EXP-01/02;
DFREF §11, §19, §43, §46; LD-01.

**Change surface.**
- Preflight query: `rg -n 'SELECT \* FROM|format!' src/semantic_query.rs`;
  enumerate `QRY`'s eight forms, prior-result role types, DAG rules, graph
  operations, coverage/absence and deterministic-order requirements against
  current DTO variants and handlers;
  `rg -n '&.static str' src/semantic_query.rs`; `rg -n 'FreshnessBarrier::default|freshness_policy'
  src/query_service.rs src/lifecycle.rs`; `rg -n 'profile_digest|EffectiveLimitsProfile'
  src/query_service.rs`
- Known touch: `src/semantic_query.rs`, `src/query_service.rs`,
  `src/lifecycle.rs`, `src/fabric/serving.rs`.

**Required changes.**
1. Implement `ParsedSemanticRequest` → `TypedSemanticRequest` → snapshot-bound
   `BoundPlanSpec`: all block IDs, typed input/result roles, source JSON
   pointers, dependencies, fan-in/fan-out, cycle rejection, coverage
   prerequisites/effects, ordering, boundedness, memory and cancellation
   contracts are explicit.
2. Lower relational find/retrieve/follow selections, projections, filters,
   joins, and summaries through `LogicalPlanBuilder`/`Expr`; advertised DTO
   fields become real or leave the public schema. Keep graph nodes typed but
   unexecuted until WP75.
3. Reject evaluative intent on `TypedSemanticRequest` before binding; after
   binding, independently validate allowed tables, functions, and relational
   or graph node families. Lowering may not erase the refusal decision.
3. Result states become generated registry enums computed from execution:
   `limit_state` from fetch-vs-produced, `freshness_state` from the live
   barrier on success and failure paths, execution/completeness from
   runtime outcome; `failed_query_count`/`errors` real.
4. `EffectiveLimitsProfile.profile_digest` hashes the limit values (WP55
   constructors); `FreshnessState::Unavailable` gains its production writer
   via the continuous engine or is withdrawn from the advertised set.
5. Results are Arrow-native typed projections; public IDs mint through
   `identity::encode_public_id`. Support advertisement is per form and checked
   before snapshot work.

**Legacy disposition.** Static states and literal-prefixed IDs retire here.
The SQL path remains transition-only until WP75 proves all eight forms and DB10
decommissions it.

**Acceptance checks.**
- Behavioral: typed relational forms reproduce the transition SQL path where
  comparable; filters/projections execute; arbitrary independent/dependent
  block parsing and typed role binding are proved without claiming graph-form
  completion.
  Executable oracle: `wp62_behavioral_acceptance`
  Governed criterion: `PC-WP62-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: state-literal ban rule green (no registered-domain string
  literals outside generated code).
  Executable oracle: `wp62_structural_acceptance`
  Governed criterion: `PC-WP62-STR` — mapped through this packet's Target invariants and Design references.
- Negative: evaluative intent fails before planning; a mutated bound plan
  touching an unapproved table/function fails after binding; cycles and role
  mismatches fail before snapshot work; stale/limited/failed states are true.
  Executable oracle: `wp62_negative_zero_state`
  Governed criterion: `PC-WP62-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: plan-validation rejections carry phase + registered code.
  Executable oracle: `wp62_operational_acceptance`
  Governed criterion: `PC-WP62-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-check`, `just root-test`, `just governance-scan`.
**Milestone.** M11. **Replan triggers.** The accepted typed IR cannot express
a `QRY` role/effect without changing public semantics, or a relational node
would require a custom DataFusion extension. **Rollback.** Typed parsing and
relational lowering land behind form advertisement; no SQL cutover yet.

### WP75 — Graph operators, eight-form scheduling, and query conformance

**Outcome.** All eight `QRY` forms and arbitrary acyclic composition execute
through one scheduler over relational `LogicalPlan` and typed
`GraphOperatorPlan` nodes, with explicit coverage/unknown/absence semantics and
canonical per-form order.

**Dependencies.** WP62; WP60 for cancellation and provider/runtime readiness.

**Target invariants.** GI-2, GI-4, GI-8, GI-12; `PRIN P2/P6/P14/P15/P20`;
`QRY §§4.2–4.10, 15–17, 21, 30, 33, 106–107`. Design references: R2,
LD-01, LD-11; audit F-002.

**Change surface.**
- Preflight query: enumerate all `QueryForm` variants, parsers, result DTOs,
  schedulers, graph/path/pattern algorithms, phrase bindings, coverage proof
  writers, response encoders, and public capability advertisement across Rust,
  proto, Python schema projections, fixtures, and gates.
- Known touch: semantic-query typed IR/compiler/scheduler, application graph
  execution substrate, response assembly, query capability registry, and
  eight-form conformance fixtures.

**Required changes.**
1. Implement graph traversal, bounded/shortest connecting paths, and
   conjunctive pattern operators over the existing graph substrate; operators
   declare typed Arrow input/output schema, semantic order, coverage effect,
   memory bound, cancellation, and cost attribution.
2. Implement combine-sets, deterministic objective summaries, and
   source/syntax-context forms; support prior-result references for every role
   allowed by `QRY`.
3. Schedule arbitrary acyclic blocks with fan-in/fan-out; preserve block
   identity and one logical response; never omit or silently truncate a block.
4. Materialize explicit unknown, candidate, completeness, coverage and
   negative-proof outputs. Empty rows alone never prove absence.
5. Apply canonical total order for every form before offset/fetch and response
   encoding. Advertise all eight forms only after their individual and composed
   conformance rows pass.

**Legacy disposition.** Three-form-only capability, fixed pipeline
assumptions, and graph semantics hidden behind relational approximations
retire. DB10 removes the SQL path after this packet and WP64 are proved.

**Acceptance checks.**
- Behavioral: every form plus mixed relational/graph fan-in/fan-out DAGs
  executes through the production query-service harness and returns one
  correctly typed response.
  Executable oracle: `wp75_behavioral_acceptance`
  Governed criterion: `PC-WP75-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: each public form has parser, typed IR, lowering, executor,
  response schema, capability row, and focused fixture; graph semantics do not
  appear in SQL/UDF/custom DataFusion nodes.
  Executable oracle: `wp75_structural_acceptance`
  Governed criterion: `PC-WP75-STR` — mapped through this packet's Target invariants and Design references.
- Negative: cycles, invalid role edges, unsupported pre-advertisement forms,
  bounded-path overflow, cancellation, unknown coverage, and evaluative intent
  fail/report exactly as specified.
  Executable oracle: `wp75_negative_zero_state`
  Governed criterion: `PC-WP75-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `semantic-query-conformance-check` runs all eight forms,
  composition, ordering, and coverage/absence with `--no-tests=fail`.
  Executable oracle: `wp75_operational_acceptance`
  Governed criterion: `PC-WP75-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just semantic-query-conformance-check`, `just root-test`,
`just wave5-integration-check`. **Milestone.** M11. **Replan triggers.** A
required graph semantic cannot be expressed by the accepted application plan,
or execution requires a custom DataFusion logical/physical extension.
**Rollback/recovery.** Form advertisement is additive per completed form; the
old path remains transition-only until all eight pass and DB10 exits.

### WP63 — Daemon activation of the query vertical

**Outcome.** The query service and continuous engine are reachable from
`daemon::serve`: the socket is declared, the vertical is constructed on the
production path, exact per-form support is advertised, and an eight-form
composed end-to-end KAT proves it.

**Dependencies.** WP60, WP75.

**Target invariants.** GI-2; `PRIN P25` (unproven-in-reachable-code);
`LIFE §122` topology. Design references: R2 (CONF DP-075, DP-105); W17
scope note in proposal R2.

**Change surface.**
- Preflight query: `rg -n 'StaticConfig' src/daemon.rs`; `rg -l
  'crate::(query_service|continuous|derivation|golden_corpus)' src/ tests/`
  (currently empty — the island detector); `rg -n 'ContinuousWorkspaceEngine::'
  src/`
- Known touch: `src/daemon.rs`, `src/coordinator.rs`, `src/query_service.rs`,
  `src/continuous.rs`.

**Required changes.**
1. `StaticConfig` declares the query socket; `daemon::serve` binds it and
   hosts `ProductionQueryService` over the real serving lease.
2. The coordinator constructs `ContinuousWorkspaceEngine` on the production
   path; `CORE_SOURCE_V1` coverage is computed and returned through the
   status surface.
3. Activation scope: bind, handshake, one composed request covering all eight
   forms end-to-end against a real snapshot, streamed logical response,
   support/status. Full W17 RPC scope
   remains W17's; this packet makes the island reachable so its proofs
   count.

**Legacy disposition.** None (additive reachability).

**Acceptance checks.**
- Behavioral: end-to-end KAT — daemon boots, adapter-side stub connects over
  UDS, the eight-form composed query returns one golden response with computed
  states and no omitted block.
  Executable oracle: `wp63_behavioral_acceptance`
  Governed criterion: `PC-WP63-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: island detector inverted — the query modules have inbound
  edges from `daemon::serve` (non-empty reference census).
  Executable oracle: `wp63_structural_acceptance`
  Governed criterion: `PC-WP63-STR` — mapped through this packet's Target invariants and Design references.
- Negative: unauthorized peer UID rejected on the query socket.
  Executable oracle: `wp63_negative_zero_state`
  Governed criterion: `PC-WP63-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: daemon shutdown under an in-flight query cancels cleanly
  (WP60 cancellation, WP61 honest shutdown).
  Executable oracle: `wp63_operational_acceptance`
  Governed criterion: `PC-WP63-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-test`, `just wave5-integration-check`.
**Milestone.** M11. **Replan triggers.** Activation surfaces a W17-owned
contract decision (e.g., socket lifecycle policy) the specs do not settle —
route per `RM §28`. **Rollback.** Socket binding is config-gated.

### WP64 — Deterministic result identity and modeled reproducibility

**Outcome.** Canonical delivered order precedes pagination; versioned
`ResultChecksumV1` proves schema plus multiset rows; plan-template, bound
semantic-query, request, and execution identities are distinct; reproducibility
is modeled.

**Dependencies.** WP75.

**Target invariants.** GI-4, GI-12; `PRIN P18/P19`. Design references: R3
(CONF DP-012); ALIGN MOD-06, OBS-10, RUN-10; LD-03, LD-09; DFREF §21, §56.

**Change surface.**
- Preflight query: `rg -n 'result_checksum|hasher.update' src/fabric/serving.rs`;
  `rg -n 'query_id' src/fabric/serving.rs src/query_service.rs`;
  `rg -n 'arrow_row|RowConverter|SortExpr|offset|fetch' src/`; enumerate every
  output Arrow type and schema-metadata key; inspect `arrow-row` version and
  unsupported/Map/float/zero-column semantics.
- Known touch: `src/fabric/serving.rs`, `src/query_service.rs`.

**Required changes.**
1. Enforce each form's canonical total order in WP75 before offset/fetch and
   response encoding; checksum sorting never substitutes for delivered order.
2. Define `ResultChecksumV1`: canonical schema bytes include field name,
   type, nullability, ordered metadata and extension metadata; hash row count
   plus length-framed sorted row encodings as a multiset. Fix sort/null,
   dictionary, Map-key, signed-zero/NaN, nested/view/extension, empty and
   zero-column semantics, memory bounds, and exact Arrow/`arrow-row` domain.
3. Define a versioned plan-template serialization for typed semantic DAG,
   relational expressions, graph nodes, aliases/qualifiers, scalar values,
   table/provider/function identity and child order. Define semantic query
   identity by adding canonical bound spec/parameters, snapshot manifest, and
   config. Request and execution identity remain separate in WP65.
4. Add the `Reproducibility` record (deterministic, inputs_pinned,
   volatile_functions, environment_recorded) to `QueryPlanArtifact`,
   derived from the plan and the environment snapshot.

**Legacy disposition.** The order-sensitive hasher and `f(sql, snapshot)`
query id deleted — DB10.

**Acceptance checks.**
- Behavioral: same bound spec/frozen snapshot under varied partitions, batch
  sizes and pagination yields identical delivered row order, response bytes,
  and checksum.
  Executable oracle: `wp64_behavioral_acceptance`
  Governed criterion: `PC-WP64-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: checksum and fingerprint domains registered (WP55 census).
  Executable oracle: `wp64_structural_acceptance`
  Governed criterion: `PC-WP64-STR` — mapped through this packet's Target invariants and Design references.
- Negative: duplicate multiplicity, bound-parameter, schema metadata, nested,
  Map, dictionary, signed-zero/NaN and empty/zero-column fixtures follow the
  versioned contract; semantic mutations change the right identity while a
  row-order permutation does not.
  Executable oracle: `wp64_negative_zero_state`
  Governed criterion: `PC-WP64-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: replay under pinned environment reproduces the artifact's
  recorded checksum.
  Executable oracle: `wp64_operational_acceptance`
  Governed criterion: `PC-WP64-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just query-determinism-check`, `just root-test`.
**Milestone.** M11. **Replan triggers.** An output type cannot be represented
under the accepted checksum contract, the required memory bound needs a new
external-sort design, or plan serialization cannot cover an accepted node.
No per-form unversioned deviation is permitted. **Rollback.** Old and v1
checksums coexist only in a version-labelled differential window; cutover
requires migration/rejection semantics.

### WP65 — Execution identity and the persisted artifact bundle

**Outcome.** An execution identity allocated before planning joins request,
plans, metrics, and result; the full `QueryPlanArtifact` is persisted with a
complete interpretation-context pin.

**Dependencies.** WP63, WP64.

**Target invariants.** GI-6; `PRIN P9/P17/P24`. Design references: R4
(CONF DP-013-residual, DP-035, DP-036, DP-053, DP-056); ALIGN OBS-01–OBS-06,
RUN-05; LD-06.

**Change surface.**
- Preflight query: `rg -n 'result_artifact_lease' src/ contracts/`;
  `rg -n 'semantic_request_id|mcp_call_id' src/ --glob '!src/generated/**'`;
  `rg -n 'schema_bundle_id|overlay_generation' src/fabric/serving.rs src/snapshot.rs`;
  `rg -n 'capture_control_schema' src/fabric/serving.rs`
- Known touch: `src/fabric/serving.rs`, `src/query_service.rs`,
  `src/operational_store.rs`, `src/snapshot.rs`.

**Required changes.**
1. Allocate `execution_id` before planning; thread `semantic_request_id` and
   `mcp_call_id` proto → handler → artifact → trace spans; propagate through
   `TaskContext`.
2. Persist `QueryPlanArtifact` with ordinary logical/optimized/physical
   `EXPLAIN`, the WP64 `Reproducibility` record, and PG-JSON/full metrics
   rendered after consuming the exact served physical-plan instance through
   `DisplayableExecutionPlan`. Governed serving never creates `AnalyzeExec` or
   re-executes for diagnostics. Key the artifact by execution identity and
   apply the retention policy.
3. Complete the pin: all seven bundle IDs, overlay generation/digest,
   overlay-supplied table versions; the control-schema capture stamped with
   a generation fingerprint recorded beside `source_table_versions`.
4. Complete `capability_status` population: `reason_code`/`diagnostic_id`
   emitted so unknown is expressible; the query service advertises real
   statuses and fingerprints.

**Legacy disposition.** The artifact-drop path and empty advertised vectors
deleted.

**Acceptance checks.**
- Behavioral: a served query's artifact is readable back with complete pins
  and metrics from the exact serving plan; a counting provider observes one
  scan/execution; two agents issuing semantically identical specs remain
  distinguishable by request identity.
  Executable oracle: `wp65_behavioral_acceptance`
  Governed criterion: `PC-WP65-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: artifact schema versioned; artifact rows carry every declared
  pin field non-null for golden queries.
  Executable oracle: `wp65_structural_acceptance`
  Governed criterion: `PC-WP65-STR` — mapped through this packet's Target invariants and Design references.
- Negative: failure, cancellation, and stream-drop persist partial metrics and
  phase without diagnostic re-execution.
  Executable oracle: `wp65_negative_zero_state`
  Governed criterion: `PC-WP65-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: artifact persistence and retention follow the declared lease
  policy (expiry *enforcement* on reads is WP67's oracle, not this one).
  Executable oracle: `wp65_operational_acceptance`
  Governed criterion: `PC-WP65-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just query-artifact-single-execution-check`, `just root-test`,
`just wave5-integration-check`.
**Milestone.** M12. **Replan triggers.** Artifact volume forces a retention
design the operational store cannot express — plan revision. **Rollback.**
Artifact persistence is additive; write path feature-gated until proof.

### WP66 — Provenance closure of durable state

**Outcome.** From any committed Delta version, the chain commit → execution
→ plans → specs → inputs → schema fingerprints → source snapshots resolves;
retention protects the explainers; an operator surface answers "why does
this row exist".

**Dependencies.** WP65, WP74.

**Target invariants.** GI-6; `PRIN P10/P11`; `HOL P22/P27`. Design
references: R4 (CONF DP-022, DP-023, DP-027, DP-051, DP-052, DP-054,
DP-055); ALIGN OBS-08, OBS-12; LD-07.

**Change surface.**
- Preflight query: `rg -n 'owner_id' src/provider_runtime.rs src/fact_ingest.rs`;
  `rg -n 'INSERT INTO update_wave' -g '*.rs' .`; `rg -n 'workspace_scope: None'
  src/generated/table_specs.rs`; `rg -n 'source_blob_digests' src/snapshot*.rs`;
  `rg -n 'SnapshotRetentionSet|cleanup_terminal_before' src/`
- Known touch: `src/provider_runtime.rs`, `src/continuous.rs`,
  `src/operational_store.rs`, `src/fabric/mutation.rs`, `src/snapshot.rs`,
  `src/snapshot_runtime.rs`, `src/source_image.rs`, schema IR for
  `table_mutation_operation`.

**Required changes.**
1. `provider_run.owner_id` encoded as Id16 (join restored); the continuous
   engine's existing `update_wave`/`update_wave_item` writers are retained and
   checked against WP73's superseding DP-022 disposition; implement only
   residual identity/completeness defects, including Id16 validation.
2. `table_mutation_operation` gains typed scope columns and a workspace
   scope; the three no-preimage digest keys gain stored preimages or are
   re-documented as integrity fields (decision recorded in the WP57
   metadata dictionary).
3. `source_blob_digests` joins `ServingSnapshotManifestBody`
   (content-addressed snapshot→bytes link). Apply the accepted `GEN §13`
   contract: `file_id` is exact file identity in the snapshot; occurrence IDs
   additionally carry source digest/range, normalized family/role, structural
   anchor, and owner. Location alone is never fact identity.
4. `SnapshotRetentionSet` unions provenance reachability; terminal cleanup
   checks publication reachability; source-blob GC consults retained
   publications.
5. `explain_version(table_code, delta_version)` reads Delta `history()` and
   the artifact store; exposed on the admin/status surface.

**Legacy disposition.** Substring-parsing of `application_id` as the only
scope join retired.

**Acceptance checks.**
- Behavioral: closure-traversal oracle — from a committed version, resolve
  every required link; fails on any missing link.
  Executable oracle: `wp66_behavioral_acceptance`
  Governed criterion: `PC-WP66-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: wave/provider/fact join proved on the golden corpus (the
  restored `owner_id` join returns rows).
  Executable oracle: `wp66_structural_acceptance`
  Governed criterion: `PC-WP66-STR` — mapped through this packet's Target invariants and Design references.
- Negative: GC cannot delete a retained publication's explainers
  (fault-injected retention test).
  Executable oracle: `wp66_negative_zero_state`
  Governed criterion: `PC-WP66-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `explain_version` answers for a golden publication within
  bounded time.
  Executable oracle: `wp66_operational_acceptance`
  Governed criterion: `PC-WP66-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-test`, `just vacuum-dry-run-check`,
`just wave6-integration-check`. **Milestone.** M12. **Replan triggers.**
Scope-column addition to `table_mutation_operation` is a schema change under
the WP57 evolution policy — if the policy's migration route proves
insufficient, plan revision. **Rollback.** Retention widening is
conservative (protects more, never less).

### WP67 — Daemon boundary hardening and one contract family

**Outcome.** Keyed tokens, distinct cancel tokens, enforced leases and
claims, a schema-covered admin protocol, typed feature negotiation, and a
single model-index decode contract.

**Dependencies.** WP56, WP61, WP63, WP65.

**Target invariants.** GI-4; `PRIN P13/P20/P22`; `SRV §6` inv. 4/10. Design
references: R8 (CONF DP-048, DP-066, DP-067, DP-078, DP-081, DP-087,
DP-093); ALIGN GOV-06, INT-10.

**Change surface.**
- Preflight query: `rg -n 'fn opaque_bytes' -A 6 src/query_service.rs`;
  `rg -n 'cancel_token' src/query_service.rs contracts/rpc/`;
  `rg -n 'lease_expires_at_unix_ms|permission_claims' src/query_service.rs`;
  `rg -n 'negotiate_feature_bits' src/ tests/`; `rg -n 'AdminEnvelope'
  src/daemon.rs`; `rg -n 'include_bytes!' src/contracts/index.rs`
- Known touch: `src/query_service.rs`, `src/rpc.rs`, `src/daemon.rs`,
  `contracts/rpc/`, `src/contracts/index.rs`,
  `codefabric-cpg-mcp/.../contracts/index.py`.

**Required changes.**
1. `opaque_bytes` uses the keyed construction from the lease secret; cancel
   tokens minted distinct from resume tokens per the proto's separation;
   `stream_query` authorizes the workspace.
2. `read_result` enforces `lease_expires_at_unix_ms`;
   `permission_claims` consumed by `authorize_workspace` or removed.
3. Feature registry projected into typed masks in both languages;
   `negotiate_feature_bits` called in the handshake with `required`
   semantics enforced.
4. The admin newline-JSON protocol gains a schema artifact under
   `contracts/`; peer-UID policy single-sourced in the interceptor.
5. The model-index seam becomes one decode contract with a differential
   test; the host capability profile digest gets a derivation rule and
   daemon-side validation, or the fields are removed.

**Legacy disposition.** Unkeyed token path, inline peer-UID copy, untyped
feature mask deleted.

**Acceptance checks.**
- Behavioral: forged-token attempts fail (unkeyed derivation no longer
  verifies); handshake rejects a client missing a required feature bit.
  Executable oracle: `wp67_behavioral_acceptance`
  Governed criterion: `PC-WP67-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: admin protocol messages validate against the new schema
  artifact; Rust/Python index decoders agree on a differential corpus.
  Executable oracle: `wp67_structural_acceptance`
  Governed criterion: `PC-WP67-STR` — mapped through this packet's Target invariants and Design references.
- Negative: expired lease read rejected; cancel with a resume token
  rejected.
  Executable oracle: `wp67_negative_zero_state`
  Governed criterion: `PC-WP67-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just wave5-integration-check` green with hardened paths.
  Executable oracle: `wp67_operational_acceptance`
  Governed criterion: `PC-WP67-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just root-test`, `just model-check`, `just adapter-ci-fast`.
**Milestone.** M13. **Replan triggers.** Token-scheme change invalidates a
persisted lease format — bounded migration decision. **Rollback.**
Keyed-token cutover within one release boundary; no dual acceptance beyond
the packet.

### WP68 — The strictly-presentational adapter

**Outcome.** The adapter passes daemon state through unmodified, holds no
authority, tests its production surface, and its contract schemas validate
the boundary they describe.

**Dependencies.** WP63, WP67.

**Target invariants.** GI-9; `SRV §6` inv. 3/6/8/9/11; `RM §1` inv. 8.
Design references: R8 (CONF DP-064, DP-065, DP-070, DP-072, DP-088, DP-089,
DP-090, DP-091, DP-092, DP-099-adapter); ALIGN P22.

**Change surface.**
- Preflight query: `rg -n 'COMPLETE|POTENTIALLY_STALE' codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py`;
  `rg -n '_leased_artifacts|while True' codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py`;
  `rg -n 'probe_mcp' codefabric-cpg-mcp/tests/`; `rg -n 'from mcp' codefabric-cpg-mcp/src/`
- Known touch: `server.py`, `daemon/client.py`, `settings.py`,
  `tests/test_adapter_contracts.py`, `tests/test_server.py`,
  `pyproject.toml`, `rules/no-framework-internal-contract-imports.yml`.

**Required changes.**
1. `server.py` passes the daemon's registry state values through unmodified
   (WP56 generated Python registries carry the full vocabulary); the
   hardcoded `COMPLETE`s and narrowing ternaries deleted; `truncated`
   comes from the daemon.
2. One request encoding: canonical request bytes with typed fields generated
   from the contract; the client's freshness-policy re-mapping and dict
   re-extraction deleted.
3. Structured daemon error records presented with code/path preserved.
4. Lease existence daemon-authoritative (`_leased_artifacts` demoted to a
   cache over a daemon status call); registered URI template for reference
   resources; the unbounded `ReadResult` loop gains a bounded retry
   contract.
5. `Settings` one instance per process with an oracle; the `mcp.*` rule gap
   closed and the dependency declared.
6. The adapter contract suite drives the production `mcp` object end-to-end
   against real stubs; the tool-manifest fingerprint gains an accepted
   baseline; generated MCP schemas validate the production boundary.

**Legacy disposition.** Probe-server-only tests and monkeypatched client
tests superseded by stub-backed protocol tests.

**Acceptance checks.**
- Behavioral: state-fidelity tests — forced non-success daemon outcomes are
  reported verbatim through MCP (`SRV §6` inv. 6/8/9 as oracles).
  Executable oracle: `wp68_behavioral_acceptance`
  Governed criterion: `PC-WP68-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: production tool manifest matches its accepted fingerprint
  baseline; boundary payloads validate against the generated schemas.
  Executable oracle: `wp68_structural_acceptance`
  Governed criterion: `PC-WP68-STR` — mapped through this packet's Target invariants and Design references.
- Negative: adapter cannot answer a lease query from local state when the
  daemon says revoked (differential test).
  Executable oracle: `wp68_negative_zero_state`
  Governed criterion: `PC-WP68-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just adapter-ci-fast` and `just adapter-stdio-test` green.
  Executable oracle: `wp68_operational_acceptance`
  Governed criterion: `PC-WP68-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just adapter-ci-fast`, `just adapter-stdio-test`,
`just adapter-wheel-test`. **Milestone.** M13. **Replan triggers.** A
pass-through value has no MCP representation — that is a `SRV` contract gap,
route per `RM §28`. **Rollback.** Adapter changes are presentation-local.

### WP69 — Model-compiler truth and derived-artifact repair

**Outcome.** The model plane's checks can fail, its Derived claims drive
production and deletion, its traceability carries normative content, and
its vocabularies have one registration.

**Dependencies.** WP56.

**Target invariants.** GI-1, GI-7; `PRIN P2/P3`; `HOL §6` pass contracts.
Design references: R10 (CONF DP-004, DP-006, DP-007, DP-008, DP-041,
DP-042, DP-044, DP-045, DP-046, DP-069, DP-073, DP-097); ALIGN MOD-01.

**Change surface.**
- Preflight query: `rg -n 'current_outputs.get' src/bin/codefabric_model/desired_tree.rs`;
  `rg -n 'model_outputs()' src/bin/codefabric_model/aggregate_driver.rs`;
  `cmp contracts/generated/model/governance/requirements.jsonl
  contracts/generated/model/governance/traceability.jsonl`; `rg -n
  'ArtifactRole::Derived' src/bin/codefabric_model/`; `rg -n 'strip_prefix("CREATE TABLE'
  src/operational_store.rs`
- Known touch: `src/bin/codefabric_model/{desired_tree.rs,incremental.rs,
  aggregate_driver.rs,transaction.rs,repository_model.rs}`,
  `src/contracts/{jcs.rs,models.rs}`, `src/operational_store.rs`,
  `src/derivation.rs`, `contracts/registry/derivation-registry.yaml`.

**Required changes.**
1. Desired bytes derive from the model; `ModelPlan::check` can fail; the
   reported action graph is the real transaction's; one action-key scheme.
2. Census computed after all outputs inserted; Derived claims drive stale
   deletion (the 17 unproduced registry projections and the stale
   arrow-delta copy regenerated or deleted).
3. Requirements parsed from the `AC-G` corpus; `verified_by` names real
   oracles; requirements/traceability distinct; the consumer graph names
   real consumers or the field is removed; adapter artifacts indexed.
4. Drivers use `codefabric-jcs-v1`; the operational store consumes typed
   generated specs (SQL re-parsing deleted); dead contract models adopted
   or deleted.
5. Derivation registry populated; bundle membership declarative;
   `derivation.rs` reads expectations from the contract, not a second copy.
6. `compatibility.rs` documented as a library-probe tier (CONF DP-073
   accepted-observation closure).

**Legacy disposition.** Shadow plan, byte-identical traceability twin,
SQL-text re-parsers, dead vocabulary deleted — DB12.

**Acceptance checks.**
- Behavioral: perturb one governed output ⇒ `model-plan` reports a non-empty
  change set (the CONF DP-004 inversion, as an expected-failure fixture).
  Executable oracle: `wp69_behavioral_acceptance`
  Governed criterion: `PC-WP69-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: census oracle — suite manifest count == committed tree ==
  validation count; requirements corpus non-templated (distinct normative
  texts > 1).
  Executable oracle: `wp69_structural_acceptance`
  Governed criterion: `PC-WP69-STR` — mapped through this packet's Target invariants and Design references.
- Negative: unproduced-Derived census zero; JCS-bypass zero-hit in drivers.
  Executable oracle: `wp69_negative_zero_state`
  Governed criterion: `PC-WP69-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just model-release-check` green.
  Executable oracle: `wp69_operational_acceptance`
  Governed criterion: `PC-WP69-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just model-check`, `just model-plan-check`,
`just model-transaction-check`, `just model-repro-check`,
`just model-release-check`. **Milestone.** M09. **Replan triggers.** Real
drift surfaces once the shadow is removed (the check was vacuous) — triage
as baseline failures, not new defects. **Rollback.** Model-plane only; no
runtime surface.

### WP70 — Rule restoration, legacy-oracle repair, and consumed fixtures

**Outcome.** Legacy alias/source-text oracles are replaced under WP54's already
active proof contract; rule coverage is restored; negative fixtures and
registries have real consumers; wave/CI gates run the new packet selectors.

**Dependencies.** WP54, WP68, WP69 (the foundation is WP54; this packet lands
late Python/model rule and fixture consumers after their production surfaces).

**Target invariants.** GI-7; `PRIN P25`. Design references: R9 (CONF
DP-057, DP-058, DP-059, DP-060, DP-061, DP-062, DP-068, DP-071, DP-099,
DP-104, DP-108); ALIGN TST-01–TST-14.

**Change surface.**
- Preflight query: `rg -n 'fn wp(49|50|52)_' tests/integration/git_state.rs`;
  `rg -n 'rust_protobuf_matches' .github/workflows/ci.yml`; `rg -n 'ignores:'
  rules/authoritative-source-read-boundary.yml`; `rg -n 'snapshotDir|skip-snapshot-tests'
  sgconfig.yml justfile`; `rg -c 'AC-G' tests/ rules/ justfile`
- Known touch: `tests/integration/`, `rules/`, `rule-tests/`, `justfile`,
  `.github/workflows/ci.yml`, `tooling/ci/artifact_contracts.py`,
  `contracts/fixtures/`, `contracts/{security,faults,comparison}/`.

**Required changes.**
1. Under WP54's already-green alias detector, replace the five legacy aliases
   with real acceptance tests meeting their governed criterion sentences.
2. Verify all new/touched oracles retain their `PC-WPNN-*` mappings to the
   applicable AC-G/design/conformance authority; do not require invented AC-G
   ownership for plan-local criteria.
3. The CI cross-language step gains its Rust test — decode of the shared
   wire fixture (`--no-tests=fail` on every gate selector).
4. Source-text oracles replaced by structural rules or decoded-artifact
   assertions (the register's 10+ census is the worklist).
5. Rule restoration: re-narrow the widened `ignores` (new modules route
   reads through `secure_path`); re-scope
   `provider-observation-boundary-only` to real paths; enable snapshot
   tests (`__snapshots__` created, `--skip-snapshot-tests` dropped); add
   Python rules covering `server.py`/`settings.py`/`__main__.py`/
   `channel.py`.
6. Negative fixtures consumed by the released verifier; the zero-digest
   released manifest re-released; security-corpus, fault-point (census
   reconciled to code), and comparison-ignore registries executed by the
   suites that cite them.
7. Wave/gate recipes wired into `ci-fast`/`ci-pr`/CI so green means ran.

**Legacy disposition.** Alias oracles, dead fixtures-without-consumers, and
the vacuous CI step retired.

**Acceptance checks.**
- Behavioral: the negative-fixture family fails its verifier when
  perturbed (each fixture has a consumer that can reject).
  Executable oracle: `wp70_behavioral_acceptance`
  Governed criterion: `PC-WP70-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: alias detector and governed-criterion mapping green repo-wide;
  rule/rule-test 1:1 held with snapshots on.
  Executable oracle: `wp70_structural_acceptance`
  Governed criterion: `PC-WP70-STR` — mapped through this packet's Target invariants and Design references.
- Negative: an intentionally-added alias test fails the gate
  (expected-failure fixture); a gate selector matching zero tests fails.
  Executable oracle: `wp70_negative_zero_state`
  Governed criterion: `PC-WP70-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: CI configuration mirrors justfile recipes for every new gate.
  Executable oracle: `wp70_operational_acceptance`
  Governed criterion: `PC-WP70-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just governance`, `just governance-scan`, `just artifacts-check`,
`just ci-fast`. **Milestone.** M14. **Replan triggers.** Re-narrowed rules
surface real boundary violations in the new modules — those are WP-scoped
fixes, recorded as discovered obligations. **Rollback.** Governance-layer
only.

### WP71 — Golden-corpus execution and review candidates

**Outcome.** The sixteen-plus edit scenarios and Gate B's eleven items execute
through the real engine to produce independently diffable review candidates;
this packet cannot release or owner-accept its own outputs.

**Dependencies.** WP63, WP64, WP66, WP70 (the `ci-pr` wave-gate wiring this
packet's operational check relies on is WP70's).

**Target invariants.** GI-7; `PRIN P25`; `SUITE` Gate B; `RM §10`. Design
references: R9 (CONF DP-082, DP-101, DP-102, DP-113, DP-114, DP-116,
DP-121); ALIGN TST-06, TST-11.

**Change surface.**
- Preflight query: `rg -c 'scenario\.json' --type rust .` (currently 0);
  `jq . tests/golden/codefabric-golden-v1/corpus-manifest.json | head`;
  `sed -n '155,160p' justfile` (gate-b-check body); `rg -n 'is_subset'
  src/golden_corpus.rs`
- Known touch: `src/golden_corpus.rs`, `tests/golden/codefabric-golden-v1/`,
  `justfile`, `src/continuous.rs`.

**Required changes.**
1. Scenario runner: deserialize `scenario.json`, apply the named edits
   through the watcher/wave path, assert terminal states; implement the
   missing scenario classes (overflow, multi-file logical save, context
   change, capability withdrawal).
2. Generate candidate outputs (IDs, rows, response bytes, checksums) from the
   activated vertical into a review-candidate location. Produce an independent
   spec/registry-derived expected-vs-candidate diff and candidate digest;
   verify manifest fields. Candidate status cannot be `released`.
3. `gate-b-candidate-check` runs the eleven `SUITE` items end to end and emits
   the review bundle. Final `gate-b-check` remains blocked on WP76's accepted
   release artifact.
4. Registry-derived expectations import generated constants and use
   equality, not subset.

**Legacy disposition.** Descriptor-only/self-hash candidate generation
retires; released `expected/` remains unchanged until WP76.

**Acceptance checks.**
- Behavioral: all scenarios execute to their expected terminals and Gate B's
  eleven items produce a complete candidate/diff bundle.
  Executable oracle: `wp71_behavioral_acceptance`
  Governed criterion: `PC-WP71-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: candidate digest chain and independent derivation inputs verify;
  zero `is_subset` comparisons against registry authorities.
  Executable oracle: `wp71_structural_acceptance`
  Governed criterion: `PC-WP71-STR` — mapped through this packet's Target invariants and Design references.
- Negative: an unexecuted scenario, self-referential digest, missing diff, or
  candidate marked released without WP76 acceptance fails.
  Executable oracle: `wp71_negative_zero_state`
  Governed criterion: `PC-WP71-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just gate-b-candidate-check` is green and produces the exact
  review bundle consumed by WP76.
  Executable oracle: `wp71_operational_acceptance`
  Governed criterion: `PC-WP71-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just gate-b-candidate-check`, `just wave5-integration-check`,
`just wave6-integration-check`. **Milestone.** M14 prerequisite.
**Replan triggers.** Candidate output disagrees with normative intent or an
independent expectation cannot be derived — a spec-feedback event per
`RM §28`. **Rollback/recovery.** Candidate artifacts are unreleased and may be
discarded; released corpus bytes are not touched.

### WP76 — Accountable golden-answer acceptance and Gate B release

**Outcome.** An accountable owner reviews the exact WP71 candidate bundle,
records a versioned acceptance decision, and releases a new immutable corpus
version; Gate B then compares execution against that accepted authority.

**Dependencies.** WP71. This packet contains an external human checkpoint;
the agent must stop after producing the review bundle and cannot self-approve.

**Target invariants.** GI-7, GI-12; `PRIN P19/P25`; `SUITE` Gate B.
Design references: R9 (CONF DP-101, DP-113, DP-114, DP-116); audit F-015.

**Change surface.**
- Preflight query: verify WP71 candidate/diff digest, current released corpus
  version, acceptance-authority registry, manifest/corpus status, and every
  consumer of `expected/` and `gate-b-check`.
- Known touch: a new versioned acceptance artifact, a new corpus version,
  corpus manifest/index, `gate-b-check`, and `ci-pr` wiring.

**Required changes.**
1. Present the immutable WP71 candidate bundle and independently derived diff
   to the named owner; pause until an explicit accept/reject decision arrives.
2. Record candidate digest, source spec/registry versions, reviewer/authority,
   decision, timestamp, and superseded corpus version in a versioned acceptance
   artifact through a confirm-gated mutating
   `just gate-b-owner-accept <candidate-bundle> <acceptance-artifact>` recipe.
   The executor may prepare and validate the inputs but must not invoke the
   accountable acceptance action. Rejection routes back to design/spec
   feedback; no bytes release.
3. On acceptance, publish a new immutable corpus version and update the corpus
   index atomically. Never overwrite or silently revert an accepted version.
4. Implement `gate-b-owner-acceptance-check` and make `gate-b-check` require
   the accepted artifact/digest before executing all eleven comparisons.

**Legacy disposition.** Self-acceptance, mutable released answers, and
fixture-local rollback claims retire.

**Acceptance checks.**
- Behavioral: Gate B's eleven items pass against the newly accepted immutable
  corpus and exact source versions.
  Executable oracle: `wp76_behavioral_acceptance`
  Governed criterion: `PC-WP76-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: corpus index, manifest, candidate digest, acceptance artifact,
  owner authority, and released bytes form one verified chain.
  Executable oracle: `wp76_structural_acceptance`
  Governed criterion: `PC-WP76-STR` — mapped through this packet's Target invariants and Design references.
- Negative: missing/rejected/wrong-digest/self-authored acceptance blocks
  release and Gate B; old corpus versions remain byte-identical.
  Executable oracle: `wp76_negative_zero_state`
  Governed criterion: `PC-WP76-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `gate-b-owner-acceptance-check`, `gate-b-check`, and `ci-pr`
  use the same accepted corpus version.
  Executable oracle: `wp76_operational_acceptance`
  Governed criterion: `PC-WP76-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just gate-b-owner-acceptance-check`, `just gate-b-check`,
`just ci-pr`. **Milestone.** M14. **Replan triggers.** Owner rejects the
candidate or identifies a spec/implementation ambiguity. **Rollback/recovery.**
Publish a superseding corpus version; never rewrite an accepted one.

### WP72 — Convergence, parity, and process closure

**Outcome.** AC-G-79 convergence is proved by a true clean rebuild compared
over effective state; gix parity and disabled-configuration equivalence are
implemented; the process findings are closed.

**Dependencies.** WP66, WP76.

**Target invariants.** GI-7; `PRIN P19/P25`; `RM §1` inv. 10; `AC-G-79
§79.2`. Design references: R9 (CONF DP-100, DP-103, DP-106, DP-107,
DP-115, DP-124-standing); ALIGN TST-06.

**Change surface.**
- Preflight query: `rg -n 'fn assert_matches_clean_rebuild' -A 8 src/continuous.rs`;
  `rg -n 'from_serving_session' src/`; `rg -n 'comparison-ignore' -l src/ tests/`;
  `rg -n 'InventoryWalker' src/ tests/integration/git_state.rs`
- Known touch: `src/continuous.rs`, `src/lifecycle.rs`,
  `tests/integration/git_state.rs`, `justfile`,
  `docs/reviews/` (superseding status artifact).

**Required changes.**
1. The AC-G-79 comparator performs a true clean rebuild (re-walk inventory,
   re-capture bytes, reconcile from zero) and compares effective state
   (durable base − tombstones + overlay rows) via
   `CanonicalState::from_serving_session` on both sides;
   `comparison-ignore-registry.yaml` consumed. First compare the versioned
   schema fingerprint including field metadata. For governed-key tables, prove
   key uniqueness, counts, and bidirectional full-row equality. Otherwise
   compare canonical rows grouped with exact multiplicities. Distinct
   set-difference and DataFusion `EXCEPT ALL` are not accepted as bag oracles.
2. `git-parity-check` constructs the authoritative `InventoryWalker`
   fallback and compares accelerated vs. authoritative results;
   gix-disabled / cache-disabled / full-rebuild configurations run the
   comparator corpus.
3. Supersede the stale implementation-status artifact (CONF DP-107) with a
   current one; this plan's per-packet proving-commit contract is the
   CONF DP-106 process closure going forward.

**Legacy disposition.** The same-wave re-run comparator retired.

**Acceptance checks.**
- Behavioral: `rebuild-equivalence-check` proves schema and exact bag equality
  for incremental versus clean rebuild over the full scenario corpus,
  including overlay/tombstone cases.
  Executable oracle: `wp72_behavioral_acceptance`
  Governed criterion: `PC-WP72-BEH` — mapped through this packet's Target invariants and Design references.
- Structural: `from_serving_session` has production callers on both
  comparator sides; ignore-registry entries consumed.
  Executable oracle: `wp72_structural_acceptance`
  Governed criterion: `PC-WP72-STR` — mapped through this packet's Target invariants and Design references.
- Negative: duplicate multiplicity swap, null/NaN, schema type/metadata,
  tombstone, overlay-row, and gix-disabled divergences all fail.
  Executable oracle: `wp72_negative_zero_state`
  Governed criterion: `PC-WP72-NEG` — mapped through this packet's Target invariants and Design references.
- Operational: `just wave7-integration-check` green and wired into CI.
  Executable oracle: `wp72_operational_acceptance`
  Governed criterion: `PC-WP72-OPS` — mapped through this packet's Target invariants and Design references.

**Gates.** `just wp72-acceptance-check` (selects all four `wp72_*` oracles
with `--no-tests=fail`), `just rebuild-equivalence-check`, `just git-parity-check`,
`just wave7-integration-check`, `just source-capture-race-check`
(regression guard). **Milestone.** M14. **Replan triggers.** True-rebuild
comparison finds a real convergence defect — that is a product bug with its
own fix obligation, recorded as a discovered obligation, not silently
absorbed. **Rollback.** Comparator-side only.

## 5. Integration milestones

### M09 — Single-authority substrate

Packets WP73, WP54, WP55, WP56, WP69. Evidence: accepted normative clauses;
superseding detector-backed register and owned baseline; foundational oracle
and dependency governance; cross-language registry/identity KATs;
purpose-aware digest census; model-plane checks that can fail; `just
design-principle-traceability-check`, `just alignment-detector-check`,
`just oracle-substance-check`, `just model-repro-check`, and `just governance`
green.

### M10 — Truthful data plane

Packets WP57, WP58, WP59, WP60, WP74. Evidence: provider contract suite green
across direct/IPC adapters; external IPC proof; six-class metadata dictionary;
Id16 application contract; candidate-state FK enforcement;
`just provider-protocol-check`, `just publication-referential-integrity-check`,
`just wave4-integration-check`, `just data-fabric-upgrade-check` green.

### M11 — Compiled query vertical

Packets WP61, WP62, WP75, WP63, WP64. Evidence: all eight forms and arbitrary
composition through the activated daemon; relational/graph boundary proof;
canonical order/checksum/identity harness; state-truth adversarial tests;
error closure; `just semantic-query-conformance-check`,
`just query-determinism-check`, and `just wave5-integration-check` green.

### M12 — Provenance closure

Packets WP65, WP66. Evidence: closure-traversal oracle from a committed
Delta version; persisted artifact bundles including failure paths;
retention fault tests; `just wave6-integration-check` green.

### M13 — Hardened boundary, thin adapter

Packets WP67, WP68. Evidence: forgery/expiry negative suite; adapter
state-fidelity suite against production surface; `just adapter-ci-fast`,
`just adapter-stdio-test` green.

### M14 — Full-alignment certification

Packets WP70, WP71, WP76, WP72; DB10–DB12 exits. Evidence: accountable
owner-accepted immutable golden corpus; Gate B end to end; exact schema/bag
convergence and parity proofs; the superseding register detector suite green
at the certification commit; the full §7 gate matrix green. M14 fails if any
normative clause is merely routed, any oracle is unselected, or any acceptance
artifact is missing.

## 6. Cross-packet decommission batches

### DB10 — Query-plane legacy

Prerequisites: WP62, WP75, WP64, M11. Deletes: the SQL string builder and any
`format!`-constructed query text on the semantic path; `&'static str`
result-state fields; `f(sql, snapshot)` query identity; the order-sensitive
checksum. (The hand-written `FreshnessState`/`NewlineKind` enums are WP56
deletions verified by DB12, not this batch.) Exit invariants (all
mechanized): state-literal rule zero-hit;
`rg -n 'SELECT ' src/semantic_query.rs src/query_service.rs` zero-hit
outside tests; the WP62/WP75/WP64 negative oracles green at the DB commit.

### DB11 — Provider-protocol legacy

Prerequisites: WP59, WP60, M10. Deletes: `ObservationMessage`,
`CanonicalFact`, `encode_selected`; the fixed `(tree, ruff)` ingest
signature; the five bespoke cancellation types; `--extract-json` and its
DTOs; `ProviderJobSpec` in domain signatures. Exit invariants: cancellation
census == 1; `rg 'ObservationMessage|CanonicalFact|extract-json'` zero-hit
in `src/` and `rustc-extractor/src/`; in-process adapters have no IPC
encode/decode loop; external IPC and direct-batch provider suites green.

### DB12 — Registry and model-plane duplicates

Prerequisites: WP56, WP69, M09. Deletes: orphan
`codefabric-cpg-mcp/.../contracts/registries.py`; the twin generated Rust
registry module; the hand-written `NewlineKind`/`FreshnessState`
re-declarations and their crosswalks (deleted in WP56, re-verified at this
batch's exit); unproduced Derived artifacts (17 registry projections, stale
arrow-delta copy); byte-identical traceability twin; dead contract
vocabulary. Exit invariants: unproduced-Derived census zero; single
generated registry module import census; zero hand-written registry-domain
enum declarations outside generated code; `cmp` on
requirements/traceability differs; governed-import oracle green.

## 7. Final gate matrix

All recipes; every proving commit runs the packet-local subset, M14 runs
all:

- `just ci-fast` · `just ci-pr`
- `just governance` (includes `artifacts-check`, `plan-status`,
  `governance-scan`, model design/assurance/zero-state checks)
- `just stable-graph-check` · `just features-each` · `just deps-fast` ·
  `just policy`
- `just root-test` (tests **and** doctests) · `just root-check` ·
  `just root-clippy` · `just root-fmt` · `just typos`
- `just adapter-ci-fast` · `just adapter-stdio-test` ·
  `just adapter-wheel-test`
- `just extractor-ci-fast` · `just sidecar-ci-fast`
- `just model-check` · `just model-repro-check` · `just model-plan-check` ·
  `just model-transaction-check` · `just model-release-check`
- `just wave4-integration-check` · `just wave5-integration-check` ·
  `just wave6-integration-check` · `just wave7-integration-check` ·
  `just gate-b-check` · `just git-parity-check` ·
  `just rebuild-equivalence-check` · `just data-fabric-upgrade-check` ·
  `just vacuum-dry-run-check`
- `just design-principle-traceability-check` ·
  `just alignment-detector-check` · `just audit-baseline-check` ·
  `just oracle-substance-check` · `just plan-dependency-check`
- `just digest-domain-contract-check` · `just provider-protocol-check` ·
  `just publication-referential-integrity-check` ·
  `just id16-extension-contract-check` ·
  `just provider-statistics-contract-check`
- `just semantic-query-conformance-check` · `just query-determinism-check` ·
  `just query-artifact-single-execution-check`
- `just gate-b-candidate-check` · `just gate-b-owner-acceptance-check` ·
  `just wp72-acceptance-check`
- Tier-C, risk-triggered per §8.4 of AGENTS: `just mutants-file <path>` on
  the WP64 checksum, WP71 scenario runner, and WP72 comparator (assertion
  strength for the new oracles); `just fuzz jcs_decode_canonicalize`
  (unchanged surface, regression only); `just miri` only if any packet
  introduces `unsafe` (none is planned — `unsafe_code = "deny"` stands).

**Proposed recipe ownership.** WP73 owns design traceability, detector, and
baseline gates. WP54 owns oracle-substance and dependency gates. WP55 owns
digest domains. WP58 owns Id16/statistics. WP59 owns provider protocol. WP74
owns publication FK integrity. WP75 owns semantic-query conformance. WP64 owns
query determinism. WP65 owns single-execution artifacts. WP71 owns candidate
generation, WP76 owner acceptance/final Gate B, and WP72 its explicit selector.
WP70 owns `wave-gates` and final CI wiring. Each recipe is introduced before a
packet cites it as completion evidence and uses `--no-tests=fail` where it
selects tests.

`just gate-b-owner-accept` is a confirm-gated mutating administrative action,
not a gate dependency. WP76 defines it; only the registered accountable owner
may invoke it after the executor pauses with the immutable review bundle.

## 8. Execution sequence

The normative direct dependency edges are exactly the packets' declared
**Dependencies** lines, restated here as an edge list (parallel branches may
interleave under the subagent-orchestration policy provided every edge is
respected):

```text
WP73 → WP54                          WP62 → WP75
WP54 → WP55, WP70                    WP75 → WP63, WP64
WP55 → WP56                          WP63 → WP65, WP67, WP68, WP71
WP56 → WP57, WP61, WP62, WP67, WP69 WP64 → WP65, WP71
WP57 → WP58, WP59, WP62, WP74        WP65 → WP66, WP67
WP59 → WP60, WP74                    WP66 → WP71, WP72
WP60 → WP63, WP75                    WP67 → WP68
WP74 → WP66
WP61 → WP62, WP67                    WP68 → WP70
WP69 → WP70                          WP70 → WP71
                                     WP71 → WP76
                                     WP76 → WP72
DB12 after M09 · DB11 after M10 · DB10 after M11 · all DB exits gate M14
```

WP73 is complete and trusted at the v3 baseline. Remaining linearized order:
WP54 → WP55 → WP56 → WP69 (M09, DB12) →
WP57 → WP58 → WP59 → WP60 → WP74 (M10, DB11) → WP61 → WP62 → WP75 →
WP63 → WP64 (M11, DB10) → WP65 → WP66 (M12) → WP67 → WP68 (M13) →
WP70 → WP71 → WP76 → WP72 (M14).

F-003's activation contract and WP73 are complete. V3 activation atomically
creates and validates the schema-2 state file before switching
`docs/plans/active-plan.json`; failure leaves the prior pointer unchanged.
Every remaining packet completion requires its four criterion-mapped oracles,
packet gate, proving commit, and current-HEAD rerun.

## 9. Plan risks and replan policy

**Risks.**

1. **Persisted-identity compatibility (WP55/WP57/WP64/WP66).** Semantic
   fingerprint, schema, checksum, and plan/query identity changes can alter
   persisted values. Integrity/security hashes are not silently migrated into
   identity. Mitigation: golden
   equivalence corpora before cutover; any persisted-value change triggers
   the packet's replan clause, not a silent migration.
2. **Wire compatibility (WP56/WP61/WP67).** Registry-emitted proto enums
   and the error envelope touch the wire. Mitigation: preserve current wire
   values; cross-language KATs at every proving commit; versioned migration
   on divergence.
3. **Vacuous-check debt surfacing (WP54/WP69/WP70/WP72).** Foundational
   oracle governance will surface real, previously invisible drift before the
   first proving commit. Policy: triage as
   baseline failures with fingerprints; fix-forward within the owning
   packet or record as discovered obligations — never re-weaken the check.
4. **Extension and statistics scope (WP58).** DataFusion 55's extension
   registry is formatter-only for the selected contract; application
   validation owns Id16 semantics. Requested statistics stay declined unless
   their full optimizer/consumer path is accepted and proved.
5. **Semantic-query scale (WP62/WP75/WP64).** Typed IR, graph execution, and
   identity are separate packets. The old SQL differential is transition
   evidence only; eight-form conformance and DB10 hold deletion until M11.
6. **Human checkpoints (WP76 remaining).** The repository owner accepted the
   exact WP73 normative amendment candidate at `c6db86e`. Golden answers still
   require the separate WP76 owner decision; lack of that accountable
   acceptance is expected blocking state, not permission for agent
   self-approval.

**Replan policy.** Implementation adaptation (mechanism-level choices within
a packet's invariants) is recorded in execution state. Plan revision
(packet boundaries, sequence, proof obligations) produces
`..._implementation_plan_v4_<date>.md`; triggers include: any declared-input
digest drift over `docs/upfront_design/` scope, any pin movement, any
persisted-format migration need, and any packet unable to remain
dependency-closed. Design reopening (architecture, public contract, library
decision, target invariant — including any request to descend the extension
ladder, GI-8) returns to the proposal/design stage before execution
continues. A rejected or unresolved normative amendment, library semantic
mismatch, or owner checkpoint blocks the dependent milestone and M14; no
merely routed principle may be certified as aligned.
