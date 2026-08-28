---
artifact: implementation-plan
plan_id: codefabric-ontology-compiled-data-fabric
version: v2
date: 2026-08-27
status: draft
design_path: docs/designs/codefabric_ontology_compiled_data_fabric_design_v3_2026-08-27.md
design_version: v3
baseline_commit: eb7a738fa55037b19706fd842737cecad65ffe16
working_tree_digest: 174abedf68765285989783ba89d8d7de09657bc60cc8390730d4f802c6a16395
state_path: docs/plans/state/codefabric-ontology-compiled-data-fabric_v2_state.json
cutover: true
---

# CodeFabric ontology-compiled data fabric — implementation plan v2

Integrates all findings from the independent v1 plan audit and implements design v3:
the fabric becomes a fully ontology-compiled, extension-typed, recursively
self-describing relational universe — a normalized twenty-relation `cpg_ontology`
plane, one typed `CompiledOntology` and governed compilation pass, per-domain logical
ID types with universal DataFusion analyzer enforcement, one logical type system across
all served surfaces, logically classified/physically probe-selected structure, truthful
planning facts, and one atomic Stage 2b activation transaction.
The plan also carries the five `docs/upfront_design` amendments the design attaches to
its schema releases (design §5.4) and the reconciliation of the paused waves 8–12
program (design §5.2, owner decision A-3: this plan lands **before** the remaining
wave 9–12 scope executes).

## 1. Outcome and non-goals

### 1.1 Outcome

At plan completion:

1. `cpg_ontology` serves twenty normalized Delta-backed, bundle-digest-carrying
   vocabulary and contract relations: nine typed dimensions plus `ontology_term`,
   `ontology_edge`, `registry_authority`, `semantic_type_binding`, table/column/result
   contracts, identity recipes, phrase bindings, and rule contracts. Every governed code,
   membership, schema, recipe, and executable relational rule resolves in-catalog (TI-1,
   TI-8).
2. Every ID column carries its domain's extension type (`codefabric.<domain>_id`,
   FSB(16) over the Binary Delta seam; `hash32` → FSB(32)); all domains are
   registered in the serving session's extension-type registry; one idempotent DataFusion
   analyzer rule rejects cross-domain comparisons, joins, `IN` lists, casts, and set
   operations from every authorized plan ingress; `codefabric.id16` is retired (TI-2).
3. `cpg_base`, `cpg_control`, `cpg_serving`, and query-form result schemas all lower
   from one generated logical-type vocabulary through one lowering path; result
   batches carry extension types and re-annotated metadata; the packed-`Binary` ID
   sequences in path/pattern results are typed lists (TI-3, TI-5).
4. Structure classification per the `REP §17` criterion is contract data independent of
   physical representation; PR-3 selects only nested-struct versus flat-constraint
   lowering for logically cohesive source spans (TI-4).
5. Statistics compose per overlay mutation class (never unknown when manifest facts are
   available, never falsely exact); statistics-request handling remains explicitly
   declined; validated PK constraints are classified as planner-consumed/application-
   validated/not DataFusion-enforced; pushdown truth is adversarially tested (TI-6).
6. One registered `SchemaContractCompilation` pass emits one typed `CompiledOntology`;
   every generator and relational semantic operation consumes it; zero parallel schema,
   row-shape, result, identity, phrase/rule, or code-literal authority survives (TI-7,
   TI-9; design D-09).
7. The complete Stage-2b candidate passes dynamic catalog-only self-description,
   relational closure, fault injection, and dimension-version stability before one owner
   acceptance and one manifest-pointer advance; ordinary fact publications reuse
   unchanged ontology table versions. The suite amendments (FAB §6.3, §7, §8,
   §9/§65.4, §§78–82/93 + AC-G-20) are
   landed with their enabling packets; the paused waves 8–12 plan's remaining packets
   are audited against the new shape (TI-8; A-3 mechanism).

### 1.2 Non-goals

- No graph-projection runtime, traversal operators, or UDTFs (design LD-05; W13+).
- No query-language expansion (W15/16) and no new QRY request forms.
- No dependency-baseline movement (design L-1) and no new Cargo roots.
- No Delta kernel features (column mapping, type widening, deletion vectors stay
  off; rollback window preserved).
- No Python adapter changes beyond regenerated wire artifacts.
- No absolute performance SLOs (Gate F, W19); performance evidence is differential
  against the WP01 anchor only.

### 1.3 Baseline, program interaction, and activation constraint

Baseline `eb7a738fa55037b19706fd842737cecad65ffe16` records WP08 complete in the
executing waves plan. The contemporaneous working tree contains the user-authored v1/v2
design and audit materials identified by `working_tree_digest`; they are declared inputs,
not implementation progress. No Wave 9 preflight is required or authorized by this plan
integration. Execution preflight occurs only after v2 is accepted and activated.

Activation constraint (owner decision A-3): activating this plan pauses the
executing waves 8–12 plan. Activation choreography, in order: (1) write an
interruption record into
`docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json` via its
schema-v2 `plan_deviations` mechanism — completed-packet history stays
authoritative, no field is rewritten; (2) move the active-plan pointer through the
existing confirm-gated activation transaction; (3) WP16 audits the remaining wave
9–12 packets against the post-amendment shape before M03 closes. Resuming waves
8–12 afterward requires its own status reconciliation (`impl-status`) and is out of
scope here.

## 2. Source design and declared inputs

| Path | sha256 |
|---|---|
| docs/designs/codefabric_ontology_compiled_data_fabric_design_v3_2026-08-27.md | 20beab86f78d5492646d9b68d486eed72e000874040375e07b4dcea137d23851 |
| docs/reviews/plan_audit_codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27_v1.md | 4c853904a3ff5ee96dbfe5b844ed785210c568f1972fd88d7348f61db432d23e |
| docs/reviews/representative_datafusion_arrow_relational_usage.md | ab9d8c58fbb48ab8e6d80b5be631c81ae9a9d9a3ae94481cdce1cbfd964bdb97 |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| contracts/schema/schema-contract-ir.json | 041c6a5efd16b0fe9ec3fef84cfb90ecfcc142b89b3a096012cfe01cfd48ee58 |
| contracts/registry/enum-registry.yaml | 63ae64e7fdf64a76a8421593bff94865116116c5af05681a133b874ae06b961e |
| contracts/registry/ontology-entity-registry.yaml | be8e64c8c3b9fa7d035ce4394c2c1945dc8120b271f36d9bc39184177b6f1687 |
| contracts/registry/ontology-relation-registry.yaml | a7aaf16eab5e9789a432eb07cb8759a4f3854e40bb5efba7f39447ac8e7f387f |
| contracts/registry/ontology-property-registry.yaml | d4cbb09acfe59bb86ec689ea6954c89052448a14f0a6d05789ebb06741c58061 |
| contracts/registry/ontology-fact-registry.yaml | fc4cdd4976a1a90275c410fd1950347a8c13c86f82f9b2b81b7b45d77ba07eaf |
| contracts/registry/phrase-registry.yaml | 8f317e433b4badd53dc9b58c3f5f1985949bef744e27cd8bd761bede6a797ce4 |
| contracts/query/query-form-contract.json | 581c91886d276b88eedccac9af76c3c689d45d7d0c4f4d45872ce4019a99ed05 |
| docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md | cd92a3735a91e04aa71c911ab4b9d11eb8ec143a0b28a806c37dbab925b5f71e |

### 2.1 Library decisions

All library use in this plan traces to design v3 §3.12; decisions are restated here
only as execution bindings.

### LD-01 — DataFusion 55 extension-type registry

**Decision:** adopt
**Version basis:** DataFusion `=55.0.0` (`ExtensionTypeRegistry`,
`MemoryExtensionTypeRegistry`, `DFExtensionType`,
`SessionStateBuilder::with_extension_type_registry`; verified in the pinned
reference §4/S7.20–21).
**Displaces:** nothing; generated DataFusion `DFExtensionType` factories provide
programmatic resolution and formatting only. Arrow field validation, storage-seam casts,
and semantic analyzer enforcement are separate responsibilities.
**Risk:** treating registration as policy. Mitigated by exact registry/compiled-model
parity and separate boundary/analyzer oracles.
**Validation:** `odf_engine_registry_domain_census`; registration/formatting probe.

### LD-02 — Arrow 59 per-domain `ExtensionType` impls

**Decision:** adopt (revises current single `Id16Extension`)
**Version basis:** arrow-schema `=59.2.0` extension module.
**Displaces:** `Id16Extension` (`src/schema_registry.rs:11-69`) and the
`codefabric.id16` name — DB01.
**Risk:** unknown-consumer degradation; fingerprint movement. Mitigated: per-domain
INT-09 round-trips in `odf_id_domain_extension_gate`; fingerprint moves in the WP07
governed release.
**Validation:** `just id-domain-extension-check` (successor recipe, WP07/WP08).

### LD-03 — deltalake `43a0cf10` Binary storage seam

**Decision:** retain-current (kernel has BINARY only, signed integers only).
**Displaces:** nothing. **Risk:** per-scan reattachment cost — PR-2 probes
the complete production `DeltaScanConfig::with_schema` provider contract; the current
wrapper is the default unless every projection/filter/pruning/statistics/batch leg passes.
**Validation:** existing round-trip gate unchanged; `odf_id_domain_lowering_conformance`.

### LD-04 — FSB literals / joins / nested keys

**Decision:** adopt-if-proven (PR-1); fallback = storage-typed literal rewrite
(`src/fabric.rs:905-916`) stands.
**Validation:** reviewed PR-1 decision transaction from WP02; consumed by WP08/WP13.

### LD-05 — Recursive CTEs / UDTFs for traversal

**Decision:** reject (design §2.2, §3.11); `GraphOperatorPlan` + derived lane remains
the sole traversal path; FAB's UDTF recommendation struck by amendment 5 (WP08).
**Validation:** `just query-legacy-zero-state-check` continues green.

### LD-06 — MemTable for operational control projections

**Decision:** retain-current. **Validation:** existing catalog oracles; WP14.

### LD-07 — String execution posture (Utf8View / dictionary)

**Decision:** retain-current, probe-gated (PR-7); any adoption is session-config
only, never schema. **Validation:** reviewed PR-7 decision transaction from WP02; no packet in this
plan flips the config.

### LD-08 — DataFusion 55 analyzer rule for ID-domain enforcement

**Decision:** adopt one idempotent, application-owned `AnalyzerRule` installed in every
serving `SessionState`; early binder diagnostics consume the same generated rule model.
**Displaces:** binder-only/path-local domain validators — DB01.
**Validation:** `odf_all_plan_ingresses_domain_checked`, cross-domain negative corpus,
double-analysis idempotence, binder/analyzer diagnostic equivalence.

### 2.2 Design-principle posture

The design carries per-decision P1–P25 citations (design §3); this plan inherits
them through packet design references. The load-bearing postures: one authority per
concept (P3 — WP03/WP04/WP09), executable models (P2 — WP03/WP05/WP08/WP11), truthful
capability claims (P20/P21 — WP08/WP15), immutable snapshots (P11 — unchanged
machinery), provenance closure (P9/P10 — WP13's checksum versioning and the
self-description oracle at M04).

## 3. Global target invariants

TI-1 … TI-9 are defined in design v3 §2.3 and are referenced by ID throughout this
plan. Functionality contracts F-1 … F-6 (design §2.1) bind every packet: no packet
may regress the eight query forms, snapshot pinning, determinism, absence-as-unknown,
identity recipes, or provenance closure. Two plan-wide standing rules:

- **Fingerprint discipline.** Packets marked *fingerprint-neutral* prove schema-byte
  equality (`AC-G-79` comparator) at their proving commit; packets marked
  *fingerprint-moving candidate-builders* (WP07, WP09, WP12) never imply activation.
  WP07 owns the Stage-2a release; WP17 exclusively owns Stage-2b owner acceptance and
  pointer advancement after the complete candidate passes its gates.
- **Gate hygiene.** Any packet that renames or relocates tests ships the
  filter-expression diff for the name-coupled recipes (WP01 policy).

## Audit Integration Log

Audit: `docs/reviews/plan_audit_codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27_v1.md`.
Source artifacts: design v2 + plan v1. Revised artifacts: design v3 + plan v2. Each
finding has exactly one disposition below. Revalidation commands are the audit's commands
verbatim; their recorded outcomes distinguish integration completeness from future
implementation proof.

| Finding | Disposition | Resolution | Revalidation command and integration-time outcome | Rationale |
|---|---|---|---|---|
| F-001 | `applied-design` | Design TI-1/TI-8, D-01; WP09/WP17; M03; `ontology-self-description-check` | `just ontology-self-description-check` — exit 1: recipe absent until WP17 (expected pre-implementation) | Complete normalized contract plane plus dynamic new-domain oracle closes recursive self-description. |
| F-002 | `applied-design` | Design TI-1, D-01; WP09/WP11; DB06 | `just ontology-relational-closure-check` — exit 1: recipe absent until WP11 (expected pre-implementation) | N:M memberships are `ontology_edge` rows; no list-valued ontology membership authority remains. |
| F-003 | `applied-design` | Design TI-2, LD-01/LD-08; WP06–WP08; DB01 | `just id-domain-extension-check && just semantic-query-conformance-check && just publication-referential-integrity-check` — exit 1: first recipe absent until WP07 | Registry, Arrow type, cast, and universal analyzer responsibilities are explicit and every plan ingress is covered. |
| F-004 | `applied-design` | Design TI-7/TI-9, D-08/D-09; WP03/WP05/WP11/WP13 | `just model-repro-check && just query-form-contract-check && just governance-scan` — exit 1: first two gates passed; governance reached the pre-existing public-error-closure failure for three unregistered prefixes | One governed pass and typed compiled object replace parallel metadata-driven pipelines and bespoke relational logic. |
| F-005 | `applied-design` | Design TI-6, D-06; WP11/WP15 | `just provider-statistics-contract-check && just ontology-dimension-check` — exit 1: existing statistics selector passed 3/3; ontology recipe absent until WP11 | Statistics requests are declined; constraints are validation-gated and accurately classified. |
| F-006 | `applied-plan` | WP06 dependency; WP09–WP17 DAG; M02/M03/M04; §8 | `just plan-dependency-check` — exit 0: active-plan closure reports 38 packets/0 overlaps; inactive v2 `validate_plan` separately passed 17 WP/4 M/6 DB | Packet dependencies, milestone barriers, shared-file ownership, and execution graph now agree. |
| F-007 | `added-packet` | Design §5.2 Stage 2b; WP17; M03 | `cargo nextest run --no-fail-fast --no-tests=fail -E 'test(stage2b_atomic_activation_fault_injection) | test(dimension_version_stability_across_fact_publications)'` — exit 4: zero tests until WP17 (expected pre-implementation) | WP17 is the sole Stage-2b acceptance/activation owner and defines stable dimension versions. |
| F-008 | `applied-plan` | Design §7; WP02 decision protocol | `just probe-suite && git diff --exit-code` — exit 1: recipe absent until WP02 (expected pre-implementation) | Probes emit immutable observations under `target`; accountable state transactions select branches. |
| F-009 | `applied-design` | Design TI-4/D-03; WP12 | `just structure-classification-check` — exit 1: recipe absent until WP12 (expected pre-implementation) | SourceSpan's logical classification is invariant; PR-3 selects physical lowering only. |
| F-010 | `applied-plan` | Frontmatter, §1.3, declared inputs | `just plan-status && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py --plan docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md artifacts-check` — exit 1: active plan status passed; inactive draft has no v2 state until activation; direct `validate_plan` passed | Baseline is WP08-complete HEAD and Wave 9 preflight is explicitly excluded. |
| F-011 | `applied-plan` | PR-2 protocol; WP02/WP07 | `cargo nextest run --no-fail-fast --no-tests=fail -E 'test(delta_scan_schema_override_full_provider_contract)'` — exit 4: zero tests until WP02 (expected pre-implementation) | PR-2 now proves the production delta-rs provider seam, not a synthetic `MemTable`. |

Integration artifact validation: the inactive-plan structural validator passed with 17
packets, 4 milestones, 6 decommission batches, four unique oracles per packet, current
declared-input digests, and an acyclic dependency graph. `just typos` exits 2 because the
intentional `odf_*` oracle namespace is not in the repository spelling allow-list (the
same pre-existing condition is present in plan v1); targeted JSON output contains no
non-`odf` finding in design v3 or plan v2. Source design v2 and plan v1 were not edited.

## 4. Work packets

### Stage 0 group — evidence floor and probes (design §5.2 Stage 0)

### WP01 — Protective oracle promotion, perf anchor, gate-diff policy

**Outcome.** The fabric's load-bearing oracles survive the refactor of the files
they live in: serving-equivalence, checksum-KAT, catalog-freeze, and
overlay-composition oracles run from `tests/integration/`; a perf baseline anchor
exists at the pre-change commit; the gate filter-expression diff policy is
mechanically checkable.

**Dependencies.** None.

**Target invariants.** TI-8; F-4.

**Design and library references.** Design §5.2 Stage 0; ops-review transition risks
1, 2, 5 (design session); `.config/nextest.toml` profiles; `justfile` name-coupled
recipes (`wave3-integration-check`, `query-determinism-check`,
`id16-extension-contract-check`, `provider-statistics-contract-check`,
`publication-referential-integrity-check`).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'wp(03|58|64|65|66)_|datafusion_55_serving_equivalence|checksum_kat' src/fabric/serving.rs src/fabric/result_checksum.rs src/fabric/snapshot_catalog.rs src/fabric/overlay.rs tests/integration/
rg -n "nextest run.*-E" justfile | head -30
ls tests/integration/
```

Known touch: `tests/integration/integration.rs` (new case modules under
`tests/integration/`), `justfile` (filter expressions), no `src/` behavior.

**Required changes.**

1. Promote (copy, not move — originals stay until their packets edit those files)
   the protective subset: serving equivalence, arrow58/59 checksum KATs, catalog
   freeze rejection, overlay composition, into `tests/integration/` case modules
   under the single `tests/integration.rs` target.
2. Capture the perf anchor: run the predeclared `data-fabric-upgrade-bench`
   workload at the pre-change commit and record commit, feature/pin graph, workload,
   fixture, OS/hardware/power posture, warm-up, sample count, and raw sample artifact in
   the benchmark comparator contract. The comparator uses repeated measurements and a
   declared statistical/non-inferiority rule; it never requires an exact median replay.
3. Add a `scripts/`-side check that renders the current `nextest` filter
   expressions of the name-coupled recipes to a committed manifest, so a later diff
   is mechanical; wire as recipe `gate-filter-census`.
4. Extend `plan-dependency-check` with a v2 readiness-state oracle that enumerates legal
   topological states and rejects WP05/WP06 before WP03, WP09 before WP08, WP17 before
   WP09–WP13/WP16, and WP14/WP15 before WP17; retain unordered known-touch overlap checks.

**Legacy Disposition and Decommission.** Protective copies are temporary: WP06 retires
the original serving-equivalence home, WP10/WP17 retire catalog-freeze copies, WP13
retires checksum-KAT copies, and WP15 retires overlay-composition copies after each
promoted oracle proves equivalent selection. No duplicate oracle authority survives M04.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_promoted_fabric_oracles_green`; Executable oracle: `odf_stage0_governance_readiness`; Executable oracle: `odf_gate_empty_selection_rejection`; Executable oracle: `odf_perf_baseline_anchor_captured`.

- **Behavioral — Executable oracle:** `odf_promoted_fabric_oracles_green` — the
  promoted integration copies pass against the unmodified tree and select > 0 tests
  per recipe filter.
- **Structural — Executable oracle:** `odf_stage0_governance_readiness` — the committed
  filter-expression manifest matches the live `justfile`, and exhaustive readiness-state
  enumeration proves every v2 stage barrier and unordered known-touch disposition.
- **Negative/Zero-State — Executable oracle:** `odf_gate_empty_selection_rejection`
  — every name-coupled recipe runs with `--no-tests=fail` semantics; a synthetic
  rename in a scratch worktree makes the census check fail.
- **Operational — Executable oracle:** `odf_perf_baseline_anchor_captured` — the
  anchor ref and environment/workload digests resolve, raw samples validate, and a
  comparator self-test proves the declared statistical rule catches a seeded regression
  without demanding identical medians.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test`, `just wave3-integration-check`,
`just plan-dependency-check`, `just packet-oracle-check WP01`.

**Integration Milestone.** M01.

**Replan Triggers.** A protective oracle cannot run outside its home module without
widening visibility — split that oracle's promotion into the packet that edits its
home file, and record the exception.

**Rollback or Recovery.** Additive; revert by commit.

### WP02 — Probe suite PR-1…PR-7

**Outcome.** All seven design probes exist as executable, non-mutating tests at the
pinned versions. Each run emits an immutable observation report under `target/`; an
accountable reviewer records the selected named branch and report digest in the v2 state
decision transaction. Downstream packets consume that reviewed transaction and reject
pin/config/evidence drift rather than treating test output as architecture authority.

**Dependencies.** None.

**Target invariants.** TI-8; design §7; L-2…L-6.

**Design and library references.** Design §7 (probe table), LD-01…LD-04, LD-07;
`datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` §40A +
gates V1–V6; deltalake reference §4.19/§6.5–6.6.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'probe' tests/integration/ src/compatibility.rs justfile | head -20
cargo tree -p datafusion -e no-dev --depth 0 && cargo tree -p arrow --depth 0
```

Known touch: new `tests/integration/` probe module(s), a `probe-suite` recipe, and the
state schema-v2 decision transaction at execution. Probe reports live only under
`target/ontology-fabric-probes/`; no committed observation fixture is created.

**Required changes.**

1. PR-1 `ScalarValue::FixedSizeBinary` literal/IN-list/join/group-by; PR-2 full
   production delta-rs provider seam using `DeltaScanConfig::with_schema` — projection,
   filters/pruning, statistics, physical plan, and emitted-batch FSB-over-Binary schema,
   with the current wrapper as the explicit default; PR-3a struct `{Int64,Int64}`
   Delta round-trip; PR-3b span pruning under production session config; PR-4
   Delta file-statistics exposure; PR-5 Parquet `ARROW:schema` metadata round-trip
   with per-domain names; PR-6 unused-left-join elimination (with and without PK
   already-validated PK constraints); PR-7 view-types-enabled correctness followed by a
   recorded-environment repeated performance comparison.
2. Each probe writes a structured observation (probe id, resolved pin/feature graph,
   environment/session/workload/fixture digests, command, verdict, raw plan/result
   evidence) to a content-addressed report beneath `target/ontology-fabric-probes/`.
3. The reviewer records exactly one branch per probe in the execution-state decision
   transaction with reviewer, timestamp, report digest, pin/config digest, rationale, and
   fallback. Downstream packets validate this closure before consuming the decision.
4. `just probe-suite` verifies the worktree is unchanged; a probe that writes tracked
   state or embeds a selected branch in test source fails.

**Legacy Disposition and Decommission.** None.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_probe_suite_observations_complete`; Executable oracle: `odf_probe_pin_identity`; Executable oracle: `odf_probe_decision_transaction_closure`; Executable oracle: `odf_probe_worktree_immutability`.

- **Behavioral — Executable oracle:** `odf_probe_suite_observations_complete` — all
  seven probes execute and each content-addressed report contains the required raw
  evidence and environment/session/workload identity.
- **Structural — Executable oracle:** `odf_probe_pin_identity` — probes assert the
  resolved datafusion/arrow/deltalake identities equal the pinned baseline before
  recording a verdict.
- **Negative/Zero-State — Executable oracle:** `odf_probe_decision_transaction_closure`
  — an unreviewed report, missing branch/fallback, wrong pin/config digest, or report
  drift blocks every dependent packet.
- **Operational — Executable oracle:** `odf_probe_worktree_immutability` — the suite
  leaves `git diff` unchanged and reruns preserve semantic verdicts while permitting raw
  timing samples to vary inside the recorded comparator protocol.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test-rust`, `just stable-graph-check`,
`just packet-oracle-check WP02`.

**Integration Milestone.** M01.

**Replan Triggers.** A probe outcome that contradicts a design L-fact (not merely
selects a fallback) — e.g. Delta accepts FSB natively — reopens the design decision
it grounds (design §8 reopening list).

**Rollback or Recovery.** Additive; revert by commit.

### Stage 1 group — single seam, registry-complete compiler, session reshape (design §5.2 Stage 1)

### WP03 — One generated column authority

**Outcome.** A registered `SchemaContractCompilation` transformation pass validates all
registry YAML/JSON and Schema Contract IR inputs once and emits one versioned typed
`CompiledOntology`. Its merged column shape per table replaces the dual emission/runtime
reconciliation immediately; it also defines the closed typed inputs later packets use for
ontology tables, ID/identity contracts, phrase/rule plans, result schemas, and semantic
operations. Schema bytes are proven unmoved.

**Dependencies.** WP01.

**Target invariants.** TI-3, TI-7, TI-9 (fingerprint-neutral).

**Design and library references.** Design §3.9 D-07, §3.11 D-09; current seam
`src/schema_registry.rs:561-583` (`model_field`),
`src/generated/model_schema_tables.rs`, `src/generated/table_specs.rs`,
`src/bin/codefabric_model/schema_driver.rs`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'MODEL_TABLES|ModelColumn|GeneratedColumn|model_field' src/schema_registry.rs src/generated/ src/bin/codefabric_model/schema_driver.rs
rg -ln 'model_schema_tables|table_specs::' src/
```

Known touch: `contracts/registry/transformation-pass-registry.yaml`,
`src/bin/codefabric_model/schema_driver.rs`, shared application-owned compiled-model
types, regenerated
`src/generated/{model_schema_tables,table_specs}.rs` (or their merged successor),
`src/schema_registry.rs`.

**Required changes.**

1. Register `SchemaContractCompilation` with its complete inputs, typed output families,
   dependency order, invalidation keys, determinism class, diagnostics, and tests; define
   closed typed `CompiledOntology` values (no provider-owned types, EAV, or opaque JSON
   rule AST).
2. Parse/validate each authority once, cross-link total references, and merge the two
   generated column shapes into the compiled object; keep generated module layout local.
3. Point every `schema_registry` lowering read at the merged shape; delete
   `model_field`'s dual-list reconciliation.
4. Add a downstream-consumer boundary: later generators accept `CompiledOntology` and
   may not parse authority files directly. Re-run model generation twice and prove
   compiled-object bytes and schema bytes equal across isolated runs.

**Legacy Disposition and Decommission.** `model_field` reconciliation → delete
(DB02). The superseded generated symbols reach zero within this packet (tier-1
clean build after deletion).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_compiled_ontology_reproducible`; Executable oracle: `odf_schema_contract_pass_closure`; Executable oracle: `odf_dual_list_reconciliation_zero`; Executable oracle: `odf_stage1_schema_fingerprint_equality`.

- **Behavioral — Executable oracle:** `odf_compiled_ontology_reproducible` — isolated
  compilations yield byte-identical `CompiledOntology`; every table's lowered schema is
  byte-identical to the pre-packet capture.
- **Structural — Executable oracle:** `odf_schema_contract_pass_closure` — the pass
  registry names every actual input/output/invalidation edge; every downstream generator
  consumes the compiled object and no direct authority parser remains outside the pass.
- **Negative/Zero-State — Executable oracle:** `odf_dual_list_reconciliation_zero`
  — ast-grep + `rg` zero-hit for the reconciliation function and legacy dual-shape
  symbols over `src/`; clean `cargo check` with them deleted.
- **Operational — Executable oracle:** `odf_stage1_schema_fingerprint_equality` —
  the `AC-G-79` comparator reports exact fingerprint equality for all 39 tables.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, focused
`schema_registry` unit tests.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just packet-oracle-check WP03`.

**Integration Milestone.** M01.

**Replan Triggers.** Merged emission cannot reproduce byte-identical schemas
(ordering or metadata differences surface) — stop; that is evidence of a live
inconsistency between the two lists, which must be dispositioned before deletion.

**Rollback or Recovery.** Revert by commit; regeneration is deterministic.

### WP04 — Generated row shapes for ingest

**Outcome.** The 29 hand-written `*Row` structs in `fact_ingest.rs` are displaced
by row-shape definitions generated beside the encoders; ingest logic is unchanged;
row shapes can no longer drift from the schema.

**Dependencies.** WP03.

**Target invariants.** TI-7 (fingerprint-neutral).

**Design and library references.** Design §3.9; `src/fact_ingest.rs:65-578`,
`src/generated/fact_row_encoders.rs` (include mechanism at `fact_ingest.rs:733`).

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep run -l rust -p 'struct $N { $$$ }' src/fact_ingest.rs | rg 'Row'
rg -n 'include!' src/fact_ingest.rs
rg -ln 'EntityRow|RelationRow|PropertyFactRow|FactEvidenceRow' src/ tests/
```

Known touch: `src/bin/codefabric_model/schema_driver.rs` (row-shape emission),
`src/generated/fact_row_encoders.rs` sibling, `src/fact_ingest.rs`.

**Required changes.**

1. Emit row-struct definitions (field set = the merged column shape, encoder-typed)
   from the schema driver; include them where the encoders are included.
2. Delete the hand-written structs; adapt construction sites only where field
   names/types differ (differences are packet evidence, not silent fixes).

**Legacy Disposition and Decommission.** 29 hand-written structs → replace-shape /
preserve-logic (DB02); zero-state within the packet.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_generated_row_shape_encoder_parity`; Executable oracle: `odf_row_shape_field_census`; Executable oracle: `odf_handwritten_row_struct_zero`; Executable oracle: `odf_ingest_replay_equivalence`.

- **Behavioral — Executable oracle:** `odf_generated_row_shape_encoder_parity` —
  encoding fixture rows through generated shapes yields batches byte-identical to
  the pre-packet capture.
- **Structural — Executable oracle:** `odf_row_shape_field_census` — every encoder
  input field maps to exactly one generated row field; no unused or missing field.
- **Negative/Zero-State — Executable oracle:** `odf_handwritten_row_struct_zero` —
  ast-grep + `rg` zero-hit for the 29 struct names as local definitions in
  `fact_ingest.rs`; clean build.
- **Operational — Executable oracle:** `odf_ingest_replay_equivalence` — a fixture
  publication built through the new shapes matches the golden publication digest.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just packet-oracle-check WP04`.

**Integration Milestone.** M01.

**Replan Triggers.** A hand-written struct carries semantics beyond its field set
(custom impls the generator cannot express) — keep that struct hand-written with a
recorded exception rather than generating behavior.

**Rollback or Recovery.** Revert by commit.

### WP05 — Registry-complete compiler and phrase-binding unification

**Outcome.** The semantic compiler contains zero literal ontology/enum code values;
phrase→predicate bindings compile from `CompiledOntology` into closed typed
`SemanticOperationSpec` variants; the divergent certainty sets (relational
`{10,20}` vs graph `{10,20,30,50}`) are unified to the registry-decided set (owner
decision A-4) with the behavior change pinned by oracle on both paths.

**Dependencies.** WP03.

**Target invariants.** TI-7, TI-9; F-1 (fingerprint-neutral; one recorded behavior fix).

**Design and library references.** Design §3.10 (D-08 + discovered defect);
`src/semantic_query.rs:1333-1352, 1961-1975`; `contracts/registry/phrase-registry.yaml`;
`contracts/registry/enum-registry.yaml` (`EVIDENCE_CERTAINTY` domain);
`src/generated/registries.rs` code constants.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'ScalarValue::Int16\(Some\([0-9]' src/semantic_query.rs
rg -n 'as_str\(\)' src/semantic_query.rs | head -20
rg -n 'certainty|resolution|directness' contracts/registry/phrase-registry.yaml | head -20
```

Known touch: `contracts/registry/phrase-registry.yaml` (authoritative code-set
decision, owner-accepted), `src/bin/codefabric_model/` (binding emission),
`src/generated/` (binding rows/constants), `src/semantic_query.rs`, new `rules/`
governance rules with `rule-tests/` fixtures.

**Required changes.**

1. Record the authoritative phrase→certainty-code binding in the phrase registry
   (owner acceptance in the packet record).
2. Compile binding rows into closed typed operation variants (column, operator, typed
   operand/code set, null/unknown policy, output role); replace both hand-written sites;
   delete divergent literals. Algorithmic exceptions require a typed algorithm variant,
   phrase ID, input/output/determinism/diagnostic contract.
3. Governance rules: (a) no bare integer literals in the compiler's
   predicate-construction modules; (b) no phrase match arm without a
   phrase-registry ID; both with negative-space fixtures.

**Legacy Disposition and Decommission.** Literal predicates and unregistered arms →
replace (DB05); zero-state via the governance rules thereafter.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_phrase_binding_dual_path_parity`; Executable oracle: `odf_phrase_arm_registry_coverage`; Executable oracle: `odf_literal_code_predicate_zero`; Executable oracle: `odf_phrase_governance_rules_active`.

- **Behavioral — Executable oracle:** `odf_phrase_binding_dual_path_parity` — the
  same phrase produces the same registry-decided code set on the relational and
  graph paths; the pinned expected sets come from the phrase registry, not the code.
- **Structural — Executable oracle:** `odf_phrase_arm_registry_coverage` — every
  phrase arm in the compiler maps to a phrase-registry ID; census equality both
  directions for bindings the registry marks compiled.
- **Negative/Zero-State — Executable oracle:** `odf_literal_code_predicate_zero` —
  the two governance rules fire on seeded violations and report zero findings on
  the tree.
- **Operational — Executable oracle:** `odf_phrase_governance_rules_active` —
  `governance-scan` includes both rules (error severity) and their `rule-tests`
  snapshots pass.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, focused semantic-query
tests.

**Packet-Local Gates.** `just governance-scan`, `just semantic-query-conformance-check`,
`just model-repro-check`, `just packet-oracle-check WP05`.

**Integration Milestone.** M01.

**Replan Triggers.** The registry-decided set changes any released KAT beyond the
two known sites — the behavior change is bigger than designed; stop and re-scope.

**Rollback or Recovery.** Revert by commit; the phrase-registry decision persists
as an accepted record either way.

### WP06 — Session reshape and one validation seam

**Outcome.** The serving session is built through `SessionStateBuilder` with an installed
(initially empty) `MemoryExtensionTypeRegistry` and an initially no-op analyzer seam; the
five scattered Arrow field-validation call sites collapse into one `schema_registry`
helper. Registry, field validation, casts, and analyzer policy are distinct seams and
serving behavior is proven unchanged.

**Dependencies.** WP03.

**Target invariants.** TI-2 (preparation; fingerprint-neutral).

**Design and library references.** Design §3.4 moves 1–2; LD-01, LD-08;
`src/fabric/serving.rs:496` (`SessionContext::new_with_config_rt`);
`has_valid_extension_type` call sites (`src/fact_ingest.rs:821`,
`src/fabric/publication.rs:609`, `src/fabric.rs:1209`, `src/fabric/serving.rs:5349`,
`src/schema_registry.rs`).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'has_valid_extension_type|try_extension_type' src/
rg -n 'new_with_config_rt|SessionStateBuilder' src/fabric/serving.rs
```

Known touch: `src/fabric/serving.rs`, `src/schema_registry.rs`, the four other
call-site files.

**Required changes.**

1. Build session state via `SessionStateBuilder::with_default_features()` + config +
   runtime + `with_extension_type_registry` (empty registry) + one application-owned
   analyzer seam (no-op until WP08); `SessionContext::new_with_state`.
2. One `schema_registry`-owned Arrow field-validation helper; call sites delegate and its
   API cannot be mistaken for logical-plan domain enforcement.

**Legacy Disposition and Decommission.** Scattered validation idioms → reshape into
the helper; zero-state for direct `has_valid_extension_type::<Id16Extension>` calls
outside the helper.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_session_builder_equivalence`; Executable oracle: `odf_extension_registry_and_analyzer_seams_installed`; Executable oracle: `odf_scattered_extension_check_zero`; Executable oracle: `odf_serving_equivalence_post_reshape`.

- **Behavioral — Executable oracle:** `odf_session_builder_equivalence` — plans and
  results for the conformance corpus are identical before/after the session
  reshape.
- **Structural — Executable oracle:** `odf_extension_registry_and_analyzer_seams_installed`
  — the serving state exposes one empty registry and exactly one no-op analyzer seam;
  Arrow validation/cast/analyzer responsibilities have distinct typed interfaces.
- **Negative/Zero-State — Executable oracle:** `odf_scattered_extension_check_zero`
  — ast-grep zero-hit for direct extension-validation idioms outside the shared
  helper.
- **Operational — Executable oracle:** `odf_serving_equivalence_post_reshape` — the
  promoted WP01 serving-equivalence oracle passes; artifact/evidence records
  unchanged in shape.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test`, `just query-determinism-check`,
`just packet-oracle-check WP06`.

**Integration Milestone.** M01.

**Replan Triggers.** `SessionStateBuilder` path changes any default the current
construction relied on (config drift surfaces in equivalence oracle) — enumerate
and pin the divergent defaults before proceeding.

**Rollback or Recovery.** Revert by commit.

### Stage 2a group — per-domain logical ID types (design §5.2 Stage 2a; fingerprint-moving)

### WP07 — ID-domain registry, per-domain extension types, id16 retirement

**Outcome.** A Contract-IR ID-domain registry enumerates every ID domain with its
extension-type name and preimage-recipe binding; the model compiler generates
per-domain Arrow `ExtensionType` impls and DataFusion `DFExtensionType` factories; the
single lowering attaches each ID
column's (and list element's) domain type; `hash32` becomes FSB(32) +
`codefabric.hash32`; `codefabric.id16` reaches zero; FAB §7 is amended; the
schema-bundle release and workspace republish complete under `EXACT_PIN`.

**Dependencies.** WP02, WP03, WP06.

**Target invariants.** TI-2, TI-3 (fingerprint-moving release).

**Design and library references.** Design §3.4 (D-02), §5.4 amendment 2; LD-02,
LD-03; `src/schema_registry.rs` lowering + `Id16Extension`;
`src/fabric.rs:874-916, 1203-1218` (storage seam); `docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` §7.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'Id16Extension|codefabric\.id16' src/ contracts/ docs/upfront_design/ -g '!docs/library_ref/**'
rg -n 'id16|hash32' contracts/schema/schema-contract-ir.json | head -20
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^7\.'
```

Known touch: `contracts/schema/schema-contract-ir.json` (ID-domain registry +
per-column domains), `src/bin/codefabric_model/` (extension-type emission),
`src/schema_registry.rs`, `src/fabric.rs` (seam generalization), FAB §7 text,
`justfile` (`id-domain-extension-check` successor recipe), regenerated
`src/generated/*` and `contracts/generated/*` + bundles.

**Required changes.**

1. Contract-IR revision: ID-domain registry (domain slug, extension name
   `codefabric.<domain>_id`, preimage-recipe id); every ID column and `IdList`
   element declares its domain; `hash32` logical type lowers to FSB(32) +
   `codefabric.hash32`.
2. Generate per-domain Arrow `ExtensionType` impls, DataFusion registration factories,
   `DomainTypedLiteral`, and the domain table consumed by WP09's `id_domain` dimension;
   lowering attaches domain types; storage seam
   (`Id16ContractProvider` generalized) re-presents Binary as the domain-typed FSB
   schema; filter-literal rewrite per reviewed PR-1 decision. Adopt
   `DeltaScanConfig::with_schema` only on a positive reviewed PR-2 decision covering the
   full production provider contract; otherwise retain the wrapper.
3. Retire `Id16Extension`/`codefabric.id16`; rename the gate to
   `id-domain-extension-check` with per-domain round-trips (Parquet metadata leg
   per PR-5).
4. Amend FAB §7 (canonical FSB(16/32) + per-domain extension types + Binary Delta
   storage mapping); regenerate bundles; run the migration probe and workspace
   republish; record owner acceptance.

**Legacy Disposition and Decommission.** `Id16Extension` and `codefabric.id16` →
replace (DB01): zero-state over `src/`, `contracts/`, `docs/upfront_design/`
(scalar + list children); the old recipe name retires with a `justfile` alias
removal in the same commit.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_id_domain_lowering_conformance`; Executable oracle: `odf_id_domain_registry_census`; Executable oracle: `odf_id16_zero_state`; Executable oracle: `odf_id_domain_republish_migration`.

- **Behavioral — Executable oracle:** `odf_id_domain_lowering_conformance` — every
  ID column and list element carries its declared domain's extension type through
  provider schema, plan, and delivered batch; round-trip gate passes with exact
  field+metadata comparison.
- **Structural — Executable oracle:** `odf_id_domain_registry_census` — every ID
  column in the merged column authority maps to exactly one domain; no ID column
  or list element is domainless; extension names are unique and namespaced.
- **Negative/Zero-State — Executable oracle:** `odf_id16_zero_state` — ast-grep +
  `rg` zero-hit for `Id16Extension`/`codefabric.id16` over `src/`, `contracts/`,
  `docs/upfront_design/`; clean build with the type deleted.
- **Operational — Executable oracle:** `odf_id_domain_republish_migration` — the
  `EXACT_PIN` migration probe passes; the republished workspace activates; the
  schema bundle digest advances exactly once and owner acceptance is recorded.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, focused schema/fabric
tests.

**Packet-Local Gates.** `just model-repro-check`, `just id-domain-extension-check`
(new), `just root-test`, `just data-fabric-stack-compat`,
`just packet-oracle-check WP07`.

**Integration Milestone.** M02.

**Replan Triggers.** Per-domain generation forces >1 extension impl pattern the
generator cannot express uniformly; or the migration probe requires data rewrite
(design A-2 false) — stop, reopen transition sequencing.

**Rollback or Recovery.** Prior schema bundle remains activatable until owner
acceptance; revert regenerates the previous shape deterministically.

### WP08 — Engine registration, domain-conformance rule, extension amendments

**Outcome.** All ID domains are registered in the serving extension-type registry and
one idempotent DataFusion `AnalyzerRule` enforces domain conformance for every logical-plan
ingress. Same-domain plans are unchanged; cross-domain comparisons, joins, `IN` lists,
casts, and set operations fail with typed errors. Binder diagnostics delegate to the same
compiled rule model; amendment 5 lands.

**Dependencies.** WP07.

**Target invariants.** TI-2; F-1 (fingerprint-neutral).

**Design and library references.** Design §3.4 (domain-conformance rule), §5.4
amendment 5; LD-01, LD-05, LD-08; `src/semantic_query.rs` bind stage;
`src/fabric/serving.rs` session build (WP06 seam).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'bind_request|BoundQueryBlock|validate' src/semantic_query.rs | head -20
rg -n 'cpg_neighbors|cpg_reachable|UDTF' docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | head
```

Known touch: `src/fabric/serving.rs` (registry + analyzer installation),
application-owned analyzer module, `src/semantic_query.rs` (early diagnostic delegation),
FAB §§78–82/93 + `AC-G-20` text,
metadata classification dictionary (consumers).

**Required changes.**

1. Populate the WP06 registry with every generated DataFusion registration factory;
   exact census against `CompiledOntology`.
2. Install one idempotent analyzer rule consuming the compiled domain/rule model. Cover
   every authorized plan ingress and comparison/equi/non-equi join/IN/cast/set-operation
   shape; domain erasure/unknown fails closed. Double analysis preserves valid plans.
3. Make bind-stage validation an optional early diagnostic delegating to the same rule
   model; prove binder/analyzer diagnostic equivalence and no binder-only authority.
4. Name and test actual consumers separately: registry resolution/formatting, Arrow field
   validation, storage-seam casts, analyzer enforcement; update classifications.
5. Amend FAB §§78–82/93 (UDTF recommendation struck; `GraphOperatorPlan` + derived
   lane canonical) and the `AC-G-20` extension example to the ID-domain registry.

**Legacy Disposition and Decommission.** None new; completes DB01's gate rename
verification at HEAD.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_domain_conformant_plans_execute`; Executable oracle: `odf_all_plan_ingresses_domain_checked`; Executable oracle: `odf_cross_domain_plan_rejection`; Executable oracle: `odf_extension_consumer_classification`.

- **Behavioral — Executable oracle:** `odf_domain_conformant_plans_execute` — the
  conformance corpus (same-domain joins/filters across all eight forms) executes
  unchanged; diagnostics render domain-typed ID literals via the registry.
- **Structural — Executable oracle:** `odf_all_plan_ingresses_domain_checked` — registry
  contents equal `CompiledOntology`; every authorized logical-plan constructor reaches
  exactly one analyzer rule; double analysis is idempotent.
- **Negative/Zero-State — Executable oracle:** `odf_cross_domain_plan_rejection` —
  cross-domain comparison/join, wrong-domain literal, mixed-domain IN-list, erasing cast,
  and set-operation mismatch each yield typed rejection; binder/analyzer diagnostics
  agree; `rg` zero-hit for UDTF recommendations in amended FAB sections.
- **Operational — Executable oracle:** `odf_extension_consumer_classification` —
  the metadata classification dictionary names a consumer for every claimed engine
  behavior; the classification census validates.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just semantic-query-conformance-check`,
`just query-determinism-check`, `just id-domain-extension-check`,
`just packet-oracle-check WP08`.

**Integration Milestone.** M02.

**Replan Triggers.** A DataFusion logical-plan ingress cannot be made to traverse the
installed analyzer rule, or analyzer ordering erases the domain before validation — stop
and reopen LD-08; a binder-only fallback is not authorized.

**Rollback or Recovery.** Revert by commit; registrations are session-scoped.

### Stage 2b group — the ontology plane (design §5.2 Stage 2b; fingerprint-moving)

### WP09 — Complete normalized ontology plane and registry builders

**Outcome.** Twenty normalized Delta-backed `BundleDimension` relations exist under the
Contract IR: nine typed vocabulary dimensions plus eleven self-description relations for
terms/edges/authorities/types/table-column-result contracts/identity/phrases/rules. N:M
memberships are rows in `ontology_edge`, never nested lists. Generated builders consume
`CompiledOntology`; FAB §6.3/§8 are amended. Non-active candidate versions are built, but
Stage 2b is not accepted or activated here.

**Dependencies.** WP08.

**Target invariants.** TI-1, TI-7, TI-8, TI-9 (fingerprint-moving candidate).

**Design and library references.** Design §3.3 (D-01), §5.4 amendments 1+3;
`enum_catalog` pattern (`src/fabric.rs:1136-1172`, table code 11);
`contracts/registry/ontology-*.yaml`, `contracts/generated/provider-raw-kinds/`;
`GENERATED_SEMANTIC_TYPE_BINDINGS`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'enum_catalog' src/ contracts/ -g '!**/generated/**'
rg -n 'BundleDimension|required_for_publication' src/generated/table_specs.rs | head
just spec-outline docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md --match '^(6|8)\.'
```

Known touch: `contracts/schema/schema-contract-ir.json` (twenty ontology tables +
relocation), `src/bin/codefabric_model/` (dimension driver), `src/fabric.rs`
(generalized builders), FAB §6.3/§8 text, regenerated artifacts + bundles.

**Required changes.**

1. Contract-IR revision: the design §3.3 twenty-table set as `BundleDimension` +
   `BaseImmutable`, with authority/version/digest provenance; `enum_catalog` relocates to
   `cpg_ontology.enum_domain` at the same grain.
   `result_schema`/`result_field` first describe every currently active response shape;
   WP13 performs the governed target-shape revision after M03.
2. Normalize every N:M membership into typed `ontology_edge` rows. `relation_kind` and
   all other tables contain no list-valued family/member/owner/property declarations;
   `ontology_term` universally resolves integer/text code domains.
3. Builders consume `CompiledOntology` and render all relations; `src/fabric.rs`
   population generalizes from the enum-catalog special case. Tables are written only at
   workspace bootstrap or when an input authority/compiled digest changes; ordinary fact
   publications reuse the exact prior ontology versions.
4. Amend FAB §6.3/§8 with the normalized plane, lifecycle, candidate invisibility, and
   single Stage-2b activation owner.
5. Build candidate Delta versions and candidate manifest only. No owner acceptance or
   active-pointer mutation is permitted until WP17.

**Legacy Disposition and Decommission.** `cpg_base.enum_catalog` address → replace
(DB04): the table identity moves; consumers (serving decoration, allowlist) update
in WP10; the old address reaches zero at WP10's proving commit.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_dimension_registry_parity`; Executable oracle: `odf_ontology_contract_table_census`; Executable oracle: `odf_nested_ontology_membership_zero`; Executable oracle: `odf_ontology_candidate_version_stability`.

- **Behavioral — Executable oracle:** `odf_dimension_registry_parity` — dimension
  rows equal registry YAML rows (codes, names, semantic columns) and generated
  Rust constants; digests match the registry bundle.
- **Structural — Executable oracle:** `odf_ontology_contract_table_census` — exactly
  twenty complete relations; every governed code/type/table/column/result/identity/phrase/
  rule authority has one discoverable row and source digest.
- **Negative/Zero-State — Executable oracle:** `odf_nested_ontology_membership_zero` —
  Contract IR/generated tables contain no list-valued ontology membership authority;
  `cpg_base.enum_catalog` is absent; every expected membership is an `ontology_edge` row.
- **Operational — Executable oracle:** `odf_ontology_candidate_version_stability` — a
  candidate manifest pins all twenty relations; identical ontology inputs across two fact
  publications reuse identical Delta versions; candidate is invisible to active leases.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just publication-referential-integrity-check`, `just packet-oracle-check WP09`.

**Integration Milestone.** M03.

**Replan Triggers.** A registry carries rows the IR's closed column model cannot
express (schema-contract compiler rejection) — extend the IR model first; do not
truncate registry semantics to fit.

**Rollback or Recovery.** Discard the non-active candidate; active pointer is untouched.

### WP10 — `cpg_ontology` serving namespace and decoration

**Outcome.** The frozen candidate catalog serves `cpg_ontology`; the plan allowlist admits
operator/compiler use; serving-view decoration extends to `ontology:*` and raw-kind codes
per projection declarations. Decoration breadth follows the reviewed PR-6 decision. PK
constraints remain withheld until WP11 proves uniqueness and WP15 classifies/exposes
them; the active pointer is unchanged.

**Dependencies.** WP09.

**Target invariants.** TI-1, TI-8 (fingerprint-neutral beyond WP09's candidate shape).

**Design and library references.** Design §3.3 (serving decoration), §3.13
(allowlist); `src/fabric/serving.rs:48-53, 1240-1281, 1733-1785`;
`src/fabric/snapshot_catalog.rs` catalog assembly.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'cpg_base|SCHEMA_NAMES|allowlist' src/fabric/serving.rs src/fabric/snapshot_catalog.rs | head -20
rg -n 'serving_view_plan|enum:' src/fabric/serving.rs
```

Known touch: `src/fabric/serving.rs`, `src/fabric/snapshot_catalog.rs`,
Contract-IR `serving_projections` records (decoration declarations), regenerated
artifacts.

**Required changes.**

1. Register the `cpg_ontology` schema in the frozen catalog (dimension providers
   from the pinned publication, same freeze semantics).
2. Extend decoration to projection-declared code columns joining `cpg_ontology`
   dimensions; breadth default per reviewed PR-6 decision. Do not expose constraints yet.
3. Allowlist the dimension tables for plan validation; agents still reach
   vocabulary only through QRY forms and views (F-1).

**Legacy Disposition and Decommission.** Completes DB04: `cpg_base.enum_catalog`
address zero across `src/` and generated artifacts; serving joins target
`cpg_ontology.enum_domain`.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_ontology_namespace_resolution`; Executable oracle: `odf_decoration_projection_census`; Executable oracle: `odf_ontology_catalog_frozen_rejection`; Executable oracle: `odf_decoration_plan_shape`.

- **Behavioral — Executable oracle:** `odf_ontology_namespace_resolution` — every
  projection-declared code column resolves to its `<field>_name` through the
  dimension join; `relation_kind_code` resolves end-to-end.
- **Structural — Executable oracle:** `odf_decoration_projection_census` — the
  decoration set equals the generated projection declarations; runtime owns no
  table/view tuples.
- **Negative/Zero-State — Executable oracle:**
  `odf_ontology_catalog_frozen_rejection` — registration/deregistration on the
  ontology schema fails frozen; `rg` + ast-grep zero-hit for the old
  `enum_catalog` address outside history.
- **Operational — Executable oracle:** `odf_decoration_plan_shape` — EXPLAIN over
  an undecorated projection matches the PR-6-recorded expectation (joins
  eliminated, or absent by declaration).

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test`, `just query-determinism-check`,
`just semantic-query-relational-conformance-check`, `just packet-oracle-check WP10`.

**Integration Milestone.** M03.

**Replan Triggers.** PR-6 negative *and* the projection contracts require broad
decoration — decoration cost becomes a measured regression against the WP01
anchor: re-scope decoration defaults before accepting.

**Rollback or Recovery.** Revert by commit; WP09 tables remain valid unserved.

### WP11 — Executable-ontology publication gates

**Outcome.** `rule_contract` and `ontology_edge` compile through D-09's operation pass
into ordinary DataFusion `Expr`/`LogicalPlan` validation plans: governed-code and FK
anti-joins, ID-domain closure, relation family/cardinality/self-edge/owner conformance,
dimension PK uniqueness, and `property_fact` one-of coherence. The standing
`ontology-relational-closure-check` and extended integrity gate execute these plans; no
row-by-row bespoke Rust fallback duplicates a mechanically expressible rule.

**Dependencies.** WP09, WP10.

**Target invariants.** TI-1, TI-8, TI-9 (fingerprint-neutral).

**Design and library references.** Design §3.3 (executable ontology), §6 TI-1
proof; `src/fabric/publication.rs` FK machinery;
`just publication-referential-integrity-check`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'foreign_key|ReferenceViolation' src/fabric/publication.rs src/generated/table_specs.rs | head -20
rg -n 'value_kind_code' src/generated/table_specs.rs | head
```

Known touch: Contract-IR typed rule declarations, compiled-model/lowering code,
`src/fabric/publication.rs` (generated-plan execution), `justfile`
(`ontology-relational-closure-check`, `ontology-dimension-check`).

**Required changes.**

1. Express FK/code/ID-domain, membership/cardinality/owner/self-edge, PK uniqueness, and
   property one-of semantics as closed typed rule variants in `CompiledOntology`.
2. Lower relational variants using DataFusion joins, anti-joins, comparisons, aggregates,
   set/null kernels, and typed expressions; application Rust orchestrates plan execution
   and diagnostics only. A non-relational exception requires a typed algorithm variant and
   design revalidation; generated bespoke loops are not an automatic fallback.
3. Add `ontology-relational-closure-check`; keep `ontology-dimension-check` as the
   aggregate of parity + relational closure + decoration. Independently authored fixture
   expectations and seeded violations prove the compiled plans, not merely their digests.

**Legacy Disposition and Decommission.** None; additive gates.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_ontology_referential_zero`; Executable oracle: `odf_compiled_rule_contract_census`; Executable oracle: `odf_ontology_violation_rejection`; Executable oracle: `odf_property_value_one_of_gate`.

- **Behavioral — Executable oracle:** `odf_ontology_referential_zero` — on the
  populated fixture publication, every anti-join and conformance query returns
  zero rows and the recipe passes end-to-end.
- **Structural — Executable oracle:** `odf_compiled_rule_contract_census` — every governed
  code/FK/domain/membership/PK/one-of requirement has one typed rule contract, complete
  input/output/diagnostic lineage, and one DataFusion lowering; no duplicate Rust loop.
- **Negative/Zero-State — Executable oracle:** `odf_ontology_violation_rejection` —
  seeded violations (unknown code, disallowed family pair, self-edge violation)
  each fail publication with the existing violation error class.
- **Operational — Executable oracle:** `odf_property_value_one_of_gate` — seeded
  multi-populated and mispopulated value rows fail; conformant rows pass.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just publication-referential-integrity-check`,
`just ontology-relational-closure-check` (new), `just ontology-dimension-check` (new),
`just packet-oracle-check WP11`.

**Integration Milestone.** M03.

**Replan Triggers.** A required rule cannot be expressed by the closed typed relational
algebra and no existing typed algorithm variant fits — stop and extend D-09 through a
design-reviewed variant; do not silently generate bespoke Rust control flow.

**Rollback or Recovery.** Revert by commit.

### WP12 — Structure classification and the span decision

**Outcome.** The Contract IR carries logical structure classification per column group
independently of physical lowering. SourceSpan is always
`StructurallyOwnedCohesive`; the reviewed PR-3a/3b decision selects either a
presence-coherent nested struct or flat `Int64` columns with a compiled all-or-none
constraint. Candidate artifacts are updated, but WP17 alone activates Stage 2b.

**Dependencies.** WP02, WP04, WP09.

**Target invariants.** TI-4 (fingerprint-moving only on the struct branch).

**Design and library references.** Design §3.5 (D-03), §5.4 amendment 4; reviewed PR-3
decision transaction + observation report; `FAB §9`/`§65.4` text.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'start_byte|end_byte' src/generated/table_specs.rs | head -20
rg -n 'PR-3' docs/plans/state/codefabric-ontology-compiled-data-fabric_v2_state.json target/ontology-fabric-probes/ 2>/dev/null
```

Known touch: `contracts/schema/schema-contract-ir.json` (classification field +
span decision), model compiler, FAB §9/§65.4 text; struct branch additionally:
encoders, row shapes, `src/fact_ingest.rs` construction sites, republish.

**Required changes.**

1. Add logical structure classification to the IR; classify source spans
   `StructurallyOwnedCohesive`, evidence relational, and property values independently
   filterable. Physical lowering is a separate field.
2. Execute the reviewed physical-lowering branch; on the flat branch,
   emit the presence-coherence validation rule (all-or-none span columns) into
   batch validation.
3. Amend FAB §9/§65.4: criterion-based classification normative; span outcome
   recorded; evidence-as-table and flat tagged property values reaffirmed.

**Legacy Disposition and Decommission.** None deleted on the flat branch; on the
struct branch the flat span columns of affected tables are replaced in one
release (no dual shape).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_span_decision_conformance`; Executable oracle: `odf_logical_structure_classification_invariance`; Executable oracle: `odf_span_incoherence_rejection`; Executable oracle: `odf_span_pruning_parity`.

- **Behavioral — Executable oracle:** `odf_span_decision_conformance` — the landed
  shape matches the reviewed physical-lowering branch; round-trip gate passes on affected
  tables.
- **Structural — Executable oracle:** `odf_logical_structure_classification_invariance`
  — every group has one logical class and one lowering; SourceSpan remains cohesive in
  both branches. `just structure-classification-check` proves the invariant.
- **Negative/Zero-State — Executable oracle:** `odf_span_incoherence_rejection` —
  a partially-populated span (struct-null/child mismatch, or flat all-or-none
  violation) is rejected at batch validation.
- **Operational — Executable oracle:** `odf_span_pruning_parity` — file-scoped and
  span-filtered fixture queries match pre-change results and the PR-3b pruning
  expectation.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just structure-classification-check`, `just packet-oracle-check WP12`; struct branch adds
the WP07-style migration probe against the non-active candidate, not activation.

**Integration Milestone.** M03.

**Replan Triggers.** PR-3 passed but span-filtered serving regresses against the
WP01 anchor beyond the comparator bound — take the flat branch anyway; record the
override.

**Rollback or Recovery.** Revert candidate changes or discard candidate versions; the
active bundle is untouched until WP17.

### Stage 2b result-boundary candidate, then Stage 4–5 group

### WP13 — Generated result schemas and ResultChecksumV2

**Outcome.** Every query-form response Arrow schema is generated through the single
lowering (extension-typed IDs, metadata, deterministic order); packed-Binary ID
sequences become typed lists; computed projections re-annotate metadata;
`ResultChecksumV2` covers the richer schema with V1 KAT continuity; hand-written
result schemas reach zero.

**Dependencies.** WP05, WP17.

**Target invariants.** TI-3, TI-5; F-1, F-4 (fingerprint-neutral for stored
tables; result-surface change).

**Design and library references.** Design §3.6 (D-04); L-4 drop-map;
`src/semantic_query.rs:1550, 1802, 1932-2020`; `src/fabric/result_checksum.rs`;
`src/generated/model_query_forms.rs`; `contracts/query/query-form-contract.json`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'Field::new' src/semantic_query.rs
rg -n 'ordered_entity_ids|binding_entity_ids|witness_fact_ids' src/ codefabric-cpg-mcp/src/ contracts/
rg -n 'RESULT_CHECKSUM_VERSION' src/
```

Known touch: Schema Contract IR result-schema/result-field authorities, query-form
contract references + driver, `CompiledOntology`, `cpg_ontology.result_schema` /
`result_field` bundle rows, `src/generated/model_query_forms.rs`,
`src/semantic_query.rs`, `src/fabric/result_checksum.rs`, snapshot re-baselines
(confirm-gated `snapshots-accept` with reviewed diff), regenerated Python wire
artifacts (shape-neutral).

**Required changes.**

1. Define every result schema and ordered field in Schema Contract IR; every query-form/
   result-role binding references exactly one `result_schema_id`. Compile into
   `CompiledOntology`, update the two self-description relations, and emit schemas via the
   single lowering; typed `List` ID columns replace byte-packing.
2. Replace the three hand-written sites; re-annotate computed projections
   (`alias_with_metadata`) at the shaping seam.
3. Mint `ResultChecksumV2` (versioned, over the richer canonical schema); keep V1
   verifiable for released KATs; add V2 KATs and continuity assertions. Selection is
   result-schema-version gated so old leases continue V1/current shapes while new leases
   use V2/target shapes; the packet updates result-contract ontology rows and runtime
   selection as one governed result-boundary transaction.

**Legacy Disposition and Decommission.** Hand-written result schemas +
byte-packed ID columns → replace (DB03); zero-state + tier-1 deletion proof.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_generated_result_schema_conformance`; Executable oracle: `odf_result_schema_census`; Executable oracle: `odf_handwritten_result_schema_zero`; Executable oracle: `odf_result_checksum_v2_continuity`.

- **Behavioral — Executable oracle:** `odf_generated_result_schema_conformance` —
  delivered batches for all eight forms match the generated schemas including
  extension types and re-annotated metadata; JSON/protobuf wire output is
  unchanged for non-list fields and losslessly equivalent for retyped lists.
- **Structural — Executable oracle:** `odf_result_schema_census` — every form and
  result role has exactly one Schema Contract IR authority, compiled model entry,
  `cpg_ontology` row set, and generated schema; no result field is untyped or
  metadata-free where the contract declares otherwise.
- **Negative/Zero-State — Executable oracle:**
  `odf_handwritten_result_schema_zero` — ast-grep + `rg` zero-hit for `Field::new`
  in result-shaping modules; hand-written schema functions deleted; clean build.
- **Operational — Executable oracle:** `odf_result_checksum_v2_continuity` — V2
  KATs pass; V1 remains verifiable against released KATs; determinism gate green
  under V2.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just query-determinism-check`,
`just semantic-query-conformance-check`, `just query-form-contract-check`,
`just ontology-self-description-check`, `just packet-oracle-check WP13`.

**Integration Milestone.** M04.

**Replan Triggers.** Wire equivalence for retyped lists cannot be preserved
(adapter consumers depend on the packed form) — bounded compatibility is not
authorized; stop and reopen D-04's list retyping.

**Rollback or Recovery.** Revert by commit; V1 checksums remain authoritative
until V2 lands.

### WP14 — Typed control plane

**Outcome.** All 27 operational projections declare logical types in the Contract
IR and lower through the common path (domain-typed FSB IDs, `TimestampUtc`);
capture converts at the SQLite boundary; the same concept is never typed two ways
in the catalog.

**Dependencies.** WP17.

**Target invariants.** TI-3 (fingerprint-neutral for Delta tables; in-memory
surface change).

**Design and library references.** Design §3.7 (D-05); `src/schema_registry.rs:796-816`;
`src/fabric/serving.rs:1283-1400` capture.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'GeneratedOperationalColumn|OperationalSqliteType' src/generated/table_specs.rs src/schema_registry.rs | head
rg -n 'capture|MemTable' src/fabric/serving.rs | head -20
```

Known touch: Contract-IR operational projections, `src/schema_registry.rs`
(`build_operational`), `src/fabric/serving.rs` capture, regenerated artifacts.

**Required changes.**

1. Operational columns gain logical types (id16+domain for 16-byte IDs,
   `TimestampUtc` for instants); `build_operational` routes through the common
   lowering.
2. Capture converts blob→FSB at the boundary; conversion failures are capture
   errors (fail-closed), not silent Binary fallbacks.

**Legacy Disposition and Decommission.** Untyped operational lowering → reshape;
zero-state for `DataType::Binary` operational ID columns.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_control_plane_typed_capture`; Executable oracle: `odf_control_projection_type_census`; Executable oracle: `odf_untyped_control_id_zero`; Executable oracle: `odf_control_capture_equivalence`.

- **Behavioral — Executable oracle:** `odf_control_plane_typed_capture` — captured
  control tables carry domain-typed FSB IDs and typed timestamps; cross-namespace
  joins on `workspace_id` type-agree.
- **Structural — Executable oracle:** `odf_control_projection_type_census` — every
  operational column has a logical type; the lowering path census shows one path.
- **Negative/Zero-State — Executable oracle:** `odf_untyped_control_id_zero` — no
  operational ID column lowers to bare `Binary`; a seeded wrong-width blob fails
  capture.
- **Operational — Executable oracle:** `odf_control_capture_equivalence` — control
  view queries return value-equivalent results pre/post typing under the promoted
  serving oracles.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test`, `just model-repro-check`,
`just packet-oracle-check WP14`.

**Integration Milestone.** M04.

**Replan Triggers.** An operational writer emits non-16-byte IDs (discovered by
fail-closed capture) — that is an operational-store defect to fix first, not a
reason to widen the type.

**Rollback or Recovery.** Revert by commit; SQLite untouched.

### WP15 — Truthful statistics and constraints

**Outcome.** Overlay-present statistics compose per mutation class (row-count only, per
the design table); publication-validated PK constraints are exposed and classified
planner-consumed/application-validated/not DataFusion-enforced; `ScanArgs` statistics-
request handling remains explicitly declined; the overlay `Exact` pushdown claim has a
standing adversarial proof; PR-4's min/max scope lands only on a positive reviewed
decision.

**Dependencies.** WP01, WP02, WP11, WP17.

**Target invariants.** TI-6 (fingerprint-neutral).

**Design and library references.** Design §3.8 (D-06); `src/fabric/overlay.rs:773-782`;
`src/fabric/snapshot_catalog.rs` statistics posture + `authenticated_statistics`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'new_unknown|supports_filters_pushdown|statistics' src/fabric/overlay.rs src/fabric/snapshot_catalog.rs | head -20
rg -n 'Constraints' src/fabric.rs src/fabric/snapshot_catalog.rs
```

Known touch: `src/fabric/overlay.rs`, `src/fabric/snapshot_catalog.rs`,
`src/fabric.rs` (constraints surface).

**Required changes.**

1. Per-mutation-class statistics composition (design §3.8 table); never unknown,
   never falsely exact; column statistics never composed.
2. Surface PK `Constraints` only for keys whose WP11 uniqueness rule passed; classify
   planner consumption and state explicitly that application publication validation —
   not DataFusion — enforces them.
3. Keep statistics-request handling declined structurally; expose cheap truthful facts
   through ordinary provider/execution-plan statistics and `StatisticsContext` only.
4. Add the adversarial pushdown-truth test (overlay-path filtered execution vs
   engine-filtered reference).

**Legacy Disposition and Decommission.** `Statistics::new_unknown` overlay
degeneracy → replace; the untested `Exact` claim → proven or downgraded.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_overlay_statistics_composition`; Executable oracle: `odf_statistics_precision_census`; Executable oracle: `odf_pushdown_truth_falsification`; Executable oracle: `odf_constraints_classification_gate`.

- **Behavioral — Executable oracle:** `odf_overlay_statistics_composition` — each
  mutation class reports the design-table precision (`FullTableReplace →
  Exact(overlay)`, replaces/upserts → Inexact upper bound, base-void → Inexact
  overlay-only).
- **Structural — Executable oracle:** `odf_statistics_precision_census` — no
  provider path returns `new_unknown` when a manifest count exists; statistics-request
  handling remains declined and no custom request/response chain exists.
- **Negative/Zero-State — Executable oracle:** `odf_pushdown_truth_falsification`
  — adversarial filters through the overlay path match the engine-filtered
  reference exactly; a seeded lying claim is caught by the test harness.
- **Operational — Executable oracle:** `odf_constraints_classification_gate` — PK
  constraints are visible only after uniqueness proof; classification records actual
  planner consumption and application validation while denying DataFusion enforcement.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just provider-statistics-contract-check`,
`just root-test`, `just packet-oracle-check WP15`.

**Integration Milestone.** M04.

**Replan Triggers.** Composition changes join orders such that the determinism
gate's plan-identity leg fails — plan identity is parameter-neutral by contract;
if statistics legitimately alter plans, the determinism contract needs a recorded
adaptation, not a silent baseline move.

**Rollback or Recovery.** Revert by commit.

### Program-reconciliation group

### WP16 — Suite/index alignment and waves 9–12 reconciliation

**Outcome.** The five spec amendments are verified landed and internally
consistent across the suite; `docs/spec_index/` navigation reflects the new
tables/namespaces; the paused waves 8–12 plan's remaining packets are audited
against the post-amendment shape with dispositions recorded (A-3 mechanism,
Stage-2b exit criterion).

**Dependencies.** WP08, WP10, WP11, WP12.

**Target invariants.** TI-8.

**Design and library references.** Design §5.2 (waves mechanism), §5.4;
`docs/spec_index/{fact-domain-map,contract-census,wave-traceability}.md`;
`docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'cpg_ontology|codefabric\.id16|enum_catalog|UDTF' docs/upfront_design/ docs/spec_index/ -g '!docs/library_ref/**' | head -40
just spec-outline | head -30
```

Known touch: `docs/spec_index/*.md`, an `integrate-plan-audit`-produced review
artifact under `docs/reviews/`, waves-plan disposition notes (the paused plan file
itself is immutable; dispositions live in the audit artifact and the waves state
`plan_deviations`).

**Required changes.**

1. Verify amendment consistency suite-wide (no stale §-references to amended
   sections; spec-outline confirms new section shapes).
2. Update `docs/spec_index/` navigation (new tables, namespace, retired names) —
   derived navigation only, never normative.
3. Run the plan-audit lens over the remaining wave 9–12 packets; record per-packet
   dispositions (unaffected / needs-revision / superseded) in a review artifact;
   write the corresponding `plan_deviations` entry in the waves state file.

**Legacy Disposition and Decommission.** Stale spec/index references to retired
names reach zero.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_spec_amendment_census`; Executable oracle: `odf_spec_index_navigation_current`; Executable oracle: `odf_retired_name_reference_zero`; Executable oracle: `odf_waves_reconciliation_recorded`.

- **Behavioral — Executable oracle:** `odf_spec_amendment_census` — spec-outline
  assertions find the five amended sections in their post-amendment shape; the
  amended text references the ID-domain registry and `cpg_ontology`.
- **Structural — Executable oracle:** `odf_spec_index_navigation_current` — index
  rows for new tables/namespace resolve; `just artifacts-check` passes on the new
  review artifact.
- **Negative/Zero-State — Executable oracle:** `odf_retired_name_reference_zero` —
  `rg` zero-hit for `codefabric.id16` and the old `enum_catalog` address across
  `docs/upfront_design/` and `docs/spec_index/` (library_ref excluded as
  historical).
- **Operational — Executable oracle:** `odf_waves_reconciliation_recorded` — the
  waves state file validates under schema v2 with the interruption + disposition
  records present; every remaining packet has exactly one disposition.

**Edit-Local Gates.** `just typos` (docs surface).

**Packet-Local Gates.** `just artifacts-check`, `just plan-dependency-check`,
`just packet-oracle-check WP16`.

**Integration Milestone.** M03.

**Replan Triggers.** The remaining-packet audit finds a wave packet whose oracles
cannot be revised without reopening this design's decisions — escalate to design
reopening, not silent packet rewrites.

**Rollback or Recovery.** Docs/index edits revert by commit; state-file entries
are append-only records.

### WP17 — Stage 2b atomic activation and recursive self-description closure

**Outcome.** The complete Stage-2b candidate — all twenty ontology relations, serving
namespace/decoration, compiled relational gates, selected span lowering, and suite/waves
reconciliation — is validated as one unit. A catalog-only oracle dynamically discovers
and resolves the full plane, including a seeded new-domain fixture; fault injection proves
the old active pointer survives every pre-commit failure. One owner acceptance and one
manifest-pointer advance activate the candidate exactly once. No earlier packet may
perform either action.

**Dependencies.** WP09, WP10, WP11, WP12, WP16.

**Target invariants.** TI-1, TI-4, TI-8, TI-9 (fingerprint-moving Stage-2b activation).

**Design and library references.** Design §3.3 D-01, §3.11 D-09, §5.2 Stage 2b,
§6 TI-8/Stage-2b atomicity; existing candidate migration, publication manifest,
serving-snapshot lease, and active-pointer transaction machinery.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'candidate|owner_acceptance|active.*pointer|ServingSnapshot' src/fabric src/fabric.rs docs/plans/state/
rg -n 'BundleDimension|required_for_publication' src/generated/table_specs.rs
```

Known touch: candidate/activation orchestration, fault-point registry/fixtures,
`tests/integration/`, `justfile` (`ontology-self-description-check`,
`ontology-stage2b-activation-check`), execution-state decision/acceptance records. No
owner-specific registry or query-form authority changes in this packet.

**Required changes.**

1. Build one candidate dossier referencing every WP09–WP12 artifact and WP16 amendment/
   reconciliation digest. Reject missing, stale, or independently accepted components.
2. Implement catalog-only dynamic discovery: begin with the leased catalog and delivered
   result artifact, find `registry_authority`/`table_contract`, discover the remaining
   relations, and resolve all codes, edges, semantic types, table/column/current-result
   contracts, ID/identity recipes, phrase/rule bindings, snapshot, publication, and plan.
   The oracle contains no fixed twenty-table list or generated constants; a seeded new
   code domain and binding must be discovered without oracle code changes.
3. Run dimension parity/version stability, relational closure, decoration, span lowering,
   integrity, compatibility, and performance-comparator gates against the frozen candidate.
4. Inject failure before/after each candidate validation, acceptance-record write, and
   pointer-advance boundary; prove the prior pointer and all active leases remain valid.
5. After every gate passes, record one owner acceptance transaction and advance the
   active manifest pointer exactly once. Re-running is idempotent; ordinary fact
   publications with unchanged ontology inputs reuse the activated ontology versions.

**Legacy Disposition and Decommission.** Any WP09/WP12 provisional per-packet activation
or owner-acceptance path is forbidden and reaches structural zero. Candidate artifacts may
remain for audit/rollback under retention policy; they are not serving authorities.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_stage2b_recursive_self_description`; Executable oracle: `odf_stage2b_candidate_closure`; Executable oracle: `odf_stage2b_atomic_activation_fault_injection`; Executable oracle: `odf_dimension_version_stability_across_fact_publications`.

- **Behavioral — Executable oracle:** `odf_stage2b_recursive_self_description` — from a
  leased candidate catalog and delivered result artifact only, dynamically resolve every
  required authority/contract/provenance family; the seeded new-domain fixture resolves
  without oracle source edits.
- **Structural — Executable oracle:** `odf_stage2b_candidate_closure` — exactly one
  candidate dossier covers every WP09–WP12/WP16 proving digest; no earlier packet exposes
  an acceptance or active-pointer mutation path.
- **Negative/Zero-State — Executable oracle:**
  `odf_stage2b_atomic_activation_fault_injection` — every injected pre-commit failure
  leaves the previous active pointer and active leases unchanged; incomplete candidates
  cannot be accepted.
- **Operational — Executable oracle:**
  `odf_dimension_version_stability_across_fact_publications` — successful activation
  advances exactly once; retry is idempotent; two later fact publications with unchanged
  ontology inputs pin the same twenty Delta versions.

**Edit-Local Gates.** `just root-fmt`, `just root-check`, focused activation tests.

**Packet-Local Gates.** `just ontology-self-description-check`,
`just ontology-relational-closure-check`, `just ontology-stage2b-activation-check`,
`just ontology-dimension-check`, `just publication-referential-integrity-check`,
`just data-fabric-stack-compat`, `just packet-oracle-check WP17`.

**Integration Milestone.** M03.

**Replan Triggers.** A complete candidate cannot be validated without making it active;
the current activation transaction cannot preserve the old pointer on any injected
failure; or dynamic discovery requires generated constants — stop and reopen D-01/Stage
2b rather than partially activating.

**Rollback or Recovery.** Before pointer advance, discard candidate. After acceptance,
reactivate the prior manifest through the existing governed rollback transaction; never
rewrite or delete accepted history.

## 5. Integration milestones

### M01 — Evidence floor and single seam (Stage 0+1 complete)

Packets: WP01–WP06. Evidence: all six packet oracle quartets green at HEAD;
`odf_stage1_schema_fingerprint_equality` proves schema bytes unmoved;
`wave3-integration-check`, `query-determinism-check`,
`semantic-query-conformance-check`, `governance-scan` green; the WP05 behavior fix
is the only intentional behavioral delta (its dual-path oracle pins it).

### M02 — ID-domain universe live (Stage 2a complete)

Packets: WP07–WP08. Evidence: `id-domain-extension-check` green end-to-end;
`odf_id16_zero_state` at HEAD; cross-domain rejection oracles green; schema bundle
advanced exactly once with owner acceptance; `data-fabric-stack-compat` green;
perf differential vs the WP01 anchor within comparator bounds.

### M03 — Ontology plane standing (Stage 2b complete)

Packets: WP09–WP12, WP16–WP17. Evidence: `ontology-self-description-check`,
`ontology-relational-closure-check`, `ontology-stage2b-activation-check`,
`ontology-dimension-check`, and extended integrity gate green; dynamic new-domain
resolution passes; five amendments and waves reconciliation are complete; fault injection
proves the previous pointer; one owner acceptance and one pointer advance are recorded;
unchanged fact publications reuse all twenty ontology Delta versions.

### M04 — Self-describing fabric complete (plan completion)

Packets: WP13–WP15. Evidence: full final gate matrix (§7) green; the Stage-2b dynamic
self-description oracle is rerun after WP13's governed result-contract update and resolves
every target result schema/field as well as snapshot, publication, plan, code/edge/type/
table/column/identity/phrase/rule authority. No closing claim is inferred by composing
narrower tests.

## 6. Cross-packet decommission batches

### DB01 — Anonymous ID typing and path-local domain authority

Prerequisites: WP07 (retirement), WP08 (gate successor verified). Exit invariant:
`odf_id16_zero_state` green at HEAD across `src/`, `contracts/`,
`docs/upfront_design/`; `just id16-extension-contract-check` no longer resolves
(recipe removed); `just id-domain-extension-check` green. Binder-only/path-local domain
validators and direct extension-name policy switches also reach zero; every plan ingress
is covered by the installed analyzer rule and the binder delegates to its generated model.

### DB02 — Dual column lists and hand-written row shapes

Prerequisites: WP03, WP04. Exit invariant: `odf_dual_list_reconciliation_zero` and
`odf_handwritten_row_struct_zero` green at HEAD; `model-repro-check` green.

### DB03 — Hand-written result schemas and packed-Binary ID sequences

Prerequisites: WP13. Exit invariant: `odf_handwritten_result_schema_zero` green at
HEAD; wire-equivalence leg of `odf_generated_result_schema_conformance` green.

### DB04 — `cpg_base.enum_catalog` address

Prerequisites: WP09, WP10. Exit invariant: `odf_enum_catalog_relocation_zero` and
the WP10 address-zero leg green at HEAD.

### DB05 — Literal code predicates and unregistered phrase arms

Prerequisites: WP05. Exit invariant: both governance rules active in
`governance-scan` with zero findings; `odf_phrase_binding_dual_path_parity` green.

### DB06 — List-valued ontology memberships

Prerequisites: WP09, WP11. Exit invariant: `odf_nested_ontology_membership_zero` green;
no ontology table/compiled model contains list-valued family/member/owner/property
authority; every expected membership is a typed `ontology_edge` row and relational
closure produces independently expected semantics.

## 7. Final gate matrix

All rows are `just` recipes; new recipes are introduced by their packets
(`gate-filter-census` WP01, `probe-suite` WP02, `id-domain-extension-check` WP07,
`ontology-relational-closure-check` WP11, `structure-classification-check` WP12,
and ontology self-description/activation checks WP17).

| Gate | Scope |
|---|---|
| `just ci-fast` | four domains + governance aggregate |
| `just root-test` | nextest + doctests (never nextest alone) |
| `just wave3-integration-check` | fabric/publication slice |
| `just query-determinism-check` | plan identity + partition-independent checksums (V2) |
| `just semantic-query-conformance-check` | eight forms end-to-end |
| `just semantic-query-relational-conformance-check` | relational forms as native plans |
| `just query-form-contract-check` | Rust/Python form parity (+ model-repro-check) |
| `just query-legacy-zero-state-check` | no SQL path; LD-05 posture |
| `just id-domain-extension-check` | per-domain extension round-trips |
| `just ontology-relational-closure-check` | compiled DataFusion rule semantics + violations |
| `just ontology-dimension-check` | normalized parity + relational closure + decoration |
| `just structure-classification-check` | logical class invariant + selected lowering |
| `just ontology-self-description-check` | dynamic catalog-only recursive resolution |
| `just ontology-stage2b-activation-check` | complete-candidate fault atomicity + single advance |
| `just publication-referential-integrity-check` | FK closure incl. dimensions |
| `just provider-statistics-contract-check` | statistics + pushdown truth |
| `just data-fabric-stack-compat` | pinned-stack compatibility |
| `just rebuild-equivalence-check` | clean-rebuild fingerprint equality |
| `just model-repro-check` | `CompiledOntology` + generated families reproducible, zero writes |
| `just stable-graph-check` | exact pins/features |
| `just governance-scan` | structural rules incl. WP05 additions |
| `just artifacts-check` + `just plan-dependency-check` | artifact/plan contracts |
| `just gate-filter-census` | name-coupled recipe filter integrity |
| `just ci-pr` | full PR aggregate at plan completion |

Perf: `just data-fabric-upgrade-bench <WP01-anchor> <HEAD>` within comparator
bounds at M02, M03, M04.

## 8. Execution sequence

```text
WP01 → WP03 → {WP04, WP05, WP06} → [M01 after WP01–WP06]
{WP02, WP03, WP06} → WP07 → WP08 → [M02]
WP08 → WP09 → WP10 → WP11
{WP02, WP04, WP09} → WP12
{WP08, WP10, WP11, WP12} → WP16           (suite/waves reconciliation)
{WP09, WP10, WP11, WP12, WP16}
  → WP17                                  (sole Stage-2b activation)
  → [M03]
{WP05, WP17} → WP13
WP17 → WP14
{WP01, WP02, WP11, WP17} → WP15
{WP13, WP14, WP15} → [M04 — plan completion]
```

Parallelism: after WP03, WP04/WP05/WP06 may run in parallel where their preflight shows
disjoint files; WP03 owns shared compiled-model/schema-registry shapes and WP06 begins
only after that ownership closes. WP13/WP14/WP15 may run in parallel after M03 with
explicit shared-file coordination. Fingerprint-moving candidate packets WP07/WP09/WP12
and the WP17 activation never run concurrently with another packet.

## 9. Plan risks and replan policy

**Risks.**

1. **Name-coupled gate erosion** — mitigated by WP01's census + the standing
   filter-diff policy; any silent gate emptying is a plan defect.
2. **Fingerprint-moving cadence** — two governed releases: Stage 2a at WP07 and the
   complete Stage 2b candidate at WP17. WP09/WP12 build non-active candidate pieces;
   concurrent fingerprint movement and per-piece acceptance are prohibited.
3. **Probe decision integrity** — probes write observations only; reviewed state
   transactions select named branches and downstream packets fail on evidence/pin/config
   drift. WP12's branches alter physical lowering, never logical classification.
4. **Checksum/KAT continuity** — V1 stays verifiable until the arrow-58 KATs
   retire (outside this plan); re-baselining only via confirm-gated
   `snapshots-accept` with reviewed diffs.
5. **Paused-program drift** — waves 8–12 resumption without WP16's dispositions
   would re-introduce the superseded shape; M03 blocks on WP16.

**Replan policy.** Implementation adaptation (recorded in state): mechanism-level
substitutions that preserve packet outcomes and invariants — e.g. relocating the
analyzer implementation module without changing universal `SessionState` installation,
or keeping a single hand-written row
struct with a recorded exception (WP04). Plan revision (new plan version): packet
boundary or sequence changes — e.g. splitting WP07, PR-outcome combinations not
covered by a specified branch. Design reopening (back to the dossier): any change
to TI-1…TI-9, a library decision, the amendment set, or a probe outcome that
contradicts a design L-fact rather than selecting a named fallback.
