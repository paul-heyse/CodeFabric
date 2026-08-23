---
artifact: implementation-plan
plan_id: codefabric-model-driven-artifact-and-assurance-control-plane
version: v1
date: 2026-08-22
status: approved
design_path: docs/designs/codefabric_model_driven_artifact_and_assurance_control_plane_design_v1_2026-08-22.md
design_version: v1
baseline_commit: 6b42f33b7de72044b40939f7d86b5dee8888d06c
working_tree_digest: 782d35cec162fe8651b4e36c87ae01d289c1ad2b2d6e0ad2f00460135171f7de
state_path: docs/plans/state/codefabric-model-driven-artifact-and-assurance-control-plane_v1_state.json
suspends_plan_path: docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md
cutover: true
---

# CodeFabric model-driven artifact and assurance control plane — implementation plan v1

This plan fully realizes the accepted target design. It replaces the current authored catalog,
hidden native-source mutation, path/ID-specific generation, duplicated proof manifests, and
packet-specific mutation campaigns with one typed repository model, read-only planning surface,
family-native derivation drivers, crash-consistent reconciler, content-addressed incremental
execution, and model-derived assurance graph.

The user request to produce this plan is the acceptance event for the dossier's
`accepted-with-named-assumptions` decision. The dossier frontmatter is correspondingly
`status: accepted`. The user's execution request approves this plan and authorizes its
schema-2 execution state.

The plan is a temporary execution overlay over the active Waves 4–7 plan v4. On approval for
execution, it becomes the sole mutable execution program. Waves 4–7 v4 and its schema-2 state are
frozen as read-only history: WP27–WP31 retain their proving commits; WP32 remains unproved; its
dirty source/syntax work is preserved and absorbed by WP06 here. No second agent may continue
WP32–WP53 concurrently. WP14 prepares and release-certifies a reconciled Waves 4–7 successor;
WP15 activates it through the sealed terminal handoff only after M04.

---

## 1. Outcome and non-goals

### 1.1 Outcome

At M05:

1. Existing-family additions are data-only: no central artifact/output list, package-data list,
   digest edit, proof manifest, Rust match, Python path map, or packet recipe changes.
2. A handwritten-only model compiler discovers current authorities, evidence, and acceptances;
   compiles a closed `RepositoryModel`; plans a complete `DesiredTree`; explains dependency and
   proof closure; and builds without any generated production output.
3. Every supported generated family—canonical/provenance views, registries and CBEF bindings,
   provider raw-kind catalogs, schema/TableSpec/DDL projections, Pydantic/FastMCP views, and the
   single-FDS Protobuf bindings—executes through typed `describe/plan/render` drivers.
4. `model-check` is read-only. `model-sync` is the sole routine writer for `Derived` paths and is
   guarded by source fences, exact output ownership, worktree-aware reader/writer locking, durable
   recovery state, independent staged validation, and crash recovery before supported readers.
5. Cache-disabled full, warm-cache, affected-closure, corrupt-cache recovery, and two-root builds
   produce identical path censuses and bytes. Cache entries are disposable optimization state,
   never authority or stored pass verdicts.
6. An assurance graph derives live Just, Rust, Python, fixture, rule, requirement, and output
   relationships. `edit`, `changed`, `tier-a`, and `release` profiles widen conservatively and are
   proved against the former full suite before it is retired.
7. Mandatory `mutants-wp*` campaigns and their permanent packet infrastructure are gone.
   `mutants-file` remains outside every profile as an optional human Tier-C diagnostic.
8. CBEF domain layouts, registry/flag allocations, TableSpec row encoders, adapter views, and
   generated package aggregators are model-enforced; the current WP32 recipe/allocation divergence
   is removed.
9. The authored suite manifest, embedded computed digests, generated requirements/traceability,
   bundle membership, toolchain projection, `PUBLIC_SCHEMA_ARTIFACTS`, artifact-ID dispatch, old
   generator chain, and authored proof-coverage manifest have reached their specified zero states.
10. A versioned Waves 4–7 successor preserves WP27–WP53 functional outcomes and dependencies,
    reconstructs progress from proving commits and current gates, replaces mutation obligations
    with model profiles, and becomes the only plan authorized to resume core-facts execution after
    a separate, sealed handoff milestone.

### 1.2 Non-goals

- No new Cargo package, workspace, external build system, persistent graph database, or daemon.
- No replacement of Cargo, Just, Nextest, Pytest, Maturin/uv, Pydantic, Protobuf, Arrow, or the
  adopted canonicalization stack.
- No automatic acceptance of release census, KAT, registry allocation, signature, schema/Proto
  compatibility baseline, or breaking change.
- No implementation of the remaining Waves 4–7 product outcomes beyond the WP32 work explicitly
  absorbed to close the current identity/allocation defect.
- No removal of committed generated outputs during this migration.
- No performance gate based on wall-clock timing, cache hit rate, mutation score, or coverage
  percentage. Performance investigation remains separate from correctness.
- No Ubuntu clean-host requirement and no license work; both remain user-deferred.

### 1.3 Baseline and active-program boundary

The baseline commit is HEAD `6b42f33b7de72044b40939f7d86b5dee8888d06c`; the frontmatter
working-tree digest covers the pre-plan dirty tree, including in-progress WP32 edits and the
accepted dossier. Those changes are user work and must not be discarded, reset, or treated as
proof of this plan.

The planning-session baseline `just ci-fast` passed the Rust, doctest, extractor, sidecar,
adapter, and structural portions and then failed at `artifacts-check` because the active Waves
4–7 v4 plan carries a stale Data Fabric declared-input digest. Record that failure summary as a
pre-existing baseline failure when execution state is initialized, without copying a derived
digest or command output. Adoption of this plan must
make the plan relationship unambiguous before broad gates are used as completion evidence.

---

## 2. Source design and declared inputs

The accepted dossier is the architecture authority. The suite and domain specifications remain
normative for product semantics; WP01 integrates the explicitly accepted corrections before code
cutover. The Waves 4–7 v4 plan supplies the functional WP27–WP53 outcomes that must be preserved,
not the superseded mutation or catalog mechanics.

| path | sha256 |
|---|---|
| docs/designs/codefabric_model_driven_artifact_and_assurance_control_plane_design_v1_2026-08-22.md | ed0659c19351f4a78d96ad8c9628ba5f759fce5e357dc75c3d604ca533e49ae8 |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 1b7cef4af1923268992b4d410e147960173e8460f17ad108e42cf37c50a8388c |
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | 805828f9303cd960040d826b95e443d1e54361965db4458c9489df00453a9f3d |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | ab03ef5b46a254023f52ea67df49c2bbe8eeaa00b7437b304ae36d297edc9e7a |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | a827e36ee6724a0a929396a21ad3e274f016248c09b854d0788c446fec7793df |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 39a1df3058076806862dd15842415b5451d804fae824fc0dcbf6abe5727cf6a8 |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | e045c7a62a45bfd9cf0f7f6387fb080be2f4767c44cfb4eec94f3eca2c2658b3 |
| docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | 47fe764943b9a7afe942252e23bb4ba8145c877dfd2556cb4d46a809248d277a |
| docs/rust_core_python_interface_repository_specification_2026-08-20.md | 42678a93d6c323d3c527255c2f266b2520bd13dca485c1f1d4af7991a9243848 |
| docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md | 5e4bc67347f3f3fbc89564210dd3894ed4c078e30ac618fe621755fe72d71795 |

Input freshness is derived by `just artifacts-check`; this table is never restamped in place.
WP01 intentionally evolves only the five normative paths it names. At state initialization, the
executor records those exact paths as one accepted `planned_design_input_evolution` owned by WP01,
using the dossier acceptance and this plan approval as authority. The evolution becomes effective
only after WP01 has a trusted ancestor proving commit; `artifacts-check` remains a post-judgment
M01 gate. Any other declared-input change is stale and requires replanning.

---

## 3. Global target invariants

- **G-01 — single compiled truth:** one immutable `RepositoryModel` owns the current source,
  evidence, acceptance, derivation, output, requirement, oracle, profile, and provenance graph.
- **G-02 — distributed authorship:** native semantic sources and irreducible acceptance are
  authored; aggregate inventories, identities, memberships, reverse indexes, and output censuses
  are generated.
- **G-03 — exact current-byte inventory:** every present tracked, staged, or untracked governed
  path is claimed once or rejected. Git/notify information is a hint/classification; stable current
  filesystem bytes plus CodeFabric BLAKE3 establish truth.
- **G-04 — independent released absence:** an owner-accepted census outside routine generation
  protects released IDs; deletion needs an accepted tombstone or major transition.
- **G-05 — typed, acyclic graph:** nodes and edges are closed variants; illegal endpoints,
  duplicate producers, cycles, unsafe paths, and output/source overlap fail before rendering.
- **G-06 — acyclic bootstrap:** the model compiler links no generated production surface and
  builds with every production generated output absent.
- **G-07 — one-way generation:** routine commands never edit authorities, evidence, acceptance,
  KATs, signatures, allocations, or compatibility baselines.
- **G-08 — complete plan before apply:** `DesiredTree`, staged validators, isolated consumer
  builds, output census, and source recheck all pass before repository writes begin.
- **G-09 — crash-consistent supported view:** recovery state survives `cargo clean`; supported
  readers and writers share a worktree-aware lock and recover before consumption.
- **G-10 — incremental equals full:** action keys include semantic/exact inputs, upstream output
  digests, output specs, exact resolved toolchain/executable/feature identity, and environment.
- **G-11 — verify before skip:** a cache hit also requires every output, matching content digests,
  and the exact output census; cached verdicts cannot bypass current validation.
- **G-12 — family-native semantics:** Serde, Pydantic, descriptor APIs, Arrow, and JSON Schema own
  their native legality; generic code coordinates rather than reimplements them.
- **G-13 — model-enforced APIs:** CBEF recipes, registry/flag codes, TableSpec encoders, adapter
  projections, Proto packages, and package aggregators cannot drift into handwritten siblings.
- **G-14 — independent assurance:** a renderer cannot generate its expected KAT or certify its
  own selected proof closure; unknown reads widen to full evidence.
- **G-15 — no packet infrastructure:** no permanent recipe, source manifest, proof ID, or cache
  namespace is named after an implementation packet.
- **G-16 — explicit acceptance:** only a guarded acceptance command after owner review may change
  release census, compatibility, KAT, signature, or allocation acceptance.
- **G-17 — one active program and one writer:** Waves 4–7 v4 stays frozen during this plan; the old
  generator remains the sole writer during shadow parity; after cutover the reconciler is the sole
  routine writer; a Waves successor activates only through WP15/M05 after M04.

Doctrine disposition:

- **Advances:** P10 Declarative Knowledge Single-Sourcing, P11 Parse Don’t Validate, P12 Illegal
  States Unrepresentable, P14 Staged Compilation, P17 Functional Core/Imperative Shell, P25
  Reproducibility and Semantic Incrementality, P27 Provenance, P30 Testability, and P31 Additive
  Extensibility/Executable Governance.
- **Maintains:** P1/P2 information hiding and concern separation, P5/P7 dependency direction and
  acyclicity, P8 least privilege, P13 stable semantic identity, P16 contracts, and P21
  command/query separation.
- **Risk — mitigated:** P20/P22/P24 transaction, resource lifecycle, and idempotency are protected
  by explicit lock/journal ownership, fault injection, recovery-before-read, and exact retry state.

---

## 4. Work packets

### WP01 — Adopt normative corrections and freeze the active program

#### Outcome

The suite, domain design, roadmap evidence doctrine, and execution governance describe the
accepted model-driven target without contradicting it. Waves 4–7 v4 is explicitly frozen as
read-only history for the duration of this plan; WP27–WP31 evidence remains valid, WP32 remains
unproved, and no concurrent mutable state or product execution path exists.

#### Dependencies

None.

#### Target Invariants

G-02, G-04, G-07, G-14, G-15, G-16, G-17.

#### Design and Library References

- Dossier §§1.5, 2, 5.1, 5.2 and 6.8.
- SUITE AC-G-02, AC-G-04, AC-G-05, AC-G-07; ONT/GEN CBEF and allocation ownership;
  FAB Contract IR ownership; LIFE current-byte and recovery roles; SRV adapter-model ownership.
- Doctrine P10, P13, P16, P19, P21, P29, P31.

#### Change Surface

##### Preflight Query

```bash
just spec-outline docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md --match '^(2|3|4|5|7)\.' --view expanded
just spec-outline docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md --match '^(0|2|6|28)\.' --view expanded
rg -n 'mutants-wp|suite-manifest|canonical_digest|source_digest|proof-coverage|Cbef|provider_node_flags' docs/upfront_design docs/plans docs/plans/state -g '!docs/library_ref/**'
jq '{status,current_packet,wp32:.packets.WP32}' docs/plans/state/codefabric-waves-4-7-core-facts_v4_state.json
```

##### Known Touch (verified this session)

- SUITE, RM, ONT, GEN, and FAB, the five normative artifacts changed by dossier §1.5.
- `docs/plans/active-plan.json`, whose single pointer must move to this plan at adoption and to
  the reconciled Waves successor only through WP15/M05.
- `docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md` and
  `docs/plans/state/codefabric-waves-4-7-core-facts_v4_state.json` as immutable source evidence.
- `tooling/ci/artifact_contracts.py` and its tests, which currently diagnose plan freshness.

#### Required Changes

- Integrate the accepted AC-G-02/04/05/07 corrections and the source identity, requirement,
  catalog, bundle, and release-census ownership rules into their normative owners.
- Specify generated recipe-aware CBEF builders and governed occurrence/flag allocations in ONT,
  GEN, and FAB where those semantics are consumed.
- Replace mandatory packet mutation doctrine in RM and all future active evidence tables with
  model/metamorphic/KAT/consumer/recovery proof; retain generic mutation only as optional Tier C.
- Record this plan as the sole execution overlay. Do not mutate v4 execution judgments or copy its
  derived evidence. At execution adoption, atomically point `docs/plans/active-plan.json` at this
  plan and initialize only this plan's schema-2 state; the v4 state remains immutable history.
- In that initial state, record one `planned_design_input_evolution` owned by WP01 for exactly
  SUITE, RM, ONT, GEN, and FAB. Keep the planning-time digests immutable. After the normative
  correction commit passes focused design/governance proof, record it as WP01's proving commit;
  only then may the repository artifact checker accept those five evolved inputs.
- Define the certification and sealed-handoff rules that WP14 and WP15 must satisfy before a Waves
  4–7 successor can become active.
- Add `just model-design-contract-check` to validate the normative ownership and active-program
  constraints without encoding per-artifact lists.
- Encode and test the external-driver security contract: a cleaned environment with credential and
  proxy variables stripped, no declared network capability, resolved input/output plans, staging
  confinement, and source-fence detection of out-of-plan repository writes. Treat an available OS
  sandbox as defense in depth, not as the portable correctness boundary.

#### Legacy Disposition and Decommission

The v4 functional outcomes and proving history are preserved. Its mutable execution path and
mutation-based evidence mechanics are suspended, not silently reinterpreted. Full supersession is
DB06/WP14 work after the replacement gates exist.

#### Acceptance Checks

##### Behavioral

- `just model-design-contract-check`

##### Structural

- `just governance-scan`
- `model_wp01_planned_input_evolution_names_exactly_five_accepted_paths`

##### Negative / Zero-State

- `model_active_program_is_unique` test: v4 cannot be selected while this plan executes.
- `model_design_rejects_routine_acceptance_writes` test.

##### Operational

- `model_wp01_state_transition_enables_post_judgment_artifact_freshness`

##### Executable Oracle Catalog

- Executable oracle: `just model-design-contract-check`
- Executable oracle: `just governance-scan`
- Executable oracle: `model_active_program_is_unique`
- Executable oracle: `model_wp01_state_transition_enables_post_judgment_artifact_freshness`

#### Edit-Local Gates

- `just typos`
- `just governance-scan`

#### Packet-Local Gates

- `just model-design-contract-check`
- `just governance`

#### Integration Milestone

M01.

#### Replan Triggers

- A normative owner cannot express the correction without changing product semantics beyond the
  accepted dossier.
- Artifact governance cannot represent one suspended plan and one mutable overlay without two
  active states.
- Preserving v4 proving history requires rewriting its state or commit identities.

#### Rollback or Recovery

No product code changes. If design integration is rejected, leave v4 and its state untouched and
mark this plan blocked; do not partially adopt the evidence doctrine.

#### Design-Bearing Contracts and Exemplars

The active-program relation is one-way:

```text
Waves 4–7 v4 (read-only history) -> this remediation (sole mutable state)
                                  -> Waves 4–7 successor (activated at WP15/M05)
```

### WP02 — Establish the generated-output-free compiler bootstrap

#### Outcome

A narrow model-compiler binary and feature surface builds in an isolated root after all production
generated outputs are omitted, does not link the production library surface, and has exact
dependency/toolchain identity. The bootstrap graph is acyclic and independently executable.

#### Dependencies

WP01.

#### Target Invariants

G-05, G-06, G-10, G-12, G-17.

#### Design and Library References

- Dossier §§2.1, 2.2 I-17, 3.2, 3.4, LD-01, LD-02, LD-03, LD-08, LD-09.
- `petgraph` 0.8.3 and resolved Serde/canonicalization pins.
- Doctrine P5, P7, P14, P17, P25, P30.

#### Change Surface

##### Preflight Query

```bash
sed -n '1,210p' Cargo.toml
ast-grep outline src/bin src/contracts --items imports --view signatures
rg -n 'include!|include_bytes!|src/generated|contracts/generated|codefabric-contracts|contracts-tooling|fact-generation' src Cargo.toml justfile scripts tooling -g '!target/**'
cargo tree --locked --no-default-features --features contracts-tooling -e features
```

##### Known Touch (verified this session)

- `Cargo.toml`, `Cargo.lock`, `src/bin/codefabric-contracts.rs`, `src/contracts/mod.rs`.
- `src/contracts/catalog.rs`, `compiler.rs`, `models.rs`, and `jcs.rs` as current typed substrate.
- Existing generator commands in `justfile`, `scripts/contracts_repro_check.sh`, and
  `tooling/contracts/derivation.py` expose the current feature-sensitive binary race.

#### Required Changes

- Add exact optional
  `petgraph = { version = "=0.8.3", default-features = false, features = ["std"], optional = true }` to a
  dedicated model-compiler feature with only the required narrow surface.
- Introduce a dedicated model-compiler binary feature/target whose handwritten source modules do
  not import or link production generated consumers or the root library target.
- Make that feature enable `dep:gix` directly for its read-only adapter rather than composing
  through `repository-state`; cfg-gate all generated consumers in `src/lib.rs`, registries,
  contract index, and repository-state modules out of the compiler closure.
- Define stable-ID newtypes, closed model/edge/output/diagnostic enums, resource bounds, and the
  driver protocol needed by later packets; keep I/O adapters outside the functional core.
- Model the compiler build itself as an upstream action whose inputs include compiler sources,
  exact lock resolution, toolchain, target triple, and feature set.
- Give compiler executables isolated target/artifact paths by complete build identity; never
  execute a shared mutable `target/debug/codefabric-contracts` path.
- Add `just model-bootstrap-check`, including a clean isolated-root build with production
  generated outputs intentionally absent and a negative dependency scan.
- Deny Arrow/DataFusion/Delta, PyO3, tonic runtime, rusqlite, provider/fact-stack, and generated
  include closure dependencies from the bootstrap feature graph.

#### Legacy Disposition and Decommission

The existing `codefabric-contracts` binary remains the only writer during shadowing. Its feature
coupling and shared-path execution are replaced only after WP10/M02 parity; DB03 owns final zero
state.

#### Acceptance Checks

##### Behavioral

- `model_bootstrap_builds_without_generated_outputs`
- `model_diagnostic_classes_are_stable_and_bounded`

##### Structural

- `just model-bootstrap-check`
- `just stable-graph-check`
- `just features-each`

##### Negative / Zero-State

- `model_bootstrap_has_no_generated_or_production_library_edge`
- `model_action_identity_distinguishes_feature_and_toolchain_builds`

##### Operational

- `model_feature_distinct_compilers_do_not_share_executable_path`

##### Executable Oracle Catalog

- Executable oracle: `just model-bootstrap-check`
- Executable oracle: `just stable-graph-check`
- Executable oracle: `model_bootstrap_has_no_generated_or_production_library_edge`
- Executable oracle: `model_feature_distinct_compilers_do_not_share_executable_path`

#### Edit-Local Gates

- `just root-fmt`
- `cargo check --locked --no-default-features --features <model-compiler-feature> --bin <model-compiler-bin>`

#### Packet-Local Gates

- `just model-bootstrap-check`
- `just stable-graph-check`
- `just features-each`
- `just deps-fast`
- `just policy`

#### Integration Milestone

M01.

#### Replan Triggers

- Cargo necessarily compiles the production library or a generated output for the selected binary.
- Petgraph's narrow direct feature set pulls DataFusion or conflicts with the stable graph.
- The compiler core cannot be shared without source inclusion tricks that create duplicate type
  identities or a second package.

#### Rollback or Recovery

Keep the new surface read-only and unreferenced by existing generation aliases. Removal is safe if
the bootstrap boundary cannot be proven; no authority or generated output changes in this packet.

#### Design-Bearing Contracts and Exemplars

```text
compiler sources + Cargo.lock + toolchain
  -> isolated model-compiler executable
  -> generated outputs
  -> isolated production-consumer validation
```

### WP03 — Compile exact inventory, family claims, and the typed graph

#### Outcome

The compiler derives a normalized `RepositoryModel` from stable current filesystem bytes across
closed roots, with byte-safe worktree topology, exact family claiming, typed graph validation,
deterministic order, and gix-optional equivalence. In read-only shadow mode it explains every
current catalog artifact/derivation/output or reports a classified mismatch.

#### Dependencies

WP02.

#### Target Invariants

G-01, G-02, G-03, G-05, G-12.

#### Design and Library References

- Dossier §§3.1–3.3, LD-01, LD-02, LD-03, LD-10, 6.1.
- gix 0.86.0 read-only adapter and LIFE current-byte authority/fallback doctrine.
- Doctrine P8, P10–P14, P17, P25, P27.

#### Change Surface

##### Preflight Query

```bash
ast-grep outline src/contracts src/secure_path.rs src/git_state.rs src/inventory.rs --items exports --view signatures
jq -r '.artifacts[].semantic_projection_source.source_kind, .derivations[].derivation_kind' contracts/manifests/suite-manifest.json | sort | uniq -c
rg -n 'CATALOG_PATH|ContractCatalog|CompiledCatalog|ArtifactDescriptor|DerivationUnitDescriptor|artifact_for_path|derivation_order|GitRepoPath|PlatformPath' src tests tooling scripts -g '!target/**'
rg --files contracts docs/upfront_design rules rule-tests tooling/proto | LC_ALL=C sort
```

##### Known Touch (verified this session)

- `src/contracts/catalog.rs` currently owns typed descriptors, a BTree-based derivation order,
  resolved invocations, and the authored catalog bootstrap.
- `src/secure_path.rs`, `src/git_state.rs`, and `src/inventory.rs` already expose byte-safe paths,
  read-only gix DTOs, and current inventory seams.
- `contracts/manifests/suite-manifest.json` currently enumerates 66 artifacts, seven derivations,
  and 60 output paths and is the temporary parity oracle only.

#### Required Changes

- Define the fixed, closed `FamilyRule` registry: root/extension claims, native parser, default
  policy, output convention, budgets, and independent validators—never individual members.
- Inventory fixed roots from current stable bytes, including tracked, staged, untracked, ignored,
  conflicted, deleted, symlink, case-collision, non-UTF-8, and linked-worktree cases.
- Use gix only for topology/classification/candidate acceleration and detached DTOs; prove bounded
  filesystem fallback yields identical model and bytes.
- Compile closed nodes/edges and use `petgraph::DiGraph` plus external stable-ID maps for cycle,
  topological, and reverse-dependency operations. Orient dependency edges prerequisite to
  dependent, use outgoing traversal for affected closure and reversed traversal for predecessor
  explanations, and do not persist `NodeIndex`.
- Define duplicate-edge policy explicitly. Use `toposort` for detection/order, then deterministic
  SCC/DFS witness projection to report sorted stable node IDs and typed internal edge kinds;
  Petgraph's single cycle node is insufficient as the public diagnostic.
- Add family/header models that retain stable identity/version/status while deriving owner,
  compatibility, consumers, bundles, resource policy, and conventional outputs.
- Add shadow comparison against the current catalog without reading it as an input to the new
  model; classify missing, extra, or intentionally corrected relationships.
- Add `model-explain` for sources, claims, edges, and mismatches and `just model-inventory-check`.

#### Legacy Disposition and Decommission

The authored catalog remains the old writer's bootstrap and an independent temporary parity
artifact. It is never imported into the new compiler. DB01 removes bootstrap authority only after
accepted census and complete family parity.

#### Acceptance Checks

##### Behavioral

- `model_inventory_classifies_tracked_staged_untracked_and_ignored`
- `model_graph_rejects_illegal_edges_duplicates_and_cycles`
- `model_graph_order_is_insertion_invariant`

##### Structural

- `just model-inventory-check`
- `just governance-scan`

##### Negative / Zero-State

- `model_inventory_rejects_symlink_escape_case_collision_and_unclaimed_paths`
- `model_gix_failure_falls_back_without_semantic_drift`
- `model_graph_indices_never_serialize`

##### Operational

- `model_linked_worktree_inventory_uses_current_bytes`
- `model_inventory_diagnostics_stay_within_budgets`

##### Executable Oracle Catalog

- Executable oracle: `just model-inventory-check`
- Executable oracle: `model_graph_rejects_illegal_edges_duplicates_and_cycles`
- Executable oracle: `model_gix_failure_falls_back_without_semantic_drift`
- Executable oracle: `model_linked_worktree_inventory_uses_current_bytes`

#### Edit-Local Gates

- `just root-fmt`
- Targeted model inventory/graph tests.

#### Packet-Local Gates

- `just model-inventory-check`
- `just model-bootstrap-check`
- `just root-clippy`
- `just root-test`

#### Integration Milestone

M01.

#### Replan Triggers

- A current authority family cannot self-identify without a central member list or a bounded typed
  adjacent record.
- Generic filesystem fallback cannot represent a governed path class accepted by gix.
- Shadow mismatch reveals current product semantics not covered by the accepted design.

#### Rollback or Recovery

Read-only shadow code can be disabled without affecting the old compiler or product. Preserve all
mismatch fixtures and record any design-level divergence before changing an authority.

#### Design-Bearing Contracts and Exemplars

```text
RepositoryModel {
  sources, evidence, acceptances, derivations, outputs,
  requirements, oracles, profiles, typed_graph
}

FamilyRule = describe + claim + parse + defaults + plan_convention + validators
```

### WP04 — Establish and obtain acceptance for the released-artifact census

#### Outcome

A compact versioned acceptance schema protects released stable IDs independently of every routine
generated output. The compiler produces a review candidate, a human owner explicitly approves the
seed census through the guarded acceptance path, and routine check/sync code is structurally
incapable of changing it.

#### Dependencies

WP03.

#### Target Invariants

G-02, G-04, G-07, G-16, G-17.

#### Design and Library References

- Dossier §§3.1, 5.1 step 5, 6.1, assumption A-03.
- SUITE AC-G-05 corrected ownership; doctrine P8, P13, P16, P21, P29.

#### Change Surface

##### Preflight Query

```bash
jq -r '.artifacts[] | select(.status == "released") | .artifact_id' contracts/manifests/suite-manifest.json | LC_ALL=C sort
rg -n 'tombstone|owner_acceptance|accepted|release.*census|model-accept|artifact-index' contracts src tooling scripts justfile -g '!target/**'
ast-grep outline src/contracts/models.rs src/bin --items structure --view signatures
```

##### Known Touch (verified this session)

- `contracts/manifests/suite-manifest.json` and the packaged artifact index currently contain the
  released census but are routine generated/mutated surfaces and therefore not independent.
- `src/contracts/models.rs` already carries owner-acceptance and artifact-status types that can be
  reused without making generated output authoritative.

#### Required Changes

- Define a closed acceptance record containing only version, suite major, released stable IDs and
  status, accepted tombstone references, owner identity, and acceptance provenance.
- Generate a candidate from the WP03 model into a review-only location; routine generation cannot
  write the accepted destination.
- Add a guarded `model-accept release-census` command that validates candidate/current
  compatibility and requires explicit owner invocation. It does not run from any gate.
- Pause execution at the human checkpoint. The owner reviews the candidate and explicitly
  authorizes acceptance; the executor must not infer approval from this plan.
- Add `just model-release-census-check` to verify schema, release completeness, tombstone rules,
  path separation, and non-writability from `model-sync`.

#### Legacy Disposition and Decommission

The current suite manifest remains the old compiler's released-census authority until the human
checkpoint and M01 pass. The generated artifact index never becomes the replacement oracle.

#### Acceptance Checks

##### Behavioral

- `model_release_census_blocks_unaccepted_deletion`
- `model_release_census_allows_additive_unreleased_candidate`

##### Structural

- `just model-release-census-check`
- `model_acceptance_paths_are_outside_routine_write_set` governance test.

##### Negative / Zero-State

- `model_sync_cannot_write_release_census`
- `model_generated_index_deletion_cannot_erase_released_history`

##### Operational

- `model_release_census_candidate_requires_explicit_accept_command`

##### Executable Oracle Catalog

- Executable oracle: `just model-release-census-check`
- Executable oracle: `model_release_census_blocks_unaccepted_deletion`
- Executable oracle: `model_sync_cannot_write_release_census`
- Executable oracle: `model_release_census_candidate_requires_explicit_accept_command`

#### Edit-Local Gates

- Schema parse plus focused acceptance tests.

#### Packet-Local Gates

- `just model-release-census-check`
- `just model-inventory-check`
- `just contracts-verify`

#### Integration Milestone

M01. M01 cannot pass until the owner-accepted record exists and
`just model-release-census-check` is green.

#### Replan Triggers

- Maintainers reject assumption A-03 or require signatures/external release infrastructure beyond
  the accepted design.
- The current released set cannot be reconciled without an unreviewed deletion or ID rewrite.
- Routine sync code must gain authority over the acceptance path.

#### Rollback or Recovery

Before owner acceptance, discard only the generated review candidate. After acceptance, changes
require a new explicit acceptance event; never rewrite the accepted record to repair a gate.

#### Design-Bearing Contracts and Exemplars

```text
ReleasedArtifactCensus = accepted(released stable IDs, statuses, tombstone refs)
model-sync write set     = Derived only
model-accept write set   = one reviewed Acceptance kind
```

### WP05 — Plan the complete desired tree and expose read-only commands

#### Outcome

The compiler derives typed output specifications, complete action identities, deterministic
affected closure, a staged `DesiredTree`, source fences, and structured `model-explain`,
`model-plan`, and `model-check` commands. It remains shadow/read-only; the old generator is still
the sole repository writer.

#### Dependencies

WP03, WP04, M01.

#### Target Invariants

G-01, G-05, G-06, G-08, G-10, G-11, G-12, G-17.

#### Design and Library References

- Dossier §§3.4–3.6, 3.9, LD-01–LD-03, LD-07–LD-09, 6.2–6.3.
- Doctrine P14, P17, P21, P24, P25, P27, P30.

#### Change Surface

##### Preflight Query

```bash
ast-grep outline src/contracts/catalog.rs src/contracts/artifacts.rs src/bin/codefabric-contracts.rs --items structure --view signatures
rg -n 'DerivationInput|ResolvedDerivationInvocation|DerivationOutput|resource_budget_profile|generate\(|verify\(|write_atomic|rename|target/debug/codefabric-contracts' src tooling scripts justfile -g '!target/**'
jq -r '.derivations[] | [.derivation_id,.derivation_kind,(.inputs|length),(.outputs|length)] | @tsv' contracts/manifests/suite-manifest.json
```

##### Known Touch (verified this session)

- `src/contracts/catalog.rs::resolved_invocation` currently drops output inputs from its resolved
  artifact-input list and exposes under-specified output records.
- `src/contracts/artifacts.rs::generate` is a fixed full-family sequence and direct writer.
- `src/bin/codefabric-contracts.rs` has generate/resolve/verify commands but no generic model plan.

#### Required Changes

- Define typed `PlannedOutput` projection variants that own Pydantic mode/model roots, schema
  public identity, Proto role, registry primary key, TableSpec projection, consumers, validators,
  and safe path convention.
- Resolve artifact inputs and upstream output content digests into every invocation.
- Define canonical action models/keys from driver/rule/schema versions, semantic and exact inputs,
  upstream output digests, normalized output specs, exact lock-resolved executable/toolchain/
  feature identity, environment, and profile.
- Use reverse petgraph traversal for affected closure and stable tie-breaking for plans.
- Build a complete immutable `DesiredTree` in staging, with one producer per path and explicit
  Add/Replace/DeleteStale/Unchanged comparison. Do not write the repository.
- Add source-generation fence identities and isolated overlay-tree construction as interfaces;
  consumer execution arrives after all drivers in M02.
- Define a closed, tracked `TransitionConsumerPatch` format under the explicitly temporary
  `tooling/model-transition/consumer-overlays/` root. Each family patch carries the governed target
  path, expected planning-baseline bytes/source identity, reviewed replacement bytes, family
  owner, and compatibility intent; the compiler derives its content identities on demand. The
  patch root is transition evidence, never routine authority, and is a mandatory WP11/DB03 zero
  state after promotion.
- Add validity-by-construction Proptest model/graph/edit strategies with bounded sizes and no
  repository regression-file persistence.
- Add `model-explain`, `model-plan`, `model-check`, and `just model-plan-check`.
- Add exact dev-only
  `proptest = { version = "=1.11.0", default-features = false, features = ["std"] }` for pure
  reference-model tests. Disable persistence; bound cases, depth, size, and rejection counts; use
  fixed replayable seeds in gates. Never spawn Cargo or Python once per property-test case.

#### Legacy Disposition and Decommission

Current derivation descriptors and generator output lists remain temporary parity inputs only to
the old compiler. No path map is deleted until family drivers plan the same complete tree at M02.

#### Acceptance Checks

##### Behavioral

- `model_action_key_changes_for_every_output_affecting_input`
- `model_affected_closure_matches_full_recomputation`
- `model_desired_tree_classifies_add_replace_delete_stale_unchanged`

##### Structural

- `just model-plan-check`
- `model_planned_output_variants_are_closed`

##### Negative / Zero-State

- `model_rejects_duplicate_output_unsafe_path_and_missing_upstream_output`
- `model_cache_entry_never_contains_pass_verdict`

##### Operational

- `model_plan_is_insertion_order_invariant_and_bounded`
- `model_explain_reports_source_lineage_consumers_and_oracles`
- `model_cycle_witness_is_stable_for_self_multi_node_and_parallel_kind_cycles`

##### Executable Oracle Catalog

- Executable oracle: `just model-plan-check`
- Executable oracle: `model_action_key_changes_for_every_output_affecting_input`
- Executable oracle: `model_affected_closure_matches_full_recomputation`
- Executable oracle: `model_desired_tree_classifies_add_replace_delete_stale_unchanged`

#### Edit-Local Gates

- `just root-fmt`
- Targeted action/DesiredTree/property tests with bounded cases.

#### Packet-Local Gates

- `just model-plan-check`
- `just model-inventory-check`
- `just model-bootstrap-check`
- `just root-clippy`
- `just root-test`

#### Integration Milestone

M02.

#### Replan Triggers

- A driver cannot declare all outputs before rendering.
- Full action identity requires ambient state that cannot be declared or isolated.
- Proptest cannot generate legal models without rejection-heavy filtering or unbounded cost.

#### Rollback or Recovery

All commands remain read-only. Remove the shadow CLI registration if the action protocol must be
reopened; the old writer and committed outputs remain untouched.

#### Design-Bearing Contracts and Exemplars

```text
ActionKey = b3(JCS(driver + schemas + inputs + upstream outputs + output specs
                   + exact executable/toolchain/features + environment + profile))

DesiredTree[path] = bytes + role + producer + lineage + output_kind + content_digest
```

### WP06 — Pilot the driver protocol with registries, CBEF, and governed allocations

#### Outcome

The first end-to-end family driver compiles registry authorities, CBEF domain recipes, provider
raw-kind catalogs, occurrence families, and flag allocations into recipe-aware Rust/Python views.
An isolated overlay proves the exact refit required for the in-progress WP32 implementation so
illegal positional identity layouts, duplicate relation meanings, and raw production code/flag
literals become unrepresentable at cutover. This packet is strictly shadow/read-only: it neither
changes live production consumers nor claims completion of Waves 4–7 WP32.

#### Dependencies

WP05.

#### Target Invariants

G-01, G-05–G-08, G-12–G-14, G-17.

#### Design and Library References

- Dossier §§3.7–3.10, 4.1–4.4, 5.1, 6.3, and LD-04.
- SUITE AC-G-02/04/05/07; ONT and GEN CBEF recipes and fact allocation ownership.
- Canonicalization pack for typed ingress/JCS/BLAKE3; doctrine P10–P14, P17, P27, P31.

#### Change Surface

##### Preflight Query

```bash
rg -n 'IdentityDomain::(Entity|RelationFact)|Cbef|semantic_key|occurrence_family_code|provider_node_flags|EXPLICITLY_PARENTHESIZED|PARSE_ERROR_AT|MISSING_AT' src contracts docs/upfront_design -g '!docs/library_ref/**'
ast-grep outline src/identity.rs src/fact_ingest.rs src/source_syntax.rs src/registries.rs src/ruff_adapter.rs src/contracts/registry_models.rs --items structure --view signatures
rg -n 'registry|cbef|provider.*kind|allocation' contracts/manifests/suite-manifest.json contracts/registry contracts/identity tooling/contracts justfile
```

##### Known Touch (verified this session)

- Dirty WP32 surfaces: `src/identity.rs`, `src/fact_ingest.rs`, `src/source_syntax.rs`,
  `src/registries.rs`, `src/ruff_adapter.rs`, `src/lib.rs`, and its synthetic fixture/manifests.
- `contracts/identity/cbef-v1.yaml` governs ENTITY as five fields and RELATION_FACT as six; the
  current dirty generic construction emits twelve and eight.
- Registry authority and its generated Rust/Python/JSON/index/bundle projections, plus
  `src/contracts/registry_models.rs` and existing registry generation.

#### Required Changes

- Finalize the closed driver protocol: `describe(model)`, `plan(invocation)`, and
  `render(plan, staging_root)`, with declared reads, complete outputs, resource profile,
  deterministic diagnostics, and no repository-root writer capability.
- Implement the registry/CBEF driver from typed native authorities. Generate domain-specific
  builders and validators that accept named semantic fields and emit the exact selected recipe;
  keep any generic codec private to validated compiler internals.
- Generate typed registry enums, lookup tables, flag masks, occurrence-family accessors, and
  allocation provenance. Remove arbitrary production positional CBEF construction and raw numeric
  code/bit allocation from runtime source.
- Build and validate the proposed WP32 refit only in the isolated overlay: use the governed ENTITY
  semantic-key and RELATION_FACT role recipes; represent parenthesization, parse-error, and missing
  nodes through `syntax_detail`/`source_annotation`; and preserve useful source-location,
  reconciliation, and disposition behavior. Materialize the reviewed consumer change as a tracked
  typed transition patch based on the plan frontmatter baseline; state records only the judgment
  that WP06 absorbed the pre-existing work, never a diff digest. Do not edit live WP32 consumers in
  WP06.
- Render into staging, validate with an independent typed decoder/KAT and isolated consumer build,
  and compare paths/bytes/semantics to the old generator. The old writer remains authoritative.
- Specify generic negative governance rules, each with a paired rule test, for direct authority
  writes, arbitrary positional CBEF production construction, and raw governed code/flag literals.
  Exercise them against positive/negative overlay fixtures now; enforce them against live source in
  WP11 after the dependency-closed promotion.
- Add the parameterized `model-family-check family=""` recipe here, with `registry-cbef` as the
  first family and an empty value reserved for the aggregate behavior completed in WP10.

#### Legacy Disposition and Decommission

Existing registry/CBEF output lists, live runtime construction, and dispatch branches remain
unchanged shadow oracles through M02. The staged candidate and generated APIs are promoted
together in WP11; the old generator writer is removed only by WP14/DB02–DB05.

#### Acceptance Checks

##### Behavioral

- `model_cbef_builders_match_every_governed_recipe`
- `model_overlay_wp32_occurrences_preserve_governed_identity_and_annotation_semantics`
- `model_registry_round_trip_preserves_codes_flags_and_tombstones`

##### Structural

- `just model-family-check registry-cbef`
- `model_registry_generated_consumers_compile_and_typecheck`

##### Negative / Zero-State

- `model_cbef_rejects_entity_twelve_field_and_relation_eight_field_layouts`
- `model_cbef_rejects_wrong_missing_extra_and_reordered_recipe_operands`
- `model_overlay_rejects_raw_governed_codes_and_flags`
- `model_registry_driver_cannot_write_authority_or_kat`

##### Operational

- `model_registry_driver_rejects_out_of_plan_output_and_repository_write`
- `model_registry_driver_environment_strips_credentials_and_proxy_settings`

##### Executable Oracle Catalog

- Executable oracle: `just model-family-check registry-cbef`
- Executable oracle: `model_cbef_builders_match_every_governed_recipe`
- Executable oracle: `model_overlay_wp32_occurrences_preserve_governed_identity_and_annotation_semantics`
- Executable oracle: `model_registry_driver_rejects_out_of_plan_output_and_repository_write`

#### Edit-Local Gates

- `just root-fmt`
- Targeted CBEF, registry, source-syntax, and rule tests.

#### Packet-Local Gates

- `just model-family-check registry-cbef`
- `just fixture-check`
- `just root-clippy`
- `just root-test`
- `just proof-coverage-check`

#### Integration Milestone

M02.

#### Replan Triggers

- The accepted CBEF recipe cannot encode a required WP32 semantic without lossy overloading.
- A registry family requires hidden member enumeration rather than a family declaration.
- Driver confinement cannot detect source/authority changes made by an external tool.

#### Rollback or Recovery

Keep the new driver read-only and preserve staged parity evidence. If the recipe itself is
insufficient, revert only the unproved WP32 adaptation, reopen the normative design, and do not
expand the generic encoder's public authority.

#### Design-Bearing Contracts and Exemplars

```text
EntityBuilder::new(workspace, context, kind_code, owner_id, semantic_key)
RelationFactBuilder::new(workspace, context, relation_code, source_id, target_id, role)

syntax detail / source annotation != a second relation allocation
```

### WP07 — Derive schema, TableSpec, DDL, Arrow, and row-encoder views

#### Outcome

Contract IR and typed TableSpecs become the sole semantic source for validation schemas, public
JSON Schema, SQLite DDL, Arrow schema construction, snapshot/table projections, and generated row
encoders. Native library validation proves every view, and no hand-maintained field list or
`PUBLIC_SCHEMA_ARTIFACTS`-style include list remains authoritative.

#### Dependencies

WP06.

#### Target Invariants

G-01, G-02, G-05–G-08, G-12–G-14, G-17.

#### Design and Library References

- Dossier §§3.7–3.10, 4.1–4.5, 6.3, LD-04, LD-06.
- FAB Contract IR, TableSpec, Arrow, SQLite, and DataFusion ownership sections.
- Arrow/DataFusion pinned references; independent Draft 2020-12 `jsonschema` consumer.

#### Change Surface

##### Preflight Query

```bash
rg -n 'PUBLIC_SCHEMA_ARTIFACTS|TableSpec|ArrowSchema|SchemaRef|CREATE TABLE|json_schema|schema_artifact|row_encoder|RecordBatch' src tooling contracts codefabric-cpg-mcp scripts justfile -g '!target/**'
ast-grep outline src/contracts/schema_models.rs src/contracts/schema_artifacts.rs src/schema_registry.rs src/fact_ingest.rs tooling/contracts/json_schema_check.py --items structure --view signatures
jq -r '.derivations[] | select(.derivation_kind|test("schema|table|ddl")) | .derivation_id' contracts/manifests/suite-manifest.json
```

##### Known Touch (verified this session)

- `src/contracts/schema_models.rs`, `schema_artifacts.rs`, catalog/compiler models, generated schema
  outputs, TableSpecs, and adapter/public schema package data.
- Existing `PUBLIC_SCHEMA_ARTIFACTS` and manually routed schema projection surfaces.
- Current independent Python `jsonschema` validation and Rust Arrow/schema consumers.

#### Required Changes

- Define family-native typed Contract IR and TableSpec projection descriptors. Every field's name,
  stable ID, type, nullability, ordering, key/constraint role, compatibility, and consumer role is
  represented once.
- Implement staged schema drivers for canonical validation schema, public JSON Schema, SQLite DDL,
  Arrow `Schema`, table/snapshot specifications, and generated Rust row encoders.
- Use Arrow-native schema/type/metadata APIs and SQLite parsing/execution for legality; use the
  independent pinned Python Draft 2020-12 validator for public schema conformance. Generic
  orchestration must not reimplement either library.
- Generate static source only where compile-time exhaustiveness or package importability needs it;
  reuse typed/module-scoped values elsewhere. Derive public schema/package views from output roles,
  not a manual list.
- Prove row encode/decode round trips in the isolated overlay for nullability, nested/list/map/union-like projections,
  stable field order, primary/foreign key fields, and unknown-field rejection where contracts are
  closed.
- Stage and independently validate all outputs before comparison to committed production views;
  keep old generation aliases read-only/shadow through M02.
- Store any required handwritten schema/TableSpec consumer migration as a typed tracked transition
  patch; never reconstruct it from `target/` or execution state.

#### Legacy Disposition and Decommission

Manual schema/include lists and sibling row encoders become compatibility oracles during shadow
parity. Their authority is removed at M02 and their source zero state is enforced in WP14/DB02.

#### Acceptance Checks

##### Behavioral

- `model_tablespec_projects_equivalent_arrow_json_schema_and_ddl`
- `model_row_encoder_round_trips_every_supported_field_shape`
- `model_schema_compatibility_diagnostics_are_path_aware`

##### Structural

- `just model-family-check schemas`
- `just schema-check`
- `model_schema_arrow_and_ddl_consumers_compile`
- `model_schema_real_datafusion_consumer_constructs_every_tablespec`

##### Negative / Zero-State

- `model_schema_rejects_unknown_duplicate_and_incompatible_fields`
- `model_schema_outputs_have_one_producer_and_no_manual_public_include_list`
- `model_driver_cannot_generate_compatibility_acceptance`

##### Operational

- `model_schema_stage_builds_without_repository_mutation`

##### Executable Oracle Catalog

- Executable oracle: `just model-family-check schemas`
- Executable oracle: `model_tablespec_projects_equivalent_arrow_json_schema_and_ddl`
- Executable oracle: `model_schema_real_datafusion_consumer_constructs_every_tablespec`
- Executable oracle: `model_driver_cannot_generate_compatibility_acceptance`

#### Edit-Local Gates

- `just root-fmt`
- Targeted schema/TableSpec/row-encoder tests.

#### Packet-Local Gates

- `just model-family-check schemas`
- `just schema-check`
- `just contracts-verify`
- `just root-clippy`
- `just root-test`

#### Integration Milestone

M02.

#### Replan Triggers

- One semantic field must be independently authored in more than one schema family.
- Arrow or SQLite legality cannot be decided from the typed TableSpec without an accepted semantic
  addition to Contract IR.
- A public schema consumer depends on output ordering or metadata not represented in the model.

#### Rollback or Recovery

Keep committed schemas and old writer unchanged. Disable the family driver and retain its staged
diff if a native validator exposes a design gap; never patch one generated view manually.

#### Design-Bearing Contracts and Exemplars

```text
Contract IR + TableSpec
  -> validation schema
  -> public JSON Schema
  -> SQLite DDL
  -> Arrow Schema
  -> generated row encoder
```

### WP08 — Derive strict Pydantic and FastMCP adapter views

#### Outcome

The Python adapter's strict frozen models, validation and serialization schemas, module-scoped
TypeAdapters, FastMCP tool fingerprints, and package exports are projections of Contract IR. No
independent handwritten adapter schema authority or hot-loop model construction remains.

#### Dependencies

WP07.

#### Target Invariants

G-01, G-02, G-05–G-08, G-10–G-14, G-17.

#### Design and Library References

- Dossier §§3.7–3.10, 4.4–4.5, 6.3, LD-06.
- SRV adapter contract pipeline and exact Pydantic/FastMCP pins.
- Pydantic 2.13.4 `BaseModel`, `ConfigDict`, `TypeAdapter`, validation/serialization schema modes;
  FastMCP 3.4.7 programmatic tool discovery and fingerprinting.

#### Change Surface

##### Preflight Query

```bash
rg -n 'BaseModel|ConfigDict|TypeAdapter|model_json_schema|FastMCP|Tool\.model_fields|fingerprint|generated.*contract|package-data' codefabric-cpg-mcp tooling/contracts pyproject.toml justfile -g '!**/.venv/**'
ast-grep outline tooling/contracts/generate_adapter_models.py codefabric-cpg-mcp/src --items structure --view signatures
uv run --project codefabric-cpg-mcp python -c 'from mcp.types import Tool; print(sorted(Tool.model_fields))'
```

##### Known Touch (verified this session)

- `tooling/contracts/generate_adapter_models.py` already emits strict frozen Pydantic models,
  module-scoped TypeAdapters, dual schema modes, and JCS fingerprints.
- Adapter generated model/schema/package-data surfaces, contract tests, wheel tests, and FastMCP
  tool registration/fingerprint evidence.
- Current Python callback into the Rust generator and explicit schema/include paths.

#### Required Changes

- Implement the adapter driver as an external exact-pinned tool with a fully resolved input/output
  plan, isolated environment/target, staging-root-only output, and executable-byte identity.
- Drive strict frozen model, aliases, validators/serializers, discriminated unions, field order,
  validation schema, serialization schema, and `TypeAdapter` construction solely from Contract IR
  projection descriptors.
- Keep adapters module-scoped and reused. Prohibit runtime/hot-loop dynamic class, schema, or
  adapter construction and independent handwritten protocol fields.
- Derive package exports and package-data inclusion from output roles. Build a wheel from the
  staged overlay and assert installed import origin, model behavior, schema fingerprints, and data
  census before parity acceptance.
- Discover FastMCP tools programmatically, freeze the accepted client-visible `Tool` field profile,
  and derive deterministic fingerprints without a parallel hand-maintained tool manifest.
- Prove the generated adapter handlers and existing public handler behavior are equivalent in the
  isolated wheel/STDIO consumer, including error mapping and strict request/response boundaries.
- Add unknown/missing/mistyped/alias/strictness and validation-vs-serialization mutation proofs;
  KAT answers and accepted fingerprints remain outside the driver write surface.
- Store any required handwritten adapter consumer/import migration as a typed tracked transition
  patch consumed by the isolated wheel overlay.

#### Legacy Disposition and Decommission

The existing generator implementation is retained as a semantic parity oracle until M02, but no
second adapter schema authority is created. Python-to-Cargo generation callbacks and manual
package lists are removed only after staged wheel parity and WP14 zero-state proof.

#### Acceptance Checks

##### Behavioral

- `model_adapter_round_trips_all_contract_ir_shapes_strictly`
- `model_validation_and_serialization_schema_modes_are_distinct_and_stable`
- `model_fastmcp_fingerprint_changes_only_for_client_visible_contract_changes`
- `model_adapter_generated_and_public_handlers_are_equivalent`

##### Structural

- `just model-family-check adapter`
- `just adapter-contracts-check`
- `just adapter-contracts-governance`

##### Negative / Zero-State

- `model_adapter_rejects_unknown_missing_mistyped_and_lax_values`
- `model_adapter_has_no_independent_schema_authority_or_hot_loop_construction`
- `model_adapter_driver_cannot_write_kats_or_acceptance`

##### Operational

- `just adapter-wheel-test`
- `model_adapter_external_driver_has_exact_executable_and_environment_identity`

##### Executable Oracle Catalog

- Executable oracle: `just model-family-check adapter`
- Executable oracle: `just adapter-contracts-check`
- Executable oracle: `model_adapter_generated_and_public_handlers_are_equivalent`
- Executable oracle: `just adapter-wheel-test`

#### Edit-Local Gates

- `just adapter-lint`
- `just adapter-type`
- Targeted adapter driver/tests.

#### Packet-Local Gates

- `just model-family-check adapter`
- `just adapter-contracts-check`
- `just adapter-contracts-governance`
- `just adapter-contracts-repro-check`
- `just adapter-ci-fast`
- `just adapter-wheel-test`

#### Integration Milestone

M02.

#### Replan Triggers

- A FastMCP-visible field cannot be derived from Contract IR and the accepted fingerprint profile.
- Pydantic validation and serialization schemas require incompatible semantic sources.
- The staged wheel cannot consume generated outputs without repository-root mutation.

#### Rollback or Recovery

Keep the existing generated adapter package intact and disable the shadow driver. Preserve the
staged wheel and fingerprint diff as evidence; do not hand-edit generated Python.

#### Design-Bearing Contracts and Exemplars

```text
Contract IR -> frozen Pydantic models -> module TypeAdapters
            -> validation schema + serialization schema
            -> FastMCP client-visible fingerprint
```

### WP09 — Derive the single-FDS Protobuf and gRPC family

#### Outcome

All governed Proto sources form typed compilation units that invoke exact-pinned
`grpc_tools.protoc` once per unit to produce the sole descriptor set and Python bindings. Rust
uses that same FDS through `tonic_prost_build::Builder::compile_fds`; descriptors, bindings,
census, package views, and interop evidence are model-planned without per-domain command branches.

#### Dependencies

WP08.

#### Target Invariants

G-01, G-02, G-05–G-08, G-10–G-14, G-17.

#### Design and Library References

- Dossier §§3.7–3.10, 4.1, 4.4–4.5, 6.3, LD-05.
- FAB/SRV Proto and daemon boundary contracts.
- grpcio-tools 1.83.0, grpcio 1.83.0, Protobuf 7.36.0 descriptor APIs; tonic/prost
  `compile_fds` integration.

#### Change Surface

##### Preflight Query

```bash
rg -n 'grpc_tools\.protoc|compile_fds|DescriptorPool|DESCRIPTOR|wave0_probe|SOURCE_RELATIVE|descriptor.*pb|proto.*source' tooling/proto src/generated tests codefabric-cpg-mcp contracts justfile scripts -g '!target/**'
ast-grep outline tooling/proto/generate.py tooling/proto/generate.rs tests/integration/rpc.rs --items structure --view signatures
jq -r '.derivations[] | select(.derivation_kind|test("proto"))' contracts/manifests/suite-manifest.json
```

##### Known Touch (verified this session)

- `tooling/proto/generate.py`, `generate.rs`, FDS/census/toolchain/compatibility artifacts,
  generated Python/Rust bindings, adapter package data, and RPC integration tests.
- The Wave-0 substrate already uses one grpcio-tools FDS and Rust `compile_fds`, but paths and
  compilation-unit coverage still contain Wave-0-specific assumptions.
- Current shared Cargo executable paths can race across feature sets.

#### Required Changes

- Derive typed Proto compilation units from claimed sources, imports, source roots, output roles,
  consumers, resource profile, and exact compiler identity; reject missing/duplicate inputs,
  import escapes, output multi-ownership, cycles, and package collisions.
- Invoke `grpc_tools.protoc` once per unit into staging for Python bindings and a deterministic FDS.
  Decode/re-encode and feed that exact FDS to Rust `compile_fds`; never invoke a second semantic
  Proto compiler.
- Replace Wave-0/domain filename dispatch and `SOURCE_RELATIVE` constants with output-role and
  descriptor-derived paths. Include all input semantic/source identities in provenance.
- Generate normalized typed descriptor census and compare to independent compatibility acceptance:
  files, packages, imports/options including unknown option wire, services/cardinality, messages,
  fields/presence/oneofs, enums, and reservations.
- Validate staged Python descriptor pool/generated modules, Rust compilation, wire round trips,
  deadlines/status/message limits, unknown-field retention, and cross-language interop.
- Use an action-specific isolated Cargo target and exact compiler executable identity. Add every
  produced binding/FDS/census path to the complete DesiredTree before any apply.
- Store any required handwritten Rust/Python Proto consumer migration as a typed tracked transition
  patch; descriptor/binding outputs themselves remain DesiredTree entries.

#### Legacy Disposition and Decommission

Existing Wave-0 generation is the substrate/parity oracle, not production-family completion.
Hard-coded Wave-0 names and old Proto scripts/aliases remain shadow-only until M02, then become
WP14/DB02–DB03 zero-state targets.

#### Acceptance Checks

##### Behavioral

- `model_proto_one_fds_drives_python_and_rust_equivalently`
- `model_proto_descriptor_census_covers_all_semantic_compatibility_dimensions`
- `model_proto_cross_language_round_trip_preserves_presence_oneofs_and_unknowns`

##### Structural

- `just model-family-check proto`
- `just proto-check`
- `just proto-repro-check`

##### Negative / Zero-State

- `model_proto_rejects_import_escape_duplicate_output_and_package_collision`
- `model_proto_has_one_descriptor_compiler_identity`
- `model_proto_plan_contains_no_wave0_filename_dispatch`

##### Operational

- `model_proto_feature_distinct_rust_consumers_use_isolated_executables`

##### Executable Oracle Catalog

- Executable oracle: `just model-family-check proto`
- Executable oracle: `model_proto_one_fds_drives_python_and_rust_equivalently`
- Executable oracle: `model_proto_descriptor_census_covers_all_semantic_compatibility_dimensions`
- Executable oracle: `just proto-repro-check`

#### Edit-Local Gates

- Targeted Proto generator and descriptor tests.
- `just root-fmt`

#### Packet-Local Gates

- `just model-family-check proto`
- `just proto-check`
- `just proto-repro-check`
- `just root-test`
- `just adapter-ci-fast`

#### Integration Milestone

M02.

#### Replan Triggers

- One governed Proto family requires a second semantic compiler or descriptor interpretation.
- Descriptor APIs cannot recover a required output/package relationship represented only by a
  hidden filename convention.
- Compatibility acceptance would need to be generated by the driver it evaluates.

#### Rollback or Recovery

Keep current committed FDS and bindings, disable the shadow compilation unit, and preserve the
descriptor diff. Never regenerate one language independently to obtain parity.

#### Design-Bearing Contracts and Exemplars

```text
sorted Proto inputs -> grpc_tools.protoc -> Python bindings + sole FDS
sole FDS            -> tonic_prost_build::compile_fds -> Rust bindings
```

### WP10 — Derive governance, provenance, aggregate, and package views

#### Outcome

Detached source identities, provenance, suite manifest, packaged artifact index, requirements,
traceability, fixture index, bundles, toolchain identity, package-data lists, module aggregators,
and reverse indexes are derived from the RepositoryModel and accepted release census. All family
drivers participate in one complete staged tree, and shadow parity passes without giving the new
system repository write authority.

#### Dependencies

WP09.

#### Target Invariants

G-01–G-08, G-10–G-17.

#### Design and Library References

- Dossier §§3.1, 3.5–3.10, 4.1–4.5, 5.1–5.2, 6.3, LD-01, LD-04, LD-07.
- SUITE AC-G-02/04/05/07 canonical/source/bundle identity and governance ownership.
- Canonicalization pack and independent cross-language JCS/BLAKE3 KATs.

#### Change Surface

##### Preflight Query

```bash
rg -n 'sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests|artifact-index|suite-manifest|fixture.*index|package-data|PUBLIC_SCHEMA_ARTIFACTS|include_bytes!|generated.*mod' src/contracts tooling scripts codefabric-cpg-mcp justfile -g '!target/**'
ast-grep outline src/contracts/artifacts.rs src/contracts/index.rs src/contracts/compiler.rs tooling/ci/artifact_contracts.py --items structure --view signatures
find contracts -type f -maxdepth 4 | LC_ALL=C sort
```

##### Known Touch (verified this session)

- `src/contracts/artifacts.rs` currently sequences native mutators and generated views.
- Suite manifest, artifact index, requirements, traceability, bundles, fixture oracles/index,
  toolchain identity, generated module aggregators, and package-data inclusion.
- Independent canonicalization KATs, compatibility acceptances, signatures, and registry
  allocations that must remain outside routine write surfaces.

#### Required Changes

- Derive source and canonical identities as detached projections. Never write computed identity
  fields back into semantic source authorities; retain nested/member/evidence digests required by
  their profile while omitting only the governed artifact's own identities.
- Derive suite manifest and artifact index from model nodes/edges and accepted release census;
  include source/semantic identities, owner, status, compatibility, compilation unit, producer,
  consumers, output role, resource profile, and provenance without becoming bootstrap inputs.
- Derive requirements and traceability from closed machine-readable requirement/test markers and
  the assurance relationships declared by families. Opaque read sets widen rather than guessing.
- Require every released requirement to resolve to normative source, implementation/output, and at
  least one executable oracle or explicit accepted deferral; require every mandatory released
  semantic node to have a requirement path. Derive reverse indexes only after that closure passes.
- Derive typed AC-G-07 bundles from artifact-sorted members and the closed bundle model; validate
  required fields, compatibility, created-by, nested digests, signatures, duplicates, and empty
  member policy independently.
- Derive fixture index, toolchain identity, package-data views, Rust/Python module aggregators, and
  all reverse indexes from output roles. Remove the need for hand-maintained include lists.
- Assemble every family output into one complete `DesiredTree`, run all independent validators,
  compare a two-root staging reproduction, and build isolated Rust/Python/Proto/schema consumers.
- Compare to the old full generator at the semantic and byte levels. Every divergence is either a
  named accepted correction or a blocker; the old generator remains the sole writer through M02.
- Compile every tracked transition patch into the isolated overlay from its declared baseline,
  reject overlaps/stale bases/undeclared targets, and emit a complete candidate consumer census.
  M02 must be reproducible using only tracked authorities, evidence, acceptances, driver code, and
  these temporary patches—never disposable staging or state-carried digests.
- Add `just model-repro-check` and the aggregate `just model-family-check` capability.

#### Legacy Disposition and Decommission

The authored catalog, embedded digest mutators, proof manifest, old generator chain, and manual
aggregators remain bounded shadow oracles through M02. After M02 they lose authority but are not
physically removed until the sole-writer and assurance replacements pass.

#### Acceptance Checks

##### Behavioral

- `model_detached_identity_matches_independent_rust_and_python_kats`
- `model_bundle_projection_matches_typed_ac_g_07_semantics`
- `model_two_root_full_tree_is_path_and_byte_identical`
- `model_isolated_overlay_consumers_build_and_validate`
- `model_tracked_transition_patches_reconstruct_exact_consumer_overlay`

##### Structural

- `just model-family-check`
- `just model-repro-check`
- `just contracts-verify`

##### Negative / Zero-State

- `model_routine_tree_excludes_authority_evidence_acceptance_and_signature_paths`
- `model_rejects_missing_duplicate_or_multi_owner_outputs`
- `model_generated_aggregates_have_no_manual_member_list_input`
- `model_transition_patch_rejects_stale_base_overlap_and_undeclared_target`
- `model_released_traceability_has_source_implementation_and_executable_oracle_closure`

##### Operational

- `model_shadow_run_leaves_worktree_unchanged`
- `model_driver_failure_keeps_staged_diagnostics_and_never_applies_partial_output`

##### Executable Oracle Catalog

- Executable oracle: `just model-family-check`
- Executable oracle: `just model-repro-check`
- Executable oracle: `model_detached_identity_matches_independent_rust_and_python_kats`
- Executable oracle: `model_released_traceability_has_source_implementation_and_executable_oracle_closure`

#### Edit-Local Gates

- Targeted governance/provenance/bundle/package tests.
- `just typos`

#### Packet-Local Gates

- `just model-family-check`
- `just model-repro-check`
- `just contracts-verify`
- `just contracts-repro-check`
- `just fixture-check`
- `just proto-repro-check`
- `just adapter-contracts-repro-check`
- `just adapter-wheel-test`

#### Integration Milestone

M02.

#### Replan Triggers

- A governed aggregate cannot be derived without reading a generated aggregate as authority.
- A compatibility/KAT/signature surface would enter routine generation.
- Isolated consumers require repository mutation or reveal incomplete output planning.
- A parity difference cannot be traced to an accepted design correction.

#### Rollback or Recovery

Discard staged trees and keep the old writer authoritative. Preserve structured parity reports;
do not synchronize only the outputs that happened to match.

#### Design-Bearing Contracts and Exemplars

```text
native sources + evidence + accepted census + typed family declarations
  -> RepositoryModel
  -> complete DesiredTree
  -> detached provenance/index/requirements/trace/bundles/package views
```

### WP11 — Cut over to the crash-consistent reconciler and one writer

#### Outcome

The cache-disabled reconciler becomes the sole routine writer for `Derived` paths. In the same
dependency-closed promotion packet, the reviewed handwritten consumer migrations proved in
WP06–WP10 and their complete generated DesiredTree become live together under a bounded one-time
cutover bridge. The reconciler validates staging, obtains one worktree-aware exclusive lock,
rechecks sources, commits through a durable journal with per-destination same-filesystem
replacements, and recovers before any supported reader. `model-sync` is unavailable until the
promotion, transaction, and reader migration suite is green; old and new writers are never
concurrently enabled.

#### Dependencies

WP10, M02.

#### Target Invariants

G-03, G-07–G-09, G-11, G-16–G-17.

#### Design and Library References

- Dossier §§3.5–3.6, 3.9, 5.1–5.2, 6.4, LD-09, LD-10.
- LIFE repository/worktree topology, current-byte authority, recovery-before-read, and watcher
  roles.
- gix 0.86.0 for read-only topology only; rustix 1.1.4 `flock`, `openat`, `renameat`, and `fsync`
  primitives; tempfile only for disposable staging.
- Doctrine P8, P17, P20–P24, P27, P29–P30.

#### Change Surface

##### Preflight Query

```bash
rg -n 'write_atomic|rename|fsync|flock|lock|journal|backup|tempdir|cargo clean|contracts-gen|adapter-contracts-gen|proto-gen|model-check' src scripts tooling justfile -g '!target/**'
ast-grep outline src/secure_path.rs src/git_state.rs src/contracts/artifacts.rs src/bin --items structure --view signatures
just --dump --dump-format json | jq '.recipes | with_entries(select(.key|test("check|gen|verify|repro|wheel|fixture|proto|adapter")))'
```

##### Known Touch (verified this session)

- Existing direct writers and generation aliases in `src/contracts/artifacts.rs`, tooling scripts,
  and `justfile`.
- Repository/worktree DTOs and secure-path primitives in `src/git_state.rs` and
  `src/secure_path.rs`.
- Supported readers include model commands, legacy verification aliases during transition,
  staged Rust/Python consumers, and proof collectors.

#### Required Changes

- Resolve the worktree's per-worktree Git directory and shared common directory explicitly. Store
  a durable private lock/journal/backup root outside `target/`; cache and staging remain disposable
  under `target/model-*`.
- Implement one outer lock owner with an inherited/internal no-reacquire protocol. Read-only
  supported commands take a shared lock and recover/await recovery before consumption; sync takes
  an exclusive lock. Nested Just/Cargo/Python invocations must not self-deadlock.
- Define the durable journal state machine with transaction ID, model/source identities, complete
  destinations, old/new digests, per-path phase, backup identity, and recovery version.
- After complete staging and independent validation, re-read every governed source and evidence
  identity. Abort on drift. Then create same-filesystem destination temporaries/backups, fsync new
  files and journal, atomically rename, fsync parent directories, record each step, and commit the
  journal before cleanup.
- Recovery is idempotent and establishes a complete old or complete new DesiredTree. Test kill
  points before/after every durable transition, stale locks, journal truncation, missing backup,
  repeated recovery, and `cargo clean` between failure and restart.
- Reject symlink substitution, path-type changes, source/output overlap, user edits to a generated
  destination since planning, and linked-worktree cross-talk.
- Promote the WP06–WP10 tracked handwritten consumer patches—including the corrected WP32 CBEF/source
  syntax consumers, TableSpec row consumers, adapter imports, Proto package consumers, and
  generated aggregator entry points—together with the complete DesiredTree. The one-time bridge
  accepts only the tracked reviewed patch records and candidate outputs proved at M02, holds the
  exclusive program lock, and leaves one dependency-closed proving commit.
- Enable the previously shadowed structural rules against live source. The promoted tree must have
  no arbitrary positional governed CBEF construction, raw governed code/flag allocation, manual
  generated include list, or consumer dependency on an output absent from DesiredTree.
- Delete the applied transition-patch root in the same dependency-closed cutover and prove no
  runtime/model input can still read it. The proving commit contains the promoted live sources and
  outputs, not a permanent migration manifest.
- Migrate all supported readers and legacy check aliases to the lock/recovery protocol. Only then
  disable old writer commands and enable confirm-guarded `model-sync`; temporary old generation
  names may become thin aliases to it but may not retain independent write code.
- Keep cache disabled for this packet. Add `just model-transaction-check`; require a cache-disabled
  full sync/check/repro sequence before M03.
- After the initial explicit promotion sync, require `model-plan` to report zero actions and a
  second explicit sync to be byte-idempotent. Gates exercise apply/failure paths only in isolated
  worktrees or fixtures; no read-only gate invokes synchronization.

#### Legacy Disposition and Decommission

At the packet's cutover point, the bounded promotion bridge and old writer are disabled in one
program change. Temporary legacy aliases may delegate to `model-sync` but cannot invoke old
generator implementations. Physical removal remains WP14/DB03 after assurance cutover.

#### Acceptance Checks

##### Behavioral

- `model_transaction_recovers_to_complete_old_or_new_tree_at_every_kill_point`
- `model_sync_adds_replaces_and_deletes_stale_outputs_exactly`
- `model_source_recheck_aborts_before_apply_on_drift`
- `model_promoted_consumers_and_desired_tree_compile_as_one_dependency_closed_state`
- `model_sync_then_plan_is_zero_and_second_sync_is_byte_idempotent`

##### Structural

- `just model-transaction-check`
- `model_supported_readers_share_one_lock_protocol`

##### Negative / Zero-State

- `model_sync_rejects_symlink_swap_user_edit_path_type_change_and_multi_owner_output`
- `model_old_writer_has_no_independent_enabled_entry_point`
- `model_one_time_promotion_bridge_is_disabled_after_cutover`
- `model_transition_consumer_patch_root_is_absent_after_promotion`
- `model_journal_and_backups_survive_cargo_clean`

##### Operational

- `model_nested_reader_writer_commands_do_not_deadlock`
- `model_linked_worktrees_do_not_share_per_worktree_transaction_state`
- `model_blocked_sync_never_exposes_a_mixed_tree_to_supported_readers`

##### Executable Oracle Catalog

- Executable oracle: `just model-transaction-check`
- Executable oracle: `model_transaction_recovers_to_complete_old_or_new_tree_at_every_kill_point`
- Executable oracle: `model_promoted_consumers_and_desired_tree_compile_as_one_dependency_closed_state`
- Executable oracle: `model_blocked_sync_never_exposes_a_mixed_tree_to_supported_readers`

#### Edit-Local Gates

- `just root-fmt`
- Targeted lock/journal/recovery tests under constrained Nextest groups.

#### Packet-Local Gates

- `just model-transaction-check`
- `just model-repro-check`
- `just model-family-check`
- `just ci-fast`
- `just adapter-wheel-test`

#### Integration Milestone

M03.

#### Replan Triggers

- Any supported reader cannot participate in recovery/locking before reading committed outputs.
- The selected durable location is shared incorrectly across linked worktrees or is removed by
  routine cleanup.
- A filesystem cannot provide same-filesystem atomic replacement for a governed destination.
- Nested command composition cannot avoid lock reacquisition without bypassing protection.

#### Rollback or Recovery

Before writer enablement, disable the new transaction entry point. After enablement, always run
the recorded recovery protocol first; restore the old writer only through an explicit rollback
transaction that proves a complete old tree and never permits simultaneous writers.

#### Design-Bearing Contracts and Exemplars

```text
shared reader: recover-or-wait -> lock shared -> read -> unlock
exclusive sync: recover -> lock exclusive -> plan/stage/validate/recheck -> journal/apply -> unlock
```

### WP12 — Add content-addressed incremental execution and differential proof

#### Outcome

Disposable action-result caching and resource-aware affected-closure scheduling accelerate checks
and sync without changing semantics. Cache-disabled full, cold, warm, partially evicted, corrupt,
and incremental runs are byte/path equivalent, and a pure reference model plus bounded real-family
corpus proves the optimized executor against full recomputation.

#### Dependencies

WP11, M03.

#### Target Invariants

G-03, G-05, G-08–G-12, G-14, G-17.

#### Design and Library References

- Dossier §§3.4, 3.8–3.9, 5.1, 6.5–6.6, LD-02, LD-08–LD-10.
- Petgraph 0.8.3 traversal and DAG APIs; Proptest 1.11.0 pure-model differential testing;
  notify only as an invalidation hint.
- Doctrine P14, P17, P24–P25, P28, P30.

#### Change Surface

##### Preflight Query

```bash
rg -n 'cache|ActionKey|affected|reverse.*depend|toposort|resource.*profile|notify|watch|CARGO_TARGET_DIR|sccache' src tooling scripts justfile .cargo -g '!target/**'
ast-grep outline src/contracts/catalog.rs src/inventory.rs src/git_state.rs --items structure --view signatures
just cache-stats
```

##### Known Touch (verified this session)

- Typed derivation graph/action planning from WP03/WP05 and cache-disabled reconciler from WP11.
- Existing resource budget profiles in the catalog substrate.
- notify/gix lifecycle seams and sccache/Cargo target conventions; none is semantic authority.

#### Required Changes

- Store immutable cache entries under `target/model-cache` keyed by the complete canonical action
  identity from WP05. Include driver/schema versions, exact semantic/source and upstream output
  digests, normalized outputs, executable bytes, lock/toolchain/feature/profile/target triple, and
  relevant normalized environment.
- On lookup, validate entry schema and manifest, content digest every cached output, require the
  exact output census, then copy/link only into staging. Never restore directly to governed paths
  and never cache a pass/fail verdict.
- Treat absent, corrupt, partial, incompatible, and evicted entries as misses. Quarantine or ignore
  corrupt entries; correctness must not depend on successful cache deletion.
- Compute affected closure from current exact/source/semantic changes plus transitive outgoing
  dependency edges. Unknown classification or undeclared read widens to the complete graph.
- Schedule deterministically by dependency order and declared resource class. Isolate conflicting
  Cargo target/executable identities or serialize them; do not infer safety from a binary path.
- Add `model-watch` as an explicit opt-in command. Notify events only prompt current-byte
  re-inventory; overflow/rescan/backend loss widens to full, and watch never becomes authority.
- Build a pure reference planner/executor oracle. Property strategies generate valid bounded
  models and typed edits; compare full vs affected nodes, outputs, digests, diagnostics, and
  stale-deletion sets. Gates use fixed seeds and print minimized edit/replay commands.
- Add a bounded real-family corpus covering every family. For each, exercise an applicable
  source-format-only edit, semantic field edit, rule/driver version edit, tool identity edit,
  output-schema version edit, and member add/delete, plus cache corruption, wrong feature-set
  executable, unknown read, and source change during execution. No Cargo/Python subprocess runs
  per Proptest case.
- Emit discovered/affected/rendered/cached/validator counts and conservative-fallback reasons as
  recomputed diagnostics only. Never store them as execution-state judgments or gates.
- Add `just model-incremental-check`; performance timing and hit rate remain diagnostic only.

#### Legacy Disposition and Decommission

The cache is optional and disposable. Existing sccache remains a compiler-object optimization and
does not substitute for model action identity. No legacy cache or stored gate verdict is imported.

#### Acceptance Checks

##### Behavioral

- `model_incremental_matches_full_for_fixed_property_seed_matrix`
- `model_cache_cold_warm_partial_corrupt_and_disabled_outputs_are_identical`
- `model_every_family_edit_class_matches_full_affected_outputs_and_oracles`
- `model_unknown_read_and_watch_loss_widen_to_full`

##### Structural

- `just model-incremental-check`
- `model_cache_manifest_contains_complete_action_identity_and_output_census`

##### Negative / Zero-State

- `model_cache_rejects_wrong_digest_missing_extra_or_incompatible_output`
- `model_cache_cannot_restore_to_repository_or_store_pass_verdicts`
- `model_scheduler_never_coexecutes_conflicting_executable_identities`
- `model_cache_wrong_feature_executable_is_an_explicit_miss`

##### Operational

- `model_watch_rescan_reconstructs_current_byte_model`
- `model_property_failure_prints_seed_minimized_edit_and_replay_command`

##### Executable Oracle Catalog

- Executable oracle: `just model-incremental-check`
- Executable oracle: `model_incremental_matches_full_for_fixed_property_seed_matrix`
- Executable oracle: `model_cache_cold_warm_partial_corrupt_and_disabled_outputs_are_identical`
- Executable oracle: `model_every_family_edit_class_matches_full_affected_outputs_and_oracles`

#### Edit-Local Gates

- `just root-fmt`
- Fixed-seed pure property tests and targeted cache tests.

#### Packet-Local Gates

- `just model-incremental-check`
- `just model-transaction-check`
- `just model-repro-check`
- `just root-clippy`
- `just root-test`

#### Integration Milestone

M04.

#### Replan Triggers

- A family has an output-affecting input absent from its declared action identity.
- The reference oracle cannot remain independent of optimized affected-closure logic.
- Correctness requires cache availability, cache cleanup, notify delivery, or measured timing.
- A driver cannot declare a safe resource class or isolated executable identity.

#### Rollback or Recovery

Disable cache/scheduling and run the cache-disabled full reconciler. Delete only disposable cache
state under the validated model-cache root; durable transaction state is unaffected.

#### Design-Bearing Contracts and Exemplars

```text
current bytes -> typed edit -> outgoing affected closure -> deterministic actions
cache hit = valid manifest AND exact census AND every output digest matches
```

### WP13 — Compile the live assurance graph and sound profiles

#### Outcome

The model derives requirement-to-code/output/oracle/evidence relationships and live collectors for
Just, Nextest, Pytest, fixtures, structural rules, package tests, and family validators. The
`edit`, `changed`, `tier-a`, and `release` profiles select conservative proof closure and are
independently shown equivalent to the retained full gate suite across a bounded perturbation
corpus. No profile runs mutation testing or trusts an authored proof manifest.

#### Dependencies

WP12.

#### Target Invariants

G-01, G-02, G-05, G-12, G-14–G-17.

#### Design and Library References

- Dossier §§3.3, 3.7, 3.9, 4.2–4.5, 5.1–5.2, 6.7, LD-07–LD-08.
- RM corrected evidence doctrine; repository assurance tiers and command contract.
- Petgraph typed assurance graph; Nextest/Pytest/Just machine-readable inventories;
  ast-grep/ripgrep structural and textual collectors.

#### Change Surface

##### Preflight Query

```bash
just --dump --dump-format json | jq '.recipes | keys'
cargo nextest list --message-format json-pretty --locked
uv run --project codefabric-cpg-mcp pytest --collect-only -q
rg --files rules rule-tests | LC_ALL=C sort
rg -n 'proof-coverage|requirement|oracle|mutants-wp|ci-fast|tier-a|changed|release' tooling/ci contracts justfile docs/plans -g '!docs/library_ref/**'
```

##### Known Touch (verified this session)

- `tooling/ci/proof-coverage.json`, `proof_coverage.py`, `artifact_contracts.py`, tests, and current
  packet mutation recipes.
- The live Just graph contains 96 recipes; rules and rule tests currently form 12 paired files.
- Existing `ci-fast`, family verification/repro checks, fixture governance, wheel/package tests,
  and the old proof manifest remain the independent shadow suite for this packet.

#### Required Changes

- Define closed requirement, oracle, evidence-node, evidence-edge, profile, and opaque-command
  models. Requirement/test co-location markers must be machine-parseable, stable, and independent
  of generated proof reports.
- Collect the live Just recipe DAG from `just --dump`, Rust tests from Nextest listing, doctest
  capability separately, Python tests from collection, structural rule/rule-test pairs, fixtures,
  family validators, wheel/package consumers, compatibility acceptances, and transaction/cache
  proofs.
- Require every driver/family to declare read sets, requirement relationships, or an explicit
  opaque boundary. Unknown reads, collection failure, dynamic discovery, or missing ownership
  conservatively widen to the containing full profile.
- Compile profiles: `edit` for direct local feedback, `changed` for affected semantic closure,
  `tier-a` for every meaningful change, and `release` for cache-disabled exhaustive certification.
  Profiles select capability names, never tool flags or packet IDs.
- Implement `model-check profile=<profile>` and `just model-assurance-check`. Preserve
  `mutants-file` as an explicit optional Tier-C command outside all profiles; delete no legacy
  proof surface yet.
- Construct an independent perturbation corpus over source, authority, evidence, acceptance,
  generated-output, toolchain, package, rule, fixture, unknown-read, and transaction changes.
  For every case, selected proof must detect the injected defect whenever the retained full suite
  detects it. Compare selected-vs-full outcomes without reading the generated profile report as
  its own oracle.
- Require every selected recipe to resolve and every emitted test selector to collect at least one
  current test. Removing or renaming a recipe, test, rule, fixture, source, requirement, or output
  must produce an explicit model error or conservative full fallback, never a smaller silent
  report.
- Preserve the current full recipes and authored proof-coverage manifest as bounded shadow oracles
  until profile-selection soundness, live-inventory failure modes, and gix-disabled equivalence
  pass. They are decommissioned only in WP14.

#### Legacy Disposition and Decommission

Packet mutation recipes stop being mandatory immediately when equivalent model profiles are
approved, but remain physically present and unused until WP14. The authored proof manifest is a
shadow comparison input only and cannot certify the new assurance graph.

#### Acceptance Checks

##### Behavioral

- `model_changed_profile_matches_full_detection_on_perturbation_corpus`
- `model_assurance_collects_just_rust_python_rule_fixture_and_package_evidence`
- `model_profiles_widen_on_unknown_or_failed_discovery`
- `model_every_selected_recipe_resolves_and_test_selector_collects_nonempty`

##### Structural

- `just model-assurance-check`
- `model_profiles_contain_capabilities_not_packet_or_tool_flag_names`

##### Negative / Zero-State

- `model_assurance_cannot_read_its_generated_report_as_oracle`
- `model_profiles_contain_no_mutants_command_or_score_threshold`
- `model_missing_rule_test_requirement_or_read_set_is_not_silently_ignored`
- `model_removed_or_renamed_evidence_node_cannot_shrink_report_silently`

##### Operational

- `model_live_collector_failure_has_stable_diagnostic_and_full_fallback`

##### Executable Oracle Catalog

- Executable oracle: `just model-assurance-check`
- Executable oracle: `model_changed_profile_matches_full_detection_on_perturbation_corpus`
- Executable oracle: `model_every_selected_recipe_resolves_and_test_selector_collects_nonempty`
- Executable oracle: `model_assurance_cannot_read_its_generated_report_as_oracle`

#### Edit-Local Gates

- Targeted assurance compiler and collector tests.
- `just proof-coverage-check` as retained shadow evidence.

#### Packet-Local Gates

- `just model-assurance-check`
- `just proof-coverage-check`
- `just ci-fast`
- `just policy`
- `just adapter-wheel-test`

#### Integration Milestone

M04.

#### Replan Triggers

- A full-suite-detected defect is missed by its selected profile and cannot be repaired by a typed
  dependency/read relationship or conservative widening.
- Test/recipe discovery is not machine-readable or stable enough to collect without member lists.
- Requirement markers become a second semantic authority or are generated by the proof compiler.
- Retiring mutation obligations would remove unique risk evidence not replaced by deterministic,
  metamorphic, KAT, consumer, fault, or differential proof.

#### Rollback or Recovery

Keep profile commands read-only and fall back to the retained full recipe suite. Do not delete the
old proof manifest or packet recipes until the independent soundness corpus is green.

#### Design-Bearing Contracts and Exemplars

```text
changed paths -> RepositoryModel affected closure -> assurance graph -> capability set
unknown read/discovery -> widen, never guess
```

### WP14 — Decommission legacy authority and certify a read-only release candidate

#### Outcome

All legacy authority and writer paths named by DB01–DB05 reach structural, textual, behavioral,
and compile-time zero state. After one separately confirmed synchronization, a strictly read-only
cache-disabled release check rebuilds the model compiler without generated outputs, performs
recovery, requires zero planned repository actions, derives and validates the entire tree in two
roots, proves incremental/full and selected/full equivalence, validates packaging/consumers, and
leaves a clean current model. A versioned Waves 4–7 successor is prepared and accepted but remains
inactive; WP15 owns the non-circular program handoff.

#### Dependencies

WP13.

#### Target Invariants

G-01–G-17.

#### Design and Library References

- Dossier §§5.2–5.3 and 6.6–6.8.
- SUITE/RM corrected governance and evidence doctrine; repository artifact/state schemas.
- Waves 4–7 v4 functional packet DAG and historical proving commits.

#### Change Surface

##### Preflight Query

```bash
git status --short
rg -n 'sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests|PUBLIC_SCHEMA_ARTIFACTS|mutants-wp|proof-coverage|suite-manifest|target/debug/codefabric-contracts|SOURCE_RELATIVE|wave0_probe' src tooling scripts justfile Cargo.toml codefabric-cpg-mcp -g '!target/**'
just governance-scan
jq '{status,current_packet,packets,milestones,decommission_batches}' docs/plans/state/codefabric-waves-4-7-core-facts_v4_state.json
```

##### Known Touch (verified this session)

- Old generator binaries/scripts/aliases, authored catalog bootstrap, embedded digest mutators,
  manual output/package lists, artifact-ID/path dispatch, proof manifest/compiler, and
  `mutants-wp29`–`mutants-wp32` recipes.
- This plan/state, the frozen v4 plan/state, and the candidate Waves successor plan/state. The
  active pointer remains on this plan throughout WP14.
- All committed generated output families and their Rust/Python/wheel/Proto/schema consumers.

#### Required Changes

- Delete the old native-source mutators, authored catalog bootstrap authority, independent
  generator chain, path/member/include maps, artifact-ID dispatch, Python Cargo callback, shared
  mutable compiler binary path, and old writer implementations. Retain only thin intent aliases
  where a documented compatibility window is justified; every alias delegates to model commands.
- Delete the authored proof-coverage manifest and its legacy self-contained evaluator path, plus
  every `mutants-wp*` recipe/source hook, after WP13's independent selected-vs-full corpus passes.
  Preserve and reshape useful Just/test live-collection adapters inside the model assurance
  compiler. Preserve `mutants-file` outside all profiles and preserve historical plans/states as
  immutable evidence excluded from zero-state searches.
- Add structural rules with negative rule tests for reintroduction: direct writes to authority,
  generated member/path lists, raw production CBEF/code/flag construction, old generator entry
  points, proof manifests, packet recipes, shared compiler executable paths, and generated-linked
  bootstrap edges.
- Run an explicit, separately confirmed `model-sync` after the decommission edits and inspect its
  diff. Then run recovery first and require a zero-action plan before release certification.
- Make `model-release-check` strictly read-only: generated-output-absent compiler build,
  cache-disabled full model/check, independent staged Rust/Python/Proto/schema consumers, exact
  two-root path/byte reproduction, gix-disabled equivalence, incremental/full differential,
  wheel/package-data validation, and all DB zero-state checks. Transaction apply/fault/concurrency
  proof runs only in isolated disposable worktrees/fixtures, never against the current repository.
- Create `just model-zero-state-check` and `just model-release-check`. The release recipe owns the
  exhaustive sequence and reports evidence; it stores no wall-clock, derived cache metric, or pass
  verdict in execution state.
- Implement `just model-handoff-check` before any pointer change. In pre-handoff mode it validates
  the approved inactive successor, current remediation readiness, frozen v4 history, trusted
  WP27–WP31 ancestors, WP32 incompleteness, and the permitted H/S state transition; after H it
  validates the active successor and prohibits product progress before the judgment seal.
- Create a versioned Waves 4–7 successor through the implementation-plan discipline. Preserve
  WP27–WP53 IDs, dependencies, functional outcomes, and WP27–WP31 historical proving judgments;
  keep WP32 incomplete and absorb the corrected CBEF/source-syntax substrate without claiming its
  remaining product acceptance. The successor paths are
  `docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md` and
  `docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json`.
- Replace every remaining mutation obligation and old generator/catalog mechanic in the successor
  with model capabilities/profiles. Carry the accepted correction that parenthesized/parse-error/
  missing syntax is annotation/detail, not duplicate relations. Preserve DB07–DB09 outcomes.
- Reconstruct successor execution status from proving commits plus current gates, never from the
  dirty-tree or this plan's derived facts. Obtain explicit user approval of the successor and stage
  its initial schema-2 state, but do not move `docs/plans/active-plan.json` or resume product work
  in this packet.

#### Legacy Disposition and Decommission

This packet closes DB01–DB05 and prepares DB06. The frozen v4 plan/state remain immutable history;
they are excluded by explicit historical-path policy rather than text edits. This plan remains
active through M04 and WP15.

#### Acceptance Checks

##### Behavioral

- `just model-release-check`
- `model_waves_successor_candidate_preserves_history_and_leaves_wp32_incomplete`

##### Structural

- `just model-zero-state-check`
- `just governance-scan`
- `just artifacts-check`

##### Negative / Zero-State

- `model_db01_db05_zero_state_and_reintroduction_rules`
- `model_generated_outputs_absent_bootstrap_and_stale_output_deletion` passes.
- `model_active_pointer_remains_on_remediation_through_release_certification` passes.

##### Operational

- `model_release_recovery_first_cache_disabled_two_root_and_gix_disabled` passes.
- `just plan-status` trusts all completed entries by ancestor proving commits and reports the
  remediation safe next action as WP15.

##### Executable Oracle Catalog

- Executable oracle: `just model-release-check`
- Executable oracle: `just model-zero-state-check`
- Executable oracle: `model_waves_successor_candidate_preserves_history_and_leaves_wp32_incomplete`
- Executable oracle: `model_active_pointer_remains_on_remediation_through_release_certification`

#### Edit-Local Gates

- `just typos`
- `just governance-scan`
- Focused zero-state rules/tests.

#### Packet-Local Gates

- `just model-zero-state-check`
- `just model-release-check`
- `just model-handoff-check`
- `just ci-fast`
- `just features-each`
- `just stable-graph-check`
- `just policy`
- `just adapter-wheel-test`
- `just tracked-target-zero-state-check`
- `just artifacts-check`
- `just plan-status`

#### Integration Milestone

M04.

#### Replan Triggers

- Any DB zero state can pass while a legacy writer/authority remains reachable.
- Cache-disabled and incremental, gix and fallback, or two-root outputs differ.
- The generated-output-absent compiler build depends on committed generated production content.
- The successor cannot preserve the Waves 4–7 DAG/proving history without claiming unproved work
  or rewriting frozen state.
- The release census, compatibility, allocation, KAT, or signature acceptance would be modified by
  routine release execution.

#### Rollback or Recovery

Do not activate the successor until the complete read-only release gate is green at one proving
commit and the successor is explicitly approved. If the separately invoked sync fails, recover
through WP11's journal; keep this plan active, restore no independent writer, and rerun the
read-only cache-disabled proof after a zero-action plan.

#### Design-Bearing Contracts and Exemplars

```text
frozen Waves v4 -> release-certified remediation -> approved inactive Waves successor
WP27..WP31 proven history preserved; WP32 remains incomplete; WP33+ remains pending
```

### WP15 — Seal the terminal state and hand off to the Waves 4–7 successor

#### Outcome

After M04 and explicit user approval of the prepared successor, handoff commit H makes that
successor the sole active plan without circular proof or invented WP32 completion. H is the entire
executable WP15/M05 outcome. Ordinary judgment-only state update S subsequently records H as the
trusted proving commit and freezes this plan; S is bookkeeping, not a second behavior H is expected
to prove. No successor product packet runs between H and S.

#### Dependencies

WP14, M04, explicit user approval of the successor plan.

#### Target Invariants

G-04, G-14–G-17.

#### Design and Library References

- Dossier §§5.1–5.3 and 6.7–6.8.
- Repository artifact/state schema completion and proving-commit rules.
- Frozen Waves 4–7 v4 packet DAG/proving history and the WP14 successor candidate.

#### Change Surface

##### Preflight Query

```bash
cat docs/plans/active-plan.json
jq '{status,current_packet,packets,milestones,decommission_batches,next_action}' docs/plans/state/codefabric-model-driven-artifact-and-assurance-control-plane_v1_state.json
jq '{status,current_packet,packets,milestones,decommission_batches,next_action}' docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json
just model-release-check
```

##### Known Touch (verified this session)

- `docs/plans/active-plan.json`.
- This plan's schema-2 state and the approved successor plan/state.
- Frozen Waves 4–7 v4 plan/state as read-only provenance.

#### Required Changes

- Re-run the read-only M04 release matrix at current HEAD and verify the successor approval record,
  current plan digest, successor declared inputs, reconstructed WP27–WP31 proving commits, incomplete
  WP32 judgment, and unchanged WP33–WP53 dependency closure.
- Create handoff commit H that changes only the active pointer and successor activation fields
  needed to name WP32 as the safe next action. This plan's WP15 remains `in_progress` at H; no
  product executor may run under the successor yet.
- At H, run `artifacts-check`, `plan-status`, and `model-handoff-check` against the now-active
  successor and explicitly validate this remediation by plan path. H is the behavioral proving
  commit for WP15/M05 and already satisfies their executable outcome.
- Create terminal seal commit S that changes only this remediation state: record WP15, M05, and
  DB06 complete with `proving_commit=H`, mark the plan complete, and state that the successor is
  active at incomplete WP32. This is the sole permitted post-handoff write to the old state.
- Re-run handoff checks at S. Only then freeze this state and release successor WP32 execution.
  Any later correction to this remediation requires a versioned status review, never an in-place
  reinterpretation.
- Use the `model-handoff-check` implementation proven in WP14. It validates one active pointer,
  both state schemas, H ancestry, current-head replay, frozen v4 history, WP27–WP31 trust, WP32
  incompleteness, and no early WP33+ in both unsealed-H and sealed-S states.

#### Legacy Disposition and Decommission

This packet closes DB06. Handoff machinery is administrative and contains no product generator or
second writer. The old remediation state is immutable after S; the successor alone owns execution.

#### Acceptance Checks

##### Behavioral

- `model_handoff_commit_activates_only_approved_successor_at_incomplete_wp32`
- `model_handoff_at_h_is_the_complete_executable_outcome`

##### Structural

- `just model-handoff-check`
- `just artifacts-check`
- `just plan-status`

##### Negative / Zero-State

- `model_handoff_rejects_two_active_plans_unapproved_successor_and_early_wp33`
- `model_handoff_allows_only_judgment_seal_after_h`

##### Operational

- `model_handoff_check_accepts_unsealed_h_and_sealed_current_head`

##### Executable Oracle Catalog

- Executable oracle: `just model-handoff-check`
- Executable oracle: `model_handoff_commit_activates_only_approved_successor_at_incomplete_wp32`
- Executable oracle: `model_handoff_at_h_is_the_complete_executable_outcome`
- Executable oracle: `model_handoff_rejects_two_active_plans_unapproved_successor_and_early_wp33`

#### Edit-Local Gates

- `just artifacts-check`
- `just plan-status`

#### Packet-Local Gates

- `just model-handoff-check`
- `just artifacts-check`
- `just plan-status`

#### Integration Milestone

M05.

#### Replan Triggers

- The artifact/state schema cannot record H as a trusted ancestor through an ordinary later
  judgment-only update without a mutable dual-active interval.
- The successor is not explicitly approved or cannot reconstruct WP27–WP31 trust and WP32
  incompleteness.
- Any product or generated-output change is required during the handoff commits.

#### Rollback or Recovery

Before H, keep this plan active. After H but before S, do not run product work; if validation fails,
use a dedicated pointer-rollback commit restoring this plan, validate both states, and rerun M04.
After S, rollback requires a new accepted successor/status plan rather than editing frozen history.

#### Design-Bearing Contracts and Exemplars

```text
A = M04-certified commit, remediation active
H = pointer + successor activation; WP15 evidence commit; no product execution
S = old-state terminal seal recording proving_commit H
then and only then: successor WP32 may resume
```

---

## 5. Integration milestones

### M01 — Accepted model bootstrap and release-history boundary

#### Dependencies

WP01, WP02, WP03, WP04.

#### Closure

- Normative ownership/design corrections and the portable external-driver security contract are
  accepted and machine checked.
- This plan is the sole active mutable execution program; Waves 4–7 v4 is frozen history.
- The handwritten-only compiler builds with production generated outputs absent and without the
  denied production dependency closure.
- Current-byte inventory, exact family claiming, typed graph/order/diagnostics, and gix-disabled
  equivalence pass.
- The owner has explicitly accepted the initial released-artifact census; routine sync cannot
  modify it.

#### Gates

- `model-design-contract-check`
- `model-bootstrap-check`
- `model-inventory-check`
- `model-release-census-check`
- `stable-graph-check`
- `features-each`
- `artifacts-check`

#### Stop Condition

M01 is blocked—not waived—until the explicit human release-census acceptance exists.

### M02 — Complete read-only family parity and isolated consumer proof

#### Dependencies

WP05, WP06, WP07, WP08, WP09, WP10.

#### Closure

- Every supported family plans all outputs through typed drivers and one complete DesiredTree.
- The old generator is still the only writer; the new system leaves the repository unchanged.
- Two isolated roots produce identical paths/bytes; every difference from committed outputs is an
  accepted correction with an independent oracle.
- Staged Rust/Python/Proto/schema consumers and a clean installed wheel validate the overlay.
- CBEF recipes/allocations and the isolated dirty-WP32 consumer overlay conform to normative design
  without changing live consumers or claiming WP32 product completion.
- Every handwritten consumer migration is a typed tracked transition patch with a validated base;
  the complete overlay reproduces from the M02 proving commit without `target/` or state-carried
  derived data.

#### Gates

- `model-plan-check`
- `model-family-check`
- `model-repro-check`
- `contracts-verify`
- `contracts-repro-check`
- `schema-check`
- `fixture-check`
- `proto-repro-check`
- `adapter-contracts-repro-check`
- `adapter-wheel-test`

#### Stop Condition

No repository write authority transfers while any family, output, validator, package consumer, or
accepted correction is absent from the complete staged tree.

### M03 — Crash-consistent sole-writer cutover

#### Dependencies

WP11.

#### Closure

- Every supported reader participates in recovery/locking.
- The reviewed WP06–WP10 consumer overlay and complete DesiredTree are live and coherent; the
  one-time promotion bridge is disabled.
- Cache-disabled full plan/stage/validate/recheck/journal/apply/recover passes all kill points,
  linked-worktree concurrency, source drift, symlink substitution, user edit, and stale deletion.
- `model-sync` is the sole enabled routine writer; old writer code is unreachable.

#### Gates

- `model-transaction-check`
- `model-repro-check`
- `model-family-check`
- `ci-fast`
- `adapter-wheel-test`

#### Stop Condition

Caching, watchers, and proof-profile replacement remain disabled until M03 passes at one proving
commit and current HEAD.

### M04 — Incremental assurance, zero state, and read-only release certification

#### Dependencies

WP12, WP13, WP14, DB01, DB02, DB03, DB04, DB05.

#### Closure

- Incremental/cache modes equal cache-disabled full execution across property and real-family
  corpora; cache/watch failure only widens or recomputes.
- Live assurance profiles are independently sound against the retained full suite and contain no
  mutation campaigns.
- DB01–DB05 and their reintroduction rules pass; DB06 is prepared but not yet closed.
- `model-release-check` passes at the proving commit and current HEAD.
- The reconciled Waves 4–7 successor is explicitly approved and ready but inactive. This
  remediation remains the sole active plan with WP15 as its safe next action.

#### Gates

- `model-incremental-check`
- `model-assurance-check`
- `model-zero-state-check`
- `model-release-check`
- `ci-fast`
- `features-each`
- `stable-graph-check`
- `policy`
- `adapter-wheel-test`
- `tracked-target-zero-state-check`
- `artifacts-check`
- `plan-status`

#### Stop Condition

No pointer or successor activation field changes during M04 certification. `model-release-check`
requires zero planned repository actions and never invokes `model-sync` against the current tree.

### M05 — Successor activation handoff

#### Dependencies

WP15, DB06.

#### Closure

- H activates exactly the approved successor at incomplete WP32 and passes successor/default plus
  explicit-remediation artifact/status checks.
- H makes the successor the only active plan and establishes this remediation and v4 as historical
  programs with no product or generated-output changes.
- H leaves every successor packet after incomplete WP32 pending and prohibits product execution
  until the ordinary judgment seal is recorded.

#### Gates

- `model-handoff-check`
- `artifacts-check`
- `plan-status`

#### Stop Condition

State update S records H as the trusted proving commit for WP15, M05, and DB06 and changes no
behavioral artifact. Successor WP32 execution is prohibited until S and current-head handoff checks
pass; this recording protocol is not part of the executable outcome attributed to H.

---

## 6. Decommission batches

### DB01 — Authored aggregate and embedded-identity authority zero state

#### Depends On

WP04, WP05, WP10, WP11, WP14.

#### Required Zero State

- No routine source manifest or packaged index is a model compiler input.
- No routine writer embeds computed canonical/source/bundle identity into native authorities.
- Requirements, traceability, fixture indexes, bundle membership, toolchain views, and reverse
  indexes are derived from typed model nodes/edges and accepted records.

#### Evidence

- Structural rule plus negative rule test.
- Generated-outputs-absent bootstrap.
- Independent digest/KAT and released-deletion negatives.

### DB02 — Manual artifact/output/member/path dispatch zero state

#### Depends On

WP06, WP07, WP08, WP09, WP10, WP14.

#### Required Zero State

- No individual artifact ID, output path, package member, Proto domain, schema list, or registry
  member is centrally enumerated for routine planning or generation.
- Only fixed family rules and typed adjacent declarations remain.
- Generated module/package aggregators are output-role projections.

#### Evidence

- Textual and structural reintroduction rules with negative fixtures.
- Add-one-family-member proof requiring data-only changes.
- Exact producer/output census from the complete model.

### DB03 — Old writer, generator chain, and executable-race zero state

#### Depends On

WP02, WP05, WP06, WP07, WP08, WP09, WP10, WP11, WP14.

#### Required Zero State

- No old native mutator, independent contracts/adapter/Proto writer, Python Cargo callback, or
  shared feature-sensitive compiler binary path remains reachable.
- No transition consumer patch, one-time promotion bridge, or `tooling/model-transition/` input
  remains after the dependency-closed live promotion.
- Legacy command aliases, if temporarily retained, delegate only to model commands.
- Exactly one reconciler owns routine `Derived` writes.

#### Evidence

- Command-DAG inspection, compile-time feature/dependency checks, and source rules.
- Concurrent distinct-feature executable identity test.
- Transaction writer-ownership negative.

### DB04 — Authored proof manifest and packet mutation infrastructure zero state

#### Depends On

WP13, WP14.

#### Required Zero State

- No active authored proof-coverage manifest or legacy manifest evaluator, `mutants-wp*` recipe,
  packet proof ID, or mutation score threshold remains outside immutable historical plans/states.
- Useful live Just/test collectors survive only as typed assurance-compiler adapters; they do not
  read an authored expected proof graph.
- `mutants-file` remains available only as an explicit optional Tier-C diagnostic and appears in no
  model profile.

#### Evidence

- Live Just/Nextest/Pytest/rule inventory.
- Independent selected-vs-full perturbation corpus.
- Historical-path-aware textual/structural zero-state rule.

### DB05 — Manual semantic construction and allocation zero state

#### Depends On

WP06, WP07, WP14.

#### Required Zero State

- Production code cannot construct governed CBEF records positionally or use raw occurrence,
  provider-kind, relation, flag, table-field, or schema allocation literals.
- Recipe-aware/generated APIs are the only production construction surface; generic codecs remain
  private and validate the selected recipe.

#### Evidence

- Compile-fail/API visibility tests, structural rules, and negative rule tests.
- All-authority recipe/allocation census and runtime round trips.

### DB06 — Active-program transition zero state

#### Depends On

WP01, WP14, WP15.

#### Required Zero State

- During execution, exactly this plan has mutable schema-2 state and owns the active pointer.
- Through M04, this remediation owns the pointer. After terminal seal S at M05, exactly the
  reconciled Waves 4–7 successor owns it.
- Frozen v4 and completed remediation state remain immutable history; no WP32 proving commit is
  invented and no WP33+ packet is released early.

#### Evidence

- Artifact-contract active-plan validation and `plan-status` provenance checks.
- Ancestor validation for WP27–WP31 proving commits.
- Successor dependency/status reconstruction test.
- H/S ancestry and terminal-seal validation through `model-handoff-check`.

---

## 7. Execution sequence, concurrency, and commit policy

### 7.1 Dependency-closed sequence

```text
WP01 -> WP02 -> WP03 -> WP04 -> M01
M01  -> WP05 -> WP06 -> WP07 -> WP08 -> WP09 -> WP10 -> M02
M02  -> WP11 -> M03
M03  -> WP12 -> WP13 -> WP14 -> {DB01 || DB02 || DB03 || DB04 || DB05} -> M04
M04  -> WP15 -> DB06 -> M05 -> active Waves 4–7 successor at incomplete WP32
```

### 7.2 Concurrency constraints

- WP01–WP05 are serial because they establish one active program, bootstrap graph, release census,
  and shared action protocol.
- WP06–WP10 are serial even though drivers are separable: their staged artifact index, bundles,
  package views, dirty WP32 files, and old-writer parity surfaces overlap. Read-only library probes
  may run independently, but no family cutover writes concurrently.
- WP11 is exclusive. No old/new writer overlap is permitted, and no cache/watch/assurance cutover
  work begins until M03.
- WP12–WP14 are serial because assurance must inspect the stable DesiredTree/action protocol and
  decommission may begin only after independent soundness proof.
- WP15 is a serial administrative handoff. No product executor runs between H and S.
- No Waves 4–7 WP33+ execution may run concurrently with this plan. Those packets consume or edit
  registries, schemas, fixtures, Proto, identity, lifecycle, and proof surfaces being migrated.

### 7.3 Commit and state policy

- Initialize the schema-2 execution state only after plan approval and active-pointer adoption.
  State contains judgment fields and recipe-name evidence only; it stores no derived head, digests,
  timings, cache rates, mutation scores, or worktree fallback proof.
- A packet becomes complete only when its packet gates pass at a non-null proving commit and again
  at current HEAD. Milestones and DBs obey the same ancestor rule.
- Preserve the pre-existing WP32 work identified by the plan frontmatter baseline. Derive current
  diffs on demand and record only the judgment that WP06 absorbed them; never place a diff digest
  or fingerprint in schema-2 state. Never overwrite, stash, reset, or run an old mutating generator
  over the work.
- Prefer one proving commit per dependency-closed packet. Do not mix unrelated user changes or
  successor-plan activation into an earlier packet. WP15 is the explicit exception: H proves the
  handoff and S is a terminal judgment-only seal that records H after it becomes an ancestor.
- Ubuntu clean-host evidence remains deferred and does not block a packet or milestone.

---

## 8. Permanent command contract

The final public Just surface is capability-based. Packet/family counts and tool flags do not
become permanent API names.

### Read-only

- `model-bootstrap-check`
- `model-plan paths...`
- `model-explain id_or_path`
- `model-check profile="edit"`
- `model-repro-check`
- `model-incremental-check`
- `model-transaction-check`
- `model-assurance-check`
- `model-release-check`
- `model-handoff-check`
- Optional migration/debugging: `model-family-check family=""`; an empty family selects the
  aggregate check used by the final gate matrix.

### Mutating or acceptance-gated

- `model-sync` — confirm-guarded routine reconciliation of `Derived` paths only.
- `model-watch` — explicit opt-in process; events are hints and loss widens to full.
- `model-accept kind` — separately guarded owner acceptance; never a gate dependency.

Legacy contracts/adapter/Proto generation and verification names may remain thin intent aliases
only through the documented compatibility window. They cannot contain their own writer or model.

---

## 9. Final gate matrix

The execution state records these Just recipe names only:

- `model-design-contract-check`
- `model-bootstrap-check`
- `model-inventory-check`
- `model-release-census-check`
- `model-plan-check`
- `model-family-check`
- `model-repro-check`
- `model-transaction-check`
- `model-incremental-check`
- `model-assurance-check`
- `model-zero-state-check`
- `model-release-check`
- `model-handoff-check`
- `ci-fast`
- `features-each`
- `stable-graph-check`
- `policy`
- `adapter-wheel-test`
- `tracked-target-zero-state-check`
- `artifacts-check`
- `plan-status`

`model-release-check` is the exhaustive read-only release capability. It runs recovery first;
requires `model-plan` to report zero actions; proves the generated-output-absent bootstrap;
performs cache-disabled full model/check; validates staged Rust, Python, schema, Proto, and wheel
consumers; reproduces exact paths/bytes in two roots; proves gix-disabled equivalence; compares
incremental/full and selected/full behavior; runs transaction/concurrency/fault proof only in
isolated worktrees/fixtures; and evaluates all zero-state rules. It neither invokes `model-sync`
against the current repository nor runs mutation testing or treats timing as correctness evidence.

---

## 10. Risks and adaptive replanning policy

### 10.1 Plan-local adaptation

The executor may change filenames, private module boundaries, helper types, test placement inside
the single integration target, and implementation order within one packet when the outcome,
dependency edges, acceptance, and decommission proof are unchanged. The executor records the
change-surface discovery in state evidence rather than editing this plan.

### 10.2 Plan revision triggers

A versioned plan revision is required when:

- a new dependency or packet edge is needed;
- the family migration serialization or sole-writer cutover order changes;
- a new governed authority/evidence/acceptance kind or external driver is added;
- output ownership, active-program transition, final gate capabilities, or a DB zero state changes;
- the Waves successor cannot preserve the original functional DAG/status judgments as specified.

### 10.3 Design reopening triggers

Stop execution and reopen the accepted dossier when:

- one typed RepositoryModel cannot represent a governed family without hidden central member
  knowledge;
- a driver cannot declare every output/read before render or needs authority write access;
- portable correctness requires an unenforceable OS security boundary rather than the accepted
  cleaned-environment/staging/source-fence contract;
- crash recovery cannot guarantee a complete old or new supported view;
- released deletion cannot be protected by independent owner acceptance;
- incremental or selected assurance cannot be made equivalent to cache-disabled/full behavior by
  typed dependencies and conservative widening;
- a CBEF, allocation, schema, adapter, or Proto semantic requirement contradicts its normative
  owner rather than merely the current implementation.

### 10.4 Highest-risk execution hazards

1. **Dirty WP32 collision:** record and preserve current diffs before WP06; no old generator may
   rewrite them.
2. **False bootstrap:** physically omit generated production outputs and deny the production
   dependency graph; a feature-name claim is not evidence.
3. **Self-certifying assurance:** retain independent KATs and the old full suite until the
   perturbation corpus passes; generated reports cannot be oracles.
4. **Release-history loss:** accept the released census before suite manifest/index loses oracle
   status; routine sync cannot edit acceptance.
5. **Shared executable race:** action identity includes executable bytes and feature/toolchain
   graph; use isolated target/artifact paths or deterministic resource locks.
6. **Transaction deadlock or split view:** one outer lock owner, no internal reacquire, recovery
   before all reads, and durable state outside `target/` are mandatory.
7. **Large cutover diff:** exact producer ownership, source recheck, complete staged validation,
   stale-root negatives, and user-edit detection precede apply.
8. **Premature product resumption:** no WP33+ work and no WP32 completion claim before the Waves
   successor is sealed active after M05.
9. **Circular handoff proof:** M04 certifies while remediation is active; H switches the pointer;
   S records H as an ancestor; no product work is allowed in the interval.

---

## 11. Planning completeness checklist

- [x] Accepted target design and named assumptions are identified.
- [x] Current dirty WP32 work and the active v4 execution boundary are explicit.
- [x] Every packet is dependency-closed and has outcome, invariants, references, preflight, known
  touch, required change, legacy disposition, layered proof, gates, triggers, recovery, and a
  design-bearing exemplar.
- [x] Bootstrap is proven independently of generated production output and overlay consumers.
- [x] Released-history acceptance is an explicit human checkpoint.
- [x] Family migrations cover registries/CBEF, schemas/TableSpecs, adapter/Pydantic/FastMCP,
  single-FDS Proto, provenance/governance views, and package aggregators.
- [x] Transaction correctness precedes cache/incremental optimization.
- [x] Assurance cutover is independently compared to the retained full suite.
- [x] DB01–DB06 cover every named legacy authority, writer, manual map, semantic construction,
  proof manifest, packet mutation surface, and plan transition.
- [x] Read-only release certification and Waves successor activation are separate, non-circular
  terminal packets with an explicit H/S seal.

---

**Plan decision:** `draft-for-audit`.
