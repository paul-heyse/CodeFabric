# CodeFabric execution-proved relational data fabric v3 — Codex execution handover

- **Date:** 2026-08-30
- **Plan:** `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md`
- **Prior handover:** `docs/plans/codefabric_execution_proved_relational_data_fabric_v3_restart_handover_2026-08-30.md`
- **Design authority:** `docs/library_ref/full_data_fabric_design_principles_v2.md` and the v2.1 authoritative design suite
- **Working-tree basis:** uncommitted shared-tree edits; no proving commit was created

## 1. Governing execution decision

The user explicitly directed this execution to:

1. skip WP28 rather than execute it;
2. pivot directly to the new programmatic relational data-fabric design;
3. prioritize first-principles functional proof and strong Arrow, DataFusion, and delta-rs use;
4. avoid spending execution time carefully preserving an unproved predecessor design;
5. preserve concurrent working-tree edits and leave the separately owned tooling/sccache/PID issue
   to its current owner.

WP28 was therefore treated as excluded, not completed. No WP28 ledger or predecessor accounting
artifact was authored. Work started at WP29, with useful disjoint WP31 and WP32 foundations advanced
in parallel. This does **not** change the plan's dependency truth: WP30 and the remaining successor
cutovers are still open.

## 2. Honest packet status at restart

| Packet | Status | What this run established | What remains before closure |
|---|---|---|---|
| WP28 | **excluded by user** | No work performed. | Nothing unless the user reverses the decision. |
| WP29 | **substantial implementation in progress** | Programmatic workspace composition, epoch-scoped query authority, direct semantic compiler, request-owned Arrow inputs, restricted child execution, Arrow publication outcome, and a drafted production backend now exist. | Concrete application ports, production `serve` cutover, end-to-end daemon construction/ownership, lifecycle integration, and the required functional/fault/restart tests. |
| WP30 | **not started** | Positive replacement seams needed for deletion are being built. | Production consumer cutover, then deletion of bootstrap/model/generated-schema/dual-epoch authority and zero-state proofs. |
| WP31 | **partial, opportunistic** | Supplemental program bindings, restricted request-local child plans, fresh physical execution, provider-identity validation, and bounded DataFusion cache TTL work exist. | Full schema/catalog fixed point, plan/provider contract matrix, complete child closure, cache collision/reuse/eviction proofs, native-rung inventory, and legacy removal after WP30. |
| WP32 | **substantial partial implementation** | Exact commit reconstruction/readback, exact CDF/fallback, exact provider/statistics evidence, table properties, write controls, and uncertain-commit foundations were implemented. | Integrate with the post-WP30 public types, complete all history/retention/maintenance/recovery oracles, and run final runtime/fault tests on a stable tree. |
| WP33 onward | **not started in this run** | None claimed. | Execute dependency order after WP29/WP30 and the required WP31/WP32 closure. |

No packet should be marked complete from this handover. The implementation is intentionally ahead
of its terminal proof, and the production entry point still selects the legacy backend.

## 3. Implemented changes

### 3.1 Arrow-native query-service outcome (`src/query_service.rs`)

The query service now supports a backend-owned Arrow publication path without serializing semantic
rows into the legacy JSON response:

- `SemanticQueryBackend::validate_execution_request` makes executable capability a backend-owned
  policy rather than a service-owned generated registry decision.
- `SemanticBackendExecutionContext` carries the authenticated agent/workspace observations and the
  typed `PublishedResultOwner`, `QueryExecutionPin`, `LeaseId`, opaque result token, and exact
  daemon-wide `PublishedArrowResultRegistry`.
- `SemanticBackendOutcome::{Legacy, PublishedArrow}` permits a controlled transition while WP29 is
  proved and WP30 remains open.
- `PublishedArrowSemanticSuccess` carries the exact relational publication, opaque lease token,
  public snapshot projection, and execution evidence.
- Published Arrow success emits snapshot/artifact/terminal control events and persists only
  evidence/control metadata. Semantic rows remain in owner-bound Arrow IPC resources.
- Cancellation or terminal races release an already-published Arrow resource instead of leaking it.
- Typed delivery identities are domain-separated and token-keyed; tests cover determinism,
  cross-agent separation, strict public workspace identity, and registry identity.

Legacy outcomes remain only for compatibility until WP30. The new backend does not use them.

### 3.2 One-admission relational execution (`src/fabric/relational_query_runtime.rs`)

`RelationalQueryRuntime::execute_admitted_and_publish` accepts the exact `FabricQueryLease` acquired
before semantic compilation plus the matching epoch resource coordinator. It rejects a resource
epoch mismatch and prevents an activation between compile and execute from mixing two epochs.

`RelationalQueryTransaction` can now attach request-owned relation collections by exact selected
output relation. Multi-block requests do not expose one block's request-local provider to another
block. The ordinary epoch-only path remains explicit.

The runtime selects either:

- ordinary sealed-epoch child execution; or
- request-owned execution through
  `AuthorizedChildSession::execute_relational_program_with_request_inputs`.

The result still passes through `ArrowResultResourcePackage`, resource retention, and the shared
published-result registry. No result cache or Python semantic-processing layer was added.

### 3.3 Direct epoch-bound semantic compiler (`src/relational_semantic_query.rs`)

A new direct compiler path was added alongside predecessor definitions that WP30 will remove:

- `EpochBoundSemanticIngressLimits` and a deterministic limits pin;
- typed block binding, repeated selection, repeated return, scope, request-owned tuple, dependency,
  and consumer-slot relations;
- `EpochBoundSemanticIngressCatalog`, keyed by exact program binding IDs and pins;
- exhaustive ingress validation: pins, bounds, row consumption, identity, cardinality, ordinals,
  DAG topology, fan-in/fan-out, and explicit request-input contracts;
- `EpochBoundSemanticExecutionCatalog`, keyed by `(program_binding_id, execution_program_pin)`;
- direct lowering of operator DAGs, consumer-slot composition, repeated-selection folds, return
  realization, explicit limits, required producer families, scopes, and request inputs;
- `compile_epoch_bound_semantic_request`, which never converts through legacy
  `SemanticRequestRelations` and never selects an executor by released form;
- `EpochBoundSemanticRuntimeHandoff`, which makes request inputs and normalized scopes
  non-discardable runtime outputs;
- causal compiler observations over epoch, request, catalog, policy, source, producer closure,
  program binding, relation, field, selection, return, scope, and request-input pins.

The released form is retained only as compatibility observation. It is not execution authority.

### 3.4 Query-local Arrow relations (`src/fabric/request_owned_relation.rs`)

This new daemon-gated module materializes compiler handoffs as exact Arrow/DataFusion inputs:

- `RequestOwnedRelationLimits` has no default and independently bounds relation, row, field, cell,
  aggregate, and text-byte use.
- Content pins are recomputed with the same domain and length framing as the compiler.
- Identities, exact program/handoff/content pins, field types, required/null semantics, contiguous
  ordinals, duplicate keys, and all resource totals are validated before allocation.
- Typed Arrow arrays, `SchemaRef` metadata, `RecordBatch`, `MemTable`, and direct DataFusion
  `RelationInput` scans are constructed without a JSON/string detour.
- Every materialized relation retains the exact `Arc<dyn TableProvider>` and can prove that a scan
  still points to that capability, not merely to an equal schema.
- The request-program binding authority frames the execution-program, handoff, and content pins.
- Supplemental program bindings expose stable relation/field identity and the exact query-local
  table reference without installing anything in the epoch catalog.

The module remains `#[cfg(feature = "daemon")]`; its child-session seam is correspondingly gated so
the narrower `data-fabric` feature graph still compiles.

### 3.5 Supplemental bindings (`src/relational_program.rs`)

`SupplementalProgramRelationBinding` and
`ProgramBindings::with_supplemental_relations` extend an immutable epoch binding set for one
compilation only. They:

- reject relation, field, and table-reference shadowing;
- reject duplicate field names/identities and invalid ordinals;
- retain exact Arrow schema and field IDs;
- derive a new deterministic authority ID from the parent authority plus relation, table, schema,
  field, and content-authority material;
- leave the base epoch `ProgramBindings` unchanged.

Tests cover immutable extension, content-pin causality, and collision rejection.

### 3.6 Restricted child execution (`src/fabric/child_session.rs`)

`AuthorizedChildSession::execute_relational_program_with_request_inputs` now:

- clones and extends exact epoch bindings for the request only;
- consumes verified direct scan plans for request relations;
- rejects shadowing, binding drift, denied epoch relations, and unused request relations;
- validates exact provider `Arc` identity and schema on compiled **and** optimized plans;
- never registers request providers in the parent or reduced child catalog;
- deliberately bypasses the shared epoch logical-plan cache until its key/validator can retain the
  complete concrete request authority;
- constructs a fresh physical plan and result for every execution;
- preserves the existing output row envelope and exact schema checks.

`ChildProgramCacheUse::{SharedCache, RequestOwnedAuthorityBypass}` records this distinction instead
of calling a bypass a cache miss.

### 3.7 Programmatic workspace and daemon composition (`src/fabric/programmatic_workspace.rs`)

This new module provides the all-or-nothing composition root:

- `ProgrammaticWorkspaceReleasePins` requires explicit non-sentinel input, provider, application,
  and policy releases.
- `WorkspaceEpochQueryAuthority` now owns the exact epoch, resource coordinator,
  `EpochBoundSemanticIngressCatalog`, `EpochBoundSemanticExecutionCatalog`, producer closure,
  base query authorization, request-owned relation limits, result limits, and result lease.
- Ingress/execution catalogs must agree on fabric, program, source, policy, and producer pins;
  application-owned fact authority/class and producer closure must align.
- `programmatic_fabric_epoch_authority_pin` derives semantic epoch identity from the sealed epoch
  ID, complete exact Delta version vector, schema authority, and runtime configuration. An arbitrary
  caller label cannot substitute for this authority.
- The exact activation-selected epoch/table vector/resource policy is re-opened and verified.
- Receipt-only reconciliation is used; the cache never selects current authority.
- Clean restart installs the reconciled selected head into admission before routes open.
- Epoch-scoped authority lookup retains old immutable authorities for admitted work.
- One daemon-wide `PublishedArrowResultRegistry` is shared by every workspace.
- Daemon build stages every workspace and starts every registered command runtime before publishing
  the composition; partial construction closes admission and shuts down started runtimes.
- Shutdown closes query admission before joining command runtimes.
- Structured startup observations name the factory, releases, activation head/fence, exact table
  vector, catalog/compiler/producer/resource/request-limit pins, runtime configuration, and schema
  authority. These observations are not acceptance authority.

### 3.8 Programmatic semantic backend (`src/fabric/programmatic_query_backend.rs`)

This new module is drafted and format-clean. It defines three explicit, no-default application
ports:

- `ProgrammaticSemanticIngressPort` transforms the already validated released DTO into exact
  epoch-bound ingress rows;
- `ProgrammaticScopeAuthorizationPort` consumes normalized scope handoffs to derive the complete
  reduced-child authorization;
- `ProgrammaticSnapshotProjectionPort` emits public control metadata for the exact admitted epoch.

`ProgrammaticSemanticQueryBackend` then:

1. resolves an admitted public workspace route;
2. verifies the service, workspace, and runtime share one published-result registry;
3. admits exactly one epoch and resolves only that epoch's query authority;
4. projects and validates the public snapshot **before** Arrow publication, so projection failure
   cannot leak a result resource;
5. verifies ingress semantic-request identity and a domain-separated pin of canonical request bytes;
6. validates and directly compiles the epoch-bound semantic program;
7. rejects every non-compiled block rather than silently omitting it;
8. invokes the scope authorization port and checks its policy identity;
9. partitions request-owned handoffs by exact output/query so multi-block authority cannot bleed;
10. constructs exact result lease/transaction inputs;
11. executes with the already-admitted epoch and publishes Arrow resources;
12. returns `PublishedArrowSemanticSuccess` with stage/coverage evidence.

Important: the backend ports have interfaces but no concrete production implementations yet. The
backend is therefore a positive composition seam, not yet the production daemon entry point.

### 3.9 Daemon routing (`src/daemon.rs`)

The daemon now has a generic backend runner plus
`serve_with_programmatic_query_backend`. Programmatic mode:

- skips `bootstrap_fabrics`;
- uses the explicitly supplied daemon-wide published-result registry;
- advertises only explicitly supplied workspace claims;
- rejects legacy workspace/ontology administrative ingress with
  `PROGRAMMATIC_COMMAND_INGRESS_REQUIRED`.

The public `serve()` function **still constructs `WorkspaceQueryBackend::default()` and selects
legacy bootstrap mode**. This is the principal unfinished WP29 production cutover and must not be
overlooked after restart.

### 3.10 Admission and activation support

`src/fabric/admission.rs` now includes:

- production clean-restart installation of a reconciled selected head;
- an explicit draining phase;
- idempotent `close_for_shutdown` behavior;
- activation-swap tests proving compilation/execution cannot mix epochs.

`src/fabric/activation_control_delta.rs` exposes the exact workspace/control relation and a
`current_snapshot` read of durable writer-fence plus activation-chain authority.

### 3.11 Bounded DataFusion caches (`src/fabric/datafusion_cache.rs`)

Object-list cache TTL is explicitly bounded to at most 30 seconds and documented as refresh policy,
never semantic validity or current-state authority. Boundary tests were added. Request-owned plans
currently bypass the shared logical-plan cache; ordinary sealed-epoch plans continue using exact
authority keys and fresh physical planning.

### 3.12 Delta exactness and write controls (`src/fabric/delta_exact.rs`, `delta_write.rs`)

The WP32 work completed during this run includes:

- exact commit readback of `commitInfo` and every `txn` action;
- rejection of missing, duplicate, or mismatched operation/transaction entries;
- the same exact reconstruction on fresh commit and restart;
- exact CDF range validation, including end-first retained-log proof and every interior commit;
- explicit exact-snapshot fallback when retained log/CDF coverage is incomplete;
- DataFusion plan execution validation and valid zero-row success;
- a non-constructible downstream-success token before checkpoint/continuation;
- exact-version providers configured with `require_files=true` and `skip_stats=false`;
- flattened Delta add-action evidence and explicit `UnknownForFiles` statistics gaps;
- DataFusion `Statistics::new_unknown` when provider statistics are absent, never a fabricated zero;
- creation-time history properties: append-only, CDF enabled, explicit statistics columns, and
  deletion vectors disabled;
- controlled 128 MiB target files, 65,536-row write batches, 65,536-row/64 MiB row groups, and ZSTD;
- layout choices committed and reread through exact `commitInfo`;
- physical Parquet metadata assertions for field metadata and compression;
- existing one-transaction, operation-metadata, zero-retry, and exact uncertain-commit
  reconciliation retained.

This is not the whole WP32 acceptance surface; retention, maintenance, recovery, and integrated
post-WP30 proof still remain.

## 4. Validation evidence available now

The following evidence was obtained before the restart request:

- `rustfmt --check` passes for all task-touched Rust files, including the three new modules.
- `git diff --check` passes for all tracked task-touched files.
- `cargo check --lib --no-default-features --features data-fabric` passes after daemon-gating the
  request-owned child seam (warnings only).
- `cargo check --tests --no-default-features --features data-fabric` passed for the completed Delta
  slice before the final shared-tree restart request.
- An earlier focused exact-commit/CDF Delta test run passed 19 tests.
- Multiple daemon/root checks reached only the separately owned Rust 1.98 blocker at
  `src/provider_sandbox.rs:832`:

  ```text
  Option<NonZero<i32>>::map(Pid::from_raw)
  expected fn(NonZero<i32>), found fn(i32)
  ```

  No task-local compiler diagnostic was emitted in those checks. The final active check was stopped
  at the user's restart request; it must not be represented as a passing terminal gate.

Not yet run successfully on the final combined tree:

- daemon tests;
- WP29 cold-start/query/command/restart vertical;
- `just root-test` (nextest plus doctests);
- the plan's named WP29/WP31/WP32 executable oracles;
- `just ci-fast` or `just ci-pr`;
- final decommission/zero-state scans.

## 5. Working-tree ownership and restart cautions

Task-relevant paths are:

```text
src/daemon.rs
src/fabric.rs
src/fabric/activation_control_delta.rs
src/fabric/admission.rs
src/fabric/child_session.rs
src/fabric/datafusion_cache.rs
src/fabric/delta_exact.rs
src/fabric/delta_write.rs
src/fabric/relational_query_runtime.rs
src/query_service.rs
src/relational_program.rs
src/relational_semantic_query.rs
src/fabric/programmatic_query_backend.rs        (untracked)
src/fabric/programmatic_workspace.rs            (untracked)
src/fabric/request_owned_relation.rs             (untracked)
```

The repository also contains concurrent tooling/configuration edits and untracked scripts related
to Rust 1.98, sccache, linker benchmarks, environment contracts, CI, and documentation. Those were
explicitly owned by another agent. Preserve them and do not fold them into data-fabric reasoning
without re-establishing ownership after restart.

No commit was created. Do not reset, checkout, or broadly format the dirty tree.

## 6. Immediate restart sequence

After the Codex environment restarts:

1. Read this handover, the v3 plan, the prior restart handover, and the v2 data-fabric principles.
2. Confirm the three new source files remain present and inspect `git status --short` without
   attributing unrelated tooling changes.
3. Confirm the external owner resolved the sccache/toolchain/PID issue; do not silently take it over.
4. Run the smallest current-tree checks first:

   ```bash
   /home/paul/.rustup/toolchains/1.98.0-x86_64-unknown-linux-gnu/bin/rustfmt \
     --edition 2024 --check \
     src/fabric/programmatic_query_backend.rs \
     src/fabric/programmatic_workspace.rs \
     src/fabric/request_owned_relation.rs \
     src/fabric/child_session.rs \
     src/fabric/relational_query_runtime.rs \
     src/relational_program.rs \
     src/relational_semantic_query.rs

   git diff --check
   just root-check-fast
   ```

5. If the unrelated blocker persists, report it and continue with read/static work that does not
   require owning that file. If it is resolved, run the daemon feature check before new refactors.

## 7. Detailed next implementation order

### Step A — finish and prove WP29 positive composition

1. Audit `programmatic_query_backend.rs` against the final workspace/child APIs after the restart.
2. Add accessors for the returned `RelationalQueryAuthorization` pins and verify the scope port's
   returned query policy equals the compiled handoff policy; verify resource policy against the
   selected epoch resource envelope.
3. Bind workspace catalog source/policy pins to the corresponding explicit release/activation
   inputs rather than accepting only mutual catalog agreement.
4. Implement concrete application-owned ingress transformation rows. It must:
   - consume the public request exhaustively;
   - select exact program binding IDs from installed application data, never from a hard-coded form
     switch;
   - emit `canonical_request_content_pin(request.canonical_bytes)`;
   - preserve repeated values, returns, scopes, tuple rows, dependency slots, and exact limits;
   - reject unknown forms/programs/fields rather than defaulting.
5. Implement the concrete scope authorization port and prove each scope row causally changes or
   rejects the child grant/policy.
6. Implement the concrete public snapshot projection from the programmatic epoch and exact Delta
   vector. Do not call the legacy snapshot/model reader. If exact Delta reads are required, change
   the current synchronous port to an asynchronous port rather than hiding I/O.
7. Couple backend route lifetime to `ProgrammaticDaemonComposition` shutdown. The backend currently
   clones workspace `Arc`s; admission closure prevents new work, but ownership/drain/resource
   teardown should be one explicit joined lifecycle.
8. Add a public/program entry that accepts a fully built composition and ports, constructs
   `ProgrammaticSemanticQueryBackend`, supplies exact claims/registry, and joins composition
   shutdown with daemon shutdown.
9. Remove the production `WorkspaceQueryBackend::default()` and `LegacyBootstrap` selection from
   `serve()`. Keep any predecessor runner test-only until WP30 deletion; missing exact construction
   inputs must fail startup.
10. Ensure every production command routes through the registered command runtime manager. The
    current programmatic admin path deliberately rejects legacy mutation ingress; it still needs the
    concrete programmatic command ingress called for by the design.

### Step B — WP29 proof cases

Use real typed inputs and temporary Delta/SQLite roots to prove:

- cold start and exact restart reconstruction;
- one source/request input change causally changes the compiled plan/result;
- a missing catalog, release pin, durable root/version, policy, credential, or command port fails
  workspace admission;
- multi-workspace route/provider/result isolation;
- activation during compile cannot mix epochs;
- request-owned input affects the result and never enters an epoch catalog;
- repeated/multi-block request inputs remain partitioned by output/query;
- scope changes alter or reject authorization;
- partial construction rolls back opened admission/command resources;
- cancellation before planning, during execution, and after publication is bounded and leak-free;
- ordered drain/restart retains no default/bootstrap backend.

Only then run and close the four WP29 oracle categories.

### Step C — execute WP30 immediately after the positive vertical

Inventory production consumers again, then delete rather than merely orphan:

- `WorkspaceQueryBackend::default` and bootstrap fabric selection;
- live model compiler/importer/replay readers;
- generated schema/current/registry authorities and dual epoch paths;
- ontology package/bundle/candidate runtime selection;
- legacy semantic executor registration and form-selected runtime catalogs;
- model-backed snapshot/current pointers and direct durable mutation/publication paths.

Preserve immutable historical documents/contracts only where the plan explicitly classifies them as
history. Run consumer and symbol zero-state scans before claiming deletion closure.

### Step D — complete WP31 and WP32 in their allowed parallel order

For WP31, finish provider/plan-derived schema closure, five self-observed histories/fixed point,
complete authorized child dependencies, native capability-rung evidence, and bounded cache
collision/reuse/eviction/fresh-physical-plan tests. Decide whether request-local logical-plan reuse
is worth implementing; until the entire provider/content authority is in the key and validator,
retain the current explicit bypass.

For WP32, complete durable history classification, retention/maintenance preconditions and proof,
candidate-free recovery, activation exact-vector integration, and fault tests for uncertain commit,
checkpoint, missing history/statistics, CDF gaps, and restart. Reconcile public types after WP30
before declaring WP32 complete.

### Step E — continue the remaining v3 dependency graph

Resume the plan's dependency order after WP31/WP32. Do not infer that the large semantic compiler or
Delta diff proves provider generation, application analyses, serving, lifecycle, or final release.
Each remaining packet needs its own causal/fault/resource evidence and eventual terminal
certification.

## 8. Specific review risks on restart

Before treating the new backend as settled, review these points deliberately:

- The concrete ingress/scope/snapshot ports do not yet exist.
- Production `serve()` still selects the predecessor default backend.
- The backend's cloned workspace handles need explicit joined shutdown ownership.
- Returned authorization pins are not yet revalidated against the compiled handoff.
- Workspace source/policy catalog pins should be tied to explicit release/activation inputs, not
  merely checked for catalog-to-catalog equality.
- Multi-block request-input partitioning was added late in the run and needs focused positive and
  negative tests.
- The request-owned child path intentionally rejects unused inputs; prove compiler handoff grouping
  matches this invariant for every supported program.
- Query artifact evidence still contains predecessor-shaped plan-artifact structures; do not fill
  those with fabricated model/snapshot values. Reshape them under the relevant later packet.
- The final Delta additions type-check, but the last runtime/fault suite did not complete on the
  combined tree before restart.
- No legacy deletion or terminal plan oracle has yet been certified.

## 9. Completion claim boundary

The correct restart claim is:

> A large, coherent WP29 programmatic query vertical and significant WP31/WP32 foundations are now
> present in the dirty working tree. They are formatted and partially type-checked, with exact
> Arrow/DataFusion/Delta authority improvements, but production cutover, concrete application
> ports, end-to-end functional proof, WP30 deletion, and all later packets remain open.

Anything stronger would overstate the available evidence.
