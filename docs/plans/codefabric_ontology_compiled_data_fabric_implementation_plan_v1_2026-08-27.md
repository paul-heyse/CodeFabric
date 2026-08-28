---
artifact: implementation-plan
plan_id: codefabric-ontology-compiled-data-fabric
version: v1
date: 2026-08-27
status: draft
design_path: docs/designs/codefabric_ontology_compiled_data_fabric_design_v2_2026-08-27.md
design_version: v2
baseline_commit: eebb958
working_tree_digest: 5b5d5567423da4fe4fcc1190659be3f69bbc6d7ab5995a2fc324fefea543de49
state_path: docs/plans/state/codefabric-ontology-compiled-data-fabric_v1_state.json
cutover: true
---

# CodeFabric ontology-compiled data fabric — implementation plan v1

Implements design v2 (accepted 2026-08-27 with assumptions A-1/A-2 probe-bound and
owner decisions A-3/A-4 resolved): the fabric becomes a fully ontology-compiled,
extension-typed, self-describing relational universe — a `cpg_ontology` dimension
plane, per-domain logical ID types with a domain-conformance plan rule, one logical
type system across all four served surfaces, criterion-classified structure, truthful
planning facts, a single generation seam, and a registry-complete semantic compiler.
The plan also carries the five `docs/upfront_design` amendments the design attaches to
its schema releases (design §5.4) and the reconciliation of the paused waves 8–12
program (design §5.2, owner decision A-3: this plan lands **before** the remaining
wave 9–12 scope executes).

## 1. Outcome and non-goals

### 1.1 Outcome

At plan completion:

1. `cpg_ontology` serves nine Delta-backed, bundle-digest-carrying dimension tables
   (six ontology registries, `enum_domain`, `provider_raw_kind`, `id_domain`);
   every `ontology:*`/`enum:*`/`opaque:provider-raw-kind` code column resolves
   in-catalog; registry-conformance anti-joins are standing publication gates (TI-1).
2. Every ID column carries its domain's extension type (`codefabric.<domain>_id`,
   FSB(16) over the Binary Delta seam; `hash32` → FSB(32)); all domains are
   registered in the serving session's extension-type registry; bound plans reject
   cross-domain joins and wrong-domain literals as typed errors; `codefabric.id16`
   is retired to zero (TI-2).
3. `cpg_base`, `cpg_control`, `cpg_serving`, and query-form result schemas all lower
   from one generated logical-type vocabulary through one lowering path; result
   batches carry extension types and re-annotated metadata; the packed-`Binary` ID
   sequences in path/pattern results are typed lists (TI-3, TI-5).
4. Structure classification per the `REP §17` criterion is contract data; the span
   decision executes per the recorded PR-3 probe outcome (TI-4).
5. Statistics compose per overlay mutation class (never unknown, never falsely
   exact); PK constraints are declared; pushdown truth is adversarially tested
   (TI-6).
6. One generated column authority per table; generated row shapes; zero hand-written
   schemas, row shapes, or literal codes between registry YAML and delivered batch;
   the phrase→certainty binding is registry-decided and behaviorally pinned on both
   query paths (TI-7; design §3.10 discovered defect fixed).
7. The suite amendments (FAB §6.3, §7, §8, §9/§65.4, §§78–82/93 + AC-G-20) are
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

Baseline `eebb958` with an intentionally dirty tree (in-flight wave 8 work;
`working_tree_digest` above). Pre-existing baseline failures: the session-context
gate baseline is red/stale at planning time; execution preflight re-runs
`just ci-fast` and records pre-existing failures per `validation-policy.md §3`
before any edit.

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
| docs/designs/codefabric_ontology_compiled_data_fabric_design_v2_2026-08-27.md | 6cac851baac54d65798973ab04cf53a0eca42c0d8b34a5ad671e5f84ea7d7c96 |
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

All library use in this plan traces to design v2 §3.11; decisions are restated here
only as execution bindings.

### LD-01 — DataFusion 55 extension-type registry

**Decision:** adopt
**Version basis:** DataFusion `=55.0.0` (`ExtensionTypeRegistry`,
`MemoryExtensionTypeRegistry`, `DFExtensionType`,
`SessionStateBuilder::with_extension_type_registry`; verified in the pinned
reference §4/S7.20–21).
**Displaces:** nothing; consumers are the WP08 domain-conformance rule, the storage
seam's field-aware cast, and diagnostics formatting.
**Risk:** claiming unsupplied behavior. Mitigated: only the three verified behaviors
are claimed; metadata classification names each consumer.
**Validation:** `odf_engine_registry_domain_census`; PR-1 probe record.

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
`DeltaScanConfig::with_schema` as absorber; fallback stands.
**Validation:** existing round-trip gate unchanged; `odf_id_domain_lowering_conformance`.

### LD-04 — FSB literals / joins / nested keys

**Decision:** adopt-if-proven (PR-1); fallback = storage-typed literal rewrite
(`src/fabric.rs:905-916`) stands.
**Validation:** PR-1 probe record in WP02; consumed by WP08/WP13.

### LD-05 — Recursive CTEs / UDTFs for traversal

**Decision:** reject (design §2.2, §3.11); `GraphOperatorPlan` + derived lane remains
the sole traversal path; FAB's UDTF recommendation struck by amendment 5 (WP08).
**Validation:** `just query-legacy-zero-state-check` continues green.

### LD-06 — MemTable for operational control projections

**Decision:** retain-current. **Validation:** existing catalog oracles; WP14.

### LD-07 — String execution posture (Utf8View / dictionary)

**Decision:** retain-current, probe-gated (PR-7); any adoption is session-config
only, never schema. **Validation:** PR-7 probe record in WP02; no packet in this
plan flips the config.

### 2.2 Design-principle posture

The design carries per-decision P1–P25 citations (design §3); this plan inherits
them through packet design references. The load-bearing postures: one authority per
concept (P3 — WP03/WP04/WP09), executable models (P2 — WP05/WP08/WP11), truthful
capability claims (P20/P21 — WP08/WP15), immutable snapshots (P11 — unchanged
machinery), provenance closure (P9/P10 — WP13's checksum versioning and the
self-description oracle at M04).

## 3. Global target invariants

TI-1 … TI-8 are defined in design v2 §2.3 and are referenced by ID throughout this
plan. Functionality contracts F-1 … F-6 (design §2.1) bind every packet: no packet
may regress the eight query forms, snapshot pinning, determinism, absence-as-unknown,
identity recipes, or provenance closure. Two plan-wide standing rules:

- **Fingerprint discipline.** Packets marked *fingerprint-neutral* prove schema-byte
  equality (`AC-G-79` comparator) at their proving commit; packets marked
  *fingerprint-moving* (WP07, WP09, WP12) are governed `EXACT_PIN` release events
  with migration probes and recorded owner acceptance.
- **Gate hygiene.** Any packet that renames or relocates tests ships the
  filter-expression diff for the name-coupled recipes (WP01 policy).

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
   workload at the pre-change commit and record the anchor ref in the benchmark
   comparator contract fixture.
3. Add a `scripts/`-side check that renders the current `nextest` filter
   expressions of the name-coupled recipes to a committed manifest, so a later diff
   is mechanical; wire as recipe `gate-filter-census`.

**Legacy Disposition and Decommission.** None; purely additive protection.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_promoted_fabric_oracles_green`; Executable oracle: `odf_gate_filter_census_current`; Executable oracle: `odf_gate_empty_selection_rejection`; Executable oracle: `odf_perf_baseline_anchor_captured`.

- **Behavioral — Executable oracle:** `odf_promoted_fabric_oracles_green` — the
  promoted integration copies pass against the unmodified tree and select > 0 tests
  per recipe filter.
- **Structural — Executable oracle:** `odf_gate_filter_census_current` — the
  committed filter-expression manifest matches the live `justfile` extraction.
- **Negative/Zero-State — Executable oracle:** `odf_gate_empty_selection_rejection`
  — every name-coupled recipe runs with `--no-tests=fail` semantics; a synthetic
  rename in a scratch worktree makes the census check fail.
- **Operational — Executable oracle:** `odf_perf_baseline_anchor_captured` — the
  anchor ref resolves, the comparator contract validates, and a re-run reproduces
  medians within the contract's bootstrap interval.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just root-test`, `just wave3-integration-check`,
`just packet-oracle-check WP01`.

**Integration Milestone.** M01.

**Replan Triggers.** A protective oracle cannot run outside its home module without
widening visibility — split that oracle's promotion into the packet that edits its
home file, and record the exception.

**Rollback or Recovery.** Additive; revert by commit.

### WP02 — Probe suite PR-1…PR-7

**Outcome.** All seven design probes exist as executable tests at the pinned
versions, their outcomes are recorded in a committed probe-record fixture with the
bound decision and fallback named, and downstream packets consume the record rather
than re-deriving library facts.

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

Known touch: new `tests/integration/` probe module(s), one committed probe-record
fixture under `tests/fixtures/` (first use of that directory per its named
trigger: reusable non-code data), a `probe-suite` recipe.

**Required changes.**

1. PR-1 `ScalarValue::FixedSizeBinary` literal/IN-list/join/group-by; PR-2
   `DeltaScanConfig::with_schema` FSB presentation; PR-3a struct `{Int64,Int64}`
   Delta round-trip; PR-3b span pruning under production session config; PR-4
   Delta file-statistics exposure; PR-5 Parquet `ARROW:schema` metadata round-trip
   with per-domain names; PR-6 unused-left-join elimination (with and without PK
   constraints); PR-7 view-types-enabled execution correctness.
2. Each probe writes a structured outcome row (probe id, verdict, evidence
   command, bound decision, fallback-selected flag) into the probe record.

**Legacy Disposition and Decommission.** None.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_probe_suite_outcomes_recorded`; Executable oracle: `odf_probe_pin_identity`; Executable oracle: `odf_probe_fallback_binding_census`; Executable oracle: `odf_probe_rerun_determinism`.

- **Behavioral — Executable oracle:** `odf_probe_suite_outcomes_recorded` — all
  seven probes execute and the record fixture contains exactly seven verdict rows.
- **Structural — Executable oracle:** `odf_probe_pin_identity` — probes assert the
  resolved datafusion/arrow/deltalake identities equal the pinned baseline before
  recording a verdict.
- **Negative/Zero-State — Executable oracle:** `odf_probe_fallback_binding_census`
  — every negative verdict names its design fallback; an unbound negative verdict
  fails the census.
- **Operational — Executable oracle:** `odf_probe_rerun_determinism` — a second run
  reproduces identical verdicts (no flaky probe enters the record).

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

**Outcome.** The model compiler emits one merged generated column shape per table
(type, nullability, semantic type, FK, domain, structure class, hidden-operational,
field id); `schema_registry` lowers from it alone; the `MODEL_TABLES` /
`GENERATED_TABLE_SPECS[*].columns` dual emission and the runtime by-name
reconciliation are gone; schema bytes are proven unmoved.

**Dependencies.** WP01.

**Target invariants.** TI-3, TI-7 (fingerprint-neutral).

**Design and library references.** Design §3.9 (D-06 in v1 numbering; v2 §3.9
D-07); current seam `src/schema_registry.rs:561-583` (`model_field`),
`src/generated/model_schema_tables.rs`, `src/generated/table_specs.rs`,
`src/bin/codefabric_model/schema_driver.rs`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'MODEL_TABLES|ModelColumn|GeneratedColumn|model_field' src/schema_registry.rs src/generated/ src/bin/codefabric_model/schema_driver.rs
rg -ln 'model_schema_tables|table_specs::' src/
```

Known touch: `src/bin/codefabric_model/schema_driver.rs`, regenerated
`src/generated/{model_schema_tables,table_specs}.rs` (or their merged successor),
`src/schema_registry.rs`.

**Required changes.**

1. Merge the two generated column shapes into one emission; keep the generated
   file/module layout decision local to the driver (planning does not fix file
   names).
2. Point every `schema_registry` lowering read at the merged shape; delete
   `model_field`'s dual-list reconciliation.
3. Re-run model generation twice (`model-repro-check` discipline) and prove
   schema-byte equality against the pre-packet snapshot.

**Legacy Disposition and Decommission.** `model_field` reconciliation → delete
(DB02). The superseded generated symbols reach zero within this packet (tier-1
clean build after deletion).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_single_column_authority_parity`; Executable oracle: `odf_merged_column_shape_census`; Executable oracle: `odf_dual_list_reconciliation_zero`; Executable oracle: `odf_stage1_schema_fingerprint_equality`.

- **Behavioral — Executable oracle:** `odf_single_column_authority_parity` — every
  table's lowered Arrow schema is byte-identical (fields, metadata, order) to the
  pre-packet golden capture.
- **Structural — Executable oracle:** `odf_merged_column_shape_census` — the merged
  shape carries all fields both legacy shapes carried; no consumer reads the legacy
  symbols.
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
phrase→predicate bindings are registry-driven (generated binding rows) or
conformance-tied to phrase-registry IDs; the divergent certainty sets (relational
`{10,20}` vs graph `{10,20,30,50}`) are unified to the registry-decided set (owner
decision A-4) with the behavior change pinned by oracle on both paths.

**Dependencies.** WP01.

**Target invariants.** TI-7; F-1 (fingerprint-neutral; one recorded behavior fix).

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
2. Generate binding rows/constants; replace both hand-written sites; delete the
   divergent literals.
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

**Outcome.** The serving session is built through `SessionStateBuilder` with an
installed (initially empty) `MemoryExtensionTypeRegistry`; the five scattered
extension-validation call sites collapse into one shared `schema_registry` helper;
serving behavior is proven unchanged.

**Dependencies.** WP01.

**Target invariants.** TI-2 (preparation; fingerprint-neutral).

**Design and library references.** Design §3.4 moves 1–2; LD-01;
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

1. Build session state via `SessionStateBuilder::with_default_features()` +
   config + runtime + `with_extension_type_registry` (empty registry);
   `SessionContext::new_with_state`.
2. One `schema_registry`-owned validation helper; call sites delegate.

**Legacy Disposition and Decommission.** Scattered validation idioms → reshape into
the helper; zero-state for direct `has_valid_extension_type::<Id16Extension>` calls
outside the helper.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_session_builder_equivalence`; Executable oracle: `odf_extension_registry_installed`; Executable oracle: `odf_scattered_extension_check_zero`; Executable oracle: `odf_serving_equivalence_post_reshape`.

- **Behavioral — Executable oracle:** `odf_session_builder_equivalence` — plans and
  results for the conformance corpus are identical before/after the session
  reshape.
- **Structural — Executable oracle:** `odf_extension_registry_installed` — the
  serving session state exposes the installed registry (empty at this packet).
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
per-domain Arrow `ExtensionType` impls; the single lowering attaches each ID
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
2. Generate per-domain `ExtensionType` impls + the domain table consumed by WP09's
   `id_domain` dimension; lowering attaches domain types; storage seam
   (`Id16ContractProvider` generalized) re-presents Binary as the domain-typed FSB
   schema; filter-literal rewrite per PR-1 outcome.
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

**Outcome.** All ID domains are registered in the serving session's extension-type
registry; `BoundPlanSpec` validation rejects cross-domain join keys, literals, and
IN-lists as typed plan errors; the three claimed engine behaviors have named,
tested consumers; amendment 5 (FAB UDTF strike + AC-G-20 example update) lands.

**Dependencies.** WP07.

**Target invariants.** TI-2; F-1 (fingerprint-neutral).

**Design and library references.** Design §3.4 (domain-conformance rule), §5.4
amendment 5; LD-01, LD-05; `src/semantic_query.rs` bind stage;
`src/fabric/serving.rs` session build (WP06 seam).

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'bind_request|BoundQueryBlock|validate' src/semantic_query.rs | head -20
rg -n 'cpg_neighbors|cpg_reachable|UDTF' docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | head
```

Known touch: `src/fabric/serving.rs` (registry population),
`src/semantic_query.rs` (bind-stage rule), FAB §§78–82/93 + `AC-G-20` text,
metadata classification dictionary (consumers).

**Required changes.**

1. Populate the WP06 registry with every generated domain registration.
2. Bind-stage domain-conformance rule: join keys, literals, IN-lists must agree on
   ID domain (resolved via the session registry + generated domain table);
   mismatch → typed `SemanticQueryError` before physical planning.
3. Name and test the three engine-behavior consumers (rule resolution, seam cast,
   diagnostics formatting); update the metadata classification dictionary.
4. Amend FAB §§78–82/93 (UDTF recommendation struck; `GraphOperatorPlan` + derived
   lane canonical) and the `AC-G-20` extension example to the ID-domain registry.

**Legacy Disposition and Decommission.** None new; completes DB01's gate rename
verification at HEAD.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_domain_conformant_plans_execute`; Executable oracle: `odf_engine_registry_domain_census`; Executable oracle: `odf_cross_domain_plan_rejection`; Executable oracle: `odf_extension_consumer_classification`.

- **Behavioral — Executable oracle:** `odf_domain_conformant_plans_execute` — the
  conformance corpus (same-domain joins/filters across all eight forms) executes
  unchanged; diagnostics render domain-typed ID literals via the registry.
- **Structural — Executable oracle:** `odf_engine_registry_domain_census` — session
  registry contents equal the generated ID-domain registry exactly.
- **Negative/Zero-State — Executable oracle:** `odf_cross_domain_plan_rejection` —
  cross-domain join, wrong-domain literal, and mixed-domain IN-list each yield the
  typed rejection; `rg` zero-hit for UDTF recommendations in amended FAB sections.
- **Operational — Executable oracle:** `odf_extension_consumer_classification` —
  the metadata classification dictionary names a consumer for every claimed engine
  behavior; the classification census validates.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just semantic-query-conformance-check`,
`just query-determinism-check`, `just id-domain-extension-check`,
`just packet-oracle-check WP08`.

**Integration Milestone.** M02.

**Replan Triggers.** Registry resolution is unavailable at bind time without
holding session state where the binder runs — if the rule cannot live at bind
stage, relocating it to plan validation is implementation adaptation; dropping it
is design reopening.

**Rollback or Recovery.** Revert by commit; registrations are session-scoped.

### Stage 2b group — the ontology plane (design §5.2 Stage 2b; fingerprint-moving)

### WP09 — Ontology dimension tables and registry builders

**Outcome.** Nine Delta-backed `BundleDimension` dimension tables exist under the
Contract IR (six ontology registries, `enum_domain` relocation, `provider_raw_kind`,
`id_domain`), populated at publication from generated builders with registry bundle
digests; FAB §6.3 and §8 are amended; the 2b schema release completes.

**Dependencies.** WP07.

**Target invariants.** TI-1, TI-8 (fingerprint-moving release).

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

Known touch: `contracts/schema/schema-contract-ir.json` (nine dimension tables +
relocation), `src/bin/codefabric_model/` (dimension driver), `src/fabric.rs`
(generalized builders), FAB §6.3/§8 text, regenerated artifacts + bundles.

**Required changes.**

1. Contract-IR revision: nine dimension tables with the design §3.3 column sets,
   `BundleDimension` + `BaseImmutable`, `version` + `canonical_digest` columns;
   `enum_catalog` relocates to `cpg_ontology.enum_domain` (same grain).
2. Dimension driver renders registry YAML / raw-kind JSON / ID-domain registry
   rows into generated batch builders; `src/fabric.rs` population generalizes from
   the enum-catalog special case.
3. Amend FAB §6.3 (add `cpg_ontology`; relocation) and FAB §8 (dimension serving
   generalized to all governed vocabularies).
4. `EXACT_PIN` release: migration probe, workspace republish, owner acceptance.

**Legacy Disposition and Decommission.** `cpg_base.enum_catalog` address → replace
(DB04): the table identity moves; consumers (serving decoration, allowlist) update
in WP10; the old address reaches zero at WP10's proving commit.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_dimension_registry_parity`; Executable oracle: `odf_dimension_table_spec_census`; Executable oracle: `odf_enum_catalog_relocation_zero`; Executable oracle: `odf_dimension_publication_pinning`.

- **Behavioral — Executable oracle:** `odf_dimension_registry_parity` — dimension
  rows equal registry YAML rows (codes, names, semantic columns) and generated
  Rust constants; digests match the registry bundle.
- **Structural — Executable oracle:** `odf_dimension_table_spec_census` — nine
  tables with the declared axes; every `ontology:*`/`enum:*`/raw-kind semantic
  type binds to exactly one dimension table.
- **Negative/Zero-State — Executable oracle:** `odf_enum_catalog_relocation_zero` —
  the Contract IR contains no `cpg_base.enum_catalog`; generation is clean;
  (address-consumer zero completes in WP10).
- **Operational — Executable oracle:** `odf_dimension_publication_pinning` — a
  fixture publication pins all nine dimension tables in its manifest; vacuum
  respects them; the migration probe and republish pass with owner acceptance.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just publication-referential-integrity-check`, `just packet-oracle-check WP09`.

**Integration Milestone.** M03.

**Replan Triggers.** A registry carries rows the IR's closed column model cannot
express (schema-contract compiler rejection) — extend the IR model first; do not
truncate registry semantics to fit.

**Rollback or Recovery.** Prior schema bundle activatable until acceptance.

### WP10 — `cpg_ontology` serving namespace and decoration

**Outcome.** The frozen catalog serves `cpg_ontology`; the plan allowlist admits
its tables; serving-view decoration extends to `ontology:*` and raw-kind codes per
projection declarations; dimension PKs are declared as constraints; decoration
breadth follows the recorded PR-6 outcome.

**Dependencies.** WP09.

**Target invariants.** TI-1, TI-8 (fingerprint-neutral beyond WP09's release).

**Design and library references.** Design §3.3 (serving decoration), §3.12
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
   dimensions; declare dimension PK `Constraints`; breadth default per PR-6.
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

**Outcome.** Publication validation proves the ontology: dimension FK contracts
(anti-join zero) for every fact code column, relation allowed-family/cardinality
conformance derived from `relation_kind` rows, and `property_fact` one-of value
coherence; all standing as the `ontology-dimension-check` recipe plus the extended
integrity gate.

**Dependencies.** WP09, WP10.

**Target invariants.** TI-1 (fingerprint-neutral).

**Design and library references.** Design §3.3 (executable ontology), §6 TI-1
proof; `src/fabric/publication.rs` FK machinery;
`just publication-referential-integrity-check`.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'foreign_key|ReferenceViolation' src/fabric/publication.rs src/generated/table_specs.rs | head -20
rg -n 'value_kind_code' src/generated/table_specs.rs | head
```

Known touch: Contract-IR FK/conformance declarations, `src/fabric/publication.rs`
(generated-check consumption), `justfile` (`ontology-dimension-check`).

**Required changes.**

1. Generated FK contracts: fact code columns → dimension tables.
2. Generated conformance checks from dimension semantic columns (allowed
   subject/object families, cardinality, self-edge policy) and the one-of value
   coherence rule.
3. `ontology-dimension-check` recipe: parity + referential zero + conformance +
   decoration legs (design §6 TI-1).

**Legacy Disposition and Decommission.** None; additive gates.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_ontology_referential_zero`; Executable oracle: `odf_dimension_fk_contract_census`; Executable oracle: `odf_ontology_violation_rejection`; Executable oracle: `odf_property_value_one_of_gate`.

- **Behavioral — Executable oracle:** `odf_ontology_referential_zero` — on the
  populated fixture publication, every anti-join and conformance query returns
  zero rows and the recipe passes end-to-end.
- **Structural — Executable oracle:** `odf_dimension_fk_contract_census` — every
  `ontology:*`/`enum:*`/raw-kind code column carries a generated FK contract; none
  is uncovered.
- **Negative/Zero-State — Executable oracle:** `odf_ontology_violation_rejection` —
  seeded violations (unknown code, disallowed family pair, self-edge violation)
  each fail publication with the existing violation error class.
- **Operational — Executable oracle:** `odf_property_value_one_of_gate` — seeded
  multi-populated and mispopulated value rows fail; conformant rows pass.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just publication-referential-integrity-check`,
`just ontology-dimension-check` (new), `just packet-oracle-check WP11`.

**Integration Milestone.** M03.

**Replan Triggers.** A conformance rule cannot be expressed over dimension rows
without new query machinery — keep it a generated Rust check (same authority)
rather than inventing plan machinery; record the adaptation.

**Rollback or Recovery.** Revert by commit.

### WP12 — Structure classification and the span decision

**Outcome.** The Contract IR carries a structure classification per column group
(the `REP §17` criterion as contract data); the span decision executes per the
recorded PR-3a/3b outcomes — either the presence-coherent span struct (with
regenerated encoders/tables and republish) or flat columns with the
relational-by-constraint classification and the presence-coherence validation
gate; FAB §9/§65.4 amended accordingly.

**Dependencies.** WP02, WP04, WP09.

**Target invariants.** TI-4 (fingerprint-moving only on the struct branch).

**Design and library references.** Design §3.5 (D-03), §5.4 amendment 4; PR-3
probe record; `FAB §9`/`§65.4` text.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'start_byte|end_byte' src/generated/table_specs.rs | head -20
jq '.probes[] | select(.id | startswith("PR-3"))' tests/fixtures/*probe*.json 2>/dev/null || rg -n 'PR-3' tests/fixtures/ -l
```

Known touch: `contracts/schema/schema-contract-ir.json` (classification field +
span decision), model compiler, FAB §9/§65.4 text; struct branch additionally:
encoders, row shapes, `src/fact_ingest.rs` construction sites, republish.

**Required changes.**

1. Add the structure-classification field to the IR column-group model; classify
   all current groups with recorded criteria.
2. Execute the span decision on the recorded probe branch; on the flat branch,
   emit the presence-coherence validation rule (all-or-none span columns) into
   batch validation.
3. Amend FAB §9/§65.4: criterion-based classification normative; span outcome
   recorded; evidence-as-table and flat tagged property values reaffirmed.

**Legacy Disposition and Decommission.** None deleted on the flat branch; on the
struct branch the flat span columns of affected tables are replaced in one
release (no dual shape).

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_span_decision_conformance`; Executable oracle: `odf_structure_classification_census`; Executable oracle: `odf_span_incoherence_rejection`; Executable oracle: `odf_span_pruning_parity`.

- **Behavioral — Executable oracle:** `odf_span_decision_conformance` — the landed
  shape matches the probe-recorded branch; round-trip gate passes on affected
  tables.
- **Structural — Executable oracle:** `odf_structure_classification_census` — every
  column group carries exactly one classification with a recorded criterion.
- **Negative/Zero-State — Executable oracle:** `odf_span_incoherence_rejection` —
  a partially-populated span (struct-null/child mismatch, or flat all-or-none
  violation) is rejected at batch validation.
- **Operational — Executable oracle:** `odf_span_pruning_parity` — file-scoped and
  span-filtered fixture queries match pre-change results and the PR-3b pruning
  expectation.

**Edit-Local Gates.** `just root-fmt`, `just root-check`.

**Packet-Local Gates.** `just model-repro-check`, `just root-test`,
`just packet-oracle-check WP12`; struct branch adds the WP07-style migration
probe + republish.

**Integration Milestone.** M03.

**Replan Triggers.** PR-3 passed but span-filtered serving regresses against the
WP01 anchor beyond the comparator bound — take the flat branch anyway; record the
override.

**Rollback or Recovery.** Flat branch: revert by commit. Struct branch: prior
bundle activatable until acceptance.

### Stage 3–5 group — result boundary, control plane, planning facts

### WP13 — Generated result schemas and ResultChecksumV2

**Outcome.** Every query-form response Arrow schema is generated through the single
lowering (extension-typed IDs, metadata, deterministic order); packed-Binary ID
sequences become typed lists; computed projections re-annotate metadata;
`ResultChecksumV2` covers the richer schema with V1 KAT continuity; hand-written
result schemas reach zero.

**Dependencies.** WP05, WP07.

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

Known touch: query-form contract + driver, `src/generated/model_query_forms.rs`,
`src/semantic_query.rs`, `src/fabric/result_checksum.rs`, snapshot re-baselines
(confirm-gated `snapshots-accept` with reviewed diff), regenerated Python wire
artifacts (shape-neutral).

**Required changes.**

1. Extend the query-form driver to emit per-form/per-role result schemas via the
   single lowering; typed `List` ID columns replace byte-packing.
2. Replace the three hand-written sites; re-annotate computed projections
   (`alias_with_metadata`) at the shaping seam.
3. Mint `ResultChecksumV2` (versioned, over the richer canonical schema); keep V1
   verifiable for released KATs; add V2 KATs and continuity assertions.

**Legacy Disposition and Decommission.** Hand-written result schemas +
byte-packed ID columns → replace (DB03); zero-state + tier-1 deletion proof.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_generated_result_schema_conformance`; Executable oracle: `odf_result_schema_census`; Executable oracle: `odf_handwritten_result_schema_zero`; Executable oracle: `odf_result_checksum_v2_continuity`.

- **Behavioral — Executable oracle:** `odf_generated_result_schema_conformance` —
  delivered batches for all eight forms match the generated schemas including
  extension types and re-annotated metadata; JSON/protobuf wire output is
  unchanged for non-list fields and losslessly equivalent for retyped lists.
- **Structural — Executable oracle:** `odf_result_schema_census` — every form and
  result role has exactly one generated schema; no result field is untyped or
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
`just packet-oracle-check WP13`.

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

**Dependencies.** WP07.

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

**Outcome.** Overlay-present statistics compose per mutation class (row-count
only, per the design table); PK constraints are declared (advisory-classified);
`ScanArgs` statistics requests are answered only where cheap-truthful; the overlay
`Exact` pushdown claim has a standing adversarial proof; PR-4's min/max scope
lands only on a positive record.

**Dependencies.** WP01, WP02.

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
2. PK `Constraints` on wrapped providers, advisory-classified in the metadata
   dictionary.
3. Statistics-request posture extended through the overlay wrapper; adversarial
   pushdown-truth test (overlay-path filtered execution vs engine-filtered
   reference).

**Legacy Disposition and Decommission.** `Statistics::new_unknown` overlay
degeneracy → replace; the untested `Exact` claim → proven or downgraded.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `odf_overlay_statistics_composition`; Executable oracle: `odf_statistics_precision_census`; Executable oracle: `odf_pushdown_truth_falsification`; Executable oracle: `odf_constraints_classification_gate`.

- **Behavioral — Executable oracle:** `odf_overlay_statistics_composition` — each
  mutation class reports the design-table precision (`FullTableReplace →
  Exact(overlay)`, replaces/upserts → Inexact upper bound, base-void → Inexact
  overlay-only).
- **Structural — Executable oracle:** `odf_statistics_precision_census` — no
  provider path returns `new_unknown` when a manifest count exists; requests
  beyond cheap-truthful are explicitly ignored.
- **Negative/Zero-State — Executable oracle:** `odf_pushdown_truth_falsification`
  — adversarial filters through the overlay path match the engine-filtered
  reference exactly; a seeded lying claim is caught by the test harness.
- **Operational — Executable oracle:** `odf_constraints_classification_gate` — PK
  constraints visible on wrapped providers; classification dictionary marks them
  advisory with the named future consumer.

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

Packets: WP09–WP12, WP16. Evidence: `ontology-dimension-check` and extended
`publication-referential-integrity-check` green; decoration resolution end-to-end;
five amendments landed and censused; waves 9–12 reconciliation recorded; the
design §6 TI-8 self-description oracle criteria for vocabulary resolution hold.

### M04 — Self-describing fabric complete (plan completion)

Packets: WP13–WP15. Evidence: full final gate matrix (§7) green; the
self-description oracle — from a leased catalog and a delivered result artifact,
resolve snapshot, publication, plan identity, every code name, every ID domain,
and every table contract version via queries and artifact records only — passes as
the plan's closing behavioral proof (folded into `odf_generated_result_schema_conformance`
+ `ontology-dimension-check` composition at this milestone).

## 6. Cross-packet decommission batches

### DB01 — `codefabric.id16` and `Id16Extension`

Prerequisites: WP07 (retirement), WP08 (gate successor verified). Exit invariant:
`odf_id16_zero_state` green at HEAD across `src/`, `contracts/`,
`docs/upfront_design/`; `just id16-extension-contract-check` no longer resolves
(recipe removed); `just id-domain-extension-check` green.

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

## 7. Final gate matrix

All rows are `just` recipes; new recipes are introduced by their packets
(`gate-filter-census` WP01, `probe-suite` WP02, `id-domain-extension-check` WP07,
`ontology-dimension-check` WP11).

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
| `just ontology-dimension-check` | parity + referential + conformance + decoration |
| `just publication-referential-integrity-check` | FK closure incl. dimensions |
| `just provider-statistics-contract-check` | statistics + pushdown truth |
| `just data-fabric-stack-compat` | pinned-stack compatibility |
| `just rebuild-equivalence-check` | clean-rebuild fingerprint equality |
| `just model-repro-check` | dual-generation reproducibility, zero worktree writes |
| `just stable-graph-check` | exact pins/features |
| `just governance-scan` | structural rules incl. WP05 additions |
| `just artifacts-check` + `just plan-dependency-check` | artifact/plan contracts |
| `just gate-filter-census` | name-coupled recipe filter integrity |
| `just ci-pr` | full PR aggregate at plan completion |

Perf: `just data-fabric-upgrade-bench <WP01-anchor> <HEAD>` within comparator
bounds at M02, M03, M04.

## 8. Execution sequence

```text
WP01 ─┬─▶ WP03 ─▶ WP04 ─────────────┐
      ├─▶ WP05 ──────────────┐      │
      ├─▶ WP06 ─────┐        │      │
WP02 ─┴────────┐    │        │      │
               ▼    ▼        │      │
              [M01 after WP01–WP06] │
                    │        │      │
        WP07 (needs WP02,WP03,WP06) │
                    ▼        │      │
                  WP08       │      │
                    ▼        ▼      ▼
              [M02 after WP07–WP08]
                    │
        WP09 ─▶ WP10 ─▶ WP11
          │               │
          └──▶ WP12 (needs WP02, WP04, WP09)
                    │
                  WP16 (needs WP08, WP10, WP11, WP12)
                    ▼
              [M03 after WP09–WP12, WP16]
                    │
        WP13 (needs WP05, WP07)   WP14 (needs WP07)   WP15 (needs WP01, WP02)
                    ▼
              [M04 after WP13–WP15 — plan completion]
```

Parallelism: WP03/WP05/WP06 may run in parallel after WP01 (disjoint files except
`schema_registry.rs`, which WP03 owns — WP06's helper lands in a WP03-coordinated
merge); WP13/WP14/WP15 are parallel after M03. Fingerprint-moving packets (WP07,
WP09, WP12-struct-branch) never run concurrently with any other packet.

## 9. Plan risks and replan policy

**Risks.**

1. **Name-coupled gate erosion** — mitigated by WP01's census + the standing
   filter-diff policy; any silent gate emptying is a plan defect.
2. **Fingerprint-moving cadence** — three governed releases (WP07, WP09,
   WP12-conditional); each with migration probe + owner acceptance; concurrent
   fingerprint movement prohibited.
3. **Probe-outcome branching (WP12)** — both branches fully specified; the branch
   decision is recorded state, not re-litigated at execution.
4. **Checksum/KAT continuity** — V1 stays verifiable until the arrow-58 KATs
   retire (outside this plan); re-baselining only via confirm-gated
   `snapshots-accept` with reviewed diffs.
5. **Paused-program drift** — waves 8–12 resumption without WP16's dispositions
   would re-introduce the superseded shape; M03 blocks on WP16.

**Replan policy.** Implementation adaptation (recorded in state): mechanism-level
substitutions that preserve packet outcomes and invariants — e.g. relocating the
domain rule within plan validation (WP08), keeping a single hand-written row
struct with a recorded exception (WP04). Plan revision (new plan version): packet
boundary or sequence changes — e.g. splitting WP07, PR-outcome combinations not
covered by a specified branch. Design reopening (back to the dossier): any change
to TI-1…TI-8, a library decision, the amendment set, or a probe outcome that
contradicts a design L-fact rather than selecting a named fallback.
