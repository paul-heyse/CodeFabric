---
artifact: plan-audit
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27.md
verdict: needs-revision
version: v1
date: 2026-08-27
status: complete
---

# Plan audit — CodeFabric ontology-compiled data fabric implementation plan v1

## Provenance and Scope

This is an independent, read-only audit of:

- `docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27.md`
- `docs/designs/codefabric_ontology_compiled_data_fabric_design_v2_2026-08-27.md`

The audit treats the design's reversals of prior decisions as intentional target-design
changes, not as regressions merely because they differ from the current suite. It tests
whether those reversals are well justified, whether the revised target is internally
coherent, and whether the plan faithfully and completely implements it.

Evidence inspected:

- every declared plan input, with every recorded SHA-256 digest rechecked;
- current `HEAD` `eb7a738fa55037b19706fd842737cecad65ffe16`, the declared baseline
  `eebb95878cc3f734df06f92aee50d255c44683ea`, and scoped drift between them;
- the active Waves 8–12 state and active-plan pointer;
- current schema, model-compiler, serving, semantic-query, publication, overlay,
  snapshot-catalog, result, and operational-projection code;
- the exact Arrow/Parquet 59.2.0, DataFusion 55.0.0, object_store 0.13.2, and delta-rs
  `43a0cf10…` source/reference baseline;
- `full_data_fabric_design_principles.md`, the holistic semantic-design doctrine, and
  `representative_datafusion_arrow_relational_usage.md`.

The plan's declared-input hashes are current. Its structural validator accepts the draft
and finds 16 work packets, four milestones, and five decommission batches. No Wave 9
preflight or repository-wide implementation gate was run: those are outside this audit.
The only repository change made by the audit is this report.

## Executive Summary

The proposed direction is materially better than the shape it replaces. In particular,
the ontology namespace, domain-specific logical IDs, generated result schemas, typed
plans, frozen snapshot catalogs, application-owned multi-table publication, and use of
Arrow as the common representation are all sound clean-sheet choices. The audit does not
recommend returning to the prior design.

The plan is nevertheless not ready to approve. Nine material defects remain:

1. the nine-table ontology plane cannot satisfy the stated self-description closure;
2. list-valued ontology memberships contradict the design's own relational-structure
   criterion;
3. ID-domain enforcement is limited to one binder path and does not specify the
   DataFusion registration or literal-domain contracts it requires;
4. the supposed single compilation seam still lacks one governed semantic object/rule
   model and an unambiguous result-field authority;
5. the DataFusion 55 statistics and constraints posture is factually wrong in opposite
   directions;
6. the packet DAG does not enforce its milestone barriers;
7. Stage 2b is accepted and activated before its executable ontology is complete;
8. probe observations are conflated with committed decisions and exact reproducibility;
9. the span's logical classification incorrectly depends on a physical-storage probe.

Two smaller corrections concern current baseline/program-state drift and the exact
delta-rs provider seam used by PR-2.

The strongest further consolidation is not one untyped EAV table or JSON rule blob. It is
one typed `CompiledOntology` object graph, built from the existing owner-specific
authorities, which emits normalized ontology relations, Arrow fields, DataFusion
extension registrations, analyzer rules, relational validation plans, serving
decorations, result schemas, constraints, and self-description tables through one
governed compilation pass.

## Readiness Verdict

**Verdict: `needs-revision`.**

The target direction is viable and should be retained, so `needs-redesign` would be too
strong. The corrections do, however, change target-design details, packet dependencies,
the Stage 2b release transaction, and multiple proof obligations. They cannot be handled
as editorial conditions on an otherwise approved plan.

Approval should wait for design v3 (or an explicit v2 amendment) and implementation plan
v2 addressing every Major finding below. Minor findings may be integrated in the same
revision.

## Finding Index

| ID | Severity | Category | Summary |
|---|---|---|---|
| F-001 | Major | Target design / completeness | The proposed catalog cannot prove TI-1/TI-8 self-description closure |
| F-002 | Major | Target design / doctrine | Independently joined ontology memberships are modeled as nested lists |
| F-003 | Major | Correctness / library grounding | ID-domain enforcement and extension registration are under-specified and path-local |
| F-004 | Major | Authority / consolidation | There is no single governed semantic object/rule pass or result-field authority |
| F-005 | Major | Library grounding / proof | WP15 overclaims `StatisticsRequest` and underclassifies PK `Constraints` |
| F-006 | Major | Dependency closure | Packet dependencies contradict the stated M02/M03 ordering and parallelism policy |
| F-007 | Major | Transition / atomicity | Stage 2b activates before serving and conformance gates, and dimension refresh is undefined |
| F-008 | Major | Proof quality / governance | Probe observations are treated as committed deterministic decisions |
| F-009 | Major | Target design / doctrine | Logical span identity is made contingent on a physical-storage probe |
| F-010 | Minor | Provenance / program interaction | The baseline narrative predates completed WP08 and lacks a current activation guard |
| F-011 | Minor | Library integration | PR-2 does not prove the production delta-rs provider construction path |

## Findings

### F-001 — The proposed catalog cannot prove TI-1/TI-8 self-description closure

- **Severity:** Major
- **Category:** Target design / completeness
- **Scope:** Design TI-1, TI-8, D-01, §3.12, and §6; plan WP09–WP11 and M04
- **Finding:** D-01 exposes nine vocabulary tables, but current delivered fields also
  depend on `registry:capability`, `opaque:diagnostic-code`,
  `identity:type-constructor`, and `schema:table-code`. The plan's structural census only
  closes `ontology:*`, `enum:*`, provider raw kinds, and ID domains. It does not expose
  registry authority, semantic-type binding, table/column contracts, result contracts,
  identity recipes, or phrase/rule bindings as catalog relations. Resolution therefore
  still depends on generated Rust dispatch such as `GENERATED_SEMANTIC_TYPE_BINDINGS`.
  M04 composes two narrower gates but never executes the stronger claim: beginning with
  only a leased catalog and delivered result, resolve every semantic field, code, ID
  domain, table contract/version/digest, snapshot, publication, and plan identity without
  reading Rust. The design also does not decide whether this closure is operator-only or
  agent-visible; the eight current query forms provide no ontology-introspection route.
- **Required resolution:** Either narrow TI-1/TI-8, or complete the catalog model. The
  preferred correction is to add typed, bundle-pinned relations for registry authority,
  semantic-type binding, table/column contract, and every currently uncovered code
  authority, with generated links to result schemas and identity recipes. Define the
  intended visibility boundary. Add one independent `ontology_self_description_closure`
  oracle that dynamically traverses the catalog/artifact chain, plus a new-domain fixture
  proving that adding a governed domain needs no hand-written consumer case.
- **Revalidation:** `just ontology-self-description-check`

### F-002 — Independently joined ontology memberships are modeled as nested lists

- **Severity:** Major
- **Category:** Target design / doctrine
- **Scope:** Design D-01, D-03, TI-4; plan WP09 and WP11
- **Finding:** `relation_kind` stores allowed subject and object family codes as lists.
  The source registries contain other independently meaningful memberships as well:
  projection memberships, allowed owner kinds, required/optional property codes, and
  phrase memberships. These are N:M relationships used independently in joins and
  conformance calculations. By the design's own `REP §17` criterion, they are relational,
  not structurally owned scalar payloads. Keeping them as lists forces list extraction or
  bespoke Rust checks and prevents ordinary FK, join, anti-join, and cardinality proof.
- **Required resolution:** Compile authored YAML arrays into normalized relations. A
  strong consolidated form is a generated `ontology_term` relation plus a typed
  `ontology_edge(subject_term_id, predicate_code, object_term_id, ordinal)` relation,
  with generated domain-pair constraints; typed bridge tables are also acceptable where
  they materially improve schema clarity. YAML remains the authority and the relations
  remain compiled projections. WP11 must validate bridge parity and use ordinary
  DataFusion joins/anti-joins rather than list/UDF logic.
- **Revalidation:** `just ontology-relational-closure-check`

### F-003 — ID-domain enforcement and extension registration are under-specified and path-local

- **Severity:** Major
- **Category:** Correctness / library grounding
- **Scope:** Design D-02 and LD-01; plan WP06–WP08 and WP13
- **Finding:** WP08 puts the domain rule in `BoundPlanSpec`, but current production code
  creates and executes `LogicalPlan`s through serving views, direct
  `ServingQuerySession::query_plan`, publication integrity, overlay, snapshot-catalog,
  maintenance, and graph lowerings. Those paths do not all traverse `BoundPlanSpec`.
  Compile-time FK/list/result-domain mismatches are also not rejected by the stated rule.
  DataFusion 55 supplies a session `AnalyzerRule` seam that runs for native plans during
  `SessionState::optimize`, making it a better universal enforcement boundary.

  The library contract is incomplete as well. Arrow `ExtensionType` implementations are
  not DataFusion registrations: `MemoryExtensionTypeRegistry` accepts generated
  `ExtensionTypeRegistration` factories that create `DFExtensionType` values. The
  registry supplies programmatic resolution and formatting, not automatic optimizer
  semantics or cast preservation. Field-aware casts are a separate DataFusion facility.
  Finally, `ScalarValue::FixedSizeBinary` carries bytes but no ID domain, so a wrong-domain
  literal must retain application-owned domain identity in bound IR; the registry cannot
  infer it from the scalar.
- **Required resolution:** Enforce domain compatibility at two levels: (1) the contract
  compiler rejects cross-domain FKs, list elements, generated joins, set-operation
  alignments, and result-role identities; (2) one generated, idempotent DataFusion 55
  `AnalyzerRule` validates every executable logical plan. Binder checks may remain for
  early diagnostics but must delegate to the same rule model. Generate separate Arrow
  extension metadata types and DataFusion registration factories, preferably backed by
  one generic resolved CodeFabric ID implementation. Preserve `DomainTypedLiteral` until
  validation. Test comparisons, joins, IN lists, casts, set operations, and every plan
  ingress, with a causal mutant that bypasses the old textual-prefix rejection.
- **Revalidation:** `just id-domain-extension-check && just semantic-query-conformance-check && just publication-referential-integrity-check`

### F-004 — There is no single governed semantic object/rule pass or result-field authority

- **Severity:** Major
- **Category:** Authority / consolidation
- **Scope:** Design §3.2, D-04, D-07, D-08; plan WP03–WP05, WP07–WP15
- **Finding:** The compilation diagram promises one seam, but the plan still assigns
  separate mechanisms to column shapes, row shapes, dimension builders, phrase bindings,
  domain checks, serving decorations, publication conformance, result schemas, and
  statistics policy. WP11 even permits a conformance rule that is hard to express to
  become a generated Rust check, creating another execution language instead of extending
  the relational compiler. There is no typed common semantic object/rule model and no
  declared compilation-pass contract covering inputs, outputs, invalidation, diagnostics,
  determinism, and tests.

  Result schemas expose the immediate authority gap. Design §3.2 places result-schema
  specs in the Schema Contract IR, while WP13 names the query-form contract/driver and
  never adds authoritative result-field records to the Schema Contract IR. The current
  query-form contract owns request fields, output roles, and canonical ordering, but not
  result-field logical types, domains, nullability, or metadata. Execution would have to
  invent the authority the packet is meant to compile.
- **Required resolution:** Add one typed `CompiledOntology`/`SemanticContractModel`
  intermediate with closed variants for terms, domains, schemas, projections, predicates,
  relational constraints, phrase bindings, result roles, and metadata consumers. Compile
  its relational rules to qualified DataFusion `Expr`/`LogicalPlan` and its global plan
  policy to the F-003 analyzer rule. Add reusable named result-row schemas to the Schema
  Contract IR (or another explicitly approved single authority), and have query forms
  reference schema IDs. Register one governed `SchemaContractCompilation` pass with the
  complete affected-output closure. Do not replace typed relations with an untyped JSON
  expression language.
- **Revalidation:** `just model-repro-check && just query-form-contract-check && just governance-scan`

### F-005 — WP15 overclaims `StatisticsRequest` and underclassifies PK `Constraints`

- **Severity:** Major
- **Category:** Library grounding / proof
- **Scope:** Design D-01, D-06, PR-4, PR-6; plan WP10 and WP15
- **Finding:** At DataFusion 55, `StatisticsRequest` is a vocabulary threaded from a
  custom optimizer's `TableScan` through `ScanArgs`; DataFusion itself neither populates
  nor consumes it. `ScanResult` only contains an execution plan. WP15 names no request
  producer, request-aware physical plan, `StatisticsContext` consumer, or production
  outcome, so its request-response checks can pass while affecting no query.

  Conversely, PK `Constraints` are not merely advisory with a future consumer. The
  default 55 optimizer includes `EliminateJoin`, which uses uniqueness/functional
  dependencies to remove unused sides of joins. False dimension constraints can therefore
  change results. DataFusion does not enforce those constraints on writes, so CodeFabric's
  ingest/publication uniqueness checks must succeed before the constraints are exposed.
  PR-6 is an integration/equivalence proof, not capability discovery.
- **Required resolution:** Either retain the current declined `StatisticsRequest` posture
  and limit WP15 to truthful physical-plan statistics, or add the complete custom
  producer-to-consumer feature and prove no planning-time I/O. Classify PK constraints as
  **planner-consumed, application-validated, not DataFusion-enforced**. Add duplicate-key
  fault injection and optimized-versus-unoptimized decorated-view equivalence. Ensure the
  dependency graph makes WP15 follow WP11's uniqueness/conformance closure.
- **Revalidation:** `just provider-statistics-contract-check && just ontology-dimension-check`

### F-006 — Packet dependencies contradict the stated milestone ordering and parallelism policy

- **Severity:** Major
- **Category:** Dependency closure
- **Scope:** Plan WP03, WP06, WP09, WP13–WP15, M02–M04, and §8
- **Finding:** The prose and diagram require WP09 after M02, but WP09 depends only on
  WP07, so it may begin before WP08. The plan says WP13, WP14, and WP15 run after M03, but
  their declared dependencies are only WP05/WP07, WP07, and WP01/WP02 respectively. A
  dependency-respecting executor can therefore start Stage 3–5 before the ontology plane
  and suite reconciliation. The plan also calls WP03/WP05/WP06 parallel and disjoint while
  acknowledging that WP03 and WP06 both write `schema_registry.rs`; a “coordinated merge”
  is not a dependency or isolation contract.
- **Required resolution:** Add WP08 → WP09 and an explicit M03 barrier (for example WP16)
  to WP13–WP15. Encode serialization for every fingerprint-moving candidate and owner
  acceptance. Either make WP06 depend on WP03, move the shared helper into WP03, or define
  an actual isolated-worktree/merge owner. Add a topological-order oracle that enumerates
  legal readiness states and proves the stage barriers rather than trusting the diagram.
- **Revalidation:** `just plan-dependency-check`

### F-007 — Stage 2b activates before its executable ontology is complete, and dimension refresh is undefined

- **Severity:** Major
- **Category:** Transition / atomicity
- **Scope:** Design §5.2 Stage 2b and D-01; plan WP09–WP12, M03, and risk 2
- **Finding:** The design defines the ontology-plane release as dimensions, serving
  decoration, referential/conformance checks, the span decision, and spec amendments.
  WP09 instead performs workspace republish and owner acceptance before WP10 serving,
  WP11 validation, WP12's selected shape, and WP16 reconciliation. The struct branch of
  WP12 then creates another schema release. This exposes an accepted intermediate bundle
  that the design does not authorize and weakens the application-owned multi-table
  activation boundary.

  The dimension lifecycle is also incomplete. The existing `enum_catalog` pattern seeds
  an empty table once. “Populated at publication” does not specify how a non-empty
  workspace reconciles a later registry-bundle digest, avoids rewriting unchanged
  vocabulary on every fact publication, or recovers idempotently from a partial
  multi-table refresh. Delta commits are table-scoped; CodeFabric's candidate manifest and
  pointer must coordinate the release.
- **Required resolution:** Let WP09 construct candidate dimensions, but defer active
  republish and owner acceptance until WP10, WP11, the selected WP12 branch, and WP16 are
  complete. Activate one Stage 2b manifest/version map, or explicitly revise the design
  and prove the safety of a separate span release. Materialize dimensions at workspace
  bootstrap or registry-bundle change, reuse exact versions while the bundle is unchanged,
  and reconcile changed dimensions idempotently before the publication pointer advances.
  Fault injection must leave the prior active version map unchanged.
- **Revalidation:** `cargo nextest run --no-fail-fast --no-tests=fail -E 'test(stage2b_atomic_activation_fault_injection) | test(dimension_version_stability_across_fact_publications)'`

### F-008 — Probe observations are treated as committed deterministic decisions

- **Severity:** Major
- **Category:** Proof quality / governance
- **Scope:** Plan WP01, WP02, WP12, and §9 risk 3
- **Finding:** WP02 says executable tests write observed verdicts into a committed fixture,
  downstream packets consume that fixture, and a second run reproduces identical verdicts.
  This conflates three different things: reproducible test semantics, derived observations,
  and accountable branch decisions. A normal test must not rewrite the worktree. Observed
  optimizer shapes, provider behavior, and especially PR-7 performance are also not
  appropriately proved by exact fixture identity. The plan later says the branch decision
  is state judgment, which contradicts the committed-output mechanism. WP01's perf oracle
  similarly requires medians to reproduce inside one interval without defining host/noise
  controls.
- **Required resolution:** Make probes non-mutating and emit ephemeral structured evidence.
  Commit independently justified expected semantics, not a transcript of current output.
  Record the bound decision/fallback through an accountable plan-state or versioned
  decision transaction after reviewing the evidence. Downstream packets consume that
  decision with pin/freshness checks. Give PR-7 the existing statistical comparator and
  environment identity; do not require byte-identical or verdict-identical performance.
  PR-2 must exercise the exact production provider seam described in F-011.
- **Revalidation:** `just probe-suite && git diff --exit-code`

### F-009 — Logical span identity is made contingent on a physical-storage probe

- **Severity:** Major
- **Category:** Target design / doctrine
- **Scope:** Design D-03 and TI-4; plan WP12
- **Finding:** The design correctly recognizes a source span as one cohesive,
  presence-coherent semantic object, but a failed Delta round-trip/pruning probe causes the
  flat representation to be classified “relational-by-constraint.” That changes semantic
  classification in response to physical storage/performance behavior, contrary to the
  logical/physical separation in P6 and the staged-compilation doctrine. Two flattened
  columns are not an independently meaningful relation merely because the preferred
  physical lowering is unavailable.
- **Required resolution:** Keep one logical `SourceSpan` group/object in the Contract IR in
  every branch. PR-3 selects only its physical lowering: native `Struct`, or flattened
  columns plus generated reassembly and all-or-none validation. Both lowerings must expose
  the same logical result contract and checksum semantics. The structure classifier must
  not change when the storage probe changes.
- **Revalidation:** `just structure-classification-check`

### F-010 — The baseline narrative predates completed WP08 and lacks a current activation guard

- **Severity:** Minor
- **Category:** Provenance / program interaction
- **Scope:** Plan frontmatter, §1.3, §2 declared inputs, and activation choreography
- **Finding:** The plan records baseline `eebb958` plus “in-flight wave 8 work.” The
  current tree is `eb7a738`; Waves 8–12 now records WP08 complete, M01 complete, and WP09
  current/not started. Scoped implementation drift from the declared baseline is small,
  but the program-control facts that justify interruption have changed. Neither the active
  pointer nor a dynamic outgoing-state precondition is part of a named activation check.
- **Required resolution:** Before approval, update the provenance narrative to the
  completed-WP08 state and choose a current clean proving baseline (normally `eb7a738` or
  its reviewed successor). Do not freeze a mutable state-file digest into the plan;
  instead add an activation precondition that checks the active pointer, schema-v2 state,
  WP08/M01 completion, and that no Wave 9 packet has begun before writing the interruption
  deviation and switching the pointer.
- **Revalidation:** `just plan-status && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py --plan docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md artifacts-check`

### F-011 — PR-2 does not prove the production delta-rs provider construction path

- **Severity:** Minor
- **Category:** Library integration
- **Scope:** LD-03, PR-2, WP07
- **Finding:** The pinned delta-rs revision exposes `DeltaScanConfig::with_schema`, but the
  current `TableProviderBuilder::build` path constructs its own configuration and exposes
  no schema override. Direct `DeltaScan::new` reaches the configuration but loses the
  builder's production integration; downstream code cannot call its crate-private
  `with_log_store`, and CodeFabric currently relies on the builder/session path for
  object-store registration and exact-version behavior. An isolated FSB presentation
  probe could therefore pass without establishing a usable production substitution.
- **Required resolution:** Keep the existing application-owned reattachment provider as
  the default. PR-2 may select direct `DeltaScan` only after a full provider-contract
  probe covers session/object-store registration, exact snapshot identity, protocol
  checks, projection, filters, limit, statistics, and plan/result equivalence. Record that
  as an operation-selection decision, not as a type-only capability result.
- **Revalidation:** `cargo nextest run --no-fail-fast --no-tests=fail -E 'test(delta_scan_schema_override_full_provider_contract)'`

## Target-Design Assessment

The design's core reversals are justified:

- replacing anonymous `id16` with per-domain logical types is appropriate while the
  project is pre-production, provided F-003's enforcement is real;
- moving governed vocabulary into `cpg_ontology` is cleaner than leaving it beside facts;
- retaining Delta `Binary` as storage while presenting FSB/extension types in Arrow is
  the correct physical/logical split at the pinned kernel;
- retaining typed application plans and rejecting a second graph authority is sound;
- generating row/result shapes and keeping Arrow batches through execution materially
  reduces drift;
- retaining application-owned manifest/pointer activation is necessary because Delta
  atomicity is table-scoped.

The clean-sheet correction is to extend those ideas, not retreat from them. The strongest
coherent target is:

| Layer | Unified typed authority/output | Principal consumer |
|---|---|---|
| Authored contracts | existing owner-specific YAML/JSON authorities | model compiler |
| Semantic object model | one `CompiledOntology` with closed Rust variants | every generator |
| Queryable ontology | normalized terms, edges, bindings, schemas, rules, identities | DataFusion catalog |
| Logical execution | generated `Expr`/`LogicalPlan` plus one `AnalyzerRule` | validation, decoration, queries |
| Physical realization | Arrow schemas, extension fields, batches, streams | DataFusion execution |
| Durable state | Delta tables and application manifest/version map | immutable snapshot catalog |

This uses the same entity/relation/fact idea recursively for the ontology itself. It does
not collapse meaningful types into one JSON payload, duplicate the authored authorities,
or create a competing graph model. Specialized algorithms may still use petgraph or
custom Rust, but their inputs and outputs should be Arrow relations and their derived
results should return to the canonical fact fabric.

## Library Capability Assessment

### DataFusion 55 and Arrow 59

- `SessionStateBuilder::with_analyzer_rule` is available and `SessionState::optimize`
  analyzes native logical plans before the optimizer. It is the appropriate universal
  DataFusion seam for F-003, provided the rule is idempotent because current serving code
  may optimize more than once.
- `MemoryExtensionTypeRegistry` requires `ExtensionTypeRegistration` factories and
  resolved `DFExtensionType` objects. Arrow `ExtensionType` implementations alone are
  insufficient.
- Cast expressions carry a full target `Field`, so metadata-preserving cast behavior is
  available, but it is separate from the extension registry and must be invoked/proved.
- `EliminateJoin` is in the default optimizer and uses uniqueness/functional dependency
  information. PK constraints are planner-consumed and correctness-sensitive.
- `StatisticsRequest` is intentionally only a vocabulary/transport hook for a custom
  feature. A provider response with no producer and consumer is inert.
- Qualified `DFSchema`, typed `Expr`, ordinary joins/anti-joins, set operations,
  aggregations, and analyzer rules are sufficient for the recommended unified relational
  rule compiler. No SQL-string or UDTF detour is needed.

### delta-rs at `43a0cf10…`

- Binary storage plus application-owned Arrow re-presentation remains the safest ID seam.
- `DeltaScanConfig::with_schema` exists below the current builder integration and must not
  be adopted from a type-only probe.
- Each dimension table has its own Delta transaction log. Exact multi-table visibility,
  idempotent retry/recovery, and final activation therefore remain CodeFabric manifest and
  pointer responsibilities.
- Stable dimension versions should be reused while their registry bundle is unchanged;
  registry-release reconciliation, not every fact publication, owns new dimension writes.

No dependency upgrade or new library is needed to resolve the findings.

## Work-Packet and Impact Assessment

The plan has unusually strong packet-local oracle catalogs and named decommission batches,
but its dependency graph is not dependency-closed. F-006 and F-007 require a revised DAG
and release owner. F-001–F-005 also add or reshape impact in:

- `contracts/schema/schema-contract-ir.json` and the model compiler's typed decoder;
- the transformation-pass registry and generated-output affected closure;
- ontology dimension/bridge table specs and builders;
- DataFusion session analyzer registration and all plan ingress tests;
- result-row schema authority and query-form references;
- publication candidate validation and Stage 2b activation;
- metadata classification/consumer census;
- statistics/constraints tests and documentation.

The plan's known-touch lists must be updated accordingly. Parallel packets must either
have disjoint files or an explicit isolated-worktree and merge-owner contract; prose
coordination is insufficient.

## Legacy, Transition, and Decommission Assessment

DB01–DB05 cover the named code/schema retirements well. Three transition obligations are
missing:

1. retirement of list-valued ontology memberships after normalized relations land;
2. retirement of path-local domain validation after the universal analyzer is proven;
3. retirement or explicit retention of the copied WP01 home-module tests after their
   promoted integration oracles become authoritative.

Stage 2b must have one candidate, one complete validation dossier, and one pointer
advance. Prior bundle activation must remain possible until acceptance, and failed
dimension/span/conformance validation must leave the current pointer untouched.

## Proof and Validation Assessment

The plan correctly demands behavioral, structural, negative, and operational evidence per
packet. It also correctly preserves checksum V1 verification while adding V2. The proof
model needs these corrections:

- a genuine catalog-only self-description traversal, not composition by assertion;
- causal tests showing the new domain analyzer, not existing textual ID-prefix checks,
  causes rejection;
- compile-time negative fixtures for domain and result-schema authority errors;
- normalized ontology-membership parity and FK closure;
- optimized/unoptimized equivalence around constraint-driven join elimination;
- a complete `StatisticsRequest` producer/consumer test or explicit declined posture;
- non-mutating probe evidence plus an accountable decision transaction;
- Stage 2b fault injection around the application pointer;
- logical-equivalence proof for both span physical lowerings;
- a topological schedule oracle for stage barriers.

The full final matrix should remain deferred to plan completion. Packet-local checks should
stay proportional and name the exact behavior each gate proves.

## Doctrine and Anti-Principle Assessment

The plan advances P1/P2/P3/P7/P8/P12/P24/P25 through generation, executable ontology,
typed schemas, and catalog self-description. The findings identify where the draft still
falls short:

- **P1/P3:** separate result/rule mechanisms still permit authority duplication (F-004);
- **P2/P21:** unused statistics requests and partially consumed metadata are theater until
  they have named consumers (F-001, F-005);
- **P6:** span semantics must not change with a physical probe (F-009);
- **P12:** result fields and ontology memberships need complete relational contracts
  (F-001, F-002, F-004);
- **P20:** constraints are correctness-sensitive planner facts, not advisory labels
  (F-005);
- **P11/P23:** Stage 2b must become visible only through one immutable pointer transition
  (F-007).

No finding supports a return to duplicated schemas, anonymous IDs, hand-written literals,
SQL builders, optional vocabulary mirrors, or a separate graph authority.

## Top Required Changes

1. Extend the target design with complete self-description relations and normalized
   ontology memberships.
2. Define one typed `CompiledOntology` plus one governed compilation-pass/affected-output
   contract; make result schemas and relational rules explicit inputs.
3. Replace binder-only domain enforcement with compile-time checks and one generated,
   session-wide DataFusion analyzer rule; specify both Arrow and DataFusion extension
   registration contracts.
4. Correct the DataFusion 55 constraints/statistics posture and its causal tests.
5. Repair the packet DAG and make Stage 2b one validated application-level activation.
6. Separate ephemeral probe evidence from accountable branch decisions.
7. Preserve logical `SourceSpan` identity across alternative physical lowerings.
8. Refresh the baseline/program-state narrative before approval.

## Re-Audit Scope

The next audit may be bounded to:

- disposition of F-001–F-011 in the revised design and plan;
- updated declared-input hashes and current baseline/program-state evidence;
- the revised packet DAG and legal topological schedules;
- the `CompiledOntology`/result-schema/rule authority and transformation-pass contract;
- DataFusion extension/analyzer, constraints, and statistics decisions against the exact
  pinned source;
- Stage 2b dimension lifecycle, fault recovery, and one-pointer activation;
- the new or revised named oracles.

A repository-wide implementation gate is not required for that re-audit; implementation
gates begin after the revised plan is approved and activated.
