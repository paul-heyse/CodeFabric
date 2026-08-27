---
artifact: implementation-plan
plan_id: codefabric-design-principles-full-alignment-review-remediation
version: v2
date: 2026-08-26
status: draft
design_path: docs/designs/codefabric_behavioral_conformance_corpus_design_v1_2026-08-26.md
design_version: v1
review_path: docs/reviews/implementation_review_codefabric_design_principles_full_alignment_implementation_plan_v3_2026-08-25_2026-08-26_v1.md
review_version: v1
baseline_commit: 412af14566393c2379ba4e174387361cea5370e8
working_tree_digest: 0d81d9864b0246a3f407212fd6085c7d4ce755eff21eb31f9d35f9075e3f2c84
state_path: docs/plans/state/codefabric-design-principles-full-alignment-review-remediation_v2_state.json
cutover: true
supersedes_plan_path: docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v1_2026-08-26.md
activation_requires: codefabric-design-principles-full-alignment-review-remediation-v1/M04+WP06
---

# CodeFabric design-principles full-alignment review remediation — implementation plan v2

## 1. Outcome and non-goals

### 1.1 Outcome

This v2 is a targeted successor to v1 after the accountable repository owner rejected
`codefabric-golden-v3.0.0-candidate.1`. The candidate's real-provider execution and integrity
chain remain useful evidence, but its expected-vs-candidate comparison is self-authorizing and
does not prove intended semantic behavior. At M05, the four findings in the independent
implementation review are closed in production behavior and in first-principles executable
proof:

1. **IR-001:** the public CodeFabric 1.3 semantic-query contract is one generated,
   form-specific tagged union for all eight QRY forms. Its registry slugs, Rust ingress,
   public JSON Schema, internal `PlanSpec`, Python presentation resource, wire capability
   projection, runtime support advertisement, and conformance fixtures are exact projections
   of one governed authority. All eight forms execute their normative semantics through a
   typed DAG: DataFusion built-ins own relational selection, projection, join, set, aggregate,
   sort, and source-context plans; the application graph plan owns traversal, path, and
   conjunctive-pattern semantics; Arrow schemas, `RecordBatch`es, and bounded streams connect
   both families.
2. **IR-003:** every allocated execution identity reaches one durable, phase-accurate terminal
   journal record before a governed terminal event is emitted. The record preserves every pin,
   logical/optimized/physical artifact, and metric available before success, failure,
   cancellation, stream drop, result insertion failure, or artifact-store failure. A primary
   payload-store failure falls back to the operational journal without diagnostic
   re-execution.
3. **IR-004:** every terminal checkpoint in all sixteen released edit scenarios compares the
   incremental serving session with a genuinely independent zero-generation rebuild that
   re-walks inventory, recaptures current bytes, runs the same required provider and semantic
   lanes, publishes to independent Delta roots, activates independent serving snapshots, and
   proves AC-G-79 schema and duplicate-sensitive bag equality.
4. **IR-002:** Gate B executes one coherent production vertical from Python and Rust source
   capture through the real providers, reconciliation, candidate validation, Delta
   publication, snapshot activation, production UDS query service, FastMCP STDIO adapter,
   streamed response, and result-artifact readback. The eleven required planes contain actual
   produced identities, rows, versions, events, bytes, checksums, diagnostics, and artifacts;
   no candidate plane is an expectation clone or descriptor assertion. Independently authored
   behavior claims, a small production-independent reference evaluator, public-boundary
   observations, causal interventions, and mandatory semantic mutant killing determine whether
   those outputs are correct.
5. A new accountable-owner decision accepts or rejects a new functional Gate B candidate.
   Acceptance publishes a superseding corpus version without rewriting any prior corpus;
   rejection returns to the owning specification or implementation packet. The rejected v3
   candidate remains immutable and cannot be promoted.
6. The affected documents under `docs/upfront_design/` are reconciled at closeout. Intentional
   implementation improvements are incorporated into the owning design authority only after
   accountable review; accidental deviations that weaken required behavior are fixed in code
   and are not normalized into documentation. A fresh independent implementation review must
   approve the corrected implementation before M14-equivalent certification is restored.

This corrective plan supersedes v1's unfinished WP07/WP08 path while preserving v1 and its
schema-2 execution state as history. WP01–WP06 and M01–M04 are carried forward only after their
proving commits and current checks are revalidated. If this draft is audited and approved,
activation creates the future `state_path` atomically and replaces the active pointer; until
then v1 remains active and its recorded rejection is authoritative.

### 1.2 Non-goals

- No movement from the authoritative FAB §2.1 dependency baseline: DataFusion 55.0.0,
  Arrow/Parquet 59.2.0, `object_store` 0.13.2, and delta-rs revision `43a0cf10` remain fixed.
- No custom DataFusion UDF, `LogicalPlan::Extension`, `ExecutionPlan`, `PhysicalExpr`, or query
  planner. Transparent built-in `Expr`/`LogicalPlan` nodes are the relational ceiling; graph
  semantics remain an inspectable application plan family.
- No petgraph pin or feature expansion. The existing exact 0.8.3 pin with only `std` supplies
  the required `Graph`/`DiGraph`, visitor, adaptor, SCC, and topological surfaces. Do not enable
  `serde-1`, `rayon`, `graphmap`, `stable_graph`, `matrix_graph`, `dot_parser`, `generate`, or
  `unstable`; do not persist petgraph representations or treat DOT as a contract artifact.
- No new Cargo root, root workspace, second top-level Rust integration-test target, native
  Python extension, Python Arrow/DataFusion processing layer, Flight, Substrait, or ADBC.
- No tenancy, masking/classification, advisory display metadata, or other divergences already
  excluded by the accepted remediation proposal.
- No rewrite of the v3 plan, v3 state, prior status/review artifacts, owner-accepted Gate B v2
  corpus, or prior acceptance records. They remain immutable history and may be referenced only
  as superseded evidence.
- No broad redesign of identity, checksum, provider, publication-integrity, error-registry,
  model-compiler, RPC-hardening, or adapter surfaces that the review found conformant. They
  receive regression gates and change only when a direct corrective dependency requires it.
- General `cargo-mutants` campaigning is not a completion gate. The behavior design does require
  a bounded, deterministic **semantic mutant matrix** implemented in the conformance harness:
  every registered claim axis has a known-wrong counterfactual or producing-seam fault and every
  required mutant must be killed. Miri remains conditional on an unexpected introduction of
  first-party unsafe code, which this plan prohibits.

### 1.3 Baseline and historical disposition

The independent review and predecessor v1 use committed review point
`412af14566393c2379ba4e174387361cea5370e8`. The v2 replan was made at
`a3efb30d699f84a0d6f190a5ff3c2574bfcf039e`; current source and executable behavior outrank
historical packet/state labels.

The tracked working-tree digest records only the v1 execution-state rejection at planning time.
The new behavior design and this plan version are new planning artifacts and are not production
proof. The executor must preserve unrelated owner changes discovered by fresh preflight.

The v3 design-principles state/status and remediation-v1 state remain historical records of what
their then-selected oracles proved. This plan does not rewrite them. Its own future execution
state and later versioned status/review artifacts record the corrected truth.

## 2. Source design and declared inputs

The accepted source design is the behavior-first conformance dossier v1. It retains the valid
production implementation decisions in remediation proposal v2 and reopens only the golden-
authority, causal-proof, human-review, and release-decision surfaces rejected by the owner. QRY,
SUITE, FAB, LIFE, GEN, SRV, ONT, and RM remain normative; library principles and references
constrain implementation mechanism without overriding domain semantics.

| path | sha256 |
|---|---|
| docs/designs/codefabric_behavioral_conformance_corpus_design_v1_2026-08-26.md | 7fb443540bbdedcc2a556894e7fa5d579ef0ae61a93de3d7b9aaa6838eba8072 |
| docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v1_2026-08-26.md | 5eee11005b44c29b9f9918d19e182b800aadb9b79db89f93ff672e537123a9ed |
| docs/reviews/implementation_review_codefabric_design_principles_full_alignment_implementation_plan_v3_2026-08-25_2026-08-26_v1.md | 0c87e3e7403cbe7cf5ad3c1bd6913599639e33bfd3e2e6d5bcb87476ba58e9ba |
| docs/reviews/design_principles_remediation_proposal_2026-08-25_v2.md | 9c0fc5067fc6f845082e6425c9eca0baff39f7883c9e4e4e21779cedda760674 |
| docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v3_2026-08-25.md | f2b4a383c881e4c8e7e7d82755966b98d402b5392f4f70cc94c49a5ce136024f |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 0f117b1c248711a9bf9e76ea6e8e78dbed851ffd42f53227e0604a52336955ef |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | 9c7780c8e23b61ce8791f7b9fdb9d82c5e4a6df2cb67d6337ded06dc74910b3e |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | 5a19a908db15dbf72fa6454f9a712944efc497c1e7d9f166ffbd9023558f1d3a |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 83c7f0ecc6ab81ef97cdc21f7087a56b870cf4e18e16902870d045f23f747b45 |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | f892b6a18fa07e914ff3829937bd6bdfcb7632b4abebfed2dec51c0fa7a09647 |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | a23d4bf821cb819d43c406ccbbc67aee762621cf494b2ada9f94623a173955e7 |
| docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | a8b9f41eee4e8ca6d29cf3cab85fdedaa83aa300b5d2e373f9344a242f194b6d |
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | 2b97f278d112ab1d7b4d5f40746f86832720edb853d2a9be8576353475d77376 |
| contracts/registry/enum-registry.yaml | 2da365e53b225fa11c969d70d7507025a635e9e59557abb4f7324dfb43d779fa |
| contracts/registry/phrase-registry.yaml | 8f317e433b4badd53dc9b58c3f5f1985949bef744e27cd8bd761bede6a797ce4 |
| contracts/schema/schema-contract-ir.json | 97593be6f5ace34fbf16b312c8ae2d26e64dce4100b592e4fd0949772570959d |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/petgraph.md | 8f5b19b2d9fbb9dfe2caf974b2a1f4c55b9244cfd167eb48956d225a076cccd9 |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |

### 2.1 Data-fabric and extension decisions

1. **Query authority and compilation.** QRY §§13–20 and 106–107 are the semantic
   authority. A governed application model declares the form-specific public variants,
   typed prior-result roles, semantic phrase slots, output contracts, and capability
   relationship. The model compiles to the public request schema and bound `PlanSpec`; it is
   not a DTO that consumers unpack and reinterpret. This advances Principles 1–3 and uses
   `MOD-01`–`MOD-08` and `LOG-01`–`LOG-07`.
2. **Highest viable extension.** DataFusion built-in `Expr`, `LogicalPlanBuilder`, set/join/
   aggregate/sort nodes, `MemTable`, optimizer, and snapshot-scoped `SessionState` own every
   relationally expressible operation. The graph plan owns only semantics DataFusion does not
   natively model: directed/distance-bounded traversal, ordered path policies, and named
   conjunctive graph bindings. Graph outputs cross back as Arrow batches. This advances
   Principle 14 (highest-level extension) and Principle 15 (optimizer visibility), selects
   `EXT-02`/`EXP-01`/`EXP-02`, and explicitly rejects `EXT-04`–`EXT-10`.
3. **Petgraph physical topology.** The accepted application graph plan compiles into one
   immutable, query-local petgraph `DiGraph` projection derived from DataFusion-filtered Arrow
   relation batches. Canonical entity/fact identities and Arrow rows remain authoritative;
   petgraph `NodeIndex`/`EdgeIndex` values are private physical handles. `Graph` preserves
   distinct parallel fact edges, while `EdgeFiltered`, `Reversed`, narrow visitor traits,
   topological sorting, and SCC routines displace bespoke topology mechanics only where their
   semantics match QRY. Path enumeration and conjunctive pattern matching remain bounded
   application algorithms over those traits when petgraph's built-ins lose fact-edge witnesses,
   cancellation, non-induced semantics, or multigraph identity.
4. **Canonical data plane.** Generated Arrow schemas, arrays, `RecordBatch`es, and bounded
   `SendableRecordBatchStream`-style boundaries remain the tabular fabric. Query-block
   composition does not introduce row DTO, pandas, or JSON materialization inside the Rust
   execution plane. This maintains Principles 7, 8, and 22 with `ARR-01`–`ARR-08`, `SCH-09`,
   and `INT-08`.
5. **Delta publication evidence.** Gate B and clean rebuild use the existing mutation and
   publication authorities. They persist application transactions and commit metadata,
   validate schema/protocol/constraints, record exact Delta versions, construct exact-version
   providers, and pin them through a `ServingSnapshot`; they never treat a single-table
   delta-rs snapshot or a candidate descriptor as CodeFabric current-state authority. This
   maintains Principles 9–13, 18, 19, and 23; FAB §§12, 70–71, 91, 98.1–98.2; and delta-rs
   reference §§3.10–3.15, 5.13–5.17, 6.24–6.25, 7.1, and 7.6–7.7.
6. **Terminal artifact durability.** The SQLite operational journal becomes the sole terminal
   execution-envelope authority. The immutable content-addressed result/plan payload store is
   a referenced projection. On a primary payload-write failure, the journal stores a bounded
   canonical fallback envelope and failure identity before terminal notification. This avoids
   two competing authorities while satisfying Principles 3, 9–11, 16–19, 23, and 24 and
   `OBS-01`–`OBS-12`.
7. **Golden-answer authority.** Produced candidate bytes are implementation output, never
   authority. Only a versioned accountable-owner acceptance record can promote their exact
   digest into a new immutable released corpus. This maintains Principles 3, 13, 20, and 25
   and SUITE Gate B.

### LD-PG-01 — Petgraph query-local topology kernel

**Decision:** adopt.

**Version basis:** the existing exact `petgraph = 0.8.3` pin with
`default-features = false` and `features = ["std"]`; no manifest change is expected.

**Displaces:** the bespoke `[u8; 16]` → adjacency `BTreeMap`, node-only shortest-path helper,
manual direction copies, and duplicated topology/cycle mechanics in the semantic-query path.
It deliberately retains CodeFabric-owned typed `GraphOperatorPlan` semantics, bounded
edge-witness traversal, all-shortest/all-simple path enumeration, conjunctive matching,
canonical output ordering, coverage classification, cancellation, and Arrow encoding.

**Risk:** petgraph shortest-path APIs return distance or node-path outputs that cannot identify
ordered fact edges in a multigraph; its isomorphism family assumes non-multigraph, induced
subgraphs; and several whole-graph algorithms have no cancellation callback. Mitigate with a
parallel-edge-capable `DiGraph`, private external-ID maps, DataFusion prefiltering, adaptors,
trait-generic bounded CodeFabric algorithms, prevalidated non-interruptible work limits, and
explicit negative structural tests against invalid API substitutions.

**Validation:** `just semantic-query-conformance-check`.

### 2.2 Full-principle posture

- **Advances:** Principle 1 (model semantics), P2 (executable models), P3 (one authority),
  P4–P6 (graph capability hierarchy, contained implementation variability, and semantic/
  physical separation), P9–P10 (intrinsic and closed provenance), P14–P20
  (extension level, optimizer visibility, lifecycle, artifacts, fingerprints,
  reproducibility, truthful claims), and P24–P25 (semantic observability and
  contract-derived testing).
- **Maintains:** P7–P8 (shared Arrow fabric), P11–P13 (immutable state, executable schema,
  boundary governance), and P21–P23
  (metadata classes, protocol boundaries, local state ownership). Passing provider and schema
  contracts are regression-protected rather than reimplemented.
- **Risk — mitigated:** P11/P16/P23 risk from cancellation and fallback persistence is bounded
  by one journal owner and explicit phase state; P20 risk from staged support is bounded by
  runtime executor registration and pre-snapshot advertisement; P14/P15 risk from using
  petgraph as an opaque relational engine is bounded by DataFusion prefiltering and an
  inspectable physical graph plan; P25 risk from self-confirming goldens is bounded by
  independent production output, fault perturbations, and accountable acceptance.

## 3. Global target invariants

- **GI-01 — one query-language authority.** Public form slugs and codes remain owned by the
  enum registry; the new form-contract model owns field/role/result semantics. Public JSON
  Schema, internal `PlanSpec`, generated Rust/Python types or resources, query-language bundle,
  capability output, and fixtures are derived and digest-linked. No projection redefines the
  form contract.
- **GI-02 — form-specific legal states.** The public query clause is a tagged union whose
  variant constructors make every required field, mutually exclusive mode, and bounded path/
  set/pattern policy explicit. A generic `label`/`input`/`where` catch-all cannot stand in for a
  normative form.
- **GI-03 — typed semantic DAG.** Parse → type → resolve/bind → validate policy → compile →
  optimize → execute → verify/encode remains explicit. Prior-result edges are role-typed;
  fan-in/fan-out are supported; cycles, role mismatches, unbounded path requests, and
  evaluative intent fail before snapshot work or physical execution as specified.
- **GI-04 — DataFusion/Arrow-first execution.** Relational semantics use transparent
  DataFusion built-ins and remain optimizer-visible. Graph semantics use the typed application
  graph plan compiled to a query-local petgraph physical projection only after pushable
  relation/scope/certainty predicates have been lowered into DataFusion. Both families accept
  and emit declared Arrow schemas and bounded batches; no SQL string, opaque UDF, custom
  DataFusion logical/physical node, or Python data-plane execution appears.
- **GI-05 — truthful support.** The daemon advertises the intersection of governed forms and
  production-registered executors. A form is absent until its schema, positive/negative
  conformance, output contract, and production UDS row pass. Unsupported is reported before a
  serving-snapshot lease or Delta provider is acquired.
- **GI-06 — factual result semantics.** Traversal preserves direction, distance,
  relationship family, exact/may/unknown certainty, and witnesses; paths preserve ordered
  entity/fact IDs and policy; patterns preserve named bindings and every conjunctive fact;
  set/summary/context operations preserve identity, provenance, grouping, exact source range,
  and coverage. Empty rows never imply absence without closed-world coverage.
- **GI-07 — terminal provenance closure.** Every allocated execution identity has one durable
  terminal journal row with explicit lifecycle stage, all available pins and artifacts,
  result/payload status, public error, and retention state. `NotReached` is distinguishable from
  `ReachedWithoutMetrics` and `Partial`.
- **GI-08 — one served physical execution.** Plan metrics are taken from the physical plan
  instance that produced or partially produced served rows. Failure diagnostics never invoke
  `EXPLAIN ANALYZE`, rebuild the plan for observation, or execute the query twice.
- **GI-09 — production Gate B.** All eleven SUITE Gate B planes are observations of one
  correlated execution and contain actual canonical artifacts. At minimum the provider plane
  proves Tree-sitter, Ruff/Python ownership, and rustc-MIR ownership; applicable sidecar
  capability is executed or explicitly unavailable with a governed reason.
- **GI-10 — true rebuild.** Each clean side starts with new zero-generation engine,
  operational store, hot overlay, candidate cache, serving snapshot, and Delta roots; performs
  authoritative inventory and current-byte capture; runs the same provider/capability policy;
  and compares independent serving sessions through AC-G-79.
- **GI-11 — Delta identity discipline.** Publication and query identity use the CodeFabric
  publication/`ServingSnapshot` plus exact Delta version map. Delta history and commit
  properties provide durable joins; provider/cache/checkpoint objects remain reconstructible
  and non-authoritative.
- **GI-12 — non-vacuous proof.** Every acceptance sentence has a selected named oracle that
  reaches production code with `--no-tests=fail`, plus at least one adverse mutation/fixture
  that proves the oracle rejects the defect class. Test code may not construct both expected
  and actual values from the same object. Non-empty lists, `matches: true`, shared references,
  and digest equality cannot satisfy a semantic assertion.
- **GI-13 — immutable history and honest closeout.** Prior plans, state, corpora, and
  acceptance records remain byte-stable. Intentional design improvements receive an owning
  design edit and accountable decision; unintended semantic deviations are fixed. Final
  certification requires a new independent implementation review.
- **GI-14 — derived graph projection discipline.** The physical graph is a bounded immutable
  `DiGraph` derived from one pinned serving snapshot and its DataFusion-filtered Arrow batches.
  Distinct fact IDs remain distinct parallel edges; external canonical IDs map to private
  graph-local handles; no `NodeIndex`, `EdgeIndex`, petgraph serialization, graph cache, or DOT
  text crosses a public, durable, Arrow, or provenance boundary. Projection identity includes
  snapshot, input-plan, schema, filter, node/edge census, index-width policy, and petgraph
  version fingerprints.
- **GI-15 — independently authored meaning.** Gate B expected semantics are strict typed claims
  authored from minimal source fixtures and normative rules before execution. Claims use source
  anchors and semantic selectors, not literal runtime IDs, captured production rows, candidate
  digests, descriptor identities, or registry census.
- **GI-16 — independent reference laws.** Query/path/set/group/order, transition, cardinality,
  coverage, and negative-proof consequences are evaluated by a small test-only standard-library
  implementation that cannot import or call production providers, reconciliation, identity,
  query, DataFusion, petgraph, lifecycle, candidate, or release code.
- **GI-17 — public semantic equivalence.** Canonical tables, production UDS result and stream,
  persisted artifact readback, and explicit FastMCP STDIO structured output resolve to the same
  independently expected semantic result. In-memory or descriptor-only checks are supporting
  component evidence, never release evidence.
- **GI-18 — causal sensitivity and closed universes.** Every claim axis has a required killed
  semantic mutant or producing-seam fault. Unexpected observations inside a declared closed
  universe fail; absence/COMPLETE/PROVEN_EMPTY pass only with explicit complete coverage.

## 4. Work packets

Each packet is complete only when every named acceptance check passes at its own proving
commit and at current HEAD. The executor must re-run each preflight immediately before a
load-bearing edit because this plan records questions, not a frozen must-touch manifest.

### WP01 — Form-specific query authority and truthful public projections

**Outcome.** One governed form-contract model defines the eight QRY 1.3 variants and generates
the public and runtime-facing projections. The public schema accepts every normative positive
fixture, rejects field/variant misuse, and uses exactly the eight registry slugs. Runtime
advertisement is empty or limited to independently complete executors until later packets
activate forms.

**Dependencies.** None.

**Target invariants.** GI-01, GI-02, GI-05, GI-12; Principles 1–3, 12, 18, 20, 22, 25;
HOL Principles 10–12, 16, 29–31.

**Design and library references.** Review IR-001; proposal R1/R2; QRY §§4.2–4.10, 12–21,
30, 33, 106–107; SUITE AC-G-05; `MOD-01`, `MOD-04`–`MOD-08`, `SCH-01`, `SCH-10`,
`SCH-12`, `GOV-06`, `INT-10`, `TST-01`, `TST-08`, `TST-09`, `TST-14`; DataFusion
schema reference S1–S2/S4/S14 and plan reference §§41, 44, 55–56; Arrow §§3, 10, 28.

**Change surface / Preflight / Known Touch.** Run:

```bash
just spec-outline docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md --match '^(12|13|14|15|16|17|18|19|20|21|30|33|106|107)\.' --view expanded
rg -n 'domain: QUERY_FORM' contracts/registry/enum-registry.yaml
jq '.public_schemas[] | select(.schema_kind == "cpg-semantic-query-request")' contracts/schema/schema-contract-ir.json
rg -n 'SemanticQueryClause|QueryInput|supported_query_forms|cpg-semantic-query-request|planspec.schema' src contracts codefabric-cpg-mcp tooling -g '!**/.venv/**'
rg -n 'retrieve facts"|follow relationships"|find paths"|summarize facts"|fetch source context"' contracts src codefabric-cpg-mcp tooling
```

Known current touch includes `contracts/registry/enum-registry.yaml`,
`contracts/registry/phrase-registry.yaml`, `contracts/schema/schema-contract-ir.json`,
`contracts/query/planspec.schema.json`, `contracts/schema/cpg-semantic-query-request.schema.json`,
the model compiler's schema/registry/aggregate drivers, `src/semantic_query.rs`,
`src/query_service.rs`, the adapter's generated contract resources and `server.py`, and the
public-schema golden instances. A new query-form authority path and generated projections are
permitted; their exact source layout is derived at preflight rather than mandated here.

**Required changes.**

1. Introduce one versioned semantic-query form contract under `contracts/` that references the
   `QUERY_FORM` registry for code/slug authority and declares, per form, required/optional
   fields, nested discriminated variants, prior-result roles, phrase slots, limit/boundedness
   rules, canonical output schema, order, coverage effect, and QRY section owner. The model
   compiler validates complete coverage of all eight registry entries and rejects extra or
   missing forms.
2. Remove the query request and bound-plan clause bodies as independently hand-authored JSON
   Schema authority. Generate both the public QRY-shaped tagged union and the internal
   snapshot-bound `PlanSpec` projection from the form contract. Their different field names and
   phases are explicit derivations, not copied schemas.
3. Generate or compile the Rust ingress variants used by `parse_request`; no runtime parser may
   deserialize the old generic `SemanticQueryClause`. Generate a Python presentation/schema
   projection and exact query-form literals for the adapter, but keep the adapter pass-through.
4. Keep canonical request JSON plus checksum as the sole semantic payload over Protobuf. Do not
   duplicate the complete query AST in Proto. Derive the existing readiness/capability slugs,
   query-language bundle identity, and Python resources from the same authority and assert them
   against the descriptor/wire path.
5. Replace the hand-written support list with a production executor registry whose entries are
   validated against the governed form authority. Until WP02/WP03 prove an executor, that form
   is not advertised; validation rejects it before snapshot acquisition with the registered
   unsupported-capability response.
6. Add QRY-derived positive and negative golden instances: at least one fixture for every
   form-specific field, every legal prior-result role, every required/optional boundary, and
   each wrong-variant/wrong-role/unbounded-path case.

**Legacy Disposition and Decommission.** Hard-cut over to the normative QRY 1.3 contract. The
five shortened public slugs and generic shared clause were implementation defects, not a
compatibility contract; no alias or dual parser preserves them. DB01 owns final zero-state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `qry_v13_form_contract_conformance`; Executable oracle:
`query_form_projection_parity`; Executable oracle:
`qry_v13_connecting_path_schema_falsification`; Executable oracle:
`query_form_contract_operational_gate`.

- **Behavioral — Executable oracle:** `qry_v13_form_contract_conformance` decodes every normative positive fixture
  through public JSON Schema and Rust serde, and rejects every wrong-variant negative fixture.
- **Structural — Executable oracle:** `query_form_projection_parity` proves exact eight-entry equality across enum
  registry, form-contract model, generated public schema, internal PlanSpec, Rust, Python,
  query bundle, and daemon capability slugs.
- **Negative/Zero-State — Executable oracle:** `qry_v13_connecting_path_schema_falsification` proves the IR-001
  `starting_from`/`ending_at`/`through`/`path_policy`/`maximum_length` request now passes and
  proves every retired shortened slug fails; a generated-projection drift fixture fails model
  compilation.
- **Operational — Executable oracle:** `query_form_contract_operational_gate` introduces
  `just query-form-contract-check` before citing it elsewhere; it
  selects the three tests above with `--no-tests=fail`, validates adapter resources, and runs
  `just model-repro-check`.

**Edit-Local Gates.** `just root-fmt`, `just model-family-check schemas`, targeted adapter
schema tests, and the directly affected Rust unit tests.

**Packet-Local Gates.** `just query-form-contract-check`, `just model-check`,
`just model-repro-check`, `just adapter-lint`, `just adapter-type`, `just root-check`,
`just governance-scan`.

**Integration Milestone.** M01.

**Replan Triggers.** QRY form semantics cannot be represented as a tagged union without a
public-contract amendment; generated Rust/Python projection would require either language to
become a second semantic authority; or a Proto payload redesign becomes necessary rather than
continuing canonical JSON bytes.

**Rollback or Recovery.** Before activation, form support stays withdrawn and generated
outputs can be reverted with their one model input. After cutover, roll forward by correcting
the authority and regenerating all projections; do not restore shortened aliases.

**Design-Bearing Contracts and Exemplars.** The governed form contract contains one complete
QRY example per variant and one composed example covering every legal result role. Generated
artifacts carry authority ID/version/digest and must not become hand-edited exemplars.

### WP02 — Typed binding and DataFusion relational form compiler

**Outcome.** Five relationally expressible forms—find entities, retrieve facts, combine result
sets, summarize objective facts, and retrieve source/syntax context—compile through one
application-owned typed lifecycle into transparent DataFusion logical plans over the pinned
serving session. Prior graph/relational results cross as typed Arrow batches; each completed
form is advertised only after its production conformance row passes.

**Dependencies.** WP01.

**Target invariants.** GI-02–GI-06, GI-08, GI-12; Principles 1–3, 5–8, 12–17, 20, 21,
23–25; HOL Principles 11–18, 21, 23, 30–31.

**Design and library references.** Review IR-001; proposal R2; QRY §§4, 12–14, 18–21,
23–33, 106–107; FAB §§72, 91–94, 98, 110; `MOD-02`–`MOD-08`, `ARR-01`–`ARR-08`,
`SCH-02`/`SCH-09`, `CAT-02`, `EXP-01`/`EXP-02`, `LOG-01`–`LOG-07`,
`RUN-03`/`RUN-05`/`RUN-09`, `OBS-01`–`OBS-04`, `TST-06`, `TST-10`, `TST-11`;
DataFusion §§10–12, 19, 21–23, 30, 32 and plan reference §§41–51, 55–56; Arrow §§3,
5–8.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/semantic_query.rs --view signatures
ast-grep run -l rust -p 'LogicalPlanBuilder::from($PLAN)' src
rg -n 'lower_relational_block|resolve_phrase|TypedQueryBlock|BoundQueryBlock|MemTable|table_plan' src/semantic_query.rs src/fabric/serving.rs
rg -n 'PHRASE_ENTRIES|planspec_mapping|allowed_request_forms|allowed_.*_roles' contracts/registry/phrase-registry.yaml src/generated/registries.rs
rg -n 'CombineResults|SummarizeFacts|RetrieveSourceContext' src tests contracts -g '!docs/**'
```

Known current touch includes `src/semantic_query.rs`, `src/fabric/serving.rs`,
`src/query_service.rs`, query contract/phrase registries and generated projections, focused
query fixtures, and justfile selectors.

**Required changes.**

1. Make parse, type, semantic phrase resolution, prior-result role binding, snapshot binding,
   structural policy validation, logical compilation, optimization, execution, and response
   verification separate typed phases. Each error carries its registered phase and source JSON
   pointer.
2. Compile `find code entities` and `retrieve facts about code` from their actual
   `looking_for`/`within`/`about`/`facts`/`at`/`where`/`return` models. Resolve governed phrases
   through `phrase-registry.yaml`; bind them to serving projections, enum codes, identities,
   and typed `Expr`s. An unresolved or coverage-incompatible phrase is explicit unsupported/
   unknown, never name matching.
3. Compile `combine result sets` to DataFusion built-in union/intersection/difference/join
   structures selected by its governed set operation and identity domain. Preserve origin and
   certainty columns. Reject incompatible identity domains before planning.
4. Compile objective summaries to built-in aggregate/group/sort expressions with deterministic
   aliases and support/provenance columns. No subjective label or opaque UDF is allowed.
5. Compile source/syntax context to relational joins/projections over the pinned manifest,
   source-file, source-span, and syntax relations. Return exact file identity, digest, half-open
   byte range, text handling, and requested enclosing context; never return every manifest
   context for an unrelated input.
6. Register bounded graph/previous-block Arrow outputs as query-local immutable `MemTable`s or
   equivalent built-in providers. Retain `SchemaRef`, nullability, metadata, ownership, and
   backpressure; JSON/row conversion occurs only at the public response boundary.
7. Capture bound and optimized logical artifacts and compare optimized/unoptimized results.
   Apply each form's canonical total order before offset/fetch and response encoding.
8. Model coverage, explicit unknown, negative, incomplete, and limited results. Empty relation
   output has an explicit category and cannot prove absence without the QRY §30 prerequisite.

**Legacy Disposition and Decommission.** Retire generic entity/fact ID-only selection as the
form contract, the single count-like summary, unconditional union-to-singletons, and
all-context source retrieval. Graph forms remain truthfully unsupported until WP03; no reduced
fallback executes under their names.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `qry_v13_relational_forms_conformance`; Executable oracle:
`semantic_query_relational_plan_visibility`; Executable oracle:
`semantic_query_relational_policy_and_absence`; Executable oracle:
`semantic_query_relational_operational_gate`.

- **Behavioral — Executable oracle:** `qry_v13_relational_forms_conformance` executes positive and negative QRY
  fixtures for the five forms through `ProductionQueryService` over a real leased serving
  session and asserts exact Arrow/public result schemas and semantics.
- **Structural — Executable oracle:** `semantic_query_relational_plan_visibility` proves every relational block is
  a DataFusion built-in logical plan with expected tables/columns/filters/set/aggregate/sort
  nodes and contains no SQL text, UDF, or custom extension.
- **Negative/Zero-State — Executable oracle:** `semantic_query_relational_policy_and_absence` proves evaluative
  requests, identity-domain mismatches, unauthorized tables/functions, ambiguous phrases,
  uncovered negation, invalid context ranges, and resource overflows fail in the owned phase.
- **Operational — Executable oracle:** `semantic_query_relational_operational_gate` introduces
  `just semantic-query-relational-conformance-check` with
  `--no-tests=fail`; it also runs optimized/unoptimized equivalence at multiple partitions and
  batch boundaries.

**Edit-Local Gates.** Targeted Rust tests for the changed variant/compiler, `just root-fmt`,
and `just root-clippy` after each coherent compiler slice.

**Packet-Local Gates.** `just query-form-contract-check`,
`just semantic-query-relational-conformance-check`, `just query-determinism-check`,
`just root-check`, `just root-test`, `just stable-graph-check`.

**Integration Milestone.** M02 prerequisite.

**Replan Triggers.** A relational form requires semantic behavior not expressible by built-in
DataFusion nodes; a query-local Arrow provider cannot preserve the declared schema/stream
contract; or phrase resolution reveals an unowned semantic vocabulary rather than a missing
registry projection.

**Rollback or Recovery.** Executor registration is form-local. A failing form is removed from
runtime support before rollback; completed form contracts and generated projections remain.

**Design-Bearing Contracts and Exemplars.** Preserve normalized logical-plan exemplars for
each relational form without making plan display text semantic identity. Exemplars name the
governing request and Arrow output contract.

### WP03 — Graph semantics, mixed-DAG scheduling, and complete QRY activation

**Outcome.** Relationship traversal, connecting paths, and named conjunctive patterns execute
their complete QRY semantics through typed application graph plans compiled into immutable,
query-local petgraph physical projections. One scheduler composes all eight forms with arbitrary
acyclic fan-in/fan-out, using Arrow batches at every relational/graph seam. The daemon advertises
all eight forms only after the full production UDS corpus is green.

**Dependencies.** WP02.

**Target invariants.** GI-02–GI-06, GI-08, GI-12, GI-14; Principles 1–8, 11–20, 22–25;
HOL Principles 13–18, 21, 23–25, 29–31.

**Design and library references.** Review IR-001; proposal R2; QRY §§15–17, 21, 30, 33,
106–107; FAB §§72, 83, 91, 94, 98, 110; original plan LD-01/LD-11; `ARR-01`–`ARR-08`,
`MOD-02`–`MOD-07`, `LOG-01`–`LOG-07`, `RUN-05`/`RUN-09`, `OBS-01`–`OBS-04`,
`OBS-11`, `EXT-02`, `TST-06`, `TST-10`, `TST-11`; DataFusion §§19–23, 28, 30, 32;
Arrow §§3, 5–8; LD-PG-01; petgraph §§2.1, 2.9–2.10, 2.13–2.14, 2.17, 4.7–4.11,
10.3, 10.12–10.13, 11.1, 11.5, 11.10, 11.17, 12.4–12.7, 12.16, 12.18–12.21,
13.2, 13.4, 13.7, 13.11–13.19, 14.16–14.20, 15.14–15.22, 16.3–16.5,
16.8–16.10, 16.13–16.16, 16.20–16.22, 19.3, 19.8–19.13, and 19.18–19.22.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'GraphOperatorPlan|GraphEdge|shortest_path|execute_graph_operator|graph_inputs|maximum_depth: 64' src/semantic_query.rs
rg -n 'FollowRelationships|FindPaths|MatchPattern|CombineResults|SummarizeFacts|RetrieveSourceContext' src/semantic_query.rs
rg -n 'petgraph|DiGraph|GraphMap|StableGraph|Csr|EdgeFiltered|Reversed|toposort|kosaraju_scc|tarjan_scc|isomorph' Cargo.toml src tests justfile
rg -n 'BTreeMap::<\[u8; 16\], Vec<\[u8; 16\]>>|VecDeque::from\(\[\(start, vec!\[start\]\)\]\)' src/semantic_query.rs
rg -n 'production_eight_form_semantic_query_conformance|wp75_' src tests justfile
ast-grep run -l rust -p 'LogicalPlan::Extension($A)' src
ast-grep run -l rust -p '$CTX.register_udf($A)' src
```

Known current touch includes `src/semantic_query.rs`, the existing application graph substrate,
`src/fabric/serving.rs`, `src/query_service.rs`, generated response/output contracts, query
fixtures, and the semantic-query/wave-5 justfile selectors. `Cargo.toml` and `Cargo.lock` are
preflight evidence: the exact 0.8.3/`std` pin should remain byte-stable unless the current
feature graph contradicts LD-PG-01 and triggers a replan.

**Required changes.**

1. Reclassify `follow code relationships` as graph semantics. Its logical plan carries governed
   relationship family/kind mask, semantic direction, distance, stop conditions, certainty
   policy, seed role, output role, depth/output/memory bounds, and cancellation contract.
   Returned facts retain originating fact IDs, endpoints, direct/transitive class, certainty,
   resolution, and witness information; petgraph types do not enter this semantic model.
2. Compile the graph plan into an immutable query-local snapshot projection. DataFusion first
   projects and filters the `relations` table by every optimizer-visible snapshot, owner,
   relationship-family, certainty, and static `where` predicate. The resulting validated Arrow
   batches back a sparse directed petgraph `Graph`/`DiGraph` whose nodes carry compact canonical
   entity IDs and whose lightweight edge handles resolve to exact Arrow rows/fact IDs and the
   traversal discriminants required in the hot loop.
3. Build nodes in canonical-ID order and distinct facts in canonical fact order, prevalidate
   node/edge counts, index-width and memory budgets, preallocate capacity, and use fallible
   `try_add_node`/`try_add_edge`. Maintain private external-ID ↔ `NodeIndex` maps, never delete or
   mutate topology after construction, and preserve every distinct fact as a parallel edge.
   `GraphMap`, `Csr`, `MatrixGraph`, `StableGraph`, `update_edge`, or endpoint-pair deduplication
   are not valid substitutions on this path.
4. Express traversal kernels against the narrow petgraph capability traits they need—normally
   `IntoEdgesDirected + Visitable`—so `EdgeFiltered` and `Reversed` views implement governed
   relationship/certainty/scope filters and incoming direction without copying or reversing
   topology. Relationally pushable filters must not be hidden inside an adaptor.
5. Implement path requests over separate starting/ending sets, governed `through` families,
   direction, maximum length, and each accepted path policy. Use cancellation-aware bounded
   CodeFabric BFS/predecessor-edge enumeration for all-shortest and deterministic-shortest
   policies and bounded per-path DFS/backtracking for all-simple policies. Results preserve
   ordered entity IDs, ordered fact IDs, start/end identity, length, per-edge relationship/
   certainty, and a non-flattening certainty summary. Do not substitute `dijkstra`,
   `bidirectional_dijkstra`, `astar`, or `k_shortest_path` unless a focused probe proves the
   exact edge-witness/output contract; their documented distance/node outputs are not sufficient.
6. Implement named conjunctive pattern bindings and relationship clauses as bounded,
   candidate-directed backtracking over filtered edge references. DataFusion supplies
   optimizer-visible candidate-set filters/joins where relationally expressible; the graph
   kernel owns transitive clauses, edge witnesses, alternatives, and coverage-aware negation.
   Petgraph's VF2/isomorphism family is not used for the normative matcher because it assumes
   non-multigraph induced-subgraph semantics. Every returned row maps each binding and optional
   fact binding to canonical identities.
7. For transitive relationship results that require recursion/cycle membership, run a bounded
   SCC operation on the already filtered/traversed projection only after its non-interruptible
   work bound is validated; prefer iterative `kosaraju_scc` over recursive whole-snapshot work.
   Canonically sort component members and never treat arbitrary algorithm output order as
   semantic. If cancellation or scale cannot be bounded around a library routine, retain a
   cancellation-aware application traversal instead of overclaiming support.
8. Replace `BlockValues`-style untyped ID unions with role-indexed Arrow results and typed
   dependency adapters. Represent the query dependency graph with private petgraph nodes/edges,
   use `toposort` for order-or-cycle validation, and use SCC/DFS evidence to report the complete
   request-local dependency cycle required by QRY. The actual scheduler retains a canonical
   ready frontier so independent-node tie order never depends on petgraph insertion or iterator
   order; fan-in/fan-out and response presentation order remain distinct.
9. Feed graph output batches into WP02 relational forms and relational ID/fact batches into
   graph nodes without JSON or pairwise DTO conversion. Validate every batch against its
   generated plan/output schema. Petgraph is never serialized, published, cached as current
   state, exposed over UDS, or used as a durable/provenance representation.
10. Share the session `RuntimeEnv`, cancellation, memory reservations, and limits with
    DataFusion execution. Account for graph nodes, parallel-edge payloads, ID maps, traversal
    frontiers, predecessor edges, per-path visited sets, pattern bindings, SCC workspace, and
    output buffers before and during execution. Poll cancellation in every custom expansion or
    enumeration loop and reject rather than silently truncate outside an explicit result limit.
11. Extend the graph plan artifact with physical projection identity: serving snapshot and exact
    Delta versions, DataFusion input-plan/schema/filter fingerprints, petgraph version/index
    policy, node/edge/parallel-edge census, algorithm/policy identifier, bounds, cancellation
    posture, coverage state, and output schema. DOT and debug formatting are non-contractual and
    cannot be golden or wire evidence.
12. Register each graph executor only after its positive, negative, ordering, coverage,
    differential/reference, and mixed-DAG conformance rows pass. Then require exact eight-form
    equality in daemon readiness, public schema, registry, Python resource, and tests. Preserve
    optimizer visibility: graph semantics stay inspectable in `GraphOperatorPlan`, relational
    subplans stay built-in DataFusion, and petgraph remains a derived physical kernel rather than
    an opaque DataFusion UDF or extension.

**Legacy Disposition and Decommission.** Delete the bespoke canonical-ID adjacency `BTreeMap`,
node-only `shortest_path`, consecutive-ID/fixed-depth BFS contract, one-edge pattern proxy,
untyped `BlockValues` union, and reduced five-form kernel. DB01 proves the public/generic/
reduced query legacy reaches zero. Do not preserve them as fallback implementations beside the
petgraph-backed projection.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `qry_v13_graph_forms_conformance`; Executable oracle:
`semantic_query_mixed_dag_contract`; Executable oracle:
`semantic_query_graph_adversarial_conformance`; Executable oracle:
`semantic_query_graph_operational_gate`.

- **Behavioral — Executable oracle:** `qry_v13_graph_forms_conformance` differentially checks
  the petgraph-backed production kernel against an independent small-graph reference enumerator
  across insertion orders and Arrow batch partitions. It covers incoming/outgoing direction,
  direct/transitive distance, stop conditions, relationship families, parallel facts between
  the same endpoints, exact/may/unknown edges, all accepted path policies, ordered entity/fact
  witnesses, SCC membership, named multi-clause patterns, alternatives, and covered negation.
- **Structural — Executable oracle:** `semantic_query_mixed_dag_contract` proves every form has
  a generated request variant, typed logical plan, executor registration, DataFusion prefilter,
  private query-local `DiGraph` physical projection, Arrow output contract, response encoder,
  capability row, and focused fixture. Structural rules reject escaped `NodeIndex`/`EdgeIndex`,
  petgraph serialization/DOT authority, `GraphMap`/`Csr`/`MatrixGraph`/`StableGraph`,
  endpoint-pair edge collapse, VF2/isomorphism, SQL strings, UDFs, and custom DataFusion nodes on
  this path.
- **Negative/Zero-State — Executable oracle:** `semantic_query_graph_adversarial_conformance`
  rejects dependency and relationship cycles where prohibited, illegal result roles, missing
  endpoints, incompatible relationship families, unbounded cyclic paths, node/edge/index-width/
  memory overflow, cancellation during each expansion family, uncovered negation, loss of one
  parallel fact witness, and insertion-order-dependent output. It proves the retired adjacency
  map and node-only shortest-path helper are absent from current production code.
- **Operational — Executable oracle:** `semantic_query_graph_operational_gate` updates
  `just semantic-query-conformance-check` to select the above plus
  `qry_v13_form_contract_conformance`, relational conformance, and
  `production_eight_form_semantic_query_conformance` through real
  `ProductionQueryService`/UDS with `--no-tests=fail`.

**Edit-Local Gates.** Targeted graph-build, filtered/reversed traversal, path/pattern, SCC/DAG,
parallel-edge, cancellation, and Arrow batch-schema tests; `just root-fmt`; `just root-clippy`.

**Packet-Local Gates.** `just semantic-query-conformance-check`,
`just query-determinism-check`, `just wave5-integration-check`, `just root-test`,
`just governance-scan`, `just stable-graph-check`, `just features-each`.

**Integration Milestone.** M02.

**Replan Triggers.** A QRY graph semantic cannot be expressed by the accepted application
graph-plan family; correct composition would require a custom DataFusion logical/physical
extension; the existing 0.8.3/`std` petgraph surface lacks a planned trait/adaptor/algorithm;
accepted graph scale cannot fit the selected index-width/memory/cancellation envelope; a
long-lived or cross-query graph cache becomes necessary; or a form cannot meet bounded memory/
cancellation without changing public semantics. A pin/feature move or authoritative/persisted
graph state reopens design rather than becoming an implementation-local adaptation.

**Rollback or Recovery.** Withdraw only the affected executor registration before restoring a
previous graph implementation. Never advertise the reduced behavior under the normative form
name.

**Design-Bearing Contracts and Exemplars.** Preserve one inspected graph-plan artifact and
Arrow input/output schema per graph form; one physical-projection artifact showing snapshot,
DataFusion input-plan, graph census, filters, petgraph/index policy, bounds, and output identity;
a parallel-edge path exemplar retaining two distinct fact witnesses; and a mixed eight-form DAG
exemplar whose public request is copied from QRY-shaped generated fixtures. Never use DOT text
or petgraph-local indices as an exemplar identity.

### WP04 — Phase-complete terminal artifact and failure-provenance closure

**Outcome.** The query backend returns a phase-aware success/failure/cancellation outcome that
retains every artifact produced before termination. One SQLite terminal journal is durable
authority; immutable payload files are content-addressed projections. The terminal journal is
committed before success/failure/cancellation notification, and `explain_version` resolves
normal and fallback records without re-executing a query.

**Dependencies.** WP03.

**Target invariants.** GI-07, GI-08, GI-11, GI-12; Principles 3, 9–11, 16–19, 23–25;
HOL Principles 17, 19, 22–25, 27–30.

**Design and library references.** Review IR-003; proposal R4; FAB §§12.10, 13, 70–71,
107, 110–112; LIFE failure/cancellation invariants; original plan WP65/WP66 and LD-06/LD-07;
`MOD-05`/`MOD-06`, `RUN-05`/`RUN-08`–`RUN-10`, `OBS-01`–`OBS-12`, `GOV-10`,
`TST-10`/`TST-11`/`TST-14`; DataFusion §§20–21, 28, 30, 33 and plan reference §55;
delta-rs §§3.11/3.15, 5.13/5.17, 6.25.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/query_service.rs --match '^(PersistedQueryArtifactBundle|ResultArtifactStore|SemanticQueryBackend|ProductionQueryService)$' --view expanded
rg -n 'plan_artifacts: Vec::new|persist_query_artifact|append_terminal|artifacts.insert|canonicalize_value' src/query_service.rs
rg -n 'QueryPlanArtifact|query_plan_in_execution|DisplayableExecutionPlan|with_metrics|with_full_metrics' src/fabric/serving.rs
jq '.operational_tables[] | select(.name == "result_artifact_lease")' contracts/schema/schema-contract-ir.json
rg -n 'fault|stream drop|result insertion|artifact persistence|wp65_negative' src/query_service.rs contracts tests
```

Known current touch includes `src/query_service.rs`, `src/fabric/serving.rs`,
`src/operational_store.rs`, the operational schema IR and generated table bindings, daemon
construction/wiring, fault registries/fixtures, and query-artifact justfile selectors.

**Required changes.**

1. Replace backend `Result<ExecutedSemanticResponse, SemanticQueryError>` with an
   application-owned terminal outcome whose failure/cancellation variants carry the execution
   context, exact registered lifecycle phase, available bound/optimized/physical artifacts,
   partial metrics, snapshot/publication/table pins, coverage state, and public error.
2. Introduce a query-execution artifact accumulator allocated with `execution_id` before
   planning. Each phase appends immutable stage evidence as soon as it exists. Stage state is
   explicit (`NOT_REACHED`, `AVAILABLE`, `PARTIAL`, `COMPLETE`, `UNAVAILABLE_WITH_REASON`), so
   cancellation before planning is not conflated with loss after physical execution.
3. Add a model-owned operational `query_execution_terminal` journal contract keyed by
   execution identity. It stores terminal phase, failing stage, canonical bundle checksum,
   primary payload URI/status, bounded fallback envelope bytes when needed, retention fields,
   and joinable request/snapshot/publication identities. The journal is terminal-state
   authority; payload files and `result_artifact_lease` remain referenced payload/lease
   projections.
4. Normal ordering is: finalize available evidence → write immutable payload → commit terminal
   journal and lease → emit artifact/terminal event. If primary payload/result insertion or
   canonical payload publication fails, commit a bounded fallback envelope with the failure
   code and every already captured pin/artifact before emitting failure. A journal commit
   failure makes the service unavailable and suppresses a governed terminal claim; it is not
   silently ignored.
5. Capture metrics from the exact `ExecutionPlan` instance being polled. Stream drop and
   cancellation after execution starts snapshot its current metrics without building or
   executing another plan. Preserve the existing `AnalyzeExec`/`EXPLAIN ANALYZE` prohibition.
6. Make terminal persistence idempotent by execution identity and canonical checksum.
   Conflicting second terminal records fail closed. Crash recovery reconciles staged payload,
   journal, and lease records without altering an already committed terminal meaning.
7. Update artifact readback, retention, and `explain_version` to resolve primary or fallback
   payloads and to report an explicit missing/expired provenance gap rather than returning an
   empty bundle.

**Legacy Disposition and Decommission.** Delete empty-on-error/cancel as a universal
representation, ignored persistence results, terminal-event-before-artifact paths, and early
returns that strand an allocated execution without a journal row. DB03 owns final zero-state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `query_failure_artifact_closure`; Executable oracle:
`query_terminal_journal_authority`; Executable oracle:
`query_artifact_no_diagnostic_reexecution`; Executable oracle:
`query_artifact_failure_operational_gate`.

- **Behavioral — Executable oracle:** `query_failure_artifact_closure` injects failure after binding, logical
  planning, optimization, physical planning, first batch, stream drop, result insertion, and
  payload persistence; every allocated execution reads back the exact phase and all artifacts/
  pins available at that point.
- **Structural — Executable oracle:** `query_terminal_journal_authority` proves one authoritative terminal row per
  execution, content-addressed payload linkage, explicit stage-state fields, and terminal-event
  ordering after journal commit.
- **Negative/Zero-State — Executable oracle:** `query_artifact_no_diagnostic_reexecution` uses counting plans and
  providers to prove success and every failure path execute at most once; conflicting terminal
  writes, missing fallback, and journal failure are rejected.
- **Operational — Executable oracle:** `query_artifact_failure_operational_gate` expands
  `just query-artifact-single-execution-check` to select all three
  tests with `--no-tests=fail`, retain the structural `AnalyzeExec` zero-state check, and cover
  daemon restart/recovery.

**Edit-Local Gates.** Operational-schema model-family check, targeted artifact-store tests,
`just root-fmt`, `just root-clippy`.

**Packet-Local Gates.** `just query-artifact-single-execution-check`,
`just semantic-query-conformance-check`, `just wave5-integration-check`,
`just wave6-integration-check`, `just model-repro-check`, `just root-test`.

**Integration Milestone.** M03.

**Replan Triggers.** The operational schema migration cannot preserve existing artifact leases;
the terminal envelope exceeds the bounded SQLite fallback policy; or a storage-failure class
cannot be made honest without a separate durable service authority.

**Rollback or Recovery.** Dual-write is permitted only inside this packet while the new journal
is compared to the old payload path and is never advertised as two authorities. Packet exit
requires journal cutover. Recovery rolls forward from journal records and content-addressed
payloads; it never deletes an unexplained terminal record.

**Design-Bearing Contracts and Exemplars.** Persist success, pre-plan cancellation,
post-physical cancellation, partial-stream failure, primary-store failure, and crash-recovery
envelopes as schema fixtures. Each fixture names which stage artifacts must be present.

### WP05 — Full-corpus independent clean rebuild and AC-G-79 equivalence

**Outcome.** The released sixteen-scenario runner performs a true clean rebuild at every
terminal checkpoint and proves effective-state equivalence over independently published and
pinned serving sessions, including semantic capability success/withdrawal, unknowns,
duplicates, tombstones, and overlays.

**Dependencies.** WP03.

**Target invariants.** GI-09–GI-12; Principles 3, 7–12, 16–20, 23–25; HOL Principles 13,
18, 22, 24–25, 27, 30–31.

**Design and library references.** Review IR-004; proposal R9; SUITE AC-G-79 §§79.1–79.3 and
Gate C; LIFE §§137–138; FAB §§12, 63, 66, 70–72, 91, 98, 112; original plan WP72;
`ARR-01`–`ARR-08`, `SCH-09`/`SCH-10`, `CAT-02`–`CAT-07`, `RUN-03`–`RUN-05`,
`OBS-08`–`OBS-12`, `TST-02`, `TST-06`, `TST-10`, `TST-11`, `TST-14`; DataFusion
§§17–23, 30, 32; Arrow §§3, 5–8; delta-rs §§3.8/3.14–3.18, 6.24–6.25, 7.1.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'clean_rebuild_equal|FastSyntaxReconciler|semantic_capabilities_required: false' src/gate_b_candidate.rs src/fabric/serving.rs
rg -n 'prove_serving_rebuild_equivalence|CanonicalState::from_serving_session|comparison-ignore' src contracts tests
rg -n 'REQUIRED_SCENARIOS|load_scenarios|ScenarioDefinition|ScenarioTerminal' src/golden_corpus.rs src/gate_b_candidate.rs
find tests/golden -name scenario.json -print | sort
just --show rebuild-equivalence-check
```

Known current touch includes `src/gate_b_candidate.rs`, `src/golden_corpus.rs`,
`src/fabric/serving.rs`, continuous/lifecycle/provider orchestration, comparison contracts,
scenario fixtures, and rebuild/wave-7 justfile selectors.

**Required changes.**

1. Replace the scenario runner's same-wave `FastSyntaxReconciler` equality claim with a
   reusable production clean-rebuild harness. A lightweight same-wave determinism test may
   remain under an honest name, but cannot satisfy AC-G-79 or corpus terminal state.
2. At every scenario terminal checkpoint, freeze the real workspace bytes and analysis-context
   set. The clean side creates new zero-generation engine state, SQLite operational store,
   overlay/candidate caches, publication roots, and serving-snapshot registry; it shares only
   immutable source fixture inputs and governed provider/model/derivation bundles.
3. Run the authoritative inventory walker and descriptor-relative current-byte capture, then
   the same Tree-sitter/Ruff/Pyrefly/rustc provider eligibility and capability policy used by
   the incremental side. Semantic success scenarios require semantic capabilities; withdrawal
   scenarios explicitly withdraw them and compare unknown/capability records. Do not globally
   set `semantic_capabilities_required: false`.
4. Reconcile and derive through normal Arrow streams, candidate validation, and Delta
   publication. Record application transaction/commit metadata, exact versions, schema/
   protocol/constraint validation, and activate a serving session built from the independent
   publication and overlay.
5. Construct both `ComparisonInput` manifests independently and reject domain mismatches before
   fact reads. Compare exact canonical schema fingerprints including field order/nullability/
   extension and governed metadata.
6. Read effective tables through their snapshot-scoped DataFusion providers, stream Arrow
   batches under the comparison budget, and let the application comparator prove primary-key
   uniqueness and duplicate-sensitive canonical bag equality. Do not use checksums, distinct
   set difference, `EXCEPT`, or `EXCEPT ALL` as the equality oracle.
7. Cover all sixteen released scenario definitions and each terminal checkpoint, including
   overflow, multi-file logical save, context change, capability withdrawal, watcher loss,
   hot-overlay flush, ACL redaction, provider withdrawal, and semantic facts.

**Legacy Disposition and Decommission.** Retire `clean_rebuild_equal` as an AC-G-79 claim, the
five-stage-only fixture as complete-corpus proof, and blanket disabled semantic capabilities.
DB03 owns zero-state while preserving any honestly renamed local determinism helper.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `full_golden_scenario_clean_rebuild_equivalence`;
Executable oracle: `clean_rebuild_independence_contract`; Executable oracle:
`clean_rebuild_equivalence_adversarial`; Executable oracle: `clean_rebuild_operational_gate`.

- **Behavioral — Executable oracle:** `full_golden_scenario_clean_rebuild_equivalence` iterates all released
  scenario definitions and compares incremental versus independent serving sessions after
  every terminal state.
- **Structural — Executable oracle:** `clean_rebuild_independence_contract` proves distinct engine/store/Delta/
  snapshot roots and that the clean side invokes authoritative inventory, byte capture,
  providers, publication, activation, and `prove_serving_rebuild_equivalence`.
- **Negative/Zero-State — Executable oracle:** `clean_rebuild_equivalence_adversarial` perturbs one tombstone,
  provider fact, context, duplicate multiplicity, schema metadata field, overlay row, and
  semantic-lane result and proves rejection.
- **Operational — Executable oracle:** `clean_rebuild_operational_gate` updates
  `just rebuild-equivalence-check` and `just wp72-acceptance-check` to
  select the full corpus/independence/adversarial tests with `--no-tests=fail` and remove the
  disjoint five-stage completion claim.

**Edit-Local Gates.** One scenario-family selector at a time, comparison schema/bag unit tests,
`just root-fmt`, `just root-clippy`.

**Packet-Local Gates.** `just rebuild-equivalence-check`, `just wp72-acceptance-check`,
`just source-capture-race-check`, `just git-parity-check`, `just wave7-integration-check`,
`just data-fabric-upgrade-check`, `just root-test`.

**Integration Milestone.** M04 prerequisite.

**Replan Triggers.** A true rebuild exposes an actual product convergence defect; one scenario
cannot execute its governed provider/capability profile; or the comparator requires a semantic
ignore not already authorized by the comparison-ignore registry. Record a discovered
obligation—never weaken equality or silently add an ignore.

**Rollback or Recovery.** Comparator and harness changes are read-only over production state
and use temporary roots. Preserve a failing corpus reproduction bundle for diagnosis; do not
alter released scenario definitions to obtain green.

**Design-Bearing Contracts and Exemplars.** One comparison manifest fixture records the exact
domain fields and independent roots. Negative fixtures encode every excluded operational field
through the governed ignore registry, not inline test exceptions.

### WP06 — Production Gate B vertical execution and rejected integrity bundle

**Outcome.** One fixture containing a Python owner and Rust MIR owner executes the real
CodeFabric vertical and emits a review candidate whose eleven planes are actual correlated
outputs. This carried-forward packet proves execution, correlation, and integrity only. The
accountable owner rejected its `codefabric-golden-v3.0.0-candidate.1` output as semantic
authority because the comparator derives shallow check-name digests, sets matches
unconditionally, and has no independently authored source-to-outcome expectations.

**Dependencies.** WP03, WP04, WP05.

**Target invariants.** GI-04–GI-13; Principles 2–3, 7–13, 16–20, 22–25; HOL Principles 6,
10, 16, 18, 22–25, 27–31.

**Design and library references.** Review IR-002; proposal R9; SUITE Gate B and corpus
acceptance/conformance contract; GEN provider stack; FAB §§12, 63, 66–72, 91, 98, 110–112;
QRY §107; SRV runtime topology; original plan WP71/WP76; `ARR-01`–`ARR-08`, `SCH-09`–
`SCH-12`, `CAT-02`–`CAT-07`, `LOG-01`–`LOG-07`, `RUN-03`–`RUN-05`, `INT-01`/
`INT-08`/`INT-10`, `OBS-01`–`OBS-12`, `TST-02`, `TST-06`, `TST-09`–`TST-12`,
`TST-14`; DataFusion §§17–23, 30, 32; Arrow §§3, 6, 10, 28; delta-rs §§3.10–3.15,
5.13–5.17, 6.24–6.25, 7.1/7.6–7.7, 11.12–11.18.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/gate_b_candidate.rs --view signatures
sed -n '980,1180p' src/gate_b_candidate.rs
sed -n '620,850p' src/golden_corpus.rs
sed -n '650,690p' src/gate_b_release.rs
rg -n 'expectations\.clone|derive_expectations|candidate_contracts|execute_artifact_contract|GateBExecution' src
rg -n 'serve_query_uds|FastMCP|stdio|artifact_ready|rustc-mir|tree-sitter|ruff|pyrefly' src codefabric-cpg-mcp tests tooling
just --show gate-b-candidate-check
just --show gate-b-check
```

Known current touch includes `src/gate_b_candidate.rs`, `src/golden_corpus.rs`,
`src/gate_b_release.rs`, daemon/query/provider/publication orchestration, a Gate B fixture
workspace, adapter protocol tests, candidate/released corpus tooling, and Gate B/wave justfile
selectors.

**Required changes.**

1. Define one hermetic Gate B workspace fixture with at least one Python owner and one Rust
   owner whose compilable crate produces a real rustc-MIR observation. The fixture also yields
   one unknown, property, relation, and derived projection and supports one hot-overlay edit.
2. Execute normal source discovery/current-byte capture and the actual provider adapters:
   Tree-sitter and Ruff for the Python source, rustc extractor/MIR for the Rust source, and the
   Pyrefly sidecar when its governed capability applies. Provider output is validated Arrow
   direct/IPC input to the ordinary reconciliation path; no candidate-only fact constructor is
   allowed.
3. Run candidate referential-integrity checks, normal Delta mutation/application-transaction
   and durable publication, reopen and validate exact table versions/protocol/constraints, then
   construct and activate the ordinary snapshot-scoped provider/catalog set.
4. Start the production daemon UDS query surface and the locked FastMCP STDIO adapter. Issue one
   composed QRY 1.3 request that exercises all eight forms, correlate `mcp_call_id`, request,
   execution, snapshot, publication, and Delta versions, consume the streamed response, and
   read back the result plus persisted plan-artifact bundle.
5. Capture actual canonical outputs for all eleven Gate B planes: source inventory, provider
   observations, identities, canonical tables, publication/version records, serving snapshot,
   query response, RPC events, MCP payload, diagnostics, rebuild comparison, and artifact
   payload/plan bundle. Capture exact bytes/checksums where the public contract is byte-based
   and canonical Arrow row fixtures where the contract is tabular.
6. Validate plane membership, schema, provider execution, referential joins, correlation keys,
   candidate bytes, and prior-release integrity as supporting evidence. Do not label these
   checks, their digests, or their source-string guards as an independent semantic oracle.
7. Preserve the immutable rejected candidate bundle containing actual outputs, environment and
   pinned version identities, prior-release diff, and detached digest. It remains unreleased and
   cannot update the corpus index. WP09–WP12 create a new functional successor rather than
   editing or reusing its expectation output.
8. Retain existing operational omission/perturbation controls as evidence that the vertical is
   real. Semantic causal interventions and counterfactual mutations at producing/public seams
   are independently owned by WP11 and WP12.

**Legacy Disposition and Decommission.** Preserve v1/v2 release bytes and the rejected v3
candidate byte-for-byte. Treat `requirement_checks`, check-name digests, unconditional
`matches: true`, `functional_candidate_projection`, and candidate-local normalization as
historical evidence mechanisms only. WP09–WP12 and DB05 own their removal from the active
semantic-authority path.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `gate_b_vertical_slice_produces_all_eleven_planes`;
Executable oracle: `gate_b_vertical_slice_adversarial`; Executable oracle:
`gate_b_candidate_operational_gate`; Executable oracle:
`gate_b_candidate_independent_oracle_contract` (integrity/source-shape regression only, not a
semantic-independence oracle).

- **Behavioral — Executable oracle:** `gate_b_vertical_slice_produces_all_eleven_planes` performs the coherent
  execution and asserts real Python owner, rustc-MIR owner, unknown/property/relation/derived
  facts, hot update, Delta publication, query, stream, and artifact.
- **Structural — Executable oracle:** `gate_b_candidate_independent_oracle_contract` is retained
  only as a regression over source shape and correlation. It explicitly does **not** prove
  semantic independence or correctness; WP10 supplies that proof.
- **Negative/Zero-State — Executable oracle:** `gate_b_vertical_slice_adversarial` executes all omission/
  perturbation controls and proves rejection.
- **Operational — Executable oracle:** `gate_b_candidate_operational_gate` updates
  `just gate-b-candidate-check` to select the three tests with
  `--no-tests=fail`, run the real adapter protocol fixture, verify the candidate digest chain,
  and emit the exact review bundle consumed by WP07.

**Edit-Local Gates.** Focused provider/publication/query/adapter slice tests, `just root-fmt`,
`just root-clippy`, `just adapter-lint`, `just adapter-type`.

**Packet-Local Gates.** `just gate-b-candidate-check`,
`just semantic-query-conformance-check`, `just query-artifact-single-execution-check`,
`just rebuild-equivalence-check`, `just provider-protocol-check`,
`just publication-referential-integrity-check`, `just adapter-ci-fast`,
`just extractor-ci-fast`, `just sidecar-ci-fast`, `just wave5-integration-check`,
`just wave6-integration-check`.

**Integration Milestone.** M04, as execution/integrity evidence only; M05 additionally requires
WP09–WP12.

**Replan Triggers.** The fixture cannot exercise a real rustc-MIR/Python provider under the
declared toolchains; a required production boundary lacks a testable entry point; actual output
reveals a specification ambiguity; or the candidate cannot be deterministic without weakening
semantic identity.

**Rollback or Recovery.** Candidate directories are unreleased disposable outputs. Preserve a
failed diagnostic bundle under `target/`; remove/recreate only the explicit candidate output
after validating its path. Never mutate a released corpus during candidate recovery.

**Design-Bearing Contracts and Exemplars.** The candidate manifest records the eleven plane
contracts, exact correlation keys, source/tool/provider identities, Delta pins, and whether a
payload is canonical JSON, Arrow rows, protocol events, or content-addressed bytes.

### WP09 — Authored functional claim pack and immutable rejection boundary

**Outcome.** A strict, reviewable Gate B claim pack defines expected source-to-semantic behavior,
named query answers, scenario transitions, proof universes, and public delivery equivalence before
any successor execution. The rejected v3 candidate is preserved and structurally impossible to
accept or use as expectation input.

**Dependencies.** WP01, WP03, WP05, WP06. The v1 state must record the accountable rejection
before this packet edits any release/candidate routing.

**Target invariants.** GI-06, GI-09–GI-13, GI-15, GI-18; design D-01, D-04–D-06 and LD-01.

**Design and library references.** Behavioral-conformance design §§1–3.2, 5, and 6; SUITE
AC-G-78/AC-G-79; GEN §§14–25 and §§34–51; QRY §§36, 46, 48, 104, 112; LIFE §§157–159;
RM §10; serde/serde_json and canonicalization references for strict ingress only.

**Change surface / Preflight / Known Touch.** Run:

```bash
find tests/golden -maxdepth 4 -type f | sort
jq . tests/golden/review-candidates/codefabric-golden-v3.0.0-candidate.1/expected-vs-candidate-diff.json
sed -n '995,1420p' src/gate_b_candidate.rs
sed -n '420,760p' src/golden_corpus.rs
ast-grep outline src/gate_b_release.rs --view signatures
rg -n 'codefabric-golden-v3|expected_terminal|matches: true|requirement_checks|functional_candidate_projection|normalize_gate_b_planes' src tests tooling justfile
rg -n 'object_pairs_hook|deny_unknown_fields|from_slice|canonical' src tests contracts
```

Known touch includes a new `tests/golden/codefabric-golden-v4/` authored input tree,
`tests/functional_golden/claim_schema.rs`, Gate B candidate/release validation, corpus-index
guards, a detached owner-rejection artifact outside the immutable candidate directory, and
justfile/governance selectors. Preflight may choose an existing neutral test-support module
instead of the conceptual path without changing the ownership boundary.

**Required changes.**

1. Author minimal Python, Rust, and FFI sources with named anchors whose expected semantics are
   directly reviewable. Cover the Gate B minimum in design §2.3 and retain all sixteen AC-G-78
   scenario IDs.
2. Replace edit-label-only scenario definitions with declarative exact operations and checkpoint
   claims: source bytes before/after, file add/remove/rename, configuration/context changes,
   provider faults, barriers, expected terminal/capability state, and named added/removed/changed/
   preserved facts and query outcomes.
3. Define strict application-owned DTOs for claim IDs, normative references, proof universes,
   anchors/selectors, closed predicates, cardinality/bag/order policies, transition claims,
   surface equivalence, and narrow reasoned allowed observations. Reject unknown fields,
   duplicate keys, unresolved/ambiguous anchors, contradictory predicates, invalid scenario
   transitions, and negative claims without a complete universe.
4. Author exact Gate B claims for Python owner/local semantics, Rust definition/MIR semantics,
   property, relation, derived result, unknown, capability, provenance, diagnostic, all eight QRY
   forms, failure/recovery, context separation, ACL redaction, and delivery surfaces. Literal
   runtime IDs/digests/descriptor identities/registry counts and copied candidate rows are
   forbidden expectation fields.
5. Add an immutable detached rejection decision that binds owner, rejected candidate ID/digest,
   decision, reason, and timestamp without changing candidate bytes. Make release validators and
   current-index resolution reject that candidate and any candidate derived from its actual output
   as expectation input.
6. Add `functional-golden-contract-check` and
   `gate-b-rejected-candidate-zero-state-check`. Extend artifact/oracle substance governance so a
   shallow non-empty check list or `matches: true` cannot satisfy a semantic oracle declaration.

**Legacy Disposition and Decommission.** Scenario `edits`/`expected_terminal` fields may remain
only as migrated manifest metadata until WP11 executes the declarative operations; they cease to
be semantic authority in this packet. The rejected candidate remains read-only. DB05 tracks final
zero-state for captured-output expectations and hard-coded edit execution.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `functional_golden_claim_schema_conformance`; Executable
oracle: `functional_golden_source_anchor_closure`; Executable oracle:
`functional_golden_negative_claim_requires_complete_universe`; Executable oracle:
`gate_b_rejected_candidate_zero_state`; Executable oracle:
`functional_golden_contract_operational_gate`.

- **Behavioral — Executable oracle:** `functional_golden_claim_schema_conformance` parses every
  Gate B source, operation, claim, query, and checkpoint and proves exact contract closure.
- **Structural — Executable oracle:** `functional_golden_source_anchor_closure` resolves every
  authored anchor uniquely against fixture bytes and rejects expected IDs/digests, full row dumps,
  unconditional matches, candidate imports, and write-capable expectation paths.
- **Negative/Zero-State — Executable oracle:** `functional_golden_negative_claim_requires_complete_universe`
  rejects absent/COMPLETE/PROVEN_EMPTY claims when any owner, context, family, capability, or
  currentness boundary is missing; `gate_b_rejected_candidate_zero_state` proves v3 immutable and
  unreleasable.
- **Operational — Executable oracle:** `functional_golden_contract_operational_gate` proves the
  two recipes select all named tests with `--no-tests=fail` and that a candidate-emission dry run
  cannot write beneath the expectation root.

**Edit-Local Gates.** Focused contract-parser tests, `just root-fmt`, `just root-clippy`, and
`just typos` over new authored fixtures.

**Packet-Local Gates.** `just functional-golden-contract-check`,
`just gate-b-rejected-candidate-zero-state-check`, `just oracle-substance-check`,
`just artifacts-check`, `just root-test`.

**Integration Milestone.** M05 prerequisite.

**Replan Triggers.** A normative semantic claim cannot be expressed without a production ID;
the source fixture is not reviewable without provider-specific internals; a negative claim lacks a
finite proof universe; or preserving the rejected candidate would require modifying its bytes.

**Rollback or Recovery.** Remove only the new unaccepted v4 expectation/candidate outputs after
validating their exact paths. Never modify v1/v2 releases, v3 candidate bytes, or the detached
rejection decision to recover a failing claim.

**Design-Bearing Contracts and Exemplars.** One Python call-target edit and one Rust compile-
failure/recovery scenario demonstrate source anchor, expected relation/fact delta, explicit
unknown/capability state, closed negative proof, and surface-equivalence requirements without
literal runtime identity.

### WP10 — Independent reference evaluators and fail-closed semantic comparator

**Outcome.** Test-only evaluators compute transparent relational/graph/query consequences from
authored logical claims and compare them with decoded observations. A wrong, extra, missing,
ambiguous, duplicated, misdirected, stale, or falsely complete observation fails without calling
the production semantic engine.

**Dependencies.** WP09.

**Target invariants.** GI-03, GI-06, GI-12, GI-15, GI-16, GI-18; design D-02–D-06 and
LD-02/LD-03/LD-06.

**Design and library references.** Behavioral-conformance design §§3.2–3.4 and §6; QRY
request forms and §§36/46/48/112; SUITE AC-G-78/AC-G-79; holistic Principles 17, 25, 30–31;
data-fabric Principles 18–19, 24–25.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/semantic_query.rs src/query_service.rs src/reconciliation.rs --view signatures
ast-grep outline src/gate_b_candidate.rs src/lifecycle.rs --view signatures
rg -n 'proptest|BTreeMap|BTreeSet|VecDeque|multiset|bag|CoverageProof|PROVEN_EMPTY|COMPLETE' src tests Cargo.toml
rg -n 'oracle-substance|single_call|nontrivial|forbidden' tooling/ci/plan_assurance.py tooling/ci/test_plan_assurance.py
rg -n 'use crate::.*(semantic_query|reconciliation|gate_b|lifecycle)|datafusion|petgraph' tests
```

Known touch includes `tests/functional_golden/reference_query.rs`, `transition.rs`,
`observation.rs`, `mutations.rs`, focused test registration beside the existing single
integration target, governance rules, and `oracle-substance-check` implementation/tests.

**Required changes.**

1. Implement a `FixtureClaimEvaluator` that validates referential closure and compares strict
   entity/fact/property/relation/unknown/capability claims against decoded logical observations.
   It performs exact bag multiplicity and fails unexpected closed-universe observations.
2. Implement a `ReferenceQueryEvaluator` over authored logical records using standard-library
   filtering, adjacency/BFS, bounded path enumeration, named-binding joins, set/bag operations,
   grouping/aggregation, and stable ordering for all eight named Gate B requests. It consumes no
   production plan, result, registry-generated answer, or provider output while constructing
   expected answers.
3. Implement a `ScenarioTransitionEvaluator` that asserts authored added/removed/changed/
   preserved claims and the required terminal/unknown/capability state between checkpoints. It
   does not schedule updates or reproduce lifecycle code.
4. Implement a `PublicObservationDecoder` that converts governed canonical Arrow rows, protobuf
   UDS messages, artifact bytes, and MCP structured output into the evaluator's neutral logical
   observation form. Decoder code is boundary plumbing and cannot construct expectations.
5. Replace `derive_diff`/`GroupComparison` semantic use with per-claim results containing expected
   logical claim, matched actual selectors/values, missing/unexpected/ambiguous/blocked status, and
   source/provenance pointers. Aggregate success is `all required claims matched` and `no closed-
   universe remainder`, never list non-emptiness.
6. Strengthen independence governance: the evaluator closure cannot import/call production
   provider, identity, reconciliation, query, DataFusion, petgraph, lifecycle, candidate, or
   release code; production cannot import test expectations; candidate writers cannot mutate
   expectations; and expected contract files cannot contain observed identity/digest/row fields.
7. Add evaluator self-tests with hand-written correct and incorrect observations and proptest laws
   for bag multiplicity, direction, bounded path, composition, ordering, coverage monotonicity,
   and the rule that negative proof cannot strengthen when coverage weakens.

**Legacy Disposition and Decommission.** Remove `requirement_checks`, check-name expected digests,
unconditional `matches: true`, non-empty aggregate success, and candidate-derived expected
projection from the successor semantic path. Preserve only a historical integrity verifier for
released prior artifacts. DB05 owns structural zero-state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `reference_query_evaluator_laws`; Executable oracle:
`functional_golden_comparator_falsification`; Executable oracle:
`functional_golden_expectation_write_isolation`; Executable oracle:
`functional_golden_independence_operational_gate`.

- **Behavioral — Executable oracle:** `reference_query_evaluator_laws` proves exact results for
  all eight forms and composition over authored logical models; cross-check examples are derived
  by manual fixtures, not production execution.
- **Structural — Executable oracle:** `functional_golden_expectation_write_isolation` and the
  governance scan prove import/call/write separation in both directions.
- **Negative/Zero-State — Executable oracle:** `functional_golden_comparator_falsification`
  injects missing/extra/duplicate/swapped/wrong-kind/wrong-certainty/stale/false-complete logical
  observations and proves every one fails the intended claim.
- **Operational — Executable oracle:** `functional_golden_independence_operational_gate` backs
  `just functional-golden-independence-check` with named tests, ast-grep rules, covered `rg`
  searches, and `--no-tests=fail`; the gate fails if a semantic aggregate depends only on a hash,
  boolean literal, shared object, or non-empty census.

**Edit-Local Gates.** Focused evaluator unit/proptest cases, `just root-fmt`,
`just root-clippy`.

**Packet-Local Gates.** `just functional-golden-independence-check`,
`just functional-golden-contract-check`, `just oracle-substance-check`, `just governance-scan`,
`just root-test`.

**Integration Milestone.** M05 prerequisite.

**Replan Triggers.** The evaluator needs to parse source or reproduce provider/compiler semantics;
a QRY form cannot be represented over authored logical records; canonical observations lack a
public field needed to resolve a claim; or structural isolation requires a new crate/root.

**Rollback or Recovery.** Evaluators are test-only and have no production state. Preserve failing
logical models and diffs as test diagnostics; never weaken a closed universe or add a comparison
ignore to make a failure disappear.

**Design-Bearing Contracts and Exemplars.** A four-node parallel-edge graph and two-context fact
bag exercise direction, witnesses, multiplicity, path bounds, named bindings, grouping, ordering,
unknown remainder, and negative proof without production graph/query types.

### WP11 — Declarative scenario runner, public-boundary equivalence, and causal interventions

**Outcome.** The authored Gate B scenarios run through fresh real workspaces, providers,
reconciliation, Delta publication, daemon UDS, artifact readback, and explicit FastMCP STDIO.
Every checkpoint satisfies independently expected semantic claims, and producing-seam faults prove
that the observed outcomes are causally connected to the intended subsystems.

**Dependencies.** WP04, WP05, WP06, WP09, WP10.

**Target invariants.** GI-07–GI-12, GI-15–GI-18; design D-02, D-04–D-06 and LD-04/LD-05.

**Design and library references.** Behavioral-conformance design §§2.2–3.5 and §6; SUITE
AC-G-78/AC-G-79; FAB §§91/98/112; QRY §§36/46/48/112; LIFE §§157–159; SRV §§17/60/68 and
AC-G-63; FastMCP reference §§21, 22.2, 30.0–30.2, 30.15, 30.19–30.20; grpcio/protobuf
public-client and deadline/error guidance; DataFusion/Arrow alignment `OBS-01`/`OBS-06`/
`OBS-08`–`OBS-10` and `TST-01`/`TST-06`/`TST-08`/`TST-10`–`TST-14`.

**Change surface / Preflight / Known Touch.** Run:

```bash
ast-grep outline src/gate_b_candidate/vertical.rs src/golden_corpus.rs src/fabric/serving.rs src/daemon.rs --view signatures
ast-grep outline tooling/gate_b_adapter_probe.py codefabric-cpg-mcp/tests/test_server.py --view signatures
rg -n 'run_scenario|replace-python|replace-rust|expected_terminal|run_pyrefly|run_rustc' src tests tooling
rg -n 'StdioTransport|ProductionStubDaemon|serve_query_uds|artifact_ready|read.*artifact|terminal' src codefabric-cpg-mcp tooling tests
just --show rebuild-equivalence-check
just --show gate-b-candidate-check
```

Known touch includes the Gate B vertical/scenario runtime, provider launcher ownership,
observation decoders, daemon/public clients, adapter probe/tests, temporary-root management,
fault hooks, and intent-level recipes. Production behavior changes only when a public seam needed
by the normative scenario is absent or wrong.

**Required changes.**

1. Implement a generic declarative scenario executor that materializes source/config fixtures,
   applies exact operations, injects named faults, and waits on deterministic barriers. Delete the
   production hard-coded edit label switch after every scenario migrates.
2. Run authoritative inventory/current-byte capture and the actual Tree-sitter/Ruff/Pyrefly and
   rustc/MIR provider policies in fresh roots. Route provider launches through neutral runtime or
   test support rather than `gate_b_candidate::vertical` as a cross-subsystem authority.
3. Reconcile, publish, reopen exact Delta versions, activate a normal `ServingSnapshot`, and
   decode canonical rows. DataFusion/Arrow/Delta output is actual evidence, never expectation
   input.
4. Issue the named all-eight-form requests over the production UDS service with deadlines, consume
   chunks and the terminal event, read the referenced artifact, and invoke the locked adapter via
   explicit FastMCP `StdioTransport` with an explicit environment and fresh subprocess. Compare
   structured results and errors with the independent evaluator.
5. At each checkpoint, resolve every authored selector and assert semantic values, exact bag/
   ordering policy, transition deltas, capabilities/unknowns, coverage/negative proof, provenance,
   and absence of unexpected closed-universe observations.
6. Retain AC-G-79 full schema and duplicate-sensitive bag equality between incremental and fresh
   clean roots, but report it separately from claim conformance. A shared wrong result fails the
   authored claim even if rebuild equivalence is green.
7. Add causal interventions before provider output, reconciliation authority choice, Delta
   publication, snapshot activation, UDS row/chunk/terminal emission, artifact persistence and
   readback, and FastMCP adaptation. Each fault must fail the named dependent claim and preserve
   unrelated claims where the design predicts isolation.
8. Produce per-scenario decoded evidence with source diff, claim, named request, expected logical
   result, actual surface observations, semantic diff, coverage/unknown limitations, convergence,
   intervention results, and separate integrity/correlation metadata.

**Legacy Disposition and Decommission.** Remove candidate-local duplicated normalization and
derive ignored operational fields only from the governed comparison-ignore registry. Remove direct
candidate-module provider helper consumption and hard-coded scenario edit dispatch after positive
replacement proof. DB05 owns final zero-state.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `gate_b_public_vertical_conformance`; Executable oracle:
`gate_b_projection_registry_closure`; Executable oracle:
`gate_b_causal_intervention_matrix`; Executable oracle:
`gate_b_public_vertical_operational_gate`.

- **Behavioral — Executable oracle:** `gate_b_public_vertical_conformance` runs the real vertical
  and satisfies every authored claim; `golden_scenario_semantic_transition_contracts` proves all
  sixteen checkpoint transitions.
- **Structural — Executable oracle:** `gate_b_projection_registry_closure` proves one governed
  comparison-ignore authority and zero candidate-local normalizer/edit dispatcher/provider-owner
  leakage.
- **Negative/Zero-State — Executable oracle:** `gate_b_causal_intervention_matrix` suppresses or
  corrupts each producing/public seam and proves the relevant semantic claim fails;
  `gate_b_named_fixture_query_causality` proves named source edits cause predicted query changes.
- **Operational — Executable oracle:** `gate_b_delivery_surface_semantic_equivalence` proves
  canonical/UDS/stream/artifact/STDIO meanings equal the independent answer; the operational gate
  wires `gate-b-public-vertical-check`, `gate-b-causal-check`,
  `gate-b-delivery-equivalence-check`, and `gate-b-projection-registry-check` with
  `--no-tests=fail`.

**Edit-Local Gates.** Focused scenario/fault/public-client tests, `just root-fmt`,
`just root-clippy`, `just adapter-lint`, `just adapter-type`.

**Packet-Local Gates.** `just gate-b-public-vertical-check`, `just gate-b-causal-check`,
`just gate-b-delivery-equivalence-check`, `just gate-b-projection-registry-check`,
`just rebuild-equivalence-check`, `just provider-protocol-check`,
`just publication-referential-integrity-check`, `just adapter-ci-fast`,
`just extractor-ci-fast`, `just sidecar-ci-fast`, `just root-test`.

**Integration Milestone.** M05 prerequisite.

**Replan Triggers.** A public boundary cannot expose a normative semantic field; deterministic
barriers require time sleeps or private state polling; a causal intervention can be applied only
after final serialization; the adapter needs Python data-plane semantics; or the functional run
cannot remain hermetic under the pinned toolchains.

**Rollback or Recovery.** All runs use validated temporary roots. Preserve a failing decoded
evidence bundle under `target/`; do not edit authored claims, released corpora, or comparison
ignores merely to restore green.

**Design-Bearing Contracts and Exemplars.** The named Python call edit, Rust compile failure,
provider withdrawal, cross-context query, ACL redaction, and stream-terminal cases each show the
same independently expected logical result through every applicable public surface.

### WP12 — Semantic mutant closure, human review dossier, and functional successor candidate

**Outcome.** Every Gate B semantic axis kills its registered counterfactual, a compact decoded
review dossier makes the intended and observed behavior independently assessable, and one new
immutable unreleased functional candidate is ready for the WP07 owner decision.

**Dependencies.** WP09, WP10, WP11.

**Target invariants.** GI-09–GI-13, GI-15–GI-18; design D-01–D-06 and §3.4–3.5.

**Design and library references.** Behavioral-conformance design §§3.4–3.5, 5, and 6; SUITE
Gate B/AC-G-78/AC-G-79; FAB §112; QRY §112; SRV §68; holistic Principles 25/30/31;
data-fabric Principles 18/19/24/25.

**Change surface / Preflight / Known Touch.** Run:

```bash
rg -n 'mutation|mutant|adversarial|fault|review-candidate|accept_candidate|corpus-index' src tests tooling justfile
find tests/golden/review-candidates -maxdepth 2 -type f | sort
du -h tests/golden/review-candidates/*/candidate.json
just --show gate-b-owner-acceptance-check
just --show oracle-substance-check
```

Known touch includes mutation registration/execution, review-report rendering, new candidate
manifest/digest, release-decision validation, corpus-index protection, justfile/CI selectors,
and plan-assurance tests.

**Required changes.**

1. Register every claim predicate and semantic axis with at least one required counterfactual.
   The minimum matrix is the design §3.4 list. Mutations carry owner claim IDs, injection seam,
   predicted failed claims, and allowed unaffected claims.
2. Run all required mutants deterministically against the evaluator or producing/public seam.
   Fail on a survivor, an unregistered claim axis, a mutation that only changes a digest, or a
   mutation that breaks unrelated claims contrary to its declared isolation boundary.
3. Render a review dossier organized by scenario/claim rather than one large minified capture.
   Include source before/after, normative rule, expected logical answer, decoded canonical/UDS/
   artifact/MCP observations, semantic diff, intervention/mutant result, rebuild result,
   limitations, and separate integrity metadata.
4. Emit a new immutable candidate version from the unchanged authored expectation pack plus the
   actual evidence/dossier. Its diff is computed claim-by-claim by WP10; no candidate output is
   copied into expected claims. The candidate cannot advance `corpus-index.json`.
5. Generalize candidate/release validation so the v2 accepted release remains verifiable, the v3
   rejected candidate remains unreleasable, and the successor can be selected only by an explicit
   future WP07 decision bound to the exact behavior-contract and evidence bundle.
6. Add the intent-level recipes `semantic-oracle-mutants-check gate-b`,
   `gate-b-review-bundle-check`, `gate-b-functional-candidate-check`, and aggregate
   `gate-b-predecessor-check`. `ci-pr` may depend on the non-owner candidate aggregate, but no gate
   may invoke the confirm-gated owner acceptance action.

**Legacy Disposition and Decommission.** The old v3 `expected-vs-candidate-diff.json` remains
rejected evidence and cannot be regenerated in place. Large raw captures may remain linked
evidence but cannot substitute for the decoded dossier. DB05 proves old self-confirming paths are
absent from the current semantic aggregate.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `semantic_oracle_required_mutants_are_killed`; Executable
oracle: `semantic_oracle_rejects_unregistered_or_surviving_mutant`; Executable oracle:
`gate_b_human_review_bundle_contract`; Executable oracle:
`gate_b_functional_candidate_is_expectation_independent`; Executable oracle:
`gate_b_functional_candidate_operational_gate`.

- **Behavioral — Executable oracle:** `semantic_oracle_required_mutants_are_killed` proves every
  Gate B claim axis detects a known-wrong observation or producing-seam fault.
- **Structural — Executable oracle:** `gate_b_human_review_bundle_contract` proves every claim has
  source, expectation, decoded actual, semantic diff, limitations, mutant, convergence, and
  integrity sections and no expectation is captured from actual output.
- **Negative/Zero-State — Executable oracle:** `semantic_oracle_rejects_unregistered_or_surviving_mutant`
  fails on missing coverage/survivors; `gate_b_functional_candidate_is_expectation_independent`
  rejects candidate-written expectations and any attempt to select rejected v3.
- **Operational — Executable oracle:** `gate_b_functional_candidate_operational_gate` proves the
  four new recipes execute the real provider/UDS/STDIO path, independent evaluator, full mutant
  matrix, dossier validator, candidate chain, and packet oracle with `--no-tests=fail`.

**Edit-Local Gates.** Focused mutation/report/release tests, `just root-fmt`,
`just root-clippy`, `just adapter-lint`, `just typos`.

**Packet-Local Gates.** `just semantic-oracle-mutants-check gate-b`,
`just gate-b-review-bundle-check`, `just gate-b-functional-candidate-check`,
`just gate-b-predecessor-check`, `just functional-golden-contract-check`,
`just functional-golden-independence-check`, `just gate-b-public-vertical-check`,
`just gate-b-causal-check`, `just gate-b-delivery-equivalence-check`,
`just rebuild-equivalence-check`, `just oracle-substance-check`, `just ci-fast`.

**Integration Milestone.** M05 prerequisite and WP07 handoff.

**Replan Triggers.** Any required mutant survives; the dossier cannot show an expected-vs-actual
semantic claim without exposing volatile internals; the successor candidate depends on rejected
candidate output; or generating the candidate changes authored expectations.

**Rollback or Recovery.** A failed successor is an unreleased new candidate version. Preserve its
diagnostics, correct code or design through a new attempt, and never overwrite a candidate already
presented for review.

**Design-Bearing Contracts and Exemplars.** The dossier summary explicitly separates semantic
claim conformance, causal-mutant sensitivity, incremental/rebuild convergence, delivery-surface
equivalence, and byte/provenance integrity.

### WP07 — Accountable Gate B acceptance and immutable superseding release

**Outcome.** The registered accountable repository owner reviews the exact WP06 actual-output
bundle together with the WP09–WP12 authored behavior contract, decoded semantic evidence,
claim-by-claim diff, causal mutants, delivery equivalence, convergence result, and integrity
metadata; accepts or rejects it; and—only on acceptance—publishes the next immutable corpus
version. The release gate re-executes the vertical and re-evaluates authored semantic claims in
addition to comparing the accepted immutable bytes.

**Dependencies.** WP12. This packet contains an external human checkpoint. The implementation
agent must stop after presenting the immutable bundle and must not invoke the acceptance action
or author the approval decision.

**Target invariants.** GI-09, GI-12, GI-13, GI-15–GI-18; Principles 3, 9–11, 13, 18–20, 25; HOL
Principles 8, 16, 24, 29–31.

**Design and library references.** Behavioral-conformance design §§1, 3.5, 5, and 6; review
IR-002; proposal R9; SUITE corpus acceptance and Gate B; original plan WP76; `GOV-06`,
`GOV-10`, `OBS-07`–`OBS-10`, `TST-11`, `TST-14`.

**Change surface / Preflight / Known Touch.** Run:

```bash
just gate-b-candidate-check
just gate-b-functional-candidate-check
just gate-b-review-bundle-check
just semantic-oracle-mutants-check gate-b
just gate-b-owner-acceptance-check
jq . tests/golden/corpus-index.json
find tests/golden -maxdepth 3 -type f \( -name '*acceptance*.json' -o -name 'corpus-manifest.json' \) | sort
rg -n 'EXPECTED_OWNER_IDENTITY|accept_candidate|verify_release_chain|current_released_corpus_root' src/gate_b_release.rs src/golden_corpus.rs tooling justfile
```

Known current touch after acceptance includes a new immutable corpus directory, versioned owner
acceptance artifact, corpus index, candidate/release validators, and Gate B/CI selectors. Prior
corpus directories are read-only.

**Required changes.**

1. Present the WP12 successor's authored sources/claims, source-to-claim trace, decoded actual
   eleven planes, exact query answers, semantic diff, causal/mutant results, delivery-surface
   equivalence, convergence result, limitations, and integrity metadata. The owner reviews
   intended and observed semantic content—not merely hashes, filenames, counts, or descriptors.
2. Pause for explicit accept/reject. The acceptance records candidate digest, governing input
   digests, owner authority, decision, timestamp, superseded corpus, and any named limitations.
   Rejection leaves released bytes/index unchanged and routes to the owning packet/spec.
3. Extend the confirm-gated `just gate-b-owner-accept` transaction to accept the functional
   candidate only for the registered authority and only after contract, independence, semantic,
   causal-mutant, delivery, rebuild, dossier, and integrity checks are green. The executor may
   validate inputs but may not run this command. The command validates again, writes the
   acceptance artifact and next free corpus version exactly once, and atomically advances the
   index; failure leaves the previous index current.
4. Preserve every prior corpus and acceptance file byte-for-byte. A correction after acceptance
   creates another version and acceptance decision.
5. Make `gate-b-check` verify the owner/candidate/corpus chain, re-execute the same production
   vertical, re-evaluate every authored claim and query answer, compare accepted bytes/Arrow
   rows as integrity evidence, run the full required semantic-mutant/causal matrix, prove public
   delivery equivalence, and run AC-G-79 convergence.

**Legacy Disposition and Decommission.** The v2 corpus becomes superseded but immutable. The v3
candidate remains rejected and unreleased. Hash-only owner review, descriptor verification, and
candidate-captured expectations cannot authorize a current release. DB02/DB04/DB05 prove
current-index and executable cutover.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `gate_b_released_functional_claims_match`;
Executable oracle: `gate_b_owner_acceptance_chain_v2`; Executable oracle:
`gate_b_release_rejects_unaccepted_descriptor_or_captured_candidate`; Executable oracle:
`gate_b_release_operational_gate`.

- **Behavioral — Executable oracle:** `gate_b_released_functional_claims_match` re-executes all
  eleven planes and proves the accepted authored claims, query answers, transitions, public
  equivalence, convergence, and exact integrity projection.
- **Structural — Executable oracle:** `gate_b_owner_acceptance_chain_v2` (name refers to acceptance schema, not
  corpus ordinal) proves authority, candidate, acceptance, corpus manifest, source digests,
  immutable predecessor, and current index form one chain.
- **Negative/Zero-State — Executable oracle:** `gate_b_release_rejects_unaccepted_descriptor_or_captured_candidate`
  rejects absent/rejected/wrong-owner/wrong-digest/self-authored acceptance, descriptor-only or
  actual-captured expectations, surviving/unregistered mutants, and modification of a predecessor
  corpus.
- **Operational — Executable oracle:** `gate_b_release_operational_gate` requires
  `just gate-b-owner-acceptance-check`, `just gate-b-check`, and `just ci-pr` to
  resolve the same new current corpus and execute with `--no-tests=fail` where selectors apply.

**Edit-Local Gates.** Candidate/acceptance validators against copied temporary fixtures and
`just root-fmt`; no released-path write in tests.

**Packet-Local Gates.** `just gate-b-owner-acceptance-check`, `just gate-b-check`,
`just gate-b-functional-candidate-check`, `just gate-b-review-bundle-check`,
`just semantic-oracle-mutants-check gate-b`,
`just wave5-integration-check`, `just wave6-integration-check`, `just adapter-ci-fast`,
`just model-release-check`, `just governance`.

**Integration Milestone.** M05 prerequisite.

**Replan Triggers.** Owner rejection; an owner identifies ambiguity between accepted design
and produced output; immutable release transaction cannot preserve the prior corpus; or
governing inputs change semantically between candidate generation and decision.

**Rollback or Recovery.** Before acceptance, discard only the unreleased candidate. After
acceptance, publish a superseding version; never rewrite, remove, or repoint history to hide an
accepted bad answer.

**Design-Bearing Contracts and Exemplars.** The owner review bundle includes one human-readable
summary per actual plane plus downloadable canonical artifacts and the exact independent diff.
It states that acceptance attests the semantic golden content, not implementation authorship.

### WP08 — Certification, design-corpus reconciliation, and independent closeout

**Outcome.** All corrective and regression gates pass at one certification commit; legacy
proof paths are decommissioned; affected upfront-design documents honestly match the final
implementation; a new status artifact reconstructs completion; and a fresh independent
implementation review approves IR-001–IR-004 closure.

**Dependencies.** WP04, WP05, WP07, WP09–WP12; DB01–DB05.

**Target invariants.** GI-01–GI-13; all Data-Fabric Principles 1–25 and applicable HOL
Principles 1–31.

**Design and library references.** Behavioral-conformance design §§1–6; independent review
Required Remediation Order and Focused Re-Review Scope; proposal §5/§8/§9; SUITE Gates B/C,
AC-G-78, and AC-G-79; QRY §§36/46/48/112; FAB §§12, 70–72, 91, 110–112; LIFE
§§137–138 and §§157–159; SRV §§17/60/68 and runtime invariants; alignment manual review
checklists §§26–34.

**Change surface / Preflight / Known Touch.** Run:

```bash
git diff --stat 412af14566393c2379ba4e174387361cea5370e8..HEAD
just plan-status
just design-principle-traceability-check
just alignment-detector-check
just spec-outline docs/upfront_design --view names
rg -n 'SemanticQueryClause|expectations\.clone|clean_rebuild_equal|plan_artifacts\.is_empty\(\)' src contracts tests justfile
rg -n 'implementation_review_codefabric_design_principles_full_alignment|implementation_status_codefabric_design_principles_full_alignment' docs/reviews
```

Known closeout touch includes only design artifacts whose normative clauses require a decision,
derived `docs/spec_index/` views when their generator reports drift, this plan's future state,
a new versioned implementation-status report, and a new independent implementation-review
report. Production touch is limited to defects discovered by the final/focused gates.

**Required changes.**

1. Run the complete final gate matrix and correct introduced failures. Preserve evidence as
   recipe name plus exit status in the proving workflow; do not paste output or add derived
   check fields to schema-2 state.
2. Review each affected owning design document under `docs/upfront_design/` against final code,
   generated contracts, and production tests. At minimum inspect QRY request/result semantics,
   SUITE Gate B/AC-G-79, FAB query/artifact/publication boundaries, LIFE clean rebuild and
   terminal lifecycle, GEN provider execution, and SRV adapter/query topology. Reconcile
   LD-PG-01 explicitly: owning design prose may name petgraph only as a derived query-local
   physical topology kernel beneath `GraphOperatorPlan`, with DataFusion/Arrow authority and
   the prohibited persistence/index/feature boundaries preserved.
3. Classify every discrepancy as either an intentional implementation improvement that
   strengthens the accepted invariants or an unintended deviation. Intentional improvements
   receive targeted edits in the owning normative document and accountable acceptance before
   their digests become certification inputs. Unintended deviations are fixed in code; they
   are not documented as the new design. A semantic change or weakened invariant triggers
   design reopening rather than closeout editing.
4. Regenerate derived spec-index/model views through their owning tools only when normative
   sources change. Do not hand-edit derived navigation or generated contracts.
5. Complete DB01–DB05 zero-state at the certification commit and re-run their positive and
   negative gates.
6. Use `impl-status` to create a new versioned implementation-status artifact for this plan,
   reconstructing proving commits, ancestry, input freshness, milestones, decommission, and
   current checks. Do not revise the old v3 status.
7. Invoke a fresh, read-only `implementation-review` against accepted design, this plan,
   execution state, baseline/final diff, current behavior, library decisions, legacy cutover,
   and proof. M05 requires `approved` or `approved-with-minor-findings` with no open blocker/
   major finding on IR-001–IR-004. Any `changes-required` result returns to the owning packet.

**Legacy Disposition and Decommission.** Supersede the false M14 completion claim through new
status/review artifacts rather than editing history. DB04 ensures only the corrected corpus and
certification are current while predecessors remain immutable.

**Acceptance Checks.**

Oracle catalog: Executable oracle: `review_remediation_end_to_end_certification`;
Executable oracle: `review_remediation_design_and_authority_closure`; Executable oracle:
`review_remediation_legacy_zero_state`; Executable oracle:
`review_remediation_operational_gate`.

- **Behavioral — Executable oracle:** `review_remediation_end_to_end_certification` selects corrected query,
  artifact, Gate B, and rebuild oracles and their adverse controls at one HEAD.
- **Structural — Executable oracle:** `review_remediation_design_and_authority_closure` validates current contract
  digests, design-principle ownership, query projection equality, LD-PG-01's physical-only
  petgraph boundary, decommission rules, and any regenerated design/index outputs.
- **Negative/Zero-State — Executable oracle:** `review_remediation_legacy_zero_state` runs DB01–DB05 exit rules and
  proves prior corpus/state/review artifacts remain immutable and non-current.
- **Operational — Executable oracle:** `review_remediation_operational_gate` introduces
  `just review-remediation-certification-check` as an aggregate of
  the three focused checks; it is selected by `ci-pr` after WP07 and is part of the final gate
  matrix.

**Edit-Local Gates.** `just typos` for documentation, targeted spec/model validation for each
design edit, and the smallest failing production test for any closeout correction.

**Packet-Local Gates.** `just review-remediation-certification-check`, `just artifacts-check`,
`just plan-status`, `just governance`, `just ci-fast`, `just ci-pr`, plus every recipe in §7.

**Integration Milestone.** M05.

**Replan Triggers.** Design reconciliation would weaken or change a public semantic contract;
any declared input/pin drifts; an independent review finds a blocker/major defect; or final
gates expose a product issue outside the bounded direct-consumer scope that cannot remain a
separate discovered obligation.

**Rollback or Recovery.** Documentation corrections are version-controlled and recoverable.
Do not roll back an accepted corpus or terminal-journal schema in place; publish a superseding
authority or execute the accepted migration. A failed certification leaves this plan
incomplete and the prior historical artifacts untouched.

**Design-Bearing Contracts and Exemplars.** Final design documents include the accepted query
contract authority, terminal artifact lifecycle, true clean-rebuild definition, and Gate B
actual-output/acceptance boundary. The independent review is the certification judgment, not a
generated plan-state field.

## 5. Integration milestones

### M01 — One truthful query contract

WP01 is complete. The form authority and all projections agree exactly; QRY-shaped positive
and negative fixtures pass; retired shortened slugs fail; unsupported execution is withdrawn
before snapshot work. Required recipes: `query-form-contract-check`, `model-repro-check`, and
`adapter-ci-fast`.

### M02 — Complete typed eight-form execution

WP02 and WP03 are complete and DB01 exits. All eight forms and arbitrary composition execute
through one typed scheduler; relational plans stay DataFusion-visible; graph semantics stay in
the application plan and compile into a bounded query-local petgraph physical projection;
parallel fact edges and ordered witnesses survive; Arrow is the seam; production UDS
conformance, determinism, ordering, coverage, and absence pass. Required recipes:
`semantic-query-conformance-check`, `query-determinism-check`, and
`wave5-integration-check`.

### M03 — Terminal provenance closure

WP04 is complete and the artifact half of DB03 exits. Every execution has a phase-appropriate
terminal journal/payload record before notification; all fault points and recovery pass with
one physical execution. Required recipes: `query-artifact-single-execution-check` and
`wave6-integration-check`.

### M04 — Non-vacuous convergence and real Gate B execution

WP05 and WP06 are complete and DB02 plus the rebuild half of DB03 exit. The full corpus runs
true independent semantic rebuilds, and Gate B emits eleven real correlated output planes. The
rejected v3 candidate proves execution/integrity but not independently expected meaning. Required
recipes: `rebuild-equivalence-check`, `wp72-acceptance-check`, and
`gate-b-candidate-check`; none of these alone satisfies M05's functional authority.

### M05 — Accountably released and independently certified

WP09–WP12, WP07, and WP08 are complete and DB04/DB05 exit. A functionally authored,
causally sensitive, owner-accepted immutable corpus is current; Gate B re-executes and satisfies
its independent claims through every public surface; all final gates pass; the design corpus is
reconciled; new status is healthy; and an independent implementation review approves the
correction.

## 6. Cross-packet decommission batches

### DB01 — Reduced query contract and false support

**Prerequisites.** WP01–WP03 and M02.

**Deletes/retires.** The hand-written generic `SemanticQueryClause`/catch-all input contract,
shortened public form slugs, `label` as a surrogate for form-specific semantics, fact-ID-only
relationship scan, consecutive-ID fixed-depth path proxy, one-edge pattern proxy,
custom canonical-ID adjacency `BTreeMap`, node-only `shortest_path`, unconditional singleton
union, single count summary, all-context retrieval, and any support list independent of
executor registration.

**Exit invariants.** `query-form-contract-check` and `semantic-query-conformance-check` green;
zero retired slug constants in current schemas/code/fixtures; zero old generic clause
construction; exact eight-form capability equality; compiler/type checker green after legacy
type deletion; query code contains one petgraph-backed physical projection and zero retired
adjacency/shortest-path implementations; `NodeIndex`/`EdgeIndex` do not cross public, durable,
Arrow, or provenance boundaries; structural query and `rg` zero-state cover Rust, Python,
contracts, tests, tooling, and generated outputs while excluding historical docs/reviews and
immutable prior corpora.

### DB02 — Descriptor/self-confirming Gate B proof

**Prerequisites.** WP09–WP12, WP07, M05.

**Deletes/retires.** Current-path `derive_expectations` descriptors,
`candidate_contracts(expectations.clone())`, literal/file/registry dispatch as the released
Gate B executor, three-form descriptor expectations, and any candidate code that manufactures
both expected and actual. This includes shallow `requirement_checks`, check-name expected
digests, unconditional match booleans, and non-empty aggregate success. Prior released corpus
bytes and verifier history remain read-only.

**Exit invariants.** `gate-b-functional-candidate-check` captures real outputs and compares them
with authored claims through an independent evaluator; `gate-b-check` re-executes the production
vertical; the complete semantic-mutant and omitted-provider/publication/UDS/MCP/stream/artifact
matrix fails as predicted; structural plus textual zero-state proves no current corpus/release
path cites captured output, descriptor execution, digest equality, or non-empty labels as
semantic evidence.

### DB03 — Empty terminal artifacts and same-wave rebuild certification

**Prerequisites.** WP04, WP05, M03, M04.

**Deletes/retires.** Universal empty plan-vector acceptance for failed/cancelled executions,
ignored artifact persistence failures, event-before-journal paths, `clean_rebuild_equal` as
AC-G-79 evidence, disjoint five-stage/full-corpus completion composition, and blanket
`semantic_capabilities_required: false`.

**Exit invariants.** `query_failure_artifact_closure` and
`full_golden_scenario_clean_rebuild_equivalence` green; each `NOT_REACHED` plan state names the
pre-planning phase; no allocated execution lacks a journal record in fault tests; all sixteen
scenarios use independent clean roots and semantic-capability profiles; compiler and
structural/textual zero-state green.

### DB04 — Superseded corpus and false completion as current authority

**Prerequisites.** WP07, WP08, M05.

**Deletes/retires.** Current-index authority of the descriptor-based corpus, any active/current
status claim that the reviewed v3 implementation is certified, and any gate that can resolve a
superseded acceptance without explicit historical selection.

**Preserves.** Every prior plan, state, status/review artifact, corpus, manifest, candidate, and
acceptance decision byte-for-byte as immutable history.

**Exit invariants.** The corpus index, Gate B recipes, and new status resolve the corrected
accepted version; predecessor digests remain unchanged; `artifacts-check`/`plan-status` are
healthy for this plan; focused independent review approves IR-001–IR-004 closure.

### DB05 — Self-referential golden and candidate-local scenario authority

**Prerequisites.** WP09–WP12, WP07, M05.

**Deletes/retires.** `requirement_checks` as semantic expectation, check-label expected digests,
unconditional `matches: true`, non-empty aggregate success, `functional_candidate_projection`
and `normalize_gate_b_planes` as semantic comparison authority, source-string-only independence
tests, production hard-coded scenario edit dispatch, final-serialization-only adverse mutations,
direct candidate-module provider ownership, and any acceptance path that can promote the rejected
v3 candidate.

**Preserves.** Historical v1/v2 release verification, rejected v3 candidate bytes and detached
rejection decision, real-provider vertical execution, governed comparison-ignore registry,
AC-G-79 clean rebuild, canonical integrity/correlation, and public transport tests.

**Exit invariants.** `functional-golden-independence-check`,
`semantic-oracle-mutants-check gate-b`, `gate-b-projection-registry-check`,
`gate-b-rejected-candidate-zero-state-check`, and `gate-b-check` pass. Covered structural/text
search over Rust, Python, tests, contracts, tooling, and justfile finds no active self-authorizing
semantic path; historical immutable artifacts/docs are explicitly excluded and reported.

## 7. Final gate matrix

All entries are justfile recipes. A packet that introduces a new recipe does so before any
dependent packet cites it. Test-selecting recipes use `--no-tests=fail` and oracle-substance
governance.

- Corrective focus: `just query-form-contract-check` ·
  `just semantic-query-relational-conformance-check` ·
  `just semantic-query-conformance-check` · `just query-determinism-check` ·
  `just query-artifact-single-execution-check` · `just rebuild-equivalence-check` ·
  `just wp72-acceptance-check` · `just gate-b-candidate-check` ·
  `just functional-golden-contract-check` ·
  `just functional-golden-independence-check` ·
  `just gate-b-rejected-candidate-zero-state-check` ·
  `just gate-b-public-vertical-check` · `just gate-b-causal-check` ·
  `just gate-b-delivery-equivalence-check` ·
  `just gate-b-projection-registry-check` ·
  `just semantic-oracle-mutants-check gate-b` ·
  `just gate-b-review-bundle-check` ·
  `just gate-b-functional-candidate-check` · `just gate-b-predecessor-check` ·
  `just gate-b-owner-acceptance-check` · `just gate-b-check` ·
  `just review-remediation-certification-check`
- Cross-wave/product: `just wave5-integration-check` · `just wave6-integration-check` ·
  `just wave7-integration-check` · `just git-parity-check` ·
  `just source-capture-race-check` · `just data-fabric-upgrade-check` ·
  `just vacuum-dry-run-check`
- Data/provider contracts: `just stable-graph-check` · `just features-each` ·
  `just provider-protocol-check` · `just publication-referential-integrity-check` ·
  `just id16-extension-contract-check` · `just provider-statistics-contract-check`
- Stable root: `just root-fmt` · `just root-check` · `just root-clippy` ·
  `just root-test` (nextest plus doctests) · `just deps-fast` · `just policy` · `just typos`
- Other build domains: `just extractor-ci-fast` · `just sidecar-ci-fast` ·
  `just adapter-ci-fast` · `just adapter-stdio-test` · `just adapter-wheel-test`
- Model/governance: `just model-check` · `just model-plan-check` ·
  `just model-repro-check` · `just model-transaction-check` · `just model-release-check` ·
  `just design-principle-traceability-check` · `just alignment-detector-check` ·
  `just audit-baseline-check` · `just oracle-substance-check` ·
  `just plan-dependency-check` · `just artifacts-check` · `just plan-status` ·
  `just governance-scan` · `just governance`
- Aggregate: `just ci-fast` · `just ci-pr`

`just gate-b-owner-accept` remains a confirm-gated mutating accountable action and is never a
gate dependency. The bounded deterministic semantic-mutant recipe is required; general
`cargo-mutants` and Miri are not in this matrix for the reasons in §1.2.

## 8. Execution sequence

Normative dependency edges:

```text
WP01 → WP02 → WP03 ─┬→ WP04 ─────────┐
                    └→ WP05 ─────┐   │
WP03 + WP04 + WP05 ─────────────→ WP06 (M04)
WP01 + WP03 + WP05 + WP06 ──────→ WP09 → WP10
WP04 + WP05 + WP06 + WP09 + WP10 → WP11 → WP12 → WP07
WP04 + WP05 + WP07 + WP09–WP12 + DB01–DB05 ─────→ WP08

DB01 after WP03 / M02
DB03 after WP04 + WP05 / M03 + M04
DB02 after WP09–WP12 + WP07
DB04 after WP07 + WP08
DB05 after WP09–WP12 + WP07
```

Recommended linearized execution:

```text
WP01 (M01)
  → WP02 → WP03 → DB01 (M02)
  → WP04 (M03)
  → WP05 → DB03
  → WP06 (M04; rejected execution/integrity evidence)
  → WP09 → WP10 → WP11 → WP12
  → DB02/DB05 candidate-side exit
  → pause for WP07 accountable decision and immutable release
  → DB02/DB05 release-side exit → WP08 → DB04 (M05)
```

Each packet gets its own proving commit. Shared recertification commits do not replace packet
proof. Implementation may adapt internal file decomposition, but any packet-boundary,
dependency, public-contract, extension-level, or proof-obligation change requires a versioned
plan revision and state deviation record.

## 9. Plan risks and replan policy

1. **Public corrective cutover.** The normative form-specific QRY 1.3 schema is incompatible
   with the implemented generic request shape. The generic shape was not the accepted contract,
   so no alias is preserved. Mitigation: withdraw unsupported forms first, generate all
   projections together, keep canonical JSON/Proto envelope stable, and prove positive/negative
   public fixtures before activation.
2. **Semantic phrase breadth.** QRY uses controlled plain-language phrases, while the registry
   owns a bounded supported vocabulary. Mitigation: compile only governed phrases and modifiers,
   return resolved interpretation, and advertise unsupported semantic families conservatively;
   never guess or name-match.
3. **Graph/path complexity.** All-simple-path requests can be combinatorial. Mitigation: QRY
   boundedness checks, maximum length/output/memory, cancellation, deterministic ordering, and
   explicit incomplete/unknown results. A requirement that cannot remain bounded reopens
   design.
4. **Petgraph semantic mismatch and physical leakage.** A convenient petgraph algorithm can
   still be wrong for CodeFabric: shortest-path helpers omit ordered fact-edge witnesses,
   isomorphism assumes induced non-multigraph input, traversal output order is not canonical,
   and graph-local indices are not domain identity. Mitigation: LD-PG-01, a parallel-edge
   `DiGraph`, private external-ID maps, DataFusion prefilters, trait-generic bounded algorithms,
   differential reference tests, structural prohibitions, and a fingerprinted query-local
   projection. If cancellation or accepted scale requires a long-lived graph service/cache,
   reopen design instead of letting the derived projection become authority.
5. **Artifact migration and failure recursion.** The journal/payload cutover can itself fail.
   Mitigation: one terminal authority, bounded fallback envelope, idempotent checksum,
   crash-recovery tests, and fail-closed service-unavailable behavior when the journal cannot
   commit. Never claim durability after an ignored write error.
6. **Gate B operational cost.** Real rustc/Python providers, Delta publication, UDS, and adapter
   execution are materially heavier than descriptors. This is intentional Gate-B evidence, not
   an every-edit unit test. Keep focused local tests cheap; run the vertical at packet,
   candidate, release, and CI-pr milestones with shared build caches and bounded fixtures.
7. **Clean-rebuild cost and real defects.** Sixteen scenarios with semantic providers may expose
   product convergence defects and take substantial time. Treat each mismatch as a discovered
   product obligation; preserve a reproducible bundle, fix the smallest owner, and never weaken
   AC-G-79, skip a scenario, or disable the semantic lane to close the plan.
8. **Human checkpoint.** WP07 may remain legitimately blocked while awaiting review. The
   executor must explain that the owner reviews authored intended behavior, decoded actual
   outputs, semantic diffs, causal mutants, convergence, delivery equivalence, limitations, and
   separate integrity evidence—not implementation authorship or merely a hash. No agent
   self-approval.
9. **Design-doc closeout bias.** Updating design to match code can hide an unintended defect.
   Mitigation: classify discrepancies against accepted invariants first; only intentional
   improvements receive normative edits and accountable acceptance; independent review checks
   the final decision.
10. **Input and owner-worktree drift.** Any declared-input digest, dependency pin, or owner-path
   attribution change invalidates assumptions. Re-run `artifacts-check`, current-tree preflights,
   and `plan-status`; preserve unrelated owner modifications.
11. **Reference-evaluator drift.** A convenient addition can turn the small evaluator into a
   duplicate compiler/query engine or make it share a production defect. Mitigation: authored
   source meaning, closed logical claim vocabulary, standard-library-only reference laws,
   structural import isolation, and required known-wrong mutants. Reopen design if expected
   semantics cannot remain independently reviewable.
12. **False causal controls.** Mutating only a final JSON diff can show comparator sensitivity
   without proving the provider, publication, stream, artifact, or adapter caused the result.
   Mitigation: inject faults before named producing/public seams and declare both affected and
   unaffected claims; surviving or over-broad mutants block WP12.

**Replan policy.** Implementation-local mechanism changes that preserve packet outcome,
dependencies, invariants, cutover, and executable evidence are recorded in schema-2 execution
state. A new plan version is required for packet/sequence changes, a new public compatibility
period, a different terminal-artifact authority, a different Gate B acceptance model, or a
changed clean-rebuild equality contract. Design reopening is mandatory for any dependency pin
move, petgraph feature expansion or authoritative/cross-query graph state, custom DataFusion
extension/UDF/planner proposal, QRY/SUITE/FAB/LIFE semantic change, owner rejection based on
specification ambiguity, or proposed weakening of an invariant.
