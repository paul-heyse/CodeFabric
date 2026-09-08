---
artifact: implementation-plan
plan_id: codefabric-real-time-cpg
version: v3
date: 2026-09-07
status: approved
design_path: docs/designs/codefabric_real_time_cpg_planning_contracts_design_v2_2026-09-07.md
design_version: v2
source_review_path: docs/designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md
baseline_commit: e24627a29ee44bc3f288eeb07197b66e6dec7c7e
working_tree_digest_mode: git-status-diffs-and-untracked-content-excluding-activation-artifacts-v1
working_tree_digest: f427c0629eecfc17bc05ffbb46f09c832e66ff6bb3cd28a1496ab6aedbe834ef
predecessor_plan_path: docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md
approval_basis: User explicitly approved the native-resource amendment, plan revision and resumed remaining execution on 2026-09-07.
state_path: docs/plans/state/codefabric-real-time-cpg_v3_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md
---

# Complete real-time Python/Rust CPG — implementation plan v3

## 1. Outcome, non-goals and execution boundary

### Execution clarification approved by the user on 2026-09-08

The data-fabric artifact and provenance requirements govern actions performed by the
running system and their observable results. They do not require an artifact for
every edit made to this repository. Ordinary Git history, dependency source pins,
code review and proportionate tests govern development and integration.

This explicit user clarification supersedes the source-packaging procedure in
planning-contracts v2 §3.1 and this plan's earlier LD-RT08/LD-RT09 packaging language.
Develop the native corrections and their application consumers together in one Git
integration branch, using tracked dependency sources and relative Cargo paths. A
bespoke content-addressed source bundle, ordered patch replay, pre-Cargo artifact
verification, or a synchronized eight-document suite successor is not a prerequisite
to editing, compiling or integrating those changes. Preserve exact upstream origins,
licenses, the selected public type universe, and actual resolved-source checks.

Runtime execution provenance, resource ownership, native behavioral compatibility,
packet acceptance and final certification remain required. This correction changes
the development process; it does not mark any implementation packet complete.

### 1.1 Full realized outcome

One continuously running Rust daemon creates and maintains the selected v2.3 Python and Rust CPG profiles, integrates actual Tree-sitter/Ruff/Pyrefly/rustc facts through owned Arrow and programmatic DataFusion, publishes exact Delta/segment epochs, and serves all eight typed request forms through modern FastMCP. Real source/context changes produce truthful syntax-current and semantic-current successors without daemon restart. Independent semantic expectations, sound withdrawals/unknowns, optimizer-visible native execution, bounded resources, lease/CDF-safe actual reclamation and one-candidate certification are required outcomes.

This is the realization plan for the **entire** comprehensive review, not only its first production vertical or fourteen defect fixes. Review §§2–6, LD-01–LD-07, all 18 native-capability selections and the full GEN/ONT selected profiles remain in scope. The accepted planning-contract successor retains LD-RT08 native maintenance and adds the approved LD-RT09 native resource correction inside WP79. Full completion requires both resource closure and actual safe reclamation.

The work retains and reproves v7's compiled release, inward feature graph, provider isolation, native scan/property contracts, exact durable authority, structured task tree, modern serving security and FreshActivation. Old completion labels/proving commits are not copied into new state. Existing physical deletions stay deleted.

### 1.2 Non-goals and genuine uncertainty

- No second graph database/differential runtime, public SQL/serialized-plan ingress, arbitrary semantic plugin/program registry, Python data plane, new conceptual Cargo root or second top-level Rust integration-test target.
- No perfect prediction of arbitrary dynamic Python/Rust behavior. Required families expose exact, sound-possible, heuristic, unresolved and unavailable states according to the selected profile; unimplemented required functionality is **not** an uncertainty waiver. In particular, MAY_ALIAS candidates do not become exact runtime alias claims merely because a finite abstract transfer is computed exactly.
- No raw provider-local canonical IDs, compiler debug-string semantic extraction, git-history/runtime-observation ontology, evaluative risk/quality/refactor conclusions, unsandboxed compilation fallback or cross-workspace/context fact merging.
- No old/new semantic selector, dual durable writer, mutable latest/receipt/hash authority, rollback-to-predecessor, broad destructive cleanup or silent compatibility alias.
- No indiscriminate feature enablement, speculative distributed/GPU/Flight/ADBC adoption or automatic dependency upgrade. LD-RT08 and LD-RT09 are the narrowly approved amendments: native maintenance and native resource ownership must acquire certified capabilities while preserving the existing public type universe.
- No remote publication, registry upload, deployment or third-party communication authorized by this planning task. Any distribution authority needed for a future dependency artifact is requested explicitly.

### 1.3 Baseline and scope preservation

This approved successor preserves all WP77–WP106, M18–M25 and DB24–DB30 scope, IDs, dependencies and existing acceptance checks. It incorporates the user's 2026-09-07 native-resource approval and the status review v2. WP79 gains native allocation/retention and coherent native/application integration obligations; WP94 maintenance and WP95 reclamation remain separate. Prior v1/v2 plans and the planning-contract v1 remain immutable history and declared inputs. This revision has no completed-packet implication.

WP77/WP78 carry historical proving commits as stale until repaired immediate consumers have coherent current proving lineage. WP79 resumes in progress; WP80–WP106 retain all unfinished work in dependency order. Earlier downstream invalidation was an acceptance dependency on the now-approved design decision. All milestones and decommission exits remain unclosed until their substantive proof succeeds. Preserve deviations and failed approaches in the fresh successor state; clear only the resolved approval blocker.
The frontmatter records the activation validator's exact whole-worktree snapshot, excluding this plan, its future state and the active pointer. The review's narrower implementation baseline remains independently reproducible with:

```bash
git status --short --untracked-files=all
git diff --stat df1c50c684a5e005c4b76ada9cd19d8030d9d7dd..HEAD -- src rustc-extractor pyrefly-sidecar codefabric-cpg-mcp Cargo.toml Cargo.lock justfile .github
git diff --binary HEAD -- src pyrefly-sidecar rustc-extractor codefabric-cpg-mcp Cargo.toml Cargo.lock justfile .github | sha256sum
```

That review-scope command excludes unrelated untracked files and documentation; the activation digest does not. Both are identity, not correctness or ownership. Attribute overlapping dirty edits before execution and preserve concurrent work. The original planning session ran `just ci-fast`: exit 101 at `root-clippy`, after root formatting/type checking. The existing [v7 status assessment](../reviews/implementation_status_codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02_2026-09-04_v2.md) additionally records stale WP65 measurements and refusal of dirty-candidate certification; these were not silently converted to passes. WP105 must close residual quality failures. A packet cannot claim completion while one of its required local checks is red; baseline attribution is not a waiver of the terminal matrix.

This approved revision authorizes the already-requested complete implementation, not new external distribution operations. State creation and active-pointer selection occur only through the confirm-gated activation transaction after artifact/snapshot validation. The future state follows schema version 2; activation never overwrites prior state.

### 1.4 Reading and packet law

Read the planning-contract dossier, then the comprehensive review end-to-end. Consult v7 §1/§3 and its accepted boundary design for retained architecture. Navigate the v2.3 masters with `just spec-outline`; cite their tagged section/title rather than line numbers. Load one packet at a time during execution and rerun its preflight before editing.

New stable IDs start at WP77/M18/DB24 because earlier plans already used WP67–WP76. IDs are not renumbered during execution. This is a new topic/version with an explicit activation predecessor, not an edit to v7. Packet numbers are identifiers; the dependency graph, not numerical sorting, determines execution order.

Every packet contains four **proposed new substantive test/oracle names**, mapped to INT/BEH/NEG/OPS criteria. WP77 introduces `just real-time-cpg-packet-check <WP>` to discover and run the packet's actual tests. Each later packet implements its four tests with actual production consumers and independent falsifying cases. A generic wrapper, textual presence check, passing absent selector or body that merely returns success is not their implementation. Existing recipes below are retained/expanded test owners, not proof that those scenarios already pass today.

## 2. Source design and declared inputs

The selected target is the accepted planning-contracts v2 native-resource amendment, incorporating every v1 contract and named assumption. The source review remains immutable draft review evidence; its unresolved details are not silently ignored. The following is the only declared-input digest table, computed once during authoring. Recompute through tooling for validation; never restamp stale rows.

| path | sha256 |
|---|---|
| docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md | 3b004150c5146b459ad800e128f3b509a9588ecc3d64a8aa3065b4bf1e31c68a |
| docs/designs/codefabric_real_time_cpg_planning_contracts_design_v2_2026-09-07.md | 648db90901976a1b8bf58b4a83e7bd9f975daeecf1a35609d98a5d1a9254ca07 |
| docs/reviews/implementation_status_codefabric_real_time_cpg_implementation_plan_v2_2026-09-04_2026-09-07_v2.md | 92b93606b2f65ecb960f4a6198553608683d3eb23ac2881e9471a7e1c4cb12cf |
| docs/plans/codefabric_real_time_cpg_implementation_plan_v1_2026-09-04.md | d3f215dafff0292e31bf704c2044fcbad3c93ee5acc976c124c5d5b29e3a4ace |
| docs/designs/codefabric_real_time_cpg_planning_contracts_design_v1_2026-09-04.md | 1a7de326e5e0afa2edbf3a0f70dfb65e0bee27e1bf92f66f68d087847fcfa36d |
| docs/designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md | e2a632e9c5a36aed90011a523e515566f0b4c9f2521e4114fa222b5995984e78 |
| docs/designs/codefabric_compiled_release_provider_fabric_runtime_boundary_design_v1_2026-09-02.md | 9d857d81899b133664fcaf48294b58c2c3fa969e9394b68e8c376c2dd6ffd620 |
| docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02.md | 6fc791d15081a3febae6481de0180f05cdbd043d2e2e2ce2ab3c8e3f52ead00e |
| docs/library_ref/full_data_fabric_design_principles_v2.md | eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md | 5d7309244156ed0833f1c0ae33f7796b9c9a4e88133c34dc2b1a956b47fc9e9f |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.3.md | aadd002c169aee5bd293fe5b9a01a07e76395157d7f9709132edcb3602a5b300 |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md | ce54d0b1bc9aeeb4d85de3e609faa1cf7f4e4c6f3a649645ee9de9c3fc63c4f5 |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md | 662b0c5bbf9f0963195b7fd3ad6f598ccfe2629fe1f7e5916e0cabef4c5104bf |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v2.3.md | 4968ce530ad4f23df2c08a4780ac3160222ab3ae6c81c183755524721969dcb3 |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md | 3b90ec9d779e350996882be28edc45fb52c9caf3e3e06af62716ca344e37194a |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md | b3b9365c2f6d53105f254f0f6a69da5fc950e321a91dfc0880e55d58b7e39143 |
| docs/authoritative_design/codefabric_2.3_implementation_roadmap_v1.0.md | 4a3741eeb740802f4161dd2b0d7ed6eddd72a640e54c92187ed40cf934e74da5 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md | 794a4ecbb38cd90d7ca4506a33c5e8c4b32e209d9a2f9b9429290f96c9af9fc1 |
| docs/library_ref/tree_sitter_rust_python.md | 615ce801958a3e74bf8cbbd5d759bade62958b1e4297185ebe8b18aa87e1428b |
| docs/library_ref/ruff_python_crates_advanced_reference_2026-08-18.md | f42e0b5e3d63c66bde68e2c3b79cef04d288eec52ee64e424f3d95578fc386d6 |
| docs/library_ref/pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md | 208582927c109dde0d399a7277442417006c878dfd21403a09a5c0bc7b2819e1 |
| docs/library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md | 1584a4ca9c7a06a495cfedacf585717aaec61949546d0191b19df48451451ea5 |
| docs/library_ref/notify_debouncer_full_rust_reference.md | aaaa48e62a582b4c6c76a77f72b569fffafa5ea1b99ba1f7343af66921631a8b |
| docs/library_ref/gix_rust_advanced_reference.md | 8325e229602411385dc824350cbfca10fec287d16e4aaa3e9c4c35c3b4bead7f |
| docs/library_ref/petgraph.md | 8f5b19b2d9fbb9dfe2caf974b2a1f4c55b9244cfd167eb48956d225a076cccd9 |
| docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md | 6dd8665f9c33e70181c292b91f6376fd76b12d7e3073a956e48ba9542d9adf32 |

### 2.1 Library and doctrine application

Use review LD-01–LD-07 through their routed reference sections. FAB §2.1 and live manifests/locks select the current pin; reference examples never select an upgrade. DataFusion reference §40A and its upgrade gates outrank historical API prose. The alignment flow/pattern bindings for schema, calculations, providers, plans, physical execution, interop, governance and provenance are carried into WP91–WP101; no custom extension is justified solely by a codebase noun.

D-RT01–D-RT07 supply closed dependency/admission/control/identity/resource/replacement/maintenance contracts. They advance P9/P10, P12/P15, P19/P20/P25, P27/P28/P30 and P23/P31/P34/P36 while maintaining the existing P3/P22/P35 ownership boundaries. Other principles remain applicable through the actual decision, not a ceremonial conformance score. Bounded exceptions have explicit DB exits; none becomes permanent by convention.

LD-RT08 remains a required native maintenance correction in WP94; WP95 must prove
actual reclamation. LD-RT09 remains the approved native resource correction in WP79.
Neither is satisfied by permanent denial or a source-only checkpoint. Source-derived
fallible admission, original retained backing, shared receipts, typed exhaustion,
finite actual execution geometry and complete production integration remain required.

Apply the 2026-09-08 execution clarification above to both amendments. Track amended
dependency sources and application consumers together in Git, select them through
relative Cargo paths, preserve the recorded upstream versions and verify the resolved
graph. Update documentation when the supported contract changes; ordinary source
edits do not require new design, plan or suite versions.

## 3. Global target invariants and dependency law

1. One fallibly compiled immutable release owns behavior-bearing provider, transformation, query, proof and policy programs and is injected once into the daemon application. No marker-only or global lookup authority.
2. Owned application contracts and Arrow/relation-scoped IPC are the provider boundary. Generated Protobuf and native compiler/provider types terminate in adapters. Keep all four build domains and useful narrow features.
3. Exact authorized bytes and effective typed contexts are causal inputs. Every inventory member has a capture disposition; full inventory and changed work are distinct; negative support participates in invalidation.
4. Raw/normalized facts, canonical identity, unknowns, authority conflicts and derived precision remain distinct and queryable. Complete selected-family conformance requires independent executable evidence.
5. One update owner stages bounded work; one command actor owns durable mutation, exact predecessor/writer fences, zero uncontrolled retry, operation reconciliation and manifest-last selection.
6. Each epoch is a complete atomic package. Syntax-current explicitly withdraws invalidated semantics; semantic-current is never stale-current. Queries retain one immutable authorized epoch.
7. Native DataFusion computation remains visible. Identity restoration uses validated mappings at every required phase; scan/projection/statistics/order/partition/metrics/extension claims are truthful.
8. Stable Delta histories and exact selectors own durable tables; immutable segments have validated durable pins; SQLite/gix/cache/plan text never select semantic truth.
9. Aggregate process/workspace/grant-bounded reservations cover epochs, providers, graph/AST/IPC/result heaps, cache, spill and disk. Ownership, cancellation, joins and release remain explicit.
10. All eight forms compose in one authorized DAG. Independent failures do not erase successes; absence needs complete scoped coverage; canonical agent responses have real delivery semantics.
11. Retention protects versions **and CDF intervals**, provenance/releases/expectations and all leases/uncertain work. Native maintenance must actually reclaim eligible resources under the approved command contract.
12. Full completion means all current packet/milestone/DB/terminal checks, raw performance evidence and independent review pass at one candidate—not a packet count or an ancestor's green result.

Semantic dependencies and known shared integration files both constrain the DAG. WP85→WP86 serializes their shared programmatic analysis integration; WP83 precedes watcher and storage construction; WP90 joins sound invalidation with selective durable publication; WP94's native amendment precedes final retention/optimization certification. Independent provider lanes WP80/WP81/WP82 can proceed after WP79 only with disjoint current write sets and isolated worktrees. The lead serializes shared recipe/generated/registry edits; a newly discovered overlap must add a true dependency or an explicit reviewed disjoint-phase disposition before parallel writes. The plan does not freeze its known-touch list as the future complete patch.

## 4. Dependency-closed work packets

### WP77 — Retain the compiled boundary and make capability claims truthful

**Outcome.** The retained architecture is executable and required capability claims are backed by independently expected behavior; the provisional sequential CFG cannot advertise complete control/dataflow semantics.

**Dependencies.** None. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** One injected behavior-bearing release; no false complete producer, generated/native type escape, reverse feature edge or copied predecessor completion. Advances P20/P25/P30/P32; maintains P3/P35.

**Design and library references.** Review F02/F12, §§2.2, 5.1; planning contracts D-RT02/D-RT03; v7 design I-60–I-71 and LD-40–LD-44; review LD-01/02/04.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/provider_contracts.rs src/provider_admission.rs src/production_provider_recipe.rs src/programmatic_derived_analysis.rs src/semantic_release.rs tooling/ci/plan_assurance.py tooling/ci/artifact_contracts.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'CompiledSemanticRelease|ProviderJob|ProviderCoverage|Complete|CFGS|Exact|packet_oracle' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/provider_contracts.rs`, `src/provider_admission.rs`, `src/production_provider_recipe.rs`, `src/programmatic_derived_analysis.rs`, `src/semantic_release.rs`, `Cargo.toml`, `tooling/ci/plan_assurance.py`, `tooling/ci/artifact_contracts.py`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Revalidate and finish the retained v7 application-contract/release/feature split against actual consumers. Preserve one fallible release construction and injected Arc; do not recreate already-deleted markers or compatibility routes.
2. Remove unsupported complete/Exact claims from the installed sequential Python CFG/dataflow path. Emit a precise temporary required-family gap until WP84/WP85 replace it; this gap is not terminal profile completion.
3. Admit independent expected source/fact/unknown fixtures and family/precision obligations through the immutable release. Expectations cannot be generated by production extraction or copied from its output.
4. Introduce the proposed real-time-cpg-packet-check, real-time-cpg-milestone-check and real-time-cpg-decommission-check dispatchers plus the initial real-time-cpg-certification orchestration contract. Use zero-selection-safe oracle discovery: future packets remain unimplemented and fail selection until their four real tests exist. Seed missing/duplicate/no-op oracle, child-failure, recursive-gate and inactive-draft-without-state tests; settle all candidate-affecting terminal tooling by WP105, before WP104 final capture.
5. Carry all retained v7 behavioral, security and architecture obligations into successor gate discovery. Do not run or restamp v7 WP65 measurements as successor evidence.

**Legacy disposition and decommission.** Preserve useful v7 target implementations and existing physical deletions. DB24 rechecks old marker/profile/global-release/generated-type/feature authorities; DB25 owns replacement of provisional CFG semantics. No new production branch.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp77_behavior`, invoked by `just real-time-cpg-packet-check WP77`: independent job/release/capability cases distinguish known-empty, missing implementation, admitted uncertainty and bad required evidence.

##### Structural

- Proposed test `rt_cpg_wp77_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp77_faults`, invoked by the same packet recipe: mutate a complete claim, producer operand, required expectation, feature edge or test selector; the owning checker rejects it. Native falsifiers additionally cover compressed oversized checkpoints, large/nested CRC/schema, cumulative small-action replay and reject-before-allocation counters, including resource exhaustion through optional CRC and checkpoint-hint fallback. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp77_operations`, invoked by the same packet recipe: feature-isolated construction and oracle discovery work in a fresh repository shell without active successor state.

Oracle catalog:

Executable oracle: `rt_cpg_wp77_integrity`
Governed criterion: `PC-WP77-INT`

Executable oracle: `rt_cpg_wp77_behavior`
Governed criterion: `PC-WP77-BEH`

Executable oracle: `rt_cpg_wp77_faults`
Governed criterion: `PC-WP77-NEG`

Executable oracle: `rt_cpg_wp77_operations`
Governed criterion: `PC-WP77-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP77`; `just provider-job-contract-check`; `just release-program-contract-check`; `just compiled-suite-identity-check`; `just feature-architecture-check`; `just features-no-default`; `just stable-graph-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M18.

**Replan Triggers.** Reopen if retained release/provider boundaries cannot express closed support and capability without outward dependencies or if the proposed successor would drop a v7 requirement. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Before publication changes, restore only this packet's edits as a coherent unit. Keep explicit gaps; never restore a false complete claim.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP78 — Close source inventory, effective contexts and dependency support contracts

**Outcome.** An immutable source/context job identifies all selected inputs and support obligations, including failures and formerly absent lookup candidates.

**Dependencies.** WP77. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Each source has a disposition; exact effective configuration drives provider jobs; positive and negative support define lawful reuse. Advances P9/P10/P27/P28.

**Design and library references.** Review F03/F05, §§3.2–3.4; D-RT01/D-RT02; GEN §8 Immutable source-image contract, AC-G-14 Analysis-context discovery, identity, and selection, AC-G-33/34/43; LIFE §§4–7; LD-04/05.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/source_image.rs src/analysis_context.rs src/python_context.rs src/provider_contracts.rs src/provider_admission.rs src/production_provider_recipe.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'CaptureOutcome|capture_with|AnalysisContext|PythonContextDiscovery|ProviderJob|dependency|support' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/source_image.rs`, `src/analysis_context.rs`, `src/python_context.rs`, `src/provider_contracts.rs`, `src/provider_admission.rs`, `src/production_provider_recipe.rs`, `contracts/rpc/provider_control.proto`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Specify and implement the D-RT01 support relation dimensions and family-specific typed lookup variants. Persist search scope, root order, namespace and policy/context selection for negative dependencies; unknown support widens invalidation.
2. Separate full selected source/module inventory from changed-work subsets. Every included/excluded/oversized/binary/generated/vendored/unreadable source has the GEN-prescribed disposition; deferred/unstable reads remain pending.
3. Connect caller-owned source generation fences to descriptor-relative capture; inventory closure and byte capture use one authorized boundary. Preserve byte-safe paths and path-based rename identity.
4. Discover actual Python version/platform/module map/search roots/stubs and Rust package/target/features/cfg/toolchain/dependency/build inputs. Effective manifests, not ad hoc hashes, prepare jobs. Context settings must have causal consumers.
5. Define complete owner/family/context replacement, withdrawal and capture/coverage/provenance obligations with immediate job/admission consumers and fixtures; no inert schema-only contract.

**Legacy disposition and decommission.** Remove convenience constant-fence/ad hoc-context construction from production consumers when replaced; preserve bounded fixture helpers as non-production only if explicitly useful. DB26 verifies the live route.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp78_behavior`, invoked by `just real-time-cpg-packet-check WP78`: context or inventory changes with unchanged source bytes alter prepared jobs and dependency support; every capture outcome is observable.

##### Structural

- Proposed test `rt_cpg_wp78_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp78_faults`, invoked by the same packet recipe: omit a source disposition, swap context, forget a failed lookup, or label a dirty subset as full inventory and expect rejection or conservative widening. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp78_operations`, invoked by the same packet recipe: capture races, non-UTF8 paths and multiple contexts preserve bounded work and deterministic terminal dispositions.

Oracle catalog:

Executable oracle: `rt_cpg_wp78_integrity`
Governed criterion: `PC-WP78-INT`

Executable oracle: `rt_cpg_wp78_behavior`
Governed criterion: `PC-WP78-BEH`

Executable oracle: `rt_cpg_wp78_faults`
Governed criterion: `PC-WP78-NEG`

Executable oracle: `rt_cpg_wp78_operations`
Governed criterion: `PC-WP78-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP78`; `just source-capture-race-check`; `just provider-job-contract-check`; `just provider-trust-coverage-remainder-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M18.

**Replan Triggers.** Reopen if a context input cannot be captured under source authority or a support record cannot conservatively represent an API's hidden dependency universe. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Unclosed inventory prevents a new complete-source claim. Preserve the selected predecessor and keep affected scopes dirty; never erase pending work.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP79 — Install aggregate resource ownership and joined operation primitives

**Outcome.** All subsequent provider, update and query work has a common finite admission/accounting contract that cannot multiply memory by creating epochs.

**Dependencies.** WP78. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** One process/workspace envelope covers providers, epochs, queues, graphs, results and spill; reservations have one owner and release after joined termination. Advances P23/P31/P34.

**Design and library references.** Review F13/F14, §3.9; D-RT05; planning-contracts v2 §§2–6 and LD-RT09; v7 WP59/WP61; LIFE §6 and cancellation/lease obligations; LD-02/07.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/cancellation.rs src/fabric/epoch_runtime.rs src/fabric/query_coordinator.rs src/fabric/command_runtime_manager.rs src/fabric/streamed_result_registry.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'FabricEpochRuntimeConfig|MemoryPool|Cancellation|JoinHandle|spawn_blocking|Resource|lease' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/cancellation.rs`, `src/fabric/epoch_runtime.rs`, `src/fabric/query_coordinator.rs`, `src/fabric/command_runtime_manager.rs`, `src/fabric/streamed_result_registry.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Introduce validated hierarchical reservations, owner identity and aggregation over current/candidate/leased epochs, provider processes, Arrow/IPC/AST/graph allocations, caches, spill and results. Wire immediate existing runtime constructors.
2. Bound rows, bytes, individual values/pages, queued/running jobs, retained generations and disk headroom. Reserve before large work; handle shared immutable allocations and retained slices explicitly.
3. Provide owned async/CPU/process operation handles, cooperative cancellation probes, deadline cleanup reserve and exactly-once joined release. Cancellation intent or stream drop alone is not task termination.
4. Reserve control/status/cancel/read/release capacity separately from heavy admission. Under pressure defer/reject new work without revoking valid leases or reporting partial output complete.
5. Expose actual budget consumption, queue age, cancellation-to-join and retained bytes as observations. Numerical release profiles remain WP103's measured policy, not guessed optimal defaults.
6. Implement LD-RT09 on exact native sources: reserve before JSON/CRC/schema/checkpoint/footer/page/dictionary/value decode and cumulative replay growth; preserve typed capacity failures through optional fallback; expose original eager backing and lifetime-shared retained non-Arrow ownership. Include both Delta engine construction routes and all actual worker/channel/fanout limits. Existing native limits are reusable only with a source-derived sufficient bound.
7. Integrate the amended native sources and their application consumers in one Git branch, with tracked source directories, relative Cargo paths and exact resolved-source checks. Preserve upstream origins, licenses, the Arrow/DataFusion universe and four first-party build domains. Do not edit Cargo's cache or depend on temporary staging directories. Source packaging and suite successor issuance are not integration prerequisites.
8. Integrate joined native operations, owned stores and continuing native receipts into production control, candidate, reconstructed epoch, query, CDF and maintenance phases. Retain physical file charges through uncertain writes and joined cleanup/reconciliation. DB30 removes all corresponding bypasses; physical census never selects Delta semantic state.

**Legacy disposition and decommission.** Replace isolated full-budget epoch constructors and unowned bridge tasks in the touched route; do not delete sound v7 task-tree infrastructure. DB30 checks absence of alternate ownership.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp79_behavior`, invoked by `just real-time-cpg-packet-check WP79`: concurrent candidates, queries and retained epochs share one enforceable envelope and release charges once. Native eager clones preserve original buffer identity and shared charges until the final owner; deserialization/reconstruction cannot create uncharged retained state.

##### Structural

- Proposed test `rt_cpg_wp79_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp79_faults`, invoked by the same packet recipe: clone a full per-epoch budget, detach CPU work, omit a buffer charge or admit a huge single value and observe rejection. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp79_operations`, invoked by the same packet recipe: live cancellation under full data admission joins hidden native workers, validates finite environment/physical fanout and leaves reserved control operations responsive. No lane output retains a terminated runtime; retained receipts and uncertain physical storage survive observer cancellation.

Oracle catalog:

Executable oracle: `rt_cpg_wp79_integrity`
Governed criterion: `PC-WP79-INT`

Executable oracle: `rt_cpg_wp79_behavior`
Governed criterion: `PC-WP79-BEH`

Executable oracle: `rt_cpg_wp79_faults`
Governed criterion: `PC-WP79-NEG`

Executable oracle: `rt_cpg_wp79_operations`
Governed criterion: `PC-WP79-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-native-resource-check`; `just data-fabric-upgrade-check`; `just stable-graph-check`; `just features-each`; `just policy`; `just real-time-cpg-packet-check WP79`; `just cancellation-tree-check`; `just grpc-flow-control-contract-check`; `just grpc-slow-consumer-check`; `just datafusion-cache-resource-operations-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M18.

**Replan Triggers.** The LD-RT09 native-resource reopening is approved and is mandatory implementation work here. Newly discovered allocation paths inside its declared boundary expand the dependency-closed write set. Reopen for a different engine/process/semantic authority, changed public type universe, or evidence that this amended contract itself cannot provide safe finite ownership. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Stop admission, cancel and join owned operations, reconcile irreversible effects, then release reservations. Do not kill unrelated workspace processes.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP80 — Complete both source/syntax lanes using retained Tree-sitter and Ruff

**Outcome.** Python and Rust source/syntax/lexical facts are available through owned provider relations, with truthful parse/capture coverage and reusable bounded native state.

**Dependencies.** WP79. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Both languages retain raw and normalized source/syntax evidence over exact bytes; borrowed vendor state never escapes. Maintains P8/P22/P35; advances P20/P27.

**Design and library references.** Review F01/F04/F05, §3.2; D-RT01/02/03; GEN §§14–18 and §§34–35 plus §67A; LD-02/04/05.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/tree_sitter_adapter.rs src/ruff_adapter.rs src/provider_native_syntax.rs src/source_image.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ExactPythonSyntaxRunner|run_full|run_incremental|Tree::|InputEdit|Ruff|SourceType' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/tree_sitter_adapter.rs`, `src/ruff_adapter.rs`, `src/provider_native_syntax.rs`, `src/source_image.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Compose Tree-sitter for both exact grammars and Ruff for Python; do not treat ExactPythonSyntaxRunner as a two-language implementation.
2. Use edited prior Tree-sitter trees, actual changed ranges plus textual/enclosing-owner invalidation, reused compiled queries and bounded parser/tree caches. Ruff whole-file reparse remains explicit.
3. Emit tokens/trivia/ranges, syntax occurrences, error nodes, raw kinds and normalized categories, lexical/scope inputs and source/generated correspondence required by the selected profile.
4. Preserve malformed/incomplete files as current syntax with explicit parse remainders; implement line/byte/encoding transformations against immutable images.
5. Integrate job bounds, capture fences, cancellation and terminal coverage immediately; clean and incremental facts must match independently expected examples.

**Legacy disposition and decommission.** Replace Python-only selection and redundant full syntax extraction on unchanged files in its owning lane. DB26 removes startup-only selection after integration.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp80_behavior`, invoked by `just real-time-cpg-packet-check WP80`: real Python and Rust constructs, edits and parse errors produce expected owned facts and clean-equivalent incremental output.

##### Structural

- Proposed test `rt_cpg_wp80_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp80_faults`, invoked by the same packet recipe: wrong grammar/revision, stale tree edit, dropped parse error, borrowed native value or silent skipped Rust file is detected. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp80_operations`, invoked by the same packet recipe: large files, cancellation, cache pressure and repeated edits stay within shared budgets with no unbounded tree retention.

Oracle catalog:

Executable oracle: `rt_cpg_wp80_integrity`
Governed criterion: `PC-WP80-INT`

Executable oracle: `rt_cpg_wp80_behavior`
Governed criterion: `PC-WP80-BEH`

Executable oracle: `rt_cpg_wp80_faults`
Governed criterion: `PC-WP80-NEG`

Executable oracle: `rt_cpg_wp80_operations`
Governed criterion: `PC-WP80-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP80`; `just inprocess-provider-lifecycle-check`; `just exact-provider-batch-check`; `just provider-type-boundary-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M19.

**Replan Triggers.** Reopen if grammar coverage lacks a required raw representation; preserve unknowns instead of substituting normalized semantics. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Discard invalid cached native state and reparse exact source; cache failure cannot alter selected source or semantic authority.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP81 — Make retained Pyrefly contexts effective and complete selected semantic extraction

**Outcome.** Actual Pyrefly facts follow the selected Python context and include the available exact-pin type, call, member, import, definition and cross-reference surfaces.

**Dependencies.** WP79. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Validated configuration changes actual checker behavior; one compatible context owns retained state and complete inventory separately from dirty work. Advances P27; maintains P22/P35.

**Design and library references.** Review F03/F04, §3.2; D-RT01/02; GEN §§19–23, §§31–33, §71A, AC-G-14/30/36/40; LD-04/07.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/pyrefly_service.rs src/pyrefly_relation_schema.rs pyrefly-sidecar/src/server.rs pyrefly-sidecar/src/pyrefly_link.rs pyrefly-sidecar/src/protocol.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'SemanticContext::new|analyze_modules|change_files|ConfigFile|AnalysisContext|Pyrefly' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/pyrefly_service.rs`, `src/pyrefly_relation_schema.rs`, `pyrefly-sidecar/src/server.rs`, `pyrefly-sidecar/src/pyrefly_link.rs`, `pyrefly-sidecar/src/protocol.rs`, `contracts/rpc/pyrefly_sidecar.proto`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Construct native settings from frozen Python version/platform, module map, ordered search roots, stubs and policy; verify unchanged source with changed configuration alters resolved facts.
2. Separate complete inventory from changed modules at daemon/protocol/native boundaries. Changing A must not remove unchanged B from retained checker state.
3. Treat delete-last-module as a valid empty-inventory transition: withdraw facts/support, join or retire native state, emit proved empty coverage, and support recreation. Do not preserve the current empty-list rejection as a provider-failure shortcut.
4. Integrate bulk Query inferred/call/member information, selected TSP declared/computed/expected/import semantics and selected Glean/internal definitions/xrefs. Add on-demand Query::is_subtype and the exact supported Query::resolve_target_from_qualified_name target class with typed admission/consumers in WP83/WP98, false-subtype/unsupported-target/type-distinction tests and bounded epoch/context ownership. No persisted all-pairs subtype closure or syntax substitution. Navigation fallback is narrow, not a per-node bulk engine.
5. Keep source/context/provider/schema/coverage/actual affected-scope evidence in relation-scoped IPC. If actual recheck scope is unavailable, use conservative context/reverse-closure recomputation, not requested-files-as-proof.
6. Serialize context mutation; preserve healthy state through joined cancellation; reconstruct on incompatible context, corruption, crash or trust loss. Missing implementation of an available required surface is not a terminal API gap.

**Legacy disposition and decommission.** Replace default-config checker construction and disposable/incorrectly rebound state. Keep isolated sidecar build domain; no Pyrefly type or process authority enters the generic fabric. DB26.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp81_behavior`, invoked by `just real-time-cpg-packet-check WP81`: configuration, stub, import root and module-create/delete changes causally produce expected semantic facts while unaffected inventory remains retained.

##### Structural

- Proposed test `rt_cpg_wp81_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp81_faults`, invoked by the same packet recipe: hash-only context change, dirty-subset-as-inventory, wrong IPC trailer, cross-context reuse and stale completion fail; delete-last/recreate cannot leave stale module facts. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp81_operations`, invoked by the same packet recipe: retained state survives lawful cancellation and bounded churn; crash recovery reconstructs from exact immutable context without leaks.

Oracle catalog:

Executable oracle: `rt_cpg_wp81_integrity`
Governed criterion: `PC-WP81-INT`

Executable oracle: `rt_cpg_wp81_behavior`
Governed criterion: `PC-WP81-BEH`

Executable oracle: `rt_cpg_wp81_faults`
Governed criterion: `PC-WP81-NEG`

Executable oracle: `rt_cpg_wp81_operations`
Governed criterion: `PC-WP81-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP81`; `just pyrefly-incremental-lifecycle-check`; `just exact-provider-batch-check`; `just provider-ipc-contract-integrity-check`; `just relation-ipc-provider-operations-check`; `just sidecar-ci-fast`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M19.

**Replan Triggers.** Reopen a selected surface only on exact-pin source/probe evidence that it is unavailable or semantically incompatible; do not silently substitute syntax for checker facts. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Cancel/join, invalidate incompatible context, emit scoped gap, reconstruct the checker from the full exact inventory and retry under a new lawful job.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP82 — Install contained Rust compilation and the complete selected extractor profile

**Outcome.** Actual contained Rust extraction produces the selected source/definition/type/MIR/private-enrichment families for real workspaces, not only successful refusals.

**Dependencies.** WP79. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Compiler work is exact-context and contained; public MIR/private enrichment/derived facts are distinguishable; no raw DefId becomes canonical. Maintains P22/P35; advances P20/P27.

**Design and library references.** Review F01/F04, §3.2; D-RT01/02/03; GEN §§34–51, §71B, AC-G-31/34/35/36/40; LD-04/07.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/rust_compilation_trust.rs src/rustc_service.rs src/rustc_relation_schema.rs rustc-extractor/src/rustc_link.rs rustc-extractor/src/protocol.rs rustc-extractor/src/wrapper.rs rustc-extractor/src/main.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'run_untrusted_rustc_provider_lifecycle|Compiler|ProviderJob|MIR|monomorph|vtable|loan|hygiene' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/rust_compilation_trust.rs`, `src/rustc_service.rs`, `src/rustc_relation_schema.rs`, `rustc-extractor/src/rustc_link.rs`, `rustc-extractor/src/protocol.rs`, `rustc-extractor/src/wrapper.rs`, `rustc-extractor/src/main.rs`, `contracts/rpc/rustc_extractor.proto`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Select package/target/features/cfg/toolchain/dependency/build-script/proc-macro inputs from immutable effective contexts; allow compiler incrementality only inside that authority.
2. Wire the actual trust launcher and extractor lifecycle. Bound environment, credentials, network, output, CPU/memory/time and process-group cleanup; unavailable containment emits a typed gap without host fallback.
3. Complete selected typed definitions/types/generics/MIR terminators, normal/unwind/cleanup/resume edges, places/projections/accesses and call/instance targets; distinguish generic definitions from executable instances.
4. Integrate exact narrow stable keys, hygiene/generated spans, borrowck loans/regions, mono/vtable/shim/drop glue and structured diagnostics where the pinned profile requires them. Preserve CTFE/statics/constants, unsafe/FFI/inline assembly and async-lowering distinctions.
5. Expose missing dependency bodies, compile failure, unsupported private variants and dynamic dispatch remainders honestly, retaining current source/syntax while withdrawing invalidated compiler facts.

**Legacy disposition and decommission.** Replace permanently absent Rust production lane and debug-string/unstable-key shortcuts; retain dedicated nightly root and typed transport. DB26.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp82_behavior`, invoked by `just real-time-cpg-packet-check WP82`: real multi-target Rust fixtures successfully compile in containment and yield independently expected typed/profile facts.

##### Structural

- Proposed test `rt_cpg_wp82_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp82_faults`, invoked by the same packet recipe: trust escape, raw provider identity, context mismatch, compile-failed stale facts, missing MIR variant and corrupted terminal IPC are rejected. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp82_operations`, invoked by the same packet recipe: actual compilation cancellation kills and joins only owned process groups; incremental and cold jobs preserve exact-context equivalence.

Oracle catalog:

Executable oracle: `rt_cpg_wp82_integrity`
Governed criterion: `PC-WP82-INT`

Executable oracle: `rt_cpg_wp82_behavior`
Governed criterion: `PC-WP82-BEH`

Executable oracle: `rt_cpg_wp82_faults`
Governed criterion: `PC-WP82-NEG`

Executable oracle: `rt_cpg_wp82_operations`
Governed criterion: `PC-WP82-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP82`; `just rustc-provider-lifecycle-check`; `just semantic-sandbox-host-matrix-check`; `just exact-provider-batch-check`; `just generated-type-boundary-check`; `just extractor-ci-fast`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M19.

**Replan Triggers.** Reopen if supported-host containment cannot safely execute required build inputs or the exact private seam cannot supply a selected required family; never silently waive it. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Retain source/syntax and explicit compiler gap, discard uncertain compiler outputs, reconstruct from exact context and reconcile before another publication.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP83 — Normalize real provider output into the installed two-language fabric

**Outcome.** A fresh real daemon exposes meaningful canonical entities/facts for Python and Rust with exact provider/context coverage, replacing unconditional external-provider gaps.

**Dependencies.** WP80, WP81, WP82. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Every canonical entity/fact is application-owned, raw evidence survives, conflicts/unknowns are explicit, and the actual daemon consumes all four providers. Advances P2/P9/P10/P27.

**Design and library references.** Review F01/F04/F06, §§3.1–3.3; D-RT01/02; GEN §§5,12–13,67–86,92–96; ONT required identity/relationship families; LD-01/02/04.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/production_provider_recipe.rs src/provider_admission.rs src/programmatic_derived_analysis.rs src/fabric/production_workspace_startup.rs src/production_query_recipe.rs tests/integration/daemon.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ProductionProviderRuns|RequiredInputAbsent|admit_and_compose|Normalization|Reconciliation|compiled_find_entities' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/production_provider_recipe.rs`, `src/provider_admission.rs`, `src/programmatic_derived_analysis.rs`, `src/fabric/production_workspace_startup.rs`, `src/production_query_recipe.rs`, `tests/integration/daemon.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Connect all four lane results through one release-owned admission and canonical transformation recipe. Close entity/type/call/range/declaration reconciliation and preserve competing evidence according to GEN authority.
2. Include complete module/import/export, scopes/bindings, callable/member/type contracts, generated/lowered correspondence, diagnostics and typed relationship families—not only functions. Preserve syntax occurrence versus semantic entity versus Rust instance.
3. Integrate context/trust/source capability into installed producer closure and query discovery. Required input missing is not an empty-success relation.
4. Make genesis the initial case of the evolving runtime, not a separate authority. Bind canonical provider output to the existing initial entity/fact/source query path for both languages; WP98 extends that same compiler to the full contract.
5. Prove real source-to-owned-Arrow-to-native-transformation-to-exact-Delta-to-daemon causality with independently authored fixtures. Downstream derived families remain explicit gaps until their own packets complete.

**Legacy disposition and decommission.** Delete unconditional Pyrefly/rustc gap construction and Python-only production selection when actual lanes are installed. Do not keep a hidden old startup route. DB24/DB26.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp83_behavior`, invoked by `just real-time-cpg-packet-check WP83`: both-language initial source changes alter expected canonical entities/facts through the installed daemon with raw provenance retained.

##### Structural

- Proposed test `rt_cpg_wp83_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp83_faults`, invoked by the same packet recipe: disable a provider, swap context/source, drop a conflict/unknown or bypass admission and the installed vertical fails. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp83_operations`, invoked by the same packet recipe: fresh startup, empty workspace, provider outage and contained compile failure yield correct lifecycle/capability outcomes.

Oracle catalog:

Executable oracle: `rt_cpg_wp83_integrity`
Governed criterion: `PC-WP83-INT`

Executable oracle: `rt_cpg_wp83_behavior`
Governed criterion: `PC-WP83-BEH`

Executable oracle: `rt_cpg_wp83_faults`
Governed criterion: `PC-WP83-NEG`

Executable oracle: `rt_cpg_wp83_operations`
Governed criterion: `PC-WP83-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP83`; `just programmatic-production-composition-check`; `just production-composition-contract-integrity-check`; `just provider-admission-exclusivity-check`; `just semantic-release-vertical-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M19.

**Replan Triggers.** Reopen if canonical reconciliation needs a second semantic owner or an existing public response cannot represent required fact/unknown distinctions. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep admission closed until one coherent genesis/successor exists; preserve exact previous epoch if later integration fails.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP84 — Replace sequential Python CFG with real control/evaluation and core dataflow

**Outcome.** Installed Python CFG/evaluation, definitions/uses, reaching definitions, liveness and value flow match independently expected branch/loop/exception semantics.

**Dependencies.** WP83. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Owner-local control semantics govern precision and transfer; nested callables do not fall through. Advances P1/P2/P14/P30.

**Design and library references.** Review F02, §3.3; D-RT03; GEN §§24–25,28,30,59–60,73–74,77A/77E; ONT control/dataflow; LD-01/04/06.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/ruff_adapter/cfg.rs src/ruff_adapter/dataflow.rs src/python_derived_analysis.rs src/programmatic_derived_analysis.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ProgrammaticPythonCfg|evaluation_ordinal|compile_python_owner_flow|Reaching|Liveness|Cfg' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/ruff_adapter/cfg.rs`, `src/ruff_adapter/dataflow.rs`, `src/python_derived_analysis.rs`, `src/programmatic_derived_analysis.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Implement D-RT03 module/class/callable/comprehension ownership and evaluation/program points; integrate useful existing kernels as named application-derived analyses.
2. Represent short circuit, condition alternatives, loop backs, abrupt completion, handlers/finally/context-manager cleanup and suspension/resumption without file-level sequential approximation.
3. Build visible native relations and native recursive transfer where semantically faithful; justify any irreducible application kernel and expose its input/output/support contracts.
4. Specify the finite intraprocedural abstractions, transfer/join and exactness scope. Include nonconvergence/unknown successors and bounds, never literal complete independent of the proof.
5. Replace the provisional CFG path and its inherited Exact dataflow claims atomically with correct analysis and immediate producer/query consumers.

**Legacy disposition and decommission.** Remove adjacency-as-CFG and file-owned complete-control projections, registrations and fixtures that bless them; keep evaluation order as a distinct valid relation. DB25.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp84_behavior`, invoked by `just real-time-cpg-packet-check WP84`: independent nested-owner, branch, loop, return, try/finally, with, comprehension, yield and await cases produce expected edges and dataflow.

##### Structural

- Proposed test `rt_cpg_wp84_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp84_faults`, invoked by the same packet recipe: replace a successor with AST adjacency, erase cleanup/back edges or merge nested owners and the tests fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp84_operations`, invoked by the same packet recipe: deep/nested/error syntax and cyclic flow converge or emit a bounded explicit remainder; active analysis cancellation is joined.

Oracle catalog:

Executable oracle: `rt_cpg_wp84_integrity`
Governed criterion: `PC-WP84-INT`

Executable oracle: `rt_cpg_wp84_behavior`
Governed criterion: `PC-WP84-BEH`

Executable oracle: `rt_cpg_wp84_faults`
Governed criterion: `PC-WP84-NEG`

Executable oracle: `rt_cpg_wp84_operations`
Governed criterion: `PC-WP84-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP84`; `just analysis-producer-semantic-check`; `just analysis-causal-fault-check`; `just analysis-fixed-point-resource-check`; `just wave8-integration-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M20.

**Replan Triggers.** Reopen if the chosen control abstraction cannot represent required Python abrupt/exception/suspension semantics without falsely narrowing the selected profile. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Withdraw affected derived families and expose an explicit gap rather than falling back to the sequential CFG.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP85 — Complete Python object, memory, effect, resource and async analyses

**Outcome.** The selected Python profile covers advanced object-model, alias/effect/resource/exception/async/capture and program-point state families in the canonical graph.

**Dependencies.** WP84. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Advanced facts remain mechanically derived, support-bound and precision-scoped; dynamic uncertainty is represented rather than judged. Advances P9/P20/P30.

**Design and library references.** Review F04, §§3.3,6.2; D-RT01/03; GEN §§20–33,61,66,71A,75–77E and AC-G-39; LD-01/04/06.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/python_derived_analysis.rs src/programmatic_derived_analysis.rs src/ruff_adapter.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'Python|Alias|PointsTo|Effect|Resource|Async|Capture|Summary|Descriptor|Mro' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/python_derived_analysis.rs`, `src/programmatic_derived_analysis.rs`, `src/ruff_adapter.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Integrate class/instance/member distinctions, MRO, descriptor/property/protocol/metaclass behavior, overloads/decorators/synthesis, patterns and lexical capture/global/nonlocal semantics using provider authority and explicit dynamic remainders.
2. Derive finite memory/alias/points-to, reads/writes/escapes/effects, exception/cleanup, resource lifetime, context-manager and async/generator/concurrency candidate facts from actual control/access observations.
3. Define selected precision, abstract locations, call/context sensitivity, widening/bounds and support provenance per family; unknown calls propagate uncertainty through relevant summaries.
4. Make closure/environment and resource/program-point state relationships queryable canonically, not hidden in provider DTOs.
5. Cover GEN's lettered relationship sections and independent tests for formerly unresolved imports/members, deletions and context changes.

**Legacy disposition and decommission.** Replace duplicate/unbound Python analysis routes and unwarranted precise/complete defaults; do not redefine raw Ruff/Pyrefly evidence as derived facts. DB25.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp85_behavior`, invoked by `just real-time-cpg-packet-check WP85`: independent object-model, closure, indirect-call, alias/resource and async fixtures expose correct facts, support and scoped unknowns.

##### Structural

- Proposed test `rt_cpg_wp85_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp85_faults`, invoked by the same packet recipe: omit unknown-call effects, overwrite conflict evidence, collapse class/instance or claim exact dynamic targets and the corpus fails. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp85_operations`, invoked by the same packet recipe: high-fanout/recursive inputs obey allocation and fixed-point budgets with deterministic bounded remainders.

Oracle catalog:

Executable oracle: `rt_cpg_wp85_integrity`
Governed criterion: `PC-WP85-INT`

Executable oracle: `rt_cpg_wp85_behavior`
Governed criterion: `PC-WP85-BEH`

Executable oracle: `rt_cpg_wp85_faults`
Governed criterion: `PC-WP85-NEG`

Executable oracle: `rt_cpg_wp85_operations`
Governed criterion: `PC-WP85-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP85`; `just analysis-producer-semantic-check`; `just analysis-causal-fault-check`; `just analysis-fixed-point-resource-check`; `just provider-trust-coverage-remainder-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M20.

**Replan Triggers.** Reopen an abstraction if it cannot preserve required soundness or unknown propagation; do not remove a selected family to make conformance green. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Regenerate complete affected owner/family output or withdraw it with a truthful remainder; never preserve a stale precise summary.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP86 — Complete Rust MIR-derived ownership, flow, effects and state analyses

**Outcome.** Rust canonical derived facts cover initialization/move/borrow state, def-use/liveness, alias/points-to, effects/resources/async and program-point state at the selected precision.

**Dependencies.** WP85. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Typed MIR is the input authority; exact private borrow facts and derived approximations stay distinct; non-SSA/unwind behavior is preserved. Advances P6/P9/P30.

**Design and library references.** Review F04, §3.3; D-RT01/03; GEN §§39–51,59–61,66,71B,73–77E and AC-G-39; LD-01/04/06. Dependency on WP85 serializes the shared programmatic analysis integration surface.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/rust_mir_derived_analysis.rs src/programmatic_derived_analysis.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'analyze_rust_mir_relations|Move|Borrow|Init|Liveness|PointsTo|Drop|Unwind|Summary' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/rust_mir_derived_analysis.rs`, `src/programmatic_derived_analysis.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Map exact typed MIR places/projections, access contexts, normal/unwind/cleanup/resume successors and owner/instance identities into analysis inputs.
2. Implement non-SSA reaching definitions, liveness, initialization/move/drop and ownership-state transfer; keep compiler-private loans/regions separate from derived borrow approximations.
3. Derive selected memory/alias, effects, escape/resource, async/coroutine and summary facts, including raw pointers, unions, FFI and unavailable external bodies with explicit uncertainty.
4. Bind each output to algorithm release, input projection/epoch/context, support, precision, completeness and bounds; integrate the production transformation recipe.
5. Author independent expected MIR-shaped and real compiled examples so clean-versus-incremental equality cannot bless a wrong abstraction.

**Legacy disposition and decommission.** Retire string-parsed/duplicate MIR-derived authority and stale-current outputs after compiler failure. Preserve raw public/private evidence. DB25.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp86_behavior`, invoked by `just real-time-cpg-packet-check WP86`: real and independent MIR cases produce expected move/init/use/drop/unwind, alias and effect states.

##### Structural

- Proposed test `rt_cpg_wp86_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp86_faults`, invoked by the same packet recipe: drop an unwind successor, conflate generic body with instance, mislabel private versus derived loans or omit unknown FFI effects and tests fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp86_operations`, invoked by the same packet recipe: large CFGs, recursive summaries, compile failure and active cancellation stay bounded and reconstructible.

Oracle catalog:

Executable oracle: `rt_cpg_wp86_integrity`
Governed criterion: `PC-WP86-INT`

Executable oracle: `rt_cpg_wp86_behavior`
Governed criterion: `PC-WP86-BEH`

Executable oracle: `rt_cpg_wp86_faults`
Governed criterion: `PC-WP86-NEG`

Executable oracle: `rt_cpg_wp86_operations`
Governed criterion: `PC-WP86-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP86`; `just analysis-producer-semantic-check`; `just analysis-causal-fault-check`; `just analysis-fixed-point-resource-check`; `just rustc-provider-lifecycle-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M20.

**Replan Triggers.** Reopen if selected MIR/body availability cannot support the promised abstraction; retain explicit unavailable facts rather than debug-string inference. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Withdraw invalidated compiler-derived partitions and rebuild from exact successful compiler inputs; do not restore last-known-good as current.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP87 — Install common graph analyses and dependency-driven SCC summaries

**Outcome.** Correct shared graph facts and interprocedural summaries execute over bounded canonical projections for both languages and update soundly after graph change.

**Dependencies.** WP86. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Graph indices are execution-local; analysis input includes isolated nodes and edge identity; deletions and SCC changes invalidate supported results. Advances P14/P23/P28.

**Design and library references.** Review F14, §§3.3–3.4; D-RT01/03/05; GEN §§52–66,79,91,95; LD-01/06.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/common_derived_analysis.rs src/programmatic_derived_analysis.rs src/fabric/graph_program.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'analyze_common_derived|compare_clean_incremental|Scc|dominator|dominance|summary|fixed_point|projection' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/common_derived_analysis.rs`, `src/programmatic_derived_analysis.rs`, `src/fabric/graph_program.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Build explicit node plus typed multiedge projections preserving isolated entities and canonical fact IDs; use Graph/Reversed/filtered views or measured Csr only with their semantics preserved.
2. Implement reachability, SCC/recursion, dominance/postdominance, control dependence, loops, connected components, distances and bounded closure/reduction/objective structural measures required by GEN.
3. Define entries, unreachable nodes, multiple exits, parallel edges and canonical tie-breaking. Prefer native DataFusion/recursive execution; justify bounded petgraph SCC/dominator/topology kernels per selected rung.
4. Run affected-SCC summaries using a changed-summary worklist and correct unknown propagation; recompute when deletions split SCCs or change support, not an add-only fixed point across epochs.
5. Reserve and bound graph/summary memory and intermediate work; native uninterruptible algorithms require safe input envelopes or a justified cooperative implementation.

**Legacy disposition and decommission.** Remove unbounded all-owner summary copying and incorrect graph projections when replaced. DB25/DB30; no persistent petgraph authority.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp87_behavior`, invoked by `just real-time-cpg-packet-check WP87`: independent chain/diamond/cycle/multi-exit/isolated/multiedge fixtures produce expected shared facts and clean-equivalent changed summaries.

##### Structural

- Proposed test `rt_cpg_wp87_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp87_faults`, invoked by the same packet recipe: erase isolated nodes, keep a deleted path, mishandle SCC split, alter tie-breaking or omit unknown support and tests fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp87_operations`, invoked by the same packet recipe: deep graphs, dense frontiers and cancellation exhaust explicit bounds safely with joined CPU work.

Oracle catalog:

Executable oracle: `rt_cpg_wp87_integrity`
Governed criterion: `PC-WP87-INT`

Executable oracle: `rt_cpg_wp87_behavior`
Governed criterion: `PC-WP87-BEH`

Executable oracle: `rt_cpg_wp87_faults`
Governed criterion: `PC-WP87-NEG`

Executable oracle: `rt_cpg_wp87_operations`
Governed criterion: `PC-WP87-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP87`; `just analysis-producer-semantic-check`; `just analysis-fixed-point-resource-check`; `just graph-query-resource-operations-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M20.

**Replan Triggers.** Reopen if a native algorithm cannot preserve required graph semantics within the accepted resource/cancellation boundary; no hidden second engine. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Discard incomplete analysis output and recompute the full affected component/context with explicit remainder if bounded precision cannot converge.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP88 — Own a live watcher, dirty registry and authoritative reconciliation loop

**Outcome.** One running daemon observes Python/Rust source and selected context changes and keeps bounded, generation-fenced work pending until reconciled.

**Dependencies.** WP83. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** notify is urgency, gix is candidate acceleration, exact authorized bytes are truth; events cannot disappear during reconciliation. Advances P16/P23/P28.

**Design and library references.** Review F01/F05, §3.2; D-RT01/02/05; LIFE §§4–6; GEN source/context clauses; LD-05.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/git_state.rs src/source_image.rs src/fabric/production_workspace_startup.rs src/fabric/source_wave_command_effect.rs src/daemon.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'GitCandidatePlanner|SourceWaveResolverPort|SourceWaveCommitPort|SourceWaveMarkerPort|PublishSourceWave|notify|dirty' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/git_state.rs`, `src/source_image.rs`, `src/fabric/production_workspace_startup.rs`, `src/fabric/source_wave_command_effect.rs`, `src/daemon.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Construct and own the pinned debouncer/watcher and one bounded dirty-scope registry; watch selected source/config/stub/dependency roots only within explicit authority.
2. Coalesce events by scope/generation/reason; overflow, loss, root replacement, ambiguous rename and need_rescan trigger authorized reconciliation. Events arriving during scans remain dirty.
3. Integrate gix present-state candidate acceleration with generic inventory parity, ignore/untracked/linked-worktree/byte-safe handling and fallback when Git is unsupported or untrusted.
4. Implement concrete read-only SourceWaveResolverPort plus commit/marker effects and immediate factory consumers. No persistent semantic selection in Git/SQLite.
5. Stage bounded parallel capture/provider work outside the command actor; maintain aging, debounce, rescan watermark and shutdown ownership. This packet establishes the loop; WP89/WP90 complete semantic invalidation/publication.

**Legacy disposition and decommission.** Replace unavailable source-wave ports and restart-only refresh wiring. The initial startup path becomes the same loop's genesis. DB26.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp88_behavior`, invoked by `just real-time-cpg-packet-check WP88`: real create/edit/delete/rename/context changes are observed without restarting the daemon; generic and Git inventories converge.

##### Structural

- Proposed test `rt_cpg_wp88_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp88_faults`, invoked by the same packet recipe: drop an event during scan, clear a newer dirty generation, trust a missing Git candidate or suppress a deferred read and tests fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp88_operations`, invoked by the same packet recipe: burst/overflow/rescan, watcher failure and shutdown maintain bounded queues and joined ownership.

Oracle catalog:

Executable oracle: `rt_cpg_wp88_integrity`
Governed criterion: `PC-WP88-INT`

Executable oracle: `rt_cpg_wp88_behavior`
Governed criterion: `PC-WP88-BEH`

Executable oracle: `rt_cpg_wp88_faults`
Governed criterion: `PC-WP88-NEG`

Executable oracle: `rt_cpg_wp88_operations`
Governed criterion: `PC-WP88-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP88`; `just lifecycle-production-vertical-check`; `just git-parity-check`; `just source-capture-race-check`; `just programmatic-runtime-lifecycle-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M21.

**Replan Triggers.** Reopen if supported watch roots cannot be monitored/reconciled under source authority or a platform requires a materially different lifecycle boundary. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep scopes dirty, fail to reconciliation rather than stale success, and restart owned watcher state from authoritative inventory without changing selected facts.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP89 — Compute positive/negative invalidation and complete owner withdrawals

**Outcome.** An edit changes exactly the sound affected owner/family/context closure, with conservative widening where support is incomplete.

**Dependencies.** WP87, WP88. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Narrow reuse requires complete support; every replaced owner withdraws old facts, unknowns and derived dependents, including newly resolvable negative lookups. Advances P9/P10/P28.

**Design and library references.** Review F05/F10, §§3.3–3.4; D-RT01/02; GEN §§66,80–88,94; LIFE §§5–7; LD-01/04/05/06.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/python_derived_analysis.rs src/common_derived_analysis.rs src/programmatic_derived_analysis.rs src/fabric/source_wave_command_effect.rs src/fabric/production_workspace_startup.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'derive_python_invalidation_closure|support|dependency|SourceWave|owner|tombstone|remainder' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/python_derived_analysis.rs`, `src/common_derived_analysis.rs`, `src/programmatic_derived_analysis.rs`, `src/fabric/source_wave_command_effect.rs`, `src/fabric/production_workspace_startup.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Join changed source/context/program inputs to D-RT01 positive and negative support, close reverse dependencies and affected SCCs, and produce typed complete replacement/withdrawal work.
2. Handle missing import becoming available, export/member/impl addition, stub/root/config change, callee deletion, rename and SCC split/merge. Positive graph edges alone are insufficient.
3. Regenerate affected owner partitions using correct provider/application analyses; replace support, provenance, diagnostics, unknowns and coverage alongside facts.
4. Retain unaffected owners only with exact dependency validity; widen to namespace/context when actual checker recheck or hidden dependency scope is unavailable.
5. Drive dirty-registry completion from executed durable outcomes, not scheduled jobs or requested module lists; preserve work arriving during computation.

**Legacy disposition and decommission.** Remove manual impact lists, positive-edge-only reuse and append-only cross-epoch summaries. DB26/DB27.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp89_behavior`, invoked by `just real-time-cpg-packet-check WP89`: create/delete/context/SCC mutations yield independently expected withdrawals and equal clean current facts, including negative-to-positive resolution.

##### Structural

- Proposed test `rt_cpg_wp89_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp89_faults`, invoked by the same packet recipe: preserve a stale dependent, omit empty replacement or reuse an owner with incomplete search support and the oracle rejects. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp89_operations`, invoked by the same packet recipe: high fanout broadens safely under bounded work queues; unrelated sustained edits do not starve stable semantic progress.

Oracle catalog:

Executable oracle: `rt_cpg_wp89_integrity`
Governed criterion: `PC-WP89-INT`

Executable oracle: `rt_cpg_wp89_behavior`
Governed criterion: `PC-WP89-BEH`

Executable oracle: `rt_cpg_wp89_faults`
Governed criterion: `PC-WP89-NEG`

Executable oracle: `rt_cpg_wp89_operations`
Governed criterion: `PC-WP89-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP89`; `just analysis-causal-fault-check`; `just provider-trust-coverage-remainder-check`; `just query-unknown-negative-proof-check`; `just inprocess-provider-lifecycle-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M21.

**Replan Triggers.** Reopen if a selected family cannot expose a sound dependency or conservative context boundary; never narrow invalidation for speed without proof. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Recompute the larger selected context from exact images; keep source/current capability honest while pending semantics are withdrawn.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP90 — Publish atomic syntax-current and semantic-current epochs continuously

**Outcome.** The same daemon publishes fast syntax-current and later semantic-current epochs for real two-language edits, preserving exact old leases and current unknown semantics.

**Dependencies.** WP89, WP92. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** One command actor selects one complete epoch; explicit semantic gaps are admitted states, never missing required proof; late providers cannot publish stale facts. Maintains P11/P34; advances P20/P27.

**Design and library references.** Review F01/F05, §§3.1,3.9; D-RT01/02/05/06; LIFE §§6–8; FAB §11 One mutation path; LD-01/03/04.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/production_workspace_startup.rs src/fabric/source_wave_command_effect.rs src/fabric/relation_publication_command_effect.rs src/fabric/programmatic_command_runtime_factory.rs src/fabric/activation.rs tests/integration/daemon.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'PublishSourceWave|PublishRelation|FreshActivation|Activation|Unavailable|expected_predecessor|writer_generation' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/production_workspace_startup.rs`, `src/fabric/source_wave_command_effect.rs`, `src/fabric/relation_publication_command_effect.rs`, `src/fabric/programmatic_command_runtime_factory.rs`, `src/fabric/activation.rs`, `tests/integration/daemon.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Connect concrete source-wave, relation-publication and activation effects to the same production runtime; reuse WP91/WP92 selective durable authority rather than fresh tables per wave.
2. Evaluate release-defined D-RT02 admission obligations for each state. Withdraw invalidated semantics before syntax-current activation and prove every retained owner; semantic-current requires the complete selected result profile.
3. Recheck exact predecessor, writer/source/context/dependency generation, authorization and component readback immediately before selection. Late/mismatched results never change the selected epoch.
4. Implement lawful stale-result discard/recompute, idempotent command outcomes, fair semantic watermark progress, capability/status observations and cancellation/unknown-outcome reconciliation.
5. Replace the separate-startup causal test with an additional same-PID edit sequence that decodes actual changed records while preserving unrelated facts and leased old epochs.

**Legacy disposition and decommission.** Delete unavailable publication effects, per-startup refresh-only authority and stale-result rebinding routes. DB26/DB27; no parallel writer or mutable-latest fallback.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp90_behavior`, invoked by `just real-time-cpg-packet-check WP90`: same-PID Python/Rust edits yield atomic syntax and semantic successor packages with correct withdrawals, gaps and later resolved facts.

##### Structural

- Proposed test `rt_cpg_wp90_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp90_faults`, invoked by the same packet recipe: activate missing proof, mix old/new providers, accept late work, lose fence or expose a partially committed vector and the oracle fails. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp90_operations`, invoked by the same packet recipe: crash/cancel at stage/commit/select/install boundaries reconciles exact state with admission closed until coherent; old queries keep pins.

Oracle catalog:

Executable oracle: `rt_cpg_wp90_integrity`
Governed criterion: `PC-WP90-INT`

Executable oracle: `rt_cpg_wp90_behavior`
Governed criterion: `PC-WP90-BEH`

Executable oracle: `rt_cpg_wp90_faults`
Governed criterion: `PC-WP90-NEG`

Executable oracle: `rt_cpg_wp90_operations`
Governed criterion: `PC-WP90-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP90`; `just real_source_to_fastmcp_causal_vertical`; `just lifecycle-production-vertical-check`; `just activation-fault-matrix-check`; `just fabric-activation-recovery-check`; `just fabric-epoch-pinning-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M21.

**Replan Triggers.** Reopen if admitted capability states conflict with required v2.3 evidence or publication needs parallel durable authority/irreversible partial semantics. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Before durable selection retain predecessor; after selection close admission and reconstruct only the selected exact vector. Repair forward; no rollback-to-predecessor command.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP91 — Classify durability and advance stable exact relation histories selectively

**Outcome.** Durable tables have stable histories and only soundly changed owner/family outputs advance; transient work does not create unnecessary tables.

**Dependencies.** WP83. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Proof-bearing output has an explicit durability class; unchanged relations reuse exact pins; scoped replacement preserves unrelated facts. Advances P11/P17/P28; maintains P34.

**Design and library references.** Review F09, §3.6; D-RT01/02/06; FAB §§9–11; LD-01/02/03.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/programmatic_relation_delta.rs src/fabric/programmatic_observation_delta.rs src/fabric/delta_write.rs src/fabric/delta_exact.rs src/fabric/programmatic_schema.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'DurabilityClass|ReplaceAll|Advance|Genesis|with_input_plan|replace_where|RequireSessionState' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/programmatic_relation_delta.rs`, `src/fabric/programmatic_observation_delta.rs`, `src/fabric/delta_write.rs`, `src/fabric/delta_exact.rs`, `src/fabric/programmatic_schema.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Make producing operations select DELTA_HISTORY, IMMUTABLE_ARROW_SEGMENT or TRANSIENT_ARROW per FAB §9.4; retain every restart/proof/invalidation/provenance-bearing history required by that clause.
2. Reuse exact predecessor versions for unchanged outputs; preserve stable table roots across epochs and lawful Genesis only for a new history.
3. Add governed typed owner predicate replacement with exact predecessor/session, zero retry, writer generation and transaction identity. Native code owns Delta file/log planning.
4. Probe a schema-bearing zero-row input plan, partitioned/nonpartitioned and shared-file owners, null predicates, constraints, no-match owners and CDF deletes. Never use table.write(vec![]) as the zero-row oracle.
5. Implement the minimum durable replacement-key/tombstone plus native effective-state anti-join/union contract and its immediate consumers in this packet, so empty-owner withdrawal passes even if the optional predicate probe fails. WP92 extends this already-correct boundary to bounded durable segment publication and consolidation. Record fast-path limitations; never silently full-replace every relation.
6. Measure rows/files/bytes/commits actually changed, not only final equality; retain exact single-selector reads, full serving statistics and uncertain-write reconciliation.

**Legacy disposition and decommission.** Replace persist-every-sealed-relation/ReplaceAll and epoch-root Genesis policy. Preserve controlled full replacement for explicit rebuild/migration. DB27.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp91_behavior`, invoked by `just real-time-cpg-packet-check WP91`: changed owner versions advance while unrelated pins/rows remain identical; empty replacement withdraws the owner correctly.

##### Structural

- Proposed test `rt_cpg_wp91_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp91_faults`, invoked by the same packet recipe: omit transaction/fence, select latest, rewrite an unchanged table, drop other owners in shared files or demote proof durability and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp91_operations`, invoked by the same packet recipe: conflicts, unknown commit outcome, schema/null changes and reopen reconstruct exact committed versions without blind retry.

Oracle catalog:

Executable oracle: `rt_cpg_wp91_integrity`
Governed criterion: `PC-WP91-INT`

Executable oracle: `rt_cpg_wp91_behavior`
Governed criterion: `PC-WP91-BEH`

Executable oracle: `rt_cpg_wp91_faults`
Governed criterion: `PC-WP91-NEG`

Executable oracle: `rt_cpg_wp91_operations`
Governed criterion: `PC-WP91-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP91`; `just delta-publication-contract-check`; `just delta-durability-protocol-integrity-check`; `just delta-exact-version-reconstruction-check`; `just data-fabric-core-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M21.

**Replan Triggers.** Reopen on an incompatible native predicate/session/transaction contract; use only the already accepted effective-state fallback, not a custom Delta writer. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Reconcile operation markers and exact versions before retry; partial component commits remain invisible until epoch selection.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP92 — Publish bounded durable native overlays and consolidate by equality

**Outcome.** Small updates can activate exact durable replacement segments without full table rewrites, and bounded consolidation restores a segment-free base with equivalent meaning.

**Dependencies.** WP91. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Zero-row replacement keys are durable authority; base anti-join plus replacement union stays optimizer-visible; consolidation preserves meaning before activation. Advances P14/P15/P17.

**Design and library references.** Review F09/F11, §§3.5–3.6; D-RT05/06/07; FAB §10 Effective state and immutable overlays; LD-01/02/03.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/programmatic_relation_delta.rs src/fabric/programmatic_schema.rs src/fabric/delta_write.rs src/fabric/programmatic_delta_maintenance_command.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'overlay|segment|tombstone|anti_join|consolidat|Compaction|ReplaceAll|effective' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/programmatic_relation_delta.rs`, `src/fabric/programmatic_schema.rs`, `src/fabric/delta_write.rs`, `src/fabric/programmatic_delta_maintenance_command.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Durably stage owned Arrow segments with schema/provenance/source/context/operation identity and exact pins; publish replacement-key/tombstone relations even when data is empty.
2. Install only native owner/family/context anti-joins, unions and visible conflict windows for effective state. No bespoke concatenate/take evaluator or raw Parquet Delta authority.
3. Enforce segment count/bytes, key size, retained generation and scan-amplification limits under shared budgets. Respect FAB-required Delta histories rather than demoting proof-bearing data to segments for speed.
4. Reserve peak rewrite headroom and use controlled zero-retry native writes for consolidation; prove facts, unknowns, coverage, provenance and public query equality before activation.
5. Make CDF semantics explicit: controlled rematerialization is dataChange=true. Consumers replay normally or use an application-proved exact range/rebase receipt; never infer OPTIMIZE neutrality.

**Legacy disposition and decommission.** Remove bespoke overlay semantics and unbounded segment accumulation; retire the full-fabric-replacement hot path once selective output is installed. DB27.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp92_behavior`, invoked by `just real-time-cpg-packet-check WP92`: successive edits, empty owners and deletes produce the same canonical state before/after consolidation and exact restart.

##### Structural

- Proposed test `rt_cpg_wp92_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp92_faults`, invoked by the same packet recipe: omit replacement keys, alter owner dimensions, activate unstaged bytes, demote durable proof or skip CDF without an equality receipt and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp92_operations`, invoked by the same packet recipe: slow old-epoch readers survive consolidation; pressure/backpressure and interrupted staging preserve bounds and exact selected state.

Oracle catalog:

Executable oracle: `rt_cpg_wp92_integrity`
Governed criterion: `PC-WP92-INT`

Executable oracle: `rt_cpg_wp92_behavior`
Governed criterion: `PC-WP92-BEH`

Executable oracle: `rt_cpg_wp92_faults`
Governed criterion: `PC-WP92-NEG`

Executable oracle: `rt_cpg_wp92_operations`
Governed criterion: `PC-WP92-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP92`; `just delta-publication-contract-check`; `just delta-exact-version-reconstruction-check`; `just fabric-epoch-pinning-check`; `just datafusion-plan-schema-cache-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M21.

**Replan Triggers.** Reopen if effective-state semantics require custom relational evaluation or compaction cannot preserve required provenance/unknown/CDF meaning. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Unselected segments remain unreachable and recoverably inventoried; keep old pins until lawful retention release. Never expose a half-rebased epoch.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP93 — Stream bounded CDF intervals with durable downstream reconciliation

**Outcome.** A real installed downstream consumer catches up through bounded, cancellable exact CDF ranges and reconstructs progress after crashes.

**Dependencies.** WP92. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** CDF transports exact changes, not state selection; downstream interval application is durable before checkpoint progress and is recoverable after checkpoint loss. Advances P17/P23/P34.

**Design and library references.** Review F10/F11, §3.6; D-RT01/05/07; FAB §9.4; LD-02/03.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/delta_cdf_replay.rs src/fabric/delta_exact.rs src/fabric/command_record_sqlite.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ExactDeltaCdfDownstream|collect|checkpoint|Cdf|schema|version' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/delta_cdf_replay.rs`, `src/fabric/delta_exact.rs`, `src/fabric/command_record_sqlite.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Replace full-range eager collection with bounded version windows and row/byte/value/time limits, streaming batches with backpressure and cancellation.
2. Install a real downstream consumer, not only test doubles. Select CDF only where a named transport consumer needs it; hot local change sets need not replay their own log redundantly.
3. Bind applied interval identity and semantic effects in durable downstream commit/reconciliation; update SQLite checkpoint afterward. Crash after effects but before SQLite progress must not duplicate or omit work.
4. Handle schema-era boundaries, gaps, expired/unavailable history and no-change ranges explicitly. Fall back only to the selected exact-snapshot rebuild contract, never empty success.
5. Represent root plus inclusive from/through and required baseline reconstruction as a CDF retention obligation; endpoint snapshot pins alone are insufficient.

**Legacy disposition and decommission.** Retire unbounded range collection and checkpoint-only proof of semantic progress. DB27/DB30; no new parallel durable current-state authority.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp93_behavior`, invoked by `just real-time-cpg-packet-check WP93`: bounded multi-version replay yields correct downstream facts and checkpoint-loss recovery yields exactly the selected semantic outcome.

##### Structural

- Proposed test `rt_cpg_wp93_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp93_faults`, invoked by the same packet recipe: advance checkpoint early, omit interval application identity, lose an interior version/schema era or treat expired CDF as empty and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp93_operations`, invoked by the same packet recipe: large/slow/cancelled replay, downstream failure, interrupted effect/checkpoint boundary and exact rebuild honor budgets.

Oracle catalog:

Executable oracle: `rt_cpg_wp93_integrity`
Governed criterion: `PC-WP93-INT`

Executable oracle: `rt_cpg_wp93_behavior`
Governed criterion: `PC-WP93-BEH`

Executable oracle: `rt_cpg_wp93_faults`
Governed criterion: `PC-WP93-NEG`

Executable oracle: `rt_cpg_wp93_operations`
Governed criterion: `PC-WP93-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP93`; `just delta-exact-reconstruction-v3-check`; `just delta-exact-version-reconstruction-check`; `just scheduled-streamed-semantic-query-check`; `just cancellation-tree-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M22.

**Replan Triggers.** Reopen if a downstream effect cannot be reconciled by durable operation/interval identity; do not assert distributed exactly-once delivery from SQLite ordering alone. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Read exact durable interval/effect markers, resume or rebuild from an explicit snapshot, and keep required source history leased until reconciliation completes.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP94 — Certify a narrow native Delta maintenance capability amendment

**Outcome.** An actually reproducible native dependency artifact supports governed obsolete-file reclamation; unsafe current-pin VACUUM/OPTIMIZE is never enabled by assertion.

**Dependencies.** WP93. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Delta owns candidates/log transactions; every native maintenance commit preserves caller operation/txn/zero-retry semantics and approved deletion scope. Maintains P3/P34; advances P20/P25.

**Design and library references.** Review F11; planning contracts D-RT07 and LD-RT08; FAB §§2.1,9.1–9.3; current LD-03 remains authority until a certified versioned amendment.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/delta_guarded_maintenance.rs src/fabric/programmatic_delta_maintenance_command.rs tooling/ci/artifact_contracts.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'OptimizeBuilder|Vacuum|CommitProperties|with_max_retries|application_transaction|43a0cf10|approved' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/delta_guarded_maintenance.rs`, `src/fabric/programmatic_delta_maintenance_command.rs`, `Cargo.toml`, `Cargo.lock`, `scripts/stable_graph_check.sh`, `deny.toml`, `tooling/ci/artifact_contracts.py`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Implement LD-RT08 on the exact native source or select a minimally changed upstream artifact that already meets the same contract. Fix every maintenance START/END/subcommit path to preserve supplied commit properties, application transaction identity and zero retry.
2. Keep candidate construction native. Provide opaque prepared operation or equivalent native approved-set enforcement bound to root/version/cutoff/operation, with exhaustive delete-route mediation and changed-predecessor rejection.
3. Prove retries, duplicate/unknown submissions, interruption after partial approved deletion, concurrent lease acquisition and schema/CDF cases. An ObjectStore allowlist alone cannot fix the current VACUUM START commit violation.
4. Before pin adoption, author a versioned dependency/authority amendment and exact source-artifact identity with reproducible acquisition/build, provenance and dependency diff review. Preserve the Arrow/DataFusion universe, narrow/local feature posture, minimum Rust floor and four first-party domains; never invent the future revision in advance.
5. Update exact pin consumers and compatibility/security/governance checks through their existing owners, preserving immutable v2.3 masters as history through the repository's synchronized authority-chain workflow. No remote publication/upload/push or deployment is authorized by plan creation; obtain any required distribution authority separately.
6. Add proposed real-time-cpg-native-maintenance-check. Native optimize remains unselected unless its entire contract is independently certified; controlled consolidation remains the safe default.

**Legacy disposition and decommission.** Current destructive-operation denial remains until this capability is proved and WP95 supplies complete retention admission. Do not replace it with custom file-action/log planning or broaden retry policy. DB27.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp94_behavior`, invoked by `just real-time-cpg-packet-check WP94`: native candidate preparation/execution preserves every command identity and reclaims only eligible approved objects under exact predecessor binding.

##### Structural

- Proposed test `rt_cpg_wp94_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp94_faults`, invoked by the same packet recipe: inject hidden retry, lost START txn, altered candidate/predecessor, new conflicting lease or an extra delete path and the operation is rejected. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp94_operations`, invoked by the same packet recipe: reproducible dependency builds, full feature/version compatibility, interrupted partial deletion and unknown-outcome reconciliation pass.

Oracle catalog:

Executable oracle: `rt_cpg_wp94_integrity`
Governed criterion: `PC-WP94-INT`

Executable oracle: `rt_cpg_wp94_behavior`
Governed criterion: `PC-WP94-BEH`

Executable oracle: `rt_cpg_wp94_faults`
Governed criterion: `PC-WP94-NEG`

Executable oracle: `rt_cpg_wp94_operations`
Governed criterion: `PC-WP94-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP94`; `just real-time-cpg-native-maintenance-check`; `just data-fabric-upgrade-check`; `just stable-graph-check`; `just features-each`; `just deps-fast`; `just policy`; `just proto-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M22.

**Replan Triggers.** A reproducible approved native artifact, synchronized pin authority and exact commit/deletion contract are mandatory. If unavailable, block this packet and full completion; do not downgrade reclamation to successful denial. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Before selecting any new artifact keep the current certified dependency and deny unsafe maintenance. After lawful pin adoption, repair forward; failed deletion reconciles the approved operation journal, never blind retry or broad filesystem cleanup.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP95 — Enforce lease/CDF-safe finite retention with real reclamation

**Outcome.** Long-running updates, CDF consumers and old readers operate within finite storage limits while eligible unleased objects are actually reclaimed safely.

**Dependencies.** WP94. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Deletion requires complete protected-resource closure and coordinated lease generation; repeated service reclaims eligible obsolete storage rather than merely refusing work. Advances P23/P28/P34.

**Design and library references.** Review F11/F13, §§3.6,3.9; D-RT05/07 and LD-RT08; FAB §9.3; LIFE retention/lease/recovery clauses.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/delta_exact.rs src/fabric/delta_guarded_maintenance.rs src/fabric/programmatic_delta_maintenance_command.rs src/fabric/streamed_result_registry.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'DeltaRetainedResource|keep_versions|Vacuum|retention|lease|Cdf|cleanup_expired' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/delta_exact.rs`, `src/fabric/delta_guarded_maintenance.rs`, `src/fabric/programmatic_delta_maintenance_command.rs`, `src/fabric/streamed_result_registry.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Derive closure over active/candidate/leased epochs, exact Add files, CDF intervals/logs/schema eras/baselines, releases/expectations/provenance, unresolved operations, result/read leases, segments, staging/orphans and spill.
2. Replace century-retention/all-table-CDF/statistics-every-column defaults with explicit measured policies and named consumers; preserve full serving statistics and required immutable evidence.
3. Coordinate lease acquisition/renewal with maintenance protection generation. A newly valid lease cannot race after approval into an object scheduled for deletion.
4. Use WP94's certified native candidate/execution contract, dry-run approval, fixed cutoffs and operation readback. Release only wholly unreferenced application-owned segments through their bounded owner; no raw active-Delta-file cleanup.
5. Bound consumer lag and define exact rebuild/expiry behavior without silently revoking valid leases. Reserve consolidation/reclamation headroom and prove actual reclaimed bytes over repeated edits, reopen and lagging-consumer cycles.

**Legacy disposition and decommission.** Retire endpoint-only CDF retention and preservation-forever defaults after closure/reclamation is proved. Unsafe operation denial remains an error path, not the success implementation. DB27/DB30.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp95_behavior`, invoked by `just real-time-cpg-packet-check WP95`: repeated updates reclaim eligible bytes while protected snapshots, CDF catch-up and provenance remain exact and readable.

##### Structural

- Proposed test `rt_cpg_wp95_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp95_faults`, invoked by the same packet recipe: delete interior CDF/log artifacts, race a new lease, omit uncertainty/staging roots or approve extra native candidates and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp95_operations`, invoked by the same packet recipe: disk pressure, long readers, renewal/expiry, partial deletion, schema-era changes and restart preserve bounds and eventual eligible reclamation.

Oracle catalog:

Executable oracle: `rt_cpg_wp95_integrity`
Governed criterion: `PC-WP95-INT`

Executable oracle: `rt_cpg_wp95_behavior`
Governed criterion: `PC-WP95-BEH`

Executable oracle: `rt_cpg_wp95_faults`
Governed criterion: `PC-WP95-NEG`

Executable oracle: `rt_cpg_wp95_operations`
Governed criterion: `PC-WP95-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP95`; `just real-time-cpg-native-maintenance-check`; `just vacuum-dry-run-check`; `just fabric-epoch-pinning-check`; `just query-retention-cancellation-restart-check`; `just delta-exact-reconstruction-v3-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M22.

**Replan Triggers.** Reopen if retention closure cannot be derived completely, if no eligible resources can be reclaimed in the sustained workload, or if a required external lease policy changes. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Deny deletion whenever closure is uncertain; backpressure safely but mark freshness/reclamation incomplete. Reconcile exact operations and release only proven-unreferenced resources.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP96 — Restore native projection optimization with executable field identity

**Outcome.** Native logical/physical projection optimization safely prunes real nested Delta/view plans without losing field identity or scan/property contracts.

**Dependencies.** WP95. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Names/equal shapes cannot substitute for identity; native planning remains visible and stock projection rules are enabled at completion. Advances P12/P14/P15/P21.

**Design and library references.** Review F08, §3.5; D-RT04 and A04; LD-01/02; FAB SchemaContract; DF reference §40A and S7/S10/S11.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/programmatic_schema.rs src/fabric/child_session.rs src/fabric/delta_exact.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'IdentityPreservingViewTable|SchemaIdentityExec|optimize_projections|ProjectionPushdown|ScanArgs|StatisticsRequest' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/programmatic_schema.rs`, `src/fabric/child_session.rs`, `src/fabric/delta_exact.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Run proposed real-time-cpg-optimizer-identity-check preflight against supported exact-pin provider/view/expression/ExecutionPlan seams before choosing the narrow implementation.
2. Compile expression/source-field to target-field mappings from typed logical authority; preserve legitimate authored identity transitions. Project/reorder the mapping with native scan arguments; reject unrecognized conflicts and ambiguous equal-shaped swaps.
3. Restore missing annotations only at narrow generic authoritative relation/result boundaries before physical optimization. Validate logical/physical/stream/batch/sink phases, ID storage casts and nullability.
4. Forward structured ScanArgs/StatisticsRequest, filters/limits/residuals, ordering/partition/equivalence/statistics/metrics and child replacement behavior. Preserve one deliberate optimizer sequence for nested views.
5. Remove global optimize_projections/ProjectionPushdown exclusions only when stock-rule and current-safe-path results/identities agree and actual scan column pruning occurs. Merely using a metadata-preserving constructor is insufficient.

**Legacy disposition and decommission.** DB28 deletes broad projection exclusions and shape-only identity rebinding; retain only the proved generic metadata seam. No domain-specific optimizer fork.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp96_behavior`, invoked by `just real-time-cpg-packet-check WP96`: aliases, duplicate/reordered columns, filter-only fields, nested views, joins/aggregates/unions/anti-joins, empty results and reopened Delta preserve values/identity with pruning.

##### Structural

- Proposed test `rt_cpg_wp96_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp96_faults`, invoked by the same packet recipe: equal-shaped identity swap, conflicting metadata, dropped residual filter, premature limit or wrong statistics map is rejected. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp96_operations`, invoked by the same packet recipe: optimized planning/execution/reopen remains bounded, cancellable and deterministic across partitions and child reconstruction.

Oracle catalog:

Executable oracle: `rt_cpg_wp96_integrity`
Governed criterion: `PC-WP96-INT`

Executable oracle: `rt_cpg_wp96_behavior`
Governed criterion: `PC-WP96-BEH`

Executable oracle: `rt_cpg_wp96_faults`
Governed criterion: `PC-WP96-NEG`

Executable oracle: `rt_cpg_wp96_operations`
Governed criterion: `PC-WP96-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP96`; `just real-time-cpg-optimizer-identity-check`; `just datafusion-plan-schema-cache-check`; `just datafusion-scan-contract-check`; `just provider-statistics-contract-check`; `just data-fabric-upgrade-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M22.

**Replan Triggers.** Failure of A04 blocks optimized acceptance and reopens the library decision; do not leave global rule exclusions as completed optimized design or flip them on unsafely. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep the existing safe exclusion until replacement proof passes. A later regression fails closed and requires a forward repaired release, not blind metadata relabeling.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP97 — Exploit native DataFusion and Arrow capabilities with truthful cost contracts

**Outcome.** The installed fabric uses native pruning, joins, distribution, streaming and compact Arrow representation where measured useful, preserving exact semantics and resource bounds.

**Dependencies.** WP96, WP87. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Native expressions/plans/scans do the work; statistics, constraints, ordering and extension properties are truthful; physical changes do not alter semantic authority. Advances P14/P15/P21/P24.

**Design and library references.** Review §3.5 capability matrix and F13/F14; D-RT04/05; LD-01/02/03/06; DF §40A and alignment flows §§4–11.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/epoch_runtime.rs src/fabric/datafusion_cache.rs src/fabric/child_session.rs src/fabric/programmatic_schema.rs src/fabric/delta_exact.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'StatisticsRequest|target_partitions|Parquet|Dictionary|View|cache|PlanProperties|GroupsAccumulator|UDF' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/epoch_runtime.rs`, `src/fabric/datafusion_cache.rs`, `src/fabric/child_session.rs`, `src/fabric/programmatic_schema.rs`, `src/fabric/delta_exact.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Use native typed Expr, semi/anti joins, unions/windows/aggregates, set operations and bounded recursion before custom functions/extensions. A custom scalar/UDAF/GroupsAccumulator exists only for missing semantics, with null/coercion/volatility/merge/state/memory proof; HOFs are not a generic rule interpreter.
2. Propagate honest exact/inexact/unknown row/column statistics and proved constraints/functional dependencies. Exercise native join ordering, requirements, dynamic filters, sort/limit/top-K pushdown and residuals through actual Delta scans.
3. Evaluate pinned Parquet row-group/page/bloom/filter pruning, chosen statistics columns, file layout/work stealing, repartition/coalescing and target partitions under one shared CPU/memory budget; availability alone is not activation evidence.
4. Use owned RecordBatch/ArrayRef, vectorized builders/kernels, supported dictionary/view encodings and bounded IPC; account retained buffers/slices and avoid repeated JSON/row conversions. Do not change public schema solely for an encoding optimization.
5. Keep optimized logical reuse scoped by full release/source/context/authority/request semantics; fresh physical plans/results per execution. Query-local inputs retain shared-cache bypass until an explicit template design proves isolation.
6. For a necessary extension, expose relational children/expressions, forward physical planning context and statistics, recompute properties on child replacement, and implement cancellation/reservations/metrics. Physical serialization, Flight/ADBC/SQL/distributed/GPU paths remain non-goals absent measured need.

**Legacy disposition and decommission.** Retire custom equivalents of selected native operations, false property declarations and unbounded materialization. DB28/DB30; no second query/graph cache authority.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp97_behavior`, invoked by `just real-time-cpg-packet-check WP97`: each selected native capability produces equal canonical output and observable useful planning/pruning/representation behavior on the real workload.

##### Structural

- Proposed test `rt_cpg_wp97_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp97_faults`, invoked by the same packet recipe: lie about ordering/uniqueness/pushdown, omit cache authority, share request-local provider state or return an unaccounted extension buffer and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp97_operations`, invoked by the same packet recipe: partition/batch/layout/encoding variants, spilling, slow consumers and cancellation obey shared bounds without claiming unmeasured speedups.

Oracle catalog:

Executable oracle: `rt_cpg_wp97_integrity`
Governed criterion: `PC-WP97-INT`

Executable oracle: `rt_cpg_wp97_behavior`
Governed criterion: `PC-WP97-BEH`

Executable oracle: `rt_cpg_wp97_faults`
Governed criterion: `PC-WP97-NEG`

Executable oracle: `rt_cpg_wp97_operations`
Governed criterion: `PC-WP97-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP97`; `just datafusion-scan-contract-check`; `just provider-statistics-contract-check`; `just datafusion-cache-resource-operations-check`; `just query-determinism-check`; `just graph-query-resource-operations-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M22.

**Replan Triggers.** Reopen a selected capability if the exact provider does not expose it or correctness/resource evidence fails; do not custom-reimplement an engine feature merely to keep a checklist green. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Use the last proved native physical strategy within unchanged logical semantics; invalidate incompatible caches and rebuild fresh plans.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP98 — Compile all eight typed query forms and partial-success composition DAGs

**Outcome.** Every released query form and supported selector/return/dependency operand executes over the actual authorized CPG, rather than only being declared.

**Dependencies.** WP99, WP90. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** One typed compiler governs all forms and operands; authorization precedes resolution/cost/errors; independent branches survive sibling failure. Advances P2/P13/P27.

**Design and library references.** Review F06/F07, §3.7; D-RT01/02/04/05; QRY §§4–7,10; LD-01/07.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/relational_program.rs src/relational_semantic_query.rs src/production_query_recipe.rs src/fabric/programmatic_ingress_port.rs src/fabric/programmatic_query_backend.rs src/fabric/child_session.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'compiled_find_entities_program|query_form|request_inputs|consumer_slots|NOT_EXECUTED_DEPENDENCY|Compiled|Scope' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/relational_program.rs`, `src/relational_semantic_query.rs`, `src/production_query_recipe.rs`, `src/fabric/programmatic_ingress_port.rs`, `src/fabric/programmatic_query_backend.rs`, `src/fabric/child_session.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Implement find code entities; retrieve facts about code; follow code relationships; find connecting fact paths; match a code fact pattern; combine result sets; summarize objective facts; retrieve source and syntax context through one compiled typed relational program.
2. Bind phrase/role/representation resolution, within/where/return, directions/distances/stops, path policy, pattern alternatives/scoped negation, set compatibility and summary precision to live typed release/capability facts. Integrate the already-proved WP99 graph executor; all eight forms are executable before this packet completes.
3. Compile per-block bindings and result/input relations; reject cycles; preserve fan-out reuse, deterministic fan-in, independent success and NOT_EXECUTED_DEPENDENCY for failed dependencies.
4. Authorize before interpretation, statistics, negative proof, artifact/error construction and source disclosure. Recursively validate bound views/providers/functions/extensions/variables/object stores; no public SQL or serialized plan ingress.
5. Make absence proof conditional on complete authorized owner/family/context universe; expose resolved interpretation, support and independent status dimensions. Remove the phrase-only production shortcut and whole-request failure on any uncompiled sibling.

**Legacy disposition and decommission.** DB29 removes eight-form declaration-only coverage, narrow phrase-only executor and whole-request-failure shortcut; keep one compiler and one authority.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp98_behavior`, invoked by `just real-time-cpg-packet-check WP98`: all forms and every semantic operand/return are causally tested on both-language facts, including mixed-success DAGs and indeterminate negation.

##### Structural

- Proposed test `rt_cpg_wp98_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp98_faults`, invoked by the same packet recipe: mutate an ignored operand, remove a form/producer, leak hidden authority, merge incompatible contexts or fail independent branches and reject. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp98_operations`, invoked by the same packet recipe: large bounded DAGs, input fan-out, cancellation, request-local cache isolation and deterministic pagination stay within one request envelope.

Oracle catalog:

Executable oracle: `rt_cpg_wp98_integrity`
Governed criterion: `PC-WP98-INT`

Executable oracle: `rt_cpg_wp98_behavior`
Governed criterion: `PC-WP98-BEH`

Executable oracle: `rt_cpg_wp98_faults`
Governed criterion: `PC-WP98-NEG`

Executable oracle: `rt_cpg_wp98_operations`
Governed criterion: `PC-WP98-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP98`; `just semantic-request-program-check`; `just semantic-request-contract-integrity-check`; `just caller-defined-semantic-authority-denial-check`; `just query-unknown-negative-proof-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M23.

**Replan Triggers.** Reopen if a released form cannot be expressed without changing public semantics or authority; do not silently narrow available operands to the existing vertical. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Seal truthful per-block failures/unknowns while retaining independent results; never substitute stale/syntax/name semantics for unavailable requested meaning.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP99 — Make graph queries demand-rooted, witness-correct and work-bounded

**Outcome.** The production graph executor consumes validated typed graph programs and produces bounded, deterministic relationship/path witnesses without all-pairs or path-prefix explosion; WP98 subsequently connects the complete public form compiler.

**Dependencies.** WP97, WP89. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set. WP89 orders the shared derived-analysis integration surface before graph-query changes.

**Target invariants.** Output limits do not stand in for intermediate-work limits; shortest/all-shortest/bounded-simple policies preserve ordered fact witnesses and isolated nodes. Advances P14/P23/P25.

**Design and library references.** Review F14, §§3.3–3.5; D-RT03/05; QRY §4.3/4.4; GEN §62; LD-01/06.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/graph_program.rs src/production_query_recipe.rs src/fabric/programmatic_query_backend.rs src/common_derived_analysis.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'RecursiveQuery|shortest|frontier|path_policy|next\(\)|Cancellation|limit' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/graph_program.rs`, `src/production_query_recipe.rs`, `src/fabric/programmatic_query_backend.rs`, `src/common_derived_analysis.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Define/validate the immediate typed graph-program contracts and production execution consumer before full form integration. Root recursive native plans in authorized starting nodes; explicitly bound working domain/depth/frontier/cardinality/time/memory before final sort/limit. Tests execute this production component, not a duplicate test-only graph engine.
2. Use distance/predecessor computation with bounded canonical reconstruction for shortest/all-shortest policies; keep simple-path enumeration only under its explicit bound. Preserve parallel fact identity and isolated from/to entities.
3. Honor direction, distance, stop conditions, scoped filters and unknown edges/coverage; do not compute unrestricted all-pairs transitive closure by default.
4. Use native joins/recursion when sufficient and justified bounded petgraph kernels otherwise. Account graph heap/intermediate witness work independently of DataFusion reservations.
5. Poll cancellation around pending asynchronous streams and within owned CPU work; test cancellation after work begins and join all operations.

**Legacy disposition and decommission.** DB29 removes all-source seed and shortest-witness prefix enumeration shortcuts from the public route; DB30 removes unowned synchronous work.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp99_behavior`, invoked by `just real-time-cpg-packet-check WP99`: chains, diamonds, cycles, equal shortest witnesses, unreachable/isolated nodes and parallel edges match independent path expectations.

##### Structural

- Proposed test `rt_cpg_wp99_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp99_faults`, invoked by the same packet recipe: all-pairs seeding for one root, wrong witness tie/order, LIMIT-only bounding or pre-cancel-only handling is detected. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp99_operations`, invoked by the same packet recipe: dense/long/adversarial graphs and cancellation/slow consumption respect intermediate and result budgets with reclaimed resources.

Oracle catalog:

Executable oracle: `rt_cpg_wp99_integrity`
Governed criterion: `PC-WP99-INT`

Executable oracle: `rt_cpg_wp99_behavior`
Governed criterion: `PC-WP99-BEH`

Executable oracle: `rt_cpg_wp99_faults`
Governed criterion: `PC-WP99-NEG`

Executable oracle: `rt_cpg_wp99_operations`
Governed criterion: `PC-WP99-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP99`; `just graph-query-resource-operations-check`; `just analysis-fixed-point-resource-check`; `just semantic-request-program-check`; `just cancellation-tree-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M23.

**Replan Triggers.** Reopen the algorithm/rung if required witness semantics cannot meet bounds; do not change shortest into arbitrary first-path or omit valid equal witnesses silently. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Terminate with explicit limit/incomplete status, retain no partial-complete result, and release/join graph work and leases.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP100 — Deliver one canonical agent response through modern FastMCP

**Outcome.** Agents receive complete, meaningful canonical CPG answers with truthful status/unknown/provenance and working automatic/inline/resource delivery.

**Dependencies.** WP98. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** The daemon owns semantic JSON meaning; Python projects only; inline/resource modes have equal content, coverage/order and independently checked disclosure. Maintains P22/P35; advances P13/P27.

**Design and library references.** Review F07/F13, §3.7; D-RT02/05; QRY §10 Canonical response; SRV §9 One logical response and delivery policy; LD-02/07.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/programmatic_query_backend.rs src/query_service.rs src/fabric/streamed_result_registry.rs codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/wire_models.py codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'canonical-response|delivery|QueryResponse|resolved_semantics|completeness|Resource|read_chunk' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/programmatic_query_backend.rs`, `src/query_service.rs`, `src/fabric/streamed_result_registry.rs`, `codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py`, `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/wire_models.py`, `codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py`, `contracts/rpc/cpg_query_service.proto`, `tooling/proto/README.md`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Construct the full daemon-owned QRY response: resolved semantics, snapshot, entity/fact/path/source dictionaries, per-block results, independent execution/availability/completeness/freshness/limit dimensions, provenance, errors and exact truncation.
2. Honor SRV delivery choice and existing thresholds after sealing; canonical inline and external resource objects are semantically identical. Arrow bulk pages complement, not replace, an agent-readable answer.
3. Wire immediate Rust/Protobuf/Python models and generated outputs through the single generator; preserve released allocations and deliberate versioning, modern four-tool/two-resource protocol, typed preparation/atomic start/guard/completion and per-agent handles.
4. Reauthorize resources/reference/source on every use; preserve byte/base64 lossless source representation and denied-existence behavior.
5. Bound page validation/reuse and adapter chunk/bytearray/base64/JSON expansion under shared policy; amortize whole-page hashing only with immutable leases and per-read authorization.

**Legacy disposition and decommission.** DB29 removes resource-only pseudo-response, ignored delivery and Python semantic duplication; retain transport-only generated types and modern security lifecycle.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp100_behavior`, invoked by `just real-time-cpg-packet-check WP100`: all forms produce equal canonical inline/resource objects including mixed-success, gaps, source bytes, dictionaries and limits through installed FastMCP.

##### Structural

- Proposed test `rt_cpg_wp100_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp100_faults`, invoked by the same packet recipe: ignore delivery, omit unknown/status dimensions, leak source/hidden existence, forge a handle or leave dangling IDs and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp100_operations`, invoked by the same packet recipe: real STDIO/UDS clients, large/slow responses, reconnect/cancel/release, two-agent isolation and expiry clean up without queue or buffer growth.

Oracle catalog:

Executable oracle: `rt_cpg_wp100_integrity`
Governed criterion: `PC-WP100-INT`

Executable oracle: `rt_cpg_wp100_behavior`
Governed criterion: `PC-WP100-BEH`

Executable oracle: `rt_cpg_wp100_faults`
Governed criterion: `PC-WP100-NEG`

Executable oracle: `rt_cpg_wp100_operations`
Governed criterion: `PC-WP100-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP100`; `just fastmcp4-stdio-vertical-check`; `just fastmcp4-resource-authority-check`; `just fastmcp4-atomic-start-check`; `just fastmcp4-completion-authorization-check`; `just fastmcp4-modern-protocol-check`; `just proto-check`; `just adapter-ci-fast`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M23.

**Replan Triggers.** Reopen if complete semantics require a public contract break or a newly exposed tool/resource; no silent wire allocation reuse or Python semantic authority. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep immutable response packages and lawful LOST/expired outcomes; reconnect through daemon-minted authority rather than reconstructing semantics in the adapter.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP101 — Make runtime proof dependency-aware without weakening re-execution

**Outcome.** Each candidate proves schema, ownership, withdrawals, capability, provenance and exact publication without needlessly retaining or recomputing every expanded upstream view.

**Dependencies.** WP100. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Proof executes actual semantics against independent expectations; reusable immutable inputs require full support; runtime candidate proof is distinct from release certification. Advances P19/P25/P28/P30.

**Design and library references.** Review F12/F13, §3.9; D-RT01/02/03/05; doctrine P17/P18/P19/P25/P30; LD-01/02/03.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/programmatic_schema.rs src/fabric/derived_producer_closure.rs src/fabric/programmatic_epoch.rs src/fabric/proof/delta_history.rs src/programmatic_derived_analysis.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'TransformationDeterminismPolicy|prove|reexecute|collect|fixed_point|expectation|closure' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/programmatic_schema.rs`, `src/fabric/derived_producer_closure.rs`, `src/fabric/programmatic_epoch.rs`, `src/fabric/proof/delta_history.rs`, `src/programmatic_derived_analysis.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Schedule proof over the executed dependency DAG; share immutable execution inputs within a command only with complete identity/support and invalidation, never across incompatible query authority.
2. Preserve required independent re-execution and expectation checks; replace full-output Vec retention where possible with bounded relational differences or spillable canonical comparison including multiplicity, nulls and identity.
3. Make source/context/owner/endpoints, unknown/coverage, cross-owner invalidation, policy/resource and exact readback violations visible as relations that block activation when required.
4. Separate runtime incremental proof from full release corpus, mutation, recovery and performance campaigns; do not run the entire release gate per save or weaken P19 to checksum comparison.
5. Prove proof-strength parity: seeded semantic/control/authority faults rejected before still reject after scheduling/materialization changes.

**Legacy disposition and decommission.** DB30 removes unbounded proof materialization, repeated accidental dependency execution and digest-as-correctness success paths; retain independent semantic expectations.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp101_behavior`, invoked by `just real-time-cpg-packet-check WP101`: incremental proof and cold proof agree on valid candidates and reject independently wrong semantic results across both language profiles.

##### Structural

- Proposed test `rt_cpg_wp101_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp101_faults`, invoked by the same packet recipe: share stale proof, generate own expected facts, erase multiplicity/unknown differences or accept matching digests without execution and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp101_operations`, invoked by the same packet recipe: large dependency DAGs, shared upstream views, cancellation and spill pressure remain within aggregate budgets without incomplete proof activation.

Oracle catalog:

Executable oracle: `rt_cpg_wp101_integrity`
Governed criterion: `PC-WP101-INT`

Executable oracle: `rt_cpg_wp101_behavior`
Governed criterion: `PC-WP101-BEH`

Executable oracle: `rt_cpg_wp101_faults`
Governed criterion: `PC-WP101-NEG`

Executable oracle: `rt_cpg_wp101_operations`
Governed criterion: `PC-WP101-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP101`; `just analysis-causal-fault-check`; `just datafusion-plan-schema-cache-check`; `just query-artifact-single-execution-check`; `just fabric-activation-recovery-check`; `just root-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M23.

**Replan Triggers.** Reopen if a proof optimization cannot preserve its falsifying tests or requires hidden cross-request result authority. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Use the previous correct bounded proof schedule; an unproved candidate never becomes selected even when ordinary queries would succeed.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP102 — Prove the complete installed real-time two-language product

**Outcome.** One real daemon continuously serves the complete selected Python/Rust CPG and all query forms through the installed FastMCP adapter.

**Dependencies.** WP101, WP95. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Capability completeness follows actual installed full-profile execution and independent expected facts, not presence of modules/recipes or permanent implementation gaps. Advances P20/P25/P27/P30.

**Design and library references.** Review F01/F04/F06/F12 and §6.1–6.2; all D-RT contracts; GEN §§93–104 and release conformance; QRY/SRV/LIFE current suite.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline tests/integration/daemon.rs src/fabric/production_workspace_startup.rs src/production_provider_recipe.rs src/production_query_recipe.rs codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'real_source_to_fastmcp|installed|same_daemon|FreshActivation|fixture|profile|conformance' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `tests/integration/daemon.rs`, `src/fabric/production_workspace_startup.rs`, `src/production_provider_recipe.rs`, `src/production_query_recipe.rs`, `codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Run preregistered small/medium/mixed-language workspaces through built provider binaries, actual native fabric, exact durable storage, supervisor/UDS and installed wheel/STDIO; no test-only semantic production ports.
2. Exercise every selected GEN/ONT family including lettered relationship sections and each QRY form/operand/return. Discover expectations from the immutable release and fail any required family lacking independent production-path evidence.
3. Mutate edit/add/delete/rename, last-module deletion/recreation, broken syntax, compile failure, missing stubs, new imports/impls, changed context/features/cfg, callee removal and SCC split/merge without changing daemon PID.
4. Observe fast syntax-current withdrawal followed by correct semantic-current state, unrelated owner/pin reuse, fairness under continued edits and clean equality after a bounded quiet window.
5. Verify canonical inline/resource content, provenance/unknowns, authorized source and old-epoch leases; compare both incremental and clean output to independent expected facts.

**Legacy disposition and decommission.** Test-only/negative-only/lane-handshake/separate-startup demonstrations cannot remain terminal production proof. DB24–DB30 are exercised target-positively here.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp102_behavior`, invoked by `just real-time-cpg-packet-check WP102`: installed same-PID two-language mutations change exactly the expected canonical facts and complete every required family/form with truthful uncertainty.

##### Structural

- Proposed test `rt_cpg_wp102_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp102_faults`, invoked by the same packet recipe: remove a real provider/producer/form/watcher/commit effect, inject stale context or ignored operand, or replace expected semantics with self-goldens and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp102_operations`, invoked by the same packet recipe: sustained edits, real containment, CDF consumers, consolidation/reclamation, multiple agents and full restart converge with exact durable state.

Oracle catalog:

Executable oracle: `rt_cpg_wp102_integrity`
Governed criterion: `PC-WP102-INT`

Executable oracle: `rt_cpg_wp102_behavior`
Governed criterion: `PC-WP102-BEH`

Executable oracle: `rt_cpg_wp102_faults`
Governed criterion: `PC-WP102-NEG`

Executable oracle: `rt_cpg_wp102_operations`
Governed criterion: `PC-WP102-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP102`; `just real-time-cpg-installed-conformance-check`; `just real_source_to_fastmcp_causal_vertical`; `just semantic-release-vertical-check`; `just fastmcp4-stdio-vertical-check`; `just lifecycle-production-vertical-check`; `just semantic-release-restart-reconstruction-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M24.

**Replan Triggers.** Reopen scope if any required selected family is impossible with the accepted provider/analysis contract; explicit runtime uncertainty is allowed, unimplemented functionality is not. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Report the specific missing behavior and keep its packet incomplete. Never relax the selected profile or substitute fixture ports to achieve terminal green.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP103 — Close cross-domain cancellation, crash, leases and recovery operations

**Outcome.** The complete installed topology remains safe and bounded through active cancellation, process loss, uncertain commits, renewal races and restart.

**Dependencies.** WP102. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Every operation has one joined terminal path; exact immutable selection survives faults; resource/grant authority is renewed on use. Maintains P11/P13/P23/P34.

**Design and library references.** Review F05/F10/F11/F13/F14, §§3.7–3.9; all D-RT contracts; retained v7 lifecycle/wire/security obligations; LD-03/04/07/RT08.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/cancellation.rs src/query_service.rs src/supervisor.rs src/fabric/query_coordinator.rs src/fabric/command_runtime_manager.rs src/fabric/activation.rs src/fabric/streamed_result_registry.rs tests/integration/daemon.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'cancel|deadline|Join|Unknown|reconcile|lease|restart|shutdown|slow_consumer' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/cancellation.rs`, `src/query_service.rs`, `src/supervisor.rs`, `src/fabric/query_coordinator.rs`, `src/fabric/command_runtime_manager.rs`, `src/fabric/activation.rs`, `src/fabric/streamed_result_registry.rs`, `tests/integration/daemon.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Cancel real active Tree-sitter/Ruff/Pyrefly/rustc, DataFusion, graph, proof, publication and resource work, not just pre-cancelled test futures. Join owned CPU/process/task work and prove reservation/lease release.
2. Inject crashes before/after each durable component commit, activation selection/install, CDF effect/checkpoint and approved native partial deletion. Reconstruct from exact durable authority with admission closed until coherent.
3. Exercise slow/unread data/event/resource streams while reserved control/status/cancel/release stays responsive; reconnect/reissue/LOST behavior must preserve modern protocol and per-agent ownership.
4. Test retained epochs, lease renewals/expiry, references, source grants, capability revocation, supervisor replacement and generation fences under concurrent updates and maintenance.
5. Preserve target-only FreshActivation, no bootstrap/latest/receipt/hash authority, no host trust fallback and no rollback/dual serving authority.

**Legacy disposition and decommission.** DB30 removes remaining detached task, drop-without-join, alternate cancellation/publication and unowned resource routes; DB24 retains all earlier decommission constraints.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp103_behavior`, invoked by `just real-time-cpg-packet-check WP103`: every injected boundary fault produces the specified exact terminal/recovery state with protected facts/resources unchanged.

##### Structural

- Proposed test `rt_cpg_wp103_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp103_faults`, invoked by the same packet recipe: detach work, admit early after selection, reuse expired/cross-agent authority or retry an unknown irreversible effect blindly and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp103_operations`, invoked by the same packet recipe: full topology restart, slow consumers, disk pressure, cancellation storms and concurrent lease/maintenance races remain bounded and reclaim resources.

Oracle catalog:

Executable oracle: `rt_cpg_wp103_integrity`
Governed criterion: `PC-WP103-INT`

Executable oracle: `rt_cpg_wp103_behavior`
Governed criterion: `PC-WP103-BEH`

Executable oracle: `rt_cpg_wp103_faults`
Governed criterion: `PC-WP103-NEG`

Executable oracle: `rt_cpg_wp103_operations`
Governed criterion: `PC-WP103-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP103`; `just installed_vertical_fault_and_recovery_matrix`; `just cancellation-tree-check`; `just grpc-flow-control-contract-check`; `just grpc-slow-consumer-check`; `just fabric-activation-recovery-check`; `just fastmcp4-cancellation-recovery-check`; `just supervisor-restart-join-operations-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M24.

**Replan Triggers.** Reopen ownership or recovery if an irreversible effect cannot be reconciled, a valid lease can race deletion or a provider cannot be joined safely. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Stop admission, cancel/drain/join, reconcile selected exact durable outcomes and repair forward. Never broadly clean targets, user sources or live storage.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP104 — Preregister and meet end-to-end real-time resource/performance targets

**Outcome.** The installed product meets preregistered latency, scaling, capacity and cancellation targets with reproducible raw measurements at the actual final topology.

**Dependencies.** WP105. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Measured current-candidate behavior proves performance; scope, corpus, limits and thresholds are fixed before capture; refused updates do not count as convergence. Advances P20/P24/P25/P30.

**Design and library references.** Review F09/F13 and §§2.3,6.3; D-RT05/07, A02/A03/A04; all relevant LD decisions including LD-RT08.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/fabric/epoch_runtime.rs src/fabric/datafusion_cache.rs tooling/ci/fastmcp4_release_performance.py tooling/ci/v7_certification.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'MEASURED_IMPLEMENTATION_DRIFT|measurement|performance|target_partitions|batch_size|expectation|capture' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/fabric/epoch_runtime.rs`, `src/fabric/datafusion_cache.rs`, `tooling/ci/fastmcp4_release_performance.py`, `tooling/ci/v7_certification.py`, `.github/workflows/ci.yml`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Choose machine/corpus/cache/edit classes and finite runtime policy before measurement. Register independent success thresholds, including actual fresh-update completion and eligible storage reclamation; review §2.3 numbers are initial goals, not automatic waivers or measured results.
2. Measure event age/backlog, actual provider work, affected closure, planning/proof/activation, first useful canonical response, scans/pruning/intermediate cardinality/spill, RSS/native/graph/IPC/result allocations, retained bytes and cancellation-to-join.
3. Run controlled full-replacement versus owner/segment comparisons; restored projection versus safe predecessor behavior; batch/partition/layout/statistics/encoding and dependency-aware proof experiments. Optimize measured bottlenecks without changing semantics.
4. Use small/medium/large Python/Rust/mixed workspaces, growing unrelated corpus, high fanout/dense graphs, real compiler costs, repeated edits, lagging CDF and long readers. Separate compiler latency from post-provider fabric overhead.
5. Create proposed confirm-gated real-time-cpg-performance-capture and nonmutating real-time-cpg-performance-check, bound to exact method/candidate/source identities. Preserve old WP65 measurements as history; reject drift and seeded threshold/method tampering. Freeze after WP105's production cleanup. Any tuning/refactor during this packet re-runs owning packet, installed/recovery and quality gates, then freezes a new candidate before final capture.

**Legacy disposition and decommission.** Retire v7-specific measurement selection as successor gate authority without rewriting its raw history. DB30; no no-op packet-count or stale proving-commit performance claim.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp104_behavior`, invoked by `just real-time-cpg-packet-check WP104`: preregistered workloads meet semantic success and latency/scaling envelopes at one identified installed candidate.

##### Structural

- Proposed test `rt_cpg_wp104_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp104_faults`, invoked by the same packet recipe: restamp old samples, change thresholds after capture, omit slow cases, misclassify backpressure as success or detach memory from the measured topology and fail. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp104_operations`, invoked by the same packet recipe: repeat warm/cold samples and sustained-update/reclamation campaigns with bounded peak resources, declared variance and reproducible environments.

Oracle catalog:

Executable oracle: `rt_cpg_wp104_integrity`
Governed criterion: `PC-WP104-INT`

Executable oracle: `rt_cpg_wp104_behavior`
Governed criterion: `PC-WP104-BEH`

Executable oracle: `rt_cpg_wp104_faults`
Governed criterion: `PC-WP104-NEG`

Executable oracle: `rt_cpg_wp104_operations`
Governed criterion: `PC-WP104-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP104`; `just real-time-cpg-performance-check`; `just semantic-profile-bench`; `just real-time-cpg-installed-conformance-check`; `just real-time-cpg-native-maintenance-check`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M24.

**Replan Triggers.** Missed targets require optimization or an explicit accepted target/design revision. Do not quietly omit compiler, proof, transport, retained epochs or reclamation costs. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep prior valid behavior and raw evidence; a failed experiment does not change default knobs, pins or acceptance thresholds automatically.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP105 — Finish target-only decommission and close baseline quality regressions

**Outcome.** Every replaced shortcut/authority is physically retired and the full target passes repository quality gates without carrying the current Clippy failure forward.

**Dependencies.** WP103. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Only the target production authority remains; historical artifacts are preserved as history; all retained domains and required baseline quality gates are green. Advances P31/P36.

**Design and library references.** Review §5.2–5.3, F12; retained v7 DB19–DB23; new DB24–DB30; doctrine P3/P31/P35/P36.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline src/lib.rs src/fabric/production_workspace_startup.rs src/programmatic_derived_analysis.rs src/production_query_recipe.rs src/fabric/programmatic_schema.rs tooling/ci/plan_assurance.py tooling/ci/v7_certification.py --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'bootstrap|predecessor|compatibility|optimize_projections|ProjectionPushdown|RequiredInputAbsent|v7_certification|WP65' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `src/lib.rs`, `src/fabric/production_workspace_startup.rs`, `src/programmatic_derived_analysis.rs`, `src/production_query_recipe.rs`, `src/fabric/programmatic_schema.rs`, `tooling/ci/plan_assurance.py`, `tooling/ci/v7_certification.py`, `.github/workflows/ci.yml`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Re-run the source/structural/build and installed-semantic portions of DB24–DB30 over the current merged tree. Replacements already remove obsolete paths in their own cutovers; this packet closes residue before final measurement. Measurement-dependent final exits run after WP104 in WP106 and cannot force a pre-measurement completion cycle.
2. Derive current exports, constructors, trait implementations, registrations, feature/wire/fixture/entrypoint/recipe/CI consumers and cross-language strings. Preserve historical docs and legitimate gap/failure code; never blindly delete by token match.
3. Close all remaining baseline and introduced formatting/lint/type/test/governance failures, including the observed root-clippy diagnostics, through scoped coherent fixes and assertion-preserving refactors. No blanket lint suppression or gate weakening.
4. Remove duplicate current authority, inactive old execution selectors and stale terminal/performance dispatch from active CI. Retain historical diagnostics only where explicitly nonauthoritative; no old product route can be selected.
5. Rebuild/install all retained roots, generated families, wheel/STDIO surface and feature profiles after purge; re-run meaningful installed semantics and source-to-query tests. Finish the terminal dispatcher and its orchestration tests before the WP104 candidate freeze; WP106 executes them without changing the measured candidate.

**Legacy disposition and decommission.** This packet owns production residue removal and pre-measurement DB24–DB30 closure; WP106 verifies measurement-dependent final exits after WP104. Prevent reintroduction with tested structural rules plus positive runtime evidence. Existing unrelated dirty edits remain user-owned.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp105_behavior`, invoked by `just real-time-cpg-packet-check WP105`: installed target still executes every required capability after obsolete route removal and quality repairs.

##### Structural

- Proposed test `rt_cpg_wp105_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp105_faults`, invoked by the same packet recipe: reintroduce an old constructor, false-complete path, alias, feature edge, writer, selector or Python semantic owner and the gate fails. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp105_operations`, invoked by the same packet recipe: all retained binaries, features, generators, wheel entrypoints, supervisor and cold restart operate after decommission.

Oracle catalog:

Executable oracle: `rt_cpg_wp105_integrity`
Governed criterion: `PC-WP105-INT`

Executable oracle: `rt_cpg_wp105_behavior`
Governed criterion: `PC-WP105-BEH`

Executable oracle: `rt_cpg_wp105_faults`
Governed criterion: `PC-WP105-NEG`

Executable oracle: `rt_cpg_wp105_operations`
Governed criterion: `PC-WP105-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP105`; `just compiled-release-legacy-zero-state-check`; `just remaining-legacy-zero-state-check`; `just provider-type-boundary-check`; `just fastmcp4-adapter-authority-zero-state-check`; `just governance`; `just fastmcp4-package-build-check`; `just ci-fast`; `just ci-pr`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M24.

**Replan Triggers.** Reopen if removal requires real external compatibility/migration not previously known, or a quality fix changes semantic/public behavior; do not preserve a silent fallback. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Restore only the packet's coherent target code if needed before publication; never revive predecessor runtime authority or erase unrelated user work.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

### WP106 — Certify the full realized target at one current candidate

**Outcome.** One clean, identified target candidate passes every packet oracle, milestone, decommission batch and final gate, with an independent implementation review and truthful remaining language uncertainty.

**Dependencies.** WP104. All immediate consumers of a changed contract belong to this packet's actual preflight-derived write set.

**Target invariants.** Completion requires current executable evidence for all obligations, sustainable eligible reclamation and independent review; no partial scope or stale evidence is terminal. Advances P20/P25/P30/P36.

**Design and library references.** Review F12 and §6; accepted planning contracts and every required packet/milestone/decommission; v2.3 suite plus the certified LD-RT08 pin amendment.

**Change surface / Preflight / Known Touch.**

#### Preflight Query

```bash
ast-grep outline tooling/ci/v7_certification.py tooling/ci/plan_assurance.py tooling/ci/artifact_contracts.py tests/integration/daemon.rs --items exports --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'certification|oracle|declared_inputs|candidate|report|exit|active_plan' src tests rustc-extractor/src pyrefly-sidecar/src codefabric-cpg-mcp/src codefabric-cpg-mcp/tests contracts rules rule-tests tooling/ci justfile Cargo.toml
```

Expand discovered constructors, trait/Protocol implementations, serialized fields, generated exports and fixtures with the shared structural-inquiry rules before editing. Negative claims require both structural/text coverage and a relevant green build; these discovery commands alone are not zero-state proof.

#### Known Touch (verified this session)

Known touch: `tooling/ci/v7_certification.py`, `tooling/ci/plan_assurance.py`, `tooling/ci/artifact_contracts.py`, `.github/workflows/ci.yml`, `tests/integration/daemon.rs`. These are baseline evidence, not a frozen must-touch manifest. Shared test/recipe/rule registration follows the lead-owned serialized integration law in §3.

**Required changes.**

1. Execute and verify the real-time-cpg-certification command introduced by WP77 and finalized by WP105, derived from this plan's oracle/milestone/final obligations and the current accepted pin amendment, not copied v7 packet ranges or prior statuses.
2. Derive an acyclic child-gate graph, excluding the terminal aggregator itself from its children while retaining substantive WP106 orchestration tests. Seed recursive recipe graphs and require rejection; never skip actual semantic/domain obligations to avoid recursion.
3. Reject uncommitted tracked candidate changes, missing required tests, zero selectors, skipped domains, stale method/samples, incomplete reclamation, child failure suppression and unreconciled authority/input drift.
4. Execute all four domains, exact feature/dependency/wire/governance/security/recovery/installed semantics and preregistered resource/performance evidence at the same candidate. Mutating capture is separate and deliberate. WP106 is nonmutating verification at the WP104 captured candidate. Any implementation, test or tooling repair that changes its required identity reopens affected packets and WP104 for revalidation, refreeze and recapture before certification resumes.
5. Commission independent implementation-review against accepted design, full plan, actual behavior and evidence. Resolve blocking/major findings and re-run affected plus final gates; a reviewer assertion without executable evidence is insufficient.
6. Record schema-2 execution judgments/proving commits only during authorized execution; derive current trust and results. Completion reports distinguish inherent language uncertainty from unfinished implementation or unavailable required deployment capability.

**Legacy disposition and decommission.** No old target, active plan, stale performance method, registry or proving-commit label may substitute for current full-target certification. Historical plans/reports remain immutable evidence.

**Acceptance checks.**

##### Behavioral

- Proposed test `rt_cpg_wp106_behavior`, invoked by `just real-time-cpg-packet-check WP106`: every required packet/family/form/transition/reclamation scenario passes its independent installed oracle at the final candidate.

##### Structural

- Proposed test `rt_cpg_wp106_integrity`, invoked by the same packet recipe: constructed contracts, immediate consumers and the stated target boundary are valid; pair structural rules with build/runtime evidence, not a source-name census.

##### Negative / Zero-State

- Proposed test `rt_cpg_wp106_faults`, invoked by the same packet recipe: remove any required child oracle/domain, seed failure, stale evidence, a dirty candidate or unfinished dependency artifact and certification fails. Applicable DB checks also prove old-route absence and target-positive behavior.

##### Operational

- Proposed test `rt_cpg_wp106_operations`, invoked by the same packet recipe: fresh install/genesis, sustained edits, cancellation, maintenance and cold recovery pass within the certified operational envelope.

Oracle catalog:

Executable oracle: `rt_cpg_wp106_integrity`
Governed criterion: `PC-WP106-INT`

Executable oracle: `rt_cpg_wp106_behavior`
Governed criterion: `PC-WP106-BEH`

Executable oracle: `rt_cpg_wp106_faults`
Governed criterion: `PC-WP106-NEG`

Executable oracle: `rt_cpg_wp106_operations`
Governed criterion: `PC-WP106-OPS`

**Edit-Local Gates.** Use the affected existing domain's `just root-fmt` / `just root-check-fast`, `just sidecar-fmt` / `just sidecar-check`, `just extractor-fmt` / `just extractor-check`, or `just adapter-lint` / `just adapter-type`. Run the named affected unit tests through their recipe owner; no all-domain gate for every micro-edit.

**Packet-Local Gates.** `just real-time-cpg-packet-check WP106`; `just real-time-cpg-certification`; `just real-time-cpg-installed-conformance-check`; `just real-time-cpg-performance-check`; `just real-time-cpg-native-maintenance-check`; `just ci-fast`; `just ci-pr`. Proposed recipes are identified in §7; they must exist and select substantive cases before completion.

**Integration Milestone.** M25.

**Replan Triggers.** Any failed mandatory gate, missing native artifact, unmet performance target or unresolved independent major finding prevents full completion; revise the plan/design explicitly if the target must change. Also apply §9's current-tree, dependency-closure, security and operational triggers.

**Rollback or Recovery.** Keep the last proved selected state and report exact unresolved obligations; never manufacture terminal green by changing expected results or omitting gates.

**Design-Bearing Contracts and Exemplars (conditional).** The referenced D-RT/LD and normative contracts are the design-bearing exemplars. Helper names, local decomposition and ordinary implementation control flow are left to execution; do not replace these contracts with an illustrative patch.

## 5. Integration milestones

Milestone checks are **proposed**, introduced by WP77 as `just real-time-cpg-milestone-check <M>`. They derive the member packet oracles and named integration gates, fail missing/zero selections, and preserve child failures. Milestone definitions are integration decisions, not stored status. Completion requires current evidence, not merely all members having once passed.

### M18 — Truthful contracts and aggregate ownership

Members: WP77, WP78, WP79. The release, source/context/support and resource/task contracts work together without outward feature/type authority. Execute all member packet oracles plus `just feature-architecture-check`, `just provider-job-contract-check`, `just release-program-contract-check`, `just cancellation-tree-check`, `just stable-graph-check`. Invocation: `just real-time-cpg-milestone-check M18`; additionally run `just native-dependency-artifacts-check` and `just real-time-cpg-native-resource-check`.

### M19 — Actual two-language provider-to-fabric genesis

Members: WP80, WP81, WP82, WP83; prerequisite M18. Real contained/provider-native execution reaches canonical facts through the installed daemon, including failed providers and empty/delete-last inventories. Execute member oracles plus `just inprocess-provider-lifecycle-check`, `just pyrefly-incremental-lifecycle-check`, `just rustc-provider-lifecycle-check`, `just semantic-sandbox-host-matrix-check`, `just programmatic-production-composition-check`, `just extractor-ci-fast`, `just sidecar-ci-fast`. Invocation: `just real-time-cpg-milestone-check M19`.

### M20 — Correct selected local and common semantic analyses

Members: WP84, WP85, WP86, WP87; prerequisite M19. Independent expectations cover the full selected Python/Rust/graph families and precision, including branches, cleanup, unknown effects and graph deletion. Execute member oracles plus `just analysis-producer-semantic-check`, `just analysis-causal-fault-check`, `just analysis-fixed-point-resource-check`, `just wave8-integration-check`. Invocation: `just real-time-cpg-milestone-check M20`.

### M21 — Same-process updates through selective exact publication

Members: WP88, WP89, WP90, WP91, WP92; prerequisites M19 and M20. Sound positive/negative invalidation, durable replacements and atomic syntax/semantic lanes converge without process restart. Execute member oracles plus `just real_source_to_fastmcp_causal_vertical`, `just lifecycle-production-vertical-check`, `just delta-publication-contract-check`, `just fabric-activation-recovery-check`, `just fabric-epoch-pinning-check`, `just git-parity-check`. Invocation: `just real-time-cpg-milestone-check M21`.

### M22 — Native optimized execution and genuinely finite operation

Members: WP93, WP94, WP95, WP96, WP97; dependencies follow the packet graph, including M20. Exact bounded CDF, certified native maintenance, actual eligible reclamation, stock projection optimization and native capabilities work together. Execute member oracles plus `just real-time-cpg-native-maintenance-check`, `just real-time-cpg-optimizer-identity-check`, `just data-fabric-upgrade-check`, `just provider-statistics-contract-check`, `just delta-exact-reconstruction-v3-check`, `just vacuum-dry-run-check`, `just features-each`, `just policy`. Invocation: `just real-time-cpg-milestone-check M22`. Denied unsafe deletion is a passing negative case, never the positive reclamation oracle.

### M23 — Complete authorized agent semantics and bounded proof

Members: WP98, WP99, WP100, WP101; prerequisites M21 and M22. All eight forms, graph witnesses, mixed-success DAGs, canonical inline/resource delivery and dependency-aware proof preserve meaning and authority. Execute member oracles plus `just semantic-request-program-check`, `just graph-query-resource-operations-check`, `just fastmcp4-stdio-vertical-check`, `just fastmcp4-resource-authority-check`, `just query-unknown-negative-proof-check`, `just proto-check`, `just adapter-ci-fast`. Invocation: `just real-time-cpg-milestone-check M23`.

### M24 — Installed conformance, failure recovery and measured real time

Members: WP102, WP103, WP105, WP104; prerequisite M23. Complete installed semantics and recovery, remove production residue and finish terminal tooling, then freeze and capture the final candidate. The installed topology proves full profiles, same-daemon edits, actual reclamation, joined cancellation, exact restart and preregistered performance. Execute member oracles plus `just real-time-cpg-installed-conformance-check`, `just installed_vertical_fault_and_recovery_matrix`, `just slow_consumer_cancel_restart_operations`, `just semantic-release-restart-reconstruction-check`, `just real-time-cpg-performance-check`. Capture is separate, deliberate and confirm-gated. Invocation: `just real-time-cpg-milestone-check M24`.

### M25 — Target-only final closure

Member: WP106; prerequisite M24. Nonmutating revalidation of WP105 and every DB24–DB30 exit includes WP104's current-candidate measurements. Execute member oracles, the decommission checks, all §7 leaf gates and independent implementation-review closure. Invocation: `just real-time-cpg-milestone-check M25`. The terminal `just real-time-cpg-certification` orchestrates this completed graph; it must not recursively invoke itself through a milestone or packet-local declaration.

## 6. Cross-packet decommission batches

The source review's generated 40-file Rust inventory and wire/presentation/tooling inventory remain the starting disposition authority. Each material path appears in the relevant packet's verified surface or inherited preservation contract. A fresh export/constructor/impl/registration/text inventory is required before deletion; the inventory is not a claim of repository-wide dead-code proof.

Each batch has a proposed `just real-time-cpg-decommission-check <DB>` introduced by WP77 and completed by its owners. It runs target-positive behavior, tested structural rules, textual/cross-language residue checks over the declared live universe and relevant builds. Rule IDs and exact symbol/file deletions are derived at execution, not guessed from this plan. Historical plans/specs/reports, legitimate failure/gap code and diagnostic strings are not blindly removed. A zero text match without structural coverage and compilation cannot close a batch.

### DB24 — Retained compiled boundary and predecessor authority

Prerequisites: WP77, WP79, WP83, WP105. Preserve useful contract/release/fabric/state/transport boundaries and old physical deletions; eliminate any remaining marker-only authority, global release lookup, provider/native/generated type escape, reverse feature edge, bootstrap/model/latest/receipt/hash selector or predecessor serving constructor. Deletion is safe only after target provider/fabric/service consumers execute without it.

Exit: `just real-time-cpg-decommission-check DB24`, `just compiled-release-legacy-zero-state-check`, `just provider-type-boundary-check`, `just feature-architecture-check`, `just installed_target_authority_integrity`. Retained v7 DB19–DB23 semantic obligations are re-executed, not copied as completed.

### DB25 — False control-flow and duplicate analysis authority

Prerequisites: WP84, WP85, WP86, WP87, WP102, WP105. Remove AST-adjacency-as-complete-CFG, file-owned callable CFG, unjustified Exact/complete defaults and duplicate/unbound analysis routes/registrations. Preserve raw visitation/evaluation observations as such and useful kernels only behind the accepted derived-analysis boundary.

Exit: `just real-time-cpg-decommission-check DB25`, `just analysis-producer-semantic-check`, `just analysis-causal-fault-check`, `just analysis-fixed-point-resource-check`. Seed reintroduction of the sequential complete projection; independent branch/loop/cleanup tests must fail.

### DB26 — Restart-only/provider-gap/context shortcuts

Prerequisites: WP78, WP80, WP81, WP82, WP83, WP88, WP89, WP90, WP102, WP105. Retire Python-only production selection, unconditional external-lane gaps, synthesized ineffective context, default-config sidecar construction, dirty-subset-as-inventory/empty-inventory rejection, constant capture fences, unavailable source-wave/publication ports, restart-only refresh and stale result rebinding. Genuine missing-input/trust/uncertainty gaps remain required.

Exit: `just real-time-cpg-decommission-check DB26`, `just real_source_to_fastmcp_causal_vertical`, `just pyrefly-incremental-lifecycle-check`, `just rustc-provider-lifecycle-check`, `just lifecycle-production-vertical-check`. The same-PID positive path must fail if any old shortcut is reselected.

### DB27 — Full-replacement and incomplete retention/CDF authority

Prerequisites: WP91, WP92, WP93, WP94, WP95, WP102, WP105. Retire epoch-root genesis for ordinary updates, unconditional full-replacement persistence, unclassified proof durability, bespoke overlay evaluation, unbounded CDF collection, checkpoint-only completion, endpoint-only CDF retention and preserve-forever defaults. Keep explicit full rebuild and controlled consolidation. Unsafe native operation denial remains a negative/error outcome, not the terminal implementation.

Exit: `just real-time-cpg-decommission-check DB27`, `just delta-publication-contract-check`, `just delta-exact-version-reconstruction-check`, `just real-time-cpg-native-maintenance-check`, `just vacuum-dry-run-check`. Prove unchanged-pin reuse, empty-owner withdrawal, protected interval reconstruction and real eligible reclamation.

### DB28 — Broad optimizer exclusions and custom native duplicates

Prerequisites: WP96, WP97, WP102, WP105. Delete global logical/physical projection-rule suppression and shape-only identity relabeling after A04 passes. Remove custom calculations/scan/pruning/relational evaluators only where their selected native replacement preserves the actual contract. Do not remove safe generic authority/schema boundaries merely because they are wrappers.

Exit: `just real-time-cpg-decommission-check DB28`, `just real-time-cpg-optimizer-identity-check`, `just datafusion-scan-contract-check`, `just provider-statistics-contract-check`. Stock rules execute with real pruning and identical semantic identity; conflicting identities fail.

### DB29 — Narrow query and presentation pseudo-functionality

Prerequisites: WP98, WP99, WP100, WP102, WP105. Remove phrase-only entity execution, declaration-only eight-form coverage, whole-request failure on an independent block gap, output-only graph bounds, all-source recursion for demand-root requests, shortest-path prefix enumeration, resource-only canonical-response placeholders and ignored delivery. Keep bounded simple-path enumeration where that is the actual requested policy.

Exit: `just real-time-cpg-decommission-check DB29`, `just semantic-request-program-check`, `just graph-query-resource-operations-check`, `just fastmcp4-stdio-vertical-check`, `just fastmcp4-adapter-authority-zero-state-check`. Every operand/delivery choice is causally exercised; Python remains presentation only.

### DB30 — Unowned resources, redundant proof and stale terminal machinery

Prerequisites: WP79, WP93, WP95, WP97, WP100, WP101, WP103, WP104, WP105. Retire per-epoch full-budget multiplication, detached/unjoined work, unbounded graph/IPC/proof/result buffers, shared request-local cache authority, accidental repeated dependency execution, digest-only correctness and active v7-specific terminal/performance selection. Preserve historical v7 evidence and honest diagnostics without allowing them to certify the successor.

Exit: `just real-time-cpg-decommission-check DB30`, `just cancellation-tree-check`, `just grpc-flow-control-contract-check`, `just datafusion-cache-resource-operations-check`, `just real-time-cpg-performance-check`, `just remaining-legacy-zero-state-check`. No baseline Clippy or required operational failure is grandfathered into final success.

## 7. Layered gate matrix

### 7.1 Proposed recipe ownership

These names are **not current capabilities**. The named packet must implement the recipe and its substantive fail-closed test selection before relying on it:

- WP77: `just real-time-cpg-packet-check <WP>`, `just real-time-cpg-milestone-check <M>`, `just real-time-cpg-decommission-check <DB>`, and the initial `just real-time-cpg-certification` orchestration contract. Reuse existing artifact/oracle infrastructure; do not create a second semantic registry or handwritten pass ledger. Later owners implement their actual tests/rules; WP105 finalizes terminal orchestration before measurement.
- WP79: `just native-dependency-artifacts-check` and `just real-time-cpg-native-resource-check` — fail-closed artifact/source integrity and native decode/replay/retention/worker falsifiers through production consumers. These checks join M18 and remain terminal leaf obligations.
- WP94: `just real-time-cpg-native-maintenance-check` — native command/approved-set/dependency contract; WP95 adds actual retention/reclamation cases.
- WP96: `just real-time-cpg-optimizer-identity-check` — mandatory A04 stock-optimizer/native-scan field-identity matrix.
- WP102: `just real-time-cpg-installed-conformance-check` — complete selected profiles/forms in the actual topology and same-PID update sequences.
- WP104: `just real-time-cpg-performance-check` and confirm-gated **mutating** `just real-time-cpg-performance-capture` — current-candidate preregistered evidence. Capture is never a gate dependency.
- WP106 verifies `just real-time-cpg-certification` nonmutatingly at the WP104 captured candidate: acyclic derived terminal orchestration, substantive child failures, current candidate and review closure.

Every ordinary `just` recipe named elsewhere was discovered in `just --list`. Those existing commands require the packet's stated workload expansions; their broad names or descriptions do not certify missing semantics. A required renamed/removed recipe causes a plan/gate revision, not a silent skip.

### 7.2 Edit-local and packet-local

Use the packet's domain-specific edit-local checks after coherent microchanges, its four named tests and its packet-local recipes before completion. Test/governance helper changes additionally run `just governance-tooling-lint`. Native provider/contract changes run the corresponding sidecar/extractor and generated-wire checks at their coherent boundary. Protobuf regeneration, formatting writes, snapshots, dependency edits and benchmark capture are deliberate mutating implementation steps with inspected diffs—not automatic repair after a red gate.

A packet is complete only at its proving commit **and** current HEAD with all named local obligations passing. Evidence may become stale; record judgment/deviations in schema-2 state and derive outcomes. Do not inflate a narrow check into all-domain or whole-profile proof.

### 7.3 Final nonmutating leaf gates

The terminal engine derives all 120 packet-oracle names, every milestone and DB obligation, and the current successor plan's declared-input/pin requirements. It expands duplicate aggregate commands to an acyclic executable graph, preserving required coverage and child exit codes. It does not invoke itself as a child or silently omit an obligation to break a cycle.

Run these retained-domain gates as applicable to the complete target (all four domains are in scope):

- `just ci-fast`
- `just ci-pr`
- `just root-test` — includes nextest and doctests
- `just extractor-ci-fast`
- `just sidecar-ci-fast`
- `just adapter-ci-fast`
- `just adapter-wheel-test`
- `just adapter-stdio-test`
- `just features-each`
- `just features-no-default`
- `just feature-architecture-check`
- `just stable-graph-check`
- `just data-fabric-upgrade-check`
- `just deps-fast`
- `just policy`
- `just sidecar-policy`
- `just proto-check`
- `just proto-repro-check`
- `just authoritative-design-conformance-check`
- `just governance`
- `just governance-tooling-lint`
- `just gate-filter-census`
- `just oracle-substance-check`
- `just tracked-target-zero-state-check`
- `just fastmcp4-package-build-check`
- `just fastmcp4-public-surface-check`
- `just fastmcp4-modern-protocol-check`
- `just fastmcp4-daemon-wire-contract-check`
- `just fastmcp4-security-negative-check`
- `just fastmcp4-resource-authority-check`
- `just fastmcp4-atomic-start-check`
- `just fastmcp4-completion-authorization-check`
- `just semantic-sandbox-host-matrix-check`
- `just supervisor-launch-platform-check`
- `just supervisor-restart-join-operations-check`
- `just compiled-release-fresh-activation-check`
- `just installed_target_authority_integrity`
- `just installed_vertical_fault_and_recovery_matrix`
- `just semantic-release-restart-reconstruction-check`
- `just native-dependency-artifacts-check`
- `just real-time-cpg-native-resource-check`
- `just real-time-cpg-native-maintenance-check`
- `just real-time-cpg-optimizer-identity-check`
- `just real-time-cpg-installed-conformance-check`
- `just real-time-cpg-performance-check`

The source review's full acceptance matrix and every packet-local behavior/security/recovery command remain obligations, not just this nonduplicative final list. At execution, `just artifacts-check`, `just plan-status` and `just plan-dependency-check` must validate the **then-active successor** and current proving commits. The unmodified v7 terminal/performance gates cannot certify this target; their valid substantive expectations are retained through the successor, not their stale outcomes.

Risk-triggered mutation/fuzz/coverage/unsafe checks are selected for actual changed parser, protocol, concurrency or security surfaces according to AGENTS.md. Do not run Tier C merely for appearance, and do not claim those tests from an ordinary CI pass. Any newly introduced unsafe boundary requires explicit design review and the prescribed assurance; no new unsafe code is presumed by this plan.

### 7.4 Completion evidence and independent review

The final candidate must be a stable committed target with no unexplained tracked implementation changes. Capture method/workload/source identity must match that candidate. Performance capture and measurement review are separate from nonmutating certification and cannot be restamped after drift.

An independent implementation review reconstructs design conformance, installed behavior, library choices, legacy exits and operational evidence. Resolve every blocking/major finding through its named failing-then-passing test and rerun affected integration/final gates. Runtime semantic uncertainty may remain only as the selected profile permits; an unavailable required library artifact, unimplemented family/form, failed reclamation, missed accepted performance target or red gate prevents full completion.

## 8. Execution sequence and state discipline

### 8.1 Dependency-safe sequence

A valid default order, with explicit independent branches:

1. WP77 → WP78 → WP79 → M18.
2. WP80, WP81 and WP82 may run independently after current-tree write-set checks; merge deliberately into WP83 → M19.
3. Semantic analysis branch: WP84 → WP85 → WP86 → WP87 → M20.
4. After WP83, source-watch WP88 and storage WP91 → WP92 may proceed independently of the analysis branch where actual writes are disjoint.
5. Join WP87 and WP88 into WP89; join WP89 and WP92 into WP90 → M21.
6. Storage/native branch after WP92: WP93 → WP94 → required versioned pin/plan-authority amendment → WP95 → WP96. WP97 joins that work with WP87 → M22.
7. WP99 joins WP97 and WP89 and closes the typed graph execution component; WP98 joins WP99 and WP90 into the full eight-form compiler, then WP100 → WP101 → M23.
8. WP102 → WP103 → WP105. WP105 closes source/compiled/semantic decommission residue, all baseline quality failures and terminal tooling before final measurement.
9. WP104 preregisters and measures the frozen target after WP105 → M24. Any candidate-affecting repair re-runs affected integration/quality checks and freezes a new candidate before final capture.
10. WP106 nonmutatingly verifies all DB24–DB30 final exits, current full certification and independent review → M25. Repaired findings reopen affected packets and WP104 for revalidation/refreeze/recapture; they never retain stale performance evidence.

Deletion occurs with the coherent replacement in its owning packet; final DB aggregation is not permission to retain a dual path until the end. Prototype/native capability probes stay private to tests until their production contract is proved.

### 8.2 Future state and activation

Future state path: `docs/plans/state/codefabric-real-time-cpg_v3_state.json`; schema version 2. Creation occurs only through the validated activation transaction.

After explicit plan approval and execution authorization, use the repository's confirm-gated activation workflow. It creates validated state before atomically selecting this plan and never overwrites existing state. The prior active v2 plan/state and the older v7 lineage remain history. Migrate v2 judgment fields and historical proving commits under §1.3; do not import completion or erase failed approaches. A failed activation leaves the prior pointer unchanged.

Execution state stores judgments, proving commits, deviations, failures and blockers only. It does not store regenerated changed-file inventories, checksums, check outcomes or hand-maintained coverage. Re-run the named checks and derive ancestry/current trust. For a planned pin-authority amendment, retain stable IDs and migrate through the accepted successor-plan/state workflow; do not edit this immutable plan or its declared inputs in place.

### 8.3 Artifact validation and inherited authoring checks

Before activation, use the repository activation transaction's preflight, which permits its not-yet-created state. After activation the same nonmutating validator functions below validate the selected artifact/state:

```bash
scripts/repo-shell.sh -c 'PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python -' <<'PY'
from tooling.ci.artifact_contracts import ROOT, validate_plan
from tooling.ci.plan_assurance import _oracle_contracts

plan = ROOT / "docs/plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md"
values = validate_plan(ROOT, plan)
print(values["plan_id"], values["version"], values["ids"])
print("oracle contracts", len(_oracle_contracts(plan)))
PY
just plan-dependency-check docs/plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md
```

These checks validate draft structure, declared inputs, four-oracle mappings, dependencies and known overlaps. They do not implement or execute proposed acceptance tests, approve the plan, create state or certify runtime behavior.

V1 authoring verification on 2026-09-04 passed: its 26 declared-input digests; 30 packets, eight milestones and seven decommission batches; 120 distinct proposed oracle/criterion mappings; acyclic dependency closure with no unordered known-touch overlaps; all 81 known-touch files present; all 30 structural preflight commands executable; and all 109 existing recipe names found, with nine proposed names explicitly owned in §7.1. `typos` and new-file whitespace checks passed. `just artifacts-check` also passed its 21 tooling tests and validation of the unchanged active v7 artifact; that separate result is not successor execution evidence. Independent planning reviewers checked scope, native feasibility and the corrected withdrawal/query/measurement sequencing. The implementation diff digest remains the baseline value; the existing `ci-fast` failure remains open.

## 9. Plan risks and adaptive replan policy

### 9.1 Named risk ownership

- **D-RT01 dependency completeness:** WP78/WP89 broaden to namespace/context when support is incomplete. Lack of narrow invalidation is a performance risk, not permission for stale facts.
- **D-RT02 admission:** WP77/WP90 reject missing required evidence and withdraw invalidated semantics. Honest language/input uncertainty does not excuse an unimplemented required producer.
- **D-RT03 precision:** WP84–WP87 must falsify plausible wrong control/flow/graph algorithms independently. A schema-correct derived relation is not semantic conformance.
- **A01 optional empty predicate fast path:** WP91 probes it and closes minimum native tombstone/effective-state semantics unconditionally; WP92 extends segment publication/consolidation. No preflight failure can remove empty-owner withdrawal.
- **A02 retained-provider improvement:** WP81/WP88/WP89 prove context isolation, inventory semantics, joined cancellation and churn progress; WP104 measures the claimed benefit.
- **A03 quantitative targets:** WP104 owns preregistration and actual installed measurements; performance target revision requires explicit acceptance.
- **A04 optimizer feasibility:** WP96 blocks optimized-path acceptance until stock-rule field-identity and scan/properties oracles pass. Safe interim exclusions have DB28 as their removal condition.
- **LD-RT08 native maintenance:** WP94 must supply an approved reproducible native capability and synchronized pin amendment; WP95 must show real lease/CDF-safe reclamation. Unavailable distribution authority or an incompatible native contract blocks full completion.
- **Dirty/shared tree:** every packet attributes current overlaps and preserves unrelated changes. No reset, broad cleanup, blanket reformat or assumption of ownership.
- **Baseline quality and evidence drift:** WP105 closes all residual required quality failures; WP106 rejects stale measurement or proving-commit evidence.
- **External deployment/migration discovery:** stop to design the actual data/compatibility handoff; FreshActivation must not silently become destructive migration.

### 9.2 Adaptation versus reopening

**Implementation adaptation** stays within these contracts: helper/module organization, supported native builder choice, stronger local test, conservative invalidation widening, measured configuration within validated authority, or the selected tombstone path when optional predicate overwrite fails. Record it in execution state.

**Plan revision** is required when packet boundaries, actual dependencies, shared-file coordination, gate ownership, integration/decommission sequence or declared target inputs materially change. Version the plan; preserve stable IDs and give every unfinished obligation an explicit successor disposition. The planned LD-RT08 and approved LD-RT09 concrete pin amendments are such input transitions, not an invitation to restamp hashes.

**Design reopening** is required for changed public semantics, a different engine/process/authority, missing selected provider capability without a sound implementation, incompatible native maintenance/schema contracts, unsafe containment, changed supported deployment guarantees, unbounded dual authority or evidence that the accepted target cannot meet its security/reliability/performance obligations. Obtain acceptance of the revised design before continuing that path.

A failure is feedback to diagnose, not an automatic task-ending condition. Continue safe independent work when dependencies allow, but never label a blocked required outcome complete. “Entire target” means every packet, milestone, decommission and final obligation, including actual native reclamation and complete agent semantics.

## 10. Planning handoff

This is the **approved native-resource successor** of the user's full implementation request. Activate through the validated transaction, migrate judgments under §1.3, implement LD-RT09 within WP79 and continue every remaining packet under impl-plan-exec. Prior plans and design inputs remain immutable; adoption of a concrete amended dependency requires its exact artifact and synchronized authority successor, already within the user's approved amendment scope.

The authoritative scope is the review's full design plus the accepted planning-contract addendum, including LD-RT08 and LD-RT09. Existing v7 boundaries and valid expectations remain required; no inherited red gate, permanent provider gap, disabled optimizer or denied maintenance operation is disguised as the completed product.
