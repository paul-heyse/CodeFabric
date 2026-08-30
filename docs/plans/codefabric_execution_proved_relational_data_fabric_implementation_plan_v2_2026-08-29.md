---
artifact: implementation-plan
plan_id: codefabric-execution-proved-relational-data-fabric
version: v2
date: 2026-08-29
status: approved
design_path: docs/designs/codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md
design_version: v2
baseline_commit: 7184b86dc80adedc8a2b8d081179fa52d3dfee20
working_tree_digest: fe78dd510af9bc98f7c6d7c0da7148604b1e5ba40636b32b6315f5b49d197b62
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v2_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v3_2026-08-28.md
---

# CodeFabric execution-proved relational data fabric — implementation plan v2

This audit-integrated successor realizes design v2 as one governed replacement program.
It does not extend the active ontology-bundle architecture. It builds a replayed relational
model, exact provider-native Arrow relations, model-compiled DataFusion execution, immutable
proved fabric epochs, and a fenced cutover; it then removes the old authorities, generators,
payloads, routes, tooling, and selectable fallbacks.

This artifact is an immutable execution specification. It does not create its future
schema-version-2 state file and does not change `docs/plans/active-plan.json`. First, externally
governed candidate-readiness remediation must land with its own state and proving commit. Then
this exact candidate and its declared inputs receive focused re-audit and explicit approval. Only
after both may `just plan-activate` create/validate state and atomically replace the active
pointer. This successor does not implement the machinery that authorizes itself. Until that
transaction, the current active plan and runtime remain the only execution authority.

## 1. Outcome and non-goals

### 1.1 Outcome

At completion:

1. The versioned authoritative suite names the v2 relational architecture as the sole current
   target. The prior v1.3 realization is historical, not coequal authority, while released wire,
   identity, and accepted-history commitments remain explicit.
2. One minimal Rust metamodel and closed intrinsic algebra replay immutable
   `ModelMigration` events under an exact `FabricCompilerRelease` into typed Arrow model
   relations. No generated bundle, YAML registry, census, fingerprint, or current-state status
   file supplies a parallel semantic answer.
3. Tree-sitter, Ruff, the exact Pyrefly Query/TSP/module-resolver/selected Glean/LSP surfaces,
   and the exact `rustc_public` plus narrow `rustc_private` seam expose only their genuine API
   families as typed provider-native Arrow relations. Separately versioned CodeFabric analyses
   own Python CFG/flow, Rust MIR-derived, common graph, effect/resource, and interprocedural
   relations. Every family carries requested/completed coverage, remainders, diagnostics,
   provenance, and explicit unknowns; semantic payloads use relation-scoped Arrow IPC rather
   than opaque JSON or row-per-message Protobuf.
4. Each immutable `FabricEpoch` owns one exact model/source/provider/table/policy/proof set,
   sealed internal DataFusion state, an authorized child-session factory, and one resource
   runtime. Generic compilers construct schemas, normalization, authority, derivation, semantic
   query, policy, and proof plans from model relations using optimizer-visible DataFusion 55
   nodes wherever possible.
5. Proof is part of epoch construction. Relational invariants, coverage, provenance closure,
   independently authored semantic expectations, causal faults, authorization, resource
   envelopes, and activation-chain validity must all discriminate pass, fail, and unknown.
6. Every durable change enters through one idempotent `FabricCommand`. A single fenced daemon
   writer stages exact Delta versions and immutable Arrow segments, appends an activation event
   only after proof, derives the unique current head, swaps one `Arc<FabricEpoch>`, and
   reconciles unknown outcomes without guessing. SQLite owns reconstructible temporal state
   only.
7. The eight semantic request forms remain compositional and bounded. Rust compiles request
   relations inside an authorized child catalog; the FastMCP adapter remains presentation-only
   and derives live reference/capability/status content from the daemon.
8. The cutover reaches `NEW_MUTATING` only after independent comparison and a bridge/external
   authority revokes the exact frozen legacy binary across restart and reboot; it then proceeds
   forward-only. Completion requires all L-20 through L-55 dispositions and DB01 through DB08
   exit invariants: the former model compiler, generated registries, opaque provider payloads,
   old serving/activation routes, static governance, packaging residue, comparator archive after
   retention expiry, and compatibility fallbacks are unselectable and absent outside immutable
   history.

### 1.2 Non-goals

- No arbitrary SQL, table-name, function-name, DataFrame, logical-plan, physical-plan, or
  serialized-plan public surface.
- No Python Arrow/DataFusion processing layer, native Python extension, or independent mutable
  adapter state.
- No new Cargo root or package. The stable root, dated-nightly extractor, pinned Pyrefly
  sidecar, and Python adapter remain separate for their existing build/process reasons.
- No universal EAV table, persisted provider-local identity, persisted petgraph `NodeIndex`,
  raw Parquet substitute for Delta snapshots, or correctness that depends on Arrow metadata
  alone.
- No defensive semantic facade for hypothetical provider or DataFusion changes. The plan binds
  directly to the current pinned APIs and treats a future change as an explicit migration.
- No concurrent multi-host mutation of one workspace. Supporting it would reopen the design
  for a proved distributed fencing protocol.
- No dual production writes or indefinite runtime fallback. Old and new engines may compare
  frozen inputs read-only before cutover; only one owns mutation and serving at a time.
- No implementation-state creation, active-plan mutation, or execution in this planning turn.

### 1.3 Baseline and current trust posture

The v2 planning baseline and reconciled cleanup endpoint are both
`7184b86dc80adedc8a2b8d081179fa52d3dfee20`. The v1 audit inspected the four commits after the
v1 plan baseline and established that they refine predecessor machinery already selected for
migration/removal without changing the corrected target. The user has declared cleanup complete.
This successor therefore does not begin from an intentionally stale census and contains no
promise that cleanup may continue underneath it.

The repository has pre-existing tracked and untracked user work. Its bounded identity is the
frontmatter `working_tree_digest`, calculated without this design-v2/plan-v2 output pair to avoid
a circular hash. Read-only reconciliation found no implementation drift under `src/`,
`rustc-extractor/`, `pyrefly-sidecar/`, `contracts/`, or `tooling/ci/` relative to cleanup HEAD;
the remaining dirty paths are preserved inputs, not changes owned by this integration. WP01 must
reconcile any later drift before its first implementation edit, but it may not reinterpret a
changed design, doctrine, audit, or external-governance prerequisite as harmless staleness.

The audit intentionally did not rerun the broad suite after the user confirmed those tests pass.
It did verify the exact load-bearing library/source claims behind F-004–F-014. The focused audit
revalidation commands are recorded in the integration log below. Most are future packet-owned
recipes and their current absence is recorded as such; this does not weaken their eventual
acceptance requirement. At entry, `just doctor`, `just stable-graph-check`, and the externally
governed candidate-readiness gate must pass on the exact active candidate.

## 2. Source design and declared inputs

The table is planning-time evidence and is never restamped. WP01 intentionally versions the
eight authoritative masters; their later digest drift is planned input evolution, not permission
to edit this table. Any change to the accepted design or v2 principles makes this plan stale and
requires revision or design reopening before execution continues.

| path | sha256 |
|---|---|
| docs/designs/codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md | dd154d818b825b9b2d177a9aef0bf62e3db36b0288d49c235c9001f82edc7dc9 |
| docs/reviews/plan_audit_codefabric_execution_proved_relational_data_fabric_implementation_plan_v1_2026-08-29_2026-08-29_v1.md | 82cb98dbe1d9877f88ca857d061983a0b95760b9eb3656fce44ce4423299e1fb |
| docs/library_ref/full_data_fabric_design_principles_v2.md | eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | bc954edb4f1b148aab53a4b6107845fec8cada695fe3fad2a822c80ec347e885 |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | 5ecb66293d6c760f71f45d15631a5e3d8ebba484fe12e33c8767097f8fc6e7a8 |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | 510c82d287238ab9a44b93277654921dffa29c2c424397de6261bcee63d89745 |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 9821d7f1b3bab0da5d5620403dc65350f97f8ad1766f74e0068c6061963a69c4 |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v1.3.md | 8cf494c039bf339dc10e1f7865a842d7d7b7ae14b88f7c8b012137cba6c047db |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | 0e57ced51ad69d52009ede10550ca10efb045ac07506810c578446796411835e |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | 08bf1158329da247c99791980a4c51e16983b2e79663f2790a6db3c385c457b6 |
| docs/authoritative_design/codefabric_1.3_implementation_roadmap_v1.0.md | 5c4ffd3d240cd3ff7d1b87b5d5e1bef32f86415609b02ee894ed50a434e234a7 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/tree_sitter_rust_python.md | 615ce801958a3e74bf8cbbd5d759bade62958b1e4297185ebe8b18aa87e1428b |
| docs/library_ref/ruff_python_crates_advanced_reference_2026-08-18.md | f42e0b5e3d63c66bde68e2c3b79cef04d288eec52ee64e424f3d95578fc386d6 |
| docs/library_ref/pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md | 208582927c109dde0d399a7277442417006c878dfd21403a09a5c0bc7b2819e1 |
| docs/library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md | 1584a4ca9c7a06a495cfedacf585717aaec61949546d0191b19df48451451ea5 |
| docs/library_ref/petgraph.md | 8f5b19b2d9fbb9dfe2caf974b2a1f4c55b9244cfd167eb48956d225a076cccd9 |
| docs/library_ref/notify_debouncer_full_rust_reference.md | aaaa48e62a582b4c6c76a77f72b569fffafa5ea1b99ba1f7343af66921631a8b |
| docs/library_ref/gix_rust_advanced_reference.md | 8325e229602411385dc824350cbfca10fec287d16e4aaa3e9c4c35c3b4bead7f |
| docs/library_ref/fastmcp_python_advanced_reference_3.4.7.md | f3c1fc3def7ab14ce09a10b66b06f89f84419525781a368b625ea9d2ff338fb3 |
| docs/library_ref/pydantic_python_advanced_reference_2.13.4.md | 4f66f29a9fde6feed03a0755942db9bb9fb0834f57ff49ab80ab448d65d6a477 |
| docs/library_ref/grpcio_python_advanced_reference_1.83.0.md | e01fd5483b679cb62ef09e2c50228ab74eab298c2d559774f1f4c7ddd3320f78 |
| docs/library_ref/protobuf_python_advanced_reference_7.36.0.md | 2b9a2151f25e610ef75a43739b23852fd5faac3b183bbe1c374ff9923001798e |

### 2.1 Bounded staleness and impact evidence

The audit and integration sessions reconciled cleanup HEAD, targeted `ast-grep outline` over all
four build domains, and hidden-aware `rg` candidate searches over source, contracts, tests,
tooling, CI, rules, packaging, and live agent instructions. The committed drift from plan v1 is
fully included in the refreshed baseline. Current-tree evidence confirms these known-touch
clusters; they are navigation evidence, not an exhaustive must-touch manifest:

- model/generation: `src/bin/codefabric_model/**`, `tooling/model/**`, `src/ontology_*`,
  `src/schema_registry.rs`, `src/registries.rs`, `src/generated/**`, `contracts/registry/**`,
  `contracts/schema/**`, `contracts/semantic-fragments/**`, `contracts/bundles/**`,
  `contracts/generated/**`, `contracts/manifests/**`, and model/toolchain identity records;
- providers: `src/ruff_adapter/**`, `src/tree_sitter_adapter.rs`, `src/pyrefly_service.rs`,
  `pyrefly-sidecar/**`, `src/rustc_service.rs`, `rustc-extractor/**`, `src/provider_runtime*`,
  `src/core_facts.rs`, and `src/fact_ingest.rs`;
- fabric/query: `src/fabric.rs`, `src/fabric/**`, `src/snapshot.rs`,
  `src/snapshot_runtime.rs`, `src/operational_store.rs`, `src/semantic_query.rs`,
  `src/query_service.rs`, `src/governed_session.rs`, `src/daemon.rs`, and lifecycle/source
  modules;
- adapter/wire: `contracts/rpc/**`, `contracts/adapter/**`, `tooling/proto/**`,
  `src/generated/codefabric.*.rs`, and `codefabric-cpg-mcp/**`; and
- assurance/decommission: `tests/**`, `tooling/ci/**`, `scripts/**`, `rules/**`,
  `rule-tests/**`, `justfile`, Cargo manifests/locks, `.github/workflows/**`,
  `docs/authoritative_design/**`, `docs/spec_index/**`, `AGENTS.md`, and the live doctrine and
  library-routing files under `.claude/skills/**`; and
- hidden, mixed, fuzz, and package surfaces: `.ignore`, `.config/nextest.toml`, `src/lib.rs`,
  `src/contracts/mod.rs`, `fuzz/Cargo.toml`, `fuzz/fuzz_targets/**`, `fuzz/corpus/**`, every
  Cargo package/feature/build target, and the installed and wheel/sdist file manifests of the
  FastMCP adapter.

Each packet repeats a narrower preflight query immediately before editing. Dynamic dispatch,
macros, generated consumers, string keys, packaging, and re-exports remain explicit residual
surfaces for compiler, structural, and textual proof.

### 2.2 Plan-wide execution rules

- This successor has an external entry dependency: a separately governed active remediation must
  implement and prove inactive-candidate validation, plan-qualified overlap identity,
  predecessor disposition, crash-recoverable activation, and candidate state creation. No WP in
  this plan may implement, waive, or self-certify that prerequisite.
- A packet is complete only when every named acceptance check passes at its proving commit and
  again at HEAD. Working-tree progress is never completion evidence.
- Every new recipe named below is added by its owning packet before that recipe is used as an
  acceptance check. Existing intent-level recipes may be reshaped only when their former
  semantics are retired in the same dependency-closed packet.
- The old runtime remains authoritative through M04. Pre-cutover comparison is read-only over
  frozen equivalent inputs. No packet may introduce a production dual write, a fallback from
  the new model to a legacy authority, or an unbounded compatibility alias.
- A current library API beats illustrative plan detail. Adaptation inside LD-17 through LD-26
  is recorded in execution state; changing a library decision or invariant reopens the design.
- Historical designs, plans, reviews, decisions, released IDs, and independent expectations are
  preserved. Current generated artifacts, manifests, registries, and status records are not
  preserved merely because a test counts or fingerprints them.
- Independent provider/query/public/security/activation expectations and the exact frozen
  comparator are accepted before their implementation consumers. An expectation change creates
  a successor candidate and reruns proof; WP20 may execute accepted evidence, not author it.
- Each decommission claim requires positive target proof, zero-hit structural and hidden-aware
  textual proof over a declared candidate set, skipped/parse-error accounting, package/feature
  inspection, and a green compiler/type-check boundary.
- The legacy inventory universe is the union of Git tracked and untracked enumeration,
  hidden `--no-ignore` filesystem enumeration with explicit secret/build-output exclusions,
  language-aware parsing and re-export analysis, Cargo package/feature/build-target facts, and
  installed plus wheel/sdist package contents. A source that is skipped, unreadable, unparsed,
  ignored without disposition, or absent from the union is a failing unknown, never evidence of
  zero state.

## 3. Global target invariants

- **I-20 — One model authority.** Replayed typed model relations are the only current semantic
  authority. Advances v2 P1–P3, P26–P31, and P36.
- **I-21 — One serving epoch.** Admission through terminal result holds one sealed
  `Arc<FabricEpoch>`; no query discovers latest state. Advances P3, P11, P17–P20, and P31.
- **I-22 — One Arrow universe and schema contract.** Arrow 59.2.0 typed schemas and one-schema-
  per-relation IPC carry semantic data across Rust domains. One model-derived `SchemaContract`
  owns qualified logical schemas, storage mappings, restoration, index remapping, and every
  plan/stream/batch/sink validation. Advances P7, P12, P16, P27, P31–P32, and P36.
- **I-23 — Raw fidelity before normalization.** Exact provider-native typed observations,
  remainders, diagnostics, and coverage remain queryable beside canonical facts. Application
  CFG/dataflow/alias/effect/summary/graph rows have distinct derived authority and provenance.
  Maintains raw/normalized and absence-is-not-proof doctrine; advances P2–P3, P9–P10, and P20.
- **I-24 — Model-compiled execution.** Generic compilers lower model compositions to native
  DataFusion plans; semantic row mutations must change independently observed behavior.
  Advances P2, P14–P15, P27, P29, and P36.
- **I-25 — One mutation path.** Every durable fact/model/publication/maintenance change is an
  authorized idempotent `FabricCommand`; no production or test bypass exists. Advances P22,
  P33–P34, and P36.
- **I-26 — Fenced writer and event-derived current.** New admissions close before proved
  activation is appended and read back. One local target daemon writer plus a bridge/external
  cross-version revocation boundary mechanically denies the exact frozen legacy binary after
  `NEW_MUTATING`; Delta history is semantic authority and SQLite/ArcSwap are reconstructible
  caches. Advances P3, P11, P13, P18–P20, P31, P34, and P36.
- **I-27 — Proof belongs to the epoch.** Activation requires covered relational invariants,
  independent expectations, provenance, causality, policy, resource, and chain proof; unknown
  cannot pass. Advances P9–P10, P20, P27, P29, and P36.
- **I-28 — Semantic bounded composition.** All eight public forms compile from request
  relations inside one epoch; FastMCP remains presentation-only. Maintains QRY/SRV behavior and
  advances P6–P8 and P31.
- **I-29 — Optimizer and validator visibility.** The compiler selects the highest DataFusion
  rung per operation, including native `RecursiveQuery` where bounded. Every unavoidable
  provider/extension satisfies DataFusion 55 scan, expression, child replacement, property
  recomputation, reset, statistics, resource, cancellation, and invariant contracts honestly.
  Query-aware `StatisticsRequest` is not claimed without a complete application path. Advances
  P14–P16, P25, and P31.
- **I-30 — Exact current APIs.** Ruff 0.0.7; Pyrefly 1.2.0 at the pinned revision through exact
  Query/TSP/module-resolver/selected Glean/LSP surfaces; Tree-sitter 0.26.12 with pinned grammars;
  and nightly-2026-08-18 `rustc_public` plus the minimal selected `rustc_private` seam are direct
  integration targets. Advances P4 and P24; future drift is a migration, not pre-emptive loss.
- **I-31 — Compiler release is epoch meaning.** Reducer, metamodel ABI, primitives, functions,
  configuration, provider schemas, dependencies, toolchains, and wire set are replay inputs.
  Advances P3, P17–P19, and P30.
- **I-32 — Authorization by catalog construction.** Public requests see only an epoch-pinned
  reduced child catalog. Views are recompiled in the child or their complete bound provider/
  function/extension/variable/nested-view/store closure is verified against fresh allowlisted
  registries; no internal provider, session, DataFrame, plan, or identifier handle crosses the
  query port. Advances P5, P13, P21, P23, P31–P32, and P35–P36.
- **I-33 — One derived-analysis authority.** Every accepted analysis/query family has exactly
  one runtime producer or an explicit unsupported remainder, with algorithm, precision,
  invalidation, completeness, materialization, and independent semantic proof. Advances P2–P3,
  P9–P10, P17–P20, P27, P29, and P36.
- **I-34 — Semantic compilation trust is explicit.** Untrusted Rust build scripts and proc
  macros execute only through a fail-closed policy launcher with immutable inputs, private
  outputs, no network/credentials, bounded resources, and process-group cancellation. Any
  `TRUSTED_LOCAL` degradation is separately authorized and visible. Advances P5, P13, P16,
  P20–P24, and P32–P36.

Library-bearing packets bind to design LD-17 through LD-26. The exact references in section 2
are evidence for those decisions; they are not a second design authority.

## Audit Integration Log

Audit:
`docs/reviews/plan_audit_codefabric_execution_proved_relational_data_fabric_implementation_plan_v1_2026-08-29_2026-08-29_v1.md`
(`v1`, verdict `needs-redesign`). Source design/plan: design v1 and implementation plan v1.
Revised design/plan: design v2 and this implementation plan v2. Revision reason: preserve the
clean-sheet relational fabric while correcting exact provider/API authority, derived-analysis
coverage, Arrow/DataFusion/Delta contracts, trust and transition enforcement, proof ordering,
and legacy teardown. D-20–D-29, I-20–I-32, WP01–WP21, M01–M06, and DB01–DB07 retain their stable
identities; new design IDs are D-30–D-35/I-33–I-34, new packets are WP22–WP27, and final archive
retirement is DB08.

- `F-001` — `applied-plan`
  - Finding: the successor implemented the governance machinery needed to validate and activate itself outside its governed DAG.
  - Resolution: §§1.3 and 2.2 make separately governed candidate-readiness an entry prerequisite; WP01 has no preactivation implementation and begins only after the external proving commit, focused re-audit, approval, and atomic activation.
  - Revalidation: `just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md` — exit 1, recipe absent; the separately governed remediation must add and pass it before focused re-audit/activation.
  - Rationale: authority to execute must exist before successor work and cannot be self-certified.
- `F-002` — `applied-design`
  - Finding: WP18 committed durable activation before closing query admission.
  - Resolution: design I-26/D-26 and WP18 order candidate proof, admission closure, predecessor/fence revalidation, append/readback, swap, cache reconciliation, reopen, and acknowledgment.
  - Revalidation: `just activation-fault-matrix-check` — exit 1, recipe absent; WP18 adds it and cannot close until the corrected ordering/concurrency matrix passes.
  - Rationale: no query can be newly admitted on the predecessor after durable head selection.
- `F-003` — `applied-design`
  - Finding: a new journal could not revoke the exact frozen binary, which never reads it.
  - Resolution: design D-34 and WP21 require a bridge monotonic retirement generation or external service/storage authority revocation, persisted across restart/reboot and proved by restarting the exact frozen executable.
  - Revalidation: `just legacy-writer-fence-check` — exit 1, recipe absent; WP21 adds it and must restart the exact frozen executable against the selected enforcement profile.
  - Rationale: the journal records recovery state; an enforcement boundary the old release cannot bypass removes authority.
- `F-004` — `added-packet`
  - Finding: raw and derived facts had incorrect authorities and accepted CFG/dataflow/alias/effect/summary families lacked a complete implementation program.
  - Resolution: design D-30 plus WP23–WP25 split Python owner-local, Rust MIR-derived, common graph/effect/resource/interprocedural work and produce exact accepted-family-to-producer closure before WP13.
  - Revalidation: `just derived-analysis-authority-coverage-check` — exit 1, recipe absent; WP25 adds it after WP23/WP24 and it gates dependent query capability.
  - Rationale: generic compilation lowers a specified calculation; it does not silently supply missing analysis semantics.
- `F-005` — `applied-design`
  - Finding: WP09 attributed imports, definitions/xrefs, and structured diagnostics to nonexistent Pyrefly `Query` APIs and omitted semantic invalidation.
  - Resolution: design D-23/LD-25 and WP09 use exact Query, TSP/module-resolver, selected Glean/internal, and accepted LSP roles with semantic-environment identity and affected-module/reverse-importer refresh.
  - Revalidation: `just pyrefly-exact-surface-matrix-check && just pyrefly-semantic-environment-invalidation-check` — exit 1 at the first absent recipe; WP09 adds and must pass both halves.
  - Rationale: the target uses current APIs directly without pretending one facade exposes every family.
- `F-006` — `applied-design`
  - Finding: WP10 attributed stable compiler keys, borrowck, and derived dataflow to `rustc_public`.
  - Resolution: design D-23/LD-25, WP10, and WP24 separate public raw MIR/access, narrow exact-nightly private enrichment, and application-owned analyses with downgraded capability when enrichment is unavailable.
  - Revalidation: `just rustc-public-private-authority-check` — exit 1, recipe absent; WP10 adds it and WP24 consumes the accepted authority split.
  - Rationale: each current API and application algorithm owns only the facts it actually supplies.
- `F-007` — `added-packet`
  - Finding: no enforceable launcher contained untrusted Rust build scripts and procedural macros.
  - Resolution: design I-34/D-33 and new WP26 establish immutable inputs, private outputs, network/credential denial, resource/process limits, fail-closed platform containment, and hostile fixtures before WP10.
  - Revalidation: `just rustc-untrusted-compilation-sandbox-check` — exit 1, recipe absent; WP26 adds it and WP10 depends on its proving receipt.
  - Rationale: a claimed sandbox digest is not containment proof.
- `F-008` — `added-packet`
  - Finding: independently accepted expectations and the frozen comparator were produced after their consumers.
  - Resolution: new WP22 freezes and accepts provider/query/public/security/activation expectations plus the exact comparator before WP02 and every later implementation consumer; WP20 only re-executes those inputs.
  - Revalidation: `just independent-evidence-dag-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md` — exit 1, recipe absent; WP22 adds it and every evidence consumer has an explicit/transitive WP22 dependency in this draft.
  - Rationale: implementations cannot author or retrospectively arrange their own expected behavior.
- `F-009` — `added-packet`
  - Finding: no owner spanned logical Arrow meaning and Delta physical reconstruction.
  - Resolution: design I-22/D-31 and new WP27 own model-derived qualified schemas, cast/index mappings, restoration, and validation across WP04/WP05/WP06/WP17; the current ID adapter remains until replacement proof closes.
  - Revalidation: `just relational-schema-lifecycle-check && just delta-provider-contract-check` — exit 1 at the first absent recipe; WP27 adds the lifecycle gate and WP17 reruns both against Delta.
  - Rationale: native storage providers are usable only when domain schema meaning remains enforced and optimizer-visible.
- `F-010` — `applied-plan`
  - Finding: WP12 defaulted whole graph families to extensions and omitted exact physical-plan obligations.
  - Resolution: WP12 selects and causally proves the highest native/recursive/function/provider/extension rung per operation and owns the complete DataFusion 55 expression, rewrite, property, reset, statistics, resource, repetition, cancellation, and invariant contract.
  - Revalidation: `just graph-extension-conformance-check && just graph-execution-contract-check` — exit 1 at the first absent recipe; WP12 adds both after language-local analyses.
  - Rationale: opaque execution is reserved for proved irreducible kernels.
- `F-011` — `applied-design`
  - Finding: reduced catalogs did not neutralize providers/functions/stores already bound inside views.
  - Resolution: design I-32/D-35 and WP14 recompile public views in the child or recursively seal their complete bound dependency closure and install fresh allowlisted object-store/function/planner registries.
  - Revalidation: `just authorized-view-bound-authority-check && just access-catalog-isolation-check` — exit 1 at the first absent recipe; WP14 adds both and proves recursive bound closure/fresh registries.
  - Rationale: authorization is proved over bound authority, not names alone.
- `F-012` — `applied-plan`
  - Finding: WP17 combined an inert Delta version selector with a supplied snapshot and required impossible zero retries from pinned `OptimizeBuilder`.
  - Resolution: design D-27 and WP17 use exactly one selector authority, validate observed root/version, forbid `OptimizeBuilder`, and return controlled-write conflicts to `FabricCommand` reconciliation.
  - Revalidation: `just delta-exact-version-reconstruction-check && just fabric-transaction-contract-check` — exit 1 at the first absent recipe; WP17 adds/reshapes both and structurally forbids retrying optimize/DML routes.
  - Rationale: exact selection and retry ownership must be causal rather than decorative.
- `F-013` — `applied-design`
  - Finding: `StatisticsRequest` had no application producer, response mapping, or consumer.
  - Resolution: design D-24/LD-20 and WP05/WP11 remove query-aware statistics from the initial feature set, forward supplied vocabulary without loss, and prove ordinary honest provider/plan statistics.
  - Revalidation: `just provider-statistics-contract-check` — not completed: the existing predecessor check entered a cold Rust compile and was stopped at user direction; for planning it is assumed passing or near-passing. WP11 still reshapes and reruns it because the existing body does not prove the new no-query-aware-feature contract.
  - Rationale: an inert transport channel is not advertised as optimizer leverage.
- `F-014` — `applied-design`
  - Finding: the heterogeneous provider relation protocol lacked implementable IPC framing.
  - Resolution: design I-22/D-32/LD-26 and WP04 define one schema/dictionary scope per relation stream under an outer relation/stream/sequence/fingerprint/status frame with explicit trailers, interleaving, truncation, cancellation, backpressure, and partial coverage.
  - Revalidation: `just relational-arrow-boundary-check && just provider-protocol-check` — exit 1 at the first absent recipe; WP04 adds the relation-scoped boundary and reshapes the existing protocol check.
  - Rationale: independent provider lanes share one canonical, unambiguous data protocol.
- `F-015` — `added-decommission`
  - Finding: executable importer and live static migration-input readers survived until final cutover.
  - Resolution: DB01 now runs immediately after M01 and the bounded importer rollback decision; only committed predecessor bytes may enter a non-live no-reader archive, and new DB08 removes that archive at retention expiry.
  - Revalidation: `just model-importer-zero-state-check && just frozen-migration-input-live-read-zero-state-check` — exit 1 at the first absent recipe; early DB01 adds/passes both and DB08 separately removes the expired archive.
  - Rationale: temporary migration authority exits as soon as its purpose ends without discarding required comparison/rollback evidence.
- `F-016` — `applied-plan`
  - Finding: plan v1's baseline preceded the completed cleanup endpoint.
  - Resolution: frontmatter and §§1.3/2.1 now bind cleanup HEAD `7184b86...`, the bounded current working-tree digest, and current known-touch evidence.
  - Revalidation: `just plan-baseline-freshness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md` — exit 1, recipe absent; external candidate readiness must add/pass it before approval, while direct frontmatter/current-tree validation below proves this draft's recorded values.
  - Rationale: approval begins from current evidence while preserving user-owned dirty work.

## 4. Work packets

### WP01 — Reset target authority and establish the bounded transition

**Outcome.**

A versioned v2-aligned authoritative suite is the sole current target, the v1.3
static-artifact realization is explicitly historical, external compatibility and rollback
commitments are bounded, and legacy authority changes are frozen until decommission. The
runtime remains explicitly legacy and single-authority; this packet does not claim target
runtime conformance.

**Dependencies.**

External entry prerequisite: the separately governed candidate-readiness remediation has a
proving commit, this exact candidate passes `plan-candidate-readiness-check`, its focused re-audit
is accepted, and its state/pointer activation transaction is complete. Those are authorization
conditions, not work in WP01. WP01 is the first packet in this DAG and begins only from the
active v2 state created by that external transaction.

**Target invariants.**

I-20, I-25, I-27, and I-30. Advances v2 P3, P26–P31, and P36; maintains released wire and
historical evidence; risk of an authority vacuum is mitigated by one atomic current-suite
selection and an explicit legacy-runtime transition status.

**Design and library references.**

Design §§1.1, 2.5, 5.1–5.2 Stage 0; L-43, L-46, L-50–L-53. No new library mechanism is
selected.

**Change surface.**

**Preflight query.**

```sh
just spec-outline
rg --hidden -n 'AC-G-|DesiredTree|suite-manifest|artifact-census|design-principle|authoritative_design|upfront_design' AGENTS.md README.md CLAUDE.md docs/authoritative_design docs/spec_index contracts tooling/ci scripts justfile .github rules rule-tests -g '!.git/**' -g '!docs/library_ref/**'
rg --hidden -n 'full_data_fabric_design_principles\.md|semantic_design_principles_holistic\.md|AC-G-05|DesiredTree|generated artifact' AGENTS.md CLAUDE.md .claude .codex .agents docs/authoritative_design docs/spec_index justfile scripts tooling rules Cargo.toml -g '!.git/**' -g '!docs/library_ref/**'
sed -n '1,40p' docs/plans/active-plan.json
jq '{status,current_packet,packets}' docs/plans/state/codefabric-ontology-compiled-data-fabric_v3_state.json
git ls-files --cached --others --exclude-standard
rg --files --hidden --no-ignore -g '!.git/**' -g '!target/**' -g '!**/target/**' -g '!.envrc.local'
cargo metadata --locked --format-version 1
git diff --stat 7184b86dc80adedc8a2b8d081179fa52d3dfee20..HEAD -- AGENTS.md README.md CLAUDE.md .claude docs/authoritative_design docs/spec_index contracts tooling/ci scripts justfile .github rules rule-tests src tests fuzz Cargo.toml codefabric-cpg-mcp
just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md
```

**Known touch:** (verified this session)

The eight masters under `docs/authoritative_design/`, `docs/spec_index/**`, `AGENTS.md`,
`.claude/skills/_shared/{doctrine-policy,artifact-schemas}.md`, the DataFusion and delta-rs
skill routing files, `contracts/manifests/**`, the v1 principle registry/baseline and alignment
tooling, `justfile`, and `.github/workflows/ci.yml` currently encode or enforce the predecessor
authority shape. Candidate-validation, plan-qualified overlap, predecessor disposition, and
activation-transaction code are external prerequisites whose accepted proving commit must be
recorded in state before this packet starts; they are deliberately not a WP01 change surface.
The ordinary ignore stack omits generated/ignored candidates from common searches, while
`.config/nextest.toml`, `fuzz/Cargo.toml`, `fuzz/fuzz_targets/registry.rs`, its YAML corpus,
`src/lib.rs`, and `src/contracts/mod.rs` mix retained infrastructure with predecessor-specific
selection, export, or assurance edges.

**Required changes.**

- Publish a coherent versioned successor of all eight masters. Preserve the product behavior
  named in design §1.1 while replacing generated-registry, bundle, census, fingerprint,
  mutable-pointer, and hand-maintained traceability mechanisms with the target relational
  authority, execution, proof, activation, and decommission contracts.
- Select that successor as the single current target in human/agent navigation. Mark the
  predecessor suite historical without rewriting it or leaving two coequal current suites.
- Verify and record, without modifying governance machinery, the accepted external proving
  commit and receipts for inactive-candidate validation, plan-qualified overlap identity,
  predecessor disposition, candidate state creation, pointer compare-and-swap, durable activation
  recovery, and exactly one active plan. Any mismatch stops WP01 and returns to the external
  remediation owner; this packet may not patch around it.
- Record the only retained compatibility classes: released wire schemas/protocols, stable
  public IDs/errors/results, accepted tombstones/history, and persisted data needed inside the
  explicit rollback window. Derive the released-artifact census and require an accountable
  preserve, migrate, supersede, or tombstone decision for every released ID; never infer
  deletability from an absent current consumer. Everything else is replaceable unless execution
  discovers an external consumer and records a plan obligation.
- Replace current document conformance logic with intent-level v2 conformance and sole-authority
  checks. Do not generate a new static suite census or detector registry.
- Compile design L-20 through L-55 and this plan's added live-routing/package selectors into
  `legacy_disposition_selector` rows. Implement `legacy-disposition-coverage-check` before any
  deletion batch: fresh inventory must cover hidden/config/generated/package surfaces and fail
  on uncovered, overlapping, no-match, skipped, unparsed, or unresolved mixed-file selectors.
- Implement `legacy-inventory-universe-check` over the exact plan-wide union: Git tracked and
  untracked names; hidden `--no-ignore` names excluding `.git/**`, every `target/**`, and
  `.envrc.local`; language/parser and re-export results; Cargo packages, features, and build
  targets; and installed plus freshly built wheel/sdist manifests. It must explicitly inventory
  `.ignore`, nextest filters, mixed Rust roots, and every fuzz target/corpus and report all
  enumeration/parser/package omissions as unknown failures.
- Add a temporary `legacy-authority-freeze-check` that rejects new legacy registry, generator,
  package, or runtime consumers. It is owned by DB04 and must be deleted with the frozen inputs.

**Legacy disposition and decommission.**

Begins L-46, L-50, and L-51 replacement; preserves L-43, L-44, L-52, and L-53. It does not
delete historical files. DB04 removes the v1 governance implementation and transition guard
after the relational replacement proves closure.

**Acceptance checks.**

Packet completion requires every check below at its proving commit and at HEAD.

**Behavioral.**

- `just authoritative-design-conformance-check` — reshaped here to prove one coherent v2 target
  suite and retained high-level behavior.
- `just v2-authority-cutover-check` — new; proves sole current selection and historical routing.
- `just predecessor-plan-transition-check` — externally supplied entry proof; rechecks that the
  predecessor packet/state disposition was accepted before successor activation.

**Structural.**

- `just legacy-authority-freeze-check` — new; rejects new consumers/writers of frozen legacy
  authority.
- `just legacy-disposition-coverage-check` — new; derives complete, unique selector coverage
  from the current tree and rejects unresolved mixed files.
- `just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`
  — externally supplied entry proof; revalidates this exact plan, declared inputs, DAG, overlap,
  predecessor disposition, and candidate-state contract.
- `just plan-overlap-ledger-check` — externally supplied entry proof; proves plan-qualified keys,
  historical backfill, no cross-plan alias, and exact missing/stale closure.
- `just plan-activation-recovery-check` — externally supplied entry proof; proves idempotent
  recovery, predecessor compare-and-swap, exact successor-state reuse, durable predecessor
  disposition, and exactly one active execution authority.
- `just legacy-inventory-universe-check` — new; proves the complete multi-source inventory union,
  source reconciliation, and zero skipped/secret-exposing enumeration.
- `just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`
  — externally supplied candidate-path form, rerun against the active artifact.
- `just governance-scan`.

**Negative / zero-state.**

- `just legacy-suite-current-authority-zero-state-check` — new; old masters may exist only in
  historical routing and cannot be selected as current.
- `just plan-overlap-cross-plan-alias-zero-state-check` — externally supplied entry proof.

**Operational.**

- `just doctor`.
- `just stable-graph-check`.
- `just artifacts-check`.

Oracle catalog:

- Executable oracle: `just v2-authority-cutover-check`
- Executable oracle: `just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`
- Executable oracle: `just legacy-suite-current-authority-zero-state-check`
- Executable oracle: `just legacy-inventory-universe-check`

**Edit-Local Gates.**

`just spec-outline`; `just typos`.

**Packet-Local Gates.**

`just authoritative-design-conformance-check`; `just legacy-disposition-coverage-check`;
`just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`;
`just plan-overlap-ledger-check`; candidate-path `just plan-dependency-check`;
`just predecessor-plan-transition-check`; `just plan-activation-recovery-check`;
`just legacy-inventory-universe-check`; `just governance-scan`; `just artifacts-check`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen the design if preserved product behavior requires a static current registry or two
coequal suites. Revise the plan if current-authority routing has a larger non-historical
consumer surface than the preflight exposes. Treat a missing dated toolchain as an execution
environment blocker, not permission to weaken exact-version gates. Revise activation mechanics
only through the external governance-remediation plan if its receipt or mechanics are invalid;
WP01 must stop rather than paper over a partially published successor state.

**Rollback or Recovery.**

Before later code depends on the successor suite, revert the sole current-suite selection as
one change. Never restore authority by marking both suites current. Historical bytes remain
available throughout.

**Design-Bearing Contracts and Exemplars.**

The successor suite must state the cutover progression
`LEGACY_AUTHORITATIVE -> ... -> LEGACY_RETIRED` and the rule that design authority can advance
before runtime conformance only when the runtime is explicitly labeled as bounded legacy.

### WP02 — Implement the minimal metamodel, compiler release, and replay core

**Outcome.**

The stable root can reconstruct a model epoch from immutable typed migrations under an exact
`FabricCompilerRelease`, emit strongly typed Arrow model relations, self-describe the bounded
bootstrap, and distinguish structural rejection from semantic causality. No current generator
or registry is consulted by the new replay path.

**Dependencies.**

WP01 and WP22. The frozen comparator and independent evidence identities must exist before shared
model/compiler implementation begins.

**Target invariants.**

I-20, I-22, I-24, and I-31. Advances P1–P3, P17–P19, P26–P32, and P36; risk of rebuilding a
second static registry is mitigated by bootstrap closure and causal mutation.

**Design and library references.**

Design D-20, LD-17, LD-19, §§6.2–6.3; Arrow 59 and DataFusion 55 declared references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src --items exports --view names --match 'OntologyProgram|OntologyRelational|Schema|Registry|Identity|Domain'
rg --hidden -n 'OntologyProgramBundle|ModelGraph|DesiredTree|model_identity|identity_recipe|include!|include_bytes!' src contracts tooling/model scripts justfile tests -g '!.git/**' -g '!docs/library_ref/**'
cargo metadata --locked --format-version 1 --no-deps
```

**Known touch:** (verified this session)

`src/ontology_program.rs`, `src/ontology_relational_program.rs`, `src/ontology_plane.rs`,
`src/schema_registry.rs`, `src/registries.rs`, `src/identity.rs`, `src/domain_conformance.rs`,
`src/generated/**`, `src/bin/codefabric_model/**`, and model tests are current adjacent or
parallel authorities.

**Required changes.**

- Define the smallest closed Rust metamodel and exhaustive intrinsic algebra needed to express
  the model families in design D-20. Domain additions must use migration rows, not new bootstrap
  constants.
- Define immutable `ModelMigration` and `ModelDecision` values, predecessor validation,
  add/supersede/retire operations, and a pure deterministic reducer that yields Arrow
  `RecordBatch` relations.
- Define `FabricCompilerRelease` as a replay input covering build/source identity, metamodel and
  reducer ABI, logical algebra, primitive/function implementations, effective policy/config,
  provider schemas, exact dependency/toolchain pins, and released wire set.
- Emit the installed intrinsic surface as derived runtime rows from the installer that registers
  implementations; model rows own compositions and bindings, never duplicate primitive
  semantics.
- Prove canonical logical row/schema equality by replay, not digest equality. Cross-release use
  must reconstruct under the old release and execute an explicit migration to the new release.
- Add the `model-replay-check`, `model-bootstrap-closure-check`,
  `compiler-release-reconstruction-check`, `cross-release-epoch-migration-check`, and
  `bootstrap-staticness-check` recipes.

**Legacy disposition and decommission.**

Creates the replacement needed by L-27–L-29 and L-55. L-20–L-24 and L-54 remain frozen
migration evidence; this packet must not route the daemon through them or delete them.

**Acceptance checks.**

**Behavioral.**

- `just model-replay-check`.
- `just compiler-release-reconstruction-check`.
- `just cross-release-epoch-migration-check`.

**Structural.**

- `just model-bootstrap-closure-check`.
- `just bootstrap-staticness-check`.

**Negative / zero-state.**

- `just new-model-legacy-input-isolation-check` — new; proves the replay crate path has no
  generated/YAML/bundle loader edge.

**Operational.**

- `just root-check`.
- `just root-test-rust`.

Oracle catalog:

- Executable oracle: `just model-replay-check`
- Executable oracle: `just model-bootstrap-closure-check`
- Executable oracle: `just new-model-legacy-input-isolation-check`
- Executable oracle: `just compiler-release-reconstruction-check`

**Edit-Local Gates.**

`just root-fmt`; `just root-check`; focused replay and reducer unit tests.

**Packet-Local Gates.**

`just model-replay-check`; `just model-bootstrap-closure-check`; `just root-clippy`;
`just root-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen D-20 if a domain addition requires an expanding handwritten registry, an opaque
general-purpose VM, or current-code interpretation of an old release. Revise packet boundaries
if the minimal bootstrap cannot be dependency-closed without the catalog assembler in WP05.

**Rollback or Recovery.**

The old runtime remains authoritative. Revert the packet's proving commit and retain immutable
migration fixtures; never convert a failed replay into acceptance of a materialized bundle.

**Design-Bearing Contracts and Exemplars.**

```text
ModelMigration + FabricCompilerRelease
  -> pure replay
  -> typed model relations + decision dependency rows
```

### WP03 — Import and independently accept the initial relational model

**Outcome.**

A one-time, non-daemon importer converts every legacy model decision and released allocation
into reviewed migration rows or an explicit disposition. The accepted initial model is
semantically reviewed, replayable, and bijective to retained commitments without treating
legacy bytes or the importer as ongoing authority.

**Dependencies.**

WP02 and WP22.

**Target invariants.**

I-20, I-24, I-27, and I-31. Advances P3, P10, P18–P20, P27, and P29–P31; risk of mechanical
self-approval is mitigated by a separate expectation/review input.

**Design and library references.**

Design §5.2 Stage 1, §§5.4–5.5; L-22, L-44, L-52, L-54, and L-55.

**Change surface.**

**Preflight query.**

```sh
rg --files contracts/registry contracts/schema contracts/generated contracts/acceptance contracts/manifests tooling/model src/bin/codefabric_model | sort
rg --hidden -n 'schema-contract-ir|registry.*yaml|released-artifact-census|identity.recipe|model_artifact|ontology.program' contracts src tooling/model scripts tests codefabric-cpg-mcp -g '!.git/**' -g '!docs/library_ref/**'
ast-grep outline src/bin/codefabric_model --items exports --view names
git diff --name-status 7184b86dc80adedc8a2b8d081179fa52d3dfee20..HEAD -- contracts src tooling/model tooling/proto rules tests
```

**Known touch:** (verified this session)

`contracts/registry/**`, `contracts/schema/schema-contract-ir.json`, schema fragments,
`contracts/acceptance/released-artifact-census-v1.json`, `contracts/generated/**`,
`contracts/manifests/**`, `contracts/bundles/**`, `src/bin/codefabric_model/**`,
`tooling/model/**`, and accepted predecessor history through cleanup HEAD supply current migration
evidence. Execution must determine which semantic decisions/artifacts remain in HEAD or accepted
history rather than resurrecting stale working-tree candidates.

**Required changes.**

- Implement a one-time importer in the existing stable package/tooling boundary; it must never
  be linked into the daemon or become an alternate authoring command.
- Emit a row-level disposition for every imported decision: migrated, combined, split,
  superseded, tombstoned, preserved released commitment, or rejected as false static.
- Import released public IDs, canonical identity rules, wire allocations, accepted historical
  decisions, and any retained semantic meaning associated with L-54 after current-tree/history
  reconciliation. Never overwrite concurrent user work or manufacture migration rows for a
  no-longer-present candidate.
- Load independently authored expected rows through a separate review port and require semantic
  review of model types, authority, normalization, unknown, query, policy, state, and proof
  decisions. Byte completeness is not acceptance.
- Add `model-migration-bijection-check`, `model-migration-independent-review-check`,
  `released-identity-migration-check`, and `legacy-importer-runtime-isolation-check`.

**Legacy disposition and decommission.**

Encapsulates L-22 and L-54; preserves L-44 and L-52; begins L-55 replacement. DB01 runs
immediately after M01 and the explicitly bounded importer rollback decision: it deletes the
executable importer and all live static-input readers, while moving only committed comparator/
old-binary bytes into WP22's non-live no-reader archive. DB08 deletes that archive at final
retention expiry.

**Acceptance checks.**

**Behavioral.**

- `just model-migration-bijection-check`.
- `just model-migration-independent-review-check`.
- `just released-identity-migration-check`.

**Structural.**

- `just legacy-importer-runtime-isolation-check`.

**Negative / zero-state.**

- `just imported-decision-disposition-closure-check` — new; rejects an unclassified input,
  duplicate semantic authority, or expected row authored by the importer.

**Operational.**

- `just model-replay-check`.
- `just artifacts-check`.

Oracle catalog:

- Executable oracle: `just model-migration-bijection-check`
- Executable oracle: `just legacy-importer-runtime-isolation-check`
- Executable oracle: `just imported-decision-disposition-closure-check`
- Executable oracle: `just model-migration-independent-review-check`

**Edit-Local Gates.**

Importer unit tests against isolated copies; `just model-tooling-lint` while the old tooling
environment remains present.

**Packet-Local Gates.**

`just model-migration-bijection-check`; `just model-replay-check`; `just root-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Revise the plan if current inputs contain a material semantic family absent from D-20 or if
released commitments require a longer importer-specific rollback decision. Reopen the design if a retained
semantic decision cannot be represented as typed relations without a parallel live authority.

**Rollback or Recovery.**

Discard candidate migration events and rerun from the immutable inputs. Never edit accepted
history in place. The legacy runtime remains unchanged and authoritative.

**Design-Bearing Contracts and Exemplars.**

The importer output is `ModelMigration` input plus a separately owned review/disposition
relation; no generated current model artifact is installed.

### WP04 — Establish the Arrow schema and cross-process data boundary

**Outcome.**

One Arrow 59.2.0 semantic boundary represents model, provider, fact, proof, and result data with
strong physical types and validated metadata. A versioned Arrow IPC data protocol coexists with
narrow released Protobuf control messages and rejects opaque semantic payloads on the new path.

**Dependencies.**

WP02 and WP03.

**Target invariants.**

I-22, I-23, I-29, I-30, and I-31. Advances P7, P9, P12, P16, P20, and P32; maintains process
isolation and released control contracts.

**Design and library references.**

Design D-23, D-31–D-32, LD-17, LD-20, LD-25, LD-26, §§6.4–6.5; Arrow 59, DataFusion 55,
grpcio/protobuf, and exact code-fact references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/fact_ingest.rs --items structure --view digest
rg --hidden -n 'ProviderFactMessage|ProviderIpcContract|RecordBatch|StreamWriter|StreamReader|Binary|_json|bytes.*payload|prost::Message' src rustc-extractor pyrefly-sidecar contracts/rpc tooling/proto codefabric-cpg-mcp tests -g '!.git/**' -g '!docs/library_ref/**'
rg -n 'arrow-|datafusion|prost|tonic' Cargo.toml Cargo.lock rustc-extractor/Cargo.toml pyrefly-sidecar/Cargo.toml codefabric-cpg-mcp/pyproject.toml codefabric-cpg-mcp/uv.lock
```

**Known touch:** (verified this session)

`src/fact_ingest.rs`, `src/provider_types.rs`, `contracts/rpc/**`, `tooling/proto/**`,
`src/generated/codefabric.*.rs`, both auxiliary Cargo roots, and adapter daemon/wire tests own
the current cross-process boundary.

**Required changes.**

- Compile model logical types into Arrow schemas using nested, dictionary, decimal, timestamp,
  and fixed-binary types where semantically correct. Treat extension metadata as annotation
  until an enforcing consumer and fault prove otherwise.
- Define provider run/schema/coverage/remainder/provenance relations and an outer versioned
  control protocol with relation ID, stream ID, schema fingerprint, sequence, source/context
  pins, flow-control acknowledgments, and terminal status. Each relation uses an independently
  framed Arrow IPC stream with exactly one schema and dictionary scope. Specify interleaving,
  trailer ordering, end-of-stream, truncation, duplicate/out-of-order frames, partial failure,
  cancellation, bounded backpressure, and explicit partial/unknown coverage.
- Establish the separately owned `ProviderBoundaryContract` ingress/review port and ownership
  enforcement. WP22—not this boundary implementation or any adapter—authors and accepts the
  rows consumed by exact-provider packets.
- Keep job negotiation, authentication, deadlines, progress, and control errors in released
  Protobuf messages; do not expand Protobuf into the semantic fact plane.
- Provide a reusable conformance harness parameterized by provider that compares decoded Arrow
  rows to independently authored fixtures and exercises IPC, Parquet/Delta, DataFusion, and
  PyArrow round trips where applicable.
- Add a separately named relational-model/Arrow-IPC ingress fuzz target with deterministic seed
  replay for malformed schemas, metadata, dictionaries, batch framing, coverage trailers, and
  model-migration rows. It is the target replacement for the predecessor YAML `registry` fuzz
  target, but the old target/corpus remains frozen until DB04 can prove consumer-first removal.
- Add `relational-arrow-boundary-check`, `provider-native-arrow-conformance-check <provider>`,
  `new-provider-protocol-opaque-payload-rejection-check`, and `proto-contract-check`; reshape
  `provider-protocol-check` around the new versioned boundary while retaining old-wire tests.
  `proto-contract-check` proves `.proto` compatibility, descriptor reproduction, generated
  Rust/Python equivalence, and shared-wire interoperability without treating generated caches as
  authority.

**Legacy disposition and decommission.**

Begins L-25 reshape and supplies the target boundary for L-30–L-34. Generated stubs remain a
derivable foreign-build cache only if packaging proof requires them. Opaque legacy payloads stay
isolated until DB02.

**Acceptance checks.**

**Behavioral.**

- `just relational-arrow-boundary-check`.
- `just provider-protocol-check`.

**Structural.**

- `just new-provider-protocol-opaque-payload-rejection-check`.

**Negative / zero-state.**

- `just arrow-universe-check` — new; rejects a second Arrow type universe or semantic JSON field
  in the new protocol.
- `just relational-model-ingress-fuzz-seeds-check` — new; deterministically replays the accepted
  target-protocol regression corpus without claiming an unbounded fuzz run as a gate.

**Operational.**

- `just stable-graph-check`.
- `just adapter-test`.

Oracle catalog:

- Executable oracle: `just relational-arrow-boundary-check`
- Executable oracle: `just provider-protocol-check`
- Executable oracle: `just new-provider-protocol-opaque-payload-rejection-check`
- Executable oracle: `just proto-contract-check`

**Edit-Local Gates.**

Schema/IPC round-trip unit tests; `just root-check`; relevant auxiliary `*-check` recipe.

**Packet-Local Gates.**

`just relational-arrow-boundary-check`; `just provider-protocol-check`;
`just relational-model-ingress-fuzz-seeds-check`; `just root-test`; `just sidecar-check`;
`just extractor-check`; `just adapter-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen I-22 only if a justified process boundary cannot exchange Arrow IPC without losing an
accepted semantic family. Revise the plan if released clients require a separately versioned
control transition; do not fall back to JSON blobs.

**Rollback or Recovery.**

Version the new protocol without changing the old runtime route. Reject incompatible handshakes
rather than decode ambiguously; revert the new version and fixtures if cross-domain proof fails.

**Design-Bearing Contracts and Exemplars.**

```text
open(relation_id, stream_id, schema_fingerprint, pins)
  -> one-schema Arrow IPC messages with stream-local dictionaries
  -> ipc_end
  -> coverage/remainder/diagnostic trailer
  -> terminal(stream_id, status)
```

Control frames may interleave streams; semantic rows never appear in Protobuf and heterogeneous
`RecordBatch` schemas never share one IPC stream.

### WP05 — Build immutable epoch catalogs and honest provider foundations

**Outcome.**

A model-only candidate `FabricEpoch` can be built from a fresh DataFusion 55 catalog/session,
exact runtime configuration, and honest provider adapters, then sealed so production callers
cannot mutate registrations or obtain a raw session handle.

**Dependencies.**

WP02, WP04, and WP27.

**Target invariants.**

I-20–I-22, I-29, I-31, and I-32. Advances P3, P7–P8, P11–P15, P17, P31–P32, and P35.

**Design and library references.**

Design D-21–D-22, D-31, LD-17–LD-18, LD-20; DataFusion 55 catalog,
`SessionStateBuilder`, `ViewTable`, `TableProvider`, `ScanArgs`, statistics, constraints, and
runtime APIs.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/fabric.rs --items structure --view digest --match 'WorkspaceFabric|ServingQuerySession|SnapshotProviderCatalog|FabricTable'
rg --hidden -n 'SessionContext|SessionState|SessionStateBuilder|register_|CatalogProvider|SchemaProvider|TableProvider|ViewTable|MemTable|ServingSnapshot|ArcSwap|scan_with_args' src tests tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
ast-grep run --lang rust --pattern '$CTX.register_table($$$A)' src tests --inspect summary
```

**Known touch:** (verified this session)

`src/fabric.rs`, `src/fabric/serving.rs`, `src/fabric/snapshot_catalog.rs`,
`src/snapshot.rs`, `src/snapshot_runtime.rs`, `src/governed_session.rs`, and their tests own
current session, provider, and serving-snapshot construction.

**Required changes.**

- Build each epoch from a fresh `MemoryCatalogProviderList`, one `MemoryCatalogProvider`, role
  `MemorySchemaProvider`s, exact runtime, object stores, functions, analyzer/optimizer rules,
  providers, and validated `ViewTable`s. These in-memory catalog objects are sealed session-local
  runtime structure, never durable metadata or model authority.
- Define the ownership-bearing `FabricEpoch` and builder boundary. Only the builder may hold
  registration handles; published code receives a sealed query/inspection facade.
- Implement one structured `plan_scan(ScanArgs)` path for every non-native provider. Forward
  projection, filters, limit, and any caller-supplied `statistics_requests` without inventing a
  producer or consumer; report ordinary pushdown, constraints, functional dependencies,
  ordering, partitioning, and statistics only after proof.
- Construct providers and qualified catalog schemas through WP27's `SchemaContract`; retain the
  current `Id16ContractProvider` until the generic adapter proves every logical/physical phase
  and negative fixture, then remove it in the same proving change.
- Register constraints/functional dependencies only after independent relational truth checks;
  never treat DataFusion metadata as enforcement.
- Expose internal catalog/runtime observations as derived `system` relations and prove closure
  between model relations, live `information_schema`, installed providers, and intrinsic rows.
- Add `fabric-epoch-construction-check`, `relational-catalog-closure-check`,
  `table-provider-contract-check`, and `active-catalog-mutation-zero-state-check`.

**Legacy disposition and decommission.**

Begins L-27–L-29, L-35, and L-37 replacement. Old snapshot/catalog paths remain live only for
the predecessor runtime until DB03.

**Acceptance checks.**

**Behavioral.**

- `just fabric-epoch-construction-check`.
- `just table-provider-contract-check`.

**Structural.**

- `just relational-catalog-closure-check`.

**Negative / zero-state.**

- `just active-catalog-mutation-zero-state-check`.

**Operational.**

- `just ontology-runtime-resource-check` — retained as an early bounded-session regression
  check and reshaped fully in WP19.

Oracle catalog:

- Executable oracle: `just fabric-epoch-construction-check`
- Executable oracle: `just relational-catalog-closure-check`
- Executable oracle: `just active-catalog-mutation-zero-state-check`
- Executable oracle: `just table-provider-contract-check`

**Edit-Local Gates.**

Focused catalog/provider unit tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just fabric-epoch-construction-check`; `just table-provider-contract-check`;
`just root-clippy`; `just root-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen D-21 if DataFusion mutability cannot be contained by ownership or if a required trust
domain cannot safely share a runtime. Revise the packet if current provider wrappers cannot be
adapted dependency-closed without moving their semantic migration into WP11.

**Rollback or Recovery.**

Candidate epochs are unpublished. Drop the candidate and revert the proving commit; never
mutate the active predecessor catalog to approximate the target.

**Design-Bearing Contracts and Exemplars.**

The conceptual `FabricEpoch` ownership signature in design D-21 is binding; exact module/file
decomposition is left to execution.

### WP06 — Compile the relational model into native DataFusion programs

**Outcome.**

One small family of exact DataFusion 55 adapters compiles model-owned schema, normalization,
authority, unknown, derivation, semantic-query, policy, and proof compositions into typed
`Expr` and `LogicalPlan` trees. Intrinsic implementation closure is derived from the runtime,
and no SQL-string, generated plan catalog, operation-kind switchboard, or opaque physical plan
becomes semantic authority.

**Dependencies.**

WP03 and WP05.

**Target invariants.**

I-20, I-24, I-27, and I-29–I-31. Advances P2, P8, P14–P15, P19, P27, P29, and P36; risk of a
hidden interpreter is mitigated by a closed typed algebra, visible plans, and causal faults.

**Design and library references.**

Design D-24, LD-19–LD-21, §§6.3 and 6.5; DataFusion 55 expression, logical-plan, function,
analyzer, optimizer, planner-extension, and statistics APIs.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src --items exports --view names --match 'OntologyProgramCompiler|OntologyRelationalProgram|OntologyRule|SemanticQuery|DomainConformance|Governed'
rg --hidden -n 'LogicalPlanBuilder|Expr::|DataFrame|sql\(|operation_kind|plan_node_kind|calculation|UDF|TableFunction|ExtensionPlanner|domain_conformance' src contracts tooling/ci tests -g '!.git/**' -g '!docs/library_ref/**'
ast-grep run --lang rust --pattern '$CTX.sql($$$A)' src tests --inspect summary
```

**Known touch:** (verified this session)

`src/ontology_relational_program.rs`, `src/ontology_executor.rs`, `src/ontology_rules.rs`,
`src/domain_conformance.rs`, `src/semantic_query.rs`, generated query/calculation artifacts,
and ontology program/compiler tests contain the current planning logic or parallel catalogs.

**Required changes.**

- Implement exact adapters for catalog assembly, normalization/authority/unknown selection,
  derivation, semantic request binding, policy analysis, and proof-plan construction over the
  closed semantic algebra.
- Resolve typed relation/field/primitive IDs against live `DFSchema` and derived intrinsic
  runtime rows. Reject unresolved, ill-typed, or unauthorized references before execution.
- Use native projection/filter/join/semi-join/anti-join/union/window/aggregate/sort/limit and
  built-ins first; then the narrowest registered UDF family. Reserve a table function for
  planning-time scalar-named providers and a typed logical extension for relational children.
- Walk every pinned plan/expression variant, including nested and subquery forms, for policy,
  resource, source, function, extension, output-schema, and metadata constraints. Optimizer
  rules may improve performance but cannot establish correctness.
- Emit decision dependency and extension-selection observations from actual compilation. Store
  `EXPLAIN`/serialized plans only as diagnostic/cache material.
- Add `model-plan-causality-check`, `semantic-plan-conformance-check`,
  `function-runtime-closure-check`, and `plan-visibility-check`.

**Legacy disposition and decommission.**

Implements the replacement side of L-28 and L-39 and prepares L-36/L-41 replacement. Old
generated catalogs and phrase branches remain predecessor-only until DB01/DB03.

**Acceptance checks.**

**Behavioral.**

- `just semantic-plan-conformance-check`.
- `just model-plan-causality-check`.

**Structural.**

- `just function-runtime-closure-check`.
- `just plan-visibility-check`.

**Negative / zero-state.**

- `just new-compiler-opaque-plan-zero-state-check` — new; rejects SQL strings, stored plan
  authority, generic bytecode, and untyped operation dispatch in the target compiler.

**Operational.**

- `just table-provider-contract-check`.
- `just root-test-rust`.

Oracle catalog:

- Executable oracle: `just semantic-plan-conformance-check`
- Executable oracle: `just function-runtime-closure-check`
- Executable oracle: `just new-compiler-opaque-plan-zero-state-check`
- Executable oracle: `just model-plan-causality-check`

**Edit-Local Gates.**

Focused compiler/algebra unit tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just model-plan-causality-check`; `just semantic-plan-conformance-check`;
`just function-runtime-closure-check`; `just root-clippy`; `just root-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen D-24 if accepted semantics require a general-purpose interpreter or correctness-only
physical optimizer rule. Revise the plan if a DataFusion 55 variant/API differs from the exact
reference or if one compiler cannot remain generic without hiding domain decisions.

**Rollback or Recovery.**

The compiler runs only for unpublished candidates. Revert its proving commit and replay the
model; never retain a failed plan artifact as a substitute for reconstruction.

**Design-Bearing Contracts and Exemplars.**

```text
model relation IDs + derived intrinsic IDs + typed bindings
  -> exact DataFusion Expr/LogicalPlan
  -> validated schema + dependency observations
```

### WP07 — Make proof, provenance, capability, and governance executable

**Outcome.**

Candidate epochs carry computed proof relations whose expectations and faults are independently
owned. Coverage, provenance closure, semantic causality, capability status, repository
governance, and terminal pass/fail/unknown are executed against exact candidate inputs; a
producer-authored expectation or empty uncovered input can never authorize activation.

**Dependencies.**

WP03, WP05, WP06, and WP22.

**Target invariants.**

I-20, I-24, I-27, I-30, and I-31. Advances P9–P10, P18–P20, P27, P29, and P36; risk of
self-certification is mitigated by separate ownership/provenance and required mutant sensitivity.

**Design and library references.**

Design D-28, §§6.1–6.4; L-44, L-45, L-47, and L-49; DataFusion programmatic query/execution
surfaces from LD-19.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src --items exports --view names --match 'OntologyGate|Candidate|Receipt|Golden|Evaluator|Capability|Provenance'
rg --hidden -n 'gate_b|golden|oracle|expectation|receipt|provenance|capability|supported|unknown|mutant|causal' src tests contracts tooling/ci scripts justfile rules rule-tests -g '!.git/**' -g '!docs/library_ref/**'
rg --files tests/golden contracts/acceptance docs/reviews | sort
```

**Known touch:** (verified this session)

`src/ontology_gate.rs`, `src/ontology_candidate.rs`, `src/functional_golden/**`,
`src/gate_b_candidate/**`, `tests/golden/**`, acceptance/fault/comparison registries, and many
`tooling/ci/**` gates currently mix useful evidence with producer-generated or census-based
authority.

**Required changes.**

- Define typed invariant, expectation, coverage, violation, provenance-edge, proof-run, mutant,
  and receipt relations. The expectation/fault port must be physically and logically separate
  from production model/provider/compiler output.
- Compile each enforcement oracle to a violation relation and require complete covered inputs as
  well as zero violations. Missing provider/runtime/expectation input yields `unknown`.
- Emit direct and multi-input lineage from the same execution that emits a fact; resolve closure
  to exact source images, model decisions, provider runs, table versions, compiler release, and
  independent expectations.
- Distinguish structural model faults, which may prove construction by rejection, from semantic
  faults, which must alter an independently observed plan/result/authorization/unknown/violation.
- Load repository/build/import/package facts as Arrow and execute governance queries. Text and
  structural scanners remain independent candidate/residue instruments, not semantic authority.
- Add `fabric-epoch-proof-closure-check`, `provenance-closure-check`,
  `oracle-independence-check`, `model-causality-check`, and the M01
  `relational-model-foundation-check`.

**Legacy disposition and decommission.**

Begins L-45, L-47, and L-49 replacement while preserving independent L-44 evidence. Old Gate B
and static governance paths remain read-only comparison evidence until WP20/DB04.

**Acceptance checks.**

**Behavioral.**

- `just fabric-epoch-proof-closure-check`.
- `just model-causality-check`.

**Structural.**

- `just provenance-closure-check`.
- `just oracle-independence-check`.

**Negative / zero-state.**

- `just producer-authored-expectation-zero-state-check` — new; rejects any expectation flow from
  production model/provider/compiler output.

**Operational.**

- `just governance` — reshaped to execute relational governance plus independent residue checks.

Oracle catalog:

- Executable oracle: `just fabric-epoch-proof-closure-check`
- Executable oracle: `just provenance-closure-check`
- Executable oracle: `just producer-authored-expectation-zero-state-check`
- Executable oracle: `just model-causality-check`

**Edit-Local Gates.**

Focused proof compiler/resolver tests; `just root-check`; `just governance-scan`.

**Packet-Local Gates.**

`just fabric-epoch-proof-closure-check`; `just model-causality-check`;
`just oracle-independence-check`; `just root-test`.

**Integration Milestone.**

M01. WP07 owns `just relational-model-foundation-check`.

**Replan Triggers.**

Reopen the design if independent semantics cannot be sourced without the implementation under
test. Revise the plan if proof cannot be dependency-closed before exact providers; in that case
retain the framework packet and defer only provider-populated rows to WP11.

**Rollback or Recovery.**

An unproved candidate is discarded. Never persist a green flag independently of its exact proof
rows and inputs; rerun proof after any candidate or expectation correction.

**Design-Bearing Contracts and Exemplars.**

```text
complete inputs + zero violation rows + independent expected discrimination
  -> pass
missing coverage -> unknown
any violation/mismatch/surviving required mutant -> fail
```

### WP08 — Emit exact Tree-sitter and Ruff provider-native relations

**Outcome.**

The stable root consumes the pinned Tree-sitter and Ruff APIs directly and emits loss-minimized,
typed provider-native relations for syntax, tokens, trivia, AST, scopes/bindings, imports,
references, diagnostics, raw kinds, and remainders. Python CFG/dataflow remain application-owned
work in WP23.

**Dependencies.**

WP04, WP06, and WP22. This exact-provider lane may proceed in parallel with the other independently
owned exact-provider lanes in isolated worktrees.

**Target invariants.**

I-22, I-23, I-29, and I-30. Advances P2, P4, P7, P9, P20, and P24; maintains raw/normalized
coexistence and application-owned canonical identity.

**Design and library references.**

Design D-23, LD-17, LD-25; Tree-sitter and Ruff exact references; GEN provider authority rules.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/ruff_adapter.rs src/ruff_adapter src/tree_sitter_adapter.rs src/source_syntax.rs src/python_semantic.rs --items structure --view digest
rg --hidden -n 'RuffSnapshot|PythonFrontendBatch|TreeSitterSnapshot|raw_kind|semantic_json|cfg_json|binding_json|diagnostic' src contracts tests tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
rg -n 'ruff_python_|tree-sitter' Cargo.toml Cargo.lock
```

**Known touch:** (verified this session)

`src/ruff_adapter.rs`, `src/ruff_adapter/**`, `src/tree_sitter_adapter.rs`,
`src/source_syntax.rs`, `src/python_semantic.rs`, current provider observation schemas, and
provider fixtures own this lane.

**Required changes.**

- Map current Tree-sitter parser/tree/edit/changed-range/query/capture/error surfaces to typed
  raw relations while retaining grammar-native kinds, fields, named/unnamed distinctions, source
  coordinates, and recovery errors.
- Map current Ruff parse/tokens/trivia/typed AST/indexer/semantic surfaces to typed raw relations
  without copying them through opaque semantic JSON or a lowest-common-denominator DTO. Do not
  emit application-built CFG, reaching definitions, liveness, alias/points-to, effects, or
  summaries under Ruff provenance.
- Emit run identity, exact provider/grammar revisions, requested/completed family coverage,
  provider-local identities, diagnostics, remainders, and unavailable reasons.
- Keep borrowed trees/AST/semantic models inside the adapter call. Canonical IDs are computed
  later from source/coordinate/semantic authority inputs.
- Consume independently authored and accepted `ProviderBoundaryContract` rows and fixtures for
  every family; the adapter implementer may propose evidence but cannot author the expected
  surface. Add provider-specific exact API compile probes and the no-argument aggregate
  `syntax-provider-native-check` and `syntax-provider-exact-api-check` recipes.

**Legacy disposition and decommission.**

Implements L-31 reshape and part of L-30. Opaque fields and defensive mirrors remain only on the
old route until WP11 switches normalization and DB02 removes them.

**Acceptance checks.**

**Behavioral.**

- `just syntax-provider-native-check` — new; aggregates the two exact API and Arrow conformance
  lanes without hiding per-family failures.
- `just provider-native-arrow-conformance-check tree-sitter`.
- `just provider-native-arrow-conformance-check ruff`.

**Structural.**

- `just syntax-provider-exact-api-check` — new.
- `just exact-provider-api-check tree-sitter`.
- `just exact-provider-api-check ruff`.

**Negative / zero-state.**

- `just syntax-provider-opaque-payload-zero-state-check` — new, scoped to the target route.

**Operational.**

- `just root-check`.
- `just root-test-rust`.

Oracle catalog:

- Executable oracle: `just syntax-provider-native-check`
- Executable oracle: `just syntax-provider-exact-api-check`
- Executable oracle: `just syntax-provider-opaque-payload-zero-state-check`
- Executable oracle: `just root-test`

**Edit-Local Gates.**

Focused adapter fixture tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

Both provider conformance invocations; both exact API probes; `just root-clippy`;
`just syntax-provider-native-check`; `just root-test`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen only the affected provider decision if a required current API family cannot be observed
without debug text or an escaping borrowed value. Add explicit remainder/unknown rows for
intentional omissions; never silently narrow the model.

**Rollback or Recovery.**

Keep the new provider route candidate-only. Revert its proving commit or publish a new provider
schema migration; do not reinterpret already identified rows under a changed schema.

**Design-Bearing Contracts and Exemplars.**

Every handler appears in derived `system.provider_surface`; closure compares it with independently
owned boundary rows and exercised fixtures.

### WP09 — Emit exact Pyrefly relations through Arrow IPC

**Outcome.**

The pinned Pyrefly sidecar emits typed context, module, inferred/declared/computed/expected type,
member, import-resolution, call-target, selected definition/xref, navigation-fallback,
diagnostic, unresolved, affected-module, and remainder relations through relation-scoped Arrow
IPC. Every family names its exact current authority; the module-level JSON payload no longer
participates in the target route.

**Dependencies.**

WP04, WP06, and WP22. This exact-provider lane may proceed in parallel with the other independently
owned exact-provider lanes in isolated worktrees.

**Target invariants.**

I-22, I-23, I-29, and I-30. Advances P2, P4, P7, P9, P20, and P24; maintains sidecar process
and exact revision isolation.

**Design and library references.**

Design D-23, LD-17, LD-25–LD-26; Pyrefly 1.2.0 exact reference and Arrow IPC reference.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline pyrefly-sidecar/src src/pyrefly_service.rs --items structure --view digest
rg --hidden -n 'get_type_table_in_file|get_callees_with_location|types_json|callees_json|diagnostics_json|FileWriter|StreamWriter|PyreflyProviderDriver' pyrefly-sidecar src contracts tests tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
rg -n 'pyrefly|arrow-' pyrefly-sidecar/Cargo.toml pyrefly-sidecar/Cargo.lock
```

**Known touch:** (verified this session)

`pyrefly-sidecar/src/pyrefly_link.rs`, its protocol/main code, `src/pyrefly_service.rs`,
sidecar manifests/lock, provider schemas, and protocol/integration fixtures own this lane.

**Required changes.**

- Implement the accepted fact-family/surface matrix exactly: `Query` for bulk inferred types,
  callees, members, subtype, and qualified-target helpers; TSP/pinned module resolution for
  imports and declared/computed/expected distinctions; the deliberately selected exact-revision
  Glean/internal seam for accepted bulk definitions/xrefs; and LSP only for a named accepted
  navigation fallback. `add_files` rendered diagnostics are not represented as a structured
  diagnostic API. Unsupported families emit explicit remainder/capability rows.
- Maintain one long-lived workspace state under the actual Pyrefly configuration/module
  resolution. Record semantic-environment identity, `Require::Everything` versus
  `Require::Exports` tier, source-coordinate conversions, actual affected modules, and
  conservative reverse-importer refresh whenever the exact surface cannot prove a smaller set.
- Preserve provider-local keys and response-local indices as provenance only. Emit the source
  image, analysis context, exact revision/config, coordinates, completeness, remainders, and
  explicit unsupported/unknown behavior.
- Replace semantic File/JSON payloads with the versioned bounded Arrow stream protocol while
  retaining control-plane backpressure, cancellation, timeout, source/context validation, and
  sandbox failure semantics.
- Consume independently authored and accepted boundary rows/fixtures; the sidecar implementer
  cannot author expected coverage. Add exact pinned-source compile probes and the no-argument
  `pyrefly-provider-native-check`, `pyrefly-exact-api-check`,
  `pyrefly-exact-surface-matrix-check`, and
  `pyrefly-semantic-environment-invalidation-check` aggregate recipes.

**Legacy disposition and decommission.**

Implements L-32 and part of L-30/L-34. The old payload remains predecessor-only until WP11 and
DB02.

**Acceptance checks.**

**Behavioral.**

- `just pyrefly-provider-native-check` — new.
- `just provider-native-arrow-conformance-check pyrefly`.
- `just provider-protocol-check`.

**Structural.**

- `just pyrefly-exact-api-check` — new.
- `just pyrefly-exact-surface-matrix-check` — new; compile- and behavior-probes the exact
  authority assigned to every accepted family.
- `just exact-provider-api-check pyrefly`.

**Negative / zero-state.**

- `just pyrefly-opaque-payload-zero-state-check` — new, scoped to the target route.

**Operational.**

- `just sidecar-ci-fast`.
- `just semantic-sandbox-host-matrix-check`.
- `just pyrefly-semantic-environment-invalidation-check` — new.

Oracle catalog:

- Executable oracle: `just pyrefly-provider-native-check`
- Executable oracle: `just pyrefly-exact-surface-matrix-check`
- Executable oracle: `just pyrefly-opaque-payload-zero-state-check`
- Executable oracle: `just pyrefly-semantic-environment-invalidation-check`

**Edit-Local Gates.**

Focused sidecar/API/IPC tests; `just sidecar-fmt`; `just sidecar-check`.

**Packet-Local Gates.**

`just sidecar-ci-fast`; `just pyrefly-provider-native-check`;
`just pyrefly-exact-surface-matrix-check`; `just pyrefly-semantic-environment-invalidation-check`;
`just provider-protocol-check`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen the Pyrefly boundary if an accepted family is absent from the pinned API or requires a
long-lived provider object outside the sidecar call. Revise the IPC packet if streaming cannot
remain bounded; do not move semantic execution into Python.

**Rollback or Recovery.**

Keep old and new protocol versions separately negotiated during read-only migration. A target
decode failure is a typed provider gap, never fallback interpretation as legacy JSON.

**Design-Bearing Contracts and Exemplars.**

Provider data uses Arrow stream IPC; Protobuf remains job/control metadata only.

### WP10 — Emit exact rustc MIR and semantic relations through Arrow IPC

**Outcome.**

The dated-nightly extractor emits typed compilation, public item/type/instance/MIR body/block/
local/place/operand/rvalue/statement/terminator/CFG/call/access relations plus a narrow private
stable-key/source-hygiene/borrowck/selected mono-vtable enrichment, diagnostics, and remainders.
Application ownership/dataflow/alias/drop/async analyses remain WP24 outputs. Names/counts/debug
strings no longer stand in for available compiler facts on the target route.

**Dependencies.**

WP04, WP06, WP22, and WP26, plus a resolved dated-nightly environment. This exact-provider lane may proceed
in parallel with the other independently owned exact-provider lanes in isolated worktrees.

**Target invariants.**

I-22, I-23, I-29, I-30, and I-34. Advances P2, P4–P5, P7, P9, P13, P20, and P24;
maintains compiler-private process isolation and application-owned canonical identity.

**Design and library references.**

Design D-23, LD-17, LD-25–LD-26; the pinned `rustc_public`/MIR reference and Arrow IPC
reference.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline rustc-extractor/src src/rustc_service.rs --items structure --view digest
rg --hidden -n 'OwnedMirItem|rustc_public::run|all_local_items|CrateDef|Body|Terminator|successor|statement_kinds|debug' rustc-extractor src contracts tests tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
rg -n 'nightly|rustc-|arrow-' rustc-extractor/Cargo.toml rustc-extractor/Cargo.lock rustc-extractor/rust-toolchain.toml
```

**Known touch:** (verified this session)

`rustc-extractor/src/rustc_link.rs`, `rustc-extractor/src/wrapper.rs`, extractor protocol/main
code, `src/rustc_service.rs`, extractor manifests/lock/toolchain, schemas, and fixtures own this
lane.

**Required changes.**

- Bind directly to pinned `rustc_public` raw item/type/instance/MIR/access surfaces. Retain native
  structure and relationships in typed relations; use debug strings only as supplemental raw
  evidence where no typed API exists and classify the remainder explicitly.
- Implement the smallest version-pinned `rustc_private` adapter required for stable compiler
  keys, exact source/hygiene, borrowck, and selected mono/vtable facts. Compile- and
  behavior-probe public and private seams independently; no borrowed compiler value escapes the
  callback/process boundary.
- Derive canonical Rust identity from the private stable-key inputs when available. If private
  enrichment is unavailable under an accepted posture, use the documented application
  qualified-name key and emit downgraded capability rather than claiming stable compiler
  identity or exact borrowck. MIR locals/blocks and provider handles remain run-local.
- Do not emit conservative reaching definitions, liveness, ownership state, alias/points-to,
  drop/resource, or async facts as raw compiler output; WP24 owns those algorithms and their
  provenance.
- Stream bounded Arrow batches with compilation context, toolchain/revision, source pins,
  requested/completed coverage, diagnostics, cancellation, and corruption behavior.
- Consume independently authored and accepted boundary rows/fixtures; the extractor implementer
  cannot author expected coverage. Compile-probe the exact dated API and add the no-argument
  `rustc-provider-native-check`, `rustc-exact-api-check`, and
  `rustc-public-private-authority-check` aggregate recipes.
- Consume only WP26 launcher receipts for untrusted compilation. A claimed sandbox digest or
  direct host compiler invocation cannot satisfy this packet.

**Legacy disposition and decommission.**

Implements L-33 and part of L-30/L-34. `OwnedMirItem` summaries remain predecessor-only until
WP11 and DB02.

**Acceptance checks.**

**Behavioral.**

- `just rustc-provider-native-check` — new.
- `just provider-native-arrow-conformance-check rustc`.
- `just provider-protocol-check`.

**Structural.**

- `just rustc-exact-api-check` — new.
- `just rustc-public-private-authority-check` — new.
- `just exact-provider-api-check rustc`.

**Negative / zero-state.**

- `just rustc-summary-payload-zero-state-check` — new, scoped to the target route.

**Operational.**

- `just extractor-ci-fast`.
- `just extractor-identity`.

Oracle catalog:

- Executable oracle: `just rustc-provider-native-check`
- Executable oracle: `just rustc-public-private-authority-check`
- Executable oracle: `just rustc-summary-payload-zero-state-check`
- Executable oracle: `just extractor-ci-fast`

**Edit-Local Gates.**

Focused extractor/API/IPC tests; `just extractor-fmt`; `just extractor-check`.

**Packet-Local Gates.**

`just extractor-ci-fast`; `just rustc-provider-native-check`;
`just rustc-public-private-authority-check`;
`just provider-protocol-check`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen the rustc boundary if the exact nightly API cannot expose an accepted fact family or a
stable identity input. Treat dated-toolchain unavailability as a blocker; never silently build
against a different nightly.

**Rollback or Recovery.**

Keep the old extractor contract independently versioned until cutover. Reject mismatched
toolchain/schema handshakes and publish explicit capability gaps; never coerce new rows into the
summary payload.

**Design-Bearing Contracts and Exemplars.**

MIR-local indices are observation coordinates. Only application-owned stable semantic inputs may
participate in canonical entity identity.

### WP11 — Integrate exact providers into normalization, authority, and capability plans

**Outcome.**

Every accepted Tree-sitter, Ruff, Pyrefly, and rustc family enters an immutable epoch as typed
provider-native Arrow relations and is transformed by model-compiled normalization, authority,
conflict, unknown, provenance, and capability plans. Advertised support is derived from requested
and completed coverage plus proof; an empty relation never implies that a provider proved none.

**Dependencies.**

WP07, WP08, WP09, and WP10. The four exact provider contracts must be independently owned before
their corresponding route can be accepted.

**Target invariants.**

I-20, I-23, I-24, I-27, I-29, I-30, and I-33. Advances P2–P4, P9–P10, P14–P16, P20,
P24, P27, P29, and P36; maintains raw/normalized coexistence, provider authority, and explicit
unknowns.

**Design and library references.**

Design D-23, D-24, D-28, LD-17–LD-21, LD-25, §§6.4–6.5; exact code-fact and DataFusion 55
references declared in section 2.

**Change surface.**

**Preflight query.**

```sh
ast-grep run --lang rust --pattern 'impl ProviderAdapter for $T { $$$BODY }' --inspect summary src
ast-grep run --lang rust --pattern 'impl SemanticProviderDriver for $T { $$$BODY }' --inspect summary src
rg --hidden -n 'ProviderRuntimeDispatch|SemanticProviderAdapterRegistry|replace_owner_rows|provider_raw_kind|supported|partial|unknown|normaliz|authority|provenance' src contracts tests tooling/ci rules -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/provider_runtime*`, `src/provider_types.rs`, `src/provider_sandbox.rs`,
`src/core_facts.rs`, `src/fact_ingest.rs`, `src/source_syntax.rs`,
`src/python_semantic.rs`, `src/contracts/{catalog,registry_models,models}.rs`, capability and
provenance schemas, provider fixtures, and direct `replace_owner_rows` consumers own the current
integration path.

**Required changes.**

- Version an independently authored `ProviderBoundaryContract` for each exact API family. It
  enumerates requested relations, intentionally unavailable fields, remainder semantics,
  coordinates, source/context pins, diagnostics, and completeness before provider output is
  accepted.
- Register installed provider schemas and handlers from the exact adapters, then derive
  provider/runtime capability rows from observed contract coverage and proof. Remove boolean or
  hard-coded capability claims from the target route.
- Compile normalization, canonical identity, per-family authority, conflict, explicit-unknown,
  provenance, and coverage plans from model rows. Keep native relations queryable beside canonical
  facts and retain conflicting evidence. Reject provider-native provenance on Python CFG/
  dataflow/alias/effect/summary and application-derived Rust facts; those flow only from
  WP23–WP25 producer rows.
- Implement every target `TableProvider::scan_with_args(ScanArgs)` path so projections, filters,
  limits, any supplied statistics requests, and ordinary statistics are accepted or honestly
  reported. Forward the request vocabulary rather than dropping it, but do not create a nonempty
  producer or claim a query-aware consumer in this release. `provider-statistics-contract-check`
  must fail any inert feature claim while proving ordinary exact/inexact/unavailable statistics.
- Route exact provider batches through the epoch builder, never through the old cold payload,
  generated kind registry, or procedural owner-row projection. Add the aggregate
  `exact-provider-fabric-check` recipe here.

**Legacy disposition and decommission.**

Completes the positive replacement for L-30–L-34 and L-55. DB02 later removes procedural
projection, opaque payloads, generated kind arrays, hard-coded registries, and summary DTOs after
their target-route zero-state checks pass.

**Acceptance checks.**

**Behavioral.**

- `just provider-normalization-authority-check` — new; checks raw/canonical coexistence,
  authority conflict, explicit unknown, and remainder semantics.
- `just provider-capability-proof-check` — new; checks requested/completed coverage against
  advertised support.
- `just exact-provider-fabric-check` — new aggregate for all four exact lanes.

**Structural.**

- `just provider-statistics-contract-check` — reshaped here to prove ordinary statistics,
  supplied-request forwarding, explicit unsupported semantics, and absence of an invented
  query-aware producer/consumer claim.
- `just exact-provider-api-check all`.

**Negative / zero-state.**

- `just provider-legacy-json-zero-state-check` — new; rejects opaque semantic JSON and summary
  payload use on every target route.
- `just provider-static-registry-target-zero-state-check` — new.

**Operational.**

- `just provider-protocol-check`.
- `just root-test`.

Oracle catalog:

- Executable oracle: `just provider-normalization-authority-check`
- Executable oracle: `just provider-statistics-contract-check`
- Executable oracle: `just provider-legacy-json-zero-state-check`
- Executable oracle: `just exact-provider-fabric-check`

**Edit-Local Gates.**

Focused normalization/authority/capability tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just exact-provider-fabric-check`; `just provider-statistics-contract-check`;
`just provider-protocol-check`; `just root-clippy`; `just root-test`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen D-23 if an accepted family is unavailable from the pinned API or cannot be represented
without loss in Arrow. Reopen D-24/LD-20 if honest optimizer contracts require a different
provider boundary. Revise the plan if discovered provider consumers make the packet
non-dependency-closed; never hide unavailable facts behind an optimistic capability row.

**Rollback or Recovery.**

The old provider path stays read-only and separately versioned until cutover. Reject a batch
whose contract/schema/source pins do not match the candidate epoch; record a typed gap and rebuild
the candidate rather than falling back inside a query.

**Design-Bearing Contracts and Exemplars.**

```text
requested coverage - completed coverage -> explicit provider gaps
provider-native rows + authority model -> canonical rows + conflicts + provenance
```

### WP12 — Compile bounded graph analyses at the highest DataFusion rung

**Outcome.**

Model-selected graph operations participate in DataFusion planning at the highest valid rung:
native relational or bounded recursive plan, function, planning-time provider, or only then a
custom logical/physical extension for a proved irreducible relational-child algorithm. petgraph
is a transient kernel; canonical IDs, resource accounting, cancellation, statistics, and
deterministic results remain owned by the fabric.

**Dependencies.**

WP06, WP07, WP11, WP23, and WP24.

**Target invariants.**

I-21, I-24, I-27, I-29, I-32, and I-33. Advances P2, P14–P16, P25, P27, P29, and P35;
maintains fact-substrate doctrine by emitting derived facts rather than evaluative judgments.

**Design and library references.**

Design D-24, D-25, D-28, LD-19, LD-21, LD-24, §§6.5 and 7; DataFusion 55 logical-extension
and petgraph 0.8.3 references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/derivation.rs src/domain_conformance.rs src/semantic_query.rs --items exports --view signatures
rg --hidden -n 'petgraph|NodeIndex|GraphNode|deriv|fixed.point|LogicalPlan::Extension|ExtensionPlanner|SAFE_TO_|TEST_IMPACTED|HIGH_RISK|SHOULD_CHANGE' src contracts tests tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/derivation.rs`, `src/domain_conformance.rs`, current petgraph projection DTOs, graph
registries/tests, query rejection logic, DataFusion planner construction, and resource/cancellation
modules own the current graph seam.

**Required changes.**

- Compile and record rung selection per operation. Prefer native projection/filter/join/union/
  window/aggregate and bounded `RecursiveQuery`; then scalar/aggregate/window functions; then a
  planning-time table function/provider; create `LogicalPlan::Extension` only when an independent
  fixture proves the algorithm irreducible at higher rungs. Emit a causal
  `system.extension_selection` row with rejected higher rungs and implementation release.
- For each surviving extension, define `UserDefinedLogicalNodeCore` with relational children,
  canonical external IDs, complete expression visitation/rewrite, output schema, stable equality/
  hash, explain formatting, and `with_exprs_and_inputs` compatibility. Register the matching
  `ExtensionPlanner` in every internal physical-planner construction path.
- Its `ExecutionPlan` must forward the received `PhysicalPlanningContext`, implement expression
  visitation/replacement, `with_new_children`, property recomputation, `reset_state`, child
  statistics requests, `statistics_from_input`, precise/inexact/unavailable statistics, physical
  invariant checks after rewrite, repeated/recursive execution, memory reservation, cancellation,
  input/output bounds, and deterministic Arrow output. Compiler-required stubs are not accepted.
- Materialize only model-selected derived relations. Keep judgmental conclusions rejected and
  represent gaps/limits explicitly.
- Remove petgraph `NodeIndex` and projection DTOs from persisted/public identity on the target
  route. Add the design-named `graph-extension-conformance-check` and the focused
  `graph-resource-contract-check` recipe.

**Legacy disposition and decommission.**

Implements L-41 and the graph portion of L-28. DB02 deletes persisted graph indices, registries,
and bypass projections; petgraph itself remains only if DB06 proves the bounded kernel is still
causally used.

**Acceptance checks.**

**Behavioral.**

- `just graph-extension-conformance-check` — new; compares extension output with independently
  expected facts and proves the selected rung for representative algorithms.
- `just graph-derivation-causality-check` — new; mutating model selection changes the observed
  derived rows.

**Structural.**

- `just graph-extension-planner-registration-check` — new; covers every epoch/session builder.
- `just graph-extension-context-forwarding-check` — new; exercises scalar subqueries and rejects
  a planner that substitutes a default or incompatible `PhysicalPlanningContext`.
- `just graph-resource-contract-check` — new; exercises memory, cancellation, statistics, and
  bounds.
- `just graph-execution-contract-check` — new; proves exact logical/physical rewrite, reset,
  property, statistics, repeated-execution, and invariant obligations.

**Negative / zero-state.**

- `just persisted-petgraph-identity-zero-state-check` — new for the target route.
- `just evaluative-fact-zero-state-check` — new; keeps excluded judgments absent.

**Operational.**

- `just root-test`.
- `just root-check`.

Oracle catalog:

- Executable oracle: `just graph-extension-conformance-check`
- Executable oracle: `just graph-execution-contract-check`
- Executable oracle: `just persisted-petgraph-identity-zero-state-check`
- Executable oracle: `just graph-resource-contract-check`

**Edit-Local Gates.**

Focused logical/physical extension tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just graph-extension-conformance-check`; `just graph-extension-planner-registration-check`;
`just graph-extension-context-forwarding-check`; `just graph-execution-contract-check`;
`just graph-resource-contract-check`;
`just root-clippy`; `just root-test`.

**Integration Milestone.**

M03.

**Replan Triggers.**

Reopen D-25 if an accepted algorithm cannot be made bounded or cannot meet epoch resource and
cancellation contracts. Revise the plan if a family is better expressed entirely in native
DataFusion nodes. Reject a new public judgment layer rather than stretching the fact ontology.

**Rollback or Recovery.**

Disable the candidate epoch containing the failing extension before activation and rebuild under
the prior model migration. A runtime extension failure is typed and scoped to the pinned epoch;
it never delegates to a hidden legacy graph engine.

**Design-Bearing Contracts and Exemplars.**

`relational inputs -> bounded extension node -> Arrow derived-fact relation`; petgraph indices
exist only inside one execution call.

### WP13 — Compile all semantic request forms from request relations

**Outcome.**

All eight released semantic request forms and their composition roles lower from typed request
relations to deterministic, bounded DataFusion logical plans over an epoch catalog. The public
surface remains semantic-first; physical names, SQL, DataFrames, and plan handles never escape.

**Dependencies.**

WP11, WP12, WP22, and WP25, with the generic compilers from WP06. No semantic form may compile
against an accepted fact family until the producer-closure relation is complete.

**Target invariants.**

I-21, I-24, I-28, I-29, I-32, and I-33. Advances P5–P8, P13–P16, P21, P27, P31–P32, and P35;
maintains the released query envelope and fact-not-judgment rejection behavior.

**Design and library references.**

Design D-22, D-24, D-25, D-29, LD-18–LD-21, §§1.1 and 6.5–6.7; QRY public contract and
DataFusion 55 planning reference.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/semantic_query.rs src/query_service.rs src/governed_session.rs --items exports --view signatures
rg --hidden -n 'currently_supported|executor_registered|plan_node_kind|query_bundle_identity|query.form|SessionContext::|DataFrame|sql\(' src contracts tests codefabric-cpg-mcp tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/semantic_query.rs` mixes stable public models with generated form imports and manual
capability/executor crosswalks; `src/query_service.rs`, `src/governed_session.rs`, query bundle
contracts, RPC projections, and conformance fixtures consume them.

**Required changes.**

- Decode each released request into typed request relations with one pinned epoch identity,
  scope/policy context, limits, and composition DAG. Validate cycles, fanout, resource estimates,
  and output schema before planning.
- Compile selection, traversal, path, neighborhood, dependence, evidence, explanation, and
  composition behavior from model rows using native joins, filters, projections, aggregates,
  windows, functions, and the bounded graph extension only where selected.
- Derive form/reference/capability rows from installed model bindings and proof. Delete fixed
  form-to-executor crosswalks and package-bound query identity from the target route.
- Canonicalize externally visible ordering, pagination, truncation, evidence, unknown, and error
  projections. Reshape the existing intent-level `semantic-query-relational-conformance-check`
  to aggregate this compiler evidence.

**Legacy disposition and decommission.**

Provides the replacement for L-39 and the query portions of L-23/L-26/L-42. DB03 later removes
generated query-form tables, fixed crosswalks, bypass planning, and static query bundles while
preserving released request/response schemas under L-43.

**Acceptance checks.**

**Behavioral.**

- `just semantic-query-relational-conformance-check` — reshaped here to exercise every form and
  composition role against typed expectations.
- `just semantic-query-conformance-check`.
- `just query-composition-dag-check` — new; covers cycles, ordering, fanout, truncation, and
  unknowns.

**Structural.**

- `just query-determinism-check` — reshaped here to compare canonical logical plans and results
  under equivalent input orderings.

**Negative / zero-state.**

- `just public-query-port-check` — new; rejects SQL/table/function/DataFrame/plan handles at RPC,
  FastMCP, and public model boundaries while proving all semantic forms remain reachable.
- `just query-static-crosswalk-target-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just provider-protocol-check`.

Oracle catalog:

- Executable oracle: `just semantic-query-relational-conformance-check`
- Executable oracle: `just query-determinism-check`
- Executable oracle: `just public-query-port-check`
- Executable oracle: `just semantic-query-conformance-check`

**Edit-Local Gates.**

Focused query compiler/form tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just semantic-query-relational-conformance-check`; `just semantic-query-conformance-check`;
`just query-composition-dag-check`; `just root-clippy`; `just root-test`.

**Integration Milestone.**

M03.

**Replan Triggers.**

Reopen D-29 if a released form cannot be expressed without exposing physical fabric names or
adding a judgment layer. Revise the plan if form families require a new dependency-closed
compiler packet. An optimizer plan shape change inside equivalent semantics is implementation
adaptation, not a reason to freeze physical plans as authority.

**Rollback or Recovery.**

Before activation, withdraw the candidate epoch and retain the old query service. After cutover,
repair forward by a new model/compiler release; never route one request form back to the old
engine.

**Design-Bearing Contracts and Exemplars.**

```text
public request -> request relations -> authorized logical plan -> bounded Arrow result
```

### WP14 — Construct authorized child catalogs and seal epoch sessions

**Outcome.**

Every admitted request receives a reduced child catalog and session derived from one immutable
epoch and authorization context. The child contains only permitted schemas, tables, views,
functions, logical extensions, variables, metadata, and resource options; internal objects and
unfiltered registries are unreachable.

**Dependencies.**

WP05, WP06, WP12, and WP13.

**Target invariants.**

I-21, I-27, I-28, I-29, and I-32. Advances P5, P13, P16, P21, P25, P31–P32, and P35;
maintains least privilege and one-snapshot admission.

**Design and library references.**

Design D-21, D-22, D-24, D-29, D-35, LD-18–LD-21, §§3.12 and 6.7–6.8; DataFusion 55 catalog,
session-state, function-registry, and planner references.

**Change surface.**

**Preflight query.**

```sh
ast-grep run --lang rust --pattern 'SessionStateBuilder::$M($$$A)' --inspect summary src tests
rg --hidden -n 'SessionContext::|SessionStateBuilder::|new_from_existing|register_(table|catalog|udf|udaf|udwf|udtf)|deregister_|FunctionRegistry|ExtensionPlanner' src tests -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Production state construction exists in `src/governed_session.rs` and
`src/fabric/serving.rs`; raw `SessionContext::new*` paths also exist in
`src/ontology_gate.rs` and `src/core_facts.rs`. Policy, catalog, query, function, metadata,
security, and test-only session builders are mixed consumers.

**Required changes.**

- Make `FabricEpoch` the only production owner of sealed internal catalog/session state and
  expose one child-session factory accepting authorization, policy, resource, and query pins.
- Construct a new reduced `MemoryCatalogProviderList`/catalog/schema graph and explicitly install
  only allowed functions, planners, variables, runtime options, and metadata. Install a fresh
  allowlisted object-store registry; sharing memory/spill resources must not retain the parent
  registry.
  Do not blindly use `SessionStateBuilder::new_from_existing`, because it can clone authorities
  and registries that the child must not possess.
- Compile each public view from model-level expressions inside the child. A precompiled
  `ViewTable` is permitted only when a recursive verifier proves every bound table-provider Arc,
  nested/subquery view, UDF/UDAF/UDWF/table function, extension node, variable, and object-store
  URL belongs to the `AccessScopeId`; unknown bound nodes fail before physical planning.
- Classify every session-construction site as sealed production, authorized child, isolated
  builder/proof, or test-only. Governance rejects raw production constructors and post-seal
  registration.
- Compile row/column/table/function/operation policy into catalog construction and plans. Apply
  metadata/diagnostic redaction before public projection and test noninterference.
- Add the design-named `access-catalog-isolation-check` and the focused
  `epoch-session-seal-check` and `authorized-view-bound-authority-check` recipes here.

**Legacy disposition and decommission.**

Completes the security replacement for L-35 and L-39 and constrains L-40. DB03 removes mutable
session registration, ontology/query bypass contexts, and unauthorized production constructors.

**Acceptance checks.**

**Behavioral.**

- `just access-catalog-isolation-check` — new; proves allowed visibility and denied absence
  across tables, functions, extensions, variables, and metadata.
- `just policy-plan-noninterference-check` — new; unauthorized rows and metadata cannot affect
  results, statistics, errors, or timing-class observations beyond the accepted envelope.

**Structural.**

- `just epoch-session-seal-check` — new; covers every production builder and post-seal mutation.
- `just function-extension-registry-scope-check` — new.
- `just authorized-view-bound-authority-check` — new; injects hidden pre-bound providers,
  functions, extensions, variables, nested views, and object-store URLs.

**Negative / zero-state.**

- `just raw-production-session-construction-zero-state-check` — new; permits only classified
  test/builder contexts.
- `just internal-catalog-public-leak-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just governance-scan`.

Oracle catalog:

- Executable oracle: `just access-catalog-isolation-check`
- Executable oracle: `just authorized-view-bound-authority-check`
- Executable oracle: `just internal-catalog-public-leak-zero-state-check`
- Executable oracle: `just policy-plan-noninterference-check`

**Edit-Local Gates.**

Focused policy/catalog/session tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just access-catalog-isolation-check`; `just epoch-session-seal-check`;
`just authorized-view-bound-authority-check`;
`just policy-plan-noninterference-check`; `just root-clippy`; `just root-test`.

**Integration Milestone.**

M03.

**Replan Triggers.**

Reopen D-21/D-22 if DataFusion 55 cannot construct a fully reduced child without sharing a
mutable authority. Revise the plan if policy compilation needs its own coherent packet. A need
to expose internal catalog handles or to filter only after execution is a design blocker.

**Rollback or Recovery.**

Reject child construction closed and retain the pinned epoch for diagnostics. Never broaden a
catalog on error. Rebuild a corrected epoch/policy release; do not mutate an admitted session.

**Design-Bearing Contracts and Exemplars.**

`FabricEpoch + AuthorizationContext -> reduced catalog + sealed child SessionState`.

### WP15 — Deliver dynamic catalog results through the daemon and FastMCP

**Outcome.**

The Rust daemon serves semantic queries, references, schemas, capabilities, proof status, and
bounded result resources from the admitted epoch. FastMCP remains a strict presentation adapter
over gRPC control plus Arrow result streams and packages no independent model, registry,
fingerprint, or query-form authority.

**Dependencies.**

WP04, WP11, WP13, and WP14.

**Target invariants.**

I-21, I-22, I-27, I-28, and I-32. Advances P5–P8, P12, P16, P21, P31–P32, and P35;
maintains the one-daemon/one-STDIO-adapter topology and released public contracts.

**Design and library references.**

Design D-21, D-22, D-29, LD-17, LD-18, LD-26, §§1.1, 3.12, and 6.7–6.8; pinned FastMCP,
Pydantic, gRPC, Protobuf, and Arrow IPC references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/daemon.rs src/query_service.rs src/rpc.rs codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py --items exports --view signatures
ast-grep run --lang python --pattern 'from codefabric_cpg_mcp.contracts.$M import $$$A' --inspect summary codefabric-cpg-mcp/src codefabric-cpg-mcp/tests
rg --hidden -n 'REGISTRY_IDS|fingerprint|package.data|model_artifact_index|query_forms|query_bundle_identity|reference|capabilit|result.resource' codefabric-cpg-mcp src contracts tooling scripts justfile .github -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/daemon.rs` reads the packaged model artifact index/toolchain identity;
`src/query_service.rs` embeds a static query bundle. RPC/result-resource modules, the adapter
server/client/settings/contracts/package data, `contracts/adapter/adapter-model-ir.json`,
generated stubs, descriptor/toolchain caches, build scripts, and adapter tests own this boundary.

**Required changes.**

- Serve all reference, schema, capability, proof, and status content as filtered projections of
  the admitted epoch, with public versioning/redaction and explicit unknowns. Remove target-route
  reads of packaged model/query bundles.
- Keep Protobuf for control, leases, errors, and compatibility metadata; use negotiated bounded
  Arrow IPC streams/resources for semantic tabular results. Preserve public JSON projections
  where released schemas require them.
- Keep Pydantic strict boundary validation and FastMCP STDIO protocol purity. Python may decode
  or present results but may not own Arrow/DataFusion transformations or mutable CPG state.
- Decide generated Protobuf cache retention from proved source-distribution/wheel builds in
  constrained foreign environments. The `.proto` and compatibility policy remain authoritative;
  any cache is derivable and regenerate/compare checked.
- Add `dynamic-reference-delivery-check`, `adapter-package-authority-zero-state-check`, and the
  M03 aggregate `semantic-delivery-vertical-check` recipes here.

**Legacy disposition and decommission.**

Implements L-25, L-26, L-42, and the public projection portion of L-43. DB03 removes packaged
semantic state, obsolete generated caches, and daemon static-bundle reads after package and
interop proof; released wire sources remain.

**Acceptance checks.**

**Behavioral.**

- `just semantic-delivery-vertical-check` — new; runs all request forms through FastMCP, real
  gRPC stubs, the daemon, authorized catalog, and result-resource lifecycle.
- `just dynamic-reference-delivery-check` — new; changes epoch rows and observes reference,
  schema, capability, and status changes.
- `just adapter-test`.

**Structural.**

- `just adapter-domain-boundary-check` — new; rejects Arrow/DataFusion/domain state ownership in
  Python.
- `just proto-contract-check`.

**Negative / zero-state.**

- `just adapter-package-authority-zero-state-check` — new; inspects wheel/sdist and import
  closure for semantic registries, fingerprints, bundles, or query tables.
- `just daemon-static-bundle-target-zero-state-check` — new.

**Operational.**

- `just adapter-ci-fast`.
- `just provider-protocol-check`.
- `just artifacts-check`.

Oracle catalog:

- Executable oracle: `just semantic-delivery-vertical-check`
- Executable oracle: `just adapter-domain-boundary-check`
- Executable oracle: `just adapter-package-authority-zero-state-check`
- Executable oracle: `just adapter-ci-fast`

**Edit-Local Gates.**

Focused Rust RPC/resource tests and adapter Ruff/Pyrefly/pytest tests;
`just proto-contract-check`.

**Packet-Local Gates.**

`just semantic-delivery-vertical-check`; `just adapter-ci-fast`;
`just provider-protocol-check`; `just artifacts-check`.

**Integration Milestone.**

M03.

**Replan Triggers.**

Reopen LD-26 if bounded Arrow delivery cannot coexist with released control messages. Revise the
plan if a foreign package environment demonstrably requires a committed generated cache. Reopen
D-29 if Python must acquire independent semantic state; do not solve packaging inconvenience by
duplicating authority.

**Rollback or Recovery.**

Before cutover, select the legacy daemon/adapter pair as one deployment. After cutover, roll
forward with a compatible daemon/adapter protocol version; never let a new adapter fall back to
packaged legacy semantics.

**Design-Bearing Contracts and Exemplars.**

Control messages identify schema/epoch/result resources; semantic rows travel as Arrow IPC and
remain owned by the Rust epoch.

### WP16 — Route every durable mutation through `FabricCommand`

**Outcome.**

One daemon actor admits every model, fact, publication, activation, maintenance, and administrative
mutation as an authorized idempotent `FabricCommand`. An OS lease plus durable writer generation
fences the single local writer; SQLite records temporal command/queue/lease progress but is never
semantic current-state authority.

**Dependencies.**

WP02, WP05, and WP07. This packet may proceed after M01 in parallel with exact-provider/query
work, but no later durable path may bypass it.

**Target invariants.**

I-20, I-21, I-25, I-26, I-27, and I-31. Advances P3, P11, P18–P19, P22, P33–P34, and P36;
maintains one central daemon owner and crash-reconcilable temporal coordination.

**Design and library references.**

Design D-21, D-26, D-27, D-28, LD-22–LD-23, §§3.12 and 6.6; delta-rs, rusqlite, rustix,
gix, and lifecycle references.

**Change surface.**

**Preflight query.**

```sh
ast-grep run --lang rust --pattern '$R.replace_owner_rows($$$A)' --inspect summary src tests
rg --hidden -n 'replace_owner_rows|remove_owners|commit_ordinary_fact_snapshot|persist_proved_ontology_candidate|activate_ontology_candidate|execute_batch|transaction|DeltaOps|with_commit_properties' src tests -g '!.git/**' -g '!docs/library_ref/**'
ast-grep outline src/operational_store.rs src/coordinator.rs src/daemon.rs src/fabric/mutation.rs src/fabric/publication.rs --items exports --view signatures
```

**Known touch:** (verified this session)

Direct mutation routes exist in core-fact and serving publication, snapshot/ontology activation,
fabric replacement, operational-store methods, coordinator/lifecycle administration, and tests.
`src/operational_store.rs` mixes valid temporal state with ontology/current semantic authority.

**Required changes.**

- Define an exhaustive typed `FabricCommand` envelope with operation ID, expected predecessor,
  authorization, workspace, writer generation, compiler/model/source/provider pins, resource
  envelope, and command-specific payload/reference. Admission validates before side effects.
- Implement one actor/reducer that owns staging, retry classification, cancellation boundaries,
  and terminal command results. Duplicate operation IDs are idempotent; mismatched duplicates are
  conflicts.
- Acquire an OS-backed workspace lease and monotonically higher durable writer generation before
  any target mutation. Fence stale generations at every durable boundary and on recovery.
- Handwrite/version SQLite migrations for queues, retries, leases, command progress, and
  reconciliation. Remove target semantic-current, ontology-package, and activation-pointer
  authority from SQLite.
- Inventory and route production, administrative, importer, maintenance, and test mutation paths
  through the command port before permitting deletion. Add the design-named
  `fabric-single-mutation-path-check`, `single-writer-fence-check`,
  `fabric-transaction-contract-check`, and `temporal-store-boundary-check` recipes here. Add a
  focused `fabric-command-miri-check` recipe for the new actor, lease, and generation primitives.

**Legacy disposition and decommission.**

Provides the replacement for L-38 and L-40 and constrains L-35/L-37/L-54. DB03 removes direct
mutation/activation methods and semantic-current SQLite tables only after target publication and
cutover are proven.

**Acceptance checks.**

**Behavioral.**

- `just fabric-single-mutation-path-check` — new; exercises every admitted command class,
  idempotency, authorization, and conflicting duplicates.
- `just single-writer-fence-check` — new; proves duplicate or stale actors cannot mutate any durable
  target.
- `just fabric-transaction-contract-check` — new; validates operation selection, predecessor,
  idempotency, and transaction metadata for every command variant.
- `just temporal-store-boundary-check` — new; reconstructs temporal state and proves it cannot
  select semantic current.

**Structural.**

- `just mutation-ingress-coverage-check` — new; derives all durable writers and requires a
  `FabricCommand` edge or explicit non-production classification.

**Negative / zero-state.**

- `just target-mutation-bypass-zero-state-check` — new, including tests and administration.
- `just sqlite-semantic-authority-target-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just fabric-command-miri-check` — new focused Miri recipe for the actor/fence primitives.

Oracle catalog:

- Executable oracle: `just fabric-single-mutation-path-check`
- Executable oracle: `just mutation-ingress-coverage-check`
- Executable oracle: `just target-mutation-bypass-zero-state-check`
- Executable oracle: `just fabric-command-miri-check`

**Edit-Local Gates.**

Focused command/reducer/SQLite tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just fabric-single-mutation-path-check`; `just single-writer-fence-check`;
`just fabric-transaction-contract-check`; `just temporal-store-boundary-check`;
`just fabric-command-miri-check`;
`just root-clippy`; `just root-test`.

**Integration Milestone.**

M04.

**Replan Triggers.**

Reopen D-26/LD-23 if one local writer cannot meet the operational requirement or an external
multi-host writer is required. Revise the packet if the write census exposes a dependency-closed
subsystem that must migrate separately. Never accept a test-only or admin-only bypass.

**Rollback or Recovery.**

Before target mutation, stop the actor and release/prove the lease. Unknown outcomes remain
pending until WP18 reconciliation can read durable markers; never retry by guessing or decrement a
writer generation.

**Design-Bearing Contracts and Exemplars.**

```text
authorize -> admit FabricCommand(op_id, expected_predecessor, writer_generation)
          -> single actor -> durable operation markers -> terminal result
```

### WP17 — Persist exact Delta relations and optimizer-visible epoch overlays

**Outcome.**

The target layout writes new typed relations to isolated Delta tables/versions, reconstructs exact
snapshot providers, and exposes effective epoch views through optimizer-visible anti-join/union/
window plans over base plus immutable Arrow segments. Retention protects every referenced version,
compiler release, expectation, result, and rollback lease.

**Dependencies.**

WP11, WP12, WP14, WP16, and WP27, using WP05's catalog foundation. WP14 first establishes the sealed
catalog/session interface in the shared serving module; this packet then wires exact-version
providers and views through that interface rather than editing an unordered shared phase.

**Target invariants.**

I-21, I-22, I-24–I-27, I-29, and I-31. Advances P3, P7, P11–P12, P14–P20, P22, P25,
P33–P34, and P36; maintains exact-version MVCC and atomic present state.

**Design and library references.**

Design D-21, D-22, D-26, D-27, D-31, LD-17–LD-22, §§3.12–3.13 and 6.6; exact Delta revision,
DataFusion 55 provider/view, Arrow memory, Parquet, and object-store references.

**Change surface.**

**Preflight query.**

```sh
ast-grep run --lang rust --pattern 'impl TableProvider for $T { $$$BODY }' --inspect summary src
ast-grep outline src/fabric.rs src/fabric/overlay.rs src/fabric/publication.rs src/fabric/snapshot_catalog.rs src/fabric/serving.rs --items exports --view signatures
rg --hidden -n 'DeltaOps|DeltaTableProvider|load_version|version|OverlayEffectiveProvider|OverlayIdentityProvider|concatenate|take|ViewTable|vacuum|retention' src tests contracts -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/fabric.rs`, `src/fabric/{mutation,publication,snapshot_catalog,serving,overlay}.rs`,
snapshot/runtime code, result-resource leases, object-store configuration, and four current
production `TableProvider` implementations/wrappers own this surface.

**Required changes.**

- Define the new physical relation/partition contract independently of legacy table layouts.
  Write through the pinned `WriteBuilder` with the exact epoch `SessionState`,
  `SessionFallbackPolicy::RequireSessionState`, schema, operation ID, writer generation,
  application transaction marker, and `CommitProperties::with_max_retries(0)`. Require that
  no selected write, DML, or compaction path enables delta-rs internal retries: a
  conflict is a typed unknown outcome that returns to `FabricCommand` reconciliation, which
  reads the application marker and committed version before deciding whether a new attempt is
  legal. Explicitly forbid the pinned retrying `OptimizeBuilder` and any hidden DML/optimize
  helper on the command-owned route; controlled write primitives are the only compaction seam.
  Reject an incompatible or missing session rather than silently constructing a fallback.
  Record exact committed versions; never discover latest during epoch construction.
- Implement exactly two mutually exclusive load recipes: (a) a previously loaded and validated
  snapshot plus session, with no table-version selector; or (b) a log store plus exact
  `with_table_version` plus session, with no supplied snapshot. Record and compare the observed
  snapshot root/version to the epoch pin before registration. Never chain snapshot and version,
  because the pinned builder ignores `table_version` when a snapshot exists. Use the pinned
  kernel-backed provider/`DeltaScanExec` path and model-generated `ViewTable` definitions. The
  removed legacy `DeltaTableProvider` type and deprecated physical codec are forbidden target
  patterns. Effective state uses native anti-join/union/window plans over base and immutable
  segments; avoid bespoke row conversion, concatenate/take consolidation, and hidden provider
  semantics.
- Apply WP27's `SchemaContract` to physical writes and every scan/plan/stream/batch boundary,
  including fixed-width ID restoration, qualified `DFSchema`, projection/filter/statistics index
  remapping, nested/nullability/metadata, empty streams, column mapping, and deletion vectors.
- Forward `ScanArgs` and honest statistics through irreducible storage providers. Prove
  projection/filter/limit behavior and optimizer visibility on production paths.
- Implement compaction as a `FabricCommand` that proves equivalence, writes new base versions,
  and activates a successor epoch. Vacuum derives protected versions/segments from activation,
  query/result, rollback, expectation, and compiler-release leases.
- Add `delta-exact-version-reconstruction-check`, `overlay-view-equivalence-check`, and
  `retention-lease-safety-check` recipes here.

**Legacy disposition and decommission.**

Implements L-35–L-37 and the physical portion of L-40. DB03 removes custom overlay/consolidation,
mutable snapshot manifests, and legacy table mutation after exact-version reconstruction and
retained-old-data compatibility are accepted.

**Acceptance checks.**

**Behavioral.**

- `just delta-exact-version-reconstruction-check` — new; reconstructs the same rows from exact
  pins after process restart.
- `just overlay-view-equivalence-check` — new; compares native views across inserts,
  replacements, deletions, conflicts, compaction, and randomized order.
- `just retained-epoch-compatibility-check` — new; reconstructs retained old data under its
  pinned compiler release or emits the accepted typed incompatibility.
- `just fabric-transaction-contract-check` — extended here with incompatible-session rejection,
  forced Delta conflicts, application-marker readback, and proof that delta-rs performs no hidden
  retries before command-owned reconciliation.

**Structural.**

- `just delta-provider-contract-check` — new; covers `ScanArgs`, statistics, pruning, and
  exact-version binding plus WP27 schema adaptation.
- `just relational-schema-lifecycle-check` — rerun against the real Delta route.
- `just retention-lease-safety-check` — new.

**Negative / zero-state.**

- `just custom-overlay-target-zero-state-check` — new.
- `just latest-version-discovery-target-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just stable-graph-check`.

Oracle catalog:

- Executable oracle: `just delta-exact-version-reconstruction-check`
- Executable oracle: `just delta-provider-contract-check`
- Executable oracle: `just custom-overlay-target-zero-state-check`
- Executable oracle: `just retained-epoch-compatibility-check`

**Edit-Local Gates.**

Focused Delta/provider/view tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just delta-exact-version-reconstruction-check`; `just overlay-view-equivalence-check`;
`just retained-epoch-compatibility-check`; `just fabric-transaction-contract-check`;
`just relational-schema-lifecycle-check`; `just delta-provider-contract-check`;
`just root-clippy`; `just root-test`.

**Integration Milestone.**

M04.

**Replan Triggers.**

Reopen D-27/LD-22 if the pinned delta-rs revision cannot bind exact snapshots, transaction
markers, controlled zero-retry writes, or `RequireSessionState` writes as required. Revise physical
partitions only within the accepted logical contract. A requirement to reinterpret legacy tables
in place reopens the design.

**Rollback or Recovery.**

Before new mutation, discard isolated candidate tables/versions only through retention-aware
administration. After target mutation, build and activate a corrective forward epoch. Unknown
writes reconcile by operation/transaction marker and readback before retry.

**Design-Bearing Contracts and Exemplars.**

`base exact version + immutable overlay segments -> model-generated effective ViewTable`.

### WP18 — Prove, activate, publish, and recover one immutable `FabricEpoch`

**Outcome.**

One activation event atomically names the exact model/source/provider/table/policy/proof/compiler
set. The single writer stages, proves, seals, closes admissions, revalidates its predecessor and
fence, commits and reads back selection, swaps one `Arc<FabricEpoch>`, reconciles its cache,
reopens admission, and acknowledges. Crash recovery derives the unique valid head and reconciles
every unknown outcome.

**Dependencies.**

WP15, WP16, WP17, and WP22. Public query/result delivery, the exclusive command path, accepted
activation expectations, and exact durable relations must all exist before activation/pinning
can be proved dependency-closed.

**Target invariants.**

I-20, I-21, I-25–I-27, I-29, and I-31. Advances P3, P9–P11, P17–P20, P22, P27, P29,
P33–P34, and P36; maintains atomic present state and no mixed generations.

**Design and library references.**

Design D-21, D-26–D-28, LD-18, LD-20, LD-22–LD-23, §§3.12 and 6.6; exact Delta,
DataFusion, Arrow, SQLite, and OS-lease references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/ontology_activation.rs src/snapshot_runtime.rs src/daemon.rs src/fabric/publication.rs src/operational_store.rs --items exports --view signatures
rg --hidden -n 'active_ontology|active_snapshot|current_ontology_pointer|serving_snapshot_manifest|ArcSwap|activate|readback|admission|operation_id|transaction' src tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Ontology candidate/activation, snapshot/runtime manifests, serving publication, daemon head
selection, `ArcSwap`, query/result leases, SQLite current-pointer tables, and Delta publication
currently split ownership of this transition.

**Required changes.**

- Define the immutable activation relation/event and canonical head-selection rules. Each event
  contains exact predecessor, writer generation, operation selection record, transaction contract,
  complete epoch pins, proof receipt, compatibility/retention class, and commit metadata.
- Build `FabricEpoch` only through a builder that validates schema/contract closure, exact
  versions, source/provider/model/compiler pins, independent expectations, policy, resource
  runtime, and proof. Seal before admission.
- Implement the ordered publish protocol: stage data; execute proof; build and seal the candidate;
  close new admissions and establish the barrier; revalidate the predecessor and writer fence;
  append and read back the exact activation event/versions; swap one epoch; reconcile the
  temporal cache; reopen admission; acknowledge. Existing leases drain on their already pinned
  predecessor epoch. No newly admitted query can observe a predecessor after durable selection.
- Derive the unique current head from the activation chain on every recovery. Reconcile crashes
  before/after every durable write, event commit, readback, barrier, swap, acknowledgment,
  compaction, and lease transition by operation ID and transaction markers.
- Add the design-named `fabric-activation-recovery-check`, `fabric-control-recovery-check`, and
  `fabric-epoch-pinning-check`, plus focused `activation-chain-validity-check` and
  `activation-fault-matrix-check` recipes here. Add a focused `fabric-epoch-miri-check` for the
  admission, lease, and epoch-swap concurrency primitives.

**Legacy disposition and decommission.**

Completes the target replacement for L-27, L-35, L-37–L-38, and L-40. DB03 removes mutable
snapshot/current pointers, package activation, and direct swap routes after cutover.

**Acceptance checks.**

**Behavioral.**

- `just fabric-activation-recovery-check` — new; proves exact pins, activation/readback, unique
  head selection, and recovery.
- `just fabric-epoch-pinning-check` — new; holds leases across activation and proves every row,
  status, stream, result, and checksum remains attributable to one epoch.
- `just fabric-control-recovery-check` — new; deletes reconstructible caches and reconciles
  durable command/activation state without guessing.
- `just activation-fault-matrix-check` — new; injects every named crash point plus concurrent
  admissions immediately before and after closure, selection, readback, swap, cache
  reconciliation, reopen, and acknowledgment, then reconciles to one valid outcome.
- `just activation-chain-validity-check` — new; covers forks, stale generation, missing proof,
  incompatible compiler release, and retained rollback heads.

**Structural.**

- `just activation-route-exclusivity-check` — new; derives every activation/swap writer.

**Negative / zero-state.**

- `just mutable-current-pointer-target-zero-state-check` — new.
- `just partial-epoch-visibility-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just fabric-epoch-miri-check` — new focused Miri recipe for swap/admission/lease primitives.

Oracle catalog:

- Executable oracle: `just fabric-activation-recovery-check`
- Executable oracle: `just activation-fault-matrix-check`
- Executable oracle: `just partial-epoch-visibility-zero-state-check`
- Executable oracle: `just fabric-control-recovery-check`

**Edit-Local Gates.**

Focused builder/activation/recovery tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just fabric-activation-recovery-check`; `just fabric-epoch-pinning-check`;
`just fabric-control-recovery-check`; `just activation-fault-matrix-check`;
`just activation-chain-validity-check`; `just fabric-epoch-miri-check`;
`just root-clippy`; `just root-test`.

**Integration Milestone.**

M04.

**Replan Triggers.**

Reopen D-26 if a durable state cannot be made idempotent and predecessor-checked, or if publication
needs more than one writer authority. Revise the plan if crash points expose another durable
phase. Never accept `ArcSwap` or SQLite as semantic activation history.

**Rollback or Recovery.**

Before cutover, discard the candidate deployment while retaining isolated events/data per policy.
After new mutation, select a compatible prior epoch or build a corrective forward epoch through
`FabricCommand`; never reactivate the legacy writer.

**Design-Bearing Contracts and Exemplars.**

```text
stage -> prove/seal -> close admission -> revalidate predecessor/fence
      -> append/read back activation -> ArcSwap -> reconcile cache -> reopen -> acknowledge
```

### WP19 — Integrate lifecycle, resource governance, and clean reconstruction

**Outcome.**

Source changes flow through authoritative gix/safe-filesystem reads, bounded notify-driven
invalidation, exact provider updates, command publication, and epoch activation. Query/update
work shares one governed DataFusion resource runtime with cancellation, spill, fairness, result
leases, and backpressure. Clean rebuild and incremental reconstruction produce equivalent logical
facts without legacy inputs.

**Dependencies.**

WP12, WP15, and WP18.

**Target invariants.**

I-21–I-27 and I-29–I-34. Advances P3–P5, P11–P13, P16, P18–P25, P31,
P33–P36; maintains safe source truth, present-state semantics, and bounded operations.

**Design and library references.**

Design D-21–D-29, LD-17–LD-26, §§1.1, 3.12–3.13, and 6.6–6.8; gix,
notify-debouncer-full, object-store, DataFusion memory/spill/cancellation, Delta, and serving
references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/coordinator.rs src/lifecycle.rs src/continuous.rs src/repository_state.rs src/source_image.rs src/daemon.rs --items exports --view signatures
rg --hidden -n 'gix::|notify|debounc|rescan|dirty|invalidate|MemoryPool|spill|cancel|backpressure|result.*lease|timeout|fair' src tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Coordinator/lifecycle/continuous update modules, repository/source image/inventory, gix/notify
adapters, provider supervisors, DataFusion runtime configuration, cancellation/security,
publication, result-resource stores, and daemon scheduling own this vertical.

**Required changes.**

- Preserve descriptor-relative byte-authoritative source reads and use gix only for repository
  acceleration/metadata. Convert debounce loss/rescan into explicit dirty coverage and a bounded
  rebuild, never an empty update.
- Lower invalidation/update work into `FabricCommand`s pinned to source generations and provider
  coverage. For Pyrefly, include semantic-environment changes, actual affected modules, and
  conservative reverse-importer refresh. For Rust, admit only WP26 launcher receipts and make
  untrusted/trusted-local posture visible in capability/provenance. Coalesce safely, cancel
  superseded work, and activate only a fully proved epoch.
- Give each epoch one governed runtime with memory-pool reservations, spill paths, CPU/concurrency
  budgets, cancellation tokens, time/row/byte limits, stream backpressure, and fair update/query
  admission. Result resources retain the epoch and physical data leases until terminal release.
- Prove cold reconstruction from migrations, exact compiler release, source/provider inputs,
  Delta versions, activation events, and independent expectations with generated legacy trees
  physically unavailable. Prove incremental equivalence and explicit degradation.
- Add the M04 aggregate `durable-epoch-reconstruction-check`, plus resource and lifecycle recipes,
  here.

**Legacy disposition and decommission.**

Implements the retained/reshaped portion of L-40 and supplies the reconstruction exit for
L-20–L-38, L-44, and L-54. DB03 removes legacy lifecycle/publication paths only after this vertical
passes.

**Acceptance checks.**

**Behavioral.**

- `just durable-epoch-reconstruction-check` — new; covers clean rebuild, restart, incremental
  equivalence, provider degradation, and FastMCP query delivery.
- `just lifecycle-invalidation-conformance-check` — new; covers rename, loss/rescan, coalescing,
  source replacement, and explicit unknown.
- `just resource-governance-check` — new; covers memory, spill, cancellation, fairness,
  backpressure, timeout, and result leases.

**Structural.**

- `just source-authority-boundary-check` — new; enforces safe byte reads and bounded gix role.
- `just epoch-resource-runtime-ownership-check` — new.

**Negative / zero-state.**

- `just clean-rebuild-legacy-input-zero-state-check` — new; runs with frozen generated/model
  input trees unavailable.
- `just stale-provider-current-zero-state-check` — new.

**Operational.**

- `just root-test`.
- `just provider-protocol-check`.
- `just adapter-test`.

Oracle catalog:

- Executable oracle: `just durable-epoch-reconstruction-check`
- Executable oracle: `just source-authority-boundary-check`
- Executable oracle: `just clean-rebuild-legacy-input-zero-state-check`
- Executable oracle: `just resource-governance-check`

**Edit-Local Gates.**

Focused lifecycle/resource/result tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just durable-epoch-reconstruction-check`; `just resource-governance-check`;
`just lifecycle-invalidation-conformance-check`; `just root-clippy`; `just root-test`;
`just adapter-test`.

**Integration Milestone.**

M04.

**Replan Triggers.**

Reopen resource or lifecycle ownership if one epoch cannot bound both update and query work or if
source truth requires a new persistent authority. Revise the plan if clean reconstruction exposes
an undeclared legacy input. Performance tuning within the logical contract is implementation
adaptation; weakening correctness under pressure is not.

**Rollback or Recovery.**

Before target mutation, retain legacy serving while rebuilding the isolated candidate. After
target mutation, cancel affected work, reconcile commands/events, and build a forward epoch.
Watcher loss always enters rescan/rebuild, never silent continuity.

**Design-Bearing Contracts and Exemplars.**

One admitted query/result lease pins both its `Arc<FabricEpoch>` and all referenced durable/
segment retention leases until terminal release.

### WP20 — Re-execute accepted semantic, causal, security, and performance release evidence

**Outcome.**

WP22's independently owned, already accepted corpus is re-executed against the completed target
to prove model meaning, exact provider and derived families, semantic requests, public delivery,
authorization, failure handling, and intentional old/new deltas. This packet cannot author or
revise expectations/comparator semantics; causal interventions, benchmarks, and security probes
produce the candidate release dossier from those frozen inputs.

**Dependencies.**

WP15, WP19, and WP22, which supply completed public/reconstruction paths and preaccepted evidence.
Any requested expectation/comparator change stops this packet, creates a successor evidence
candidate under WP22's ownership rules, and reruns all affected downstream proof.

**Target invariants.**

I-20–I-34. Advances P4, P9–P10, P20–P21, P27, P29–P30, and P36; maintains independent proof,
explicit limitations, and released public behavior.

**Design and library references.**

Design D-28–D-29, §§5.2 Stage 5 and 6.1–6.8, L-44–L-45 and L-49. All exact provider,
DataFusion/Arrow/Delta, and public-boundary references declared in section 2.

**Change surface.**

**Preflight query.**

```sh
rg --files tooling/ci rules rule-tests tests/golden tests/integration contracts/acceptance contracts/governance | sort
rg --hidden -n 'gate_b_|golden_corpus|functional_golden|expected|causal|mutant|benchmark|security|comparison|ignore' src tests contracts tooling scripts justfile .github rules rule-tests -g '!.git/**' -g '!docs/library_ref/**'
git worktree list --porcelain
```

**Known touch:** (verified this session)

`src/{gate_b_candidate,gate_b_release,golden_corpus,functional_golden,functional_scenario}.rs`,
`tooling/gate_b_candidate.rs`, current corpus/acceptance/governance inputs, integration/golden
tests, comparison-ignore machinery, security/resource tests, benchmarks/profilers, recipes, and CI
own the predecessor evidence path.

**Required changes.**

- Load only WP22's accepted typed expectation, hostile/security, activation, public, provider,
  query, causal-control, and limitation relations through the separate proof port. Validate their
  immutable identities and ownership before execution; a production-model/provider/compiler
  output or packet-local edited expectation is a structural failure.
- Add causal interventions that change one model binding, invariant, authority rule, policy row,
  provider remainder, activation predecessor, or expectation and demonstrate the intended
  discriminating failure/result change. Unknown must never satisfy proof.
- Run WP22's exact frozen executable/worktree and decoded comparison contract read-only against
  identical frozen inputs; use independent expectations as the oracle and classify every delta.
  Legacy output is comparison evidence only, and no new comparator is created here.
- Exercise clean rebuild, incremental update, degraded providers, corrupt IPC, crash points,
  concurrency, foreign credentials/workspace IDs, resource exhaustion, metadata leakage, package
  installation, and public four-layer delivery. Record limitations and independently review the
  decoded release dossier.
- Execute WP22's accepted latency, memory, spill, throughput, and update-amplification workload/
  budget definitions before tuning. Add the M05 readiness aggregate and its independent-semantic,
  causal, comparison, security, and performance execution recipes here.

**Legacy disposition and decommission.**

Provides the positive replacement for L-44, L-45, and L-49. DB05 later removes producer-owned
Gate B execution, count/digest acceptance, and static fixtures only after independent evidence is
accepted; immutable released decisions remain history.

**Acceptance checks.**

**Behavioral.**

- `just independent-semantic-oracle-check` — new; validates decoded expectations across model,
  providers, queries, and public delivery.
- `just relational-causal-intervention-check` — new; proves discriminating semantic mutations.
- `just old-new-independent-comparison-check` — new; classifies all deltas without treating old
  output as expected truth.

**Structural.**

- `just expectation-independence-check` — WP22-owned gate rerun against the release execution;
  rejects producer-generated expected rows and production-to-oracle dependency edges.
- `just comparison-engine-isolation-check` — WP22-owned gate rerun against the exact comparator;
  proves the legacy engine remains frozen/read-only.

**Negative / zero-state.**

- `just producer-generated-golden-target-zero-state-check` — new.
- `just public-leakage-negative-check` — new; covers internal names, metadata, credentials,
  denied rows, and foreign workspace/result identities.

**Operational.**

- `just relational-fabric-security-check` — new.
- `just relational-fabric-performance-check` — new, measured against accepted budgets.
- `just package-interop-check` — new.

Oracle catalog:

- Executable oracle: `just independent-semantic-oracle-check`
- Executable oracle: `just expectation-independence-check`
- Executable oracle: `just producer-generated-golden-target-zero-state-check`
- Executable oracle: `just relational-fabric-security-check`

**Edit-Local Gates.**

Focused independent fixture validators and causal tests; decoded dossier review tooling;
`just typos`.

**Packet-Local Gates.**

`just independent-semantic-oracle-check`; `just relational-causal-intervention-check`;
`just old-new-independent-comparison-check`; `just relational-fabric-security-check`;
`just relational-fabric-performance-check`; `just expectation-independence-check`;
`just comparison-engine-isolation-check`; `just package-interop-check`.

**Integration Milestone.**

M05.

**Replan Triggers.**

Reopen the target design when independently justified semantics contradict a target invariant or
public contract. Revise the plan when the comparison requires another coherent provider/query
packet. Performance shortfall may adapt physical layout within the logical contract; an
unbounded fallback, hidden cache authority, or weakened proof requires design reopening.

**Rollback or Recovery.**

This packet is read-only against both engines. A failed oracle blocks cutover and leaves the
legacy runtime authoritative. Never repair a failure by copying legacy output into expected rows
or broadening an ignore registry.

**Design-Bearing Contracts and Exemplars.**

The public release dossier contains request, decoded expected result/evidence/error/status rows,
actual rows, provenance/proof receipt, causal-control result, and explicit limitation rationale.

### WP21 — Execute the durable fenced cutover state machine

**Outcome.**

The workspace advances through the accepted durable state machine from one legacy authority to one
new authority. The legacy writer drains before the new generation is acquired; the new epoch
serves read-only before mutation; `NEW_MUTATING` requires a bridge/external revocation boundary
that the exact frozen binary cannot bypass after process restart or host reboot; no runtime
fallback or dual write remains.

**Dependencies.**

WP20 and its accepted independent release evidence. The predecessor-plan disposition and
successor activation are WP01 entry evidence, not deferred to this cutover packet.

**Target invariants.**

I-20–I-34, especially I-21, I-25–I-27, and I-31. Advances P3, P11, P13, P18–P23, P31,
P33–P36; maintains exactly one mutation and serving authority through every transition.

**Design and library references.**

Design §§5.1–5.3 and 6.6–6.9, D-21, D-26–D-29, D-34, LD-22–LD-23 and LD-26; all L-20–L-55
exit dependencies.

**Change surface.**

**Preflight query.**

```sh
rg --hidden -n 'LEGACY_AUTHORITATIVE|LEGACY_QUIESCED|NEW_BINARY_FENCED_READ_ONLY|NEW_EPOCH_SELECTED|NEW_SERVING_NO_MUTATION|NEW_MUTATING|LEGACY_RETIRED|fallback|writer.generation|active-plan' src contracts docs/plans scripts tooling/ci justfile .github tests -g '!.git/**' -g '!docs/library_ref/**'
rg --hidden -n 'bind|listen|socket|lease|activate|serve|mutat|rollback' src/daemon.rs src/rpc.rs src/coordinator.rs codefabric-cpg-mcp scripts tests -g '!.git/**'
```

**Known touch:** (verified this session)

Daemon process/UDS ownership, writer leases/generations, activation events, epoch selection,
admission/mutation gates, deployment/admin tooling, predecessor active-plan state, rollback
artifacts, adapter routing, CI, and release evidence own cutover.

**Required changes.**

- Persist and predecessor-check every transition:
  `LEGACY_AUTHORITATIVE -> LEGACY_QUIESCED -> NEW_BINARY_FENCED_READ_ONLY ->
  NEW_EPOCH_SELECTED -> NEW_SERVING_NO_MUTATION -> NEW_MUTATING -> LEGACY_RETIRED`.
  Re-entry is idempotent and derives the next safe action from durable evidence.
- Make one external Rust `CutoverController` in the existing stable package the sole writer of a
  `DeploymentTransitionJournal` in the platform-private per-workspace state directory. Its fixed,
  versioned control schema records workspace, transition/operation ID, predecessor transition and
  state, next state, old/new binary release identity, expected/released/acquired writer
  generations, selected epoch and Delta activation-event identity where applicable, proof-receipt
  references, and terminal reconciliation status. This schema is static because it is the
  cross-version crash-recovery wire contract, not semantic model authority and not the authority
  that revokes an older binary.
- Store immutable transition records plus one deployment-head pointer under mode-`0700`
  descriptor-relative directories and mode-`0600` files. Under a controller-only OS lease,
  validate the expected predecessor/head, write and fsync the record, atomically replace and fsync
  the head and containing directory, then read both back before the process side effect is
  acknowledged. A changed head, mismatched record, unsafe owner/mode, or ambiguous write is a
  fail-closed recovery state. Restart reconciliation compares journal/head, running binary,
  writer lease/generation, selected epoch, and named Delta activation event and converges from
  every crash prefix without inventing a transition.
- Permit the daemon to read the deployment journal only for binary-phase and mutation-admission
  fencing. It remains read-only to the daemon and cannot select semantic current: the unique
  serving epoch is still derived from the validated Delta activation chain. `NEW_EPOCH_SELECTED`
  and `NEW_MUTATING` must reference the exact selected epoch, activation event, and writer
  generation, so neither journal nor Delta evidence can be silently paired with another release.
- Before `NEW_MUTATING`, choose and install exactly one D-34 enforcement profile: (a) a bridge
  legacy release checks a monotonic retirement generation at every serving and durable-write
  ingress, or (b) an external platform authority revokes the frozen release's service entrypoint
  and storage/write credential or namespace. Record persistence, owner, restart/reboot recovery,
  rotation/revocation semantics, and a receipt binding workspace, old/new releases, selected
  activation event, and writer generation. The journal records this receipt but cannot substitute
  for it.
- Drain legacy queries/results as policy requires, stop the legacy daemon, and prove its writer
  lease released before starting the new binary. Acquire a strictly higher generation and prove
  the candidate epoch before selection or serving.
- Run public read-only smoke/semantic/security/resource probes at
  `NEW_SERVING_NO_MUTATION`. Permit old-binary rollback only from the design-accepted pre-mutation
  states and only after proving the new lease/generation released.
- Enter `NEW_MUTATING` only after restarting the exact frozen legacy executable and proving it
  cannot bind/serve or perform any durable write through every old ingress. Repeat after target
  crash, controller restart, and host-reboot simulation. Thereafter recover only through
  compatible prior/new corrective epochs on the target command path.
- Remove runtime fallback flags/routes and freeze the legacy engine for decommission only. Add
  `fabric-cutover-state-machine-check`, `cutover-transition-authority-check`,
  `legacy-writer-fence-check`, and the M05 aggregate
  `relational-fabric-cutover-readiness-check` recipes here. The aggregate runs the already closed
  M03 and M04 gates, WP20's independent release evidence, and WP21's transition/fence/authority
  checks; it never invokes M05 or itself. `milestone-aggregate-closure-check M05` enforces that
  acyclic expansion before M05 can be accepted.

**Legacy disposition and decommission.**

Establishes when remaining L-20–L-55 deletions become safe. DB01 already removed the importer and
live migration-input routes after M01. DB02–DB07 perform remaining consumer-first removal after
`LEGACY_RETIRED`; DB08 removes the non-live comparator/archive only after retention expiry.

**Acceptance checks.**

**Behavioral.**

- `just fabric-cutover-state-machine-check` — new; covers every transition, retry,
  rollback-eligible state, and forward-only recovery state.
- `just relational-fabric-cutover-readiness-check` — new; aggregates M03, M04, WP20 independent
  release evidence, and WP21 cutover/fence/authority checks at the selected epoch, with no M05 or
  self edge.
- `just cutover-crash-reconciliation-check` — new; injects failure before/after each transition
  record, head CAS, binary/lease side effect, Delta activation linkage, and readback.

**Structural.**

- `just serving-mutation-authority-exclusivity-check` — new; derives daemon, writer, serving,
  and activation authorities in every state.
- `just cutover-transition-authority-check` — new; proves the controller is the sole journal
  writer, validates schema/predecessor/CAS/fsync/readback/permission behavior, and reconciles every
  durable prefix.
- `just milestone-aggregate-closure-check M05` — new; rejects a milestone aggregate that reaches
  M05, invokes itself, or omits any owned prerequisite evidence.

**Negative / zero-state.**

- `just legacy-writer-fence-check` — new; restarts the exact frozen executable after
  `NEW_MUTATING`, target/controller restart, and reboot simulation and proves it cannot serve or
  mutate through any legacy ingress.
- `just runtime-fallback-zero-state-check` — new.
- `just deployment-transition-semantic-authority-zero-state-check` — new; proves the deployment
  journal cannot write facts, select an epoch, or replace Delta activation-chain derivation.

**Operational.**

- `just semantic-delivery-vertical-check`.
- `just durable-epoch-reconstruction-check`.
- `just relational-fabric-security-check`.

Oracle catalog:

- Executable oracle: `just fabric-cutover-state-machine-check`
- Executable oracle: `just cutover-transition-authority-check`
- Executable oracle: `just legacy-writer-fence-check`
- Executable oracle: `just relational-fabric-cutover-readiness-check`

**Edit-Local Gates.**

Focused state-machine/fence/deployment tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

`just fabric-cutover-state-machine-check`; `just cutover-crash-reconciliation-check`;
`just cutover-transition-authority-check`; `just serving-mutation-authority-exclusivity-check`;
`just deployment-transition-semantic-authority-zero-state-check`;
`just legacy-writer-fence-check`; `just milestone-aggregate-closure-check M05`;
`just relational-fabric-cutover-readiness-check`.

**Integration Milestone.**

M05.

**Replan Triggers.**

Reopen D-26/D-34 if authority cannot remain singular in any transition, the selected bridge/
external revocation cannot deny the exact frozen binary across restart/reboot, an external
controller must write fabric facts, or rollback after new mutation is required. Revise the plan if deployment
topology exposes another durable phase or the platform-private journal cannot provide one
same-user controller lease plus durable atomic head replacement. A failed M05 oracle blocks
transition; it is not an operational override.

**Rollback or Recovery.**

Before `NEW_MUTATING`, follow the design's fenced old-binary rollback after stopping and proving
release of the new generation. At or after `NEW_MUTATING`, old-binary rollback is forbidden;
activate a compatible prior epoch or build a corrective forward epoch through `FabricCommand`.

**Design-Bearing Contracts and Exemplars.**

```text
CutoverController (sole writer)
  -> immutable DeploymentTransitionRecordV1
  -> predecessor-CAS deployment head
  -> process / lease side effect
  -> fsync + readback + reconciliation

daemon reads deployment phase for admission only
Delta activation chain derives semantic current
bridge/external authority revokes old serving and write ingress
```

The controller may start/stop binaries and append predecessor-checked deployment transitions; it
cannot write semantic facts, select an unproved epoch, or override a writer fence.

### WP22 — Freeze and independently accept evidence before implementation consumers

**Outcome.**

The exact legacy comparator and every independent model/provider/query/public/security/
activation expectation needed by later packets are immutable, decoded, reviewable, and accepted
before their consumers execute. Production model, provider, compiler, query, security, and
cutover implementations cannot author or revise these inputs.

**Dependencies.**

WP01 only. Comparator capture and evidence ownership occur at the transition start, before any
model/provider/query implementation packet. Evidence may describe not-yet-implemented target
rows because its authority is the accepted design, exact upstream API behavior, released public
contracts, and independent semantic judgment—not generated target output.

**Target invariants.**

I-20, I-23, I-27, I-30, I-33, and I-34. Advances P4, P9–P10, P20, P25, P27, P29–P30,
and P36; prevents producer-authored proof and post-hoc comparator construction.

**Design and library references.**

Design D-20, D-23, D-28–D-34, §§5.2 Stage 1/Stage 5 and 6.1–6.8; audit F-008; exact
provider references and released QRY/SRV/wire contracts declared in section 2.

**Change surface.**

**Preflight query.**

```sh
rg --files tests/golden contracts/acceptance contracts/governance docs/reviews tooling/ci | sort
rg --hidden -n 'expected|golden|comparison|legacy.*binary|worktree|security|activation|provider.*contract|independent' tests contracts docs/reviews tooling scripts justfile -g '!.git/**' -g '!docs/library_ref/**'
git worktree list --porcelain
```

**Known touch:** (verified this session)

Current Gate B/golden fixtures, comparison-ignore inputs, released-artifact evidence, provider
fixtures, public protocol fixtures, security/activation tests, build outputs, and any retained
legacy worktree/executable are candidate evidence surfaces; none is accepted merely because it
already exists.

**Required changes.**

- Assign accountable independent owners and immutable identities to decoded expectation rows for
  bootstrap/model semantics; exact Tree-sitter/Ruff/Pyrefly/rustc public/private boundaries;
  provider failures/remainders; normalization and every derived-analysis family; all eight query
  forms/composition roles; public responses/status/reference/result lifecycle; authorization and
  redaction; Rust hostile compilation; activation ordering/recovery; and legacy revocation.
- Freeze the exact legacy executable or isolated worktree, toolchain/dependency inputs, frozen
  source/provider inputs, and a decoded old/new comparison contract now. Record how it is rebuilt
  or verified, enforce read-only/no-network/no-write execution, and keep legacy output as evidence
  rather than expected truth.
- Define independent causal controls and explicit limitations for every accepted family. A digest
  may identify the fixture, but decoded expected rows and rationale are the acceptance material.
- Version a separate expectation ingress/review transaction. Any later change produces a
  successor evidence candidate and invalidates every dependent proving receipt; no packet-local
  edit may update accepted expectations in place.
- Add `independent-evidence-dag-check`, `early-evidence-acceptance-check`,
  `comparison-engine-isolation-check`, and `expectation-independence-check`. The DAG check parses
  this plan and proves every evidence consumer has WP22 as a transitive predecessor.

**Legacy disposition and decommission.**

Preserves L-44/L-52 evidence and supplies the safe non-live comparator archive used after DB01.
It creates no live legacy authority. DB08 deletes comparator/archive bytes only after retention
and rollback commitments expire.

**Acceptance checks.**

**Behavioral.**

- `just early-evidence-acceptance-check` — validates decoded rows, rationale, limitations, owner
  acceptance, and exact source/API/public-contract provenance.
- `just independent-evidence-dag-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`.

**Structural.**

- `just expectation-independence-check` — rejects production-to-expectation authoring edges.
- `just comparison-engine-isolation-check` — proves the exact comparator is frozen and read-only.

**Negative / zero-state.**

- `just late-expectation-authoring-zero-state-check` — rejects evidence first created by WP08–
  WP21 or modified without a successor review transaction.

**Operational.**

- `just legacy-comparator-reconstruction-check` — proves the exact frozen executable/worktree is
  available without using target outputs.

Oracle catalog:

- Executable oracle: `just early-evidence-acceptance-check`
- Executable oracle: `just independent-evidence-dag-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`
- Executable oracle: `just expectation-independence-check`
- Executable oracle: `just comparison-engine-isolation-check`

**Edit-Local Gates.**

Decoded evidence schema/fixture checks; `just typos`.

**Packet-Local Gates.**

`just early-evidence-acceptance-check`; `just independent-evidence-dag-check
docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`;
`just expectation-independence-check`; `just comparison-engine-isolation-check`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Revise the plan if an evidence consumer is discovered without a transitive WP22 edge. Reopen the
target only when independently justified semantics contradict design v2. Never copy target or
legacy output into expected rows to repair a mismatch.

**Rollback or Recovery.**

Evidence is immutable once accepted. Reject the candidate and author a successor evidence
version under independent review; do not rewrite an accepted corpus.

**Design-Bearing Contracts and Exemplars.**

`independent decoded expectation + owner + limitations + exact input provenance -> immutable
evidence identity`; implementation output is never on the authoring path.

### WP23 — Implement Python owner-local CFG and flow analyses

**Outcome.**

CodeFabric consumes accepted Ruff structure plus Pyrefly semantics and emits application-owned,
typed Python CFG/evaluation-order, exceptional-flow, def-use, reaching-definition, liveness,
value-flow, conservative memory/alias/points-to, effect, resource, async, and explicit-unknown
relations. No derived row is mislabeled as Ruff or Pyrefly native output.

**Dependencies.**

WP08, WP09, and WP11. Raw providers and canonical identity/authority must be accepted before this
analysis phase.

**Target invariants.**

I-20, I-23–I-24, I-27, I-30, and I-33. Advances P2–P4, P9–P10, P14–P20, P27, P29,
and P36; maintains explicit unknowns and fact-not-judgment doctrine.

**Design and library references.**

Design D-23–D-24, D-30, LD-19, LD-21, LD-25; GEN Python analysis §§24–41 and exact
Ruff/Pyrefly references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/python_semantic.rs src/derivation.rs src/domain_conformance.rs --items exports --view signatures
rg --hidden -n 'cfg|reaching|liveness|def.use|value.flow|alias|points.to|effect|resource|async|python' src tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/python_semantic.rs`, current derivation/domain-conformance code, Python semantic fixtures,
canonical call/type/import relations, incremental owner replacement, and capability/provenance
schemas are the adjacent surfaces.

**Required changes.**

- Define model-selected input/output relation contracts for Python CFG nodes/edges, evaluation
  order, exceptional edges, definitions/uses, reaching definitions, liveness, value flow,
  conservative heap/attribute/subscript memory locations, alias/points-to candidates, effects,
  resource acquire/release/escape, async suspension, and unknown/precision limits.
- Implement owner-local algorithms over immutable Arrow relations using native DataFusion plans
  where visible and bounded Rust algorithms only where needed. Record algorithm/precision
  release, input model/provider/source identities, projection, coverage, and provenance on every
  output family.
- Define incremental invalidation from changed owners plus Pyrefly's affected-module/reverse-
  importer relation. Compare incremental replacement with clean recomputation for additions,
  deletions, exceptional flow, dynamic features, partial typing, and provider degradation.
- Emit no evaluative conclusions. Unsupported dynamic semantics become typed multi-candidate or
  unknown rows, never optimistic absence.
- Add `python-derived-analysis-conformance-check`, `python-analysis-incremental-equivalence-check`,
  and `python-derived-provider-authority-zero-state-check`.

**Legacy disposition and decommission.**

Replaces Python portions of L-28/L-30 and prevents procedural/current JSON derivations from
surviving DB02. Raw providers remain intact and queryable.

**Acceptance checks.**

**Behavioral.**

- `just python-derived-analysis-conformance-check` — compares every family with WP22 expectations.
- `just python-analysis-incremental-equivalence-check` — bag/schema/provenance equality with clean
  recomputation.

**Structural.**

- `just derived-analysis-provenance-contract-check python`.

**Negative / zero-state.**

- `just python-derived-provider-authority-zero-state-check` — rejects Ruff/Pyrefly-native labels
  and judgment facts on application output.

**Operational.**

- `just root-test`.

Oracle catalog:

- Executable oracle: `just python-derived-analysis-conformance-check`
- Executable oracle: `just python-analysis-incremental-equivalence-check`
- Executable oracle: `just derived-analysis-provenance-contract-check python`
- Executable oracle: `just python-derived-provider-authority-zero-state-check`

**Edit-Local Gates.**

Focused owner-local algorithm tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

All four oracle-catalog checks; `just root-clippy`; `just root-test`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Revise the family partition if an algorithm requires a coherent cross-owner packet. Reopen D-30
if accepted semantics cannot expose precision/unknown behavior as facts; do not assign the work
back to a provider that does not expose it.

**Rollback or Recovery.**

Derived output belongs only to an unpublished candidate. Drop it and recompute from raw/canonical
inputs; never mutate native observations to preserve a failed derivation.

**Design-Bearing Contracts and Exemplars.**

`Ruff raw structure + Pyrefly semantic evidence -> CodeFabric Python analysis release -> derived
relations + unknowns + provenance`.

### WP24 — Implement Rust MIR-derived ownership and flow analyses

**Outcome.**

CodeFabric consumes accepted raw MIR/access plus optional exact private enrichment and emits
application-owned ownership state, def-use, reaching-definition, liveness, conservative alias/
points-to, drop/resource, async/lowering, unsafe/FFI, control-flow, and explicit-unknown
relations. Exact borrowck observations remain a distinct private-provider family.

**Dependencies.**

WP10 and WP11. Public/private raw authority, canonical identity, and sandbox receipts must be
accepted first.

**Target invariants.**

I-20, I-23–I-24, I-27, I-30, I-33, and I-34. Advances P2–P5, P9–P10, P14–P20,
P24, P27, P29, and P36.

**Design and library references.**

Design D-23–D-24, D-30, D-33, LD-19, LD-21, LD-25; GEN Rust analysis §§42–51 and the
exact MIR reference.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline rustc-extractor/src src/rustc_service.rs src/derivation.rs --items exports --view signatures
rg --hidden -n 'ownership|borrow|loan|region|reaching|liveness|alias|points.to|drop|resource|async|unsafe|ffi|MIR' src rustc-extractor tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Extractor raw MIR relations, `src/rustc_service.rs`, current derivation/projection code, Rust
semantic fixtures, identity/authority models, and incremental owner replacement are adjacent.

**Required changes.**

- Normalize raw MIR places/projections/operands/rvalues/statements/terminators into typed access
  events without discarding provider-native coordinates or confusing MIR indices with canonical
  identity.
- Define and implement versioned application algorithms for ownership state, def-use, reaching
  definitions, liveness, conservative alias/points-to, drop/resource, async/generator lowering,
  unsafe/FFI, control dependence inputs, and unknown/precision limits. Keep exact private loans/
  regions and application approximations in different relations and provenance domains.
- Record algorithm release, input toolchain/compiler/model/source pins, precision, invalidation,
  completeness, and materialization policy. Owner-local incremental replacement must equal clean
  recomputation under control-flow, cleanup/unwind, drop, async, compile-failure, and degraded
  private-enrichment fixtures.
- Add `rust-mir-derived-analysis-conformance-check`,
  `rust-analysis-incremental-equivalence-check`, and
  `rust-derived-provider-authority-zero-state-check`.
- Extend `exact-provider-fabric-check` as the M02 aggregate over the four raw lanes, WP26 trust
  receipts, and both owner-local analysis packets; it must preserve individual failure detail.

**Legacy disposition and decommission.**

Replaces Rust derivation portions of L-28/L-30/L-33 and supplies positive proof before DB02
removes MIR summary/procedural projection paths.

**Acceptance checks.**

**Behavioral.**

- `just rust-mir-derived-analysis-conformance-check`.
- `just rust-analysis-incremental-equivalence-check`.

**Structural.**

- `just derived-analysis-provenance-contract-check rust`.

**Negative / zero-state.**

- `just rust-derived-provider-authority-zero-state-check` — rejects `rustc_public`/private labels
  on application approximations and application provenance on exact borrowck rows.

**Operational.**

- `just extractor-ci-fast`.

Oracle catalog:

- Executable oracle: `just rust-mir-derived-analysis-conformance-check`
- Executable oracle: `just rust-analysis-incremental-equivalence-check`
- Executable oracle: `just derived-analysis-provenance-contract-check rust`
- Executable oracle: `just rust-derived-provider-authority-zero-state-check`

**Edit-Local Gates.**

Focused MIR-analysis tests; `just root-fmt`; `just root-check`; `just extractor-check`.

**Packet-Local Gates.**

All four oracle-catalog checks; `just root-clippy`; `just root-test`; `just extractor-ci-fast`.
Also require the extended `just exact-provider-fabric-check` aggregate.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen only the affected semantic family if the exact raw/private inputs cannot support its
accepted precision. Emit downgraded capability or unsupported remainder rather than claiming
borrowck or stable identity that the selected seam did not provide.

**Rollback or Recovery.**

Discard candidate derived rows and rerun from immutable raw inputs under their pinned algorithm
release; never rewrite raw compiler evidence.

**Design-Bearing Contracts and Exemplars.**

`rustc public raw MIR/access + distinct private enrichment -> CodeFabric MIR analysis release ->
derived ownership/flow/resource relations`.

### WP25 — Close common graph, effect/resource, and interprocedural analysis producers

**Outcome.**

Common control/data-dependence and graph facts, cross-language effects/resources, and
interprocedural callable summaries are produced by explicit versioned analyses. A relational
closure proves that every accepted ontology/query family has exactly one runtime producer or an
explicit unsupported remainder before semantic query compilation.

**Dependencies.**

WP12, WP23, and WP24. Language-local facts and the native-first graph execution mechanism must
exist before common fixed points and producer closure.

**Target invariants.**

I-20, I-23–I-24, I-27–I-30, I-32–I-33. Advances P2–P3, P9–P10, P14–P20, P25,
P27, P29, and P36.

**Design and library references.**

Design D-24–D-25, D-28, D-30, LD-19, LD-21, LD-24; GEN common graph/effect/resource and
interprocedural summary §§52–66; DataFusion 55 and petgraph 0.8.3 references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/derivation.rs src/domain_conformance.rs src/semantic_query.rs --items exports --view signatures
rg --hidden -n 'dominat|control.depend|data.depend|call.graph|effect|resource|summary|fixed.point|producer|unsupported' src tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Current derivation/domain-conformance, graph projections, callable/call-target relations,
effect/resource schemas, query capabilities, fixed-point logic, proof relations, and ontology/
query family declarations are the adjacent surface.

**Required changes.**

- Define input/output relations and versioned algorithms for dominator/post-dominator/control-
  dependence, data-dependence, call graph, accepted SCC/reachability families, effect/resource
  propagation, and interprocedural callable summaries. Use WP12's selected DataFusion/petgraph
  rung and keep canonical IDs relational.
- Make fixed points monotone, deterministic, bounded by explicit convergence/resource policy,
  and provenance-producing. Unknown callees, partial providers, recursion, dynamic dispatch,
  compile/type gaps, and resource exhaustion propagate explicit incomplete/unknown results.
- Define owner/module/call-graph invalidation and compare incremental recomputation with a clean
  whole-candidate run. Materialization policy derives from measured reuse/cost and never changes
  semantic identity.
- Compile `accepted_fact_family`, `runtime_producer`, `query_family_requirement`, and
  `unsupported_remainder` relations. `derived-analysis-authority-coverage-check` fails on zero or
  multiple producers, wrong authority, missing algorithm/precision/provenance, or a query family
  whose required facts lack supported/remainder closure.
- Add `common-derived-analysis-conformance-check`,
  `interprocedural-summary-fixed-point-check`, and
  `derived-analysis-authority-coverage-check`.

**Legacy disposition and decommission.**

Completes the positive semantic replacement for L-28/L-30/L-41 and the derived portions of
L-39/L-49. DB02 may remove legacy graph/derivation consumers only after this closure is accepted.

**Acceptance checks.**

**Behavioral.**

- `just common-derived-analysis-conformance-check`.
- `just interprocedural-summary-fixed-point-check`.

**Structural.**

- `just derived-analysis-authority-coverage-check` — exactly one producer or explicit remainder
  for every accepted fact/query family.

**Negative / zero-state.**

- `just derived-analysis-judgment-zero-state-check` — rejects evaluative conclusions and
  provider-native mislabeling.

**Operational.**

- `just root-test`.

Oracle catalog:

- Executable oracle: `just common-derived-analysis-conformance-check`
- Executable oracle: `just interprocedural-summary-fixed-point-check`
- Executable oracle: `just derived-analysis-authority-coverage-check`
- Executable oracle: `just derived-analysis-judgment-zero-state-check`

**Edit-Local Gates.**

Focused fixed-point/closure tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

All four oracle-catalog checks; clean/incremental equivalence fixtures; `just root-clippy`;
`just root-test`.

**Integration Milestone.**

M03.

**Replan Triggers.**

Revise packets if one analysis family has a different dependency/invalidation closure. Reopen
D-30 only if an accepted family cannot have a single explicit producer/remainder without a new
semantic authority; never hide the gap in capability prose.

**Rollback or Recovery.**

Discard candidate summaries and rebuild from accepted language-local relations. A failed fixed
point or resource bound yields explicit unknown and blocks dependent capability/activation.

**Design-Bearing Contracts and Exemplars.**

`accepted family LEFT JOIN runtime producer/remainder -> exactly one authority`; every producer
row names algorithm, precision, inputs, invalidation, materialization, completeness, and proof.

### WP26 — Establish the Rust untrusted-compilation trust launcher

**Outcome.**

Every untrusted Rust semantic extraction runs through one policy-bearing launcher that contains
build scripts and procedural macros with immutable inputs, private outputs, no inherited network
or credentials, bounded resources, and process-group cancellation. Platforms without the
selected containment fail closed; `TRUSTED_LOCAL` is explicit, separately authorized, and visible.

**Dependencies.**

WP01 and WP04. The repository trust policy and control protocol must exist before the compiler
lane invokes untrusted code.

**Target invariants.**

I-22, I-27, I-30, and I-34. Advances P4–P5, P13, P16, P20–P24, P32–P36; maintains the
dated-nightly process boundary without treating process isolation alone as sandboxing.

**Design and library references.**

Design I-34/D-33, D-23, LD-25–LD-26, §§3.12 and 6.8; GEN AC-G-35 and MIR reference §53.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline rustc-extractor/src src/rustc_service.rs src/provider_sandbox.rs src/provider_runtime.rs --items exports --view signatures
rg --hidden -n 'sandbox|build.script|proc.macro|network|credential|target.dir|rlimit|timeout|kill|process.group|TRUSTED_LOCAL' src rustc-extractor scripts tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

Provider sandbox descriptors, rustc process launch, environment construction, repository/source
views, target/output directories, cancellation, resource governance, capability/provenance, and
host-matrix tests own or border the trust seam.

**Required changes.**

- Define versioned `RustCompilationTrustPolicy`, platform capability probe, launcher receipt, and
  provenance/capability rows. The untrusted profile requires descriptor-relative immutable/
  read-only workspace and dependency views, offline registry/cache inputs, a minimal allowlisted
  environment, removal of credential/proxy/agent variables and inherited descriptors, and no
  network namespace/capability.
- Create private per-run target/output/temp directories outside source, validate every ingress/
  egress path, bound stdout/stderr/artifact bytes, CPU, memory, wall time, process count, and file
  count/size, and terminate the full process group on cancel/timeout/limit.
- State the build-script/proc-macro policy explicitly: they either execute inside the proved
  containment or extraction fails closed. Never silently invoke host Cargo/rustc. A
  `TRUSTED_LOCAL` profile requires a distinct authorization input and appears as degraded trust in
  every capability/provenance/public status projection.
- Add hostile fixtures that attempt network access, environment/credential reads, source and
  parent writes, symlink/path escape, child-process survival, output explosion, fork/process
  exhaustion, timeout, and memory/CPU exhaustion. Prove the real launcher, not a claimed digest.
- Add `rustc-untrusted-compilation-sandbox-check`, `rustc-trust-policy-closure-check`, and
  `rustc-host-compiler-bypass-zero-state-check`.

**Legacy disposition and decommission.**

Reshapes L-33–L-34 and supplies the trust replacement before DB02 removes legacy extractor/
sandbox assumptions. It adds no new Cargo root.

**Acceptance checks.**

**Behavioral.**

- `just rustc-untrusted-compilation-sandbox-check` — executes all hostile fixtures through the
  real supported-platform launcher.
- `just rustc-sandbox-cancellation-resource-check`.

**Structural.**

- `just rustc-trust-policy-closure-check` — every compiler ingress selects an explicit proved
  profile and emits a receipt.

**Negative / zero-state.**

- `just rustc-host-compiler-bypass-zero-state-check` — rejects direct untrusted Cargo/rustc
  launches and claimed-only sandbox receipts.

**Operational.**

- `just semantic-sandbox-host-matrix-check`.

Oracle catalog:

- Executable oracle: `just rustc-untrusted-compilation-sandbox-check`
- Executable oracle: `just rustc-sandbox-cancellation-resource-check`
- Executable oracle: `just rustc-trust-policy-closure-check`
- Executable oracle: `just rustc-host-compiler-bypass-zero-state-check`

**Edit-Local Gates.**

Focused launcher/policy tests; `just root-fmt`; `just root-check`; `just extractor-check`.

**Packet-Local Gates.**

All four oracle-catalog checks; `just semantic-sandbox-host-matrix-check`; `just root-test`;
`just extractor-ci-fast`.

**Integration Milestone.**

M02.

**Replan Triggers.**

Reopen D-33 if a supported platform cannot enforce the required untrusted profile. A business
decision to allow `TRUSTED_LOCAL` must be explicit and cannot be inferred from launcher failure.

**Rollback or Recovery.**

Terminate the process group, delete only the validated private run directory, emit a typed gap,
and leave the candidate unproved. Never retry outside containment.

**Design-Bearing Contracts and Exemplars.**

`untrusted workspace + exact toolchain + RustCompilationTrustPolicy -> contained process group ->
launcher receipt + provider run or explicit gap`.

### WP27 — Implement the model-derived logical/physical schema lifecycle

**Outcome.**

One executable `SchemaContract` preserves logical Arrow/DataFusion domain meaning across provider
ingress, analysis, optimization, physical planning, IPC, Delta/Parquet storage, scanning,
streaming, batches, and sinks. Fixed-width IDs and semantic metadata cannot silently degrade to
ordinary binary storage values.

**Dependencies.**

WP03 and WP04. Accepted model rows and Arrow relation framing are required; later catalog,
compiler, and Delta packets consume this contract.

**Target invariants.**

I-20, I-22, I-24, I-27, I-29, and I-31. Advances P2–P3, P7–P8, P12, P14–P15, P27,
P31–P32, and P36.

**Design and library references.**

Design I-22/D-31, D-22–D-24, D-27, LD-17, LD-20, LD-22; Arrow/Parquet 59.2.0,
DataFusion 55, and delta-rs `43a0cf10` exact references.

**Change surface.**

**Preflight query.**

```sh
ast-grep outline src/schema_registry.rs src/fabric/snapshot_catalog.rs src/fabric/serving.rs src/fact_ingest.rs --items exports --view signatures
rg --hidden -n 'Id16ContractProvider|FixedSizeBinary|DataType::Binary|DFSchema|SchemaRef|projection|statistics.*index|column.mapping|deletion.vector|extension.*metadata' src tests contracts tooling/ci -g '!.git/**' -g '!docs/library_ref/**'
```

**Known touch:** (verified this session)

`src/schema_registry.rs`, `Id16ContractProvider`, provider/IPC schema checks, catalog/view
construction, Delta scan/write paths, Parquet mappings, expression outputs, and result-resource
validation own fragments of the current contract.

**Required changes.**

- Compile `SchemaContract` from model relation/field/type/key/representation rows and exact
  physical bindings. It contains source schema identity, Arrow `SchemaRef`, qualified `DFSchema`,
  logical/storage types and casts, projection/filter/column/statistics mappings, nullability,
  nested/dictionary/extension metadata, column mapping/deletion-vector behavior, restoration,
  and explicit empty-stream schema.
- Validate the contract after analysis, optimized logical plan, initial and optimized physical
  plan, stream construction, every `RecordBatch`, and every sink. Reject wrong-width IDs, missing/
  changed metadata where semantically required, invalid nullability/nesting, mapping errors, and
  provider-declared schemas that disagree with emitted batches.
- Use native projections/casts/views where possible and at most one generic transparent adapter
  for irreducible storage adaptation. The adapter must preserve filter/projection/statistics
  index semantics and optimizer visibility.
- Retain `Id16ContractProvider` and its negative fixtures until the generic replacement passes
  every phase and the real Delta route. Delete it only in the same proving commit that closes
  positive equivalence and zero-state evidence.
- Add `relational-schema-lifecycle-check` and `schema-phase-boundary-check`, and publish reusable
  Delta contract cases for empty stream, wrong-width, nested, column mapping, and deletion vectors.
  WP17 owns `delta-provider-contract-check` and must consume those cases on the real route.

**Legacy disposition and decommission.**

Supplies the positive replacement for schema portions of L-24/L-29/L-35. DB03 may remove the
domain-specific wrapper only after WP17 reruns the contract on exact Delta providers.

**Acceptance checks.**

**Behavioral.**

- `just relational-schema-lifecycle-check`.
- `just schema-phase-boundary-check`.

**Structural.**

- `just schema-contract-runtime-closure-check` — every accepted logical/physical relation has
  one model-derived contract consumed by catalog, compiler, provider, IPC, and sink paths.

**Negative / zero-state.**

- `just schema-adaptation-bypass-zero-state-check` — rejects unchecked provider/batch/write
  paths and premature ID-wrapper removal.

**Operational.**

- `just root-test`.

Oracle catalog:

- Executable oracle: `just relational-schema-lifecycle-check`
- Executable oracle: `just schema-phase-boundary-check`
- Executable oracle: `just schema-contract-runtime-closure-check`
- Executable oracle: `just schema-adaptation-bypass-zero-state-check`

**Edit-Local Gates.**

Focused Arrow/DataFusion schema tests; `just root-fmt`; `just root-check`.

**Packet-Local Gates.**

All four oracle-catalog checks; `just relational-arrow-boundary-check`; `just root-clippy`;
`just root-test`.

**Integration Milestone.**

M01.

**Replan Triggers.**

Reopen D-31 if a required domain type cannot survive the chosen logical/physical mapping with
one generic transparent seam. Do not delete an enforcing wrapper or accept metadata-only meaning
until replacement proof is causal.

**Rollback or Recovery.**

Reject the candidate schema/plan/batch before publication and retain the currently proved
adapter. Build a successor contract migration rather than reinterpreting stored bytes.

**Design-Bearing Contracts and Exemplars.**

`model logical field -> qualified DF field -> physical storage field -> restored logical field`,
with explicit maps and validation at every phase boundary.

## 5. Integration milestones

### M01 — Replayed model foundation is independently accepted

**Packets:** WP01–WP07, WP22, and WP27.

**Evidence:** one current v2 suite; complete legacy-disposition selector coverage; deterministic
bootstrap/release replay; independently reviewed row-level import disposition; frozen comparator
and preaccepted provider/query/public/security/activation evidence; relation-scoped Arrow IPC;
model-derived logical/physical schema lifecycle; model-only catalog closure; native compiler
causality; relational proof and provenance.

**Gate:** WP07 adds `just relational-model-foundation-check`, an aggregate that runs the WP01–WP07,
WP22, and WP27 packet gates without mutating artifacts. No target provider or production mutation
is claimed. DB01 executes immediately after this acceptance and the bounded importer rollback
decision.

### M02 — Exact provider fabric is complete and honest

**Packets:** WP08–WP11, WP23, WP24, and WP26.

**Evidence:** all accepted current API families have independent boundary contracts and typed
native relations on their real Tree-sitter/Ruff/Pyrefly/rustc public/private surfaces; Rust
untrusted compilation is contained; Python and Rust owner-local derived relations have separate
authority; IPC integrity, requested/completed coverage, normalized/canonical coexistence,
provenance, explicit unknowns, capability proof, and honest ordinary statistics are proved.

**Gate:** WP24 extends `just exact-provider-fabric-check` to aggregate WP08–WP11, WP23, WP24,
and WP26 without hiding provider, trust, or derived-authority failures.

### M03 — Authorized semantic delivery is end-to-end

**Packets:** WP12–WP15 and WP25.

**Evidence:** per-operation native/recursive/function/extension graph selection, complete custom
execution contracts, common graph/effect/resource/interprocedural analyses, accepted-family
producer closure, all semantic forms/compositions, sealed reduced child catalogs with bound-view
and fresh-store closure, policy/redaction, dynamic catalog reference/status, Arrow result
resources, and a real FastMCP-to-daemon vertical with no public physical surface or packaged
semantic authority.

**Gate:** `just semantic-delivery-vertical-check`, whose dependency closure includes WP25's
accepted-family producer proof.

### M04 — Durable epoch reconstruction and recovery are proved

**Packets:** WP16–WP19.

**Evidence:** exclusive command ingress, writer generation fencing, mutually exclusive exact
Delta selectors, controlled zero-retry compaction, native overlay views, immutable activation
events with admission-before-selection, crash reconciliation, resource governance, lifecycle
integration, and clean/incremental reconstruction with live legacy inputs unavailable.

**Gate:** `just durable-epoch-reconstruction-check`.

### M05 — Independent release evidence and fenced cutover are accepted

**Packets:** WP20–WP21.

**Evidence:** independently authored decoded semantics and causal controls, classified old/new
deltas, security/performance/package evidence, crash-reconcilable cutover, one serving/mutation
authority, and bridge/external exact-old-binary revocation at `NEW_MUTATING`.

**Gate:** `just milestone-aggregate-closure-check M05` first proves that
`relational-fabric-cutover-readiness-check` expands only to M03, M04, WP20 independent-release
evidence, and WP21 cutover/fence/authority evidence; the latter aggregate must then pass. M05 is
accepted only after that acyclic aggregate completes and is never one of its own inputs.

### M06 — Legacy functionality is physically absent

**Batches:** early DB01 after M01; DB02–DB07 after `LEGACY_RETIRED`; DB08 after final comparator/
archive retention expiry.

**Evidence:** every current inventory row has exactly one resolved disposition; consumers are
removed before authorities; released IDs are preserved or tombstoned; all target behaviors still
pass; no legacy loader/generator/import/activation/query/package/fallback path remains; no
retained comparator/archive bytes remain after expiry; all four build domains and feature graphs
are green.

**Gate:** DB08 completes `just relational-fabric-final-certification` after DB07's live-code and
history-detachment proof.

## 6. Cross-packet decommission batches

### DB01 — Retire the one-time importer and all live migration-input routes

**Prerequisites:** M01 accepted; the row-level migration and independent expectation evidence are
immutable; the explicitly bounded importer rollback decision is closed; model epochs reconstruct
from accepted migrations and compiler releases; and WP22 has frozen any exact predecessor bytes
required for comparison or old-binary rollback in a non-live archive. L-20–L-22 and L-54
selectors must be fresh at the batch proving commit. `LEGACY_RETIRED` is not a prerequisite.

**Disposition:** delete the importer executable/feature/tests/build route and every live build,
runtime, generator, test-authoring, or tooling read of frozen YAML, schema IR, fragments, policy/
comparison/fault/config registries, and predecessor bundles. Preserve accepted `ModelMigration`/
`ModelDecision` rows, independent review, released allocations, and historical commits. Move only
explicitly committed comparator/old-binary bytes to WP22's immutable non-live archive; no package,
build, runtime, generator, or mutable tool may read it. DB08 removes the archive at expiry.

**Exit checks:** `just model-importer-zero-state-check`;
`just frozen-migration-input-live-read-zero-state-check`; `just model-replay-check`;
`just comparison-engine-isolation-check`; `just legacy-comparator-reconstruction-check`;
`just legacy-disposition-coverage-check`.

**Rollback/recovery:** restore no live importer after exit. A later correction is a new reviewed
migration under a new compiler release, not reactivation of predecessor inputs. Comparison or
old-binary rollback may consume only the separately isolated WP22 harness/archive under its exact
commitment; it cannot make the importer live again.

### DB02 — Purge legacy provider, projection, and graph consumers

**Prerequisites:** M02–M04 accepted and `NEW_MUTATING` reached. Exact native relations,
normalization/authority/unknown plans, capability proof, graph extension, and retained-data
compatibility are serving; Python/Rust/common derived producer closure and the Rust trust launcher
are accepted.

**Disposition:** complete L-30, L-31, L-32, L-33, L-34, and L-41 by deleting cold/opaque semantic payloads,
procedural projection DTO chains, generated provider-kind registries, summary messages,
hard-coded adapter/capability inventories, defensive mirrors, persisted graph DTO/index identity,
misattributed provider-native CFG/dataflow/borrowck claims, and every target-route compatibility
decoder. Preserve short-lived exact vendor bindings,
provider process/sandbox lifecycle, raw kinds, diagnostics, remainders, Arrow IPC, and the bounded
petgraph kernel only where positively used.

**Exit checks:** `just provider-legacy-json-zero-state-check`;
`just provider-static-registry-target-zero-state-check`;
`just persisted-petgraph-identity-zero-state-check`;
`just exact-provider-fabric-check`; `just graph-extension-conformance-check`;
`just derived-analysis-authority-coverage-check`;
`just rustc-untrusted-compilation-sandbox-check`;
`just legacy-disposition-coverage-check`.

**Rollback/recovery:** forward-fix exact adapters/contracts. No opaque compatibility payload or
legacy provider route may be restored after new mutation.

### DB03 — Purge legacy serving, storage, mutation, query, and adapter consumers

**Prerequisites:** M03–M05 accepted, legacy writer durably fenced, retained old epochs either
reconstruct or fail with an accepted typed incompatibility, and package/protocol proof is green.

**Disposition:** complete L-25, L-26, L-35, L-36, L-37, L-38, L-39, L-40, and L-42. Remove custom overlay/consolidation,
mutable snapshot/ontology/current pointers, semantic SQLite tables, direct mutation/activation/
session routes, static query bundles/crosswalks, daemon model-artifact reads, adapter fingerprints/
registries/schema aggregates/query tables, and obsolete generated Protobuf caches. Preserve
released `.proto`/public schemas, narrow boundary/canonical helpers, exact Delta providers,
the generic proved schema adapter, authorized catalogs, result lifecycle, FastMCP topology, and only a foreign-build cache whose
need and derivability were explicitly proved.

**Exit checks:** `just target-mutation-bypass-zero-state-check`;
`just custom-overlay-target-zero-state-check`; `just mutable-current-pointer-target-zero-state-check`;
`just query-static-crosswalk-target-zero-state-check`;
`just adapter-package-authority-zero-state-check`; `just daemon-static-bundle-target-zero-state-check`;
`just package-interop-check`;
`just durable-epoch-reconstruction-check`; `just legacy-disposition-coverage-check`.
Also require `just relational-schema-lifecycle-check`, `just delta-provider-contract-check`, and
`just authorized-view-bound-authority-check` before deleting domain-specific provider/view seams.

**Rollback/recovery:** after `NEW_MUTATING`, repair only through a new command/compiler/epoch.
Restoring an old runtime route or package datum is forbidden.

### DB04 — Purge model compiler, generated authorities, and static semantic products

**Prerequisites:** DB02–DB03 complete; no target consumer reads predecessor model inputs/products;
clean reconstruction and independent expectations pass with those paths unavailable.

**Disposition:** complete L-20, L-21, L-23, L-24, L-27, L-28, L-29, and L-55. Delete the
`codefabric-model` binary and
feature, generator drivers, DesiredTree tooling, generated model/provider/schema/governance
products and manifests, installed ontology bundles, generated Rust registries/encoders/IDs/result
schemas/table specs/identity arrays, and predecessor schema/registry runtime modules. Split mixed
files such as `Cargo.toml`, `src/lib.rs`, `src/contracts/mod.rs`, `src/generated/**`,
`src/contracts/{catalog,index,registry_models}.rs`, `src/derivation.rs`, `tooling/model/**`,
`tooling/proto/**`, bundles, toolchain identities, and adapter outputs by symbol/artifact
disposition; preserve only minimal bootstrap/wire primitives, foreign Protobuf generation,
accepted history, and target relational code. Remove the `registry` bin from `fuzz/Cargo.toml`,
`fuzz/fuzz_targets/registry.rs`, its YAML corpus, and the retired
`replay_bounded_registry_ingress` export only after WP04's separately named relational-model/IPC
target builds and its deterministic seed replay proves the retained parser/protocol risks.
Preserve JCS, identity, and Protobuf fuzz targets only where their current target contracts remain
causally reachable; never relabel the old registry target as relational proof.

**Exit checks:** `just model-compiler-zero-state-check`;
`just generated-model-authority-zero-state-check`; `just legacy-include-zero-state-check`;
`just new-model-legacy-input-isolation-check`; `just clean-rebuild-legacy-input-zero-state-check`;
`just fuzz-registry-target-transition-check`; `just legacy-inventory-universe-check`;
`just legacy-disposition-coverage-check`.

**Rollback/recovery:** do not restore a current generator. A missing target semantic row is fixed
through reviewed migrations/model compilers and a new epoch.

### DB05 — Replace predecessor governance, tests, rules, recipes, and active routing

**Prerequisites:** DB02–DB04 complete; target replay, causal, exact-provider, transaction/fence,
semantic/public, and zero-state checks are installed and independently accepted.

**Disposition:** complete L-45, L-46, L-47, L-48, L-49, L-50, and L-51. Delete v1 detector
registries/baselines and 124 mappings, Gate B producer machinery, artifact/count/digest/census
acceptance, generated-authority rules/tests/recipes/jobs, and temporary freeze checks. Reshape
`justfile`, CI, feature checks, tests, and navigation around intent-level target gates. Preserve
independent goldens, protocol/KAT/behavior evidence, historical doctrine/design/plan/review, true
build/process/wire structural rules, and disposable nonnormative navigation only when it has no
authority/runtime edge. Replace the transitional YAML overlap ledger with plan-qualified
relational plan/resource/phase-decision facts and derived overlap queries while preserving
accountable historical decisions. Update `AGENTS.md` and live `.claude` doctrine/library routing
so no agent is instructed back toward v1 static authority. Remove the predecessor Gate B test
selectors and timeout overrides from `.config/nextest.toml` with their producer tests; retain or
add only target-test resource policy justified by measured execution. Retain `.ignore` solely as
scanner hygiene, delete retired generated-path entries, and prove that it cannot subtract from the
`--no-ignore` legacy inventory universe.

**Exit checks:** `just predecessor-governance-zero-state-check`;
`just producer-generated-golden-target-zero-state-check`; `just v2-authority-cutover-check`;
`just expectation-independence-check`;
`just nextest-predecessor-filter-zero-state-check`; `just ignore-stack-inventory-isolation-check`;
`just governance-scan`; `just artifacts-check`; `just legacy-inventory-universe-check`;
`just legacy-disposition-coverage-check`.

**Rollback/recovery:** fix target gates in place. Do not reintroduce count/digest acceptance or a
static detector registry to obtain green CI.

### DB06 — Remove retired dependencies, features, targets, and package edges

**Prerequisites:** DB01–DB05 complete; current reachability, macro expansion, build scripts,
generated code, examples/tests, renamed dependencies, platform `cfg`, packaging, and all feature
graphs show no retained use. L-20, L-25, L-48, and L-53 decisions are resolved.

**Disposition:** remove only dependencies/features/targets/jobs unique to retired machinery;
delete `model-compiler` and related binary/recipe edges; retain four build roots, exact locks,
toolchains, Arrow/DataFusion/Delta/object-store universe, justified provider roots, gix/notify,
petgraph only if positively used, and Protobuf generation required by the accepted foreign-build
decision. Never treat machete/shear output alone as deletion authority.

**Exit checks:** `just deps-fast`; `just policy`; `just stable-graph-check`;
`just features-each`; `just features-no-default`; `just root-test`; `just extractor-ci-fast`;
`just sidecar-ci-fast`; `just adapter-ci-fast`; `just fuzz-target-build-check`;
`just adapter-built-package-contents-check`; `just package-interop-check`.

**Rollback/recovery:** restore only a dependency whose retained target code proves a direct need;
do not restore the retired feature/target as an organizing convenience.

### DB07 — Detach immutable history and comparator evidence from every live read

**Prerequisites:** DB01–DB06 complete; every released ID has an accepted preservation,
migration, supersession, or tombstone transaction; retention/rollback leases are resolved; the
legacy daemon is retired and physically unavailable to clean-rebuild proof. A WP22 comparator/
old-binary archive may remain only if an explicit unexpired retention or rollback commitment
still requires its bytes; it has no live reader.

**Disposition:** complete L-43, L-44, L-52, L-53, and L-54 by preserving immutable released wire,
allocations, canonicalization KATs, independent expectations, accepted decisions, prior
principles/designs/plans/state/reviews, and necessary retained Delta history only as explicit
history. Remove every active runtime/build/test-authoring/generator/package/status/pointer read
from that archive. Comparator execution is isolated through WP22's read-only harness and cannot
be a current target dependency. Delete obsolete retained data only under accepted tombstone/
retention policy. Remove transition-only checks whose forbidden state is now structurally
impossible while retaining permanent residue guards.

**Exit checks:** `just released-artifact-disposition-check`;
`just archived-history-live-read-zero-state-check`;
`just relational-fabric-legacy-zero-state-check`;
`just frozen-migration-input-live-read-zero-state-check`;
`just legacy-inventory-universe-check`; `just legacy-disposition-coverage-check`.

**Rollback/recovery:** completion is forward-only. Archived evidence remains readable by humans
and explicit migration/recovery tooling only when named by a current policy; it never becomes a
selectable runtime fallback.

### DB08 — Expire the non-live comparator/archive and certify total purge

**Prerequisites:** DB07 complete; every explicit comparator, old-binary rollback, legal,
operational, and release-retention commitment is expired or has an accountable superseding
disposition; `NEW_MUTATING` and `LEGACY_RETIRED` are durable; no accepted recovery route can invoke
the legacy executable or importer.

**Disposition:** delete the frozen legacy executable/worktree, archived static migration-input
bytes, comparator-only toolchain/cache material, and isolated comparison harness that WP22/DB01
retained. Preserve only human-readable immutable designs/plans/reviews/decisions, released
allocations/tombstones, independent decoded expectation rows, and Delta history still required by
current retention policy. Add the aggregate `relational-fabric-final-certification` recipe here;
it includes DB01–DB08 exit checks and proves that no legacy byte or reader remains outside the
explicit human/history and current-retention allowlists.

**Exit checks:** `just legacy-comparator-archive-zero-state-check`;
`just frozen-migration-input-live-read-zero-state-check`;
`just archived-history-live-read-zero-state-check`;
`just relational-fabric-legacy-zero-state-check`;
`just released-artifact-disposition-check`; `just legacy-inventory-universe-check`;
`just legacy-disposition-coverage-check`; `just relational-fabric-final-certification`;
`just ci-pr`.

**Rollback/recovery:** final archive deletion is forward-only and occurs only after the named
retention authorities expire. A later semantic correction uses current migrations, sources,
compiler releases, and target commands; it never restores the importer, old binary, or comparator
archive.

## 7. Final gate matrix

The final proving commit and HEAD must resolve and pass this intent-level recipe matrix. Aggregate
recipes expand to their packet checks and must remain non-mutating.

- Environment and artifact integrity: `just doctor`, `just artifacts-check`,
  `just stable-graph-check`, `just authoritative-design-conformance-check`,
  `just v2-authority-cutover-check`,
  `just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`,
  `just plan-overlap-ledger-check`,
  `just plan-dependency-check`, `just plan-activation-recovery-check`,
  `just legacy-inventory-universe-check`, `just legacy-disposition-coverage-check`.
- Relational model and proof: `just relational-model-foundation-check`,
  `just model-replay-check`, `just compiler-release-reconstruction-check`,
  `just model-migration-bijection-check`, `just model-migration-independent-review-check`,
  `just relational-catalog-closure-check`, `just fabric-epoch-proof-closure-check`,
  `just early-evidence-acceptance-check`, `just expectation-independence-check`,
  `just relational-causal-intervention-check`.
- Exact providers and Arrow: `just exact-provider-fabric-check`,
  `just provider-protocol-check`, `just provider-statistics-contract-check`,
  `just relational-arrow-boundary-check`, `just relational-schema-lifecycle-check`,
  `just pyrefly-exact-surface-matrix-check`,
  `just pyrefly-semantic-environment-invalidation-check`,
  `just rustc-public-private-authority-check`,
  `just rustc-untrusted-compilation-sandbox-check`,
  `just python-derived-analysis-conformance-check`,
  `just rust-mir-derived-analysis-conformance-check`,
  `just derived-analysis-authority-coverage-check`, `just extractor-ci-fast`,
  `just sidecar-ci-fast`.
- Query, policy, and delivery: `just semantic-query-relational-conformance-check`,
  `just semantic-query-conformance-check`, `just access-catalog-isolation-check`,
  `just authorized-view-bound-authority-check`,
  `just public-query-port-check`, `just graph-extension-conformance-check`,
  `just graph-extension-context-forwarding-check`, `just graph-execution-contract-check`,
  `just semantic-delivery-vertical-check`,
  `just adapter-ci-fast`, `just package-interop-check`.
- Mutation, durability, and operations: `just fabric-single-mutation-path-check`,
  `just single-writer-fence-check`, `just fabric-transaction-contract-check`,
  `just temporal-store-boundary-check`, `just delta-exact-version-reconstruction-check`,
  `just delta-provider-contract-check`,
  `just fabric-activation-recovery-check`, `just fabric-control-recovery-check`,
  `just fabric-epoch-pinning-check`, `just activation-fault-matrix-check`,
  `just durable-epoch-reconstruction-check`, `just resource-governance-check`,
  `just lifecycle-invalidation-conformance-check`.
- Independent release and cutover: `just independent-semantic-oracle-check`,
  `just old-new-independent-comparison-check`, `just relational-fabric-security-check`,
  `just relational-fabric-performance-check`, `just relational-fabric-cutover-readiness-check`,
  `just milestone-aggregate-closure-check M05`, `just fabric-cutover-state-machine-check`,
  `just cutover-transition-authority-check`,
  `just deployment-transition-semantic-authority-zero-state-check`,
  `just legacy-writer-fence-check`.
- Decommission and repository closure: `just model-importer-zero-state-check`,
  `just provider-legacy-json-zero-state-check`, `just target-mutation-bypass-zero-state-check`,
  `just adapter-package-authority-zero-state-check`, `just model-compiler-zero-state-check`,
  `just generated-model-authority-zero-state-check`,
  `just fuzz-registry-target-transition-check`,
  `just predecessor-governance-zero-state-check`, `just archived-history-live-read-zero-state-check`,
  `just frozen-migration-input-live-read-zero-state-check`,
  `just legacy-comparator-archive-zero-state-check`,
  `just relational-fabric-legacy-zero-state-check`, `just released-artifact-disposition-check`,
  `just relational-fabric-final-certification`.
- Four-domain and feature closure: `just features-each`, `just features-no-default`,
  `just root-test`, `just extractor-ci-fast`, `just sidecar-ci-fast`, `just adapter-ci-fast`,
  `just fuzz-target-build-check`, `just adapter-built-package-contents-check`, `just deps-fast`,
  `just policy`, `just governance-scan`, `just ci-fast`, `just ci-pr`.

Tier-C checks remain risk-triggered rather than ceremonial. WP16/WP18 run focused Miri seeds for
new concurrency/fencing primitives; parser/protocol changes run their existing fuzz, coverage,
snapshot, and mutation recipes when their packet risk classification requires them. Final
certification records any accepted platform-specific exemption explicitly; it cannot waive a
semantic, authority, cutover, or zero-state gate.

## 8. Execution sequence

Activation prerequisites are outside this successor DAG and must already have a separately
governed proving commit: inactive-candidate validation, plan-qualified overlap identity,
predecessor disposition, crash-recoverable state/pointer activation, and
`plan-candidate-readiness-check`. After that remediation lands, independently re-audit this exact
design-v2/plan-v2 pair and declared inputs, obtain explicit approval, and execute the confirm-
gated activation transaction. WP01 begins only after the pointer selects this plan and its exact
schema-version-2 state. No preactivation implementation substep belongs to WP01 or any other
packet here.

The dependency graph is:

```text
external candidate-readiness + focused re-audit + approval + activation
  -> WP01
       -> WP22 (independent evidence and frozen comparator)
            -> WP02 -> WP03 -> WP04
                                              +-> WP26 -> WP10
                                              +-> WP27 -> WP05 -> WP06 -> WP07
                                              +-------------> WP08
                                              +-------------> WP09

WP07 + WP08 + WP09 + WP10 -> WP11
WP08 + WP09 + WP11        -> WP23
WP10 + WP11               -> WP24
WP06 + WP07 + WP11 + WP23 + WP24 -> WP12
WP12 + WP23 + WP24        -> WP25
WP11 + WP12 + WP22 + WP25 -> WP13 -> WP14 -> WP15 -> M03

M01 -> DB01 (early live importer/input teardown; non-live archive only)
M01 -> WP16
WP11 + WP12 + WP14 + WP16 + WP27 -> WP17
WP15 + WP16 + WP17 + WP22 -> WP18 -> WP19 -> M04
M03 + M04 + WP22 -> WP20 -> WP21 -> M05
M05 -> DB02 -> DB03 -> DB04 -> DB05 -> DB06 -> DB07
retention expiry -> DB08 -> M06
```

WP22 runs before implementation consumers and may not share their authoring ownership. WP08–WP10
may run in parallel only in isolated worktrees after their accepted boundary rows, with WP10 also
blocked on WP26. WP23 and WP24 may then run in parallel. WP12 and WP25 close common derived
semantics before WP13. WP16 may begin after M01 while provider/query work proceeds, but WP17
cannot accept provider data before M02 and WP19 cannot complete before M03. DB01 is deliberately
the sole early decommission batch. DB02–DB07 wait for `LEGACY_RETIRED`; DB08 additionally waits
for final comparator/archive retention expiry. Within each chain, consumers precede authorities
and dependency removal.

Execution loads one packet or batch at a time, reruns its preflight against current HEAD, records
new obligations in state, and proves it at a commit. A packet gate that passed only before a
downstream change is stale until rerun at HEAD. M01–M05 are integration trust boundaries, not
substitutes for packet proof.

## 9. Plan risks and replan policy

### 9.1 Principal execution risks

- **Post-baseline drift.** Cleanup is complete at `7184b86...`; every packet still reruns current-
  tree preflight. Drift into a declared design/doctrine/audit input, external-governance receipt,
  frozen evidence, or packet change surface is classified before edits and may require a plan
  successor. User-owned dirty paths are preserved rather than silently adopted.
- **External candidate readiness.** Successor validation and activation are not self-hosted work.
  The separately governed proving commit, expected-predecessor compare-and-swap, durable recovery,
  plan-qualified overlap, and exact-candidate readiness receipt are approval prerequisites. A
  missing/mismatched/orphan state stops this plan and returns to that external owner.
- **Exact unstable provider APIs.** The plan intentionally targets current pinned APIs without a
  defensive semantic facade. Missing accepted facts, changed ownership/lifetimes, or unusable
  streaming bounds reopen the affected D-23/LD-25 decision rather than lowering fidelity.
- **DataFusion extension/provider/view honesty.** Default provider methods, copied sessions,
  pre-bound `ViewTable` plans, retained object-store registries, or incomplete custom nodes can
  discard optimizer, security, schema, statistics, rewrite/reset, resource, or cancellation
  contracts. Per-operation rung selection and bound-authority/production-path probes are mandatory.
- **Logical/physical schema loss.** Delta/Parquet may reconstruct logical fixed-width IDs or
  metadata as ordinary binary/storage values. WP27 and WP17 retain the current enforcing wrapper
  until phase-by-phase replacement proof; native provider adoption cannot precede it.
- **Derived-analysis incompleteness.** Raw provider acceptance does not implement CFG/dataflow/
  alias/effect/resource/summary semantics. WP23–WP25 and the accepted-family producer closure are
  blockers for dependent capabilities and queries.
- **Rust compilation trust.** Process isolation does not contain build scripts/proc macros. WP26
  must prove the real supported-platform launcher; unavailable containment fails closed or uses a
  separately authorized, visibly degraded `TRUSTED_LOCAL` posture.
- **Independent proof contamination.** Importers, producer output, legacy results, and generated
  Gate B artifacts cannot author expected semantics. WP22 freezes decoded evidence and causal
  controls before consumers; WP20 can only re-execute them.
- **Legacy comparison availability.** Shared-source changes can destroy the old reference engine.
  WP22 freezes it at the transition start; DB01 permits only a non-live no-reader archive and
  WP20 uses the isolated harness. DB08 deletes it at retention expiry.
- **Persisted-data compatibility.** Old Delta versions, activation/snapshot rows, result resources,
  and compiler releases may expose unknown migration cases. Each retained epoch reconstructs
  exactly or yields an accepted typed incompatibility before deletion/vacuum.
- **Deployment-journal and legacy-fence ambiguity.** The journal governs deployment phase and
  Delta governs semantic current; neither can revoke a binary that ignores them. A separately
  proved bridge/external authority must bind the exact release/activation/generation and deny old
  serving/writes across restart/reboot. Any mismatch fails closed.
- **Foreign package builds.** Generated Protobuf cache deletion depends on proved wheel/sdist and
  constrained/offline build behavior. A narrowly retained cache must remain derivable and cannot
  carry semantic authority.
- **Deletion incompleteness.** Hidden files, generated/binary artifacts, mixed files, dynamic
  registrations, macro expansion, string keys, packaging, build scripts, and platform `cfg` can
  evade a simple search. Derived inventory, unique selectors, skipped/unparsed accounting,
  compiler/package/feature proof, and permanent residue guards close the claim.
- **Cross-plan packet-ID aliasing.** The external governance-remediation proving commit must
  plan-qualify and backfill overlap identity before candidate approval. WP01 only verifies its
  receipt; a stale/missing/cross-plan record blocks entry and cannot be patched in this DAG.
- **Cutover irreversibility.** `NEW_MUTATING` forbids old-binary rollback. Admission-before-
  selection, the bridge/external exact-binary revocation, independent semantics, crash/reboot
  matrix, and forward recovery must be accepted before that transition.
- **Performance uncertainty.** More relational representation can increase planning time, memory,
  or update amplification. Measure first; tune partitions, statistics, caches, and materialization
  only inside the accepted logical/authority contract.

### 9.2 Replan levels

- **Implementation adaptation:** exact helper/module names, native plan shapes, batch sizing,
  partitioning, statistics collection, spill thresholds, and other choices that stay within D-20
  through D-35 and LD-17 through LD-26. Record the adaptation and evidence in execution state.
- **Plan revision:** packet boundaries/order, newly discovered consumers, an additional durable
  crash phase, selector/decommission scope, a package-cache exception, or materially different
  proof obligations. Stop the affected dependency cone, write a new immutable plan version (or
  amend only while this artifact remains unapproved draft), and re-audit.
- **Design reopening:** any second semantic authority, dual production write, runtime fallback,
  public physical query surface, Python semantic state, multi-host writer, weakened exact-provider
  fidelity, unproved current pointer, judgment facts, or change to a D/LD/target invariant.
  Stop execution and produce a new accepted design before replanning.

### 9.3 Execution stop conditions

Stop the affected packet and propagate staleness when a named library API is absent, the preflight
surface is materially larger than the packet can leave coherent, an acceptance oracle cannot
discriminate fail/unknown, a dependency-closed proving commit is impossible, a deletion selector
is uncovered/overlapping/unparsed, old/new comparison lacks independent truth, a raw/derived
family lacks exactly one producer/remainder, schema meaning cannot be restored, untrusted
compilation cannot be contained, the exact old binary can regain service/write authority,
retained data cannot be classified, or authority would be dual at any point.

The plan remains `draft`. This planning turn creates no state file, changes no active-plan
pointer, and authorizes no implementation or cutover.
