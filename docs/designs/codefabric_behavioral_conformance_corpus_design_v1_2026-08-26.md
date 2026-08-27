---
artifact: design-dossier
design_id: codefabric-behavioral-conformance-corpus
version: v1
date: 2026-08-26
status: accepted
baseline_commit: a3efb30d699f84a0d6f190a5ff3c2574bfcf039e
working_tree_digest: 0d81d9864b0246a3f407212fd6085c7d4ce755eff21eb31f9d35f9075e3f2c84
primary_scope:
  - tests/golden/
  - tests/functional_golden/
  - src/gate_b_candidate.rs
  - src/gate_b_candidate/
  - src/gate_b_release.rs
  - src/golden_corpus.rs
  - src/fabric/serving.rs
  - src/lifecycle.rs
  - src/daemon.rs
  - tooling/gate_b_candidate.rs
  - tooling/gate_b_adapter_probe.py
  - tooling/ci/plan_assurance.py
  - codefabric-cpg-mcp/tests/
  - justfile
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric behavioral conformance corpus design v1

## 1. Executive decision

CodeFabric will replace the active Gate B candidate's self-referential comparison with an
independently authored, behavior-first conformance system. A golden is authoritative only for
the intended semantic behavior it states and the independently observed behavior it compares.
References, canonical identifiers, checksums, fingerprints, registry censuses, and artifact
digests authenticate which execution and contract were reviewed; they do not determine whether
CodeFabric understood the fixture correctly.

The rejected `codefabric-golden-v3.0.0-candidate.1` remains immutable rejected evidence. It must
not be accepted, released, or used as the expectation source for a successor. Released v1 and v2
corpus bytes and their acceptance records also remain immutable. The successor is a new version
whose semantic expectations exist before execution and cannot be generated or rewritten by the
system under test.

The selected authority has three deliberately separate parts:

1. **Authored behavior claims** state what a small source fixture means, what an edit changes,
   what remains unchanged, and what a public query or failure must return. They select entities
   by reviewable source anchors and semantic attributes, never by copied runtime identifiers.
2. **Independent reference evaluators** derive only small, explicit consequences of those
   authored claims: set/bag equality, cardinality, adjacency, bounded paths, grouping, ordering,
   transition deltas, completeness, and negative-proof rules. They use test-only standard data
   structures and must not import the production query compiler, reconciliation engine,
   provider adapters, candidate generator, DataFusion plan, or petgraph execution path.
3. **Black-box public observations** run the real providers, canonical Arrow/Delta fabric,
   daemon, UDS gRPC service, result artifact store, and FastMCP STDIO process. The harness decodes
   actual output at public boundaries, resolves authored selectors against it, and reports every
   claim as matched, missing, unexpected, ambiguous, or blocked by an explicit capability gap.

This decision implements SUITE AC-G-78's normative executable corpus and AC-G-79's independent
clean-rebuild comparator, QRY §§36, 46, 48, 104, and 112, FAB §112, LIFE §§157–159, SRV §68,
and the functional exits of RM §§10 and 13–17. It corrects the current candidate, where
`derive_diff` digests shallow check names, writes `matches: true` unconditionally, and treats
non-empty check lists as aggregate success.

### 1.1 First-principles meaning

“First principles” has an operational definition here:

- fixture source is intentionally small enough for a reviewer to reason about directly;
- a claim cites the governing semantic rule and a human-readable source anchor;
- expected entities, facts, relations, unknowns, completeness, and public query answers are
  authored independently of a CodeFabric run;
- causal edits predict named additions, removals, changes, and preserved observations;
- negative claims name the searched universe and require the completeness state that makes
  absence meaningful;
- a known semantic fault must make the relevant claim fail; and
- the same semantic result must be observable through canonical tables, UDS result/stream,
  persisted artifact readback, and FastMCP structured output where those surfaces apply.

## 2. Constraints and target invariants

### 2.1 Authority invariants

**D-01 — Expected meaning is authored, not captured.** The expectation tree contains source
fixtures, declarative operations, typed semantic claims, named queries, and expected public
outcomes. It contains no expected runtime digest, canonical ID literal, descriptor identity,
registry census, candidate row dump, or unconditional comparison result. A candidate command has
read-only access to expectations and no output path beneath the expectation tree.

**D-02 — Production supplies observations only.** Production provider, reconciliation, query,
DataFusion, Delta, UDS, artifact, and FastMCP code may produce or decode actual observations. It
may not synthesize expected claims, expected query answers, or the reference evaluator's graph.

**D-03 — Reference logic is smaller and independent.** The reference evaluator operates over
authored claim records using `BTreeMap`, `BTreeSet`, stable sorting, and direct bounded algorithms.
It does not reproduce Python or Rust parsing, type checking, compiler lowering, reconciliation,
identity construction, DataFusion planning, lifecycle scheduling, or daemon logic. Provider
outputs are compared to claims; they are never fed back into expectation generation.

**D-04 — No unchecked remainder.** Every observation within a scenario's declared proof universe
is either expected, explicitly allowed with a reason, or a failure. Exact bag multiplicity is the
default for canonical rows. “At least these rows” is permitted only for an explicitly open
diagnostic plane and can never prove a COMPLETE or PROVEN_EMPTY semantic result.

**D-05 — Absence requires a proof universe.** `absent`, `count: 0`, `PROVEN_EMPTY`, and COMPLETE
claims carry the language/profile, owner/context, fact families, provider/capability state,
snapshot identity relation, and query scope over which absence is asserted. Missing, stale,
partial, unsupported, or redacted evidence produces an explicit unknown or capability gap.

**D-06 — Behavior and integrity are orthogonal.** Semantic conformance, causal sensitivity,
transport equivalence, clean-rebuild convergence, canonical byte integrity, correlation, and
release-chain integrity are separately named gates. No digest or successful transport replaces a
semantic claim; no semantic claim replaces byte/provenance integrity.

### 2.2 Runtime and repository invariants

- The root Rust daemon remains the sole source-state, snapshot, provider-orchestration, query,
  and capability authority. Python remains presentation-only.
- Provider isolation, application-owned identity, raw/normalized coexistence, authority-based
  reconciliation, owner-scoped replacement, manifest-pinned MVCC, and one immutable snapshot per
  query remain unchanged.
- The exact FAB §2.1 dependency baseline remains unchanged. No new Cargo root, workspace,
  top-level Rust integration-test target, native Python extension, or Python Arrow/DataFusion
  processing layer is introduced.
- The real Rust UDS boundary and explicit FastMCP `StdioTransport` are mandatory for release
  conformance. In-memory FastMCP tests remain useful component tests but cannot prove STDIO
  process isolation, environment, serialization, or lifecycle.
- Tests use fresh independent roots and fresh STDIO subprocesses where state isolation matters.
  Shell environment required by the adapter is passed explicitly.
- Scenario operations are data, not production `match` arms. The harness supports exact byte
  replacement, file add/remove/rename, context/config replacement, provider fault injection,
  watcher loss, overlay flush, restart, authorization change, and deterministic barrier waits.
- Fingerprints and canonical IDs may be asserted relationally—for example, equal across public
  surfaces, changed after a semantic edit, or stable for an unaffected owner—but an authored
  literal is never the semantic expectation.

### 2.3 Required semantic coverage

The Gate B successor is deliberately small but functional. It must prove, from source to MCP:

- one Python owner with function, parameter, binding/reference, call-site, direct def-use, CFG,
  type/call-target enrichment when available, and explicit partial/unknown behavior when not;
- one Rust owner with definition/type, MIR body/local/basic block, CFG/call, place/access, and
  explicit compiler-capability withdrawal without stale-current compiler facts;
- at least one property, relation, derived result, provenance record, diagnostic, unknown, and
  capability record whose exact meaning a reviewer can derive from the source;
- all eight QRY request forms, including exact rows/groups/paths/source context, deterministic
  order, composition, status, coverage, and negative-proof behavior;
- one causal Python edit, one causal Rust edit, one parse/compile failure and recovery, one
  provider withdrawal, one context change, one multi-owner update, one restart, one watcher-loss
  reconciliation, one overlay flush, and one ACL-redaction transition;
- incremental current effective state equal to an independently constructed clean rebuild as
  schemas and duplicate-sensitive row bags, while the authored transition claims independently
  establish that the shared state is semantically correct; and
- UDS stream terminal behavior, artifact content/readback, and FastMCP structured output that
  decode to the same semantic result as the canonical query observation.

Waves 8–12 extend the same contract, not a parallel golden format. Each work packet adds claims
for its fact families and failure semantics; each wave closes only when its profile claim pack,
causal mutants, public queries, and clean-rebuild convergence pass.

## 3. Target architecture — contracts, ownership, flows, and library decisions

### 3.1 Artifact layout and ownership

The successor uses this conceptual layout; exact semantic source decomposition remains an
implementation decision:

```text
tests/golden/codefabric-golden-v4/
  corpus-manifest.json                 integrity and release metadata only
  workspace/                           minimal reviewable Python/Rust/FFI sources
  profiles/
    gate-b/
      claims.json                      authored semantic claims
      queries.json                     named requests and expected semantic answers
      scenarios/*/scenario.json        declarative operations and checkpoint claims
      allowed-observations.json         narrow reasoned open diagnostics, if any
    wave8/ ... wave12/                  additive claim packs

tests/functional_golden/
  claim_schema.rs                      strict authored-contract parser/validator
  reference_query.rs                   independent set/graph/group/order evaluator
  transition.rs                        before/after delta evaluator
  observation.rs                       public-output decoders and selector resolution
  mutations.rs                         registered counterfactual/fault operators
  report.rs                            human-readable evidence dossier
```

The expectation tree is repository-owner-authored input. The candidate runner may write only to
a fresh review-candidate output directory. The release transaction copies an already reviewed
candidate and its detached decision metadata; it never copies actual observations into the
expectation tree and never derives an expected outcome from the candidate.

### 3.2 Typed behavior-claim contract

Every claim has a stable claim ID, normative reference, checkpoint, proof universe, selector,
predicate, expected cardinality or value, and the surfaces on which it must be observed. The
closed initial predicate vocabulary is:

- `entity`, `fact`, `property`, and `relation` with exact kind/value/cardinality;
- `unknown` and `capability` with reason, provider, scope, and currentness;
- `query_rows`, `query_paths`, `query_groups`, and `source_context` with bag/order rules;
- `response_status`, `coverage`, and `negative_proof`;
- `added`, `removed`, `changed`, and `preserved` across checkpoints; and
- `stream_event`, `artifact_result`, and `mcp_result` equivalence to a named query result.

Selectors use language, repository-relative path, authored anchor name, source text/span
relation, semantic kind, qualified name where the source defines one, owner/context, and
property/relation kind. The observation resolver fails on zero or multiple matches unless the
claim explicitly states a cardinality. Runtime IDs remain in the actual evidence report for
traceability but are not expectation keys.

An abbreviated authored example is:

```json
{
  "claim_id": "GB-PY-CALL-EDIT-01",
  "reference": "GEN §23",
  "checkpoint": "after-target-replacement",
  "universe": {
    "profile": "PYTHON_SEMANTIC_V1",
    "owner": "workspace/python/calls.py",
    "families": ["entity", "call_site", "relation", "coverage"]
  },
  "expect": {
    "relation": "CALLS",
    "from": {"anchor": "caller", "kind": "FUNCTION"},
    "to": {"anchor": "replacement", "kind": "FUNCTION"},
    "resolution": "EXACT",
    "count": 1
  },
  "must_be_absent": {
    "relation": "CALLS",
    "from": {"anchor": "caller"},
    "to": {"anchor": "old_target"}
  },
  "surfaces": ["canonical", "uds", "artifact", "mcp"]
}
```

Expected query answers reference claim-selected logical records and literal scalar values, not
serialized production rows. The reference query evaluator performs straightforward filtering,
adjacency/BFS, conjunctive binding joins, set/bag operations, grouping, aggregation, and stable
ordering over the authored logical records. It explicitly reports which QRY semantics it covers;
unsupported evaluator behavior is a plan blocker, not permission to reuse production code.

### 3.3 Execution and comparison flow

```text
authored sources + claims + operations
              |                         independent reference evaluator
              |                                      |
              v                                      v
fresh workspace -> real providers -> reconciliation -> canonical Arrow/Delta snapshot
                                                |             |
                                                |             +-> decoded canonical observation
                                                v
                                      daemon UDS query/stream
                                                |
                                  result artifact readback
                                                |
                                  FastMCP STDIO structured result
                                                |
                                                v
                                public observation decoder
                                                |
                     selector resolution + claim/query/delta comparison
                                                |
                         semantic report + separate integrity report
```

The runner applies each declared operation and waits on deterministic lifecycle barriers. At each
checkpoint it performs the named public queries, decodes canonical tables and public responses,
resolves selectors, compares every authored claim, rejects unexpected closed-universe rows, and
records a concise source-to-claim-to-observation diff. The clean-rebuild comparator runs from a
fresh inventory/source capture/provider/publication/serving root and compares the full governed
projection. That comparator proves convergence; the authored claims prove meaning.

### 3.4 Causal intervention and mutation authority

Every semantic axis in a claim pack registers at least one mutation expected to make the pack
fail. Required mutations include:

- omit a required entity/fact/relation/query row;
- duplicate a bag row or change exact cardinality;
- swap relation subject and object;
- change semantic kind or property value;
- downgrade EXACT to POSSIBLE or collapse an unresolved candidate set;
- remove an expected unknown/capability gap;
- mark stale evidence CURRENT or emit compiler facts after compilation failure;
- claim COMPLETE or PROVEN_EMPTY with an incomplete universe;
- merge distinct analysis contexts or leak an ACL-redacted source field;
- shift the source anchor/span to a wrong construct;
- suppress a provider before its output, publication, snapshot activation, UDS chunk/terminal,
  artifact persistence/readback, or FastMCP adaptation;
- corrupt canonical Arrow content while preserving outer reference fields; and
- perturb only incremental execution so clean rebuild and incremental state diverge.

Mutations operate at the producer or public-boundary seam, not only on the final serialized diff.
The aggregate gate fails if any registered required mutant survives. Property-based generation may
vary authored logical graphs, request order, and bag multiplicity, but generated expectations are
confined to mathematical laws over the authored model and never used to bless provider semantics.

### 3.5 Human review dossier and accountable decision

The candidate review bundle is decoded and claim-oriented. For every scenario it includes:

- the source before/after and exact declared operation;
- the normative claim in plain form;
- the named query/request;
- expected logical records and result;
- decoded actual canonical, UDS, artifact, and MCP observations;
- a semantic diff with missing, unexpected, ambiguous, and blocked claims;
- causal-intervention results and any surviving mutant;
- clean-rebuild comparison, coverage/unknown limitations, and integrity metadata; and
- a summary that distinguishes semantic conformance from execution/integrity conformance.

The accountable owner accepts or rejects the authored contract version and its observed evidence.
Acceptance records the immutable bundle digest only after semantic review; the digest identifies
the decision's subject and never supplies its reasoning.

### 3.6 Library and platform decisions

### LD-01 — Serde and canonical JSON for the claim boundary

**Decision:** retain-current.

**Version basis:** the current root `serde`, `serde_json`, canonical-JSON, and contract-model
feature surfaces at the FAB §2.1 repository baseline; no dependency or feature expansion.

**Displaces:** ad hoc fixture parsing and untyped map access. Strict application-owned claim DTOs
parse human-authored JSON and reject duplicate keys, unknown fields, invalid predicate shapes,
unresolved anchors, and open proof universes where a closed claim is required.

**Risk:** canonical serialization can be mistaken for semantic correctness. The claim validator
uses it only for deterministic contract bytes and integrity; semantic comparison remains typed.

**Validation:** `just functional-golden-contract-check`.

### LD-02 — Standard collections for independent reference semantics

**Decision:** build.

**Version basis:** Rust standard library `BTreeMap`, `BTreeSet`, `VecDeque`, and stable sort at the
repository Rust floor; no third-party dependency.

**Displaces:** reuse of the production DataFusion, petgraph, reconciliation, or query execution
path as its own oracle.

**Risk:** a second full semantic engine would drift and create competing authority. The evaluator
is restricted to explicit logical records and small relational/graph laws; it never parses source
or assigns canonical identity.

**Validation:** `just functional-golden-independence-check`.

### LD-03 — Proptest for semantic-law and mutant sensitivity

**Decision:** retain-current.

**Version basis:** the current test-only `proptest` dependency and feature graph.

**Displaces:** a handful of example-only comparator tests for ordering, bag multiplicity,
relation direction, path bounds, composition, and negative-proof algebra.

**Risk:** generated tests can reproduce production assumptions. Strategies generate only small
authored logical models and counterfactual observations; provider expectations remain human
authored.

**Validation:** `just semantic-oracle-mutants-check all`.

### LD-04 — Arrow, DataFusion, Delta, and petgraph remain systems under test

**Decision:** retain-current.

**Version basis:** Arrow/Parquet 59.2.0, DataFusion 55.0.0, delta-rs exact revision `43a0cf10`,
`object_store` 0.13.2, and petgraph 0.8.3 with the already accepted feature set.

**Displaces:** none in the expectation evaluator. Their production outputs are decoded and
cross-checked through governed public schemas; their optimizer, storage, and graph behavior never
generates expected meaning.

**Risk:** shared normalization can hide a defect. The observation decoder reads governed public
fields and the comparison-ignore registry; a test-only evaluator does not call production
normalizers. Causal Arrow-content corruption and registry-closure tests detect masking.

**Validation:** `just gate-b-public-vertical-check` and
`just gate-b-projection-registry-check`.

### LD-05 — Tonic/Prost and FastMCP public clients for delivery proof

**Decision:** retain-current.

**Version basis:** tonic/tonic-prost 0.14.6, prost 0.14.4, grpcio 1.83.0, Protobuf 7.36.0,
FastMCP 3.4.7, and Pydantic 2.13.4 at the current locked graphs.

**Displaces:** direct adapter-function calls, descriptor-only assertions, and in-memory-only MCP
claims at the release boundary. Generated stubs and strict models remain boundary mechanisms, not
domain validators.

**Risk:** a shared generated schema can prove transport agreement while both ends carry a wrong
semantic result. Tests compare decoded UDS, artifact, and STDIO results with independently expected
logical answers and also assert transport equivalence.

**Validation:** `just gate-b-delivery-equivalence-check`.

### LD-06 — Snapshot libraries are diagnostic, not semantic authority

**Decision:** reject.

**Version basis:** the current test-only `insta` dependency may remain for reviewable diagnostic
renderings; it is not an accepted-answer mechanism for functional claims.

**Displaces:** automatic acceptance of a large captured production object as the expected result.

**Risk:** mechanically accepted snapshots can preserve the same implementation defect indefinitely.
All semantic expectations stay in the typed authored claim/query format; snapshot changes never
make a semantic gate green.

**Validation:** `just functional-golden-independence-check`.

### 3.7 Doctrine posture

- **Advances:** holistic Principles 17, 25, 27, 30, and 31 by separating a functional core from
  execution, making reproducible claims provenance-bearing, and turning intended behavior into
  executable governance. Data-fabric Principles 18, 19, 24, and 25 are advanced by distinguishing
  identity fingerprints from correctness, recording semantic observations, and deriving tests
  from contracts and invariants.
- **Maintains:** single semantic authority, application-owned identity, provider isolation,
  immutable present-state snapshots, Arrow as the typed data fabric, DataFusion as the canonical
  relational runtime, Delta as publication state, and FastMCP as presentation only.
- **Risk-mitigated:** the unavoidable test-only reference implementation is bounded to explicit
  mathematical consequences of authored claims and structurally prevented from importing
  production semantic engines. Mutant sensitivity demonstrates that this separation is effective,
  not merely declared.

## 4. Alternatives and clean-sheet challenge

### 4.1 Keep the reference/hash candidate as the golden — rejected

The candidate proves that a coherent vertical ran, emitted structurally populated planes, and
produced stable bytes. It does not prove the source-to-fact mapping, relation direction, unknown
semantics, query answer, or negative-proof outcome. `derive_diff` cannot disagree with a wrong but
populated candidate. Retain this machinery only as integrity and execution evidence.

### 4.2 Capture full current output and review/accept the snapshot — rejected

A full capture improves review visibility but still makes production output the expectation.
Large minified captures are hard to reason about, couple expected results to volatile physical
fields, and preserve shared producer/normalizer defects. Decoded captures belong in the review
dossier as actual evidence, not in the expectation authority.

### 4.3 Build a second parser/compiler/query engine — rejected

A fully independent implementation could provide differential evidence but would duplicate Ruff,
Pyrefly, rustc/MIR, reconciliation, DataFusion, and lifecycle semantics. It would be expensive,
drift-prone, and eventually become a second product authority. The selected evaluator is purposely
smaller: humans author source meaning, while code derives only transparent logical consequences.

### 4.4 Differentially compare upstream providers — retained as supporting evidence

Direct Ruff/Pyrefly/rustc observations can expose adapter loss, and clean rebuild exposes
incremental divergence. Neither defines CodeFabric's normalized ontology, authority resolution,
unknown algebra, query response, or MCP outcome. Provider differential checks remain packet-local
evidence subordinate to authored CodeFabric claims.

### 4.5 Declarative claims plus black-box execution, small reference laws, and mutants — selected

This is the smallest architecture that has independent expected meaning, exercises the actual
system, detects shared-normalization defects, and remains reviewable. It scales additively through
Waves 8–12 without freezing volatile IDs or turning a second compiler into the oracle.

The fresh independent design challenge and library-impact review both reached this selection.
They additionally require causal faults at producing seams, claim-by-claim human evidence, and an
acyclic predecessor repair before Waves 8–12 activation.

## 5. Transition, cutover, and legacy disposition

### 5.1 Acyclic sequence

1. Record the owner's rejection outside the immutable candidate directory and block WP07
   acceptance of `codefabric-golden-v3.0.0-candidate.1`.
2. Revise the active remediation plan as a new version that implements the Gate B behavior-claim
   pack, independent evaluators, black-box observations, causal mutants, review dossier, and
   decision transaction. Its activation prerequisite is the already proved predecessor M04/WP06,
   not the rejected M05.
3. Generate a new successor candidate ID. Run all semantic, causal, clean-rebuild, delivery, and
   integrity gates; then pause for the accountable owner to review and accept or reject it.
4. Only an accepted functional successor closes predecessor M05 and permits Waves 8–12 activation.
5. Revise the Waves 8–12 plan as a new version. Preserve WP01–WP38 semantics and identifiers,
   add the functional-claim mechanism as an inherited dependency, and make every wave integration
   gate run its additive profile pack and mutant matrix.
6. At Wave 12, execute the union of Gate B and Wave 8–12 packs through the complete AC-G-78
   scenarios and the required GEN, FAB, LIFE, QRY, and SRV conformance surfaces.

This two-plan sequence avoids activating downstream semantic-profile work while its accountable
predecessor remains rejected. It also avoids moving Gate B acceptance into the downstream plan,
which would create an activation cycle.

### 5.2 Legacy matrix

| Existing surface | Disposition | Replacement proof |
|---|---|---|
| Released `codefabric-golden-v1` and `v2` | Preserve immutable historical evidence. | Existing release-chain checks. |
| Rejected `codefabric-golden-v3.0.0-candidate.1` | Preserve bytes; add detached rejection decision; exclude from release index. | `just gate-b-rejected-candidate-zero-state-check`. |
| `requirement_checks` and check-name digests | Remove from active semantic comparison. | Typed authored claims and exact per-claim diff. |
| Unconditional `matches: true`/non-empty aggregate | Delete. | Fail-closed evaluator and mutant-killing gate. |
| `functional_candidate_projection` and `normalize_gate_b_planes` | Decommission as semantic authority; use governed comparison registry only for declared operational fields. | `just gate-b-projection-registry-check`. |
| Production hard-coded scenario edit switch | Replace with declarative byte/file/fault operations in fixtures. | `golden_scenario_semantic_transition_contracts`. |
| Source-string “independent oracle” test | Delete. | Structural import/write isolation plus behavioral self-tests and mutants. |
| Descriptor/reference-only expected planes | Retain only where they are contract-integrity evidence; never count them as functional claims. | Separate integrity report/gate. |
| `gate-b-check` | Keep as the stable aggregate name after it depends on functional, causal, rebuild, delivery, integrity, and accepted-decision gates. | `just gate-b-check`. |
| Direct candidate-module provider launch helpers used by rebuild tests | Move to neutral runtime/test support or route through `ProviderRuntime`. | `just rebuild-equivalence-check` plus structural zero-state check. |
| Waves 8–12 v2 plan | Preserve immutable draft; supersede with a version that names the independent authority for every exact-row oracle. | Plan audit and artifact checks. |

No legacy surface is deleted until its replacement gate passes at the proving commit and HEAD.
Rollback restores only routing to the last accepted released corpus; it never promotes the
rejected candidate or rewrites historical acceptance records.

## 6. Proof strategy

### 6.1 Required executable gates

The implementation plan must add intent-level recipes rather than embedding tool flags:

| Proof question | Required recipe |
|---|---|
| Are authored claims strict, closed, source-anchored, and free of observed IDs/digests? | `just functional-golden-contract-check` |
| Is expectation code structurally and behaviorally independent of production semantic engines and candidate writers? | `just functional-golden-independence-check` |
| Does the real provider-to-Delta-to-daemon path satisfy Gate B semantic claims? | `just gate-b-public-vertical-check` |
| Do named source changes cause the predicted additions/removals/preservation? | `just gate-b-causal-check` |
| Does every required counterfactual fault make its owning claim fail? | `just semantic-oracle-mutants-check gate-b` |
| Does incremental state equal a fresh independent rebuild after every required checkpoint? | `just rebuild-equivalence-check` |
| Do canonical, UDS, artifact, stream, and FastMCP STDIO outputs have the same independently expected meaning? | `just gate-b-delivery-equivalence-check` |
| Is comparison normalization owned only by the governed registry? | `just gate-b-projection-registry-check` |
| Is the decoded human dossier complete and claim-oriented? | `just gate-b-review-bundle-check` |
| Is the rejected v3 candidate preserved and impossible to release? | `just gate-b-rejected-candidate-zero-state-check` |
| Does the owner decision refer to the exact reviewed functional contract and evidence bundle? | `just gate-b-owner-decision-check` |
| Does the accepted predecessor satisfy all behavior and integrity obligations? | `just gate-b-check` |

Each Waves 8–12 integration recipe must depend on
`just semantic-functional-golden-check waveN` and
`just semantic-oracle-mutants-check waveN`. The final aggregate is
`just semantic-functional-golden-check all` plus the existing required integration, rebuild,
provider, schema, query, serving, and governance gates.

### 6.2 Named core tests

At minimum the recipes exercise these independently useful tests:

- `functional_golden_claim_schema_conformance`;
- `functional_golden_expectation_write_isolation`;
- `reference_query_evaluator_laws`;
- `functional_golden_rejects_unexpected_closed_universe_rows`;
- `functional_golden_negative_claim_requires_complete_universe`;
- `gate_b_independent_semantic_oracle`;
- `gate_b_public_vertical_conformance`;
- `gate_b_named_fixture_query_causality`;
- `gate_b_causal_intervention_matrix`;
- `gate_b_delivery_surface_semantic_equivalence`;
- `golden_scenario_semantic_transition_contracts`;
- `gate_b_projection_registry_closure`;
- `gate_b_human_review_bundle_contract`; and
- `gate_b_rejected_candidate_zero_state`.

### 6.3 Independence and anti-cheating checks

Structural governance scans the expectation and evaluator closure with `ast-grep` and `rg`:

- no expectation file contains an observed digest/ID field, `matches: true`, full candidate row,
  or released-output import;
- no evaluator imports or calls production provider, reconciliation, semantic-query, DataFusion,
  petgraph, lifecycle, candidate, or release modules;
- no production source imports the test-only expectation tree;
- the candidate emitter has no write-capable path into the expectation tree;
- every claim predicate and semantic axis has a registered required mutant; and
- no aggregate passes from non-empty lists, reference equality, or checksum equality alone.

The scan is necessary but not sufficient. The evaluator's own tests feed known-correct and
known-wrong observations directly, and the mutation gate must kill every registered wrong
observation or upstream fault. A surviving mutant blocks the packet even if every structural scan
is green.

### 6.4 Review and completion rule

Candidate emission is complete only when all non-owner gates pass and the decoded dossier exists.
Release remains blocked until the accountable owner makes an explicit decision. A rejection
preserves evidence and triggers a versioned replan. An acceptance may publish only the exact
reviewed candidate and may not create or modify expected semantic claims.

The design is complete when the predecessor Gate B proves independently expected functional
meaning and Waves 8–12 inherit the same authority for their full semantic profiles. Hashes,
references, execution counts, and successful transports remain necessary supporting evidence and
are never sufficient completion evidence.

accepted
