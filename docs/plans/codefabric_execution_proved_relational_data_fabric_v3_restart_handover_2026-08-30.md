# CodeFabric relational data fabric v3 restart handover

Date: 2026-08-30
Status: implementation in progress; no packet is complete or commit-proved

## 1. Canonical restart pointers

- Active plan: `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md`
- Accepted design: `docs/designs/codefabric_execution_proved_relational_data_fabric_design_v3_2026-08-30.md`
- Execution state: `docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json`
- Doctrine: `docs/library_ref/full_data_fabric_design_principles_v2.md`
- Active-plan pointer: `docs/plans/active-plan.json`
- Reconciled baseline HEAD: `db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34`

The v3 plan is active. State is `executing`, `current_packet` is `WP29`, and `WP28` is
`invalidated`, not complete. The owner explicitly waived WP28's ledger/process work and directed
the implementation to begin with the functioning programmatic fabric. Downstream readiness must
therefore treat WP28's dependency edges as removed; final reporting must not claim its oracles ran.

## 2. Owner decisions that control implementation

1. Pivot now to the v3 architecture. Do not ground acceptance in whether the unfinished legacy
   architecture behaved correctly.
2. Correctness should be proved directly from independently specified inputs, typed rows,
   invariants, causal faults, exact durable versions, and end-to-end behavior. A predecessor
   comparator is optional diagnostic evidence only.
3. Provider facts, explicit non-derivable typed inputs, and typed programmatic transformations are
   the only semantic construction inputs. Bootstrap/replay/generated-schema authority is rejected.
4. Maximize native Arrow 59.2.0, DataFusion 55.0.0, and pinned delta-rs capabilities. Avoid custom
   abstractions that duplicate catalog, schema, plan, provider, execution, statistics, transaction,
   or exact-version behavior.
5. Delta histories carry proof-bearing/restart/audit/incremental state. DataFusion and Arrow
   execution buffers and bounded caches remain reconstructible non-authority.
6. Prioritize the target production path and first-principles proof over elaborate legacy
   comparison or decommission bookkeeping. Still delete rejected authority once its last target
   consumer has moved; do not create a fallback or dual-write route.
7. The workspace is intentionally very dirty and shared. Preserve unrelated edits and never reset
   or clean broad paths.

## 3. Work completed immediately before restart

### Plan activation

- Updated the v3 plan's declared design digest from the stale separately authored value to the
  current accepted design bytes:
  `fc70bb9b356367595fae504dc605f513f8234500fd86eaf46945016c241e4945`.
- Ran the repository activation transaction successfully. `docs/plans/active-plan.json` now points
  to v3 and the schema-2 v3 state file exists.
- Recorded the WP28 owner waiver, WP29 start, the runtime-native durability obligation, and the
  mechanical design-digest reconciliation in state.
- The direct artifact contract validator passed with 16 declared inputs, 15 packets, and 131
  released fixtures:

  ```bash
  PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp \
    python tooling/ci/artifact_contracts.py artifacts-check
  ```

- `just artifacts-check` did not reach the artifact validator because an already-present Ruff
  formatting defect in `tooling/ci/test_plan_assurance.py` blocked `model-tooling-lint` at the test
  signature around line 30. This is not evidence against the v3 artifacts and was not edited.

### Semantic compiler pivot in progress

`src/relational_semantic_query.rs` was mechanically and semantically renamed from replayed-model
vocabulary to explicit typed program-catalog vocabulary. Important changes include:

- `SemanticQueryModelRows` -> `SemanticQueryProgramCatalog`
- `model_epoch_pin` -> `program_catalog_pin`
- `compiler_release_pin` -> `program_compiler_release_pin`
- `Model*` binding/operator/schema row types -> `Program*` types
- `SemanticCompilerDependency::ModelEpoch` -> `ProgramCatalog`
- model-specific error and local vocabulary -> program-catalog vocabulary

The file was formatted before the validation command started, but its targeted test run was
interrupted and has no pass/fail result. It is untracked along with much of the current v2/v3 work,
so ordinary `git diff` will not display its patch. Verify it first after restart.

### Agent workstream handoffs

The three delegated agents were stopped for restart and made no edits in their final v3 slices.
Their findings are incorporated below.

## 4. Current architectural picture

### Strong reusable target foundations already in the tree

The current untracked/shared implementation already contains substantial target code:

- `programmatic_schema.rs`: provider/plan-derived contracts, typed transformations, dependency
  ordering, candidate catalog observations, and sealing.
- `programmatic_epoch.rs`: one fresh DataFusion runtime/catalog, exact provider inputs,
  transformations, five observation histories, exact reopen, program bindings, and epoch-owned
  logical-plan cache.
- `datafusion_cache.rs`: bounded DataFusion metadata/statistics/listing cache policy and
  collision-safe logical-plan authority keys.
- `programmatic_observation_delta.rs`: stable five-family Delta history provisioning,
  append/readback, exact-version reopen, same-session rebinding, and fixed-point checking.
- `activation_control_delta.rs` and `activation_transaction.rs`: typed exact version vectors,
  append-only activation control, marker/chain reconstruction, receipt non-authority, and
  candidate-free epoch reconstruction.
- `relational_query_runtime.rs`: epoch admission, authorized child execution, Arrow-native result
  packaging, shared result registry, resource leases, and cancellation.
- `command_runtime_manager.rs`: registered per-workspace command runtimes and recovery management.
- Relation-scoped Arrow IPC and provider services have previously passed focused interoperability
  checks across root, Pyrefly, rustc extractor, and Python protobuf consumers.

These are implementation assets, not completion claims. Most are untracked and have no v3 proving
commit.

### The central production gap

`src/daemon.rs` still calls `bootstrap_fabrics`, constructs `WorkspaceQueryBackend::default()`, and
serves the predecessor `ProductionQueryService<WorkspaceQueryBackend>`. The isolated target
components are therefore not yet the real daemon composition.

The existing `WorkspaceQueryBackend` owns legacy `ServingQuerySession` values and cannot execute
`RelationalQueryRuntime`. A production factory alone is insufficient unless the daemon's semantic
query backend is adapted to compile an accepted semantic request into target `SelectedQueryOutput`
programs and execute them through the relational runtime. Until that adapter exists, production
must fail closed rather than silently keep bootstrap serving.

## 5. Recommended implementation sequence after restart

### A. Verify the interrupted local edit

Run:

```bash
rustfmt --check --edition 2024 src/relational_semantic_query.rs
cargo nextest run -E 'test(relational_semantic_query)'
```

If compilation exposes an external caller using the old public names, migrate it to the
program-catalog names. Do not add deprecated aliases; this compiler had no current external
production caller in the last search.

### B. Finish WP29: make the target the real daemon

Implement an all-or-nothing typed production workspace factory. Its input should contain only
exact, explicit authority: workspace identity, provider batches, typed program inputs,
transformations, runtime/cache/resource policy, canonical Delta roots, exact activation/version
selection, command ports, and authorization policy.

Construction should:

1. create the governed `FabricEpochRuntimeConfig` and fresh `ProgrammaticFabricEpochBuilder`;
2. register exact provider/input relations and typed transformations;
3. either historicize a candidate or reopen the activation-selected exact Delta vector;
4. validate one sealed `ProgrammaticFabricEpoch` before publishing anything;
5. construct the matching `FabricAdmissionRuntime`, `EpochResourceCoordinator`, shared
   `PublishedArrowResultRegistry`, `RelationalQueryRuntime`, and complete command runtime;
6. validate that all epoch/workspace/resource/registry identities agree; and
7. atomically install the workspace only after the complete composition succeeds.

Missing batches, policies, roots, exact versions, activation head, credentials, or command effects
must reject admission. There must be no empty-success backend, bootstrap fallback, or partially
installed workspace.

Then replace `serve()`'s `WorkspaceQueryBackend::default()` and `bootstrap_fabrics` route. Add one
real typed cold-start/query/restart case plus missing-input and partial-construction rejection.

### C. WP30: remove the old epoch/bootstrap authority after the consumer moves

Before deleting `src/fabric/epoch.rs`, extract its authority-neutral target primitives into a
target-owned module:

- `FabricEpochId` (currently an alias of command `EpochId`)
- `FABRIC_CATALOG`
- `FabricSchemaRole` (the programmatic path already excludes `Model`)
- `FabricEpochRuntimeConfig` and runtime observations
- `epoch_identity_text`

The old `FabricEpoch` is the only non-test caller found for the transitional
`RelationalProgramCompiler::{bind_catalog_inputs, compile}` methods that consume `ModelEpoch`.
The target uses the corresponding `*_with_bindings` methods. Once daemon/provider consumers are
programmatic, delete the old epoch and then delete those replay-backed compiler overloads and
`ProgramBindings::from_model_epoch`.

`provider_admission.rs` still exposes both old `FabricEpochBuilder` and
`ProgrammaticSchemaAssembly` routes. Cut consumers to the programmatic route, then remove the old
builder/overloads. Do not begin by deleting `relational_model/**`; first move every genuinely
retained identity/wire/schema-phase primitive so compiler errors identify real remaining legacy
consumers rather than collateral target breakage.

### D. WP31: close the three concrete DataFusion gaps

1. `DataFusionCachePolicy::try_new` currently accepts an object-list TTL greater than 30 seconds.
   Reject `> 30s` and add boundary tests. TTL is a refresh bound, never validity.
2. Add explicit bounded observation-closure validation for duplicate, dangling, incomplete,
   non-self-describing, inert, row-overflow, and iteration-overflow cases. Require an unchanged
   reread before sealing.
3. Authorized child construction currently copies `ViewTable` providers whose logical plans may
   retain parent-bound `TableSource` Arcs. Rebuild each granted view in topological order by
   rewriting every table scan/subquery to providers already installed in the child; reject
   ungranted or cyclic dependencies. Recursively validate table, function, extension, variable,
   runtime, and object-store closure.

Keep using native DataFusion logical plans and complete tree traversal. Reuse cached logical plans
only under the full authority fingerprint; build fresh physical plans and results for every
execution.

### E. WP32: close exact Delta evidence and CDF behavior

Verify and, if absent, implement exact commit-entry readback of both `CommitInfo` and `Action::Txn`
after every write. Do not infer a version from `history()` iterator order. The pinned source route
is `log_store.read_commit_entry(version)` followed by `deltalake::logstore::get_actions`.

Finish production CDF execution and explicit gap/fallback behavior. CDF is incremental transport,
not semantic authority. Always provide start and inclusive end, prove the end exists first, never
use out-of-range tolerance for proof, and advance a consumer checkpoint only after downstream
success—even for an empty interval.

Retain the five stable append-only history tables rather than per-epoch table roots. Exact Delta
root/version vectors and the unique activation-chain head select current state. Keep statistics
enabled (`skip_stats=false`), zero application retries, protocol/feature gates before reads/writes,
and guarded retention/vacuum.

### F. Continue the functional vertical before broad cleanup

After WP29-WP32:

1. exact provider batches and released Arrow IPC (`WP34`);
2. application-owned Python/Rust/common derived producers (`WP35`);
3. all eight semantic request programs and authorized graph/query execution (`WP36`);
4. source lifecycle -> command -> activation -> query -> UDS -> FastMCP (`WP37`);
5. independently authored first-principles expectations and causal faults (`WP33`/`WP38`);
6. residual legacy/tool/package removal and post-purge proof (`WP39`/`WP40`);
7. sole-authority cutover and final certification (`WP41`/`WP42`).

This ordering honors the owner's speed priority: establish one real production vertical first,
then let compiler/build/search failures drive precise legacy deletion.

## 6. Library-specific rules to preserve

### Arrow and DataFusion

- Admitted `SchemaRef` and built `LogicalPlan::schema()` are authority. Expected schemas are
  assertions only.
- Keep one Arrow/DataFusion type universe and use native providers, expressions, logical builders,
  catalog/session APIs, statistics, execution streams, memory reservations, spilling, and
  cancellation at the highest viable rung.
- One candidate session derives and observes its own relation, field, schema, dependency, and
  provenance relations to bounded fixed point.
- Child sessions are fresh restricted catalogs, not a parent context with names hidden.
- Cache bounded metadata, file statistics, object listings, and compiled/optimized logical plans
  under complete authority keys. Never cache semantic-current selection, physical plans, or
  results.

### Delta

- Use one stable history table for each observation/proof family and append epochs as rows.
- Select every table by canonical root plus exact version; never resolve `latest` after admission
  and never scan a Delta root as raw Parquet/listing state.
- Delta transactions are atomic per table, not across the five histories. The application-owned
  observation-set identity and activation vector supply cross-table meaning and reachability.
- Commit history/statistics/CDF are physical evidence and optimization inputs, not semantic
  completeness proof.
- Orphaned component commits may exist after partial failure but must remain unreachable because no
  complete activation vector names them.

### Code-fact providers

- Consume the current pinned Ruff, Pyrefly, Tree-sitter, and `rustc_public` APIs directly behind
  application-owned adapters; do not weaken the unified design for hypothetical future API drift.
- Provider-native facts remain raw; normalization and derived analysis are separate typed
  transformations.
- Missing output is explicit unknown/capability remainder, never an empty negative fact.
- Application identity remains independent of provider-local node/definition indices.

## 7. Validation boundary at handoff

- No v3 packet is complete and no v3 proving commit exists.
- The direct artifact/state validator passed after activation.
- The semantic compiler targeted nextest command was interrupted and has no result.
- A delegated `just root-check-fast` reached root compilation but was interrupted before an exit
  code; it is not evidence of pass or failure.
- No broad validation was run or is implied.
- All observed Cargo/rustc processes were terminated before this handoff was written.

After the restart, prefer focused checks while production composition is moving. Run the broad
four-domain/release matrix only when the functional vertical and deletions have stabilized.

## 8. Dirty-tree and ownership warning

The repository already contained dozens of modified and untracked files before this activation.
In particular, most `src/fabric/*.rs` target modules are untracked shared work. `git diff` does not
show untracked file contents. Use `git status --short`, direct file inspection, and scoped hashes;
do not infer that an untracked file was created by the most recent agent.

Changes attributable to the final pre-restart turn are limited to:

- `docs/plans/active-plan.json`
- `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md`
- `docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json`
- `src/relational_semantic_query.rs`
- this handover document

The delegated WP29/WP31/WP32 agents made no file edits. Existing changes such as
`src/fabric/overlay.rs`, `src/fabric/serving.rs`, and the many untracked fabric modules predate their
final assignments and must be preserved.

## 9. Minimal restart checklist

```bash
git status --short
sed -n '1,40p' docs/plans/active-plan.json
sed -n '1,120p' docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json
just --list
PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp \
  python tooling/ci/artifact_contracts.py artifacts-check
rustfmt --check --edition 2024 src/relational_semantic_query.rs
cargo nextest run -E 'test(relational_semantic_query)'
```

Then resume at WP29's all-or-nothing production workspace factory and semantic-backend adapter.
Do not resume WP28, do not reactivate v2, and do not restore `bootstrap_fabrics` as a temporary
fallback.
