---
artifact: design-dossier
design_id: codefabric-ontology-fabric-datafusion-governance-remediation
version: v1
date: 2026-08-28
status: draft
baseline_commit: 71a888f
working_tree_digest: 1b162d023cd554b15422aaad9df95d27e32ace4195d859d7fb91ce7d3f0bf4d3
parent_design: docs/designs/codefabric_ontology_compiled_data_fabric_design_v3_2026-08-27.md
review_source: docs/reviews/implementation_review_codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27_2026-08-28_v1.md
primary_scope:
  - src/compiled_ontology.rs
  - src/ontology_rules.rs
  - src/ontology_activation.rs
  - src/ontology_plane.rs
  - src/domain_conformance.rs
  - src/semantic_query.rs
  - src/snapshot_runtime.rs
  - src/snapshot.rs
  - src/fabric/serving.rs
  - src/fabric/publication.rs
  - src/fabric/result_checksum.rs
  - src/daemon.rs
  - src/coordinator.rs
  - src/bin/codefabric_model/
  - src/generated/
  - contracts/schema/
  - contracts/registry/
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
governing_principles: docs/library_ref/full_data_fabric_design_principles.md
---

# CodeFabric ontology fabric — DataFusion-unified governance remediation design v1

**Standing of this document.** This is a *candidate* remediation design for the accepted
v3 ontology-compiled-data-fabric lineage, produced in parallel with a second candidate
authored independently
(`codefabric_ontology_compiled_data_fabric_datafusion_arrow_unified_design_v4_2026-08-28.md`).
It does not claim the lineage's next version number; the design owner adjudicates
between (or merges) the candidates. Its decision IDs continue v3's numbering
(D-10…D-15) so a merge can reference them stably.

**What this design is.** The implementation review (verdict: changes-required; four
blockers, six major findings) confirmed v3's architecture and condemned the
implementation's central omission: *the compiled contracts exist as data but do not
drive execution* — handwritten validators, phrase tables, a prose-operand rule model, a
census-shaped "closure", a test-only activation path, and hardcoded result-authority
selection all survive beside the generated authorities. This design revises the v3
lineage so that weak implementations **cannot** satisfy the invariants: it replaces the
prose rule model with a typed operation algebra lowered by one generic interpreter into
DataFusion plans, makes closure a receipt computed by executing generated plans over the
leased catalog, makes Stage-2b acceptance durable and owner-routed, pins result
authorities in the snapshot manifest, completes the ID-domain lattice, and unifies gate
execution with the serving path's existing plan-artifact and metrics machinery. This is
the owner-directed end-state: **orchestration and execution maximally based in
DataFusion and Arrow** — schemas, planning, calculation, validation, and observability
all flow through the same engine objects, with Rust reduced to compilation,
orchestration, and transaction discipline.

Baseline: HEAD `71a888f` with the uncommitted plan-v2 candidate worktree (digest above,
122 entries at analysis time). Review findings cited as `IR-001`…`IR-010`; v3
decisions/invariants cited by their v3 IDs; constitution `P1`–`P25`; alignment-manual
pattern IDs by family; DataFusion/Arrow references as `df`/`arrow` with section numbers.

## 1. Executive decision

Six decisions, numbered continuing v3's D-01…D-09. Each names the findings it closes and
the DataFusion/Arrow capability it stands on (verified this session at the pins unless
marked probe):

| ID | Decision | Closes | Engine substrate |
|---|---|---|---|
| **D-10** | **Typed operation algebra + one generic lowering interpreter.** `CompiledRuleContract`'s prose operands are replaced by a closed, typed plan-spec model; one interpreter lowers every rule, phrase predicate, and FK contract to `LogicalPlanBuilder` chains; the ten handwritten validators, the three handwritten phrase tables, and the 89-contract FK loop are deleted as authorities | IR-004, IR-008 | `LogicalPlanBuilder` scan/filter/join/`LeftAnti`/aggregate (`df-plan` §43); `case`/`coalesce`/`in_list` built-ins (`df` §11–12); zero UDFs |
| **D-11** | **Gate-execution plane + relational closure receipts.** All governance plans (rules, FK, closure) execute once through a governed session; every execution yields a `GateExecutionArtifact` (plan displays + native metrics harvest); TI-8 closure is an opaque `ClosureReceipt` digesting canonical results of generated closure plans, bound to full candidate identity and engine fingerprint | IR-001; unifies observability | `ExecutionPlan::metrics()` post-collect + `children()` walk (`df` §30); `DisplayableExecutionPlan` (diagnostics only); app-side blake3 over canonical bytes (no in-plan digest — DF has sha only, no blake3) |
| **D-12** | **Durable, owner-routed Stage-2b activation.** Opaque receipt-derived dossier binding the full manifest identity; a dedicated durable activation record written in the same SQLite transaction as the pointer; recovery reconciles it; one production owner route through the daemon admin surface; candidate classification makes generic activation refuse unaccepted ontology candidates | IR-002, IR-003 | transaction discipline (SQLite), not engine; receipts from D-11 |
| **D-13** | **Manifest-pinned result authority.** The snapshot manifest pins `{result_schema_version, checksum_version}`; serving selects generated result schemas and the checksum function from the leased snapshot's pin; leases pin snapshots, so old/new coexistence is structural | IR-006 | generated `result_schemas` + `result_checksum_for_version` as the production dispatch |
| **D-14** | **Complete, fail-closed ID-domain lattice.** `DomainState = Domain(id) ∣ Neutral ∣ Bottom ∣ Opaque` propagated over the full `Expr` grammar via `TreeNode`, with join-key, subquery, set-op, CASE, function, and aggregate rules; unknown variants and metadata-erasing forms are `Opaque` and comparisons against them are rejected; one governed session-construction seam installs the rule for serving *and* gate execution | IR-005 | `AnalyzerRule` runs pre-decorrelation on all four ingress paths (`df-plan` §41/§48); `TreeNode` on `Expr` (`df-plan` §46) |
| **D-15** | **Observation-only probes, accountable decisions.** Probes measure the actual capability and emit observations; owner decisions are external schema-v2 state transactions; drift in pins/config/report digests invalidates | IR-007 | n/a (governance) |

Process obligations carried into transition and proof, not new architecture: DB01's
retired recipe is removed from the command contract (IR-009); remediation executes as a
new plan version with a proving-commit-per-packet mandate and accepted input evolutions
(IR-010).

**What is deliberately not done:** no UDFs (built-ins cover every operation in the
algebra census; the escalation ladder and `ExtensionDecisionRecord` gate stand); no
`EXPLAIN ANALYZE` anywhere (the query-path ban stays; the gate plane harvests typed
metrics from the once-executed plan — strictly better than a re-executing text surface);
no custom logical/physical nodes for rules (P14: the closed algebra needs only built-in
relational operators).

## 2. Constraints and target invariants

### 2.1 Constraints

All v3 §2.1 constraints stand (F-1…F-6 functionality contracts; L-1…L-6 library facts;
P1–P25). This design adds library facts verified this session:

- L-7. The DataFusion 55 analyzer phase runs between binding and logical optimization on
  the shared downstream path of all four plan ingress routes (SQL, DataFrame,
  `LogicalPlanBuilder`, custom IR); DataFrame laziness defers analysis to action time;
  `SQLOptions::verify_plan` does **not** apply to `execute_logical_plan` — the analyzer
  is the fail-closed boundary for programmatic plans. Direct `Optimizer`/planner
  invocation bypasses analysis; bypass tests are mandatory evidence (`LOG-07`,
  `TST-12`).
- L-8. `AnalyzerRule` sees subqueries **pre-decorrelation** (`Expr::InSubquery/Exists/
  ScalarSubquery` intact); decorrelation is an optimizer-rule family. The `Expr` variant
  list in the reference is labeled "common", and DF54 added `HigherOrderFunction`/
  `Lambda`/`LambdaVariable` — exhaustive matches must carry a fail-closed default arm.
- L-9. Post-collect `ExecutionPlan::metrics()` harvest with a recursive `children()`
  walk is the documented metrics pattern (`ExecutionPlanVisitor` is undocumented at the
  pin); `ScalarSubqueryExec` has more children than its main input. `EXPLAIN ANALYZE`
  re-executes and aggregates to text.
- L-10. DataFusion 55 built-in digest functions are the sha family only — no blake3.
  Receipt digests are therefore computed application-side (the canonicalization stack)
  over canonical Arrow bytes, namespaced by an engine fingerprint (`MOD-06`, `OBS-09`);
  `RowConverter` byte stability is **not documented** in the reference corpus — row-byte
  digests are engine-version-namespaced and the existing cross-version KAT posture
  (arrow58/59) is the model.
- L-11. Verified in-tree at the pin (compile evidence): `SessionStateBuilder::
  with_analyzer_rule` (`src/fabric/serving.rs:511-513`). Probe-gated (§7): `in_subquery`
  /`exists` Expr builder functions, `is_distinct_from`, exact regex built-in names —
  none currently required by the algebra census.

### 2.2 Review findings as design forces

| Finding | Answer in this design |
|---|---|
| IR-001 recursive self-description is a census | D-11: closure = executed plans + opaque receipt with per-family negative corruption oracles |
| IR-002 Stage 2b test-only, bypassable | D-12: daemon admin owner route; candidate classification refuses bypass |
| IR-003 dossier unbound, idempotence process-local | D-12: receipt-derived opaque dossier binding full manifest identity; durable activation record in the same transaction; recovery reconciliation |
| IR-004 rule contracts don't drive execution | D-10: prose operands → typed plan specs; one interpreter; handwritten validators deleted; causal mutation oracles |
| IR-005 lattice bypassable | D-14: full-grammar fail-closed lattice; ingress + bypass proofs |
| IR-006 result selection global | D-13: manifest pin; version dispatch in production |
| IR-007 probes self-authorized | D-15: observation/decision separation |
| IR-008 phrase duplication | D-10: phrase bindings compiled and dispatched generically; fail-closed unmatched phrases |
| IR-009 DB01 recipe live | transition: recipe + census entry removed |
| IR-010 nothing certifiable | transition: proving-commit mandate, input-evolution acceptance, gate re-runs at proving HEAD |

### 2.3 Target invariants (v3 TIs strengthened; satisfaction criteria now exclude weak implementations)

- **TI-2′ (universal domain enforcement).** One `AnalyzerRule` carrying the complete
  domain lattice is installed by the single governed session-construction seam used by
  every serving *and* gate session. Satisfaction requires: (a) the lattice census —
  every documented `Expr` variant has an explicit lattice rule and unknown variants are
  `Opaque`; (b) the nested-expression rejection suite (CASE, coalesce, BETWEEN,
  IN-subquery, set comparisons, joins, aggregates, unknown extensions) passes with
  same-domain controls; (c) bypass-path tests prove direct-optimizer invocation is not
  reachable from any production entry (`LOG-07`, `TST-12`). (IR-005)
- **TI-7′/TI-9′ (one causal compiled authority).** Every rule, phrase, and FK contract
  in `CompiledOntology` carries typed operands sufficient for lowering; exactly one
  interpreter lowers them; satisfaction requires the **bijection census** (each
  `rule_contract`/`phrase_binding` row lowers to exactly one executed plan per gate run,
  and no validation plan exists without a contract row) *and* **causal mutation
  oracles** — for every operation kind and phrase binding, a mutated contract observably
  changes execution outcome. A green gate with a severed causal link is definitionally
  impossible once the handwritten authorities are deleted and the zero-state rules land.
  (IR-004, IR-008)
- **TI-8′ (receipt-verified closure).** Self-description is satisfied only by a
  `ClosureReceipt` produced by executing the generated closure-plan families over the
  leased catalog and the delivered result artifact: every governed code resolves through
  `ontology_term`; every edge endpoint resolves; every `semantic_type_binding` targets a
  served dimension; every `table_contract`/`column_contract` row matches the served
  schema census (schema-digest equality per table); the delivered result artifact's
  schema digest and checksum version resolve to `result_schema`/`result_field` rows;
  identity recipes resolve to `id_domain` rows; `phrase_binding`/`rule_contract` rows
  match the compiled authority census; snapshot, publication, and plan identities are
  recorded. Per-family corruption fixtures must flip the receipt. (IR-001)
- **TI-10 (gate executions are artifacts).** No governance plan executes in a throwaway
  session: every rule/FK/closure plan runs in a governed session and yields a
  `GateExecutionArtifact` (logical + physical displays, native metrics, output counts,
  violating-row projections), and every gate bundle yields one receipt. Plan text is
  diagnostics; receipts are identity — engine-version-namespaced, owned-algorithm
  digests. (`OBS-01/02/03/11`, `MOD-06`)
- **TI-11 (durable activation truth).** Stage-2b acceptance exists iff the durable
  activation record exists; process memory is a cache of it; recovery reconciles before
  any retry; the serving pointer cannot advance to an ontology-class candidate without a
  matching durable acceptance. (IR-002/003; `P11`, `P20`, `P24` of the holistic set)
- TI-1, TI-3, TI-4, TI-5′ (result authority now manifest-pinned per D-13), TI-6 continue
  from v3 unchanged in intent.

### 2.4 Out of scope

Unchanged from v3 §2.4. Additionally out of scope: rewriting completed candidate work
that the review found materially sound (typed control capture, statistics composition,
adversarial pushdown, span classification, generated row shapes) — those surfaces need
regression evidence and proving commits, not redesign.

## 3. Target architecture

### 3.1 The unified engine picture

```text
contracts/ registries + Schema Contract IR
        ▼  SchemaContractCompilation (one pass)
CompiledOntology  — vocabulary · contracts · ID domains · phrase bindings
                    · rule plan-specs (TYPED OPERANDS)          [D-10]
        ▼ consumed by
┌────────────────────────────────────────────────────────────────────┐
│ one governed session seam (SessionStateBuilder factory):           │
│   extension-type registry (all ID domains) + DomainConformanceRule │
│   (complete lattice)                                        [D-14] │
├──────────────────────┬─────────────────────────────────────────────┤
│ serving sessions     │ gate sessions                               │
│ (agent query path)   │ (publication validation · closure · probes) │
│ typed PlanSpec →     │ generic interpreter lowers rule/phrase/FK   │
│ LogicalPlan →        │ plan-specs → LogicalPlans → execute once    │
│ QueryPlanArtifact    │ → GateExecutionArtifact + metrics    [D-11] │
│ + manifest-pinned    │ → per-plan canonical digests → receipts     │
│ result authority     │                                             │
│ [D-13]               │ ClosureReceipt binds candidate identity     │
└──────────────────────┴─────────────────────────────────────────────┘
        ▼ receipts feed
opaque OntologyCandidateDossier → owner route (daemon admin) →
durable activation record + pointer, one SQLite transaction    [D-12]
```

One engine, one session seam, one interpreter, one artifact family, one receipt
discipline. Rust owns: the compiler pass, the interpreter's lowering match, transaction
and recovery logic, and diagnostics rendering — no relational semantics.

### 3.2 D-10 — Typed operation algebra and the generic interpreter

**The algebra (closed, from the implementation census).** Every one of the eleven
`CompiledRuleOperationKind` variants, all 89 FK contracts, and all phrase predicates
reduce to this operator set — no Rust algorithm survives in any validator:

```text
RulePlanSpec := source · step* · assertion
source       := scan(table_code) | scan_filtered(table_code, predicate)
step         := project(cols, widening_casts)                 -- casts on code columns only
              | filter(predicate)
              | inner_join(right: source, on: [(l,r)])
              | left_anti_join(right: source, on: [(l,r)])
              | count_aggregate(group_by: cols, having_count_gt: n)
predicate    := eq | not_eq | lt | lt_eq | gt | gt_eq
              | is_null | is_not_null | and | or | not
              | in_list(col, codes) | literal(scalar)
assertion    := assert_empty { diagnostic_code, projection: cols }
```

The three things the implementation census found living in Rust become **compiled
operands**:

1. *Operand discovery* — governed-code column sets, span column groups, PK column
   lists, `(value_kind_code, value_column)` pairs (killing the `(index+1)*10`
   arithmetic), the semantic-authority→dimension mapping — all emitted by the model
   compiler, which knows them statically from the Contract IR.
2. *Vocabulary literals* — cardinality names with their group-key sides, self-edge
   policy values, edge predicate ids, and **one owner-resolution operand per
   owner-selection rule** (the census found all six rules collapsed to one behavior —
   this design completes the semantics: each rule names the join path that produces the
   expected owner, still within the closed algebra).
3. *Diagnostic projection* — each spec declares the columns surfaced on violation; one
   generic renderer materializes them from the violating batch (deleting the per-rule
   Arrow downcasts).

**The interpreter.** One `lower(RulePlanSpec) → LogicalPlan` function: an exhaustive
match over the algebra, building `LogicalPlanBuilder` chains (`scan → filter → join/
join_on(LeftAnti/Inner) → aggregate → filter → limit`), executed in a gate session
(D-11). It is the *only* execution route for governance semantics; `frame()`/
`rejects_any()` and the ten validators are deleted, and `publication.rs`'s FK loop
becomes iteration over generated `ForeignKeyAntiJoin` specs through the same
interpreter.

**Phrase plane.** The `phrase_binding` relation (table 29) becomes the *complete*
compiled authority: every arm of the three handwritten tables (7 entity + 8 relation +
3 property) becomes a generated binding row + typed operation
(`in_set(column, codes)`-class predicates with `operator`, `operand_domain`,
`null_policy`, `output_role`, `diagnostic_code` all honored — `null_policy` lowers to
explicit three-valued handling via `IS TRUE`/`coalesce` composition). The relational and
graph paths consume one dispatch. **Unmatched phrases become typed
`SemanticQueryError`s** — the census found the current wildcards silently produce an
empty code set (no predicate at all), a correctness hazard the review did not flag; this
design closes it fail-closed.

**Calculation posture.** Built-in `Expr` composition only (`EXP-01/02`); the algebra
requires no regex, no digest, no custom kernel in-plan; any future operation that cannot
be expressed with built-ins arrives as a typed algorithm variant (v3 TI-9 provision)
or — only with an accepted `ExtensionDecisionRecord` — a UDF implementing the full
truthful hook set. Expected UDF count: zero.

**Principles:** Advances `P1`/`P2` (the model executes), `P3` (one authority), `P14`
(highest sufficient abstraction), `P15` (transparent, optimizer-visible predicates);
closes constitution anti-patterns "hidden semantic logic" and "multiple authorities".

### 3.3 D-11 — Gate-execution plane and closure receipts

**Gate sessions.** A gate session is built by the same governed seam as serving
(extension registry + lattice rule installed) — the census's throwaway
`SessionContext::new()` sites are deleted. Gate plans therefore get domain enforcement
for free, and the governed-code widening casts (on plain code columns) pass the lattice
by construction.

**GateExecutionArtifact.** Each lowered plan executes exactly once; after stream
exhaustion the executor harvests `metrics()` via the documented recursive `children()`
walk and renders `display_indent` / `DisplayableExecutionPlan::with_full_metrics` —
reusing the serving path's existing helpers (`physical_metrics`,
`physical_metric_map`), which move to a shared module. The artifact records: rule/plan
id, contract digest, logical + physical displays (diagnostics), typed metric map,
output row count, violating-row projection (bounded), and the per-plan **canonical
result digest** — RowConverter row bytes digested by the application blake3 stack,
exactly the `ResultChecksum` discipline.

**Receipts.** A `GateBundleReceipt` digests, with an owned canonical encoding: the
ordered per-plan result digests, the compiled-authority digest set (bundle ids), the
candidate identity (workspace/repository/worktree ids, `publication_id`, manifest
digest, and per-table `{table_uri, delta_version, schema_digest,
effective_content_digest}` from the manifest fields the census found unused), and an
**engine fingerprint** (datafusion/arrow versions, relevant config) as namespace —
`MOD-06`/`OBS-09` discharged. Receipts are opaque types with constructor-only creation;
their digests are what dossiers and oracles consume.

**Closure plans (TI-8′).** The closure families are themselves generated
`RulePlanSpec`s (mostly `left_anti_join … assert_empty` over the ontology plane joined
to served-surface censuses) plus a small typed non-relational census (Arrow schema
digests per served table, delivered-artifact schema-digest lookup into
`result_schema`). The `ClosureReceipt` extends the bundle receipt with the resolved
snapshot/publication/plan identities. Negative proof: one corruption fixture per
closure family (unknown code, dangling edge, unbound semantic type, schema-digest
mismatch, unresolvable delivered result, missing identity recipe, contract-census
mismatch) must flip the receipt — the review's exact re-test.

**ANALYZE posture (owner-raised, decided).** `EXPLAIN ANALYZE` is rejected everywhere:
on the agent query path it is already banned (single-execution artifact discipline);
on the gate plane, direct post-collect metrics harvest yields typed `MetricsSet` from
the same execution that produced the digested results — a second, text-producing
execution would be strictly worse. `EXPLAIN` displays are captured as advisory
diagnostics, never identity.

**Principles:** Advances `P9`/`P10` (provenance closure by construction), `P17`
(inspectable intermediates), `P24` (semantic observability), `P25`; `OBS-01/02/03/04/
06/11`, `TST-11`.

### 3.4 D-12 — Durable, owner-routed Stage-2b activation

- **Dossier v2.** Private fields, constructor-only; derived from the frozen candidate
  manifest + the `GateBundleReceipt`/`ClosureReceipt` digests (looked up from real
  receipt objects, never caller-supplied strings); binds the full identity listed in
  D-11. `activate` recomputes the dossier digest from its inputs and rejects mismatch;
  replay onto a different candidate fails on manifest-digest binding even with
  coincident `(table_code, delta_version)` pairs.
- **Durable record.** A dedicated operational-store `ontology_activation` row
  (workspace-keyed: pointer generation, input digest, table-version digest, acceptance
  owner/digests/timestamp) written **in the same SQLite transaction** as the manifest
  insert and pointer advance (the census located the transaction:
  `activate_unphased`). The existing audit event remains as trail, not truth.
  `recover` loads the record and reconstructs `OntologyActivationState` before any
  retry — a post-commit crash then retries as the idempotent no-op the design always
  required. Fault matrix extends to: predecessor present, active leases held, all
  ontology and snapshot fault points × restart × retry, asserting exactly one
  acceptance row and one pointer generation.
- **Owner route.** New `WorkspaceAdminCommand::AcceptOntologyCandidate` on the daemon
  admin surface (the census confirmed the admin envelope/dispatch is the only
  production command path) → coordinator method that freezes the candidate, runs the
  D-11 gate bundle, verifies receipts, records accountable owner identity from the
  authenticated admin boundary, and calls the Stage-2b transaction.
- **Candidate classification.** `ServingSnapshotCandidate` gains a class
  (`OntologyFingerprintMoving` vs `Ordinary`) derived from the manifest's ontology
  bundle/table set delta; generic `activate` **refuses** ontology-class candidates
  without a matching durable acceptance (attachment point: the existing
  manifest↔dossier binding check). Gate-B vertical and overlay-rebase callers continue
  using generic activation for ordinary candidates — the authority split is by class,
  not by caller goodwill.

**Principles:** `P11` (explicit state transitions), `P13` (governance at the owning
boundary), constitution "mutable authority"/"provenance afterthought" anti-patterns
closed; holistic Principles 20/24/25/27 (unified transactions, idempotency,
reproducibility, provenance) restored per the review's doctrine assessment.

### 3.5 D-13 — Manifest-pinned result authority

`ServingSnapshotManifestBody` gains `result_authority { result_schema_version,
checksum_version }`, populated from the schema bundle at candidate construction.
Serving reads the pin from the leased snapshot: result-schema selection
(`graph_output_schema`'s hardcoded match and the global
`RESULT_CHECKSUM_VERSION` constant at the emission sites) is replaced by
manifest-driven lookup into the generated result-schema set and by
`result_checksum_for_version` as the production dispatch. Because every lease pins a
snapshot, an old lease holds its old manifest and therefore its old authority across
activation and restart — coexistence is structural, not scheduled. Oracle: hold an old
lease across a result-authority-moving activation plus a restart; old lease = prior
version/schema, new lease = target, both stable; matrix added to determinism and
form-contract gates.

### 3.6 D-14 — The complete domain lattice

`DomainState = Domain(id) | Neutral | Bottom | Opaque` with `⊔`: `Bottom` joins with
anything (typed/untyped NULL literals); `Neutral` = non-ID values; `Domain(a) ⊔
Domain(a) = Domain(a)`; `Domain(a) ⊔ Domain(b≠a)` = rejection at comparison points;
anything `⊔ Opaque = Opaque`, and **comparison, join-key, IN-membership, or set-op
alignment against `Opaque` is rejected** (fail-closed). Rules per variant family (the
verified census, with the mandatory default arm):

- pass-through: `Alias`, `Cast`/`TryCast` (still rejected when erasing a domain —
  retained from current behavior), parenthesized/`Negative` (numeric ⇒ `Neutral`);
- sources: `Column` (schema lookup incl. outer references), `Literal` (extension
  metadata ⇒ `Domain`, null ⇒ `Bottom`, else `Neutral`), `ScalarSubquery` (output
  column's domain), `OuterReferenceColumn`;
- boolean producers consuming any: `IsNull/IsNotNull/IsTrue/IsFalse/IsUnknown` family,
  `Not`, `And`/`Or`, `Like`/`SimilarTo` (operands must be `Neutral`);
- comparison points (same-domain-or-both-neutral required): `BinaryExpr` comparison
  ops, `Between` (value/low/high tri-agreement), `InList`, `InSubquery` (expr vs
  subquery output column), `Exists` (recurse into subquery plan);
- structure: `Case` (result = `⊔` of branch results; predicate branches checked
  independently), `coalesce`/`nullif`/`ifnull`/`nvl` (domain-preserving iff all
  arguments agree, else `Opaque`), all other `ScalarFunction`s over a `Domain` argument
  ⇒ `Opaque`;
- aggregates/windows: `min`/`max`/`first_value`-class preserve; `count` ⇒ `Neutral`;
  others over `Domain` ⇒ `Opaque`;
- plan-level: `Union`/set-op positional alignment (existing) extended to **equi-join
  key domain agreement** on every `Join`, and to `Distinct`/`Intersect`/`Except`
  inputs; `Unnest` propagates element domain;
- unknown/lambda/higher-order and any future variant: `Opaque` via the default arm.

The rule stays analysis-only (never rewrites), runs pre-decorrelation (L-8), and is
installed solely by the governed session seam — one authority for serving and gates.
Proof: lattice-census test (every documented variant has an explicit arm), the
nested-expression rejection suite with same-domain controls, ingress coverage through
SQL/DataFrame/`execute_logical_plan`/binder delegation, and the `LOG-07`/`TST-12`
bypass suite.

### 3.7 D-15 — Probe and decision governance

The probe runner becomes observation-only: each PR probe measures the capability it
names (PR-3a: real Delta `STRUCT` round-trip; PR-5: real Parquet
`ARROW:schema` extension-metadata persistence; PR-6: real unused-left-join EXPLAIN
comparison), emits a structured observation with genuine environment identity (engine
versions, lockfile digest, relevant config, fixture digests), and **cannot** record a
decision. Owner decisions are separate schema-v2 state transactions supplied outside
the test process; downstream packets fail on missing, stale (drifted pins/config/report
digest), or unreviewed observations. The recorded PR-7 performance waiver is preserved
verbatim.

### 3.8 Library decisions (delta over v3)

### LD-08′ — DataFusion 55 `AnalyzerRule` as the universal domain boundary: adopt (extended)

**Decision:** retain v3's adoption; extend to the complete lattice and to gate sessions.
**Version basis:** analyzer phase position, ingress convergence, and pre-decorrelation
order verified (`df-plan` §§41/46/48); `with_analyzer_rule` proven in-tree.
**Risk:** bypass via direct optimizer invocation — mitigated by `TST-12` bypass suite
and the absence of any production direct-optimizer call (governance rule added).
**Validation:** lattice census + rejection suite + ingress/bypass oracles.

### LD-09 — `TreeNode` traversal for lattice and interpreter: adopt

**Version basis:** `TreeNode` implemented for `Expr`/`LogicalPlan` with
`apply/visit/transform*` (`df-plan` §46). `TreeNodeRecursion` early-exit variants
probe-gated if needed.
**Validation:** compile + lattice tests.

### LD-10 — Post-collect metrics harvest via `children()` walk: adopt

**Version basis:** `df` §30 documented pattern; `ExecutionPlanVisitor` undocumented at
pin — not used. `ScalarSubqueryExec` extra-children caution encoded in the walker test.
**Displaces:** any temptation toward `EXPLAIN ANALYZE` on governance paths (rejected).
**Validation:** metrics-harvest test on a plan containing subquery, join, aggregate.

### LD-11 — App-side blake3 receipts over canonical Arrow bytes: adopt

**Version basis:** DF built-in digests are sha-family only; blake3 absent; `RowConverter`
stability undocumented in the corpus — digests engine-version-namespaced (`MOD-06`),
following the existing arrow58/59 KAT posture.
**Displaces:** nothing; extends the `ResultChecksum` discipline to gates.
**Validation:** receipt determinism KATs + engine-fingerprint namespacing test; upstream
row-format stability probe recorded (§7).

### LD-12 — Built-ins-only calculation plane, zero UDFs: adopt

**Version basis:** the algebra census requires only scan/project/filter/join/anti/
aggregate + `case`/`coalesce`/`in_list`/null-tests — all NATIVE built-ins at 55.
**Displaces:** any UDF proposal without an `ExtensionDecisionRecord`.
**Validation:** governance rule — no `create_udf`/`ScalarUDFImpl` outside an
EDR-referenced module; `query-legacy-zero-state-check` continues.

### 3.9 Governance, state, failure (deltas)

- The governed session seam is the single construction point for serving and gate
  sessions (one place where registries, lattice, and config are installed) — a new
  structural rule bans `SessionContext::new()`/ad-hoc `SessionStateBuilder` outside it
  in governed modules.
- Failure taxonomy gains: `GATE_PLAN_LOWERING_FAILED` (interpreter rejects a malformed
  spec — a compile-time census makes this unreachable in release),
  `CLOSURE_FAMILY_UNRESOLVED` (per-family diagnostic codes),
  `ONTOLOGY_ACCEPTANCE_UNBOUND` (dossier/candidate mismatch),
  `RESULT_AUTHORITY_UNPINNED` (manifest missing the pin — fail-closed at lease).
- Security: owner acceptance rides the authenticated admin boundary (UDS peer
  credentials today; capability tokens at W17 unchanged).

## 4. Alternatives and clean-sheet challenge

**Alt A — patch the handwritten validators to *read* the compiled contracts** (minimal
diff: keep ten validators, parameterize them from contract rows). Rejected: it
preserves the dual authority the review condemned (IR-004's exact failure mode —
"changing a governed contract does not change enforcement" would still hold for any
semantics the parameters don't reach), keeps the prose operands, and makes the mutation
oracle unwinnable. This is the legacy-preserving trap.

**Alt B — the selected design** (D-10…D-15).

**Alt C — push orchestration itself into DataFusion** (rules as UDTFs, closure as a
recursive plan, activation as DML). Rejected: traversal/DML/UDTF paths are
EDR-gated with no semantic need (`P14`); activation is transaction discipline SQLite
already owns; a UDTF rule surface would *hide* rule semantics from the plan validator
(`P15`) — the same reasoning that rejected UDTFs for traversal in v3.

**Clean sheet:** starting from nothing, one would build exactly B — a compiler emitting
typed plan specs, one interpreter, one session seam, receipts from executed plans, and
a durable acceptance record. Nothing in B exists to accommodate the current code; the
current code is either completed into B's shape or deleted by it.

## 5. Transition, cutover, and legacy disposition

### 5.1 Position

The plan-v2 candidate stays uncommitted and uncertified (IR-010); this design requires
a **new plan version** (packet boundaries change). The review's remediation order is
preserved with one dependency correction: the receipt plane (D-11) consumes the
interpreter core (D-10), so the trust vertical lands on a minimal interpreter first,
then the full authority migration.

### 5.2 Stages

- **R0 — governance floor.** D-15 probe/decision separation; the governed session seam
  (serving adopts it; behavior-equivalence oracle); DB01 recipe + census-entry removal
  (IR-009). No schema movement.
- **R1 — trust vertical (review order 1).** Interpreter core sufficient for closure
  families + `GateExecutionArtifact`/receipt machinery [D-11]; opaque dossier, durable
  activation record, recovery reconciliation, owner route, candidate classification
  [D-12]. Exit: the four blocker re-tests (closure corruption suite, owner-route +
  bypass negative, unbound/stale dossier rejection, post-commit restart idempotence)
  green.
- **R2 — causal authority (review order 2).** Contract-IR revision: typed operands for
  all eleven operation kinds + 89 FK contracts + complete phrase bindings; full
  interpreter migration; delete the ten validators, the three phrase tables, the
  `code_dimension` match, `frame()`/`rejects_any`; fail-closed unmatched phrases;
  mutation + bijection oracles. Fingerprint-moving only in the ontology-plane rows
  (contract relations), governed release.
- **R3 — boundary completeness (review order 3).** D-14 full lattice + rejection/
  ingress/bypass suites; D-13 manifest pin + coexistence matrix (one schema-bundle
  revision adding the manifest field).
- **R4 — certification (review orders 4–5).** Proving commits per dependency-closed
  packet; input-evolution acceptance through owning packet transactions; DB/milestone
  closure; final non-performance gate matrix + `ci-pr` at proving HEAD; focused
  re-review per the review's scope.

Rollback: R0–R3 revert by commit (the only durable-schema moves are the ontology-plane
contract rows in R2 and the manifest field in R3, each a governed bundle revision with
the prior bundle activatable); R1's durable activation table is additive and inert
until an ontology-class candidate exists.

### 5.3 Legacy disposition matrix

Inventory: the implementation-review censuses plus this session's architect census
(`grep`/`ast-grep` commands recorded in the census scratchpad; coverage 103 Rust files,
0 skipped). Dispositions over every surface the remediation touches:

| Surface | Disposition | Justification |
|---|---|---|
| `ontology_rules.rs` ten validators + `frame`/`rejects_any` + `code_dimension` match | **delete** | replaced by compiled operands + interpreter (D-10); zero-state + tier-1 proof at R2 |
| `publication.rs` `validate_references` FK loop | **reshape** | 89 contracts lower through the interpreter; violation type and error classes preserved |
| `CompiledRuleContract` prose fields | **replace** | typed operand model in Contract IR; prose retained only as generated description columns |
| `semantic_query.rs` phrase tables (:1194-1322) + `compiled_condition_predicate` partial consumption + `graph_certainty_codes` | **replace** | complete generated phrase bindings, one dispatch honoring operator/null-policy/diagnostics; fail-closed wildcards |
| `domain_conformance.rs` | **reshape** | current checks retained as lattice special cases; full-grammar propagation added; sole installation via the seam |
| `ontology_activation.rs` dossier/resolution/state | **replace** | opaque receipt-derived types; census resolution superseded by closure plans |
| `snapshot_runtime.rs` `activate`/`activate_stage2b`/`recover` | **reshape** | classification, durable record, recovery reconciliation; stage/fault enums preserved and extended |
| `serving.rs` result-version emission + `semantic_query.rs` `graph_output_schema` match | **replace** | manifest-pinned dispatch (D-13) |
| `result_checksum_for_version` | **preserve → promote** | becomes the production dispatch |
| serving metrics/display helpers (`physical_metrics`, `physical_metric_map`, artifact types) | **preserve + share** | move to a shared module consumed by gates (D-11) |
| `scripts/ontology_fabric_probe_suite.py` decision recording | **replace** | observation-only runner + external decision transactions (D-15) |
| `justfile` `id16-extension-contract-check` + `gate-filter-census.json` entry | **delete** | DB01 exit (IR-009) |
| daemon admin surface | **preserve + extend** | new acceptance command arm; dispatch pattern unchanged |
| coordinator | **preserve + extend** | gains the activation orchestration method |
| candidate-complete surfaces the review certified materially sound (generated row shapes, typed control capture, statistics, span classification, result-schema generation) | **preserve** | regression evidence + proving commits only |

No `encapsulate-temporarily` surfaces. The only versioned coexistence remains
`ResultChecksumV1`/`V2`, now correctly routed by manifest pin (D-13) with the v3
retirement condition unchanged.

### 5.4 Spec and index alignment

The v3 spec amendments stand. R2/R3 carry two amendment increments: the rule-contract
relation's typed-operand columns and the manifest `result_authority` field enter
`FAB` §11/§92-adjacent text and `AC-G-19` (manifest fields) with their bundle
revisions; `docs/spec_index` navigation updates ride the same packets.

## 6. Proof strategy

The review's meta-finding — selectors green with the causal link absent — is answered
by three proof classes that make weak implementations fail *structurally*:

1. **Causal mutation oracles (TI-7′/TI-9′).** Per operation kind and per phrase
   binding: execute with the shipped `CompiledOntology`, then with a test-scoped
   mutated contract; assert the outcome changes accordingly. Plus the bijection census
   (contract rows ↔ executed plans, both directions) and zero-state + tier-1 deletion
   proof for every handwritten authority.
2. **Negative-family receipts (TI-8′).** Per closure family, a corruption fixture that
   must flip the `ClosureReceipt`; the review's named re-test
   (`odf_stage2b_self_description_rejects_unresolved_result_and_broken_edges`) is the
   umbrella oracle.
3. **Trust-vertical operations (TI-11).** The review's named re-tests adopted verbatim:
   `odf_daemon_stage2b_activation_owner_route` (+ generic-bypass negative),
   `odf_stage2b_rejects_unbound_or_stale_dossier`,
   `odf_stage2b_postcommit_restart_retry_idempotent` — with predecessor present, live
   leases, full fault × restart × retry matrix, exactly one acceptance and pointer
   generation.

Further named obligations: `odf_nested_expression_cross_domain_rejection` (D-14 suite)
plus lattice census and `LOG-07`/`TST-12` bypass tests; the D-13 old/new lease matrix
in determinism and form-contract gates; D-15's "probe cannot self-authorize" negative
(`packet-oracle-check` fails until an external decision transaction exists); gate
artifacts snapshot-tested with normalization (`TST-11`, `OBS-04`); receipt determinism
KATs with engine-fingerprint namespacing; session-seam and UDF-zero governance rules in
`governance-scan`; and the IR-010 certification protocol — every packet completes only
at a proving commit where its named checks pass, input evolutions are accepted through
owning packet transactions, and the final non-performance matrix plus `ci-pr` run at
the proving HEAD (performance remains owner-waived).

## 7. Probe register (delta)

| Probe | Binds | Fallback |
|---|---|---|
| PR-8 `TreeNodeRecursion` early-exit variants at 55 | lattice implementation detail | full traversal without early exit |
| PR-9 `in_subquery`/`exists` builder functions | interpreter closure-plan construction | construct via `Expr::InSubquery`/`Exists` variants directly |
| PR-10 arrow-rs 59.2 row-format stability wording (upstream docs) | LD-11 receipt caveats | posture unchanged — engine-version namespacing already assumed |
| PR-3a′/PR-5′/PR-6′ | honest re-runs of the miswired probes (D-15) | per original design fallbacks |

## 8. Acceptance

**accepted-with-named-assumptions** — ready for implementation planning (a new plan
version superseding plan v2, subject to the owner's adjudication between this candidate
and the parallel candidate design), with:

1. **A-1 (assumption):** PR-8/PR-9/PR-10 outcomes as tabled — all fallbacks are
   architecture-preserving.
2. **A-2 (assumption):** the six owner-selection rules' resolution semantics are
   confirmed against the ontology relation registry by the ontology owner during R2's
   Contract-IR revision (the census proved current code collapses them; the registry is
   the authority for what each rule means). Consequence if a rule proves genuinely
   non-relational: it becomes a typed algorithm variant under v3 TI-9's provision.
3. **A-3 (decision, owner):** plan v2's uncommitted candidate is carried forward as the
   working tree for the new plan (not reverted); packets re-prove from it with proving
   commits. This matches the review's "remediate then certify" order.

Reopening triggers: a probe contradicting L-7/L-8 (analyzer position or ingress
coverage); an operation the closed algebra cannot express arising during R2 (reopens
the algebra's closure, not the architecture); an accepted `ExtensionDecisionRecord`
introducing a UDF (reopens LD-12's zero-UDF claim); Delta-pin movement (unchanged v3
triggers).
