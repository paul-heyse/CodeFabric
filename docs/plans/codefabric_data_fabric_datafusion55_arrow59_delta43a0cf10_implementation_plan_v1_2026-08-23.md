---
artifact: implementation-plan
plan_id: codefabric-data-fabric-datafusion55-arrow59-delta43a0cf10
version: v1
date: 2026-08-23
status: approved
design_path: docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md
design_version: v1.3
baseline_commit: d89cc90cd2e51c4b716b2a4da2c0a8d6f79d5409
working_tree_digest: 29889c91ea4d38e7c8d1a76d0b2aaca8e22061376a7cb827fe79d9e5a1cf927c
state_path: docs/plans/state/codefabric-data-fabric-datafusion55-arrow59-delta43a0cf10_v1_state.json
cutover: true
---

# CodeFabric DataFusion 55, Arrow 59, and delta-rs 43a0cf10 pivot — implementation plan v1

This plan moves every live CodeFabric data-fabric authority from DataFusion 54.1.0,
Arrow/Parquet 58.4.0, and delta-rs revision
`9f9223197469897ef05ae4369eb4fd1390174e65` to DataFusion 55.0.0,
Arrow/Parquet 59.2.0, and exact delta-rs revision
`43a0cf10a313e5077c48637ad786a05359136bbb`. `object_store` remains exactly 0.13.2,
and CodeFabric's Rust floor remains 1.95.0.

The two 2026-08-23 comprehensive references named in the declared inputs are the
version-specific library authorities for this pivot. The existing DataFusion 54/Arrow 58 and
delta-rs `9f922319` references and their current skill routes are legacy inputs only; they were
not used as API authorities for this plan and must cease to be live routing authorities during
the cutover.

## 1. Outcome and non-goals

### 1.1 Outcome

At M05, CodeFabric has one coherent target stack:

1. all direct Arrow and Parquet dependencies in the stable root resolve at 59.2.0, DataFusion
   resolves at 55.0.0, `object_store` resolves at 0.13.2, and delta-rs resolves from the exact
   `43a0cf10a313e5077c48637ad786a05359136bbb` Git source;
2. the dated-nightly extractor emits Arrow 59.2.0 IPC that the stable root decodes without schema,
   nullability, value, identity, or deterministic-checksum drift;
3. the local-workstation graph remains free of `deltalake-aws` and AWS SDK authority, while the
   explicit `s3-storage` graph retains its designed cloud boundary;
4. existing Delta tables produced by the old stack remain readable and mutable under the target
   stack, exact-version serving remains snapshot-isolated, and the rollback compatibility oracle
   establishes whether an old-stack reader can consume target-stack output without new protocol
   features;
5. DataFusion query results, schemas, allowlist behavior, resource bounds, cancellation, and fact
   semantics remain invariant even when plan/operator diagnostics legitimately change;
6. mutation, application-transaction, publication, recovery, checkpoint, optimize, pruning, and
   vacuum-safety contracts continue to satisfy FAB §112.6; and
7. current specifications, repository instructions, model-owned identities, security policy,
   graph validators, fixtures, and agent routing all name and prove the target stack.

### 1.2 Non-goals

- Do not adopt DataFusion 55 range partitioning, `MERGE INTO`, higher-order UDFs, pluggable spill
  storage, or other new capability merely because the upgrade exposes it.
- Do not enable Delta Change Data Feed, deletion vectors, type widening, V2 checkpoints, column
  mapping, in-commit timestamps, or another protocol/table feature.
- Do not add custom `ExecutionPlan`, `QueryPlanner`, `PhysicalPlanner`, UDF/UDAF, FFI codec, or
  external-table DDL work unless the target compiler or a complete execution-time census finds a
  real current consumer.
- Do not add a second top-level Rust integration-test target, another Cargo root, a native Python
  extension, or a Python Arrow/DataFusion processing layer.
- Do not enable delta-rs internal write retries. CodeFabric continues to own coordination,
  application transactions, unknown-outcome reconciliation, and retry policy.
- Do not introduce dual-version source shims. A checksum-format incompatibility or persisted-state
  incompatibility is a replan event requiring an explicit versioned migration design.
- Do not rewrite completed plans, state files, audits, reviews, or legacy version-pinned reference
  documents. They remain historical evidence after live routing moves away from them.
- Do not invent production vacuum orchestration or `num_retries` telemetry. The current vacuum
  dry-run safety contract is re-proved; newly available metrics remain deferred until a contract
  owns them.

### 1.3 Current disposition and planning baseline

The target library documents and other pre-existing user files are untracked at the planning
baseline. Execution must first make the two declared target references reproducible repository
inputs; no packet may claim a reproducible target-pin proof while they remain untracked.

The planning-session `just ci-fast` baseline was attempted both in the managed sandbox and with
approved escalation. Both attempts stopped before compilation because `sccache` returned
`Operation not permitted (os error 1)`. This is a harness-level baseline obstruction, not evidence
of a product failure or a green baseline. WP01 must retry the baseline in an environment where the
committed `sccache` wrapper can execute and record any real failures in execution state.

No execution-state file is created by this planning artifact. Execution begins only after this
draft is approved and the state file is initialized by the execution workflow.

## 2. Source design and declared inputs

The source design is FAB v1.3, especially FAB §2, §2.1, §2.2, §12.5–12.9,
§98, §100.1, §101, §103.4, §111.1, and §112.6. The suite-wide governing
constraints are SUITE §83.6 and §83.7. The user-approved target versions in this plan
supersede only the old version-retention decisions; they do not supersede the architecture,
authority, isolation, snapshot, or evidence requirements in those artifacts and accepted designs.

For this plan:

- **DF55/AR59 reference** means
  `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`, with its exact-source
  precedence layer in §40A and its V1–V5 upgrade gate taking precedence over stale illustrative
  version strings elsewhere in that large document.
- **DELTA43 reference** means
  `deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md`, with its
  exact target banner and Cargo snippets taking precedence over the Arrow/Parquet `58` typo in its
  alignment matrix.

| path | sha256 |
|---|---|
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 81a6ea7baa3eb4229802acfba0c538051de27bc9dfaa026f174ce0422cc6e3ff |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 0cacf3cbb3e0abe2b5e3f358b59ac198066d721c56761219905208231e32e7c6 |
| docs/designs/codefabric_local_storage_dependency_isolation_design_v1_2026-08-20.md | 9251cfbc4fcd23db6858c0bf0ead1b05dc675833e90e62816f8cd3298b1e7745 |
| docs/designs/codefabric_build_cache_and_feature_isolation_design_v1_2026-08-20.md | 460e2f36a8a61a9972c976adbd20e1a86b82a26cd7bd38840cb1565948475e8f |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |

This table is immutable after approval. Updating FAB's version authority is a planned input
evolution owned by WP02 and must be recorded as such in execution state; it is not permission to
restamp this planning-time digest.

## 3. Governing library decisions

### LD-01 — DataFusion upgrade

**Decision:** upgrade
**Version basis:** DataFusion 55.0.0 with DF55/AR59 reference §40A and its V1–V5 gate
**Displaces:** DataFusion 54.1.0 in the root manifest, lock, active documentation, validators,
fixtures, compatibility probes, graph checks, and routing; application-owned query and snapshot
boundaries are retained
**Risk:** source-compatible traits can still lose structured statistics or change optimizer and
cardinality behavior; explicit provider-contract, result-equivalence, pruning, and resource tests
mitigate that risk
**Validation:** `just data-fabric-upgrade-check`

### LD-02 — Arrow and Parquet upgrade

**Decision:** upgrade
**Version basis:** Arrow and Parquet 59.2.0 across the stable root and dated-nightly extractor
**Displaces:** every direct 58.4.0 Arrow/Parquet dependency and the extractor's Arrow 58 IPC
producer; `object_store` remains exactly 0.13.2
**Risk:** `RowConverter` bytes participate in durable `codefabric-arrow-batch-v1`, table,
primary-key, overlay, and query-result checksums; pre-upgrade KATs and cross-version IPC/persisted
fixtures make any byte drift a replan trigger rather than a golden refresh
**Validation:** `just data-fabric-stack-compat <wp01-baseline-ref> <target-ref>`

### LD-03 — delta-rs exact revision upgrade

**Decision:** upgrade
**Version basis:** exact Git revision `43a0cf10a313e5077c48637ad786a05359136bbb`, resolving
DataFusion 55.0.0, Arrow/Parquet 59.2.0, `object_store` 0.13.2, and the released
`buoyant_kernel`/`buoyant_kernel_engine` 0.25.x Arrow-59 line
**Displaces:** exact revision `9f9223197469897ef05ae4369eb4fd1390174e65`; kernel crates
remain transitive and are not direct-pinned
**Risk:** lazy snapshot replay, statistics, checkpoint refresh, DML metadata, retry reporting,
maintenance, and schema adaptation changed across the revisions; FAB §112.6 becomes an executable
cross-version gate and protocol features remain fail-closed
**Validation:** `just data-fabric-upgrade-check`

### LD-04 — Preserve the application boundary

**Decision:** wrap
**Version basis:** CodeFabric application-owned DTOs, providers, transaction coordinator, snapshot
catalog, serving runtime, and local/S3 feature split over the target stack
**Displaces:** no application boundary; no raw-Parquet bypass, provider type leakage, internal
delta retry ownership, or multi-version compatibility shim is introduced
**Risk:** opportunistic adoption could change fact or transaction semantics under cover of an
upgrade; focused zero-state checks and result/persistence equivalence keep the change infrastructural
**Validation:** `just governance`

## 4. Global target invariants

1. **One coherent type universe.** The stable root contains one Arrow/Parquet 59.2.0,
   DataFusion 55.0.0, `object_store` 0.13.2, and target delta-rs universe. The extractor uses
   Arrow 59.2.0 at its IPC producer boundary. No active Arrow 58 package, feature, or type crosses
   either boundary.
2. **Exact source, not moving main.** delta-rs source identity is the full `43a0cf10...` commit.
   The legitimate delta-core/DataFusion `sqlparser` 0.61/0.62 split is documented and allowed;
   duplicate type-bearing Arrow/DataFusion/object-store/kernel universes are not.
3. **Stable authority boundaries.** Local workstation builds retain local provider authority and
   omit `deltalake-aws`/AWS SDK packages; only `s3-storage` activates the explicit S3 graph.
4. **No fact semantic change.** Gate-B fact differentials, canonical row identities, schemas,
   nullability, ordering, batch checksums, primary-key digests, query-result checksums, and rebuild
   equivalence remain exact. A `RowConverter` byte difference is not automatically acceptable.
5. **Atomic present state.** Each query pins one exact immutable snapshot and exact Delta versions.
   Provider caches, lazy materialization, statistics, and same-version checkpoint arrival do not
   become semantic identity.
6. **Delta is read through Delta.** Query serving uses delta-rs schema and physical adaptation;
   no consumer substitutes raw Parquet reads for Delta protocol/state interpretation.
7. **Statistics are explicit.** Query-serving handles retain `skip_stats=false`; metadata-only
   profiles and `without_files` behavior remain deliberate. The two custom `TableProvider`
   wrappers explicitly forward or reject DataFusion 55 structured scan/statistics requests; no
   default silently erases known statistics or invents them for the effective overlay.
8. **Coordinator-owned writes.** `CommitProperties::with_max_retries(0)`, commit metadata,
   application transactions, predecessor checks, exactly-once reconciliation, and unknown-outcome
   recovery remain CodeFabric-owned.
9. **Features fail closed.** Reopening a table rejects unexpected CDF, deletion vectors, type
   widening, or other unapproved protocol/table features. Target writes do not raise reader/writer
   protocol versions or enable a feature.
10. **Generated outputs have one writer.** Manifest/lock/version source changes flow through
    `model-plan` and confirmed `model-sync`; generated schema validation and toolchain identity are
    never edited directly.
11. **Diagnostics are not semantics.** DataFusion logical/optimized/physical plan strings and
    decimal formatting may be deliberately rebaselined only after result, pruning intent,
    resource, cancellation, and snapshot invariants pass.
12. **Rollback preserves namespaces.** An old binary never writes a target-stack namespace.
    Rollback uses a preserved old-stack namespace; protocol-feature activation and vacuum of
    rollback-required files remain frozen through the rollback window.
13. **Performance is measured at the boundary.** Activation, first query, warmed filtered/full
    queries, owner replacement/publication, and checkpoint reopen record latency and peak RSS.
    The target fails the comparison when its median or peak RSS is more than 10% worse and the
    95% bootstrap interval excludes parity; a correctness or resource-ceiling breach fails
    regardless of statistical significance.
14. **Live authority reaches zero.** Active manifests, locks, code, scripts, model inputs and
    outputs, current specifications/indexes, current skills, repository instructions, fixtures,
    and security policy contain no old-stack authority after DB01/DB02. Historical artifacts and
    the pre-existing untracked user tree are not silently rewritten.

## 5. Packet contract

Each packet begins by rerunning its preflight against the current tree. Known-touch paths below
are planning-session evidence, not an exhaustive must-touch manifest. A newly discovered consumer
is absorbed into the dependency-closed packet unless it changes architecture, persistence,
protocol, or public semantics, in which case the executor stops and replans.

Completion requires every named acceptance check at the packet's proving commit and again at
HEAD. Mutating `model-sync` and any snapshot acceptance remain deliberate, diff-reviewed actions;
no mutating recipe is embedded in a read-only gate.

### WP01 — Freeze the old-stack compatibility and performance baseline

**Outcome:** Before any dependency pin changes, the repository contains reusable, stack-neutral
oracles and fixtures that distinguish a successful compiler upgrade from persisted, checksum,
protocol, or performance drift. The exact two target references are tracked inputs.

**Dependencies:** approved plan; a usable `sccache` execution environment.

**Target invariants and design references:** FAB §112.6; SUITE §83.6–83.7; LD-02,
LD-03; P25 reproducibility, P30 testability, and P31 executable governance.

**Preflight query:**

```bash
git status --short
just ci-fast
just wave3-integration-check
just extractor-ci-fast
just stable-graph-check
just policy
rg -n --glob '*.rs' \
  'RowConverter::new|convert_columns|codefabric-arrow-batch-v1|result_checksum|table_checksum|primary_key_digest' \
  src tests rustc-extractor/src
rg -n --glob '*.rs' -i \
  'cdf|change data feed|deletion vector|vacuum|with_skip_stats|without_files|num_retries|merge_schema|DeltaOps' \
  src tests rustc-extractor/src
```

**Known touch (verified this session):** `justfile`, the existing `tests/integration.rs` target and
its `tests/integration/` cases, `src/fabric/mutation.rs`, `src/fabric/publication.rs`,
`src/fabric/snapshot_catalog.rs`, `src/fabric/serving.rs`, `rustc-extractor/src/wrapper.rs`, and a
new justified reusable non-code fixture area under `tests/fixtures/`.

**Required changes and legacy disposition:**

- Add `just data-fabric-stack-compat <baseline-ref> <target-ref>`, a read-only cross-revision
  orchestrator that uses isolated temporary worktrees/artifacts and the existing single
  integration-test target. WP01's proving commit becomes `<baseline-ref>` so both revisions own
  the same producer/consumer contract.
- Commit a small old-stack Delta/Parquet fixture with multiple exact Delta versions, commit
  metadata and application-transaction evidence, nullable/nested columns, empty and non-empty
  batches, and protocol/table-feature metadata. Include binary IDs, strings, floats, timestamps,
  lists, and nulls in Arrow checksum KATs.
- Capture old-stack `batch_checksum`, primary-key, provider-content, overlay, and query-result
  digests. The fixture is an intentional trigger for the otherwise-absent `tests/fixtures/`
  directory, not a new test crate.
- Add old-write/new-read and target-write/old-read modes. The old reader never writes the target
  namespace; target-write/old-read exists only to prove the rollback/readability envelope.
- Add `just data-fabric-upgrade-bench <baseline-ref> <target-ref>` over a reusable workload in the
  same integration target. Record environment, repeated samples, the invariant resource ceilings,
  and the predeclared comparator from global invariant 13 before target measurements exist.
- Track the exact two target reference documents. Do not modify any other pre-existing untracked
  user material.

**Acceptance checks:**

- `data_fabric_54_arrow58_delta9f_persisted_baseline` creates, reopens, queries, mutates, restarts,
  and validates the old fixture at exact versions.
- `arrow58_codefabric_batch_checksum_kat` fixes the application-visible checksum bytes and covers
  null, nested, floating, binary, timestamp, empty, and order-independent cases.
- `extractor_arrow58_ipc_baseline` fixes schema, nullability, row, and deterministic
  canonicalization evidence across the extractor/root boundary.
- `just data-fabric-upgrade-bench <wp01-proving-commit> <wp01-proving-commit>` validates the
  benchmark harness against itself and emits no correctness differential.

**Edit-local gates:** `just root-test`, `just extractor-ci-fast`, `just wave3-integration-check`.

**Packet-local gates:** `just ci-fast`, `just stable-graph-check`, and the four acceptance checks.

**Integration milestone:** M01.

**Replan triggers and rollback/recovery:** Stop if the old stack cannot reproduce its own persisted
fixture or checksum KAT, if the benchmark is not stable enough to apply the comparator, or if a
real baseline failure appears behind the prior `sccache` obstruction. WP01 changes no pins; revert
only its unproved harness/fixture work while retaining failure evidence.

Executable oracle: `wp01_behavioral_old_stack_fixture`
Executable oracle: `wp01_structural_single_test_target`
Executable oracle: `wp01_negative_protocol_feature_baseline`
Executable oracle: `wp01_operational_benchmark_self_compare`

### WP02 — Cut over the exact graph, version authorities, and model identities

**Outcome:** Root and extractor dependency graphs, locks, active version authorities, generated
tool identities, security policy, graph validators, and current agent routing coherently name the
target stack. The repository compiles far enough to expose exact API fallout without a dual-stack
shim.

**Dependencies:** WP01.

**Target invariants and design references:** FAB §2–2.2 and §112.6; SUITE §83.6;
DF55/AR59 reference §40A and V1–V2; DELTA43 reference §0 and §1; LD-01–LD-04.

**Preflight query:**

```bash
git status --short
rg -n \
  '54\.1\.0|58\.4\.0|9f9223197469897ef05ae4369eb4fd1390174e65|arrow-58|DataFusion 54|Arrow 58' \
  AGENTS.md Cargo.toml Cargo.lock rustc-extractor/Cargo.toml rustc-extractor/Cargo.lock \
  src tests contracts scripts tooling docs/upfront_design docs/spec_index docs/designs .claude/skills
rg -n --glob '*.rs' \
  'impl (TableProvider|ExecutionPlan|QueryPlanner|PhysicalPlanner|ExtensionPlanner|GroupsAccumulator|ScalarUDFImpl|OptimizerRule|PhysicalOptimizerRule)' \
  src tests rustc-extractor/src
just model-plan Cargo.toml Cargo.lock rustc-extractor/Cargo.toml rustc-extractor/Cargo.lock \
  contracts/fixtures/synthetic/source-syntax-canonicalization-v1.json \
  src/bin/codefabric_model/schema_driver.rs \
  docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md
```

The old-pin census deliberately uses named repository scopes rather than `rg -uu`; it must not
read `.envrc.local`. At execution, report candidate counts and ignored/parse coverage before using
an empty result as global evidence.

**Known touch (verified this session):** `Cargo.toml`, `Cargo.lock`,
`rustc-extractor/Cargo.toml`, `rustc-extractor/Cargo.lock`,
`scripts/stable_graph_check.sh`, `tooling/ci/duplicate-family-fixture/Cargo.toml`, `deny.toml`,
`tooling/security/advisory-exceptions.json`, `src/compatibility.rs`,
`src/bin/codefabric_model/schema_driver.rs`,
`contracts/fixtures/synthetic/source-syntax-canonicalization-v1.json`, FAB v1.3, `AGENTS.md`,
`docs/spec_index/README.md`, `docs/spec_index/library-routing.md`, and the two tracked current
reference-routing skills under `.claude/skills/`.

**Required changes and legacy disposition:**

- Pin all root Arrow subcrates and Parquet to 59.2.0, DataFusion to 55.0.0, `object_store` to
  0.13.2, and delta-rs to exact `43a0cf10...`. Keep `rust-version = "1.95.0"`.
- Pin extractor `arrow-array`, `arrow-ipc`, and `arrow-schema` to 59.2.0 and resolve its separate
  lock. Do not merge Cargo roots or target directories.
- Update the resolved-graph validator to prove the full target family, exact delta source,
  local/S3 activation boundary, one `buoyant_kernel`/`buoyant_kernel_engine` 0.25.x line with
  `arrow-59`, and absence of `arrow-58`. Preserve and explain the expected delta-core/DataFusion
  `sqlparser` 0.61/0.62 split.
- Move the duplicate-family fixture's primary family to Arrow 59.2.0 while retaining a deliberately
  distinct legacy family that cannot be mistaken for an active Arrow 58 authority.
- Re-run RustSec, bans, sources, and unused-dependency checks against the resolved lock. Remove
  stale advisory exceptions from both registry and deny policy; any retained or new exact
  exception requires an accountable owner/rationale. Do not mechanically rename old exception
  prose.
- Adapt only compile-proven DF55/delta43 API changes. Planning found exactly two custom
  `TableProvider` implementations and no custom execution plan, planner, UDF/UDAF, FFI codec, or
  external-table surface; rerun the census before preserving that negative claim.
- Update FAB's canonical baseline, version-alignment invariant, and upgrade-gate wording; update
  current repository instructions and derived navigation. Preserve old accepted designs as v1
  history and create versioned successors where their active decisions name Arrow 58 or the old
  kernel/revision.
- During execution, use the `skill-creator` workflow to update the two current `.claude/skills/`
  navigation skills and their references to the new documents. `.agents/skills` and
  `.codex/skills` are symlinks and receive no duplicate edits.
- Update authored version sources and compatibility probes. Extend the synthetic fixture decoder
  to assert `arrow_version == arrow::ARROW_VERSION`; it must no longer ignore that field.
- Run read-only `model-plan`, then confirmed `model-sync` as the sole writer. Inspect the complete
  manifest/lock digest cascade and require a zero-action follow-up plan; never edit
  `contracts/generated/model/schema/schema-validation.json` or toolchain identity directly.

**Acceptance checks:**

- `stable_dependency_contract_is_executable` resolves and compiles all target public type families.
- `extractor_arrow59_ipc_contract` compiles the producer and root consumer on the same 59.2.0 IPC
  and schema contract.
- `target_graph_source_and_feature_contract` proves exact versions/source, one type universe,
  kernel Arrow-59 selection, local/S3 separation, and the documented `sqlparser` exception.
- `model_toolchain_identity_tracks_data_fabric_pins` proves fixture, validator identities, locks,
  and G-07 provenance derive from the authored target sources.

**Edit-local gates:** `just root-check`, `just extractor-ci-fast`, `just model-family-check schemas`,
and `just advisory-policy-check`.

**Packet-local gates:** `just stable-graph-check`, `just duplicate-family-check`,
`just features-each`, `just deps-fast`, `just policy`, `just model-repro-check`, and
`just model-release-check`.

**Integration milestone:** M02.

**Replan triggers and rollback/recovery:** Stop if resolution produces duplicate Arrow/DataFusion/
object-store families, a different delta source, split kernel lines, unexpected local cloud/AWS
authority, a lower Rust floor requirement, or an API change that requires a new application
boundary. Before any target write, rollback is the WP01 proving commit and its old namespace.
After a target write, apply WP05's compatibility and namespace rules; never restore only
`Cargo.lock` and assume state compatibility.

Executable oracle: `wp02_behavioral_target_compile`
Executable oracle: `wp02_structural_exact_graph`
Executable oracle: `wp02_negative_old_family_graph`
Executable oracle: `wp02_operational_model_identity`

### WP03 — Preserve Arrow IPC, canonical checksums, and DataFusion provider/query contracts

**Outcome:** Arrow 59 bytes and DataFusion 55 runtime behavior preserve application-owned schemas,
rows, durable digests, query results, and resource semantics. The two custom table-provider
wrappers have explicit DF55 structured-scan/statistics behavior.

**Dependencies:** WP02.

**Target invariants and design references:** FAB §103.4 and §112.6; DF55/AR59 reference
§40A.3–40A.6 and V3–V5; LD-01, LD-02; P1 information hiding, P16 contracts, P22 resource
lifecycle, and P30 testability.

**Preflight query:**

```bash
rg -n --glob '*.rs' \
  'impl (TableProvider|ExecutionPlan|QueryPlanner|PhysicalPlanner|ExtensionPlanner|GroupsAccumulator|ScalarUDFImpl)' \
  src tests rustc-extractor/src
rg -n --glob '*.rs' \
  'ScanArgs|scan_with_args|supports_filters_pushdown|statistics\(|RowConverter::new|codefabric-arrow-batch-v1' \
  src tests rustc-extractor/src
rg -n --glob '*.rs' \
  'logical_plan|optimized_logical_plan|physical_plan|DeltaScanExec|pruning_predicate|pushdown_filters|FairSpillPool|cancel' \
  src tests
```

**Known touch (verified this session):** `src/fabric/snapshot_catalog.rs` contains
`OverlayIdentityProvider`; `src/fabric/overlay.rs` contains `OverlayEffectiveProvider`;
`src/fabric/serving.rs` owns query evidence/resource behavior; `src/fabric/mutation.rs`,
`publication.rs`, `snapshot_catalog.rs`, and `serving.rs` consume `RowConverter` bytes;
`rustc-extractor/src/wrapper.rs` and the root compatibility integration path own IPC.

**Required changes and legacy disposition:**

- Compile-probe the exact DF55 `TableProvider::scan`, `scan_with_args`, `ScanArgs`, `ScanResult`,
  and statistics signatures rather than relying on illustrative prose. Keep required `scan`.
- Make `OverlayIdentityProvider` forward `scan_with_args` and structured statistics requests to
  its wrapped Delta provider when supported. Add a spy-provider test that fails if projection,
  filters, limit, or statistics requests are dropped.
- Give `OverlayEffectiveProvider` an explicit disposition: forward only what its materialized
  provider can prove, or retain the compatibility fallback with `statistics() == None`. Never
  claim statistics it cannot own.
- Inspect target `SessionConfig` and Parquet defaults, then explicitly configure or capture the
  chosen filter-pushdown posture. The target references conflict about the default, so no implicit
  default is assurance evidence.
- Compare extractor IPC and representative root RecordBatches against WP01 schema, nullability,
  value, ordering, and canonicalization evidence.
- Run every WP01 checksum KAT. If Arrow 59 changes `RowConverter` bytes, stop: design an
  application-owned/versioned canonical row encoding and dual-read persisted migration before
  proceeding. Do not accept new checksum snapshots.
- Rebaseline DataFusion version-stamped logical, optimized, and physical diagnostics only after
  query schemas, ordered rows, result checksums, allowlist decisions, plan intent, cancellation,
  memory/spill ceilings, and exact snapshot bindings remain invariant.
- Do not implement DF55 `apply_expressions`, `PhysicalPlanningContext`, UDF/UDAF, or planner work
  unless the compiler or rerun structural census finds a real implementation.

**Acceptance checks:**

- `datafusion_55_table_provider_scan_contract` covers projection, filter, limit, structured
  statistics requests, and both wrappers' explicit behavior.
- `arrow59_codefabric_batch_checksum_kat` must exactly match every WP01 checksum and identity KAT.
- `extractor_arrow59_root_decode_equivalence` compares IPC schema, nullability, values, rows, and
  canonicalization with the old-stack fixture.
- `datafusion_55_serving_equivalence` compares result semantics, pruning intent, exact source
  versions, memory/spill evidence, cancellation, and reviewed plan diagnostics.

**Edit-local gates:** `just root-check`, `just root-clippy`, `just root-test`, and
`just extractor-ci-fast`.

**Packet-local gates:** `just data-fabric-upgrade-check`, `just wave3-integration-check`,
`just wave5-integration-check`, and `just gate-b-check`.

**Integration milestone:** M03 with WP04.

**Replan triggers and rollback/recovery:** Replan on any checksum byte drift, Arrow IPC semantic
drift, query row/schema/checksum change, dropped statistics request, unexpected cardinality change,
resource-ceiling breach, or requirement for a new custom execution/planner interface. Restore the
last old-stack namespace for runtime rollback; retain target diagnostics and fixtures for analysis.

Executable oracle: `wp03_behavioral_query_equivalence`
Executable oracle: `wp03_structural_provider_forwarding`
Executable oracle: `wp03_negative_checksum_drift`
Executable oracle: `wp03_operational_resource_cancellation`

### WP04 — Preserve delta-rs snapshot, provider, statistics, cache, and checkpoint semantics

**Outcome:** The target delta-rs provider reopens old state and serves exact versions with the same
snapshot, lazy/eager, statistics, pruning, cache, restart, and checkpoint semantics required by FAB
§112.6.

**Dependencies:** WP02; WP03 provider contract available.

**Target invariants and design references:** FAB §12.5–12.9, §98, §103.4, and
§112.6; DELTA43 reference §3.28 and §6.36; LD-03, LD-04; P5 dependency direction,
P22 resource lifecycle, and P25 reproducibility.

**Preflight query:**

```bash
rg -n --glob '*.rs' \
  'DeltaTableBuilder|with_skip_stats|without_files|with_version|table_provider|DeltaScanExec|pruning_predicate|checkpoint' \
  src tests
rg -n --glob '*.rs' \
  'SnapshotCache|provider_cache|exact_provider|metadata_only|query_serving|skip_stats' \
  src tests
just model-explain contracts/generated/model/schema/schema-validation.json
```

**Known touch (verified this session):** `src/fabric.rs` owns `DeltaHandleFactory`, open profiles,
exact providers, schema checks, and table validation; `src/fabric/snapshot_catalog.rs` owns private
catalog/provider caches; `src/fabric/serving.rs` owns `DeltaScanExec`/pruning evidence.

**Required changes and legacy disposition:**

- Reopen the WP01 old-stack fixture with full statistics, metadata-only, and intended
  `without_files` profiles. Query-serving exact providers retain `skip_stats=false`.
- Prove exact-version binding before and after process restart, lazy/eager equivalence, snapshot
  reconstruction, root/version cache separation, and failure on a cache/root/version mismatch.
- Prove same-version checkpoint arrival is identity-neutral while cached/materialized state may be
  refreshed. Verify this against the exact pinned SHA because the DELTA43 prose describes nearby
  tip behavior ambiguously.
- Treat fresh full-stat reloads as the supported policy. Do not invent same-handle `skip_stats`
  mutation if the target API does not support it.
- Through the DF55 structured scan contract, prove projection/filter/limit pushdown and statistics
  requests reach the Delta provider and preserve pruning. Measure open/activation cost separately
  from first-query and warmed-query cost.
- Continue using delta-rs schema and physical adaptation for nested/logical types. Add no raw
  Parquet shortcut.
- Strengthen `validate_open_table` so unexpected deletion-vector configuration or protocol
  features fail closed in addition to CDF and type widening. This makes target CDF/DV changes a
  proven non-consumer exemption, not an assumption.

**Acceptance checks:**

- `delta_43a0cf10_snapshot_replay_contract` covers exact version, lazy/eager replay, skip-stats
  profiles, restart, and checksum identity.
- `delta_43a0cf10_checkpoint_identity_contract` covers same-version checkpoint refresh and cache
  isolation without semantic-identity drift.
- `delta_43a0cf10_provider_pruning_contract` covers structured statistics, projection/filter/limit,
  `DeltaScanExec`, and pruning on query-serving handles.
- `delta_43a0cf10_unapproved_feature_rejection` rejects CDF, deletion vectors, type widening, and
  any unexpected protocol/table feature on reopen.

**Edit-local gates:** `just root-check`, `just root-test`, and `just wave3-integration-check`.

**Packet-local gates:** `just data-fabric-upgrade-check`, `just wave5-integration-check`,
`just rebuild-equivalence-check`, and `just vacuum-dry-run-check`.

**Integration milestone:** M03 with WP03.

**Replan triggers and rollback/recovery:** Replan if lazy/eager state differs, same-version
checkpoint arrival changes identity, pruning or statistics disappear, cache keys can mix roots or
versions, schema adaptation changes facts, or an old exact version cannot reopen. Rollback serves
the preserved old namespace; do not point the old runtime at target-write state until WP05 proves
the cross-version reader oracle.

Executable oracle: `wp04_behavioral_snapshot_equivalence`
Executable oracle: `wp04_structural_delta_provider_path`
Executable oracle: `wp04_negative_feature_cache_mismatch`
Executable oracle: `wp04_operational_checkpoint_restart`

### WP05 — Preserve Delta mutation, publication, idempotency, maintenance, and persisted compatibility

**Outcome:** Target writes and maintenance retain owner replacement, application transaction,
commit metadata, exactly-once recovery, concurrency, schema/protocol, optimize, and vacuum-safety
semantics, and cross-version persisted compatibility is explicitly established.

**Dependencies:** WP01–WP04.

**Target invariants and design references:** FAB §100.1, §101–101.1, §111.1, and
§112.6; DELTA43 reference §5.13, §5.17, §9, and §13; SUITE §83.7; LD-03,
LD-04; P23 failure semantics, P24 idempotency, and P29 versioned contracts.

**Preflight query:**

```bash
rg -n --glob '*.rs' \
  'CommitProperties|with_max_retries|with_metadata|with_application_transaction|Transaction::new|transaction_version|write\(|delete\(|update\(|optimize|vacuum' \
  src tests
rg -n --glob '*.rs' \
  'unknown.outcome|reconcile|predecessor|concurrent|no.op|merge_schema|schema_mode|protocol' \
  src tests
just data-fabric-stack-compat <wp01-baseline-ref> HEAD
```

**Known touch (verified this session):** `src/fabric/mutation.rs` owns prepare/apply/delete and
checksums; `src/fabric/publication.rs` owns append/publication/recovery; `src/fabric.rs` owns reload,
open validation, write setup, and optimize/vacuum-facing policy; operational-state tests own
application transaction and unknown-outcome reconciliation.

**Required changes and legacy disposition:**

- Compile-probe every load-bearing delta API, including APIs not covered by the DELTA43 prose:
  `with_application_transaction`, `Transaction::new`, `transaction_version`, write return shape,
  delete return/metrics shape, commit metadata, and predecessor/version handling.
- Preserve `with_max_retries(0)`. The new delta `num_retries` metric does not authorize internal
  retries; prove persisted retry behavior is zero/benign or explicitly mark the metric
  non-applicable without adding a contract.
- Re-run no-op delete, insert/replace/update/delete, concurrent-writer exactly-one-wins,
  predecessor mismatch, unknown-outcome reconciliation, restart/reload, and duplicate application
  transaction cases.
- Read the WP01 fixture, append/update/delete with the target, time-travel all retained versions,
  rebuild providers, and compare schema, nullability, rows, checksums, commit metadata,
  transactions, and protocol/table features.
- Produce a target-stack fixture and run the WP01 old reader against it. A pass bounds rollback
  readability only; operations still roll back to a preserved old namespace and never let the old
  binary write target state.
- Prove target writes do not enable CDF, deletion vectors, type widening, V2 checkpoints, column
  mapping, or another new feature and do not raise protocol versions. Freeze such enablement and
  deletion/vacuum of rollback-required files through the rollback window.
- Re-run nested-schema optimize, action-path handling, retention/reference indexing, and vacuum
  dry-run safety. Because there is no production `.vacuum()` orchestration today, do not invent
  one; isolate any direct target-library compatibility probe in tests.
- Do not substitute DataFusion 55 `MERGE INTO` hooks for Delta transaction semantics.

**Acceptance checks:**

- `delta_43a0cf10_mutation_recovery_contract` covers owner replacement, DML, metadata,
  application transactions, zero internal retries, concurrency, and unknown outcomes.
- `data_fabric_old_write_new_read_compatibility` reopens and mutates the old fixture under the
  target with exact semantic and protocol equality.
- `data_fabric_new_write_old_read_compatibility` proves the bounded read-only rollback envelope and
  fails if target writes raise protocol/features or change schema/nullability.
- `delta_43a0cf10_maintenance_contract` covers nested optimize, opaque action paths, retained-file
  indexing, checkpoint reopen, and vacuum dry-run safety.

**Edit-local gates:** `just root-check`, `just root-test`, `just wave3-integration-check`, and
`just vacuum-dry-run-check`.

**Packet-local gates:** `just data-fabric-stack-compat <wp01-baseline-ref> HEAD`,
`just data-fabric-upgrade-check`, `just rebuild-equivalence-check`, and
`just wave6-integration-check`.

**Integration milestone:** M04.

**Replan triggers and rollback/recovery:** Stop for changed application-transaction semantics,
non-zero hidden retries, lost commit metadata, protocol/feature elevation, checksum/schema drift,
old-state unreadability, target-state unreadability by the old reader when that envelope is
required, or unsafe maintenance candidates. Before target publication, rollback to WP01. After
target publication, quarantine the target namespace, preserve all files, and serve the preserved
old namespace until a migration/forward-fix is approved.

Executable oracle: `wp05_behavioral_mutation_recovery`
Executable oracle: `wp05_structural_coordinator_retry_ownership`
Executable oracle: `wp05_negative_protocol_feature_elevation`
Executable oracle: `wp05_operational_cross_version_compatibility`

### WP06 — Certify performance, governance, rollout, and old-authority decommission

**Outcome:** The complete target stack passes proportional repository, data-fabric, model,
security, feature, cross-version, rebuild, and performance gates; live old-stack authorities and
routes are gone; rollback evidence and operational constraints are explicit.

**Dependencies:** WP01–WP05.

**Target invariants and design references:** FAB §112.6; SUITE §83.6–83.7; all four LDs;
P25 reproducibility, P29 versioned public contracts, P30 testability, and P31 executable governance.

**Preflight query:**

```bash
git status --short
just data-fabric-upgrade-check
just data-fabric-stack-compat <wp01-baseline-ref> HEAD
just data-fabric-upgrade-bench <wp01-baseline-ref> HEAD
rg -n \
  '54\.1\.0|58\.4\.0|9f9223197469897ef05ae4369eb4fd1390174e65|arrow-58|DataFusion 54|Arrow 58' \
  AGENTS.md Cargo.toml Cargo.lock rustc-extractor/Cargo.toml rustc-extractor/Cargo.lock \
  src tests contracts scripts tooling docs/upfront_design docs/spec_index docs/designs .claude/skills
```

**Known touch (verified this session):** every WP02 live authority plus the new compatibility and
benchmark recipes, current plan/skill routing, and generated model outputs. Completed plans,
execution states, audits, reviews, legacy version-pinned library references, the old persisted
fixture, and pre-existing untracked user material are historical/external exclusions, not targets.

**Required changes and legacy disposition:**

- Add `just data-fabric-upgrade-check` as the stable aggregate for the named provider, checksum,
  IPC, snapshot, checkpoint, pruning, mutation, protocol-feature, maintenance, and serving tests.
  It remains read-only and does not accept snapshots.
- Compare WP01 and target workloads for activation, first query, warmed filtered/full query, owner
  replacement/publication, checkpoint reopen, and peak RSS. Apply global invariant 13, investigate
  statistically significant regressions, and treat any correctness/resource ceiling breach as a
  failure rather than a performance trade.
- Complete DB01 and DB02. The zero-state census covers tracked live authorities; retain a reviewed
  historical-exemption list so the check neither rewrites history nor becomes a blanket dirty-tree
  claim.
- Prove Gate B's empty fact differential and toolchain-identity restamp, clean rebuild equivalence,
  Wave 3–7 integration, feature isolation, dependency policy, model reproducibility/release, and
  CI. Review rather than automatically accept DataFusion plan diagnostic changes.
- Record the exact WP01 and target proving commits, preserved namespace locations, protocol-feature
  freeze, and rollback-window end in operational handoff evidence. These are execution judgments,
  not hard-coded in this immutable plan.

**Acceptance checks:**

- `data_fabric_target_stack_release_contract` runs the complete target behavioral aggregate with no
  old-version process or package in the target execution.
- `data_fabric_upgrade_performance_contract` applies the predeclared comparator and resource
  ceilings to all FAB §112.6 workloads.
- `data_fabric_old_live_authority_zero_state` proves DB01/DB02 across the complete tracked-live
  coverage envelope while allowing only reviewed historical exclusions.
- `data_fabric_gate_b_empty_differential` proves the storage-substrate change alters no fact
  meaning and binds current G-07 toolchain identity.

**Edit-local gates:** `just data-fabric-upgrade-check`, `just data-fabric-upgrade-bench
<wp01-baseline-ref> HEAD`, `just governance`, and `just model-release-check`.

**Packet-local gates:** every recipe in the final gate matrix.

**Integration milestone:** M05.

**Replan triggers and rollback/recovery:** Replan on a live old authority that cannot be removed,
an unreviewed performance regression, a non-empty fact differential, policy exception without an
owner, plan drift accompanied by semantic drift, or a rollback window that cannot preserve the old
namespace/files. If final certification fails after target writes, freeze mutation and vacuum,
quarantine the target namespace, and serve the preserved old namespace while retaining both
revision fixtures and diagnostics.

Executable oracle: `wp06_behavioral_release_equivalence`
Executable oracle: `wp06_structural_old_authority_zero_state`
Executable oracle: `wp06_negative_fact_differential`
Executable oracle: `wp06_operational_performance_rollback`

## 6. Integration milestones

### M01 — Reproducible baseline and rollback oracle

WP01 is complete. The old stack reproduces its persisted fixture, checksum/IPC KATs, and benchmark
self-comparison at a proving commit usable by later cross-revision recipes.

### M02 — Exact target graph and authority cutover

WP02 is complete. Both Cargo domains compile on their target Arrow family, the root resolves the
exact target stack and source, generated identities are reconciled by the sole writer, live
version documentation/routing names the target, and local/S3 isolation remains exact.

### M03 — Read/query equivalence

WP03 and WP04 are complete. Arrow IPC/checksums, DF55 provider/query behavior, delta snapshot/
checkpoint/cache behavior, schema adaptation, statistics, pruning, resource limits, and exact
version serving are equivalent to the accepted baseline.

### M04 — Persisted write and maintenance equivalence

WP05 is complete. Old-state/target-state compatibility bounds, transaction/idempotency/recovery,
protocol-feature closure, optimize, checkpoint, and vacuum safety are proven.

### M05 — Full target certification

WP06, DB01, and DB02 are complete. All final recipes pass at a proving commit and current HEAD,
performance stays within the predeclared envelope, Gate B has an empty fact differential, and
operational rollback evidence is complete.

## 7. Cross-packet decommission batches

### DB01 — Remove the live DataFusion 54, Arrow 58, and old delta-rs authority

**Owned by:** WP02 begins; WP06 closes.

**Zero-state scope:** tracked active manifests and locks; root and extractor source/tests; scripts
and tooling; security policy; current FAB/SUITE authority and repository instructions; spec index;
current accepted-design successors; authored fixtures; current model sources and generated outputs.

**Required zero:** no 54.1.0, 58.4.0, old full delta revision, `arrow-58`, or prose asserting
DataFusion 54/Arrow 58 is current. No target graph path resolves an old type-bearing family.

**Historical exclusions:** completed plans/state/audits/reviews; versioned v1 design predecessors;
the retained old persisted fixture and its manifest; legacy version-pinned reference documents;
explicitly historical comparison prose; pre-existing untracked user material outside the plan's
authorized edit scope.

**Proof:** `data_fabric_old_live_authority_zero_state`, `just stable-graph-check`,
`just duplicate-family-check`, and compiler evidence for both Cargo domains.

### DB02 — Remove legacy agent routing

**Owned by:** WP02 begins; WP06 closes.

**Zero-state scope:** `docs/spec_index/library-routing.md`, current `.claude/skills/` navigation
skills and reference maps, `AGENTS.md`, and `docs/spec_index/README.md`. `.agents/skills` and
`.codex/skills` are symlink consumers, not independent copies.

**Required zero:** no current task route tells an agent to use the DataFusion 54/Arrow 58 or old
delta-rs document for target-stack work. Legacy documents may remain directly addressable as
historical references but are not the default/current route.

**Proof:** `data_fabric_current_reference_routing_contract` and `just governance`.

## 8. Final gate matrix

The plan adds the first three stable recipes because no current recipe owns the cross-version or
FAB §112.6 aggregate. Final certification runs:

- `just data-fabric-stack-compat <wp01-baseline-ref> <target-ref>`
- `just data-fabric-upgrade-check`
- `just data-fabric-upgrade-bench <wp01-baseline-ref> <target-ref>`
- `just root-check`
- `just root-clippy`
- `just root-test`
- `just extractor-ci-fast`
- `just stable-graph-check`
- `just duplicate-family-check`
- `just features-each`
- `just msrv`
- `just deps-fast`
- `just advisory-policy-check`
- `just policy`
- `just model-family-check schemas`
- `just model-repro-check`
- `just model-release-check`
- `just governance`
- `just gate-b-check`
- `just rebuild-equivalence-check`
- `just vacuum-dry-run-check`
- `just wave3-integration-check`
- `just wave4-integration-check`
- `just wave5-integration-check`
- `just wave6-integration-check`
- `just wave7-integration-check`
- `just ci-fast`
- `just ci-pr`

Tier-C Miri, fuzzing, mutation testing, coverage, and performance profiling beyond the named
upgrade benchmark are not automatic gates for this safe-Rust dependency migration. They are added
only if execution discovers a parser/protocol, unsafe/concurrency, assertion-strength, or unexplained
performance risk that they can answer.

## 9. Execution sequence

1. Approve this plan, initialize its schema-2 execution state, and make WP01 ready.
2. Execute WP01 on the old stack; commit it and preserve the full proving commit as the baseline
   reference used by every cross-revision check.
3. Execute WP02 as one coherent pin/lock/authority/model-identity cutover. Do not begin target
   behavior work on a partially mixed graph.
4. Execute WP03 and WP04 after M02. They may be developed in either order after their shared
   interfaces compile, but M03 requires both and their provider/statistics decisions must agree.
5. Execute WP05 only after exact target reads and checksums are proven. Its first target write is
   the operational point after which namespace/file preservation rules apply.
6. Execute WP06 and both decommission batches. Run the final gate matrix at the proving commit and
   again at HEAD before M05.
7. Activate the target stack only after M05 and an owner-reviewed rollback handoff. End the
   rollback window only through an explicit operational decision; do not infer it from elapsed time.

## 10. Plan risks, assumptions, and replan policy

### 10.1 Accepted assumptions

- DataFusion 55.0.0 and Arrow/Parquet 59.2.0 are the exact target. The DELTA43 alignment table's
  Arrow/Parquet `58` cells are typographical errors because its exact banner, Cargo snippets, and
  the DF55/AR59 reference consistently resolve 59.2.0.
- The exact delta target is `43a0cf10a313e5077c48637ad786a05359136bbb`, not the latest moving
  branch tip.
- `object_store` remains 0.13.2; CodeFabric's Rust floor remains 1.95.0 even though the target
  delta repository has a lower upstream floor.
- The DF55 required `TableProvider::scan` remains source-compatible. `scan_with_args` and granular
  statistics are audited for semantic forwarding, not blindly adopted.
- Current-tree planning found exactly two custom `TableProvider` implementations and no custom
  execution plan/planner/UDF/UDAF/FFI/external-table surface. Execution reruns a complete census
  before relying on this negative result.
- The FastMCP adapter remains presentation-only and has no Python Arrow interoperability surface.
- CDF and deletion-vector changes are non-consumer changes only after reopen validation proves
  both remain disabled and rejects their unexpected activation.

### 10.2 Reference ambiguities resolved by executable evidence

- DF55 material disagrees about Parquet filter-pushdown defaults. Inspect the exact target
  `SessionConfig`, explicitly configure or record the posture, and prove filter/pruning behavior.
- DELTA43 wording around post-commit lazy cache behavior and same-version checkpoint refresh is
  ambiguous. Test the exact pinned SHA; do not infer behavior from nearby `main`.
- DELTA43 does not cover every CodeFabric-native application-transaction API. Compiler probes and
  WP05 runtime oracles own continued availability and semantics.
- Illustrative 58.4.0 strings remain in deep portions of the large DF55/AR59 reference. Its exact
  target banner, §40A precedence layer, and upgrade gate are authoritative for this plan.

### 10.3 Mandatory replan triggers

Stop execution and create a versioned successor plan or design when any of the following occurs:

- target resolution violates the one-universe, exact-source, kernel, object-store, local/S3, or
  Rust-floor invariants;
- `RowConverter` or IPC drift changes application-visible bytes, canonical/durable checksums,
  schema, nullability, values, or fact identities;
- old exact Delta versions cannot reopen, or target writes change protocol/table features,
  schema/nullability, transaction evidence, or the required rollback-read envelope;
- `scan_with_args` cannot be forwarded without changing overlay semantics, statistics become
  falsely known/unknown, or pruning materially regresses;
- application transaction, commit metadata, predecessor, retry ownership, or unknown-outcome
  semantics change;
- lazy/eager replay, checkpoint arrival, provider caching, exact-version binding, or raw/native
  schema adaptation changes present-state identity;
- query rows, schemas, checksums, allowlist behavior, snapshot isolation, cancellation, memory/
  spill ceilings, or fact differentials change. Plan text drift alone is not such a trigger;
- the target exceeds the predeclared performance envelope and the regression cannot be explained
  and accepted through a design-level decision;
- a new public/application boundary, Cargo root, test target, protocol feature, security exception,
  or production maintenance/telemetry contract becomes necessary; or
- the two target reference inputs cannot be tracked at their declared bytes or FAB's planned
  authority update conflicts with another governing artifact.

### 10.4 Rollback policy

Before the first target write, rollback restores WP01's proving commit and preserved old namespace.
After the first target write, rollback is namespace-based: stop target mutation and vacuum,
quarantine the target namespace, preserve all referenced files and both compatibility fixtures,
and serve the preserved old namespace with the old binary. No old binary writes new schemas or
target-stack state. A forward migration or dual-read checksum design requires a successor design
and plan; it is never improvised inside this pivot.
