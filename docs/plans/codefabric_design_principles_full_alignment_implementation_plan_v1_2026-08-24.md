---
artifact: implementation-plan
plan_id: codefabric-design-principles-full-alignment
version: v1
date: 2026-08-24
status: draft
design_path: docs/reviews/design_principles_remediation_proposal_2026-08-24_v1.md
design_version: v1
baseline_commit: f2dfcfe25dbfe46f0ca779a2fc4273787e18a445
working_tree_digest: 849b82101ddbbf02e3f162891d7c0d900b35c54340a91199dac08b84b7d4375d
state_path: docs/plans/state/codefabric-design-principles-full-alignment_v1_state.json
cutover: true
---

# CodeFabric design-principles full alignment — implementation plan v1

This plan executes the ten remediation moves R1–R10 of the design-principles
remediation proposal: full resolution of the 124 conformance-register findings
and full alignment with the 25 data-fabric design principles through
best-in-class utilization of DataFusion 55.0.0 and Arrow/Parquet 59.2.0.

Citation tags follow the proposal: `PRIN Pn` (data-fabric principles),
`CONF DP-nnn` (conformance register), `ALIGN` (alignment manual and its
pattern IDs), `DFREF §n` (DataFusion 55 comprehensive reference), `ARROW §n`
(Arrow 59 reference), `HOL Pn` (holistic doctrine), plus the design-corpus
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

1. every registry, digest domain, identity recipe, and enum vocabulary has
   exactly one authority in `contracts/` with generated, digest-linked
   projections per language, and an executable drift oracle (R1);
2. the semantic query plane compiles typed `QuerySpec`s through
   `LogicalPlanBuilder`/`Expr` into `LogicalPlan` — no SQL text, no
   compile-time-constant result states — and is reachable from
   `daemon::serve` (R2);
3. `result_checksum` is an order-independent, contract-defined function of the
   result set and schema, computed over `arrow-row` canonical encodings, and
   reproducibility is a modeled status (R3);
4. every governed execution persists a `QueryPlanArtifact` bundle, carries an
   execution identity allocated before planning, and resolves provenance
   closure from any durable result (R4);
5. Arrow IPC is the only provider fact transport, decoded by a validating
   `StreamDecoder` path into one merged ingest pipeline behind real
   `ProviderAdapter` implementations (R5);
6. serving `TableProvider`s advertise only truthful statistics, pushdown, and
   constraints; the schema IR's declared semantics are enforced or explicitly
   reclassified; `codefabric.id16` is an engine-registered Arrow extension
   type (R6);
7. every fabric error carries a lifecycle phase and a registry-closed public
   code; state-machine guards are evaluated, not merely legal (R7);
8. the daemon boundary is one proto-authoritative contract family behind
   keyed tokens and enforced leases, presented by a strictly pass-through
   adapter (R8);
9. Gate B executes end-to-end against released golden answers; AC-G-79
   convergence and gix parity are proved by real comparators; oracle
   substance is governed (R9); and
10. the model-compiler plane's checks can fail, its derived artifacts are
    produced by something, and its vocabularies have one registration (R10).

### 1.2 Non-goals

- No tenancy model, masking/classification metadata, advisory display
  channel, or user-facing expression surface — the register's divergence
  ledger stays closed (proposal §9).
- No UDF, custom `ExecutionPlan`, `PhysicalExpr`, `LogicalPlan::Extension`,
  or custom planner (`ALIGN EXT-04`–`EXT-10` unselected; `PRIN P14`).
- No Substrait, Flight, or ADBC adoption; process boundaries remain UDS gRPC
  + Arrow IPC.
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

Baseline commit `f2dfcfe` with a dirty working tree. The recorded
`working_tree_digest` is `sha256(git diff HEAD)` at planning time; it covers
the two artifact-type registration edits from the proposal session
(`.claude/skills/_shared/artifact-schemas.md`,
`tooling/ci/artifact_contracts.py`). Five declared inputs are untracked at
baseline: the remediation proposal, the conformance register, the data-fabric
principles, the alignment manual, and the Arrow 59 reference. WP54 commits
all of these; no later packet may claim a proving commit while its declared
inputs remain untracked.

The register's file:line anchors were measured at `d89cc90`; code drift since
(the data-fabric upgrade execution) moved lines but not findings — verified
this session for the load-bearing anchors (`semantic_query.rs:366` SQL
builder, `query_service.rs:64 opaque_bytes`, `serving.rs:205
QueryPlanArtifact`, `provider_runtime.rs:320 ProviderAdapter`,
`gate-b-check` still dependency-only at `justfile:157`). Packets therefore
carry preflight queries, not frozen line numbers.

A planning-session `just ci-fast` baseline was not re-run; the session
context records the cached baseline as green but stale (HEAD moved). WP54
re-derives it and records real failures in execution state
(`validation-policy.md §3`).

No execution state is created by this plan. Execution begins by pointing
`docs/plans/active-plan.json` at this plan and initializing the schema-2
state file — a deliberate start step, per the plan-governance contract.

## 2. Source design and declared inputs

The source design is the remediation proposal v1 (accepted by the user as
the basis for this plan), which is itself grounded in the conformance
register's 124 findings and the alignment manual's pattern catalogue. The
proposal's §4 disposition table is the finding-to-move map; this plan's
packet **Design references** cite moves and findings rather than restating
that table.

| path | sha256 |
|---|---|
| docs/reviews/design_principles_remediation_proposal_2026-08-24_v1.md | 04161fb4a18e81a46b9e1f3d866622f19c5e00f22ca31bca058c848ca14a694d |
| docs/reviews/design_principles_conformance_2026-08-23_v1.md | 9d3ec5bcd8569a8acc8900162f8859546dea4778951f932b751ec99a6c832fe5 |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |

### 2.1 Library decisions

The proposal settles the library approach; this plan records the load-bearing
decisions with their evidence status. **Verified** = confirmed in-tree or in
the pinned reference this session; **probe** = a compile/API probe is a
preflight obligation of the owning packet.

| ID | Decision | Status | Authority |
|---|---|---|---|
| LD-01 | `QuerySpec` compiles through `LogicalPlanBuilder`/`Expr`/`DFSchema`; no SQL text on the semantic path | verified (DF55 pinned; APIs per DFREF §11, §19, §43) | DFREF §43, §46; ALIGN MOD-02/03, LOG-01–07 |
| LD-02 | Provider IPC chunks decode through `arrow::ipc::reader::StreamDecoder` (push-based, partial-buffer tolerant); validation never skipped on provider input; `require_alignment` policy explicit | verified — the facade's default `ipc` feature is enabled and `arrow::ipc::reader::StreamReader` is already imported at `src/core_facts.rs`; reach the decoder through the `arrow` facade, not a new direct `arrow-ipc` dependency | ARROW §10.8–10.9, §10.12 |
| LD-03 | Canonical result identity via `arrow-row` `RowConverter` encodings: encode → sort encoded rows → BLAKE3 over ordered encodings; order-independent by construction | verified (`arrow-row =59.2.0` already a root dependency) | ARROW §1.3–1.4, §7.9; DFREF §21 |
| LD-04 | `codefabric.id16` custom Arrow `ExtensionType` over `FixedSizeBinary(16)`, engine-registered through DataFusion's `ExtensionTypeRegistry` so planning and `cast_to_type` honor it | probe (registry traits present in DF55) — owner WP58 | ARROW §26.3–26.6, §26.9–26.17, §26.22; DFREF S7 |
| LD-05 | Provider truth through `scan_with_args`/`ScanArgs`, per-predicate `TableProviderFilterPushDown`, `Statistics` with `Precision::{Exact,Inexact,Absent}` | verified (DF55 APIs; ALIGN A.1) | DFREF §18, §47, §51; ALIGN CAT-05–07 |
| LD-06 | Artifact bundle captures logical/optimized/physical plans, `EXPLAIN`/`EXPLAIN ANALYZE` output, and operator metrics | verified | DFREF §30, §55; ALIGN OBS-01–04 |
| LD-07 | Provenance at the table-transition boundary via delta-rs commit properties and `history()`; constraint presence verified at open | verified | DELTA43 reference; ALIGN P9 |
| LD-08 | IPC protocol profile: configurable writer compression; sans-IO stream encoding available where the transport owns backpressure; codec recorded in the protocol contract | verified (Arrow 59.1/59.2 ledger) | ARROW migration ledger, §10.6 |
| LD-09 | Plan fingerprints via application-owned canonicalization namespaced by engine version; `datafusion-proto` bytes only inside the pinned compatibility domain | verified (policy) | DFREF §56; ALIGN MOD-06, P18 |
| LD-10 | Boundary shape validation via fallible Arrow construction (`FixedSizeBinaryArray` `TryFrom`, `RecordBatch::try_new`, batch-vs-schema checks) | verified (Arrow 59.0 ledger) | ARROW migration ledger, §6.3; ALIGN SCH-09 |

## 3. Global target invariants

Every packet inherits these; packet-level invariants add to them.

- **GI-1 (PRIN P3/P18).** One authority per concept; every generated
  projection carries a digest link to its authority; no consumer re-encodes a
  registered vocabulary.
- **GI-2 (PRIN P6).** No SQL text is constructed or executed on the semantic
  query path; `LogicalPlan` is the only internal query representation.
- **GI-3 (PRIN P8/P22; HOL P8).** Arrow IPC is the only provider fact
  transport; IPC validation is never skipped for provider or extractor input.
- **GI-4 (PRIN P20).** Every advertised capability, state, statistic, or
  digest is computed from runtime facts or reported absent/unknown — never a
  constant standing in for a measurement.
- **GI-5 (PRIN P16; HOL P23).** Every fabric error carries a lifecycle phase;
  every public error code is a member of `PUBLIC_ERROR_IDS`.
- **GI-6 (PRIN P9/P10).** Every governed execution and publication resolves
  its provenance chain through stored references; missing links are explicit.
- **GI-7 (PRIN P25).** Acceptance is executable: oracles name contracts, no
  alias oracles, no source-text oracles where a structural rule exists.
- **GI-8 (PRIN P14).** Extension-ladder discipline: built-ins and transparent
  composition only; any proposal to descend is a replan trigger.
- **GI-9 (RM §1 inv. 8; SRV §6).** Python remains presentation-only; no
  adapter re-derivation of daemon state.
- **GI-10.** The divergence ledger stays closed; no packet implements a
  principle clause the design corpus refuses.
- **GI-11 (ALIGN A.3).** One Arrow/DataFusion type universe at the pins;
  `just stable-graph-check` green at every proving commit.

## 4. Work packets

Numbering continues the global sequence (prior plans end at WP53/M08/DB09).

### WP54 — Baseline, input canonicalization, and register-hygiene scaffolding

**Outcome.** The plan's inputs are reproducible repository artifacts, the
gate baseline at HEAD is re-derived and recorded, and the registration and
hygiene defects that would contaminate later proofs are closed.

**Dependencies.** None.

**Target invariants.** GI-1, GI-7. Design references: proposal §1.1, R10
(CONF DP-074, DP-094, DP-124); `artifact-schemas.md §7/§8`.

**Change surface.**
- Preflight query: `git status --porcelain`; `just ci-fast` (record failures);
  `diff -rq skills/ .claude/skills/`; `rg -l 'docs/reviews' scripts/ tooling/`
- Known touch (verified this session):
  `.claude/skills/_shared/artifact-schemas.md`,
  `tooling/ci/artifact_contracts.py` (both already edited: the
  `design-principles-remediation-proposal` row exists in both authorities),
  `scripts/seed_zero_state_check.sh`.

**Required changes.**
1. Commit the five untracked declared inputs and the two registration edits.
2. Remove the untracked `skills/` duplicate (or replace with a symlink to
   `.claude/skills`) and add the shape detector to `seed-zero-state-check`
   (CONF DP-094's detector verbatim).
3. Add the artifact-vocabulary comparison oracle: `artifact-schemas.md §7`
   table keys == `REVIEW_REQUIREMENTS` keys (CONF DP-074), wired into
   `artifacts-check`'s pytest.
4. Add the detector-hygiene convention: whole-repo governance detectors carry
   `--glob '!docs/reviews/**'` (CONF DP-124), applied to the scripts the
   preflight query finds.
5. Re-run `just ci-fast`; record baseline failures in execution state at
   execution start.

**Legacy disposition.** `skills/` duplicate deleted; no aliases.

**Acceptance checks.**
- Behavioral: vocabulary-comparison test green; inputs tracked.
  Executable oracle: `wp54_behavioral_acceptance`
- Structural: seed-zero-state includes the skills-shape detector.
  Executable oracle: `wp54_structural_acceptance`
- Negative: `skills/` absent-or-symlink; no governance script matches
  `docs/reviews/**` without the exclusion.
  Executable oracle: `wp54_negative_zero_state`
- Operational: `just governance` green with the new checks.
  Executable oracle: `wp54_operational_acceptance`

**Gates.** Edit-local: `just artifacts-check`. Packet: `just governance`,
`just ci-fast`. **Milestone.** M09. **Replan triggers.** `ci-fast` baseline
reveals failures attributable to the data-fabric upgrade that block later
packets. **Rollback.** Revert the commit; no runtime surface touched.

### WP55 — Fingerprint-domain registry and identity consolidation

**Outcome.** Every digest domain is a registered contract record; digest
construction is confined to `crate::identity`; generated CBEF recipes carry
their declared normalization.

**Dependencies.** WP54.

**Target invariants.** GI-1; `PRIN P3/P18`; `GEN §13` (application-owned
identity). Design references: R1 (CONF DP-005, DP-031, DP-044-digest,
DP-086, DP-119, DP-120); ALIGN MOD-06, SCH-10, OBS-09.

**Change surface.**
- Preflight query: `git grep -n 'b"codefabric' -- src/ rustc-extractor/`;
  `rg -n 'fn digest_bytes' src/ --glob '!src/generated/**'`;
  `rg -n 'blake3::Hasher' src/ --glob '!src/identity.rs'`;
  `rg -n 'normalization' contracts/identity/cbef-v1.yaml src/generated/model_identity_recipes.rs`
- Known touch: `src/identity.rs`, `contracts/identity/` (new registry),
  `src/core_facts.rs` (duplicate scope fingerprints), `src/source_syntax.rs`
  (unguarded twin), the model driver emitting recipes.

**Required changes.**
1. Author the fingerprint-domain registry contract: domain string, separator
   convention, field set/order, normalization; migrate every domain the
   preflight census finds (27+ at register time).
2. `crate::identity` compiles the registry into the only digest
   constructors; migrate call sites; collapse the twin `capability-scope`
   implementations to one guarded constructor.
3. Unify `digest_bytes` to a single definition consumed by all drivers.
4. Emit `normalization: ASCII_LOWER` in generated recipes and honor it in
   the generated-recipe evaluation path (closes the latent CONF DP-005
   before governance pushes production onto that path).
5. Add the governance rule: no `blake3::Hasher` construction outside
   `src/identity.rs` and generated recipe code (tested `rules/` rule).

**Legacy disposition.** Ad-hoc domain literals deleted at their call sites;
the nine `digest_bytes` copies reduced to one.

**Acceptance checks.**
- Behavioral: registry-driven digests reproduce the pre-change values for
  every migrated domain (golden equivalence corpus).
  Executable oracle: `wp55_behavioral_acceptance`
- Structural: domain census oracle — domains in use ⊆ registry.
  Executable oracle: `wp55_structural_acceptance`
- Negative: zero `blake3::Hasher` hits outside the permitted files
  (ast-grep rule + rg, both zero-hit).
  Executable oracle: `wp55_negative_zero_state`
- Operational: `just model-repro-check` green (recipes regenerate
  identically twice).
  Executable oracle: `wp55_operational_acceptance`

**Gates.** `just root-check`, `just root-test`, `just governance-scan`,
`just model-repro-check`. **Milestone.** M09. **Replan triggers.** A domain
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
- Structural: registry↔proto cross-check green; `digest_frames`
  byte-equality green.
  Executable oracle: `wp56_structural_acceptance`
- Negative: zero imports of the deleted module paths; zero hand-written
  registry-domain enums outside generated code (rule + rg zero-hit).
  Executable oracle: `wp56_negative_zero_state`
- Operational: `just adapter-ci-fast` and `just model-repro-check` green.
  Executable oracle: `wp56_operational_acceptance`

**Gates.** `just root-check`, `just root-test`, `just adapter-ci-fast`,
`just extractor-ci-fast`, `just model-check`, `just model-repro-check`.
**Milestone.** M09. **Replan triggers.** Proto enum renumbering would break
the wire — the registry↔proto emission must preserve current wire values or
the boundary needs a versioned migration (plan revision). **Rollback.**
Generated outputs revert with the model; the role-matcher fix is isolated.

### WP57 — Schema contracts enforced: metadata classes, evolution policy, generated encoders

**Outcome.** The schema IR's declared semantics are enforced or explicitly
reclassified with named consumers; evolution policy is a stated, versioned
contract; row encoders are generated from the IR they were hand-checked
against.

**Dependencies.** WP56.

**Target invariants.** GI-1, GI-4; `PRIN P12/P21`; `FAB` App. C inv. 11.
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
1. Metadata dictionary: classify every schema-IR annotation
   (enforced / planner-consumed / contractual / lineage / advisory) with a
   named consumer; oracle asserts each non-advisory consumer exists.
2. Promote `foreign_key` to enforced cross-table validation (consumed by the
   ingest validator; full enforcement activates with WP59's merged
   pipeline); record the SQLite `REFERENCES` decision (emit clauses or drop
   the pragma).
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
  fact-table golden corpus; constraint-drop is detected at open.
  Executable oracle: `wp57_behavioral_acceptance`
- Structural: metadata-dictionary oracle; semantic-type resolution oracle;
  evolution-policy artifact validated.
  Executable oracle: `wp57_structural_acceptance`
- Negative: mutated constraints/schema rejected with the registered code;
  unresolvable `semantic_type` fails the build (expected-failure fixture).
  Executable oracle: `wp57_negative_zero_state`
- Operational: `just model-repro-check` and `just stable-graph-check` green.
  Executable oracle: `wp57_operational_acceptance`

**Gates.** `just root-check`, `just root-test`, `just model-check`,
`just model-repro-check`. **Milestone.** M10. **Replan triggers.** FK
enforcement over existing golden data surfaces real referential violations —
disposition (fix data vs. stage enforcement) is a plan-level decision.
**Rollback.** Encoder generation lands behind equivalence proof; revertible
per change.

### WP58 — Truthful providers and the `codefabric.id16` extension type

**Outcome.** Serving providers advertise computed statistics with honest
precision and per-predicate pushdown; observed evidence replaces echoed
configuration; Id16 identity columns carry an engine-registered extension
type.

**Dependencies.** WP57.

**Target invariants.** GI-4; `PRIN P15/P20/P21`. Design references: R6
(CONF DP-019, DP-063-instance-validation); ALIGN CAT-05–CAT-07, SCH-06,
INT-09; ARROW §26; DFREF §18, §47, §51, S7, S10.

**Change surface.**
- Preflight query: `rg -n 'fn statistics|supports_filters_pushdown|scan_with_args'
  src/fabric/`; `rg -n 'parquet_pruning|repartition_' src/fabric/serving.rs`;
  `rg -n 'ExtensionType|ARROW:extension' src/`; probe LD-04
  (`ExtensionTypeRegistry` in `datafusion_expr::registry`) and LD-05 with a
  compile check.
- Known touch: `src/fabric/overlay.rs`, `src/fabric/serving.rs`,
  `src/schema_registry.rs`, `tooling/model/validate_staged_schemas.py`.

**Required changes.**
1. Overlay/serving providers return `Statistics` with per-column
   `Precision::{Exact,Inexact,Absent}` (exact row counts for materialized
   overlay batches; Delta-stat-supported values otherwise; `Absent`
   elsewhere); adopt `scan_with_args`/`ScanArgs` and declare per-predicate
   pushdown truthfully.
2. `ServingRuntimeEvidence` records observed pruning/repartition facts from
   `EXPLAIN ANALYZE` metrics, not configuration read-back.
3. Define the `codefabric.id16` `ExtensionType` (storage
   `FixedSizeBinary(16)`, versioned metadata), attach it to Id16 fields in
   the generated Arrow schemas, register it in the session's extension-type
   registry, and verify IPC/Parquet round-trip preservation; unknown
   consumers degrade to storage type.
4. Public JSON schemas gain instance validation: golden envelopes validated
   against `planspec.schema.json` and siblings in the staged-schema check.

**Legacy disposition.** The tautological evidence assertions deleted;
`statistics() -> None` replaced.

**Acceptance checks.**
- Behavioral: adversarial pushdown-truth tests — claimed-exact predicates
  falsified with boundary rows; statistics precision matches measured data.
  Executable oracle: `wp58_behavioral_acceptance`
- Structural: extension-type round-trip (IPC + Parquet + unknown-consumer
  fallback); instance validation wired into the staged-schema gate.
  Executable oracle: `wp58_structural_acceptance`
- Negative: a provider claiming exact pushdown it does not enforce fails the
  contract suite (expected-failure fixture).
  Executable oracle: `wp58_negative_zero_state`
- Operational: `just root-test` and `just data-fabric-upgrade-check` green.
  Executable oracle: `wp58_operational_acceptance`

**Gates.** `just root-check`, `just root-test`,
`just data-fabric-upgrade-check`. **Milestone.** M10. **Replan triggers.**
LD-04 probe fails (extension-type registry API absent/incompatible in DF55)
— fall back to metadata-only extension types with WP57's dictionary naming
the consumer, and record the deviation. **Rollback.** Extension-type
attachment is schema-metadata additive; statistics changes revert cleanly.

### WP59 — Arrow IPC as the sole fact protocol; one ingest pipeline

**Outcome.** One validated Arrow IPC channel from providers to a single
merged ingest pipeline covering every fact table; provider-computed facts
and diagnostics survive to their tables.

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
1. Implement the `StreamDecoder`-based decode path for
   `ProviderEvent::ArrowIpcChunk` (validation on, alignment policy explicit,
   `with_skip_validation` prohibited by governance rule); batches validate
   against the generated table schemas (`RecordBatch`-level and
   `TryFrom`-level shape checks, LD-10).
2. Retire the `ObservationMessage`/`CanonicalFact` channel; coverage becomes
   schema-driven (any generated table spec is representable).
3. Merge the projection and observation paths above
   `ValidatedFactBatch::validate`: one cross-table referential validator
   (consuming WP57's FK facts), one row-budget mechanism, one precedence
   table, one conflict/evidence encoder, fingerprint fencing on every path.
4. Carry provider-computed facts as batch columns (`evaluation_ordinal`,
   `source_ordinal`, positions, `depth`, provider-parsed names); attribution
   columns carry the true producer and `derivation_code`; the
   `RuffTokenClass` narrowing becomes a declared registry mapping or is
   dropped for the raw+normalized pair.
5. Derived relations gain evidence rows via the common accumulator;
   `IngestDiagnostic`/`ConflictRecord` are written to the `diagnostic`
   table.
6. Record the IPC protocol profile (codec, compression level, alignment)
   as a versioned contract (LD-08).

**Legacy disposition.** `ObservationMessage`, `CanonicalFact`,
`encode_selected`, and the duplicated above-validator logic deleted — DB11
carries the exit invariants.

**Acceptance checks.**
- Behavioral: IPC round-trip KATs across the provider seam; merged pipeline
  reproduces both former paths' accepted corpora (differential test).
  Executable oracle: `wp59_behavioral_acceptance`
- Structural: schema-driven coverage — every generated fact table has an
  ingest path (census oracle); evidence and diagnostic rows appear for the
  golden corpus.
  Executable oracle: `wp59_structural_acceptance`
- Negative: malformed/truncated/misaligned IPC chunks rejected with
  registered codes; `with_skip_validation` zero-hit rule.
  Executable oracle: `wp59_negative_zero_state`
- Operational: `just wave4-integration-check` green.
  Executable oracle: `wp59_operational_acceptance`

**Gates.** `just root-check`, `just root-test`,
`just wave4-integration-check`, `just governance-scan`. **Milestone.** M10.
**Replan triggers.** The merged pipeline cannot preserve both former paths'
semantics without a behavioral choice the proposal did not settle.
**Rollback.** The IPC decode path lands beside the old channel until the
differential proof is green; deletion is DB11's.

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
   emitting the WP59 IPC protocol; the ingest entry point takes a
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
  adapter (substitution test as oracle); extractor determinism proved on the
  gRPC path.
  Executable oracle: `wp60_behavioral_acceptance`
- Structural: crosswalk registry is the only field-role source; provider
  registration is the only provider-set authority.
  Executable oracle: `wp60_structural_acceptance`
- Negative: cancellation-type census == 1 (rule); `--extract-json` zero-hit;
  `ProviderJobSpec` absent from domain signatures.
  Executable oracle: `wp60_negative_zero_state`
- Operational: cancellation propagation test RPC→stream-drop under load;
  `just extractor-ci-fast` green.
  Executable oracle: `wp60_operational_acceptance`

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
- Structural: error-code closure oracle green repo-wide.
  Executable oracle: `wp61_structural_acceptance`
- Negative: guard-falsification — a Rust-bearing wave parks in the
  explicit non-terminal state (no silent terminal declaration).
  Executable oracle: `wp61_negative_zero_state`
- Operational: forced-failure shutdown reports the true completed-step set.
  Executable oracle: `wp61_operational_acceptance`

**Gates.** `just root-check`, `just root-test`, `just governance-scan`.
**Milestone.** M11. **Replan triggers.** Envelope adoption forces a breaking
change on a wire error contract — boundary versioning decision (plan
revision). **Rollback.** Envelope adoption is per-subsystem; revert per
module.

### WP62 — The `QuerySpec` → `LogicalPlan` compiler

**Outcome.** The semantic query plane compiles typed requests through one
binder into `LogicalPlan`; advertised filters and projections are real;
every result state is computed from execution facts.

**Dependencies.** WP56, WP57, WP61.

**Target invariants.** GI-2, GI-4; `PRIN P1/P2/P6/P20`; `SRV §6` inv. 5/6.
Design references: R2 (CONF DP-077, DP-080, DP-095, DP-098, DP-109, DP-110,
DP-111, DP-112, DP-123); ALIGN MOD-02/03/05/07, LOG-01–LOG-07, EXP-01/02;
DFREF §11, §19, §43, §46; LD-01.

**Change surface.**
- Preflight query: `rg -n 'SELECT \* FROM|format!' src/semantic_query.rs`;
  `rg -n '&.static str' src/semantic_query.rs`; `rg -n 'FreshnessBarrier::default|freshness_policy'
  src/query_service.rs src/lifecycle.rs`; `rg -n 'profile_digest|EffectiveLimitsProfile'
  src/query_service.rs`
- Known touch: `src/semantic_query.rs`, `src/query_service.rs`,
  `src/lifecycle.rs`, `src/fabric/serving.rs`.

**Required changes.**
1. Implement the `QuerySpec` binder: request forms → `LogicalPlanBuilder`
   over the serving session's schemas; predicates/projections compiled from
   the typed request DTOs (`QueryInput`/`QueryPredicate`/
   `response_projection` become real or leave the schema); limits as logical
   fetch; the SQL string builder retired.
2. Policy validation as a logical-plan pass between binding and execution
   (evaluative-request refusal, table allowlist) — the refusal doctrine gains
   a structural enforcement point.
3. Result states become generated registry enums computed from execution:
   `limit_state` from fetch-vs-produced, `freshness_state` from the live
   barrier on success and failure paths, execution/completeness from
   runtime outcome; `failed_query_count`/`errors` real.
4. `EffectiveLimitsProfile.profile_digest` hashes the limit values (WP55
   constructors); `FreshnessState::Unavailable` gains its production writer
   via the continuous engine or is withdrawn from the advertised set.
5. Results are Arrow-native projections with typed fields; public IDs mint
   through `identity::encode_public_id`.

**Legacy disposition.** SQL text, `&'static str` states, literal-prefixed
IDs deleted — DB10 exit invariants.

**Acceptance checks.**
- Behavioral: spec-compiled plans reproduce the SQL path's results over the
  serving corpus (transition differential), and the request route is cut
  over to the binder (final deletion of the SQL builder completes in DB10);
  filter/projection requests return filtered/projected results.
  Executable oracle: `wp62_behavioral_acceptance`
- Structural: state-literal ban rule green (no registered-domain string
  literals outside generated code).
  Executable oracle: `wp62_structural_acceptance`
- Negative: forced stale/limited/failed executions report the true enums —
  a `best_available_snapshot` query over a stale workspace cannot report
  `CURRENT`.
  Executable oracle: `wp62_negative_zero_state`
- Operational: plan-validation rejections carry phase + registered code.
  Executable oracle: `wp62_operational_acceptance`

**Gates.** `just root-check`, `just root-test`, `just governance-scan`.
**Milestone.** M11. **Replan triggers.** A `QRY` request form is not
expressible in built-in logical nodes (would require `LOG-08`) — design
reopening per GI-8. **Rollback.** The binder lands behind the differential
proof; cutover is the last change.

### WP63 — Daemon activation of the query vertical

**Outcome.** The query service and continuous engine are reachable from
`daemon::serve`: the socket is declared, the vertical is constructed on the
production path, and one end-to-end KAT proves it.

**Dependencies.** WP60, WP62.

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
3. Minimal activation scope: bind, handshake, one semantic query end-to-end
   against a real snapshot, streamed result, status. Full W17 RPC scope
   remains W17's; this packet makes the island reachable so its proofs
   count.

**Legacy disposition.** None (additive reachability).

**Acceptance checks.**
- Behavioral: end-to-end KAT — daemon boots, adapter-side stub connects over
  UDS, semantic query returns the golden answer with computed states.
  Executable oracle: `wp63_behavioral_acceptance`
- Structural: island detector inverted — the query modules have inbound
  edges from `daemon::serve` (non-empty reference census).
  Executable oracle: `wp63_structural_acceptance`
- Negative: unauthorized peer UID rejected on the query socket.
  Executable oracle: `wp63_negative_zero_state`
- Operational: daemon shutdown under an in-flight query cancels cleanly
  (WP60 cancellation, WP61 honest shutdown).
  Executable oracle: `wp63_operational_acceptance`

**Gates.** `just root-test`, `just wave5-integration-check`.
**Milestone.** M11. **Replan triggers.** Activation surfaces a W17-owned
contract decision (e.g., socket lifecycle policy) the specs do not settle —
route per `RM §28`. **Rollback.** Socket binding is config-gated.

### WP64 — Deterministic result identity and modeled reproducibility

**Outcome.** `result_checksum` is an order-independent, contract-defined
digest over `arrow-row` canonical encodings; query identity separates plan,
snapshot, and requester; reproducibility is a modeled status.

**Dependencies.** WP62.

**Target invariants.** GI-4; `PRIN P18/P19`. Design references: R3
(CONF DP-012); ALIGN MOD-06, OBS-10, RUN-10; LD-03, LD-09; DFREF §21, §56.

**Change surface.**
- Preflight query: `rg -n 'result_checksum|hasher.update' src/fabric/serving.rs`;
  `rg -n 'query_id' src/fabric/serving.rs src/query_service.rs`;
  `rg -n 'arrow_row|RowConverter' src/`
- Known touch: `src/fabric/serving.rs`, `src/query_service.rs`.

**Required changes.**
1. Replace the arrival-order sequential hash: convert result batches through
   `RowConverter` to canonical row encodings, sort the encodings, digest the
   ordered sequence (BLAKE3, WP55 domain). The checksum contract states it
   is a function of (result row set, output schema, checksum-domain
   version) — partitioning-independent by construction.
2. Compute the plan fingerprint from the compiled `LogicalPlan` under an
   engine-version-namespaced canonicalization (LD-09); query identity
   becomes (plan fingerprint, snapshot manifest digest, config fingerprint),
   with requester identity carried separately (WP65).
3. Add the `Reproducibility` record (deterministic, inputs_pinned,
   volatile_functions, environment_recorded) to `QueryPlanArtifact`,
   derived from the plan and the environment snapshot.

**Legacy disposition.** The order-sensitive hasher and `f(sql, snapshot)`
query id deleted — DB10.

**Acceptance checks.**
- Behavioral: determinism harness — same spec, same frozen snapshot, varied
  `target_partitions` (1, 2, 8) and batch sizes ⇒ identical checksum; the
  CONF DP-012 detector inverted.
  Executable oracle: `wp64_behavioral_acceptance`
- Structural: checksum and fingerprint domains registered (WP55 census).
  Executable oracle: `wp64_structural_acceptance`
- Negative: a one-row difference changes the checksum; row-order permutation
  does not (property test).
  Executable oracle: `wp64_negative_zero_state`
- Operational: replay under pinned environment reproduces the artifact's
  recorded checksum.
  Executable oracle: `wp64_operational_acceptance`

**Gates.** `just root-test`. **Milestone.** M11. **Replan triggers.**
RowConverter cannot represent an output type in use (unsupported type class)
— fall back to the canonical-sort contract for that form and record the
deviation. **Rollback.** Old checksum retained in the artifact during the
packet's differential window only.

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
2. Persist `QueryPlanArtifact` (including `EXPLAIN`/`EXPLAIN ANALYZE`
   output, operator metrics, and the WP64 `Reproducibility` record) to
   `result_artifact_lease`, keyed by execution identity, with a retention
   policy.
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
  and metrics; two agents issuing identical SQL-equivalent specs are
  distinguishable by request identity.
  Executable oracle: `wp65_behavioral_acceptance`
- Structural: artifact schema versioned; artifact rows carry every declared
  pin field non-null for golden queries.
  Executable oracle: `wp65_structural_acceptance`
- Negative: a failed query still persists a partial bundle through the
  failing phase (WP61 phases).
  Executable oracle: `wp65_negative_zero_state`
- Operational: artifact persistence and retention follow the declared lease
  policy (expiry *enforcement* on reads is WP67's oracle, not this one).
  Executable oracle: `wp65_operational_acceptance`

**Gates.** `just root-test`, `just wave5-integration-check`.
**Milestone.** M12. **Replan triggers.** Artifact volume forces a retention
design the operational store cannot express — plan revision. **Rollback.**
Artifact persistence is additive; write path feature-gated until proof.

### WP66 — Provenance closure of durable state

**Outcome.** From any committed Delta version, the chain commit → execution
→ plans → specs → inputs → schema fingerprints → source snapshots resolves;
retention protects the explainers; an operator surface answers "why does
this row exist".

**Dependencies.** WP59, WP65.

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
   engine writes `update_wave`/`update_wave_item`; `wave_id` Id16-validated.
2. `table_mutation_operation` gains typed scope columns and a workspace
   scope; the three no-preimage digest keys gain stored preimages or are
   re-documented as integrity fields (decision recorded in the WP57
   metadata dictionary).
3. `source_blob_digests` joins `ServingSnapshotManifestBody`
   (content-addressed snapshot→bytes link); the `file_id` location-identity
   contract is declared beside it, and the `GEN §13` ambiguity is routed to
   its owner per `RM §28`.
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
- Structural: wave/provider/fact join proved on the golden corpus (the
  restored `owner_id` join returns rows).
  Executable oracle: `wp66_structural_acceptance`
- Negative: GC cannot delete a retained publication's explainers
  (fault-injected retention test).
  Executable oracle: `wp66_negative_zero_state`
- Operational: `explain_version` answers for a golden publication within
  bounded time.
  Executable oracle: `wp66_operational_acceptance`

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

**Dependencies.** WP56, WP61.

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
- Structural: admin protocol messages validate against the new schema
  artifact; Rust/Python index decoders agree on a differential corpus.
  Executable oracle: `wp67_structural_acceptance`
- Negative: expired lease read rejected; cancel with a resume token
  rejected.
  Executable oracle: `wp67_negative_zero_state`
- Operational: `just wave5-integration-check` green with hardened paths.
  Executable oracle: `wp67_operational_acceptance`

**Gates.** `just root-test`, `just model-check`, `just adapter-ci-fast`.
**Milestone.** M13. **Replan triggers.** Token-scheme change invalidates a
persisted lease format — bounded migration decision. **Rollback.**
Keyed-token cutover within one release boundary; no dual acceptance beyond
the packet.

### WP68 — The strictly-presentational adapter

**Outcome.** The adapter passes daemon state through unmodified, holds no
authority, tests its production surface, and its contract schemas validate
the boundary they describe.

**Dependencies.** WP67.

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
- Structural: production tool manifest matches its accepted fingerprint
  baseline; boundary payloads validate against the generated schemas.
  Executable oracle: `wp68_structural_acceptance`
- Negative: adapter cannot answer a lease query from local state when the
  daemon says revoked (differential test).
  Executable oracle: `wp68_negative_zero_state`
- Operational: `just adapter-ci-fast` and `just adapter-stdio-test` green.
  Executable oracle: `wp68_operational_acceptance`

**Gates.** `just adapter-ci-fast`, `just adapter-stdio-test`,
`just adapter-wheel-test`. **Milestone.** M13. **Replan triggers.** A
pass-through value has no MCP representation — that is a `SRV` contract gap,
route per `RM §28`. **Rollback.** Adapter changes are presentation-local.

### WP69 — Model-compiler truth and derived-artifact repair

**Outcome.** The model plane's checks can fail, its Derived claims drive
production and deletion, its traceability carries normative content, and
its vocabularies have one registration.

**Dependencies.** WP54.

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
- Structural: census oracle — suite manifest count == committed tree ==
  validation count; requirements corpus non-templated (distinct normative
  texts > 1).
  Executable oracle: `wp69_structural_acceptance`
- Negative: unproduced-Derived census zero; JCS-bypass zero-hit in drivers.
  Executable oracle: `wp69_negative_zero_state`
- Operational: `just model-release-check` green.
  Executable oracle: `wp69_operational_acceptance`

**Gates.** `just model-check`, `just model-plan-check`,
`just model-transaction-check`, `just model-repro-check`,
`just model-release-check`. **Milestone.** M09. **Replan triggers.** Real
drift surfaces once the shadow is removed (the check was vacuous) — triage
as baseline failures, not new defects. **Rollback.** Model-plane only; no
runtime surface.

### WP70 — Oracle substance, rule restoration, and consumed fixtures

**Outcome.** The proving layer catches what the register caught: no alias
oracles, contract-named tests, restored rule coverage, wired gates, and
consumed fixtures.

**Dependencies.** WP54, WP68 (the Python-rule surface: WP68 edits
`no-framework-internal-contract-imports.yml`; this packet's new Python rules
land after it on the same tree).

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
1. Alias-oracle detector (no `#[test]` whose body is a single call to
   another test) as a governance gate; replace the five aliases with real
   acceptance tests meeting their packets' sentences.
2. Oracle-substance validation: the plan validator additionally requires
   per-oracle acceptance-criterion references; new/touched oracles carry
   `AC-G-NN` references — this plan's own wp54–wp72 oracles are the first
   population.
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
- Structural: alias-detector green repo-wide; AC-G reference census > 0 for
  all new oracles; rule/rule-test 1:1 held with snapshots on.
  Executable oracle: `wp70_structural_acceptance`
- Negative: an intentionally-added alias test fails the gate
  (expected-failure fixture); a gate selector matching zero tests fails.
  Executable oracle: `wp70_negative_zero_state`
- Operational: CI configuration mirrors justfile recipes for every new gate.
  Executable oracle: `wp70_operational_acceptance`

**Gates.** `just governance`, `just governance-scan`, `just artifacts-check`,
`just ci-fast`. **Milestone.** M14. **Replan triggers.** Re-narrowed rules
surface real boundary violations in the new modules — those are WP-scoped
fixes, recorded as discovered obligations. **Rollback.** Governance-layer
only.

### WP71 — The golden corpus executes: scenarios and Gate B

**Outcome.** Golden answers are released outputs; the sixteen-plus edit
scenarios execute through the real engine; Gate B runs its eleven items end
to end.

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
2. Populate `expected/` with real released outputs (IDs, rows, response
   bytes, checksums) produced by the activated vertical and accepted by the
   owner; acceptance digest computed from reviewed answers, not
   self-referential; manifest digest fields verified; `corpus_status` gates
   acceptance.
3. `gate-b-check` gains a body: the eleven `SUITE` items run end-to-end and
   compare against released answers.
4. Registry-derived expectations import generated constants and use
   equality, not subset.

**Legacy disposition.** Descriptor-only `expected/` files replaced by
released answers; the self-hash-only tests retired.

**Acceptance checks.**
- Behavioral: all scenarios execute to their expected terminals; Gate B's
  eleven items pass against released answers.
  Executable oracle: `wp71_behavioral_acceptance`
- Structural: corpus digest chain verified end-to-end; zero `is_subset`
  comparisons against registry authorities.
  Executable oracle: `wp71_structural_acceptance`
- Negative: perturbing one released answer fails Gate B (tamper fixture);
  an unexecuted scenario cannot pass by existing as a directory.
  Executable oracle: `wp71_negative_zero_state`
- Operational: `just gate-b-check` green and wired into `ci-pr`.
  Executable oracle: `wp71_operational_acceptance`

**Gates.** `just gate-b-check`, `just wave5-integration-check`,
`just wave6-integration-check`. **Milestone.** M14. **Replan triggers.**
An expected answer cannot be owner-accepted (real output disagrees with
spec intent) — that is a spec-feedback event per `RM §28`, not a fixture
edit. **Rollback.** Corpus changes are fixture-tree local.

### WP72 — Convergence, parity, and process closure

**Outcome.** AC-G-79 convergence is proved by a true clean rebuild compared
over effective state; gix parity and disabled-configuration equivalence are
implemented; the process findings are closed.

**Dependencies.** WP66, WP71.

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
   `comparison-ignore-registry.yaml` consumed. DataFusion set-difference
   queries over the serving session are the comparator mechanism.
2. `git-parity-check` constructs the authoritative `InventoryWalker`
   fallback and compares accelerated vs. authoritative results;
   gix-disabled / cache-disabled / full-rebuild configurations run the
   comparator corpus.
3. Supersede the stale implementation-status artifact (CONF DP-107) with a
   current one; this plan's per-packet proving-commit contract is the
   CONF DP-106 process closure going forward.

**Legacy disposition.** The same-wave re-run comparator retired.

**Acceptance checks.**
- Behavioral: `rebuild-equivalence-check` proves incremental == clean
  rebuild over the full scenario corpus, including overlay-present cases.
  Executable oracle: `wp72_behavioral_acceptance`
- Structural: `from_serving_session` has production callers on both
  comparator sides; ignore-registry entries consumed.
  Executable oracle: `wp72_structural_acceptance`
- Negative: an injected divergence (mutated overlay row) fails the
  comparator (fault fixture); a gix-disabled run differing from accelerated
  fails parity.
  Executable oracle: `wp72_negative_zero_state`
- Operational: `just wave7-integration-check` green and wired into CI.
  Executable oracle: `wp72_operational_acceptance`

**Gates.** `just rebuild-equivalence-check`, `just git-parity-check`,
`just wave7-integration-check`, `just source-capture-race-check`
(regression guard). **Milestone.** M14. **Replan triggers.** True-rebuild
comparison finds a real convergence defect — that is a product bug with its
own fix obligation, recorded as a discovered obligation, not silently
absorbed. **Rollback.** Comparator-side only.

## 5. Integration milestones

### M09 — Single-authority substrate

Packets WP54, WP55, WP56, WP69. Evidence: cross-language registry/identity
KATs; digest-domain census; model-plane checks that can fail; `just
model-repro-check`, `just governance`, `just adapter-ci-fast` green. The
CONF DP-001/002/003 drift class is structurally closed.

### M10 — Truthful data plane

Packets WP57, WP58, WP59, WP60. Evidence: provider contract suite green
across all adapters; IPC differential proof; metadata dictionary complete;
`just wave4-integration-check`, `just data-fabric-upgrade-check` green.

### M11 — Compiled query vertical

Packets WP61, WP62, WP63, WP64. Evidence: end-to-end KAT through the
activated daemon; determinism harness; state-truth adversarial tests;
error-closure oracle; `just wave5-integration-check` green.

### M12 — Provenance closure

Packets WP65, WP66. Evidence: closure-traversal oracle from a committed
Delta version; persisted artifact bundles including failure paths;
retention fault tests; `just wave6-integration-check` green.

### M13 — Hardened boundary, thin adapter

Packets WP67, WP68. Evidence: forgery/expiry negative suite; adapter
state-fidelity suite against production surface; `just adapter-ci-fast`,
`just adapter-stdio-test` green.

### M14 — Full-alignment certification

Packets WP70, WP71, WP72; DB10–DB12 exits. Evidence: Gate B end-to-end;
convergence and parity proofs; the register's detector suite re-run green
at the certification commit; the full §7 gate matrix green. This milestone
is the proposal's §5 end-state matrix made executable.

## 6. Cross-packet decommission batches

### DB10 — Query-plane legacy

Prerequisites: WP62, WP64, M11. Deletes: the SQL string builder and any
`format!`-constructed query text on the semantic path; `&'static str`
result-state fields; `f(sql, snapshot)` query identity; the order-sensitive
checksum. (The hand-written `FreshnessState`/`NewlineKind` enums are WP56
deletions verified by DB12, not this batch.) Exit invariants (all
mechanized): state-literal rule zero-hit;
`rg -n 'SELECT ' src/semantic_query.rs src/query_service.rs` zero-hit
outside tests; the WP62/WP64 negative oracles green at the DB commit.

### DB11 — Provider-protocol legacy

Prerequisites: WP59, WP60, M10. Deletes: `ObservationMessage`,
`CanonicalFact`, `encode_selected`; the fixed `(tree, ruff)` ingest
signature; the five bespoke cancellation types; `--extract-json` and its
DTOs; `ProviderJobSpec` in domain signatures. Exit invariants: cancellation
census == 1; `rg 'ObservationMessage|CanonicalFact|extract-json'` zero-hit
in `src/` and `rustc-extractor/src/`; provider contract suite green.

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
- Tier-C, risk-triggered per §8.4 of AGENTS: `just mutants-file <path>` on
  the WP64 checksum, WP71 scenario runner, and WP72 comparator (assertion
  strength for the new oracles); `just fuzz jcs_decode_canonicalize`
  (unchanged surface, regression only); `just miri` only if any packet
  introduces `unsafe` (none is planned — `unsafe_code = "deny"` stands).

**Proposed new recipes** (added by their owning packets rather than raw
flags): `alignment-detector-check` — owner **WP70** — aggregating the
proposal §8 standing oracles into one gate (including the two detectors
WP54 lands earlier inside `governance`); a `wave-gates` aggregate wiring
wave4–7 + gate-b into `ci-pr` (owner WP70); and the WP71 `gate-b-check`
body replacing the dependency-only recipe.

## 8. Execution sequence

The normative direct dependency edges are exactly the packets' declared
**Dependencies** lines, restated here as an edge list (parallel branches may
interleave under the subagent-orchestration policy provided every edge is
respected):

```text
WP54 → WP55, WP69, WP70              WP62 → WP63, WP64
WP55 → WP56                          WP63 → WP65, WP71
WP56 → WP57, WP61, WP62, WP67        WP64 → WP65, WP71
WP57 → WP58, WP59, WP62              WP65 → WP66
WP59 → WP60, WP66                    WP66 → WP71, WP72
WP60 → WP63                          WP67 → WP68
WP61 → WP62, WP67                    WP68 → WP70
                                     WP70 → WP71
                                     WP71 → WP72
DB12 after M09 · DB11 after M10 · DB10 after M11 · all DB exits gate M14
```

Linearized default order: WP54 → WP55 → WP56 → WP69 (M09, DB12) → WP57 →
WP58 → WP59 → WP60 (M10, DB11) → WP61 → WP62 → WP63 → WP64 (M11, DB10) →
WP65 → WP66 (M12) → WP67 → WP68 (M13) → WP70 → WP71 → WP72 (M14).

Execution starts by pointing `docs/plans/active-plan.json` at this plan and
initializing the schema-2 state file; every packet completion requires a
per-packet proving commit (the CONF DP-106 closure is this contract).

## 9. Plan risks and replan policy

**Risks.**

1. **Persisted-identity compatibility (WP55/WP57/WP66).** Digest-domain and
   schema changes can alter persisted values. Mitigation: golden
   equivalence corpora before cutover; any persisted-value change triggers
   the packet's replan clause, not a silent migration.
2. **Wire compatibility (WP56/WP61/WP67).** Registry-emitted proto enums
   and the error envelope touch the wire. Mitigation: preserve current wire
   values; cross-language KATs at every proving commit; versioned migration
   on divergence.
3. **Vacuous-check debt surfacing (WP69/WP70/WP72).** Making checks able to
   fail will surface real, previously-invisible drift. Policy: triage as
   baseline failures with fingerprints; fix-forward within the owning
   packet or record as discovered obligations — never re-weaken the check.
4. **LD probe (WP58).** The extension-type registry (LD-04) carries the one
   remaining probe obligation, with a recorded metadata-only fallback.
   LD-02 was probe-resolved at planning time: the facade `ipc` feature is
   live at HEAD.
5. **Scale of WP62.** The binder replaces the query plane's core; the
   transition differential (old SQL path vs. compiled path) is mandatory
   before deletion, and DB10 holds the deletion until M11.

**Replan policy.** Implementation adaptation (mechanism-level choices within
a packet's invariants) is recorded in execution state. Plan revision
(packet boundaries, sequence, proof obligations) produces
`..._implementation_plan_v2_<date>.md`; triggers include: any declared-input
digest drift over `docs/upfront_design/` scope, any pin movement, any
persisted-format migration need, and any packet unable to remain
dependency-closed. Design reopening (architecture, public contract, library
decision, target invariant — including any request to descend the extension
ladder, GI-8) returns to the proposal/design stage before execution
continues. Unowned-principle ambiguities route to their owning
specifications per `RM §28` and the proposal §6; a routed question blocks
only the packet that raised it.
