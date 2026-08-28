---
artifact: implementation-plan
plan_id: codefabric-ontology-compiled-data-fabric
version: v3
date: 2026-08-28
status: approved
design_path: docs/designs/codefabric_ontology_compiled_data_fabric_datafusion_arrow_unified_design_v5_2026-08-28.md
design_version: v5
baseline_commit: 71a888fed8aae660f97a8bc420f04a039f5aacae
working_tree_digest: 99a9fe460e37f23855ba525807a4aac679627d23308550ffcf0eb3dce12b0a74
supersedes: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md
state_path: docs/plans/state/codefabric-ontology-compiled-data-fabric_v3_state.json
cutover: true
---

# CodeFabric ontology-compiled DataFusion/Arrow unified data fabric — implementation plan v3

This plan implements design v5 as one governed transition from the partially implemented v2
candidate to a data fabric whose authored ontology compiles into Arrow-native relational
programs, whose validation and semantic planning run through sealed DataFusion 55 paths, and
whose candidate proof, activation, recovery, and result compatibility are durable and bound to
exact Delta table versions. It also releases the eight master design documents under
`docs/authoritative_design/` as one dependency-closed authority change before any target
candidate is sealed.

The plan supersedes v2; it does not certify or discard the useful v2 implementation. Current
code is migration substrate only until the packets below have proving commits and their named
checks pass again at HEAD.

## 1. Outcome and non-goals

### 1.1 Outcome

At completion:

1. The authoritative design suite consists of exactly the eight expected tracked Markdown
   masters under `docs/authoritative_design/`. All eight describe the same program, execution,
   candidate, activation, lease, and decommission contracts; generated manifests and the six
   non-normative `docs/spec_index/` views agree with their bytes. Live infrastructure uses the
   new authority root, while immutable historical plans, reviews, designs, states, and golden
   evidence retain their original path references.
2. The handwritten Schema Contract IR and registries compile reproducibly into one logical
   `OntologyProgramBundle`: normalized, schema-homogeneous Arrow IPC relation members plus a
   canonical package manifest. Bootstrap, content, logical-program, IPC-member, and package
   identities are content-addressed and acyclic. The bundle is descriptive until a governed
   runtime compiles and executes it.
3. One application-owned decoder and DataFusion program compiler lowers every current rule,
   foreign-key check, semantic-closure operation, phrase binding, and calculation through
   native DataFusion expressions and built-ins. Any unrepresentable operation fails closed;
   custom UDFs, logical nodes, or physical nodes require design reopening rather than a local
   escape hatch.
4. Every executable plan enters through one sealed `GovernedPlan` boundary. A complete,
   wildcard-free analyzer evaluates the concrete domain lattice and effects across every pinned
   DataFusion `Expr` and `LogicalPlan` variant, preserves the default analyzer stack, and rejects
   statements, DDL/DML, `COPY`, `ANALYZE`, raw optimizer/planner access, and direct physical
   execution.
5. A candidate is validated in one bounded, explicitly configured session. Each gate is drained
   once, produces a versioned semantic `GateResultChecksum`, and emits a separate diagnostic
   `GateExecutionArtifact`; recursive physical metrics are observed only after exhaustion and
   cannot authorize acceptance. Canonical maps are validated from actual `MapArray` entries,
   not inferred from datatype flags.
6. Stable bootstrap relations discover and execute the compiled closure program. Opaque
   receipts bind the workspace, candidate manifest, package/program/config/policy identities,
   exact Delta table URI and version tuples, schemas, semantic result checksums, and predecessor
   authority. Public trust bags, caller-supplied proof strings, and process-local acceptance are
   absent.
7. One durable admin command owns candidate activation. File-backed SQLite persists candidate,
   gate, receipt, decision, request-key, pointer-generation, and recovery state; the short
   transaction performs acceptance plus pointer CAS only after immutable Delta commits and
   artifact persistence. Retry, unknown outcome, concurrent CAS, restart, and governed forward
   rollback are deterministic and idempotent.
8. Every serving lease pins a reconstructible result-contract, query-form, function-catalog,
   program, policy, checksum-version, and exact-table authority tuple. Old and new leases can
   coexist across activation and restart; retained predecessors remain executable for rollback.
9. The target candidate is activated atomically only after all predecessor packets and the
   authoritative suite release are proved. The old semantic arrays, fixed validators,
   handwritten phrase branches, generic activation bypasses, self-authorizing probes, global
   result-version selection, and retired recipes then reach structural zero state.

### 1.2 Non-goals

- No performance baseline, benchmark, profiler run, throughput target, or latency claim. This
  plan does not include a performance recipe; resource tests are deterministic correctness and
  failure-semantics tests only.
- No DataFusion, Arrow, Parquet, object-store, delta-rs, or Rust toolchain upgrade.
- No graph traversal runtime, recursive graph query language, UDTF, table-function parallel
  entry point, public SQL/DataFrame surface, or ninth semantic request form.
- No custom DataFusion logical node, physical node, optimizer, query planner, or semantic
  interpreter.
- No new Cargo root, Rust package, top-level `tests/*.rs` integration target, Python data-plane
  authority, or native Python extension.
- No claim that DataFusion, Delta, Arrow metadata, plan text, metrics, test names, filenames, or
  digest shape is application acceptance authority.
- No global rewrite of immutable historical artifacts and no hand-edit of generated model
  projections.

### 1.3 Baseline and current trust posture

The baseline is review HEAD `71a888fed8aae660f97a8bc420f04a039f5aacae`. The contemporaneous
tree is intentionally dirty and includes user-owned design/master-document work captured by the
frontmatter digest. Those changes are declared inputs, not v3 implementation evidence.

The independent v2 implementation review records `changes-required`: no v2 packet has a proving
commit; recursive self-description is a census rather than semantic closure; Stage-2b activation
is test-only, caller-constructible, process-local, and bypassable; compiled rule records do not
drive validation; analyzer coverage is partial; leases do not select result authority; probes
self-authorize decisions; phrase semantics remain duplicated; and retired gate debris remains
callable. Passing selected v2 tests does not certify v3.

The authority-root transition is also non-clean at planning time. The eight formerly tracked
`docs/upfront_design/*.md` paths are deleted, the corresponding
`docs/authoritative_design/*.md` files are untracked, and a stray `.DS_Store` is present. Five
masters match their prior committed bytes while FAB, QRY, and RM already carry semantic changes.
`just spec-outline` reports the missing old default root but exits zero, and
`just model-design-contract-check` fails three of five checks: one direct missing-path failure
and two failures masked by stale active-v2 declared inputs. WP18 owns this whole release; no
failure is attributed to path movement alone.

This draft does not create its future state file and does not modify `active-plan.json`. Approval
and activation remain separate confirm-gated transactions. When activated, state must classify
all eight master documents as `planned_design_input_evolution` owned by WP18; declared-input
hashes below remain immutable planning-time evidence and are never restamped.

## 2. Source design and declared inputs

| path | sha256 |
|---|---|
| docs/designs/codefabric_ontology_compiled_data_fabric_datafusion_arrow_unified_design_v5_2026-08-28.md | f8aabdfdbce9ad07701a69877f67238a25e49006fbf620ad90056473d0100fec |
| docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md | 86c38a3b79baf8cde12e205d100efeb9370ad909415cdbbf548c400825757865 |
| docs/reviews/implementation_review_codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27_2026-08-28_v1.md | 67167a26f6aee64cca50bac104b5aefd9a1b4625868fb1c605ef3333386523fb |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |
| docs/library_ref/full_data_fabric_design_principles.md | c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 4bb8d7b4e4998b7215beb60e63580f2ad5207a0346d03efd2665be6490431984 |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | 9c7780c8e23b61ce8791f7b9fdb9d82c5e4a6df2cb67d6337ded06dc74910b3e |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | d72a302255daff31fe8e3c85e639239dac3246408fd1d1f63a9b6fd7f2d2b502 |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 6b052bf0224eb5c665630ba39e2185d5b9a916f17ce0308ec181d97278273664 |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v1.3.md | 48d64b73641b3db82f02ef4ebac8f92913de0dbdc9162ee564375df1aaf8fcf3 |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | 0bc1e7d13a138e54f10bbf4b3930d97491a80176d84ddf27568bb42edc477956 |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | a8b9f41eee4e8ca6d29cf3cab85fdedaa83aa300b5d2e373f9344a242f194b6d |
| docs/authoritative_design/codefabric_1.3_implementation_roadmap_v1.0.md | 8f3bb3fe9ffdacb57474097e3584278670c1dfd62ebd25a5ccce806377eef1b3 |

### 2.1 Planning clarifications and design-reopen conditions

These are explicit planning assumptions, not silent factual substitutions:

- **A-1 — IPC package topology.** D-10's singular logical package is implemented as one
  canonical package manifest plus an ordered, schema-homogeneous Arrow IPC member for each
  relation. Arrow 59 writers bind one schema per file/stream; an EAV or opaque payload envelope
  would violate the relational design. WP19 proves byte reproducibility and the distinction
  between logical program identity and packaging identity. If D-10 instead requires one physical
  IPC file for heterogeneous schemas, reopen D-10 before implementation.
- **A-2 — Publication-neutral compiled manifest.** Compiler output records stable logical table
  identity/address, schema, and content, but no assigned Delta version. The external sealed
  candidate manifest binds the package to exact table URI/version tuples after publication.
  WP19 and WP24 prove the acyclic boundary. If `ontology_manifest` must itself contain assigned
  Delta versions, reopen D-10 and D-13 because the compiler/publication cycle is otherwise
  unsatisfiable.
- **A-3 — Authoritative root.** The user's explicit authority root is
  `docs/authoritative_design/`. The four `docs/upfront_design` locators retained in design v5 are
  stale path locators, not a second authority decision. WP18 records the correction in all live
  authority/governance surfaces without rewriting the accepted v5 file or immutable history. If
  both roots are meant to remain live masters, revise this plan and the design; duplicate masters
  are forbidden.
- **A-4 — Predecessor compatibility.** Before target activation, the retained predecessor must be
  representable as a restart-reconstructible `ServingEpoch` with explicit program/function/policy/
  result pins. If that cannot be derived without retaining legacy semantic authority, reopen
  D-16 and the rollback design before WP26.

### 2.2 Library decisions and design bindings

- **LD-09 / Arrow 59 program substrate — adopt.** Use application-owned Arrow schemas,
  `RecordBatch` construction, deterministic IPC writer options, canonical schema metadata and
  ordering, and content addressing. Reference families: MOD-01/04/06/08, ARR-01–03,
  SCH-01/05/10, INT-01/10, TST-01/09/14.
- **LD-10 / native DataFusion program compiler — adopt.** Lower the normalized records to
  `Expr` and ordinary logical plans; use built-in relational operators and catalog providers.
  No custom logical or physical plan node. Reference families: MOD-02/03/05, EXP-01/02,
  LOG-01/04/06, GOV-04, TST-05/06.
- **LD-11 / calculation catalog — adopt built-ins, UDF only by EDR.** Generate one bijective
  catalog of program operations to supported DataFusion built-ins. An uncovered current
  operation is a design-reopen trigger, not permission to add a UDF.
- **LD-12 / analyzer and plan-policy hooks — adopt behind a sealed adapter.** Append the
  application rule to DataFusion's default analysis/coercion stack, analyze resolved plans, and
  prevent raw optimizer, planner, and physical execution access outside the adapter. Reference
  families: MOD-07, SCH-05/06/09, LOG-04/05/07, GOV-03/04, RUN-03, TST-01/06/12.
- **LD-13 / exact Delta versions — retain and extend.** Reopen with `with_version`, verify the
  loaded version, preserve `DeltaTableProvider` schema adaptation/statistics, use application
  transaction IDs and `max_retries(0)`, and reconcile lost responses. Never replace the provider
  with raw Parquet. Delta reference families: MOD-04/06/07, STA-03/10, SCH-08–11,
  TXN-01–06, QRY-01–05/10, OBS-03/08–10, INT-05/10, TST-02/03/05/08/12/14.
- **LD-14 / SQLite activation kernel — adopt.** SQLite owns durable candidate, decision,
  idempotence, pointer, and recovery state over immutable Delta versions; it does not pretend to
  participate in a cross-store transaction.
- **LD-15 / plan serialization — diagnostic/cache only.** Plan strings or serialized plans do
  not define semantic identity, proof, acceptance, or replay authority.
- **LD-16 / Arrow row encoding for gate checksums — adopt as a new integrity domain.** Reuse the
  validated canonical row encoder mechanics while preserving V1/V2 query-result replay. A
  `RowConverter` KAT change, admitted-type expansion, or map-canonicalization incompatibility
  creates a new checksum version; V1/V2 fixtures are never refreshed.

## 3. Global target invariants

The plan inherits v5 hard constraints and TI-10 through TI-19. The following execution
interpretation is binding:

- **TI-10 — Arrow-native compiled authority.** One normalized bundle and one acyclic identity
  chain; source registries remain authored authority, generated output never becomes a second
  handwritten authority.
- **TI-11 — DataFusion causal execution.** Program rows causally determine plans and gate
  results. Mutating every supported operand, binding, or calculation changes the intended
  plan/result/typed diagnostic.
- **TI-12 — Fail-closed semantic planning.** Every authorized ingress is sealed and analyzed;
  every pinned variant has an explicit domain-state/effect case; unknown operations reject.
- **TI-13 — Semantic self-description.** Bootstrap discovers and executes the closure program;
  a table census, row count, filename, or nonempty digest is insufficient.
- **TI-14 — Candidate-bound proof.** Receipts are opaque, canonical, persisted, and bound to the
  complete candidate, package, exact tables, config, policy, and semantic outputs.
- **TI-15 — One activation command.** One daemon admin route reaches one durable kernel. Query,
  generic runtime, FastMCP, test-only, and direct-pointer paths cannot activate.
- **TI-16 — Durable idempotence and recovery.** Identical retries are no-ops, same-key/different-
  bytes requests collide, unknown outcomes reconcile, and concurrent CAS has exactly one winner.
- **TI-17 — Lease-scoped compatibility.** Every lease selects all semantic and result authority;
  process-global checksum or schema selection is forbidden.
- **TI-18 — Bounded shared execution.** Candidate and serving sessions use explicit resource,
  cancellation, spill, batch, and deadline contracts. Resource failure changes no durable
  acceptance state.
- **TI-19 — Accountable decisions.** Observations, decisions, candidate evidence, and activation
  records are distinct identities. Observation never creates a decision.

Plan-wide rules:

1. Every packet begins with its preflight queries against the then-current tree. Known-touch
   lists below are planning-session evidence, not frozen file manifests; newly discovered
   consumers become state obligations and may force a replan.
2. A packet is complete only when its four named oracles and listed gates pass at its proving
   commit and again at HEAD. Dirty worktree behavior is progress evidence only.
3. Generated artifacts are changed only through the model compiler's sole writer and proved by
   isolated reproduction. Handwritten authority lands before generated projections.
4. No target pointer advances before WP26. Comparison sessions remain fixture-only and
   non-authoritative; temporary dual execution is removed in the cutover proving commit.
5. Master-design evolution lands before candidate sealing because TI-14 binds authored-authority
   bytes. Historical v2 plan/state/review artifacts remain immutable superseded evidence.

## 4. Work packets

### WP18 — Authoritative design suite alignment and authority-root cutover

**Outcome.** The eight master documents under `docs/authoritative_design/` become one tracked,
cross-consistent normative release. All live infrastructure, model ownership, navigation, and
governance use that sole root; derived indexes and generated projections agree. This packet
lands before bundle generation or candidate sealing.

**Dependencies.** None.

**Target invariants.** TI-10, TI-14, TI-19; A-3; v5 §5.2 and §6.6, with the sequencing correction
that authority bytes precede candidate identity.

**Design and library references.** V5 §§1–3, §5, §6; holistic principles P2/P3/P9/P10/P11/P20;
all eight master documents; generated-output doctrine in the model-compiler contracts.

**Change surface / Preflight Query.** Before editing, run focused, read-only discovery:

```bash
git status --short -- docs/upfront_design docs/authoritative_design
git ls-files 'docs/upfront_design/*.md' 'docs/authoritative_design/*.md'
rg -l --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'docs/upfront_design|docs/authoritative_design'
just spec-outline
just model-design-contract-check
```

Classify live consumers separately from immutable historical plans, reviews, designs, states,
and goldens; a global replacement is prohibited.

**Known Touch.** The eight master documents; `AGENTS.md`, `README.md`, `_typos.toml`, `justfile`,
`scripts/spec-outline.sh`, `scripts/bootstrap.sh`, model-plan/model-zero-state scripts,
`tooling/ci/model_design_contracts.py` and tests, `tooling/model/validate_aggregate.py`,
`rules/model-no-direct-authority-write.yml` and fixtures/snapshots, model-compiler ownership,
desired-tree, aggregate, registry, and transaction drivers, suite/design-principle manifests,
detector/baseline registries, adapter artifact index, and all six `docs/spec_index/*.md` files.

**Required changes.**

1. Amend all eight masters synchronously:
   - SUITE owns precedence, artifact census, bundle fingerprints, build proof versus runtime
     receipts, accountable decisions, release gates, and cross-document consistency.
   - ONT preserves fact meaning, normalized memberships/types/ID recipes, and the prohibition on
     evaluative facts; the compiled program is a projection, not a second ontology truth.
   - GEN keeps provider adapters as application-DTO fact producers and records consumption of a
     pinned generated semantic artifact without granting providers Arrow/DataFusion authority.
   - FAB specifies bootstrap/program/package identities, compiler/analyzer, gate checksums and
     artifacts, exact Delta bindings, durable SQLite state, result epochs, resource contracts,
     and failure semantics.
   - QRY preserves exactly eight sealed semantic forms, total phrase/calculation bindings,
     fail-closed errors, lease-pinned result authority, and no parallel table-function language.
   - LIFE specifies durable candidate states, CAS/idempotence/recovery, leases, rollback,
     retention, and vacuum protection.
   - SRV separates admin and query authorization, keeps FastMCP presentation-only, and defines
     lease-version compatibility errors without exposing activation.
   - RM supersedes the old Stage-2b sequence with WP18–WP27/M05–M08 direction and contains no
     performance baseline obligation.
2. Make `docs/authoritative_design/` the only live master root. Track exactly the eight expected
   Markdown basenames; reject `.DS_Store`, extra masters, missing masters, missing required
   anchors, and mixed root authority. Preserve old path strings only inside explicitly reviewed
   immutable historical evidence.
3. Make `spec-outline` default to the new root and exit nonzero when the requested authoritative
   root is missing, empty, or yields no masters.
4. Refresh the six non-normative spec indexes and their explicit non-authority notices.
5. Update live infrastructure/agent guidance and handwritten model authority, then regenerate
   manifests, indexes, Rust/Python projections, and adapter resources only through `model-sync`.
6. Add `just authoritative-design-conformance-check`; it owns exact census, path authority,
   anchors, cross-document consistency, spec-outline failure behavior, generated digest parity,
   and historical-exclusion review.

**Legacy Disposition and Decommission.** DB12 begins here: obsolete live
`docs/upfront_design` consumers are replaced; old tracked master paths are removed as the same
release transaction; historical citations are preserved. No duplicate master copy or alias
directory survives.

**Acceptance Checks.**

- **Behavioral:** all eight masters resolve through navigation and the generated suite manifest.
- **Structural:** exactly eight expected tracked Markdown masters and no stray authoritative file.
- **Negative:** missing/empty root, ninth master, mixed path, and stale generated digest fail.
- **Operational:** `model-design-contract-check` and isolated model reproduction pass after the
  authority and active-plan inputs are coherently reconciled.

Oracle catalog:

- Executable oracle: `ontology_authoritative_design_conformance`
- Executable oracle: `ontology_authoritative_design_path_authority`
- Executable oracle: `ontology_authoritative_design_legacy_reference_policy`
- Executable oracle: `ontology_plan_v3_readiness_graph`

**Edit-Local Gates.** `just authoritative-design-conformance-check`; `just spec-outline
docs/authoritative_design`; focused Typos/format validation for edited Markdown and scripts.

**Packet-Local Gates.** `just model-design-contract-check`; `just model-repro-check`; `just
governance-scan`; `just artifacts-check` once v3 is activated.

**Integration Milestone.** M05, together with WP19.

**Replan Triggers.** The two authority roots are intentionally co-authoritative; existing AC-G
ownership cannot express the program/receipt/decision contracts without new owner decisions;
the eight documents cannot be released atomically; generated ownership requires editing
immutable history; or source-design bytes are excluded from candidate identity contrary to
TI-14.

**Rollback or Recovery.** Before activation, revert only WP18's authority release and regenerated
projections as one commit. After a candidate is sealed, never roll master bytes backward under
the same candidate identity; create a new design release and candidate.

**Design-Bearing Contracts and Exemplars.** Eight master documents, suite manifest, model
authority registry, spec index, authoritative-suite census, and missing-root negative fixture.

### WP19 — Authored machine contracts and reproducible Arrow program package

**Outcome.** Handwritten Schema Contract IR and registries express every current relation,
operation, operand, phrase, calculation, policy, and bootstrap edge; the model compiler emits
one reproducible logical package under A-1/A-2. Existing runtime remains the only production
authority.

**Dependencies.** WP18.

**Target invariants.** TI-10, TI-11, TI-13, TI-19; A-1 and A-2; v5 D-10, D-13, LD-09.

**Design and library references.** V5 §§3.2, 3.5, 3.12, Stage 1; Arrow reference MOD/ARR/SCH/INT/
TST families named in §2.2.

**Change surface / Preflight Query.**

```bash
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'CompiledRuleOperationKind|RuntimeCompiledOntology|ontology_manifest|phrase.registry|calculation|schema-contract-ir'
ast-grep outline src/bin/codefabric_model src/compiled_ontology.rs src/ontology_plane.rs src/schema_registry.rs
just model-repro-check
```

**Known Touch.** `contracts/schema/schema-contract-ir.json`, identity/ontology/phrase/query/
calculation/policy registries, model compiler drivers, generated Rust/Python/schema artifacts,
`src/compiled_ontology.rs`, `src/ontology_plane.rs`, `src/schema_registry.rs`, packaging tests,
and command-contract recipes.

**Required changes.**

1. Extend authored contracts so every executable semantic operation has a normalized operation
   kind, ordered operands, typed relation/column references, phrase/calculation binding, expected
   result contract, and owning policy identity. Reject bare semantic codes and operand-free
   executable records.
2. Generate normalized, schema-homogeneous relation batches and one canonical package manifest.
   Fix relation/member order, row order, schema/field metadata order, dictionary policy, batch
   boundaries, writer options, and version tags.
3. Implement acyclic identities: bootstrap schema identity, authored content identity, logical
   program identity, per-member IPC identity, and package identity. Packaging-profile movement
   changes package identity without changing logical program identity.
4. Keep compiled `ontology_manifest` publication-neutral per A-2. Define the external candidate
   manifest binding seam but do not publish, validate, or activate a candidate in this packet.
5. Add generated application-owned DTOs/adapters and a digest-checked package handle; no
   long-lived Arrow writer/provider type becomes authored authority.
6. Add the compiler/packaging recipes in this packet so its proving commit is independently
   rerunnable.

**Legacy Disposition and Decommission.** DB07 is prepared, not executed: old semantic arrays and
validators remain production authority until parity and cutover; generated program output must
not call them.

**Acceptance Checks.**

- **Behavioral:** all heterogeneous relation members round-trip into their exact schemas.
- **Structural:** every executable registry record has a generated normalized program record.
- **Negative:** cyclic digests, unknown operations, duplicate keys, unstable metadata, and an
  attempt to embed assigned Delta versions fail.
- **Reproducibility:** isolated builds produce byte-identical members and manifest.

Oracle catalog:

- Executable oracle: `ontology_program_bundle_semantic_parity`
- Executable oracle: `ontology_program_bundle_digest_acyclicity`
- Executable oracle: `ontology_program_bundle_ipc_reproducibility`
- Executable oracle: `ontology_program_bundle_model_rebuild`

**Edit-Local Gates.** Focused model compiler tests; `just ontology-program-compiler-check`;
`just ontology-program-packaging-check`.

**Packet-Local Gates.** The two new recipes; `just model-repro-check`; `just
authoritative-design-conformance-check`; relevant narrow feature checks.

**Integration Milestone.** M05.

**Replan Triggers.** A-1 or A-2 is false; Arrow cannot represent a required relation without an
opaque envelope; isolated same-profile output is not byte-identical; a current executable
operation has no authored contract; or generated output becomes a second hand-edited authority.

**Rollback or Recovery.** Delete only non-authoritative generated package artifacts through the
model writer and revert authored contract additions together. No pointer or Delta state exists.

**Design-Bearing Contracts and Exemplars.** Schema Contract IR, operation/calculation/phrase
registries, package manifest schema, digest KATs, heterogeneous-package fixture, additive-domain
fixture.

### WP20 — Generic DataFusion program compiler and causal semantic profile

**Outcome.** One decoder/lowerer and calculation catalog compile all current rule, FK, closure,
phrase, and calculation records to ordinary DataFusion plans. A bounded candidate session can run
in comparison fixtures, but it has no production activation or receipt authority.

**Dependencies.** WP19.

**Target invariants.** TI-11, TI-12, TI-18, TI-19; v5 D-11, LD-10, LD-11.

**Design and library references.** V5 §§3.3, 5.2 Stage 2, 6.2; DataFusion EXP/LOG/GOV/RUN/TST
families; no custom-node/UDF exception.

**Change surface / Preflight Query.**

```bash
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'validate_compiled_ontology_rules|phrase_binding|lit\(|col\(|create_udf|register_udf|LogicalPlan::|ExecutionPlan'
ast-grep outline src/ontology_rules.rs src/semantic_query.rs src/fabric
just semantic-query-conformance-check
```

**Known Touch.** `src/ontology_rules.rs`, compiled-program DTOs, semantic-query binder,
calculation catalog, catalog/session builders, validation orchestration, independent fixtures,
and generated registry projections.

**Required changes.**

1. Decode only digest-checked packages. Validate graph edges, relation/column references,
   operation arity/types, output contract, phrase bindings, and calculation identities before
   lowering.
2. Generate a bijective calculation catalog. Lower every current operation through DataFusion
   built-ins and typed expressions; return a typed unsupported-program error for every unknown
   operation/variant. Zero custom UDFs are expected for the current profile.
3. Make program rows causal: mutate every operator, operand, relation, field, phrase, calculation,
   and expected output independently and prove the intended plan/result/diagnostic changes.
4. Compile phrase bindings through the same program and catalog. Unknown or unbound phrases fail
   closed; empty/false fallbacks and handwritten literal branches are not accepted.
5. Compare optimized and unoptimized results, schemas, extension metadata, and semantic checksum
   under independent fixtures. Plan text is diagnostic only.
6. Keep this execution path fixture-only and non-authoritative until WP22 seals ingress and WP23
   issues trustworthy receipts.

**Legacy Disposition and Decommission.** DB07 remains live for production; DB11 tracks the
temporary differential/comparison runner and requires its deletion in WP26.

**Acceptance Checks.**

- **Behavioral:** every current semantic record lowers and executes with expected rows/schema.
- **Causal:** every operand/binding mutant affects only the governed outcome.
- **Negative:** unknown operation, phrase, calculation, type, and broken graph reject.
- **Structural:** no current-profile custom UDF or custom logical/physical node.

Oracle catalog:

- Executable oracle: `ontology_compiled_program_native_profile`
- Executable oracle: `ontology_compiled_program_causality_matrix`
- Executable oracle: `ontology_phrase_binding_fail_closed`
- Executable oracle: `ontology_calculation_catalog_bijection`

**Edit-Local Gates.** Focused lowerer/catalog tests; `just ontology-calculation-catalog-check`;
`just ontology-program-causality-check`.

**Packet-Local Gates.** Both new recipes; `just semantic-query-conformance-check`; `just
query-form-contract-check`; relevant governance rules.

**Integration Milestone.** M06 begins; no milestone closes here.

**Replan Triggers.** A current semantic operation cannot use a native built-in; DataFusion
irreversibly erases required extension metadata; a UDF/custom node appears necessary; plan text
is needed for semantic identity; or comparison code leaks into a production session.

**Rollback or Recovery.** Remove the non-authoritative lowerer and comparison registration while
retaining WP19's package contract.

**Design-Bearing Contracts and Exemplars.** Calculation catalog, operation-to-expression
mapping, phrase-binding fixtures, causal mutant corpus, typed unsupported-operation errors.

### WP21 — Once-executed gate checksum, execution artifact, and resource envelope

**Outcome.** One terminal gate action drains results once, computes semantic gate identity, then
collects diagnostic metrics from the exhausted physical plan. Resource failures are typed,
deterministic, and leave durable authority unchanged.

**Dependencies.** WP20.

**Target invariants.** TI-11, TI-14, TI-18, TI-19; v5 D-14, D-17, D-18, LD-15, LD-16.

**Design and library references.** V5 §§3.6, 3.9–3.12, 6.4; Arrow ARR-03/08, SCH-10,
RUN-04/05/09/10, OBS-01/03/06/09/11/12, TST-01/06/07/10/11/12/14.

**Change surface / Preflight Query.**

```bash
ast-grep outline src/fabric/result_checksum.rs src/fabric/serving.rs src/fabric/physical_metrics.rs src/fabric/physical_metric_map.rs
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'RESULT_CHECKSUM_VERSION|AnalyzeExec|LogicalPlan::Analyze|EXPLAIN ANALYZE|metrics\(|RowConverter|MapArray'
```

**Known Touch.** `src/fabric/result_checksum.rs`, serving terminal-action order, physical metric
collection/mapping, candidate session limits, error/fault/resource registries, receipt/artifact
DTOs, and KAT fixtures.

**Required changes.**

1. Define a versioned `GateResultChecksum` integrity domain distinct from query-result digests.
   Preserve V1/V2 query replay and their immutable fixtures.
2. Recursively validate admitted Arrow arrays before encoding. For `MapArray`, inspect actual
   entries for canonical key order, uniqueness, non-null keys, and recursively canonical values;
   datatype `keys_sorted` metadata alone is insufficient.
3. Define “once” as one terminal query action. Drain all partitions/batches once while collecting
   bounded violation projections; compute semantic checksum before metrics; never re-execute for
   observation.
4. Traverse metrics recursively, including scalar-subquery plans, only after exhaustion. Missing,
   renamed, or reordered metrics are diagnostic absence/change, never gate failure or receipt
   identity.
5. Persist/read back a separate `GateExecutionArtifact` with plan/config/package/session,
   resource-use, metric, and diagnostic identity. It cannot substitute for a result checksum.
6. Enforce memory, row/byte, batch, time, cancellation, spill, and artifact limits. Boundary and
   oversize failures perform no durable candidate-state mutation.
7. Structurally prohibit `LogicalPlan::Analyze`, `AnalyzeExec`, and `EXPLAIN ANALYZE` from governed
   gate execution.

**Legacy Disposition and Decommission.** DB10 is prepared: global production checksum selection
will be removed only after lease-scoped dispatch exists. Existing encoder mechanics are
preserved where their KATs remain valid.

**Acceptance Checks.**

- **Behavioral:** batch/partition/arrival permutations yield one semantic checksum.
- **Negative:** duplicate/unsorted maps, oversize results, cancellation, and second execution fail.
- **Identity:** schema/rows change checksum; metrics/plan-display changes do not.
- **Operational:** artifact readback is complete and no failed gate mutates durable state.

Oracle catalog:

- Executable oracle: `ontology_gate_checksum_canonical_kats`
- Executable oracle: `ontology_gate_single_execution_metric_closure`
- Executable oracle: `ontology_gate_artifact_identity_separation`
- Executable oracle: `ontology_candidate_resource_failure_no_mutation`

**Edit-Local Gates.** Focused checksum/metrics/resource tests; `just
ontology-gate-result-checksum-check`; `just ontology-gate-execution-artifact-check`.

**Packet-Local Gates.** Both new recipes; `just ontology-runtime-resource-check`; retained V1/V2
KAT selector; relevant governance rules.

**Integration Milestone.** M06 continues.

**Replan Triggers.** Canonical maps cannot be validated within the resource contract; an admitted
datatype or RowConverter KAT changes; evidence needs a second execution; metrics enter acceptance
identity; or bounded collection cannot drain all partitions deterministically.

**Rollback or Recovery.** Remove new gate versions/artifacts before they are referenced by any
sealed candidate. Never alter or refresh V1/V2 fixtures.

**Design-Bearing Contracts and Exemplars.** Gate checksum schema/version registry, map negative
KATs, execution artifact schema, metric policy, resource limit registry, fault matrix.

### WP22 — Sealed plan ingress and exhaustive ID-domain enforcement

**Outcome.** All candidate and serving plans cross one non-forgeable `GovernedPlan` adapter. One
complete domain-state/effect analyzer runs after built-in resolution/coercion and rejects every
unmodeled expression, plan, statement, or execution bypass.

**Dependencies.** WP21.

**Target invariants.** TI-12, TI-18; v5 D-12, LD-12; v2 review IR-005.

**Design and library references.** V5 §§3.4, 6.3, 6.6; DataFusion MOD-07, SCH-05/06/09,
LOG-04/05/07, GOV-03/04, RUN-03, TST-01/06/12.

**Change surface / Preflight Query.**

```bash
ast-grep outline src/domain_conformance.rs src/schema_registry.rs src/semantic_query.rs src/fabric
ast-grep run -l rust -p 'SessionContext::new($$$A)' src tests --inspect summary
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'execute_logical_plan|create_physical_plan|query_planner|optimizer|PREPARE|LogicalPlan::Analyze|Expr::'
```

**Known Touch.** `src/domain_conformance.rs`, session builders, extension-type factories,
semantic-query binder, candidate session, serving path, rule/test fixtures, generated domain
registry, and structural governance rules.

**Required changes.**

1. Replace partial `DomainConformanceRule` logic with generated concrete `DomainState` and
   `DomainEffect` types and a total transition lattice for scalar, aggregate, window, subquery,
   join, set, cast, function, alias, grouping, wildcard-expanded, and nested expressions.
2. Match every pinned `Expr` and `LogicalPlan` variant explicitly with no accepting wildcard.
   Generate a compile-time variant census so dependency drift fails visibly.
3. Append the application analyzer to the default DataFusion analyzer set; do not replace
   resolution, coercion, or built-in semantic rules. Prove idempotence.
4. Seal session construction, optimizer, planner, physical execution, and direct DataFrame/SQL
   access behind an application-owned adapter that yields `GovernedPlan` only after policy and
   domain checks.
5. Reject `PREPARE`, statements, DDL, DML, `COPY`, `ANALYZE`, raw optimizer/planner calls, direct
   physical execution, and any public SQL/table-function bypass.
6. Validate exact output-field extension identity and metadata after casts, built-ins, joins,
   IPC/Parquet round trips, Delta reopen, result encoding, and delivery.
7. Add tested structural rules with positive/negative fixtures and snapshots for analyzer
   wildcard acceptance, default sessions, raw planner/executor access, Analyze paths, and missing
   extension-factory enforcement.

**Legacy Disposition and Decommission.** DB08 starts: partial analyzer and direct execution paths
remain only behind explicit migration fixtures until WP26; no new production caller may use them.

**Acceptance Checks.**

- **Behavioral:** all supported same-domain plans preserve exact extension metadata.
- **Structural:** exhaustive variant census and no raw execution surface outside the adapter.
- **Negative:** every cross-domain composite and statement/planner bypass rejects.
- **Boundary:** IPC, Parquet, Delta, functions, joins, and delivery retain/revalidate identity.

Oracle catalog:

- Executable oracle: `ontology_domain_state_effect_truth_table`
- Executable oracle: `ontology_analyzer_pinned_variant_census`
- Executable oracle: `ontology_analyzer_bypass_matrix`
- Executable oracle: `ontology_arrow_extension_boundary_matrix`

**Edit-Local Gates.** Focused analyzer/session tests; `just id-domain-plan-enforcement-check`;
rule fixture tests.

**Packet-Local Gates.** The new recipe; `just id-domain-extension-check`; `just
semantic-query-conformance-check`; `just governance-scan`.

**Integration Milestone.** M06 continues.

**Replan Triggers.** The sealed adapter cannot control raw optimizer/planner/physical execution;
analyzer ordering cannot see resolved metadata; a pinned variant lacks a truthful state/effect;
or DataFusion/Arrow enum/API pins move.

**Rollback or Recovery.** Keep the new exhaustive analyzer disabled from production serving,
remove sealed-session registration, and retain old production sessions until the packet can be
replanned. Do not weaken wildcard handling.

**Design-Bearing Contracts and Exemplars.** Domain lattice registry, variant census, ingress
capability matrix, extension boundary fixture, ast-grep rules and snapshots.

### WP23 — Honest bootstrap, compiled semantic closure, and candidate receipts

**Outcome.** The stable bootstrap discovers and executes the complete closure program against a
frozen exact-version catalog in the sealed session. Successful gates produce opaque,
candidate-bound receipts; shallow table-census success is impossible.

**Dependencies.** WP22.

**Target invariants.** TI-11, TI-13, TI-14, TI-18; v5 D-13, D-14; v2 review IR-001, IR-003,
IR-004, IR-008.

**Design and library references.** V5 §§3.5–3.6, 6.2, 6.5; LD-09/10/12/13/16; Delta exact-version
query/provider patterns.

**Change surface / Preflight Query.**

```bash
ast-grep outline src/ontology_activation.rs src/ontology_rules.rs src/fabric/snapshot_catalog.rs src/fabric/publication.rs
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'OntologyCatalogResolution|OntologyCandidateDossier|REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS|table_contract|closure|receipt'
```

**Known Touch.** Bootstrap/program relations, exact-version catalog/provider, closure compiler,
gate orchestration, receipt DTOs, ontology activation compatibility shim, registry authorities,
and corruption/additive-domain fixtures.

**Required changes.**

1. Build a frozen catalog only from candidate-manifest URI/version tuples; reopen each Delta
   table with `with_version`, assert the loaded version, preserve provider schema adaptation and
   statistics, and forbid refresh-to-latest or raw-Parquet fallback.
2. Execute bootstrap to discover program members and then run compiled semantic closure across
   every authority family: codes, edges/memberships, semantic types, table/column/result
   contracts, identity recipes, phrase/calculation/rule bindings, snapshot/publication/plan,
   package, policy, and exact table identities.
3. Generate one opaque receipt per gate from canonical candidate identity, program/package,
   session/config/policy, exact Delta table set, semantic checksum, expected result contract, and
   artifact identity. Constructors remain private to the trusted gate runner.
4. Make self-description additive: a correctly modeled new domain/relation is discovered with no
   resolver code change. Corrupt each authority family independently and require rejection.
5. Prove one-to-one bindings among program operation, terminal execution, semantic checksum,
   diagnostic artifact, and receipt. Hard-coded proving artifact IDs and caller-supplied digest
   strings are forbidden.
6. Add `ontology-candidate-delta-binding-check` and the closure/receipt recipes before completion.

**Legacy Disposition and Decommission.** DB09 begins: public dossier fields and process-local
acceptance remain non-authoritative shims only until durable storage lands in WP24; hard-coded
proof ID authority is marked for deletion.

**Acceptance Checks.**

- **Behavioral:** bootstrap discovers and executes every closure family.
- **Additive:** a new domain/relation succeeds without resolver branching.
- **Negative:** every broken family, stale table, unbound result, and forged receipt rejects.
- **Causal:** every program execution maps to exactly one checksum/artifact/receipt tuple.

Oracle catalog:

- Executable oracle: `ontology_bootstrap_program_package_closure`
- Executable oracle: `ontology_semantic_closure_corruption_matrix`
- Executable oracle: `ontology_self_description_additive_relation`
- Executable oracle: `ontology_program_execution_receipt_bijection`

**Edit-Local Gates.** Focused closure/receipt/exact-provider tests; `just
ontology-self-description-check`; `just ontology-candidate-receipt-check`.

**Packet-Local Gates.** Those recipes; `just ontology-relational-closure-check`; `just
ontology-candidate-delta-binding-check`; `just ontology-plan-artifact-boundary-check`.

**Integration Milestone.** M06 closes only after WP20–WP23 gates pass together.

**Replan Triggers.** Closure requires handwritten family branching; exact-version Delta reopen
cannot preserve required schema/statistics; a table feature is unsupported; a receipt must trust
caller-provided evidence; or sealed analysis cannot precede `PROVED`.

**Rollback or Recovery.** Delete unpersisted receipts and disable the candidate runner. Published
non-active Delta versions remain immutable and unreferenced; no pointer changes.

**Design-Bearing Contracts and Exemplars.** Bootstrap schema, closure program, candidate manifest
schema, opaque receipt wire/storage shape, exact-version fixture, corruption matrix.

### WP24 — Durable candidate, decision, receipt, and activation transaction kernel

**Outcome.** File-backed SQLite persists the complete candidate state machine and opaque evidence;
acceptance plus pointer CAS is one short idempotent transaction over already committed immutable
Delta versions and durable artifacts.

**Dependencies.** WP23.

**Target invariants.** TI-14, TI-15, TI-16, TI-19; v5 D-14, D-15, D-17, LD-13, LD-14.

**Design and library references.** V5 §§3.6–3.7, 3.9, 3.11, 5.2 Stage 4, 6.5; delta-rs transaction,
application-ID, exact-snapshot, and recovery families.

**Change surface / Preflight Query.**

```bash
ast-grep outline src/operational_store.rs src/snapshot_runtime.rs src/fabric/mutation.rs src/fabric/snapshot_catalog.rs
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'SEALED|PROVED|ACTIVATED|application_transaction|max_retries|request_key|pointer_generation|recover\('
```

**Known Touch.** Operational Contract IR/generated SQL/table specs, operational store,
snapshot/candidate manifests, mutation transactions, receipt/artifact persistence, decision and
fault registries, state transition errors, and recovery fixtures.

**Required changes.**

1. Generate durable candidate, gate, receipt, artifact, owner-decision, activation-request,
   acceptance, pointer-generation, and recovery tables through the model compiler. Public Rust
   structs are projections over this contract, not authority.
2. Model legal states and transitions (`BUILDING`, `SEALED`, `PROVED`, `ACCEPTED`, `ACTIVE`,
   `SUPERSEDED`/failure states as specified by the authoritative suite). Only the trusted runner
   can persist `PROVED`; only an accountable owner decision can authorize acceptance.
3. Bind the sealed candidate to workspace, manifest bytes/digest, package/program/config/policy,
   exact table URI/version/schema/content identities, predecessor epoch, rollback retention, and
   every receipt/artifact.
4. Persist artifacts before the short transaction. In that transaction validate current
   predecessor/policy, request-key semantics, opaque receipts, owner decision, and CAS generation;
   write acceptance and pointer update atomically. Never perform DataFusion or Delta work inside.
5. Use application transaction IDs and `max_retries(0)` for Delta commits. Reconcile lost commit
   responses by reopening exact state; same operation ID/same bytes is idempotent, same ID/
   different bytes is a collision.
6. Implement pre-commit/post-commit fault points, unknown-outcome reconciliation, concurrent CAS,
   and restart-safe recovery over file-backed SQLite. Metrics remain outside decision identity.

**Legacy Disposition and Decommission.** DB09 process-local `OntologyActivationState` and mutable
trust bags lose authority here but are not deleted until the admin route and migration complete.

**Acceptance Checks.**

- **Behavioral:** legal state transitions persist and reopen with all bindings intact.
- **Atomicity:** exactly one acceptance/pointer generation commits; no Delta work occurs inside.
- **Negative:** forged/stale receipt, owner mismatch, collision, policy drift, and CAS loss reject.
- **Recovery:** every fault boundary and lost response reconciles after process restart.

Oracle catalog:

- Executable oracle: `ontology_candidate_receipt_binding_matrix`
- Executable oracle: `ontology_activation_state_transaction_atomicity`
- Executable oracle: `ontology_candidate_delta_exact_version_binding`
- Executable oracle: `ontology_decision_observation_separation`

**Edit-Local Gates.** Focused operational-store/mutation/recovery tests; `just
ontology-candidate-receipt-check`; `just ontology-decision-integrity-check`.

**Packet-Local Gates.** Those recipes; `just ontology-candidate-delta-binding-check`; `just
ontology-activation-recovery-check`; `just model-repro-check`.

**Integration Milestone.** M07 begins.

**Replan Triggers.** A Delta transaction is assumed atomic with SQLite or another table; evidence
must be computed inside the short commit; receipt identity cannot survive restart; owner policy
would be supplied by the candidate itself; or the pinned delta-rs transaction/retry API differs.

**Rollback or Recovery.** Before route activation, migrate SQLite forward/back only through a
versioned reversible migration and remove non-active candidate rows/artifacts. Never delete an
active/predecessor table version or receipt.

**Design-Bearing Contracts and Exemplars.** Operational state Contract IR, generated SQL,
transition table, request-key contract, fault registry, owner-decision schema, recovery fixture.

### WP25 — Unified admin activation route, recovery, and lease-scoped result authority

**Outcome.** One daemon admin command reaches the durable kernel, restart recovery reconstructs
active/predecessor epochs, and every lease selects explicit program/function/policy/result
authority. No query or presentation path can activate.

**Dependencies.** WP24.

**Target invariants.** TI-15, TI-16, TI-17, TI-19; v5 D-15, D-16, LD-14; v2 review IR-002,
IR-003, IR-006.

**Design and library references.** V5 §§3.7–3.9, 5.2 Stage 4, 5.3, 6.5; existing daemon/admin,
snapshot runtime, serving checksum, and compatibility contracts.

**Change surface / Preflight Query.**

```bash
ast-grep outline src/daemon.rs src/bin/codefabric.rs src/snapshot.rs src/snapshot_runtime.rs src/fabric/serving.rs
ast-grep run -l rust -p '$R.activate_stage2b($$$A)' src tests --inspect summary
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'WorkspaceAdminCommand|ServingLease|RESULT_CHECKSUM_VERSION|activate_stage2b|FastMCP|ActivateCandidate'
```

**Known Touch.** Daemon admin command/parser, private UDS authorization if already present,
snapshot manifest/runtime/catalog, lease manager, serving/result dispatch, operational recovery,
error/protocol contracts, and admin/query negative fixtures.

**Required changes.**

1. Add exactly one `WorkspaceAdminCommand::ActivateCandidate` route and CLI/admin transport
   parser. It resolves durable candidate and owner records by identity; callers cannot supply
   dossiers, proofs, policies, or pointer contents.
2. Keep activation out of query UDS and FastMCP. Generic `activate` rejects ontology-capable
   candidates; `activate_stage2b` becomes a migration shim pending DB09 deletion.
3. Add `ResultAuthorityPin` and program, function-catalog, policy, query-form, checksum-version,
   and exact-table pins to versioned serving manifests/epochs/leases.
4. Define deterministic legacy-manifest decoding or a persisted replacement epoch before any old
   lease is served by the final binary. Preserve V1/V2 result encoders as versioned dispatch,
   never as a global production constant.
5. On restart, recover candidate/acceptance/pointer/epoch/lease compatibility before accepting
   retry or query work. Identical activation requests become durable no-ops.
6. Materialize a rollback-ready predecessor compatible with the new runtime and protect its
   package, Delta versions, result/function/policy contracts, and artifacts from vacuum.
7. Prove concurrent old/new leases before/after activation and restart, including typed errors
   for unavailable retained authority.

**Legacy Disposition and Decommission.** DB09 activation duplicates and DB10 global result
selection are structurally unreachable after this packet but remain until atomic cutover proves
predecessor compatibility.

**Acceptance Checks.**

- **Behavioral:** authorized admin activation reaches one durable owner route.
- **Negative:** query, FastMCP, generic runtime, direct pointer, and caller-proof bypasses reject.
- **Recovery:** retry and old/new lease behavior survives process restart.
- **Compatibility:** predecessor and target dispatch their own result/program/function/policy pins.

Oracle catalog:

- Executable oracle: `ontology_admin_activation_owner_route`
- Executable oracle: `ontology_activation_restart_idempotency`
- Executable oracle: `ontology_result_authority_lease_matrix`
- Executable oracle: `ontology_activation_concurrency_forward_rollback`

**Edit-Local Gates.** Focused daemon/runtime/lease tests; `just ontology-activation-route-check`;
`just result-authority-lease-check`.

**Packet-Local Gates.** Those recipes; `just ontology-activation-recovery-check`; `just
ontology-decision-integrity-check`; daemon/query/FastMCP negative protocol tests.

**Integration Milestone.** M07 closes.

**Replan Triggers.** A-4 fails; legacy manifests cannot map to explicit pins without invented
authority; a rollback epoch requires the retired runtime; activation must enter query/FastMCP;
or recovery cannot distinguish committed success from safe retry.

**Rollback or Recovery.** Keep the route disabled and retain existing serving pointer/leases;
versioned manifest decoding must remain backward compatible. Do not delete V1/V2 or predecessor
artifacts.

**Design-Bearing Contracts and Exemplars.** Admin command contract, versioned serving manifest,
`ResultAuthorityPin`, legacy decoder fixture, old/new lease fixture, rollback epoch schema.

### WP26 — Non-active candidate proof and atomic semantic cutover

**Outcome.** A real target candidate is built, sealed, proved, accepted, and activated only through
WP25's route. End-to-end tests cover exact Delta tables, durable SQLite, restart, concurrency,
old/new leases, and forward rollback. Temporary comparison authority is removed in the same
proving commit.

**Dependencies.** WP25.

**Target invariants.** TI-10 through TI-19; v5 Stage 5, rollback/recovery, and acceptance.

**Design and library references.** V5 §§5.2 Stage 5, 5.3, 6.5–6.7; all LD-09–LD-16; Delta exact
snapshot/transaction/recovery patterns.

**Change surface / Preflight Query.**

```bash
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'comparison|dual.execute|fixture.only|active_pointer|ServingEpoch|rollback|vacuum'
ast-grep outline tests/integration.rs tests/integration src/snapshot_runtime.rs src/fabric/publication.rs
just --list | rg 'ontology-(datafabric|activation|candidate|program|gate)'
```

**Known Touch.** Existing `tests/integration.rs`, a new module below `tests/integration/`, daemon
admin/runtime/publication/catalog/serving paths, temporary comparison runner, retention/vacuum
policy, exact candidate artifacts, and release recipes.

**Required changes.**

1. Build and publish all target Delta table versions without moving the active pointer. Install
   and retain the exact program package and artifacts at durable addresses; seal the external
   candidate manifest only after all identities are known.
2. Validate and prove through the sealed session. Any program, analyzer, gate, receipt, resource,
   policy, or exact-table failure leaves the predecessor active and candidate state truthful.
3. Obtain the accountable owner decision and activate only through
   `WorkspaceAdminCommand::ActivateCandidate`; prove exactly one concurrent CAS winner.
4. Add one real integration module under `tests/integration/`, wired through the existing sole
   top-level `tests/integration.rs`. Use temporary real Delta tables, file-backed SQLite, daemon
   admin transport, subprocess restart, lost response, CAS races, exact artifact readback,
   simultaneous old/new leases, fact publication after cutover, and governed forward rollback.
5. Prove restart-reconstructible predecessor and target epochs. Advance to the predecessor as a
   new governed forward activation for rollback; never mutate historical pointers or table data.
6. Remove the fixture-only dual/comparison runner, toggles, and registration in this packet's
   proving commit. No temporary semantic authority remains after pointer movement.

**Legacy Disposition and Decommission.** DB11 closes here. DB07–DB10 become deletion-ready only
after the integration oracle proves the successor and rollback epoch.

**Acceptance Checks.**

- **Integration:** the full real-storage/admin/restart path activates exactly one target.
- **Atomicity:** every predecessor failure and injected fault leaves the old pointer active.
- **Compatibility:** old/new leases and rollback survive restart with exact authority.
- **Continuity:** ordinary post-cutover fact publication reuses unchanged ontology/program pins.

Oracle catalog:

- Executable oracle: `ontology_datafabric_end_to_end_cutover`
- Executable oracle: `ontology_datafabric_predecessor_failure_atomicity`
- Executable oracle: `ontology_datafabric_old_new_lease_restart`
- Executable oracle: `ontology_datafabric_post_cutover_fact_publication`

**Edit-Local Gates.** Focused integration selectors; `just ontology-datafabric-integration-check`;
`just ontology-activation-recovery-check`.

**Packet-Local Gates.** All M08 semantic/activation recipes; `just query-determinism-check`;
`just publication-referential-integrity-check`; `just root-test`.

**Integration Milestone.** M08 begins; cutover is not certified until WP27.

**Replan Triggers.** More than one irreversible pointer move is required; rollback cannot use the
new runtime; final proof needs MemTable/in-memory SQLite instead of real state; a second top-level
test target is required without a distinct harness; or package/Delta/artifact retention cannot
protect active and predecessor leases.

**Rollback or Recovery.** Before CAS, leave the candidate non-active. After CAS, use the governed
forward activation of the retained predecessor. Never edit SQLite/Delta pointers manually.

**Design-Bearing Contracts and Exemplars.** End-to-end integration fixture, candidate package,
exact-table manifest, owner decision, activation record, old/new lease traces, rollback record.

### WP27 — Legacy zero state, structural governance, and final certification

**Outcome.** All duplicate semantic/execution/activation/result authorities and retired command
surfaces are absent; governance encodes every v5 bypass prohibition; the committed HEAD passes
the full gate matrix and is ready for an independent implementation review.

**Dependencies.** WP26.

**Target invariants.** TI-10 through TI-19; v5 Stage 6 and §6.6–§8.

**Design and library references.** V5 §5.4 legacy matrix, §6 proof strategy, §8 acceptance;
repository governance and evidence doctrine.

**Change surface / Preflight Query.**

```bash
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' 'CompiledRuleOperationKind|RuntimeCompiledOntology|validate_compiled_ontology_rules|activate_stage2b|OntologyCandidateDossier|OntologyActivationState|RESULT_CHECKSUM_VERSION|id16-extension-contract-check|ontology_fabric_probe_suite|probe-suite'
ast-grep run -l rust -p '$R.activate_stage2b($$$A)' src tests --inspect summary
ast-grep run -l rust -p 'SessionContext::new($$$A)' src tests --inspect summary
just --list
just gate-filter-census
```

The zero-state gate declares live roots and separately inspects documented exclusions. It does
not count immutable historical plans/reviews/designs/states or accepted golden evidence as live
runtime authority, but it reports every exclusion and every skipped candidate.

**Known Touch.** `src/compiled_ontology.rs`, `src/ontology_rules.rs`, `src/ontology_activation.rs`,
`src/snapshot_runtime.rs`, `src/fabric/result_checksum.rs`, semantic-query phrase branches,
`scripts/ontology_fabric_probe_suite.py`, `justfile`, gate-filter census, CI, structural rules,
fixtures/snapshots, and live old-authority path consumers.

**Required changes.**

1. Execute DB07–DB10 and DB12. Remove, do not merely stop calling, superseded Rust types,
   constants, functions, scripts, recipes, registrations, filters, fixtures, and imports unless a
   specifically versioned compatibility decoder/encoder remains required by a retained lease.
2. Delete `id16-extension-contract-check` and its gate-filter census entry while retaining
   `id-domain-extension-check`. Delete `probe-suite`, `ontology_fabric_probe_suite.py`, all
   callers/tests/CI entries, and any preselected self-authorizing decision record.
3. Add tested ast-grep rules with positive/negative fixtures and snapshots for sealed session
   construction, raw optimizer/planner/physical execution, Analyze paths, wildcard analyzer
   acceptance, unbound phrase fallback, bare semantic codes, operation-specific validator
   dispatch, public trust-bearing receipt fields, process-local activation, global result version
   selection, and activation from query/FastMCP.
4. Replace/extend the existing old-authority checker with a hidden-aware declared-envelope census,
   ast-grep structural scans, candidate/skipped-file inspection, compiler proof, `just --list`,
   gate-filter census, and historical-exclusion policy. Do not rely on version-name allowances.
5. Prove every new recipe is registered, every retired recipe is absent, generated projections
   reproduce, exact pins/features remain stable, the active plan/state is healthy, and all Tier A
   repository gates pass at committed HEAD.
6. Request a separate read-only `implementation-review` after completion; executor/state claims
   are not acceptance.

**Legacy Disposition and Decommission.** DB07, DB08, DB09, DB10, and DB12 close. DB11 was closed
by WP26. Preserved compatibility is restricted to versioned result/manifest decoders and retained
epoch artifacts that an active lease or rollback record can name.

**Acceptance Checks.**

- **Behavioral:** successor runtime serves and activates without predecessor semantic code.
- **Structural:** complete live-envelope zero state with reviewed historical exclusions.
- **Negative:** retired commands, generic activation, planner bypass, and self-authorizing probes
  are unavailable.
- **Certification:** the full final matrix passes at the proving commit and at HEAD.

Oracle catalog:

- Executable oracle: `ontology_datafabric_successor_authority`
- Executable oracle: `ontology_datafabric_legacy_zero_state`
- Executable oracle: `ontology_datafabric_retired_command_absence`
- Executable oracle: `ontology_datafabric_release_certification`

**Edit-Local Gates.** Focused zero-state/rule fixture tests; `just
ontology-datafabric-legacy-zero-state-check`; `just governance-scan`.

**Packet-Local Gates.** Full §7 final gate matrix at committed HEAD.

**Integration Milestone.** M08 closes.

**Replan Triggers.** Any removed authority still has a live production consumer; a compatibility
decoder is needed by retained state but was classified as duplicate authority; zero-state scans
cannot produce a complete candidate/skipped census; final gates expose cross-packet defects; or
the target requires source-design changes beyond WP18's accepted suite release.

**Rollback or Recovery.** Before certification, restore a required compatibility decoder only
with an explicit state deviation and rerun WP25–WP27. After certification, rollback is a new
governed candidate activation, never restoration of deleted bypasses.

**Design-Bearing Contracts and Exemplars.** Governance rule suite, zero-state candidate envelope,
historical-exclusion policy, retired-command negative fixture, release evidence generated by
commands.

## 5. Integration milestones

### M05 — Authoritative suite and reproducible Arrow program foundation

**Packets.** WP18–WP19.

**Exit condition.** The sole authoritative suite is tracked and coherent; live path consumers and
generated manifests agree; the non-authoritative program package rebuilds byte-identically; no
runtime route, receipt, or pointer treats it as production authority.

**Required gates.** `authoritative-design-conformance-check`,
`ontology-program-compiler-check`, `ontology-program-packaging-check`,
`model-design-contract-check`, `model-repro-check`.

### M06 — Complete governed semantic pipeline without activation authority

**Packets.** WP20–WP23.

**Exit condition.** Every current semantic operation is program-driven and causal; gate execution
is once-only and resource-bounded; ingress/analyzer coverage is total; exact-version semantic
closure produces opaque candidate-bound receipts. The path remains comparison-only and cannot
advance the target pointer.

**Required gates.** `ontology-program-causality-check`,
`ontology-calculation-catalog-check`, `ontology-gate-result-checksum-check`,
`ontology-gate-execution-artifact-check`, `ontology-runtime-resource-check`,
`id-domain-plan-enforcement-check`, `ontology-self-description-check`,
`ontology-relational-closure-check`, `ontology-candidate-receipt-check`,
`ontology-candidate-delta-binding-check`, `ontology-plan-artifact-boundary-check`,
`semantic-query-conformance-check`, `governance-scan`.

### M07 — Durable activation and lease-compatibility kernel

**Packets.** WP24–WP25.

**Exit condition.** Candidate/receipt/decision state survives restart; one authorized admin route
owns idempotent CAS activation; old/new leases select explicit authority; a rollback-ready
predecessor is reconstructible; target pointer has not yet moved.

**Required gates.** `ontology-candidate-receipt-check`,
`ontology-activation-route-check`, `ontology-activation-recovery-check`,
`result-authority-lease-check`, `ontology-decision-integrity-check`,
`ontology-candidate-delta-binding-check`, `model-repro-check`.

### M08 — Atomic cutover, zero state, and certification

**Packets.** WP26–WP27.

**Exit condition.** The target and rollback predecessor are proved with real Delta/SQLite/admin/
restart integration; target activates through one CAS; temporary comparison and predecessor
authorities are removed; the full matrix passes at committed HEAD.

**Required gates.** `ontology-datafabric-integration-check`,
`ontology-datafabric-legacy-zero-state-check`, every prior milestone gate, and §7.

## 6. Cross-packet decommission batches

### DB07 — Duplicate semantic and phrase authority

**Owner.** WP27, prepared by WP19–WP23.

Delete `CompiledRuleOperationKind`, operand-free executable contracts,
`RuntimeCompiledOntology` semantic arrays/constants/accessors, fixed validators, handwritten
phrase/literal branches and false/empty fallbacks, and old ontology-row reconstruction authority.
Preserve only application DTOs generated from the new program contracts.

### DB08 — Governed execution bypasses

**Owner.** WP27, prepared by WP22.

Delete partial analyzer authority, governed default sessions, direct plan/session execution, raw
optimizer/planner access, Analyze instrumentation, accepting wildcard behavior, and any public
SQL/DataFrame/table-function semantic ingress.

### DB09 — Activation and proof duplicates

**Owner.** WP27, prepared by WP23–WP25.

Delete public trust-bearing dossier fields, synthetic proof maps, process-local ontology
activation state, `activate_stage2b`, ontology-capable generic activation, hard-coded work-packet/
report proof IDs, and caller-constructible permits. One durable admin/kernel path remains.

### DB10 — Global result selection and self-authorizing decisions

**Owner.** WP27, prepared by WP21 and WP25.

Delete production `RESULT_CHECKSUM_VERSION` selection, semantic acceptance derived from plan/
metric/artifact identity, preselected probe branches, `probe-suite`,
`scripts/ontology_fabric_probe_suite.py`, its tests/callers/CI registration,
`id16-extension-contract-check`, and its gate-filter entry. Preserve V1/V2 versioned replay and
`id-domain-extension-check`.

### DB11 — Temporary comparison authority

**Owner.** WP26.

Delete the fixture-only dual-execution runner, feature toggles, and comparison registrations in
the cutover proving commit. It must never coexist with an active target as a production authority.

### DB12 — Obsolete live master-design root and Stage-2b governance wording

**Owner.** WP18 for authority/path cutover; WP27 for final zero state.

Remove live `docs/upfront_design` master ownership, aliases, defaults, generated locations, and
current governance references. Replace obsolete live Stage-2b command/authority wording with the
unified candidate model. Preserve immutable historical references with explicit exclusions.

## 7. Final gate matrix

Every recipe below must exist and pass at WP27's proving commit and again at HEAD. Recipes added by
this plan are marked **new**. Arguments for the compatibility recipe are resolved from the
accepted baseline and target candidate; no performance command is part of the matrix.

- `just authoritative-design-conformance-check` — **new**
- `just ontology-program-compiler-check` — **new**
- `just ontology-program-packaging-check` — **new**
- `just ontology-program-causality-check` — **new**
- `just ontology-calculation-catalog-check` — **new**
- `just id-domain-plan-enforcement-check` — **new**
- `just ontology-candidate-receipt-check` — **new**
- `just ontology-gate-result-checksum-check` — **new**
- `just ontology-gate-execution-artifact-check` — **new**
- `just ontology-activation-route-check` — **new**
- `just ontology-activation-recovery-check` — **new**
- `just result-authority-lease-check` — **new**
- `just ontology-runtime-resource-check` — **new**
- `just ontology-decision-integrity-check` — **new**
- `just ontology-datafabric-integration-check` — **new**
- `just ontology-datafabric-legacy-zero-state-check` — **new**
- `just ontology-candidate-delta-binding-check` — **new**
- `just ontology-plan-artifact-boundary-check` — **new**
- `just ontology-self-description-check`
- `just ontology-relational-closure-check`
- `just artifacts-check`
- `just plan-status`
- `just plan-dependency-check`
- `just model-design-contract-check`
- `just model-repro-check`
- `just governance-scan`
- `just gate-filter-census`
- `just data-fabric-stack-compat <baseline> <target>`
- `just query-determinism-check`
- `just semantic-query-conformance-check`
- `just query-legacy-zero-state-check`
- `just id-domain-extension-check`
- `just publication-referential-integrity-check`
- `just query-form-contract-check`
- `just stable-graph-check`
- `just features-each`
- `just root-test`
- `just ci-pr`

The final state records command-derived evidence only through the repository's artifact tooling;
the plan does not hand-maintain check results, changed-file lists, hashes, or traceability.

## 8. Execution sequence

```text
WP18 → WP19 → WP20 → WP21 → WP22 → WP23 → WP24 → WP25 → WP26 → WP27
  └ M05 ┘                 └──── M06 ────┘    └ M07 ┘     └── M08 ──┘
```

1. Approve v3, then activate it with the repository transaction; do not hand-create the state
   file or edit `active-plan.json`.
2. Record the eight master documents as WP18-owned `planned_design_input_evolution`. Preserve all
   planning-time declared hashes.
3. Execute one packet at a time. At most one packet is `in_progress`; every completion requires a
   proving commit and all four packet oracles at that commit and HEAD.
4. Do not seal or prove a target candidate until WP18–WP22 are complete. Do not persist `PROVED`
   until sealed analysis and exact-version closure in WP23 are complete.
5. Do not advance the target pointer until WP26. If any gate fails, retain the predecessor as
   active and record the candidate's truthful failure state.
6. Complete decommission and the full repository matrix in WP27, then commission a separate
   read-only implementation review. Only that review may independently assess completeness.

## 9. Plan risks and replan policy

### 9.1 Design reopening

Reopen design v5 rather than improvising if:

- A-1, A-2, A-3, or A-4 is false;
- a current semantic operation requires a custom UDF, logical node, physical node, or non-
  relational interpreter;
- the sealed adapter cannot prevent raw optimizer/planner/executor access or cannot analyze
  resolved extension metadata;
- canonical Arrow rows/maps cannot meet the integrity/resource contract;
- exact Delta reopen, application transaction identity, zero retry, or lost-response recovery is
  unavailable at the pinned revision;
- any requirement assumes atomicity across SQLite and Delta or across multiple Delta tables;
- trustworthy proof requires a second execution or treats metrics/plan text as semantic identity;
- rollback cannot reconstruct the predecessor without preserving retired semantic authority; or
- S3/another backend, dependency-pin movement, graph execution, public SQL, or another request
  form enters scope.

### 9.2 Plan revision

Revise this plan if packet boundaries cease to be dependency-closed, a newly discovered live
consumer crosses decommission ownership, the authoritative suite cannot precede candidate
sealing, old leases/recovery name a DB07–DB10 target after its deletion packet, temporary dual
execution leaks into production, cutover needs more than one irreversible packet, or final
integration needs a materially different harness.

### 9.3 Execution-state adaptation

Record ordinary file movement, additional callers found by preflight, test relocation within the
existing integration target, or gate implementation detail as state obligations/deviations when
they preserve outcomes, authority boundaries, packet dependencies, and the four named oracles.
Such adaptation never authorizes a new library, architecture, public surface, performance scope,
or semantic decision.

### 9.4 Stop and rollback conditions

Stop before pointer movement on any unresolved semantic, checksum, receipt, exact-version,
policy, resource, or recovery failure. Keep the candidate non-active and the predecessor serving.
After pointer movement, rollback only by the governed forward activation of the retained
predecessor epoch. Never repair evidence by restamping hashes, refreshing immutable KATs,
editing generated artifacts by hand, or mutating Delta/SQLite pointers outside the kernel.
