---
artifact: implementation-plan
plan_id: codefabric-execution-proved-relational-data-fabric
version: v7
date: 2026-09-02
status: approved
design_path: docs/designs/codefabric_compiled_release_provider_fabric_runtime_boundary_design_v1_2026-09-02.md
design_version: v1
baseline_commit: de5db65c2834458eb57c7133183b8cef67a2491a
working_tree_digest_mode: git-status-diffs-and-untracked-content-excluding-activation-artifacts-v1
working_tree_digest: 1491556a494f2964cf87c44e71e03d31a6ed097801aedb3cdd92201fc6a0028f
activation_baseline_failure_1: "fact-generation narrow feature check fails on four daemon-gated provider authority imports"
activation_baseline_failure_2: "active v5 plan health is false because WP44 and WP47 proving commits are untrusted at current HEAD"
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v7_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
revises_draft_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v6_2026-09-02.md
---

# CodeFabric execution-proved relational data fabric -- implementation plan v7

This plan is the dependency-corrected successor to the never-activated v6 draft. It pivots the
unfinished active v5 program to the accepted compiled-release, provider, fabric, and runtime
boundary design while restoring the design's required sequence: application contracts, compiled
release, provider migration, fabric split, state split, and runtime ownership. It retains the v6
packet IDs because their outcomes and proof contracts remain valid; only the dependency closure,
milestone grouping, and activation assurance were defective. V5 WP43--WP48 proving commits remain
historical evidence for functional outcomes that the final v7 candidate must re-execute; their
completion labels are not copied into v7 state. Target-neutral deletions already present in the
dirty WP49 tree are retained after current-tree attribution, but uncommitted bytes receive no
completion credit.

This artifact is an execution specification, not an implementation patch. It does not create its
declared state file, change `docs/plans/active-plan.json`, mutate v5 state, or authorize a partially
cut-over production path. V6 remains immutable audit history and never becomes runtime authority.
Approval and the confirm-gated activation transaction remain separate operations.

## 1. Outcome, non-goals, baseline, and execution law

### 1.1 Outcome

At completion, the supported production path is:

```text
immutable source/context
  -> release-prepared ProviderJob
  -> provider-native execution
  -> owned Arrow batches or relation-scoped Arrow IPC + terminal coverage/gaps/provenance
  -> release-owned admission
  -> typed DataFusion transformations and independent relational proof
  -> exact Delta table versions + one activation event
  -> immutable FabricEpoch and least-authority child session
  -> daemon application service
  -> bounded generated-Protobuf/Tonic adapter
  -> one lifespan-owned Python DaemonPort
  -> presentation-only FastMCP 4
```

The completed implementation has all of the following observable properties:

1. One fallibly compiled, immutable `CompiledSemanticRelease` owns behavior-bearing provider,
   transformation, query, proof, and policy programs plus one categorical v2.3 suite identity.
   One `Arc` is constructed at daemon startup and injected into every application service; no
   global `current()` lookup or empty authority token remains.
2. Application-owned provider jobs and results are the only provider contract. Tree-sitter and Ruff
   emit owned Arrow batches; Pyrefly and rustc emit relation-scoped Arrow IPC. Provider-native,
   borrowed, and generated Protobuf values terminate inside their adapters.
3. The fabric capability is useful with synthetic Arrow inputs and depends on Arrow/DataFusion/
   Delta, not concrete providers, daemon/supervisor, Tonic, gix, or SQLite. Generic proof and closure
   evaluation remain inward; release-specific programs remain in the compiled release.
4. Delta owns exact single-table versions, the application activation event owns the exact
   multi-table vector, SQLite owns reconstructible operational coordination only, and gix owns
   repository observation only.
5. Every accepted query, provider run, DataFusion execution, materialization, resource operation,
   and stream bridge belongs to one structured daemon cancellation/task tree and is cancelled,
   observed, and joined exactly once.
6. One compatible Pyrefly sidecar per workspace/context retains incremental state across source
   generations. Cancellation preserves a healthy context; corruption, crash, trust loss, or an
   incompatible context emits an explicit gap and reconstructs from immutable inputs.
7. DataFusion 55 receives complete structured `ScanArgs`, including `StatisticsRequest`, through
   every provider/view wrapper. Schema-identity wrapping preserves values, ordering, partitioning,
   equivalence properties, metrics, and optimizer sequencing.
8. Generated Prost/Tonic messages terminate at transport or provider-process adapters. The released
   `codefabric.cpgd.v2` semantics and the four-tool/two-resource FastMCP 4 public surface remain, but
   transport and presentation cannot author semantic state.
9. Cargo features describe useful compiled capabilities. `provider-contracts`, `fact-generation`,
   `data-fabric`, `repository-input`, `operational-state`, `rpc`, `semantic-release`, and `daemon`
   have the dependency direction accepted by design I-60; `daemon` is the sole stable-root union.
10. Empty authority markers, mixed provider execution profiles, repeated global release lookup,
    stale live v2.2 identity, generated messages in admitted state, disposable Pyrefly ownership,
    reverse feature edges, abort/drop-without-join paths, and the interim core-module `daemon` cfg
    workaround reach physical zero state.
11. FreshActivation, restart, unknown-outcome reconciliation, slow-consumer behavior, provider
    faults, cancellation, installed FastMCP delivery, and the complete four-domain release are
    proved from one target candidate without predecessor comparison or hash-based correctness.

### 1.2 Non-goals

- No operability bridge, compatibility facade, dual execution, fallback constructor, feature alias,
  dormant old route, or restoration of v5 completion machinery.
- No dynamic suite registry, user-authored release program, YAML/JSON semantic program loader, hot
  suite switching, plugin substrate, or second production release constructor.
- No new Cargo root, root workspace, conceptual microcrate, Python service, or process boundary
  created solely for organization.
- No Python Arrow/DataFusion/Delta processing, semantic catalog, resource authority, query planning,
  identity authority, or provider inventory.
- No Protobuf duplication of semantic relations, generated Protobuf type in admitted domain state,
  or one-row semantic fact transport.
- No all-provider process migration without measured containment evidence, and no shared
  multi-workspace Pyrefly process without a separately accepted isolation design.
- No implicit-latest Delta authority, SQLite-selected semantic state, raw object listing as table
  truth, blind library retry, or cross-table atomicity claim.
- No new digest, registry, census, generated schema, comparator, or self-golden used as correctness
  proof. Existing identity/integrity bytes remain only where a released immutable contract requires
  them.
- No arbitrary SQL, serialized logical plans, unbounded bridge queues, generic Tower retries,
  speculative HTTP/2 tuning, or custom physical operator where a higher DataFusion abstraction
  preserves the required semantics.
- No source-file decomposition mandate. The repository's one-package constraint governs; execution
  may choose internal module names and file boundaries that best preserve the accepted dependency
  direction.

### 1.3 Baseline and inherited transition state

The baseline is `de5db65c2834458eb57c7133183b8cef67a2491a`, the same committed HEAD reviewed
by the accepted design. There is no committed drift from that baseline. The frontmatter
`working_tree_digest` identifies the complete pre-activation dirty tree by hashing porcelain-v2
status, staged and unstaged binary diffs, and every untracked file's path, mode, and bytes. The v7
plan, its future state file, and the active-plan pointer are excluded because activation creates or
switches those three artifacts. Nothing else, including `Untitled`, is excluded. The digest is
identity and drift proof only, not correctness or ownership proof.

The planning-session baseline observations are:

- `cargo check --locked --no-default-features --features fact-generation` fails with four
  `E0432`/`E0433` imports from `ruff_adapter` and `tree_sitter_adapter` into daemon-gated
  `production_kernel` and `production_provider_recipe` types. This is the primary architectural
  failure that WP53, WP57, and WP60 close.
- `just plan-status` reports the active v5 plan unhealthy, with WP44 and WP47 untrusted at current
  HEAD. No declared v5 input is stale and its baseline remains ancestral.
- `just artifacts-check` has one failure,
  `test_active_program_operational_acceptance`, because active v5 health is false. Its other
  artifact checks pass.
- V5 WP43--WP48 have recorded proving commits. V7 treats them as historical evidence for v2.3,
  released wire, FastMCP 4, and independent expectation outcomes, not as v7 packet completion.
- V5 WP49 is uncommitted and in progress. Its target-neutral removals are presumptively preserved;
  every overlap is attributed before editing and every deleted target consumer is independently
  re-established or deliberately displaced under this plan.

Execution records these baseline failures in v7 state when the plan is activated. A later failure
with a different fingerprint is not grandfathered.

### 1.4 Functional-outcome and decision-reversal law

The accepted v1 design and the v2.3 authoritative suite define the target outcomes and invariants.
Current code, v5 packet wording, earlier reviews, historical designs, generated products, and
proving commits are evidence, not compatibility constraints. An executor may delete, replace,
rename, combine, or reverse an earlier implementation or design decision when doing so better
achieves the accepted functional outcome and preserves or advances the target invariants.

This authority does not turn plan drift into an unrecorded local choice:

- an implementation adaptation that preserves the accepted architecture and invariants is recorded
  in execution state and proceeds;
- a change to packet boundaries, dependency order, cutover, or proof obligations requires a
  versioned plan revision; and
- a change to architecture, released public contract, library decision, or target invariant
  reopens the design and then produces a successor plan.

No stale oracle may force restoration of a hash, registry, static schema, old package, evidence
wrapper, comparator, legacy route, or daemon-owned provider profile. Preserve the valid functional
claim, move it to the target boundary, and prove it causally. No predecessor is preserved without a
verified external consumer, persisted-format constraint, or deployed-operability requirement.

### 1.5 Packet completion and cutover law

- A packet begins only after its dependencies are complete, current input freshness is derived,
  current-tree impact queries are rerun, and overlapping dirty paths are attributed or serialized.
- A packet is complete only when every named acceptance check passes at its proving commit and at
  current HEAD. The state file stores the proving commit and judgment only; check outcomes and
  changed files remain derived.
- New structural rules include positive and negative fixtures. A zero-hit source search alone is
  never a decommission proof; deletion requires structural, textual, and compiler/build evidence
  over a stated live scope.
- New contracts may exist in tests before production cutover, but there is never a second
  production execution path. WP61 performs the one constructor/composition cutover; WP62 deletes
  the displaced route before release-candidate work begins.
- A packet may refine a recipe introduced earlier, but the recipe's intent is monotonic. Earlier
  packet proof is rerun at HEAD after every refinement.
- Before hard cutover, rollback is ordinary commit-level rollback to the last dependency-closed
  target packet. After cutover, repair moves forward through the sole target constructor; no legacy
  route is revived.
- A target-format durable mutation uses the sole `FabricCommand`, exact Delta readback, and
  activation rules. Unknown outcomes are reconciled; durable target state is never deleted merely
  to make a test fresh.

## 2. Source design and declared inputs

The plan depends on the accepted pivot design, the audited but never activated v6 draft, the active
v5 execution specification, the sole current doctrine, and the synchronized v2.3 authoritative
suite. Digests are computed once at plan authoring time and thereafter only recomputed by artifact
tooling.

| path | sha256 |
|---|---|
| docs/designs/codefabric_compiled_release_provider_fabric_runtime_boundary_design_v1_2026-09-02.md | 9d857d81899b133664fcaf48294b58c2c3fa969e9394b68e8c376c2dd6ffd620 |
| docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v6_2026-09-02.md | 4b7b78c86aa450351bc1ecebd0b2238e7f34d24358985f1290320f2118805e28 |
| docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md | fe3259191cf8e90f8593d35ca913145789eed4ca6ba7f7592218a88effb398fb |
| docs/library_ref/full_data_fabric_design_principles_v2.md | eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md | 5d7309244156ed0833f1c0ae33f7796b9c9a4e88133c34dc2b1a956b47fc9e9f |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.3.md | aadd002c169aee5bd293fe5b9a01a07e76395157d7f9709132edcb3602a5b300 |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md | ce54d0b1bc9aeeb4d85de3e609faa1cf7f4e4c6f3a649645ee9de9c3fc63c4f5 |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md | 662b0c5bbf9f0963195b7fd3ad6f598ccfe2629fe1f7e5916e0cabef4c5104bf |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v2.3.md | 4968ce530ad4f23df2c08a4780ac3160222ab3ae6c81c183755524721969dcb3 |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md | 3b90ec9d779e350996882be28edc45fb52c9caf3e3e06af62716ca344e37194a |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md | b3b9365c2f6d53105f254f0f6a69da5fc950e321a91dfc0880e55d58b7e39143 |
| docs/authoritative_design/codefabric_2.3_implementation_roadmap_v1.0.md | 4a3741eeb740802f4161dd2b0d7ed6eddd72a640e54c92187ed40cf934e74da5 |

### 2.1 Design and library authority

The plan carries design I-60--I-71 and LD-40--LD-44 without weakening them. Live manifest/lock
resolution and the authoritative FAB §2.1 pin ledger outrank copied version prose. No dependency
upgrade is planned. The load-bearing decisions are:

- LD-40: Arrow 59.2.0 is the owned provider and fabric data boundary.
- LD-41: DataFusion 55.0.0 remains the visible semantic compiler and streamed executor.
- LD-42: delta-rs at exact revision `43a0cf10...` owns exact single-table durable state.
- LD-43: Tonic/Prost own transport and Tokio owns process-local async structure behind
  application-owned control values.
- LD-44: Tree-sitter, Ruff, Pyrefly, and rustc retain their native strengths behind owned adapters.

The implementation sequence follows accepted design §6.2 exactly: application contracts first,
then a behavior-bearing release compiler against current generic engines, then provider migration,
generic fabric separation, state separation, and full runtime task ownership. Fixture jobs and
synthetic Arrow inputs causally prove the compiled release before production provider call sites
move. The old route remains the sole production path until WP61, and no dual production execution
is introduced.

## 3. Global target invariants and dependency graph

Every packet preserves all applicable accepted design invariants:

- **I-60:** dependencies point inward and remain acyclic.
- **I-61:** one behavior-bearing compiled release is constructed once and injected.
- **I-62:** provider execution is an immutable job; semantic authority is release-owned admission.
- **I-63:** Arrow batches or relation-scoped Arrow IPC are the provider data boundary.
- **I-64:** provider-native state remains bounded and adapter-private.
- **I-65:** DataFusion retains visible typed semantics and complete scan/execution properties.
- **I-66:** exact Delta domain state, repository input, and temporal SQLite state remain distinct.
- **I-67:** cancellation and spawned work form one owned, joined hierarchy.
- **I-68:** generated transport types terminate at adapters.
- **I-69:** suite, application, adapter, provider, query, proof, wire, package, and storage identities
  are categorical rather than conflated strings.
- **I-70:** cutover is hard and physically deletes the displaced route.
- **I-71:** architecture and correctness are proved by discriminating execution, not declarations.

These invariants advance doctrine P1--P3, P5, P9--P16, P20--P21, P23, and P26--P36 while
maintaining P7--P8, P11, P18--P19, P22, and P34. In particular, mutable inventories and capability
lists are derived, program operands are causally read, application types make invalid states hard to
construct, expectations remain independent, and recurring boundary claims become executable rules.

The dependency graph is:

```text
WP53 application contracts and feature substrate
  -> WP60 behavior-bearing compiled release
       +--> WP57 Tree-sitter/Ruff jobs --+
       +--> WP58 rustc boundary ---------+--> WP55 generic fabric
       +--> WP59 Pyrefly lifecycle ------+       -> WP54 state split
                                                    -> WP56 cancellation/task ownership
                                                         -> WP61 injected runtime/Tonic
                                                         -> WP62 hard deletion
                                                         -> WP63 target correctness/recovery
                                                         -> WP64 FreshActivation/DB23
                                                         -> WP65 performance/resources
                                                         -> WP66 certification
```

WP53 freezes the application boundary. WP60 then compiles behavior-bearing programs against that
boundary and the existing generic engines, using fixture jobs and synthetic Arrow inputs before any
production provider call site moves. WP57--WP59 migrate the three provider lanes in parallel and
exercise the WP53 synchronous cancellation probe. WP55 can then make the fabric genuinely
provider-independent; WP54 separates durable semantic state from repository and operational state;
and WP56 composes all migrated work into the daemon-rooted structured task hierarchy. WP61 is the
only production cutover. FreshActivation and DB23 closure precede performance so the measured
candidate is the actual final topology. Any repair or tuning returns to the owning implementation
packet and invalidates all dependent WP63--WP65 evidence before WP66.

## 4. Dependency-closed work packets

### WP53 — Establish application-owned provider contracts and feature substrate

**Outcome.** The stable package exposes a useful `provider-contracts` capability containing validated,
application-owned categorical identities, provider jobs, results, coverage, gaps, diagnostics,
provenance, resource bounds, synchronous cancellation probes, and rustc compiler-run/owner/control
projections. It imports minimal Arrow array/schema types but no provider library, generated
Protobuf type, DataFusion, Delta, gix, SQLite, Tonic, daemon, or supervisor. Contract construction
and tamper faults are executable. No production provider path is switched in this packet.

**Dependencies.** None. Execution first attributes all dirty overlaps and records the planning-session baseline
failures.

**Target invariants.** I-60, I-62, I-63, I-64, I-67, I-69, and I-71.

**Design and library references.** Accepted design §§3.2--3.4, 3.10, 6.2 steps 1--2, and 7.2--7.3; LD-40 and LD-44; GEN §§4, 7,
10--11, and 90 plus AC-G-32/35/36; FAB §6; doctrine P1--P5, P9, P12, P16, P20, P25--P27,
P29--P33, P35--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/production_provider_recipe.rs src/provider_boundary.rs src/provider_admission.rs src/provider_types.rs src/analysis_context.rs src/identity.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' -g '!docs/plans/**' 'CompiledProviderExecutionProfile|CompiledProviderLane|ProviderRun|ProviderCoverage|ProviderGap|Cancellation' Cargo.toml src tests rules tooling/ci justfile
just stable-graph-check
```

Known touch: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/analysis_context.rs`, `src/cancellation.rs`,
`src/identity.rs`, `src/provider_boundary.rs`, `src/provider_types.rs`,
`src/provider_admission.rs`, `src/production_provider_recipe.rs`, `rules/`, `rule-tests/`,
`tooling/ci/`, tests, and `justfile`.

**Required changes.**

1. Add the minimal `provider-contracts` feature edge inside the existing package. Its public types
   are application-owned and provider-neutral; Arrow schema/array dependencies are allowed, but
   DataFusion/Delta and every concrete provider/runtime dependency are forbidden.
2. Define validated categorical types for suite/provider/protocol/schema/source/context/run and
   relation identities. Do not use one free-form version string for distinct concepts.
3. Define the application-owned rustc compiler-run header, owner header/terminal, compilation
   terminal, and provenance projections required by release compilation and later admission. They
   contain only categorical pins, stable application identities, counts, coverage, diagnostics,
   and provenance; generated Prost messages and compiler-private values are forbidden.
4. Define immutable lane jobs that bind requested families/scopes, exact identities, effective
   resource ceilings, deadline, trust posture, run provenance, and a bounded cancellation probe.
   Make illegal combinations unconstructable or reject them in one boundary constructor.
5. Define one terminal result shape for owned relation batches/streams, requested/completed/
   remainder/unknown coverage, diagnostics, trust/resource outcome, and terminal status. Empty
   complete, explicit unknown, intentional remainder, timeout, cancellation, corruption, and
   oversize are distinct states.
6. Separate release semantics, compiled policy ceilings, and per-run effective values. Establish
   one provider-neutral admission seam over the new result contract, then begin moving reusable
   relation/schema/authority knowledge out of the mixed provider execution profile without creating
   a second production authority. Later lane packets switch call sites but do not redefine this
   shared seam.
7. Add `provider-job-contract-check` and the provider-contracts slice of
   `feature-architecture-check`. Both include negative fixtures that would fail on a provider-native
   field, generated-message/control-projection field, forbidden dependency, widened bound, wrong
   pin, or false complete terminal.
8. Derive contract observations from constructed values. Do not add a generated registry, static
   capability census, or digest used as semantic proof.

**Legacy disposition and decommission.** The existing marker/profile route remains temporarily production-selected only because its
consumers have not moved. It may construct no target contract on an alternate execution path.
`CompiledProviderExecutionProfile`, `CompiledProviderLane`, and provider authority markers are
assigned to DB19/DB20 and must be deleted by WP62. No alias from the new types back to those types is
permitted.

**Acceptance checks.**

##### Behavioral

- `just provider-job-contract-check` constructs and executes independently authored valid job/result
  fixtures across every terminal/coverage class.

##### Structural

- `just feature-architecture-check provider-contracts` proves the feature is useful and has no
  forbidden library or outward-module edge.

##### Negative / Zero-State

- `just provider-job-contract-check` rejects wrong categorical identity, widened resources,
  malformed Arrow shape, false completion, omitted remainder, and non-owned boundary types.

##### Operational

- `just stable-graph-check` proves the single Arrow universe and no unintended feature activation.

Oracle catalog:

Executable oracle: `provider_contract_type_boundary_integrity`
Governed criterion: `PC-WP53-INT`

Executable oracle: `provider_job_result_terminal_semantics`
Governed criterion: `PC-WP53-BEH`

Executable oracle: `provider_job_validation_fault_matrix`
Governed criterion: `PC-WP53-NEG`

Executable oracle: `provider_contract_feature_isolation`
Governed criterion: `PC-WP53-OPS`

**Edit-Local Gates.** `just root-fmt`, focused provider-contract tests, `just root-check-fast`, and the affected structural
rule fixtures.

**Packet-Local Gates.** `just provider-job-contract-check`; `just feature-architecture-check provider-contracts`;
`just features-no-default`; `just stable-graph-check`; `just root-check`.

**Integration Milestone.** Advances M13.

**Replan Triggers.** Replan if minimal Arrow types pull DataFusion/Delta into the contract graph, categorical identities
cannot represent a required released concept, job/result construction requires provider-native or
generated types, or keeping one package cannot enforce the inward boundary.

**Rollback or Recovery.** No production route changes. Revert the contract packet as a unit if it cannot remain provider
neutral; do not widen `fact-generation` or `data-fabric` to `daemon`.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
ProviderJob = exact categorical pins + requested scope + bounded effective policy
              + deadline + cancellation probe + run provenance
ProviderRunResult = owned Arrow relations + terminal coverage/gaps/diagnostics
                    + exact input/output pins + resource outcome
```

Names and local file placement may change; this ownership and information content may not.

### WP54 — Separate fabric, repository-input, and operational-state authority

**Outcome.** `data-fabric`, `repository-input`, and `operational-state` are useful, independently checked feature
capabilities. `data-fabric` has no gix or SQLite dependency; repository observation has no fabric or
transport authority; operational SQLite state cannot select semantic table state. With the provider
and fabric migrations complete, this packet also activates the final `semantic-release ->
fact-generation + data-fabric` feature edge and proves the compiled release as an isolated useful
provider-to-proof capability. Existing exact Delta publication, readback, reconstruction,
activation, and unknown-outcome behavior remains causally green.

**Dependencies.** WP55. This packet follows the generic fabric split so semantic state can be
separated without preserving a reverse provider or runtime edge.

**Target invariants.** I-60--I-66, I-69, and I-71.

**Design and library references.** Accepted design §§3.7, 3.10--3.11, 6.2 step 6, and 7.3; LD-42; FAB §§9--11 and 14; LIFE §§1,
3, and 13; doctrine P3, P5, P7--P13, P19--P20, P23, P28--P29, P34--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/operational_store.rs src/workspace_registry.rs src/git_state.rs src/fabric/delta_exact.rs src/fabric/delta_write.rs src/fabric/activation.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'repository-state|rusqlite|gix|OperationalStore|WorkspaceRegistry|ExactDeltaPin|ActivationEvent' Cargo.toml src tests scripts tooling/ci justfile
just stable-graph-check
```

Known touch: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/fabric.rs`, `src/operational_store.rs`,
`src/workspace_registry.rs`, `src/git_state.rs`, `src/fabric/delta_exact.rs`,
`src/fabric/delta_write.rs`, `src/fabric/delta_semantic_read.rs`,
`src/fabric/programmatic_relation_delta.rs`, `src/fabric/activation.rs`,
`scripts/stable_graph_check.sh`, tests, tooling, rules, and `justfile`.

**Required changes.**

1. Replace the broad `repository-state` feature with separate `repository-input` and
   `operational-state` capabilities. Keep compatibility-probe aggregation explicit and keep daemon
   as the sole union.
2. Reclassify module gates so gix/descriptor-relative source observation belongs only to
   repository input and SQLite queues, attempts, leases, checkpoints, workspace records, and
   progress belong only to operational state.
3. Remove the `data-fabric -> repository-state` edge. Fabric contracts accept application-owned
   ports/values rather than importing concrete gix/SQLite adapters.
4. Preserve exact delta-rs providers, zero library retries, exact commit readback, unknown-outcome
   reconciliation, exact-version reopen, partial-publication invisibility, and application-owned
   activation vectors.
5. Add `delta-publication-contract-check` with partial commit, exact activation, newer-head/older-pin
   reopen, competing writer, and unknown-outcome faults. Extend `feature-architecture-check` to
   derive and enforce the three state graphs.
6. Keep existing hashes only for immutable transaction/integrity identity. Correctness remains exact
   readback, decoded rows, activation-chain validation, and reconstruction.
7. Activate the final `semantic-release` feature over the now-isolated `fact-generation` and
   `data-fabric` capabilities. Prove its provider-to-proof fixture under `--no-default-features`
   and reject repository-input, operational-state, RPC, supervisor, or daemon reachability.

**Legacy disposition and decommission.** The old `repository-state` feature, reverse imports, and any SQLite/gix semantic-state selection
enter DB21. A temporary Cargo feature alias is forbidden. Current operational schemas and Delta
history are retained; this is an ownership/feature migration, not a data rewrite.

**Acceptance checks.**

##### Behavioral

- `just delta-publication-contract-check` proves exact single-table commit and application
  multi-table activation behavior.

##### Structural

- `just feature-architecture-check state` proves independent useful state graphs and forbidden
  dependency-family absence; `just feature-architecture-check semantic-release` proves the final
  release feature over the completed inward graph.

##### Negative / Zero-State

- `just activation-receipt-nonauthority-check` rejects SQLite, receipt, hash, cache, predecessor
  schema, or implicit-latest substitution.

##### Operational

- `just delta-exact-reconstruction-v4-check`, `just delta-durability-protocol-integrity-check`, and
  `just semantic-request-program-check` prove exact restart, protocol behavior, and isolated
  provider-to-proof release execution.

Oracle catalog:

Executable oracle: `state_authority_feature_graph_integrity`
Governed criterion: `PC-WP54-INT`

Executable oracle: `exact_delta_activation_authority_split`
Governed criterion: `PC-WP54-BEH`

Executable oracle: `operational_receipt_nonauthority_faults`
Governed criterion: `PC-WP54-NEG`

Executable oracle: `state_capability_reconstruction_operations`
Governed criterion: `PC-WP54-OPS`

**Edit-Local Gates.** Focused feature checks, Delta/activation tests, `just root-check-fast`, and affected graph-script
tests after each dependency cluster.

**Packet-Local Gates.** `just feature-architecture-check state`;
`just feature-architecture-check semantic-release`; `just release-program-contract-check`;
`just semantic-request-program-check`; `just delta-publication-contract-check`;
`just delta-durability-protocol-integrity-check`; `just delta-exact-reconstruction-v4-check`;
`just activation-receipt-nonauthority-check`; `just stable-graph-check`.

**Integration Milestone.** Advances M15.

**Replan Triggers.** Replan if exact Delta behavior requires SQLite/gix authority, an operational store contains
irreducible semantic relation truth, delta-rs lacks the exact readback/reconciliation seam, the
final semantic-release graph retains an outward state/runtime dependency, or the feature split
requires a new package rather than same-package boundaries.

**Rollback or Recovery.** Before runtime cutover, revert feature/module gating as one coherent packet while leaving durable
Delta and SQLite bytes untouched. Never create an alias or dual semantic selector to ease rollback.

**Design-Bearing Contracts and Exemplars (conditional).** No new persistence schema is prescribed. The load-bearing distinction is exact Delta domain truth
versus reconstructible repository/operational inputs.

### WP55 — Establish the lossless generic Arrow/DataFusion fabric boundary

**Outcome.** The isolated `data-fabric` capability constructs a candidate from synthetic application-owned Arrow
batches, installs typed programmatic relations, executes generic transformations and relational
predicates, persists/reopens exact Delta state, and streams DataFusion results without concrete
providers, daemon, repository input, operational state, RPC, or supervisor. Every provider/view
wrapper preserves DataFusion 55 structured scan inputs and execution properties. Release-specific
proof/query constants remain selected by the existing production route until WP60 moves them into
the behavior-bearing release in the same dependency-closed transaction.

**Dependencies.** WP57, WP58, WP59, and WP60. All production provider lanes must terminate at the
application contract before fabric loses concrete-provider reachability.

**Target invariants.** I-60, I-63, I-65, I-66, and I-71.

**Design and library references.** Accepted design §§3.6--3.7, 6.2 step 5, and 7.2--7.3; LD-40--LD-42; FAB §§3, 6--10, and 13;
doctrine P2, P4--P8, P12, P14--P16, P20--P21, P25, P27, P29, P33, P35--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/fabric/derived_producer_closure.rs src/fabric/proof.rs src/fabric/programmatic_schema.rs src/fabric/provider.rs src/fabric/child_session.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'IdentityPreservingViewTable|SchemaIdentityExec|scan_with_args|StatisticsRequest|CompiledProofAuthority|CompiledQueryAuthority|feature = "daemon"' src/fabric src tests rules tooling/ci justfile
just datafusion-contract-matrix-integrity-check
```

Known touch: `src/fabric.rs`, `src/fabric/derived_producer_closure.rs`, `src/fabric/proof.rs`,
`src/fabric/programmatic_schema.rs`, `src/fabric/provider.rs`, `src/fabric/child_session.rs`,
`src/fabric/programmatic_epoch.rs`, `src/fabric/relational_query_runtime.rs`,
`src/fabric/arrow_result_resource.rs`, Cargo feature metadata, tests, rules, tooling, and `justfile`.

**Required changes.**

1. Establish or narrow the application-owned generic program/evaluator seams needed by the future
   compiled release. Do not move release-specific relation IDs, expectations, faults, or query
   selection without their WP60 owner, and do not create a placeholder second authority.
2. Ungate only fabric behavior that is already semantically generic. The remaining proof/closure
   daemon gates are assigned to WP60, where their release-specific inputs move outward atomically.
3. Implement lossless `scan_with_args` delegation for `IdentityPreservingViewTable`, including
   projection, filters, limit, and `StatisticsRequest`. Preserve the deliberate nested optimizer
   ordering or replace it only with a proven DataFusion-native higher seam.
4. Keep `SchemaIdentityExec` only as a narrow metadata-identity boundary. Prove identical values,
   output schema identity, ordering, partitioning, equivalence properties, metrics, and child
   visibility.
5. Retain typed `Expr`/`LogicalPlan`, plan-derived schema, native catalog/provider APIs, streamed
   execution, least-authority child sessions, and explicit bounded materialization only where proof
   requires it.
6. Add `data-fabric-core-check` and `datafusion-scan-contract-check`. Use a spy provider and
   independent faults that detect any dropped scan argument, schema/property drift, opaque semantic
   operator, daemon import, or eager unbounded collection.

**Legacy disposition and decommission.** Release-specific constants/expectations and their interim daemon cfg gates move atomically in WP60.
Generic fabric code may expose an application-owned program/evaluator port but no release token.
Daemon/resource bridge imports enter DB21. `SchemaIdentityExec` is retained only if its complete
contract passes.

**Acceptance checks.**

##### Behavioral

- `just data-fabric-core-check` executes a synthetic Arrow-to-proof-to-exact-Delta fixture.

##### Structural

- `just feature-architecture-check data-fabric` rejects provider, daemon, repository, operational,
  and RPC dependencies.

##### Negative / Zero-State

- `just datafusion-scan-contract-check` faults every `ScanArgs` member and execution property and
  must detect the loss.

##### Operational

- `just datafusion-plan-schema-cache-check` and
  `just scheduled-streamed-semantic-query-check` preserve plan-derived schema, reuse, bounds, and
  streaming behavior.

Oracle catalog:

Executable oracle: `generic_fabric_dependency_boundary_integrity`
Governed criterion: `PC-WP55-INT`

Executable oracle: `synthetic_arrow_fabric_end_to_end`
Governed criterion: `PC-WP55-BEH`

Executable oracle: `datafusion_scan_and_property_loss_faults`
Governed criterion: `PC-WP55-NEG`

Executable oracle: `datafusion_stream_cache_resource_operations`
Governed criterion: `PC-WP55-OPS`

**Edit-Local Gates.** Focused fabric/provider unit tests, `just root-fmt`, `just root-check-fast`, and the two new focused
recipes after each wrapper or module-boundary change.

**Packet-Local Gates.** `just data-fabric-core-check`; `just datafusion-scan-contract-check`;
`just datafusion-contract-matrix-integrity-check`; `just datafusion-plan-schema-cache-check`;
`just scheduled-streamed-semantic-query-check`; `just feature-architecture-check data-fabric`.

**Integration Milestone.** Advances M15.

**Replan Triggers.** Replan if DataFusion 55 cannot preserve structured scan requests or physical properties through the
chosen wrapper, generic proof cannot be separated from release semantics without duplicating
meaning, or a custom operator becomes necessary below the accepted highest viable extension rung.

**Rollback or Recovery.** Revert the affected wrapper/generic split to the last green target packet before production
cutover. Do not restore blanket daemon gating as the solution; a failed separation reopens the
boundary design.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
generic fabric: application-owned Arrow + typed program -> Expr/LogicalPlan -> stream/predicate
release layer in WP60: owns relation IDs, expectations, faults, and query families
```

### WP56 — Establish structured cancellation and owned task scopes

**Outcome.** One daemon-rooted `tokio-util::sync::CancellationToken` tree and application wrapper owns every
accepted query and child operation. Durable cancellation identity remains CodeFabric state;
synchronous loops receive a bounded probe. Every production spawn is registered with an owner and
every terminal, expiry, failure, and drain path cancels, observes, and joins it.

**Dependencies.** WP54. Provider migration, generic fabric separation, and state separation are
complete before the daemon composes their work into one task hierarchy.

**Target invariants.** I-60, I-62, I-64, I-67, I-68, and I-71.

**Design and library references.** Accepted design §§3.8, 3.11, 6.2 step 7, and 7.3--7.4; LD-43; LIFE §§1, 3, and 10; QRY §9;
SRV §§6 and 8; doctrine P11--P13, P16, P20, P23--P25, P32--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/cancellation.rs src/fabric/query_coordinator.rs src/daemon.rs src/query_service.rs src/supervisor.rs --view signatures
ast-grep run -l rust -p 'tokio::spawn($$$A)' src tests rustc-extractor pyrefly-sidecar
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'JoinHandle|\.abort\(\)|tasks.remove|cancel_idempotent|cleanup reserve|drain' src tests rules tooling/ci justfile
```

Known touch: `Cargo.toml`, `Cargo.lock`, `src/cancellation.rs`, `src/fabric/query_coordinator.rs`,
`src/fabric/child_session/resource_governance.rs`, `src/fabric/command_actor.rs`,
`src/fabric/command_runtime.rs`, `src/daemon.rs`, `src/query_service.rs`, `src/supervisor.rs`,
`src/freshness.rs`, `src/pyrefly_service.rs`, `src/rustc_service.rs`, rules, tests, tooling, and
`justfile`.

**Required changes.**

1. Add exact direct `tokio-util` cancellation support only to daemon/runtime capabilities. Preserve
   a narrow application cancellation type for durable IDs, budgets, terminal semantics, and
   synchronous polling.
2. Define parent/child ownership from daemon to workspace, query, provider/freshness, DataFusion,
   materialization/package, resource, and transport-observation tasks. Dropping a watch stream ends
   observation, not accepted logical work.
3. Bridge each child token into the bounded synchronous cancellation probes already consumed by
   Tree-sitter, Ruff, rustc, and the long-lived Pyrefly workspace/context. Prove Pyrefly run cancel
   preserves a healthy context while daemon drain cancels and joins the context owner.
4. Replace unowned spawn and abort/drop-without-join behavior in production paths with registered
   task scopes. Terminal and expiry cannot merely remove a `JoinHandle`.
5. Drain in order: stop admission, signal the parent, reserve cleanup time, join children, release
   reservations/leases/provider/result resources, persist required terminal state, and close
   services.
6. Keep `CancelQuery` idempotent and durable across reconnect. Process escalation occurs only after
   a bounded cooperative-cancellation timeout.
7. Add `cancellation-tree-check` with parent/child propagation, sibling isolation, replay,
   cancellation at every lifecycle stage, cleanup reserve, and zero-live-task faults.

**Legacy disposition and decommission.** The atomic probe survives only as the synchronous leaf view of the structured token tree. Detached
retention work, unowned spawns, abort/drop-without-join, and conflation of stream drop with durable
cancel enter DB21.

**Acceptance checks.**

##### Behavioral

- `just cancellation-tree-check` proves parent/child propagation, sibling isolation, idempotent
  durable cancel, and exactly one terminal.

##### Structural

- `just feature-architecture-check cancellation` proves Tokio cancellation remains outward and all
  production spawns match an owned scope.

##### Negative / Zero-State

- `just cancellation-tree-check` injects a leaked task, removed-unjoined handle, exhausted cleanup
  reserve, and watch-drop-as-cancel fault and must reject each.

##### Operational

- `just query-retention-cancellation-restart-check` and
  `just supervisor-restart-join-operations-check` prove cleanup and restart behavior.

Oracle catalog:

Executable oracle: `structured_task_tree_ownership_integrity`
Governed criterion: `PC-WP56-INT`

Executable oracle: `cancellation_terminal_and_sibling_semantics`
Governed criterion: `PC-WP56-BEH`

Executable oracle: `unjoined_task_and_cleanup_reserve_faults`
Governed criterion: `PC-WP56-NEG`

Executable oracle: `cancellation_restart_drain_operations`
Governed criterion: `PC-WP56-OPS`

**Edit-Local Gates.** Focused cancellation/query-coordinator tests, `just root-check-fast`, and lifecycle tests after each
ownership transition.

**Packet-Local Gates.** `just cancellation-tree-check`; `just query-retention-cancellation-restart-check`;
`just programmatic-runtime-lifecycle-check`; `just supervisor-restart-join-operations-check`;
`just feature-architecture-check cancellation`.

**Integration Milestone.** Advances M15.

**Replan Triggers.** Replan if a production async task cannot acquire one owner/join path, a provider API cannot observe
bounded cancellation, durable query semantics require token persistence, or direct Tokio utility
adoption leaks into fact-generation contracts.

**Rollback or Recovery.** Before production cutover, revert one ownership cluster at a time to the last joined implementation.
Never recover by detaching a task, making a queue unbounded, or treating process kill as ordinary
cancellation.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
daemon token -> workspace token -> query token
  -> provider/freshness | DataFusion | materialization/package | resource children
durable cancellation identity != process-local token != dropped observation stream
```

### WP57 — Migrate Tree-sitter and Ruff to job-driven Arrow lanes

**Outcome.** Tree-sitter and Ruff constructors validate only their exact adapter/library configuration. Their run
methods consume application-owned jobs and emit owned Arrow batches, coverage, gaps, diagnostics,
and provenance without importing fabric, release, daemon, DataFusion, Delta, gix, SQLite, or RPC.
Provider-native values and catalog-observation helpers are private to their adapters. The isolated
`fact-generation` feature performs useful job-to-Arrow behavior and passes.

**Dependencies.** WP60.

**Target invariants.** I-60, I-62, I-63, I-64, I-67, and I-71.

**Design and library references.** Accepted design §§3.4--3.5, 3.10, 6.2 step 4, and 7.3; LD-40 and LD-44; GEN §§4--5, 7,
10--11, 90, and 93; FAB §6; doctrine P3--P5, P8--P9, P12, P16, P20, P22--P23, P25,
P27, P29--P33, P35--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/tree_sitter_adapter.rs src/ruff_adapter.rs src/provider_native_syntax.rs src/provider_raw_kinds.rs src/provider_boundary.rs src/provider_admission.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'tree_sitter::|ruff_python_|CompiledProviderAuthority|CompiledProviderExecutionProfile|CompiledProviderLane|provider_raw_kinds' src tests rules rule-tests tooling/ci justfile
just exact-provider-batch-check
```

Known touch: `src/tree_sitter_adapter.rs`, `src/ruff_adapter.rs`, `src/provider_native_syntax.rs`,
`src/provider_raw_kinds.rs`, `src/provider_boundary.rs`,
`tests/integration/fact_generation_build.rs`,
`rules/tree-sitter-boundary-only.yml`, `rules/ruff-boundary-only.yml`, their rule tests/snapshots,
Cargo feature metadata, provider tests/tooling, and `justfile`.

**Required changes.**

1. Replace adapter-constructor `CompiledProviderAuthority` and daemon execution-profile inputs with
   exact adapter configuration plus immutable `ProviderJob` at execution time.
2. Build relation batches directly against the job/session-derived Arrow-only contract. Validate
   lengths, types, nullability, metadata, row/byte ceilings, sequence, pins, raw-plus-normalized
   kinds, coverage, unknowns, and provenance before returning.
3. Move helpers that accept `tree_sitter::Language`, Ruff `NodeKind`, `TokenKind`, or other native
   values behind adapter-private seams. Only application-owned raw-kind entries and Arrow rows
   escape.
4. Preserve bounded Tree-sitter revision/change-range state and Ruff's honest whole-file reparse
   capability. Prove clean-versus-incremental semantic equivalence and bounded revision eviction.
5. Upgrade the isolated fact-generation integration test from raw library availability to actual
   provider-job execution and decoded Arrow/coverage assertions.
6. Tighten Tree-sitter/Ruff structural rules and negative fixtures; remove current exemptions for
   public native-type helper surfaces.
7. Add `inprocess-provider-lifecycle-check`; close the in-process portion of
   `provider-type-boundary-check` and extend `feature-architecture-check fact-generation`.

**Legacy disposition and decommission.** Delete the Tree-sitter/Ruff marker-authority constructor path and native-type public helpers in this
packet when their target consumers land. Residual shared marker/profile definitions remain DB19/
DB20 until other lanes migrate and WP62 proves repository-wide absence. No adapter compatibility
overload survives.

**Acceptance checks.**

##### Behavioral

- `just exact-provider-batch-check` proves decoded Tree-sitter/Ruff Arrow values, coverage, gaps,
  and raw-plus-normalized semantics.

##### Structural

- `just provider-type-boundary-check` and `just feature-architecture-check fact-generation` reject
  native/outward types and forbidden dependency families.

##### Negative / Zero-State

- `just provider-admission-exclusivity-check` rejects wrong jobs, malformed/oversize batches,
  false empty success, and bypass admission.

##### Operational

- `just inprocess-provider-lifecycle-check` proves incremental equivalence, cancellation
  responsiveness, bounded revision state, and clean reconstruction.

Oracle catalog:

Executable oracle: `inprocess_provider_boundary_integrity`
Governed criterion: `PC-WP57-INT`

Executable oracle: `tree_sitter_ruff_arrow_job_semantics`
Governed criterion: `PC-WP57-BEH`

Executable oracle: `inprocess_provider_admission_faults`
Governed criterion: `PC-WP57-NEG`

Executable oracle: `inprocess_provider_incremental_lifecycle`
Governed criterion: `PC-WP57-OPS`

**Edit-Local Gates.** Focused adapter tests, rule tests, `just root-fmt`, `just root-check-fast`, and the isolated
fact-generation test after each adapter boundary change.

**Packet-Local Gates.** `just provider-job-contract-check`; `just exact-provider-batch-check`;
`just inprocess-provider-lifecycle-check`; `just provider-type-boundary-check`;
`just provider-admission-exclusivity-check`; `just provider-trust-coverage-remainder-check`;
`just feature-architecture-check fact-generation`.

**Integration Milestone.** Advances M14.

**Replan Triggers.** Replan if owned Arrow construction is a measured prohibitive bottleneck, a native type is required
outside the adapter to preserve semantics, in-process crash/resource/cancellation evidence exceeds
the governed envelope, or fact-generation cannot remain useful without a daemon edge.

**Rollback or Recovery.** Before production cutover, revert one complete lane API and its consumers. Do not restore a shared
marker overload alongside the target API. A containment failure reopens process placement rather
than adding a hidden fallback.

**Design-Bearing Contracts and Exemplars (conditional).** No new adapter hierarchy is prescribed. Both lanes implement the same application job/result
contract while retaining their honest native incremental capabilities.

### WP58 — Terminate rustc generated types at the extractor adapter

**Outcome.** The dated-nightly rustc extractor retains its released Protobuf/Arrow IPC process contract, but the
stable root converts `CompilationBegin`, `OwnerBegin`, `OwnerEnd`, and `CompilationEnd` immediately
into application-owned compiler-run, owner, terminal, and provenance values. No generated message or
compiler-private value survives into admitted provider state. Rustc execution consumes the same
bounded provider-job semantics as other lanes.

**Dependencies.** WP60.

**Target invariants.** I-60, I-62--I-64, I-67--I-69, and I-71.

**Design and library references.** Accepted design §§3.4--3.5, 3.9, 6.3 material disposition, and 7.2--7.4; LD-43--LD-44;
GEN §§7, 11, 90 and AC-G-35; FAB §6; doctrine P3--P5, P9--P13, P16, P20, P22--P25,
P27, P30, P32--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/rustc_service.rs src/provider_admission.rs src/rust_compilation_trust.rs src/rustc_relation_schema.rs rustc-extractor/src/wrapper.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'AcceptedRustcOwner|AcceptedRustcCompilation|OwnerBegin|OwnerEnd|CompilationBegin|CompilationEnd|CompiledProviderAuthority' src rustc-extractor tests rules tooling/ci contracts/rpc justfile
just provider-ipc-contract-integrity-check
```

Known touch: `contracts/rpc/rustc_extractor.proto`, `rustc-extractor/src/wrapper.rs`, `src/rustc_service.rs`,
`src/rust_compilation_trust.rs`, `src/rustc_relation_schema.rs`, relation-IPC modules,
extractor/integration tests, boundary
rules/fixtures, proto tooling, and `justfile`.

**Required changes.**

1. Use the application-owned compilation/owner headers and terminals defined by WP53; do not
   redefine them in the adapter. Populate their validated categorical pins, stable application
   identities, counts, coverage, diagnostics, and provenance from transport-local events.
2. Convert every generated begin/end event at the stable-root transport ingress. Keep generated
   messages inside generated modules, the extractor wrapper, and the narrow transport validator.
3. Convert at transport ingress and pass only application-owned values into the provider-neutral
   admission seam established by WP53. Update trust qualification, relation-schema validation,
   production provider composition, fixtures, and tests to consume only those values without
   redefining the shared admission contract.
4. Replace rustc marker/profile inputs with a provider job at the lifecycle boundary. Preserve
   exact build/toolchain/protocol/schema/trust/resource validation, bounded Arrow IPC, explicit
   compile gaps, cancellation, process-group escalation, and joined cleanup.
5. Add the rustc slice of `generated-type-boundary-check` plus
   `rustc-provider-lifecycle-check`, each with seeded generated-type, protocol, cancellation,
   truncation, wrong-pin, compile-failure, and cleanup faults.
6. Retain all four canonical proto sources and generated Rust families. The dirty removal of Python
   rustc/provider-sidecar bindings remains correct because the FastMCP adapter consumes only the CPG
   query family.

**Legacy disposition and decommission.** Generated rustc message fields in `AcceptedRustc*`, marker-authority lifecycle arguments, and any
stable-root admitted DTO containing generated types are deleted in this packet. Residual shared
definitions and stale fixtures enter DB19/DB20/DB22. The released rustc process protocol remains.

**Acceptance checks.**

##### Behavioral

- `just rustc-provider-lifecycle-check` executes success, compile gap, cancel, crash, and clean
  reconstruction through application-owned accepted values.

##### Structural

- `just generated-type-boundary-check` rejects generated Rust/Python messages outside their
  approved transport/process modules.

##### Negative / Zero-State

- `just provider-ipc-contract-integrity-check` and
  `just provider-admission-exclusivity-check` reject corruption, wrong pins, false completion, and
  admission bypass.

##### Operational

- `just relation-ipc-provider-operations-check` and `just extractor-ci-fast` prove bounded flow,
  cancellation, cleanup, and the dated-nightly build domain.

Oracle catalog:

Executable oracle: `rustc_generated_type_termination_integrity`
Governed criterion: `PC-WP58-INT`

Executable oracle: `rustc_provider_application_result_semantics`
Governed criterion: `PC-WP58-BEH`

Executable oracle: `rustc_ipc_and_admission_faults`
Governed criterion: `PC-WP58-NEG`

Executable oracle: `rustc_provider_process_lifecycle`
Governed criterion: `PC-WP58-OPS`

**Edit-Local Gates.** Focused rustc-service/admission tests, `just extractor-check`, `just root-check-fast`, proto checks,
and generated-type rule fixtures after each conversion cluster.

**Packet-Local Gates.** `just generated-type-boundary-check`; `just rustc-provider-lifecycle-check`;
`just provider-ipc-contract-integrity-check`; `just relation-ipc-provider-operations-check`;
`just provider-trust-coverage-remainder-check`; `just extractor-ci-fast`; `just proto-check`.

**Integration Milestone.** Advances M14.

**Replan Triggers.** Replan if the released process protocol lacks a field required to construct a correct
application-owned value, conversion would duplicate semantic relations in Protobuf, rustc-private
values must escape the compiler callback, or cleanup cannot remain bounded and joined.

**Rollback or Recovery.** Before runtime cutover, revert the entire stable-root conversion and its consumers without editing
the released proto. Do not retain parallel generated/application DTOs or a conversion bypass.

**Design-Bearing Contracts and Exemplars (conditional).** Opaque Arrow IPC remains opaque. The conversion applies only to control headers/terminals and must
not decode semantic relations into a second DTO family.

### WP59 — Adopt a long-lived supervised Pyrefly workspace/context lifecycle

**Outcome.** One contained Pyrefly sidecar per compatible workspace/context retains one native query/state across
generations, applies add/change batches, and emits relation-scoped Arrow IPC through provider jobs.
Normal run cancellation preserves a healthy context. Context drift reconstructs deliberately;
crash, corruption, trust loss, or unresponsive cancellation emits an explicit capability gap and
performs a bounded, joined restart from immutable inputs. Repeated changes stay within a frozen
memory/resource envelope.

**Dependencies.** WP60.

**Target invariants.** I-60, I-62--I-64, I-67--I-69, and I-71.

**Design and library references.** Accepted design §§3.4--3.5, 3.8, 6.2 step 4, and 7.3--7.5; LD-43--LD-44; GEN §7 and
AC-G-30/32/35/36; LIFE §§1, 3, and 10; doctrine P3--P5, P9--P12, P16--P20, P23--P25,
P27--P30, P32--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/pyrefly_service.rs src/provider_sandbox.rs pyrefly-sidecar/src/server.rs src/supervisor.rs src/daemon.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'DisposablePyreflySidecarProcess|change_files|add_files|Shutdown|terminate|PyreflyRunGap|CompiledProviderAuthority' src pyrefly-sidecar tests rules tooling/ci justfile
just sidecar-ci-fast
```

Known touch: `src/pyrefly_service.rs`, `src/provider_sandbox.rs`, `src/supervisor.rs`, `src/daemon.rs`,
`pyrefly-sidecar/src/server.rs`, sidecar protocol and tests,
relation-IPC modules, `rules/no-pyrefly-public-api.yml` and its fixtures, Cargo manifests/locks,
tooling, and `justfile`.

**Required changes.**

1. Replace `DisposablePyreflySidecarProcess` and terminate-after-every-run with a supervisor-owned
   workspace/context process whose lifecycle is spawn, exact validation, initialize, add/analyze,
   change/analyze, drain, and join.
2. Define exact context compatibility from interpreter/typeshed/search path/config/protocol/build
   identities. A mismatch reconstructs rather than mutating incompatible state.
3. Route source generations through provider jobs and preserve the sidecar's provider-reported
   affected scope. Admission still validates every Arrow relation, pin, coverage trailer, terminal,
   and bound.
4. Drive cooperative run cancel through the provider job's bounded synchronous cancellation probe.
   Escalate to process-group termination only after the bounded acknowledgment deadline; implement
   a real shutdown signal so local service drain stops and joins the serving loop. WP56 later
   bridges the daemon-rooted structured token tree into this already-proved provider contract.
5. Preserve syntax capability when Pyrefly is degraded. Publish only a later proved epoch after
   clean reconstruction; never reuse possibly stale semantic output.
6. Add `pyrefly-incremental-lifecycle-check` covering two compatible generations in one process,
   affected scope, cancellation survival, context replacement, forced crash/gap/reconstruction,
   bounded repeated-change memory, drain, and zero live processes.

**Legacy disposition and decommission.** Delete disposable ownership, per-run unconditional termination, marker/profile inputs, and tests
whose contract is disposal. Residual shared definitions and stale Python generated bindings enter
DB19/DB20/DB22. The pinned sidecar Cargo root and provider protocol remain.

**Acceptance checks.**

##### Behavioral

- `just pyrefly-incremental-lifecycle-check` proves same-context state reuse and clean-equivalent
  results across two generations.

##### Structural

- `just provider-type-boundary-check` and the Pyrefly boundary rules reject provider-native state
  outside the sidecar/adapter.

##### Negative / Zero-State

- `just pyrefly-incremental-lifecycle-check` faults context drift, cancellation, crash, corrupt IPC,
  trust loss, memory bound, and unjoined shutdown.

##### Operational

- `just sidecar-ci-fast` and `just relation-ipc-provider-operations-check` prove the auxiliary
  domain and bounded process/flow-control contract.

Oracle catalog:

Executable oracle: `pyrefly_native_state_containment_integrity`
Governed criterion: `PC-WP59-INT`

Executable oracle: `pyrefly_same_context_incremental_semantics`
Governed criterion: `PC-WP59-BEH`

Executable oracle: `pyrefly_context_trust_and_memory_faults`
Governed criterion: `PC-WP59-NEG`

Executable oracle: `pyrefly_cooperative_drain_reconstruction`
Governed criterion: `PC-WP59-OPS`

**Edit-Local Gates.** Focused stable-root and sidecar lifecycle tests, `just sidecar-check`, `just root-check-fast`, and
relation-IPC/cancellation checks after each ownership transition.

**Packet-Local Gates.** `just pyrefly-incremental-lifecycle-check`; `just provider-type-boundary-check`;
`just provider-ipc-contract-integrity-check`; `just relation-ipc-provider-operations-check`;
`just provider-trust-coverage-remainder-check`; `just provider-job-contract-check`;
`just sidecar-ci-fast`.

**Integration Milestone.** Completes M14 with WP57--WP58.

**Replan Triggers.** Replan if context compatibility cannot be made exact, the sidecar cannot cooperatively drain/join,
incremental state exceeds the frozen workspace envelope, a shared multi-workspace process becomes
necessary, or correctness requires Python/native state outside the adapter.

**Rollback or Recovery.** Before runtime cutover, revert to the last coherent provider API for development only; do not retain
a production disposable fallback. Runtime crash recovery is explicit gap plus bounded clean
reconstruction from immutable inputs.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
compatible context: reuse one process and native query state
incompatible context: drain/join -> reconstruct from immutable inputs
run cancel: preserve healthy context
crash/trust/corruption: explicit gap -> terminate group -> reconstruct
```

### WP60 — Compile the behavior-bearing semantic release and program seam

**Outcome.** One fallible release compiler constructs a private, immutable v2.3 `CompiledSemanticRelease` with
behavior-bearing provider, transformation, query, proof, and policy programs. The release prepares
provider jobs, admits results, compiles native fabric/query behavior, constructs independent proof
inputs/faults, and reduces policy. Release-specific proof/closure/query meaning moves out of the
generic engine in this same packet. Default-composition tests causally execute the compiler through
fixture jobs and synthetic Arrow/IPC results without selecting it in production. Final
`semantic-release` feature activation and isolation proof wait until WP54 has removed the current
outward feature edges.

**Dependencies.** WP53.

**Target invariants.** I-60--I-65, I-67, I-69, and I-71.

**Design and library references.** Accepted design §§3.3--3.6, 3.10, 6.2 steps 3 and 5, and 7.2--7.3; LD-40--LD-44; SUITE
§§2, 5--8, and 11; GEN §§5, 90, and release obligations; FAB §§6--8 and 13; QRY §6;
doctrine P1--P5, P7--P16, P18--P21, P25--P33, P35--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/fabric/production_kernel.rs src/production_provider_recipe.rs src/production_query_recipe.rs src/fabric/proof.rs src/fabric/derived_producer_closure.rs src/programmatic_derived_analysis.rs --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'CompiledSemanticRelease|CompiledProviderAuthority|CompiledTransformationAuthority|CompiledQueryAuthority|CompiledProofAuthority|CompiledPolicyAuthority|COMPILED_SUITE_VERSION|2[.]2[.]0|current\(\)' src tests rules tooling/ci justfile
just semantic-request-program-check
```

Known touch: `src/fabric/production_kernel.rs`, `src/production_provider_recipe.rs`,
`src/production_query_recipe.rs`, `src/fabric/proof.rs`,
`src/fabric/derived_producer_closure.rs`, `src/fabric/programmatic_ingress_port.rs`,
`src/fabric/programmatic_query_backend.rs`, `src/programmatic_derived_analysis.rs`,
`src/relational_semantic_query.rs`, `src/provider_admission.rs`, Cargo feature metadata, tests,
rules/tooling, and `justfile`.

**Required changes.**

1. Define one closed Rust release definition and a fallible compiler that validates categorical
   identities, the program graph, relation/field/schema/dependency coverage, provenance closure, and
   policy coherence before producing an immutable release.
2. Replace five zero-sized marker authorities with private behavior-bearing program values. Every
   program operand is causally read; removing or mutating an operand changes or rejects execution.
3. Declare the suite identity once as categorical v2.3 data and structurally bind subprogram
   identities. Derive epoch/status/reference observations from the compiled product; do not
   duplicate suite strings or add a semantic digest.
4. Move release-specific provider descriptors/admission meaning, transformation programs, query
   forms, proof expectations/faults, and policy ceilings into their program owners. Generic fabric
   evaluators accept typed programs and know no release constants.
5. Make release-owned preparation the only target constructor for provider jobs and release-owned
   admission the only target conversion into candidate epoch inputs. Before WP57--WP59 migrate
   production call sites, exercise this authority with explicit test-only fixture definitions,
   fixture jobs, and synthetic Arrow/IPC results.
6. Establish the target release module/program boundary inside the existing default composition so
   downstream provider and fabric packets can consume it. Do not claim or activate the final
   `semantic-release` feature while `fact-generation` or `data-fabric` still has a known outward
   edge; WP54 owns that resolved-feature activation after those dependencies are closed.
7. Add `release-program-contract-check` and `compiled-suite-identity-check`; extend
   `provider-job-contract-check` to prove release-owned preparation/admission causality. Each uses
   independently authored operand faults, not self-generated expectations.

**Legacy disposition and decommission.** The new release is executable but not production-selected until WP61. Unit marker types, global
`current()` lookup, stale constants, mixed profiles, and interim proof/closure cfg gates enter DB19.
No wrapper converts a new program back to an old token and no second production selector exists.

**Acceptance checks.**

##### Behavioral

- `just release-program-contract-check` mutates/removes each program operand and observes the
  dependent behavior or compile rejection.

##### Structural

- `just feature-architecture-check release-compiler` proves one closed release definition and no
  direct repository, operational, RPC, supervisor, or generated-transport dependency. It does not
  claim the final resolved `semantic-release` feature graph.

##### Negative / Zero-State

- `just provider-job-contract-check` rejects forged/non-release jobs and invalid result admission;
  `just compiled-suite-identity-check` rejects categorical conflation and stale v2.2 live identity.

##### Operational

- `just semantic-request-program-check` executes all eight query forms through the compiled program
  and fixture-backed current fabric evaluator with bounded streaming behavior.

Oracle catalog:

Executable oracle: `compiled_release_program_identity_integrity`
Governed criterion: `PC-WP60-INT`

Executable oracle: `compiled_release_operand_causality`
Governed criterion: `PC-WP60-BEH`

Executable oracle: `compiled_release_forgery_and_conflation_faults`
Governed criterion: `PC-WP60-NEG`

Executable oracle: `compiled_release_query_program_operations`
Governed criterion: `PC-WP60-OPS`

**Edit-Local Gates.** Focused release/program/proof tests, `just root-fmt`, `just root-check-fast`, and the three new
recipes after each program-owner migration.

**Packet-Local Gates.** `just release-program-contract-check`; `just compiled-suite-identity-check`;
`just provider-job-contract-check`; `just semantic-request-program-check`;
`just feature-architecture-check release-compiler`;
`just stable-graph-check`.

**Integration Milestone.** Completes M13 with WP53.

**Replan Triggers.** Reopen the design if multiple suites must coexist/hot-swap, one program cannot be causally exercised
without a second authority, suite v2.3 cannot express a load-bearing contract, or same-package
visibility/features cannot keep the release inward and transport/state independent.

**Rollback or Recovery.** The production route is unchanged in this packet. A compile failure prevents the release value from
existing. Revert the release/program transaction as a unit; never fall back from a failed target
compile to an old token or partially compiled program set.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
CompiledSemanticRelease
  suite: SuiteIdentity
  providers: CompiledProviderProgram
  transformations: CompiledTransformationProgram
  queries: CompiledQueryProgram
  proof: CompiledProofProgram
  policy: CompiledPolicyProgram
```

Fields remain private; behavior, not possession, is the authority.

### WP61 — Inject one release into application services and thin Tonic adapters

**Outcome.** Daemon startup compiles one `Arc<CompiledSemanticRelease>` and injects that exact value into
workspace startup/update, provider admission, query, status, reference, and recovery application
services. One atomic composition change makes this the sole production route. Tonic handlers own
transport validation, peer/session authorization, deadlines, message bounds, conversion, and status
mapping only; application services own accepted work and semantic outcomes. Old global/token code
may remain physically present until WP62, but no production selector can reach it.

**Dependencies.** WP56 and WP60.

**Target invariants.** I-60--I-71.

**Design and library references.** Accepted design §§3.1, 3.3, 3.8--3.9, 6.2 steps 7--8, and 7.3--7.4; LD-43; SUITE §§2,
7--8, and 11; LIFE §§1, 3, 10, and 13; QRY §§6 and 9; SRV §§5--8; doctrine P3--P5,
P9--P13, P16, P20, P22--P25, P27, P32--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline src/daemon.rs src/fabric/production_workspace_startup.rs src/query_service.rs src/session_authority.rs src/rpc.rs src/supervisor.rs src/bin --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'CompiledSemanticRelease::current\(\)|ProductionQueryService|CpgQueryService|tokio::spawn|relation_ipc_proto_types|DaemonPort|release\(' src tests contracts/rpc codefabric-cpg-mcp tooling/proto rules justfile
just proto-check
```

Known touch: `src/daemon.rs`, `src/fabric/production_kernel.rs`,
`src/fabric/production_workspace_startup.rs`, `src/fabric/programmatic_query_backend.rs`,
`src/query_service.rs`, `src/session_authority.rs`, `src/rpc.rs`, `src/supervisor.rs`,
`src/bin/codefabric.rs`, `src/bin/codefabricd.rs`, `tests/integration/daemon.rs`,
`tests/integration/rpc.rs`, `contracts/rpc/`, `tooling/proto/`,
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py`, adapter tests, rules/tooling, and
`justfile`.

**Required changes.**

1. Compile and validate one release before workspace readiness. Store one shared immutable `Arc` in
   the daemon application composition and pass it explicitly to every semantic service; no service
   performs a global lookup or reconstructs a release.
2. Define application-owned startup/update/query/status/reference service inputs and outcomes.
   Provider runs are prepared/admitted through the injected release; fabric and operational ports
   remain explicit dependencies.
3. Split combined `ProductionQueryService` concerns so the Tonic implementation validates and
   converts generated messages, calls the application service, and maps outcomes back. Generated
   values cannot enter application DTO fields or persistence.
4. Preserve the released four-family proto topology, descriptor/code generation, private UDS peer
   credentials, accepted handles, watch/resume, explicit cancel, resource handles, bounded reads,
   launch grants, and one Python CPG query client. Do not restore Python provider-process bindings.
5. Preserve direct/bounded stream composition and reserved control capacity. Add
   `grpc-flow-control-contract-check` for bounded channels, ordered frames, prompt control calls,
   watch-drop semantics, and component-level cancellation without claiming real slow-consumer RSS.
6. Use the WP56 task tree for handler/application/provider/execution work and drain. No Tonic stream
   owns durable logical cancellation.
7. Switch every production consumer in one dependency-closed commit. A mixed injected/global route
   or a service-by-service production transition is forbidden.
8. Re-execute the valid v5 WP45--WP47 wire/FastMCP behaviors against the new application service;
   their old proving commits are not current runtime proof.

**Legacy disposition and decommission.** This is the live cutover for DB19--DB21. Old tokens, global lookups, combined handler logic, reverse
imports, and stale tests become unreachable here and are physically deleted in WP62. No fallback
selector, alternate constructor, environment switch, or compatibility service is retained.

**Acceptance checks.**

##### Behavioral

- `just compiled-release-consumer-cutover-check` and
  `just programmatic-production-composition-check` prove every live consumer uses the same injected
  release through real provider/fabric behavior.

##### Structural

- `just generated-type-boundary-check` proves generated types terminate at provider/transport
  adapters; `just feature-architecture-check daemon` proves daemon is the sole union.

##### Negative / Zero-State

- `just release-program-contract-check` rejects a mismatched/missing release and
  `just grpc-flow-control-contract-check` rejects unbounded/detached/conflated transport behavior.

##### Operational

- `just fastmcp4-daemon-wire-contract-check`, `just proto-check`, and
  `just public-lifecycle-wire-contract-integrity-check` prove the released real UDS boundary.

Oracle catalog:

Executable oracle: `injected_release_runtime_boundary_integrity`
Governed criterion: `PC-WP61-INT`

Executable oracle: `single_release_consumer_composition`
Governed criterion: `PC-WP61-BEH`

Executable oracle: `grpc_flow_control_and_release_mismatch_faults`
Governed criterion: `PC-WP61-NEG`

Executable oracle: `released_uds_transport_operations`
Governed criterion: `PC-WP61-OPS`

**Edit-Local Gates.** Focused application-service/Tonic tests, `just root-check-fast`, `just proto-check`, cancellation
tests, and adapter client tests after each conversion boundary.

**Packet-Local Gates.** `just compiled-release-consumer-cutover-check`;
`just programmatic-production-composition-check`; `just release-program-contract-check`;
`just generated-type-boundary-check`; `just grpc-flow-control-contract-check`;
`just cancellation-tree-check`; `just fastmcp4-daemon-wire-contract-check`;
`just proto-check`; `just proto-repro-check`; `just feature-architecture-check daemon`.

**Integration Milestone.** Completes M15 with WP54--WP56.

**Replan Triggers.** Replan if all consumers cannot switch atomically, a missing control field requires a released wire
change, an application service requires generated/provider-native types, transport backpressure
cannot preserve control responsiveness, or a second live release selector is required.

**Rollback or Recovery.** Before any target-format durable mutation, revert the entire composition commit to the last green
target packet. After target-format mutation, repair forward through the injected release and sole
command path. Never revive a mixed or global route.

**Design-Bearing Contracts and Exemplars (conditional).** ```text
Tonic handler
  -> validate transport/peer/session/bounds/deadline
  -> convert to application control value
  -> call injected-release application service
  -> map application outcome to generated response/status
```

### WP62 — Physically purge the displaced architecture and stale live evidence

**Outcome.** Every production and live-tooling remnant of the marker/global/profile architecture, provider or
generated type leakage, disposable Pyrefly lifecycle, reverse feature/state ownership, interim cfg
workaround, and superseded v3/v4/v5 evidence/gate/package route is absent. The purged repository
retains only target consumers, immutable historical artifacts, the four canonical proto sources,
justified generated products, two thin production binaries, and target-only oracles. Retained
behavior passes after deletion.

**Dependencies.** WP61.

**Target invariants.** I-60, I-64, I-67--I-71.

**Design and library references.** Accepted design §§6.1, 6.3--6.4, and 7.1--7.2; all material-surface dispositions; RM §4;
doctrine P3, P5, P18, P25--P31, P35--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' -g '!docs/authoritative_design/**' -g '!docs/designs/**' -g '!docs/plans/**' -g '!docs/reviews/**' 'CompiledProviderAuthority|CompiledTransformationAuthority|CompiledQueryAuthority|CompiledProofAuthority|CompiledPolicyAuthority|CompiledSemanticRelease::current|CompiledProviderExecutionProfile|CompiledProviderLane|DisposablePyreflySidecarProcess|RFV5_|2[.]2[.]0|production_evidence|successor_evidence|successor_certification|repository-state' .
ast-grep outline src src/bin codefabric-cpg-mcp/src rules tooling/ci scripts --view signatures
rg --files src/bin codefabric-cpg-mcp/src contracts/rpc tooling/proto rules rule-tests scripts tooling/ci .github | sort
just remaining-legacy-zero-state-check
```

Known touch: All current marker/profile consumer files identified in WP53--WP61; `Cargo.toml`, `Cargo.lock`,
`src/lib.rs`, `src/fabric.rs`, `src/bin/`, Python adapter source/tests/lock/wheel resources,
`contracts/rpc/`, `tooling/proto/`, `rules/`, `rule-tests/`, `scripts/`, `tooling/ci/`,
`.github/workflows/ci.yml`, `sgconfig.yml`, `justfile`, `AGENTS.md`, and `README.md`.

**Required changes.**

1. Delete the five unit authority structs, every `CompiledSemanticRelease::current()` call, inert
   authority parameter, mixed provider lane/profile type, duplicated live suite v2.2/provider/query
   literal, and the interim daemon gates on generic fabric behavior.
2. Delete provider-native public signatures, generated rustc messages in admitted DTOs,
   marker-taking adapter constructors, disposable/terminate-per-run Pyrefly ownership, unowned
   spawn/abort-drop paths, and generated Tonic types outside approved adapters.
3. Delete `data-fabric -> repository-state`, the old aggregate feature, reverse gix/SQLite/fabric
   imports, obsolete dependency activations, and any hidden feature alias or default-only bypass.
4. Preserve target-neutral dirty WP49 deletions: duplicate Python identity, unused Python provider
   bindings, predecessor issuance/certification scripts, old production-evidence modules, the v3
   disposition ledger, stale comparator assets, and obsolete recipes are not restored.
5. Replace `RFV5_*`, v5-plan-bound selectors, gate-census entries, and current-tooling imports with
   target v7 behavior or remove them. Immutable v5 plans/states/proving commits remain history but
   are not selectable by release gates.
6. Preserve independently authored FastMCP expectations only when their semantic clauses remain
   valid; bind and execute them anew against v7. Do not treat a v5 evidence result as v7 proof.
7. Adjudicate the dirty removal of `FASTMCP_MCP_CAMELCASE_COMPAT=false` by live modern protocol
   behavior. Retain no compatibility bridge; keep only configuration still required to enforce the
   bridge-off target.
8. Retain all four `.proto` authorities and generated Rust process/control families. Retain only the
   Python CPG query generated family packaged by FastMCP. Keep `src/bin/codefabric.rs` and
   `src/bin/codefabricd.rs` thin; delete any semantic settings/schema/provider profile from them.
9. Add `compiled-release-legacy-zero-state-check` with structural rules, textual coverage,
   compiler/feature builds, package/generated inventories, and a seeded fault for every legacy
   class. The check derives its live candidate set and reports skipped/unparsed scope.
10. Update operational documentation to the actual target. Historical designs/reviews/plans are
    neither edited nor searched as live runtime authority.

**Legacy disposition and decommission.** Completes DB19--DB22. DB23 remains deliberately open until WP64 proves FreshActivation and the
read-only deployment census. A real target consumer of a deletion stops the packet and is moved to
the target boundary; it never justifies wholesale predecessor restoration.

**Acceptance checks.**

##### Behavioral

- `just fastmcp4-retained-target-behavior-check` and
  `just compiled-release-consumer-cutover-check` rerun representative target behavior after purge.

##### Structural

- `just feature-architecture-check`, `just provider-type-boundary-check`, and
  `just generated-type-boundary-check` prove the final live dependency/type boundaries.

##### Negative / Zero-State

- `just compiled-release-legacy-zero-state-check`,
  `just fastmcp4-decommission-zero-state-check`, and
  `just remaining-legacy-zero-state-check` reject every seeded legacy class.

##### Operational

- `just fastmcp4-package-build-check`, `just features-each`, and
  `just stable-graph-check` build/inventory the retained target graph and packages.

Oracle catalog:

Executable oracle: `purged_target_feature_and_type_integrity`
Governed criterion: `PC-WP62-INT`

Executable oracle: `post_purge_target_behavior`
Governed criterion: `PC-WP62-BEH`

Executable oracle: `compiled_release_legacy_zero_state`
Governed criterion: `PC-WP62-NEG`

Executable oracle: `post_purge_package_feature_operations`
Governed criterion: `PC-WP62-OPS`

**Edit-Local Gates.** Targeted structural/text searches, rule fixtures, dependency/lock/package inspection, focused
target behavior, and `just root-check`/`just adapter-ci-fast` after each deletion cluster.

**Packet-Local Gates.** `just compiled-release-legacy-zero-state-check`; `just feature-architecture-check`;
`just provider-type-boundary-check`; `just generated-type-boundary-check`;
`just fastmcp4-post-purge-surface-check`; `just fastmcp4-retained-target-behavior-check`;
`just fastmcp4-decommission-zero-state-check`; `just remaining-legacy-zero-state-check`;
`just fastmcp4-package-build-check`; `just features-each`; `just stable-graph-check`.

**Integration Milestone.** Completes M16's sole-live-target boundary; WP63 closes correctness/recovery evidence.

**Replan Triggers.** Replan if a deletion candidate has a genuine target consumer, a historical file remains generated/
packaged/selectable, a zero-state coverage envelope skips a live scope, one-package enforcement
repeatedly permits reverse imports, or removal requires changing a released public contract.

**Rollback or Recovery.** Restore only a specific target consumer proven necessary, then redesign its ownership before
continuing. Do not restore a predecessor subsystem, feature, generated family, or evidence suite.
After package/lock changes, repair forward and prove a clean install.

**Design-Bearing Contracts and Exemplars (conditional).** None. This packet deletes displaced representations and proves the remaining target; it creates no
new semantic authority.

### WP63 — Prove the real installed target correctness and recovery vertical

**Outcome.** From real source change through all four providers, release admission, DataFusion proof, exact Delta
publication/activation, injected application services, generated Tonic over private UDS, and an
installed FastMCP 4 wheel, the target returns independently expected bounded results exactly once.
Provider failure, Pyrefly crash/rebuild, rustc compile gap, cancellation at every phase, slow
consumer, daemon restart, reconnect, and drain all preserve exact authority and leave zero owned
tasks/processes. Correctness and recovery are established before performance tuning.

**Dependencies.** WP62.

**Target invariants.** I-61--I-71.

**Design and library references.** Accepted design §§7.3--7.4 and 6.5; SUITE §§5--8 and 11; GEN release obligations; FAB
§§11, 13--14; LIFE §§10 and 13; SRV §§5--8; QRY §§6 and 9; doctrine P9--P13,
P16--P20, P23--P25, P27--P30, P33--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline tests/integration src/daemon.rs src/query_service.rs src/supervisor.rs codefabric-cpg-mcp/tests tooling/ci --view signatures
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'slow.consumer|restart|reconstruct|crash|cancel|resource|installed|stdio|two.agent|provider gap' tests codefabric-cpg-mcp tooling/ci scripts justfile
just fastmcp4-stdio-vertical-check
```

Known touch: `tests/integration/daemon.rs`, `tests/integration/rpc.rs`, provider/fabric/activation integration
modules, `codefabric-cpg-mcp/tests/test_stdio.py`, `test_server.py`, `test_daemon_client.py`,
`tooling/fastmcp4_modern_client_driver.py`, `scripts/run_fastmcp4_modern_client.sh`, real-process
fixtures/tooling, rules, and `justfile`.

**Required changes.**

1. Add `semantic-release-vertical-check` around the installed production topology and independently
   authored semantic expectations. The expected clauses predate execution and do not import
   production compilers/providers/comparators.
2. Exercise one source mutation whose dependent result changes and unrelated result does not; prove
   all provider pins, coverage/gaps, proof, exact Delta vector, activation, daemon response, and
   presentation projection refer to the same epoch.
3. Fault each provider lane, malformed/oversize Arrow/IPC, false empty completion, proof rejection,
   partial Delta publication, unknown commit result, cancellation at every stage, daemon loss,
   generation change, reconnect, and resource lease retention/release.
4. Add a real `grpc-slow-consumer-check`: generated clients over UDS hold an event/resource stream
   unread while measuring bounded queue depth and RSS, ordered sequence, reserved control RPC
   response, cancellation acknowledgement, one terminal, lease retention, and the distinction
   between watch drop and logical cancel.
5. Add `semantic-release-restart-reconstruction-check`: discard process-local releases, sessions,
   provider caches, Pyrefly process state, tokens, channels, handles, and cursors; recompile the
   release, reopen the exact activation vector, reconcile, fail closed on release/vector mismatch,
   and only then reopen admission.
6. Re-run the complete modern FastMCP wire/resource/security/guard/completion/cancellation/reconnect/
   public-surface suite, plus wheel and STDIO installation. Preserve one daemon per workspace and
   one channel/server per adapter.
7. Re-prove DB19--DB22 remain closed under real installed execution and clean reconstruction.

**Legacy disposition and decommission.** No predecessor implementation, old package, v5 evidence result, mock-only launch, or self-generated
expectation enters the verdict. Immutable historical expectations may be reused only as reviewed
clauses executed anew against v7.

**Acceptance checks.**

##### Behavioral

- `just semantic-release-vertical-check` proves the real source-to-FastMCP result and causal source
  mutation.

##### Structural

- `just compiled-release-legacy-zero-state-check` and
  `just fastmcp4-adapter-authority-zero-state-check` prove installed topology cannot select legacy
  or Python semantic authority.

##### Negative / Zero-State

- `just fastmcp4-security-negative-check`, `just provider-admission-exclusivity-check`, and
  `just semantic-release-restart-reconstruction-check` exercise boundary/recovery faults.

##### Operational

- `just grpc-slow-consumer-check`, `just fastmcp4-cancellation-recovery-check`, and
  `just adapter-wheel-test`/`just adapter-stdio-test` prove real process/resource behavior.

Oracle catalog:

Executable oracle: `installed_target_authority_integrity`
Governed criterion: `PC-WP63-INT`

Executable oracle: `real_source_to_fastmcp_causal_vertical`
Governed criterion: `PC-WP63-BEH`

Executable oracle: `installed_vertical_fault_and_recovery_matrix`
Governed criterion: `PC-WP63-NEG`

Executable oracle: `slow_consumer_cancel_restart_operations`
Governed criterion: `PC-WP63-OPS`

**Edit-Local Gates.** Focused real-process fixture tests and the owning packet's oracle after any discovered repair. A
semantic repair returns to WP53--WP61; it is not hidden in the vertical harness.

**Packet-Local Gates.** `just semantic-release-vertical-check`; `just semantic-release-restart-reconstruction-check`;
`just grpc-slow-consumer-check`; `just provider-trust-coverage-remainder-check`;
`just datafusion-plan-schema-cache-check`; `just delta-publication-contract-check`;
`just cancellation-tree-check`; all retained `fastmcp4-*` wire/resource/security/guard/completion/
cancellation/public-surface recipes; `just adapter-ci-fast`; `just adapter-wheel-test`;
`just adapter-stdio-test`.

**Integration Milestone.** Completes M16 and freezes the correctness/recovery candidate for WP64.

**Replan Triggers.** Replan if the real topology must be replaced by a fake, slow-consumer bounds require unbounded
buffering or transport redesign, clean restart needs predecessor/process-local state, an independent
expectation cannot discriminate the target, or any semantic fix changes an accepted boundary.

**Rollback or Recovery.** A failed vertical leaves WP63 incomplete. Repair through the owning packet, create a new target
candidate, and rerun every dependent oracle. Never accept a partial vertical or make the fixture
construct hidden production state.

**Design-Bearing Contracts and Exemplars (conditional).** None. The vertical must use the same production binaries, supervisor, UDS, generated clients,
installed wheel, provider paths, and exact durable state as users.

### WP64 — Execute FreshActivation and remove dormant predecessor authority

**Outcome.** From an empty supported workspace with no predecessor head, model, generated registry, cache,
handoff, or cutover controller, the purged target creates, reads back, activates, serves, restarts,
and forward-repairs one exact v2.3 authority. A read-only deployment census proves whether a real
predecessor exists. If none exists, dormant handoff/cutover roles, branches, recipes, fixtures, and
services reach physical zero state while exact target activation/reconciliation/writer/lease
primitives remain.

**Dependencies.** WP63.

**Target invariants.** I-61, I-66--I-71.

**Design and library references.** Accepted design §§6.5, 7.3, and 9; SUITE §§7 and 11; FAB §§11 and 14; LIFE §13; SRV
§§5--8; RM §4; doctrine P3, P9--P13, P16--P20, P23--P30, P32--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'FreshActivation|AuthorityHandoff|switchable_activation_authority|cutover|handoff|ExpectedHead::Empty|unknown.*outcome|forward repair|predecessor' src tests contracts tooling/ci scripts justfile .github
ast-grep outline src/fabric/activation.rs src/fabric/production_workspace_startup.rs src/fabric/switchable_activation_authority.rs src/daemon.rs tests/integration --view signatures
just fabric-activation-recovery-check
```

Known touch: `src/fabric/activation.rs`, `src/fabric/production_workspace_startup.rs`,
`src/fabric/switchable_activation_authority.rs`, activation/command/reconciliation/lease modules,
`src/daemon.rs`, supervisor/lifecycle integration tests, zero-state tooling/rules, `justfile`, and
deployment/service configuration.

**Required changes.**

1. Provision an empty bounded workspace/runtime/storage root and start through the same supervisor,
   binaries, and installed FastMCP package as users.
2. Submit lawful genesis through the sole `FabricCommand` with `ExpectedHead::Empty`; execute release
   compile, providers, transformations, proof, exact Delta writes, activation append/readback,
   epoch reconstruction, ready admission, and modern query/resource behavior.
3. Restart every process after discarding all process-local state. Reopen exactly the activation-
   selected versions and prove identical decoded authority; no candidate, receipt, digest, latest
   lookup, model replay, or old schema may substitute.
4. Inject failure before append, acknowledged append, unknown append outcome, delayed/unavailable
   readback, competing writer, incoherent horizon, process loss, generation change, reconnect, and
   forward repair. Reconcile exact state or remain failed closed; never blindly retry.
5. Perform a read-only census for a real deployed predecessor. If one exists, stop and reopen design
   for a one-shot `AuthorityHandoff`. If none exists, delete dormant handoff/cutover controllers,
   roles, states, features, recipes, services, fixtures, and recovery branches, then rerun the full
   FreshActivation case.
6. Add `compiled-release-fresh-activation-check` as the target-only real-process aggregate and close
   DB23 with structural/textual/compiler/package zero-state faults.

**Legacy disposition and decommission.** Completes DB23 only after the deployment census and target FreshActivation pass. Historical
descriptions remain immutable. Exact activation, writer fencing, readback/reconciliation, and lease
retention are target primitives and are never deleted as “cutover code.”

**Acceptance checks.**

##### Behavioral

- `just compiled-release-fresh-activation-check` proves empty-to-serving behavior and exact restart.

##### Structural

- `just compiled-release-legacy-zero-state-check` proves dormant predecessor/handoff reachability
  is absent after the census-approved deletion.

##### Negative / Zero-State

- `just activation-receipt-nonauthority-check` and
  `just predecessor-restart-revocation-check` reject seed/model/cache/hash/latest/predecessor routes.

##### Operational

- `just fabric-activation-recovery-check`, `just unknown-cutover-reconciliation-check`, and
  `just candidate-free-recovery-check` prove every append/readback/restart/repair edge.

Oracle catalog:

Executable oracle: `fresh_activation_sole_authority_integrity`
Governed criterion: `PC-WP64-INT`

Executable oracle: `empty_root_fresh_activation_semantics`
Governed criterion: `PC-WP64-BEH`

Executable oracle: `predecessor_seed_hash_latest_route_faults`
Governed criterion: `PC-WP64-NEG`

Executable oracle: `fresh_activation_reconciliation_operations`
Governed criterion: `PC-WP64-OPS`

**Edit-Local Gates.** Focused activation/lifecycle/Delta/supervisor/session/resource tests and the FreshActivation recipe
after each recovery or DB23 deletion cluster.

**Packet-Local Gates.** `just compiled-release-fresh-activation-check`; `just fabric-activation-recovery-check`;
`just fabric-control-recovery-check`; `just unknown-cutover-reconciliation-check`;
`just candidate-free-recovery-check`; `just fabric-epoch-pinning-check`;
`just activation-receipt-nonauthority-check`; `just predecessor-restart-revocation-check`;
`just compiled-release-legacy-zero-state-check`.

**Integration Milestone.** Advances M17 and closes DB23. WP65 measures this final topology; WP66 certifies it.

**Replan Triggers.** Stop and reopen design if the census discovers a real predecessor, exact coherent readback cannot
determine authority, supported storage cannot express empty-head append/reconciliation, or forward
repair would require predecessor restoration.

**Rollback or Recovery.** Before append, discard the private candidate and keep admission closed. After an unknown result,
reconcile exact state. After target mutation, repair forward. Do not delete durable target state or
restore predecessor machinery to make a test appear fresh.

**Design-Bearing Contracts and Exemplars (conditional).** No generic handoff abstraction is introduced. Discovery of a deployed predecessor is a design
reopen trigger, not a dormant feature.

### WP65 — Measure the final target resource and performance envelope

**Outcome.** The correct, purged, FreshActivation-proved target is measured against pre-registered explicit
workloads and resource bounds. Measurements separately cover release compilation/reconstruction,
provider cold/incremental behavior, long-lived Pyrefly memory, DataFusion planning/first-batch/full
stream/spill/cancel, exact Delta commit/readback/reopen/activation, gRPC event/resource/slow-consumer/
cancel behavior, and one-to-maximum admitted queries. No legacy implementation or v5 evidence is a
performance or correctness baseline. Any optimization preserves semantic equality and returns to
its owning packet for proof.

**Dependencies.** WP64.

**Target invariants.** I-63--I-67, I-71, and every resource/cancellation bound attached to I-62 and I-68.

**Design and library references.** Accepted design §7.5 and performance reopen triggers in §9; LD-40--LD-44; GEN provider resource
obligations; FAB §§8, 13--14; LIFE §10; SRV §8; doctrine P6, P8, P14--P20, P23--P25,
P28--P30, P33, P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'performance|benchmark|latency|throughput|RSS|memory|first.batch|slow.consumer|spill|incremental|resource envelope' tooling/ci scripts tests codefabric-cpg-mcp justfile .github
ast-grep outline tooling/ci/fastmcp4_release_performance.py tooling/ci/test_fastmcp4_release_performance.py tests/integration --view signatures
just semantic-release-vertical-check
```

Known touch: `tooling/ci/fastmcp4_release_performance.py`, its tests, real-workload fixtures, resource samplers,
provider/DataFusion/Delta/daemon integration harnesses, `scripts/`, opt-in CI configuration,
`justfile`, and only measured target code returned to an owning packet.

**Required changes.**

1. Before the first measurement, freeze candidate-neutral workloads, data scale, sample counts,
   cold/warm classification, concurrency levels, resource ceilings, cancellation deadlines,
   environment capture, distributions/uncertainty, and pass/fail bounds. A failed candidate cannot
   edit its own bound.
2. Measure cold daemon and release compile, empty/retained workspace reconstruction, Tree-sitter/
   Ruff cold and incremental jobs, Pyrefly initial load plus repeated `change_files`, rustc process
   lifecycle, provider row/byte throughput, and peak bounded memory.
3. Measure DataFusion planning, first batch, full stream, spill/resource governance, and cancellation;
   exact Delta write/readback/reopen/activation; and gRPC first event, bounded slow-consumer RSS/
   queue, resource throughput, control latency, and cancellation acknowledgement.
4. Measure one and maximum admitted queries for fairness, aggregate daemon/provider/Python memory,
   joined cleanup, and absence of per-call channels/servers/processes.
5. Compare only to explicit functional/resource requirements and, where useful for attribution, a
   minimal same-version library control with identical topology/input. Never compare semantic
   correctness to v5 or an unvalidated predecessor.
6. Prove structural resource properties independently of timing: bounded buffers/pages/rounds/
   completions/logs, one daemon/workspace, one channel/adapter, long-lived compatible Pyrefly,
   streamed DataFusion, and zero live owned tasks/processes after drain.
7. Add `compiled-release-resource-performance-check`. It preserves raw samples and a reviewed
   summary outside execution state and fails on seeded unbounded/per-call/blocking/cleanup faults.
8. If a target-owned cost misses a bound, return the change to its owning packet and rerun every
   dependent oracle through WP65. A topology, authority, security, custom-operator, unsafe, or lower
   Delta rung change reopens design.

**Legacy disposition and decommission.** No legacy package, history comparator, old lock, v5 evidence result, or benchmark-only production
path enters the verdict. The deleted benchmark comparator stays deleted. Temporary measurement
products are bounded non-authoritative evidence.

**Acceptance checks.**

##### Behavioral

- `just semantic-release-vertical-check` proves identical decoded semantics before accepting any
  measured optimization.

##### Structural

- `just feature-architecture-check` and `just cancellation-tree-check` prove bounded target
  structure independently of noisy timing.

##### Negative / Zero-State

- `just compiled-release-legacy-zero-state-check` and
  `just fastmcp4-expectation-drift-check` reject history dependence and post-result method/bound
  drift.

##### Operational

- `just compiled-release-resource-performance-check` executes the frozen workload and resource
  envelope with discriminating faults.

Oracle catalog:

Executable oracle: `resource_envelope_method_integrity`
Governed criterion: `PC-WP65-INT`

Executable oracle: `performance_optimization_semantic_equivalence`
Governed criterion: `PC-WP65-BEH`

Executable oracle: `unbounded_resource_and_method_drift_faults`
Governed criterion: `PC-WP65-NEG`

Executable oracle: `compiled_release_resource_performance_envelope`
Governed criterion: `PC-WP65-OPS`

**Edit-Local Gates.** Harness unit tests, semantic-equality controls, resource sampler checks, and the owning packet gates
for any measured target change.

**Packet-Local Gates.** `just compiled-release-resource-performance-check`; `just semantic-release-vertical-check`;
`just grpc-slow-consumer-check`; `just pyrefly-incremental-lifecycle-check`;
`just inprocess-provider-lifecycle-check`; `just datafusion-cache-resource-operations-check`;
`just delta-publication-contract-check`; `just cancellation-tree-check`;
`just fastmcp4-expectation-drift-check`; `just compiled-release-legacy-zero-state-check`.

**Integration Milestone.** Advances M17 and freezes the final measured candidate for WP66.

**Replan Triggers.** Replan if no independent functional/resource envelope can be issued, the target misses a frozen
bound after target-local optimization, resource evidence requires weaker isolation or unbounded
state, a lower library extension rung is proposed, or comparisons cannot preserve semantic equality.

**Rollback or Recovery.** Revert only candidate tuning, never evidence or semantics. Keep the correct untuned implementation
and recorded limitation when an experiment fails; topology/authority changes require design and plan
revision.

**Design-Bearing Contracts and Exemplars (conditional).** None. The measurement method is a frozen test contract, not production semantics.

### WP66 — Certify the complete successor at one trusted HEAD

**Outcome.** At one committed candidate, every v7 packet oracle, milestone, decommission batch, retained v2.3
functional outcome, isolated feature, exact provider/fabric/Delta behavior, FreshActivation case,
installed FastMCP vertical, resource/performance envelope, all four build domains, packages,
governance, policy, and an independent implementation review pass. The certification packet changes
no semantic code. Only then may governed v7 state mark the plan complete.

**Dependencies.** WP53--WP65.

**Target invariants.** I-60--I-71 and every accepted v2.3 release-readiness obligation.

**Design and library references.** Accepted design §7.6 and §10; all LD decisions; SUITE §11; GEN release obligations; FAB §14;
RM §4; full doctrine P1--P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
just plan-status
just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02.md
just artifacts-check
just oracle-substance-check
just gate-filter-census
just authoritative-design-conformance-check
```

Known touch: Only aggregate certification recipes/tooling, the independent review artifact, and governed state
updates after proving commits. Any semantic, dependency, package, or runtime repair returns to its
owning packet and creates a new candidate.

**Required changes.**

1. Freeze one candidate commit and derive the WP53--WP66 packet/oracle universe from this parsed
   plan. Do not hard-code v5 counts, selectors, statuses, or aliases that discard child failures.
2. Re-run every packet acceptance check at candidate HEAD. Derive proving-commit ancestry, input
   freshness, selector substance, state conformance, milestone closure, and decommission closure.
3. Run the complete final matrix in §7 from clean supported roots, including useful isolated
   feature behavior, all provider domains, DataFusion/Delta/recovery, real UDS/FastMCP, wheel/STDIO,
   policy/governance, FreshActivation, and frozen performance/resource evidence.
4. Re-execute valid v5 WP43--WP48 functional expectations against v7 where still applicable. Do not
   import their completion statuses, old production evidence, or proving results as v7 proof.
5. Obtain an independent implementation review against the accepted design, this plan, state
   deviations, current code/behavior, library decisions, cutover coverage, final diff, and gate
   evidence. A blocker/major finding returns to the owning packet and invalidates dependent proof.
6. Add `relational-fabric-v7-certification` as a transparent aggregate that lists every child,
   preserves every exit status, rejects empty selectors, and proves a seeded child failure makes the
   aggregate fail.
7. After all gates and the accepted independent review, commit the certification packet and use the
   governed state transaction to record its proving commit and mark packets/milestones/batches/plan
   complete. No hand-written check outputs enter state.

**Legacy disposition and decommission.** DB19--DB23 must be physically closed. V2.2/v5 and earlier artifacts remain immutable history only.
Any live dependency on marker/global release authority, provider/generated leakage, disposable
sidecar state, reverse feature/state ownership, stale proof tooling, duplicate Python authority, or
dormant predecessor handoff blocks certification.

**Acceptance checks.**

##### Behavioral

- `just relational-fabric-v7-certification` includes and preserves every behavioral child in §7.

##### Structural

- `just artifacts-check`, `just plan-status`, `just oracle-substance-check`,
  `just gate-filter-census`, and `just feature-architecture-check` prove artifact and architecture
  closure.

##### Negative / Zero-State

- `just compiled-release-legacy-zero-state-check` plus the final aggregate's seeded-child fault
  prove no legacy route or failure-masking alias survives.

##### Operational

- `just ci-pr`, `just compiled-release-fresh-activation-check`, and
  `just compiled-release-resource-performance-check` prove the supported release and environment.

Oracle catalog:

Executable oracle: `v7_certification_artifact_and_ancestry_integrity`
Governed criterion: `PC-WP66-INT`

Executable oracle: `v7_complete_successor_behavior_matrix`
Governed criterion: `PC-WP66-BEH`

Executable oracle: `v7_legacy_and_failure_masking_zero_state`
Governed criterion: `PC-WP66-NEG`

Executable oracle: `v7_terminal_release_certification`
Governed criterion: `PC-WP66-OPS`

**Edit-Local Gates.** No semantic edits are allowed. Any repair returns to the owning packet; re-freeze a new candidate
and rerun all dependents.

**Packet-Local Gates.** `just relational-fabric-v7-certification` and every command in §7, followed by an independent
implementation review on the unchanged candidate.

**Integration Milestone.** Completes M17 and the v7 implementation program.

**Replan Triggers.** Stop if an accepted input drifts, a proving commit leaves history, a selector is missing/empty/
non-discriminating, a final gate cannot run in the supported environment, a semantic repair is
needed, or independent review finds a design-level defect. Never weaken a bound or mark completion
because time is exhausted.

**Rollback or Recovery.** A failed terminal gate leaves v7 executing. Repair through the owning packet, produce a new
candidate, and rerun all dependent evidence. The last packet remains trusted only while its proving
commit is ancestral and its named checks pass at HEAD.

**Design-Bearing Contracts and Exemplars (conditional).** None. Certification derives and executes the plan; it does not define new behavior.

## 5. Integration milestones

### M13 — Application contracts and the behavior-bearing release are coherent

**Entry:** v7 is activated with a fresh state file and WP53 is ready.

**Packets:** WP53 and WP60.

**Completion:** Provider contracts are application-owned and useful; one fallibly compiled release
causally owns provider, transformation, query, proof, and policy programs; fixture jobs and
synthetic Arrow/IPC results execute those programs in the current default composition without
production provider cutover or a premature final-feature claim.

**Gates:** All packet-local gates for WP53/WP60, especially `provider-job-contract-check`,
`release-program-contract-check`, `compiled-suite-identity-check`,
`semantic-request-program-check`, and the two narrow feature checks.

### M14 — Provider-native lifecycles terminate at application-owned results

**Entry:** M13 complete; the application contracts and compiled release are frozen.

**Packets:** WP57, WP58, and WP59.

**Completion:** Tree-sitter/Ruff use job-driven Arrow; rustc generated types terminate at ingress;
Pyrefly retains bounded compatible state and reconstructs explicitly. Each lane exercises the
provider-contract cancellation probe without importing daemon/runtime ownership.

**Gates:** `inprocess-provider-lifecycle-check`, `rustc-provider-lifecycle-check`,
`pyrefly-incremental-lifecycle-check`, provider/type/IPC gates, and extractor/sidecar domain gates.

### M15 — Fabric, state, task ownership, and one production route are coherent

**Entry:** M14 complete.

**Packets:** WP55, WP54, WP56, and WP61.

**Completion:** The provider-independent Arrow/DataFusion/Delta fabric preserves complete scan and
execution properties; semantic Delta truth, repository input, and operational state are separate;
the final isolated `semantic-release` feature executes provider-to-proof behavior; every task is
owned and joined; one injected release reaches all application services; thin Tonic adapters
preserve the released wire; no production selector reaches the old route.

**Gates:** `data-fabric-core-check`, `datafusion-scan-contract-check`,
`delta-publication-contract-check`, `cancellation-tree-check`,
`compiled-release-consumer-cutover-check`, `programmatic-production-composition-check`,
`grpc-flow-control-contract-check`, and proto/wire gates.

### M16 — Physical zero state and real correctness/recovery are proved

**Entry:** M15 complete.

**Packets:** WP62 and WP63.

**Completion:** DB19--DB22 are closed; target packages contain no displaced architecture; the real
installed source-to-FastMCP path and clean restart pass every provider, proof, publication,
cancellation, slow-consumer, security, and cleanup fault.

**Gates:** All WP62/WP63 gates, especially `compiled-release-legacy-zero-state-check`,
`semantic-release-vertical-check`, `semantic-release-restart-reconstruction-check`, and
`grpc-slow-consumer-check`.

### M17 — FreshActivation, measured final topology, and certification are complete

**Entry:** M16 complete.

**Packets:** WP64, WP65, and WP66.

**Completion:** DB23 is closed after the deployment census; FreshActivation/restart/forward repair
pass; the final topology meets its frozen resource/performance envelope; the full matrix and
independent review pass at one trusted HEAD.

**Gates:** All WP64--WP66 gates and the final matrix in §7.

## 6. Cross-packet decommission batches

### DB19 — Remove the marker/global release-authority route

**Target consumers:** WP60 behavior-bearing release and WP61 injected application services.

**Deletion safe after:** WP61 proves every production consumer uses one injected release.

**Scope:** Five unit authority structs, repeated `CompiledSemanticRelease::current()`, inert
authority arguments, mixed lane/profile types, duplicated live v2.2 suite/provider/query literals,
and interim daemon cfg gates whose generic/release split is complete.

**Exit invariants:** `just compiled-release-legacy-zero-state-check` finds no live source/export/
fixture/config/package route; `just release-program-contract-check` and
`just compiled-release-consumer-cutover-check` prove the replacement.

**Closed by:** WP62; re-certified by WP63 and WP66.

### DB20 — Remove provider-native and generated-type leakage

**Target consumers:** WP57 Tree-sitter/Ruff, WP58 rustc, WP59 Pyrefly, and WP61 Tonic adapters.

**Deletion safe after:** Each lane's application-owned job/result path and transport conversion pass.

**Scope:** Public provider-native helper signatures, generated rustc messages in admitted DTOs,
marker-taking constructors, disposable/terminate-per-run Pyrefly ownership, provider-process Python
bindings without a FastMCP consumer, and stale leakage exemptions/tests.

**Exit invariants:** `just provider-type-boundary-check` and
`just generated-type-boundary-check` pass seeded faults; all provider lifecycle/IPC gates and the
real vertical remain green.

**Closed by:** Immediate lane deletions in WP57--WP59 and residual closure in WP62; re-certified by
WP63 and WP66.

### DB21 — Remove fabric/state/task/transport dependency inversion

**Target consumers:** WP54 state ports, WP55 generic fabric, WP56 task scopes, WP60 release seam, and
WP61 transport/application composition.

**Deletion safe after:** WP61 completes the live composition cutover.

**Scope:** `data-fabric -> repository-state`, obsolete aggregate feature names/edges, gix/SQLite
reverse ownership, daemon-gated generic proof/closure, legacy scan delegation, interim cfg
workaround, unowned spawn/abort-drop paths, and generated Tonic types outside transport.

**Exit invariants:** `just feature-architecture-check`, `just datafusion-scan-contract-check`,
`just cancellation-tree-check`, `just generated-type-boundary-check`, `just features-each`, and
`just stable-graph-check` all pass with negative fixtures.

**Closed by:** WP62; recovery behavior re-proved by WP63--WP64 and certified by WP66.

### DB22 — Remove superseded live evidence, package, and gate reachability

**Target consumers:** WP62 target-only zero-state/oracle tooling and WP63 real vertical.

**Deletion safe after:** Target behavior and independent expectation clauses are executable without
the superseded tooling.

**Scope:** V3/v4/v5 release/issuance/certification scripts, deleted production-evidence modules,
stale disposition/comparator assets, `RFV5_*` selectors, old gate-census rows, retired recipes,
stale package resources/generated bindings, and any historical artifact that remains selectable.

**Exit invariants:** `just compiled-release-legacy-zero-state-check`,
`just fastmcp4-decommission-zero-state-check`, `just remaining-legacy-zero-state-check`, and
`just fastmcp4-package-build-check` prove absence while target behavior passes.

**Closed by:** WP62; re-certified by WP63 and WP66.

### DB23 — Remove dormant predecessor handoff/cutover authority

**Target consumer:** WP64 sole FreshActivation/forward-repair path.

**Deletion safe after:** A read-only deployment census finds no real predecessor and the target
FreshActivation/restart/reconciliation path passes.

**Scope:** `switchable_activation_authority`, predecessor handoff/cutover controllers, roles, states,
features, recipes, services, fixtures, and dual-authority recovery branches. Exact activation,
writer/fence, reconciliation, and lease primitives are retained.

**Exit invariants:** `just compiled-release-fresh-activation-check`,
`just predecessor-restart-revocation-check`, and
`just compiled-release-legacy-zero-state-check` pass after physical deletion.

**Closed by:** WP64; certified by WP66.

## 7. Final gate matrix

Every command below runs at the frozen WP66 candidate. Commands marked as new are plan deliverables
of their owning packet; they must be added to `just --list` rather than embedded as raw tool flags.
The final aggregate derives this matrix from the parsed plan, preserves every child exit, and fails
on an empty selector or seeded child failure.

### Artifact, design, and oracle integrity

- `just authoritative-design-conformance-check`
- `just artifacts-check`
- `just plan-status`
- `just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02.md`
- `just oracle-substance-check`
- `just gate-filter-census`
- `just tracked-target-zero-state-check`

### Feature and architecture boundaries

- `just feature-architecture-check` **new, WP53 and extended monotonically through WP62**
- `just features-no-default`
- `just features-each`
- `just stable-graph-check`
- `just governance-scan`

### Provider and release contracts

- `just provider-job-contract-check` **new, WP53/WP60**
- `just provider-type-boundary-check` **new, WP57--WP59**
- `just inprocess-provider-lifecycle-check` **new, WP57**
- `just rustc-provider-lifecycle-check` **new, WP58**
- `just pyrefly-incremental-lifecycle-check` **new, WP59**
- `just release-program-contract-check` **new, WP60**
- `just compiled-suite-identity-check` **new, WP60**
- `just exact-provider-batch-check`
- `just provider-ipc-contract-integrity-check`
- `just relation-ipc-provider-operations-check`
- `just provider-admission-exclusivity-check`
- `just provider-trust-coverage-remainder-check`
- `just provider-statistics-contract-check`

### Fabric, DataFusion, Delta, and activation

- `just data-fabric-core-check` **new, WP55**
- `just datafusion-scan-contract-check` **new, WP55**
- `just delta-publication-contract-check` **new, WP54**
- `just datafusion-contract-matrix-integrity-check`
- `just datafusion-plan-schema-cache-check`
- `just datafusion-cache-resource-operations-check`
- `just scheduled-streamed-semantic-query-check`
- `just semantic-request-program-check`
- `just delta-durability-protocol-integrity-check`
- `just delta-exact-reconstruction-v4-check`
- `just fabric-activation-recovery-check`
- `just fabric-control-recovery-check`
- `just activation-chain-validity-check`
- `just activation-fault-matrix-check`
- `just fabric-epoch-pinning-check`
- `just activation-receipt-nonauthority-check`
- `just candidate-free-recovery-check`

### Runtime, cancellation, transport, and serving

- `just cancellation-tree-check` **new, WP56**
- `just generated-type-boundary-check` **new, WP58/WP61**
- `just grpc-flow-control-contract-check` **new, WP61**
- `just grpc-slow-consumer-check` **new, WP63**
- `just semantic-release-restart-reconstruction-check` **new, WP63**
- `just query-retention-cancellation-restart-check`
- `just programmatic-runtime-lifecycle-check`
- `just supervisor-restart-join-operations-check`
- `just public-lifecycle-wire-contract-integrity-check`
- `just proto-check`
- `just proto-repro-check`
- `just fastmcp4-startup-contract-integrity-check`
- `just fastmcp4-daemon-wire-contract-check`
- `just fastmcp4-atomic-start-check`
- `just fastmcp4-resource-authority-check`
- `just fastmcp4-daemon-security-recovery-check`
- `just fastmcp4-guard-roundtrip-check`
- `just fastmcp4-completion-authorization-check`
- `just fastmcp4-contract-observation-check`
- `just fastmcp4-stdio-vertical-check`
- `just fastmcp4-security-negative-check`
- `just fastmcp4-cancellation-recovery-check`
- `just fastmcp4-dependency-contract-check`
- `just fastmcp4-modern-protocol-check`
- `just fastmcp4-adapter-authority-zero-state-check`
- `just fastmcp4-public-surface-check`

### Cutover, installed vertical, FreshActivation, and performance

- `just compiled-release-consumer-cutover-check`
- `just programmatic-production-composition-check`
- `just compiled-release-legacy-zero-state-check` **new, WP62**
- `just semantic-release-vertical-check` **new, WP63**
- `just compiled-release-fresh-activation-check` **new, WP64**
- `just compiled-release-resource-performance-check` **new, WP65**
- `just fastmcp4-post-purge-surface-check`
- `just fastmcp4-retained-target-behavior-check`
- `just fastmcp4-decommission-zero-state-check`
- `just remaining-legacy-zero-state-check`
- `just fastmcp4-package-build-check`
- `just predecessor-restart-revocation-check`
- `just unknown-cutover-reconciliation-check`

### Four build domains, packages, policy, and terminal aggregate

- `just root-fmt`
- `just root-check`
- `just root-clippy`
- `just root-test`
- `just extractor-ci-fast`
- `just sidecar-ci-fast`
- `just adapter-ci-fast`
- `just adapter-wheel-test`
- `just adapter-stdio-test`
- `just policy`
- `just governance`
- `just ci-pr`
- `just relational-fabric-v7-certification` **new, WP66**

## 8. Execution sequence and state discipline

1. Independently audit this draft. Apply material corrections in a successor plan artifact, approve
   the selected version, then use the confirm-gated activation transaction to create a fresh
   schema-v2 v7 state and atomically switch the active pointer. Preserve v5 plan/state as history.
2. V7 state contains only WP53--WP66, M13--M17, and DB19--DB23. It copies no v5 completion status,
   milestone status, batch status, proving commit, or check result. V5 proving commits remain
   discoverable historical evidence.
3. Execute WP53 first and freeze the application contract and feature names. Attribute every dirty
   overlap; preserve target-neutral WP49 deletions without claiming them complete.
4. Execute WP60 immediately after WP53. Compile every behavior-bearing program and move the
   release-specific proof/query seam in one transaction. Causally execute it with fixture jobs,
   synthetic Arrow/IPC results, and current generic evaluators; no inert program product is
   accepted and no production selector changes.
5. Execute WP57, WP58, and WP59 after WP60. These provider lanes may proceed in parallel because
   WP53/WP60 contracts are frozen. Serialize shared Cargo, `src/lib.rs`, rules, tests, and `justfile`
   integration; each lane uses the synchronous cancellation probe without importing daemon types.
6. Execute WP55 after all provider lanes terminate at application-owned results. Then execute WP54
   after the generic fabric split, and WP56 after state separation. This preserves the accepted
   provider -> fabric -> state -> runtime ownership sequence without a reverse dependency.
7. Execute WP61 as the atomic live composition cutover. Every production consumer switches to the
   same injected release before the proving commit; mixed global/injected authority is forbidden.
8. Execute WP62 immediately after WP61. Complete DB19--DB22 and rebuild target packages/features;
   do not postpone dead-route deletion until certification.
9. Execute WP63 against the purged installed topology. Any semantic/recovery defect returns to its
   owning packet and invalidates dependent proof.
10. Execute WP64 from an empty target root. Close DB23 only after the read-only deployment census
    and successful FreshActivation, then rerun FreshActivation after deletion.
11. Execute WP65 against the exact final WP64 topology and pre-registered bounds. Any implementation
    tuning returns to its owning packet and requires WP63--WP65 re-execution.
12. Execute WP66 at one frozen candidate. Run the full matrix, obtain independent implementation
    review, and make no semantic repair inside certification.
13. Each packet's proving commit contains its coherent implementation, tests, rules/fixtures, and
    recipes. State records only status, proving commit, deviations, failed approaches, blockers, and
    next action. Changed files, checks, digests, and ancestry are derived.
14. Shared sockets, services, target directories, caches, locks, and runtime roots remain shared
    resources even when worktrees are separate. Use isolated test/runtime roots and preserve
    unrelated dirty work, including `Untitled`.

## 9. Plan risks and replan policy

| Trigger | Required response |
|---|---|
| An accepted input or v2.3 authority drifts. | Stop dependent work and issue a versioned design/plan update; never restamp the declared-input table. |
| Provider contracts require DataFusion, Tokio async types, Tonic, concrete provider types, or daemon state. | Reopen WP53 boundary design; do not widen the inward graph. |
| Same-package features/rules repeatedly fail to prevent reverse dependencies. | Reopen design for a justified multi-crate split with measured build/runtime implications. |
| DataFusion cannot preserve every `ScanArgs`, statistic request, value, and physical property at the selected seam. | Reopen LD-41/extension placement; never silently drop information or hide semantics. |
| Delta needs a lower-level action/kernel path for a new functional outcome. | Re-run the delta-rs operation-selection ladder and design new protocol/concurrency proof before descending. |
| gix/SQLite/raw listing is discovered to select semantic state. | Stop WP54 and redesign the authority boundary; do not grandfather the reverse edge. |
| A program operand cannot be causally exercised without an inert token or second production path. | Merge/refactor the release and cutover packets or reopen design; never accept declaration theater. |
| Tree-sitter/Ruff show process-fatal, memory, cancellation, or trust failure under the governed workload. | Reopen their process placement using measured evidence; do not migrate all providers for symmetry. |
| Pyrefly context compatibility, bounded memory, cooperative drain, or clean reconstruction cannot be proved. | Reopen lifecycle/isolation design; do not retain a silent disposable fallback. |
| The released wire lacks a genuinely required control field. | Reopen the four-plane interface design and issue a versioned wire change; do not encode semantic rows or opaque JSON. |
| Every production consumer cannot switch atomically to one injected release. | Revise packet boundaries or design; do not leave mixed global/injected authority. |
| Zero-state coverage skips/unparses/unclassifies a live candidate or reaches secret/vendor/cache paths. | Correct the bounded coverage envelope and seeded fixtures before any deletion claim. |
| A deletion candidate has a real target consumer. | Move that consumer to the target boundary and replan the batch; never restore a predecessor subsystem wholesale. |
| Cross-user, network, remote, or multi-host daemon deployment enters scope. | Reopen transport, credentials, fencing, security, supervisor, and FastMCP deployment design. |
| Multiple suites must coexist or hot-swap in one daemon. | Reopen release construction; do not add a global registry or selector opportunistically. |
| A real deployed predecessor is discovered. | Stop WP64 and design a one-shot `AuthorityHandoff`; do not activate dormant generic cutover machinery. |
| Exact coherent readback cannot determine FreshActivation or unknown-outcome authority. | Remain failed closed and reopen durable activation storage/readback; do not seed, retry blindly, use latest, or compare hashes. |
| A performance bound is absent, edited after results, or missed after target-local optimization. | Issue independent requirements or reopen topology using measured attribution; never weaken the bound or compare to an unvalidated legacy path. |
| Independent review finds an architectural or contract defect. | Return to design and plan versioning. A certification/state edit cannot absorb the change. |

Implementation adaptation stays within accepted I-60--I-71 and is recorded in execution state.
Packet dependency, cutover, decommission, or oracle changes require plan revision. Architecture,
public contract, library decision, or target-invariant changes require design reopening. The user's
explicit design-phase authority permits prior decisions to be overturned; it does not permit an
unrecorded change to the accepted target.

## 10. Activation and completion boundary

This audited successor remains inactive until explicit approval and activation. Activation must
validate this exact file and declared inputs, verify baseline ancestry and the complete dirty-tree
inventory with the declared algorithm, seed the two declared baseline failures into state, create a
new schema-v2 state at the declared v7 path with only WP53--WP66/M13--M17/DB19--DB23, atomically
replace the active-plan pointer, and leave v5/v6 artifacts immutable. A failed activation leaves v5
selected and creates no partial v7 authority.

Implementation is complete only when every packet and milestone has an ancestral proving commit,
DB19--DB23 are physically closed, every named check passes again at one trusted candidate HEAD, the
installed source-to-FastMCP vertical and FreshActivation/restart/reconciliation pass from clean
roots, the final topology meets its frozen resource/performance envelope, all four build domains and
packages pass, and the independent implementation review is accepted. Only the governed state
transaction may then mark v7 complete.
