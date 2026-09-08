---
artifact: implementation-status
plan_path: docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md
state_path: docs/plans/state/codefabric-real-time-cpg_v2_state.json
version: v1
date: 2026-09-07
status: complete
---

# Implementation Status: real-time CPG plan v2

## Provenance

**Judgment: implementation remains in progress; no packet currently satisfies its full completion contract.** WP77 and WP78 retain real implementation and historical proving commits, but current acceptance is stale. WP79 has substantial resource/task ownership implementation with passing focused tests and unresolved integration, oracle and storage ownership obligations. WP80–WP106 remain `not_started` in this plan's execution model; existing predecessor foundations are not counted as new packet completion.

This audit applies the `impl-status` skill to the exact requested approved v2 plan and its accepted v1 planning-contract dossier. The comprehensive review supplies the broader outcome; the unchanged v2.3 suite remains normative. The active pointer selects this plan, despite the older v5 status prose in AGENTS.md. No plan, design, active pointer, production source, fixture, gate implementation or predecessor state was edited by this audit. Report frontmatter `status: complete` means the **audit** is complete.

The audit candidate is HEAD `e24627a29ee44bc3f288eeb07197b66e6dec7c7e` plus the pre-existing working tree. `git rev-parse HEAD`, `git status --short --untracked-files=all`, and the preservation snapshot identify it; HEAD alone does not include the resource ownership work. Working files were unchanged during evidence gathering. Temporary raw logs and preservation checks are in `/tmp/codefabric-impl-status-20260907-gii_zmd6/`; the commands below are the reproducible evidence, not a requirement to retain that temporary directory.

Risk classification: report/state reconciliation, with focused executable verification of drifted Rust and assurance boundaries. No implementation repair, broad repository suite, benchmark capture, dependency amendment, deployment or new external operation was performed.

## Derived Status Snapshot

The following is **verbatim `just plan-status` output before reconciliation**:

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "df1c50c684a5e005c4b76ada9cd19d8030d9d7dd",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [
    "WP77",
    "WP78"
  ],
  "declared_input_count": 27,
  "healthy": true,
  "plan_path": "docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md",
  "stale_inputs": [],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

This derivation proves declared-input freshness, commit ancestry and name/recipe availability. `tooling/ci/artifact_contracts.py::derive_plan_status` does **not** inspect source changes after each proof, execute tests, or establish that an oracle is substantive. Thus `healthy: true` is compatible with the failures below. This is the distinction the status skill's judgment phases must resolve.

Verbatim baseline drift output from the §8 derivation command:

```bash
git diff --stat df1c50c684a5e005c4b76ada9cd19d8030d9d7dd..HEAD -- src rustc-extractor pyrefly-sidecar codefabric-cpg-mcp Cargo.toml Cargo.lock justfile .github
```

```text
 .../daemon/generated/cpg_query_service_pb2.py      |    2 +-
 .../daemon/generated/cpg_query_service_pb2.pyi     |    2 +-
 .../daemon/generated/cpg_query_service_pb2_grpc.py |    2 +-
 justfile                                           |   22 +-
 pyrefly-sidecar/Cargo.lock                         |    1 +
 pyrefly-sidecar/Cargo.toml                         |    5 +-
 pyrefly-sidecar/src/pyrefly_link.rs                |  453 +++++-
 pyrefly-sidecar/src/pyrefly_link/preparation.rs    |  621 +++++++++
 pyrefly-sidecar/src/server.rs                      |  180 ++-
 src/analysis_context.rs                            |   10 +
 src/analysis_context/inputs.rs                     |  273 ++++
 src/analysis_context/rust_context.rs               |  802 +++++++++++
 src/fabric/production_workspace_startup.rs         |  174 ++-
 .../input_observations.rs                          | 1436 ++++++++++++++++++++
 src/fabric/production_workspace_startup/inputs.rs  |  539 ++++++++
 src/fabric/proof.rs                                |   11 +-
 src/generated/codefabric.cpgd.v2.rs                |    2 +-
 src/generated/codefabric.provider.v1.rs            |    2 +-
 src/generated/codefabric.pyrefly.v1.rs             |    8 +-
 src/generated/codefabric.rustc.v1.rs               |    2 +-
 src/inventory.rs                                   |  250 +++-
 src/production_provider_recipe.rs                  |  104 +-
 src/programmatic_derived_analysis.rs               |  751 ++++------
 src/provider_admission.rs                          |  252 +++-
 src/provider_contracts.rs                          | 1051 +++++++++++++-
 src/provider_contracts/inputs.rs                   | 1056 ++++++++++++++
 src/provider_native_syntax.rs                      |  344 ++++-
 src/provider_raw_kinds.rs                          |    2 +-
 src/pyrefly_service.rs                             |  446 +++++-
 src/python_context.rs                              |  944 ++++++++++++-
 src/ruff_adapter.rs                                |   25 +-
 src/rust_compilation_trust.rs                      |  594 +++++++-
 src/rustc_service.rs                               |  136 +-
 src/semantic_release.rs                            |   88 +-
 src/semantic_release/independent.rs                | 1122 +++++++++++++++
 src/source_image.rs                                |  219 ++-
 src/source_image/inventory_capture.rs              |  960 +++++++++++++
 src/tree_sitter_adapter.rs                         |    4 +-
 38 files changed, 12003 insertions(+), 892 deletions(-)
```

This committed baseline diff excludes unstaged and untracked work. For the drifted WP78 consumer boundary, the current-tree derivation is:

```bash
git diff --stat 0dd1fe19568164fc63da5bba8db9df789df274f7 -- src/source_image.rs src/source_image/inventory_capture.rs src/analysis_context.rs src/python_context.rs src/provider_contracts.rs src/provider_contracts/inputs.rs src/provider_admission.rs src/production_provider_recipe.rs src/fabric/production_workspace_startup.rs
```

Its changed contracts are directly implicated by the compiler diagnostics below. The new `src/resource_budget.rs`, `src/cancellation/owned.rs` and related untracked modules also belong to the inspected working candidate; they are not evidence at the recorded proving commits.

Oracle availability was reconstructed with the repository's own `load_plan` and `_definition_commands` routines, without executing future packets. WP77/WP78 resolve; WP79 fails substance validation; WP80–WP106 each lack all four definitions. Reproduce the discovery without changing state:

```bash
scripts/repo-shell.sh -c 'PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python -' <<'CHECK'
from tooling.ci.real_time_cpg_assurance import ROOT, load_plan, _definition_commands, AssuranceError
from tooling.ci.plan_assurance import PlanAssuranceError
plan = load_plan(ROOT)
for packet in plan.contracts:
    try:
        _definition_commands(ROOT, plan, packet)
        print(packet, "resolved")
    except (AssuranceError, PlanAssuranceError) as error:
        print(packet, error)
CHECK
```

Coverage is the repository oracle scanner's Rust/Python/test definition universe and this plan's exact names. Missing definitions are evidence that acceptance is unimplemented, not proof that every reusable algorithm or integration is absent. No repository-wide semantic zero-state claim is made.

## Reconciliation Decisions

### Current evidence and its limits

| Command | Exit | What it establishes |
|---|---:|---|
| `just artifacts-check` | 0 | Active artifact, declared inputs and schema-2 state are valid; no behavioral certification. |
| `just plan-status` | 0 | The pre-reconciliation snapshot above. |
| `just plan-dependency-check docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md` | 0 | The declared packet DAG and shared-touch dispositions are valid. |
| `just real-time-cpg-packet-check WP77` | 101 | Stops at compilation of the first oracle's integration target; no packet-local gate pass. |
| `just real-time-cpg-packet-check WP79` | 1 | Rejects `rt_cpg_wp79_integrity` as a single-call Rust alias before executing the packet. |
| Focused library-only command below | 0 | Eleven named WP77–WP79 Rust tests pass; integration target and packet-local gates are outside this diagnostic. |
| Focused WP77 Python operations command below | 0 | One operations oracle passes. |
| `just real-time-cpg-certification` | 1 | Rejects the nonterminal execution phase before running the terminal graph. This is not a terminal test campaign. |

The diagnostic commands were:

```bash
scripts/repo-shell.sh -c 'CARGO_INCREMENTAL=0 cargo nextest run --locked --lib --no-fail-fast -E "test(/(^|::)rt_cpg_wp(77|78|79)_(integrity|behavior|faults|operations)$/)" --no-tests=fail'
scripts/repo-shell.sh -c 'PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -p tooling.ci.real_time_cpg_assurance tooling/ci/test_real_time_cpg_assurance.py::test_rt_cpg_wp77_operations'
```

The first diagnostic uses the packet engine's Cargo/nextest mode with an explicit library-only boundary because the integration target is already known to be broken. It does not redefine the packet gate. WP78's full dispatcher and the full M18 suite were not redundantly rerun: they share the failing integration target, and M18 additionally depends on WP79's rejected oracle. Passing the library alias directly does not satisfy the separate substance rule.

### WP77 — complete → stale

**What is proved:** the original proving commit remains an ancestor; the current integrity, behavior, faults and operations tests pass in the focused runs. `src/semantic_release/independent.rs` exercises immutable independent requirements; `src/programmatic_derived_analysis.rs` retains explicit unavailable Python control/dataflow implementations instead of treating sequential visitation as complete semantics. The four-oracle dispatcher exists and rejects invalid selection.

**Why completion is reopened:** resource contract changes affect provider/admission/release consumers after historical proof, and the named packet gate cannot compile the integration target. The original commit `7ca5407a128c6384b33eba7e1f49b8382853d8a6` remains provenance, not current full acceptance.

**Remaining / resume:** repair the WP79-owned immediate consumer break, rerun `just real-time-cpg-packet-check WP77` including its declared provider/release/feature/default/graph/root gates, then reconcile proof at a coherent committed candidate. Original instructions remain valid. Do not restore the false CFG completion claim to make a test pass.

### WP78 — complete → stale

**What is proved:** original proving commit ancestry plus four current library oracles for source inventory, effective Python context, support rejection and capture operations. Current source/job contracts distinguish inventory/context/support and carry charged ownership; source observations still enter the candidate. State's prior Pyrefly API limitation remains an explicit WP81 obligation, not a completion waiver.

**Why completion is reopened:** `ProviderJobSpec`, `ProviderText`, `ProviderNativeSourceImage` and `ExactPythonSyntaxRunner` changed after commit `0dd1fe19568164fc63da5bba8db9df789df274f7`; the real integration fixture has not followed those changes. Its compile failures affect WP78's named proof path too. This is inferred from the shared target and directly read signatures, not a claim that the WP78 dispatcher itself was rerun.

**Remaining / resume:** migrate the integration fixture using real process/workspace/job ownership, rerun `just real-time-cpg-packet-check WP78` including capture-race/provider/trust/root gates after WP77 revalidation, and preserve or supersede the proving commit appropriately. Original source/support contracts remain valid; watcher generation ownership and actual external provider composition remain later work.

### WP79 — retain in_progress

**What exists and is locally exercised:**

- `src/resource_budget.rs` supplies process/workspace/operation reservation lineage, independent dimensions, control reserve, shared charged values/slices, native accounting and exactly-once release. Its named negative oracle passes.
- `src/fabric/workspace_resources.rs` constructs one workspace owner; `src/fabric/resource_ownership.rs`, epoch construction and child scheduling share native managers. The behavior oracle exercises current/candidate/leased epochs, shared memory pressure and retained results.
- `src/cancellation/owned.rs` supplies owned async/blocking operations, cancellation observation and joins. The operations oracle keeps control work responsive with data capacity occupied and observes joined release.
- Source capture, provider admission, fresh/reconstructed workspace construction, query/result paths and durable result/source ledgers contain real ownership plumbing. `src/fabric/programmatic_epoch.rs` exercises assembly lifetime and policy drift; its helper passes when called by the direct library diagnostic.

**What remains:** the broken integration fixture, a substantive accepted integrity oracle, and complete material allocation/durable-I/O ownership. In particular, `src/fabric/programmatic_relation_delta.rs` still creates a history with `CreateBuilder::new().with_location(...)` and reopens it through `DeltaTableBuilder::from_url(...).load()` without accepting the workspace resource owner in those functions. The existing execution deviation already assigns native Delta/local-object-store blocking and cleanup ownership to WP79. The currently read code supports keeping that obligation open. Native transaction and exact-version semantics must remain intact; an application-authored Delta log planner is not an allowed shortcut.

Result/source ledgers are real progress beyond an entirely unowned path, but their existence and these eleven tests do not prove all uncertain writes, retained materializations, task lifetimes or hidden native allocations. The configured envelope is admission/accounting policy, not an allocator-complete RSS or measured latency claim. WP79 has no proving commit.

**Original instructions remain valid.** Complete immediate consumers and the resource/I/O closure, make the named integrity oracle directly substantive without weakening the validator, then run `just real-time-cpg-packet-check WP79` and M18. Do not defer a material WP79 allocation omission to the later performance packet; WP103 owns combined-workload calibration, not a waiver of foundational ownership.

### WP80–WP106 — retain not_started

The following are per-packet judgments. For **every row**, the exact four-oracle discovery above reports missing definitions, no proving commit is recorded, and completion requires `just real-time-cpg-packet-check <ID>` plus that packet's declared local gates. Retained implementation listed here is starting material, not plan-specific acceptance. Original instructions remain valid for every row, with the already-required authority transition at WP94 called out explicitly.

| Packet / dependencies | Current evidence or reusable starting point | Remaining outcome and focused completion direction |
|---|---|---|
| WP80 / WP79 | Owned syntax providers and parser adapters exist; startup still uses the Python syntax path. | Complete both language syntax/lexical lanes, actual retained-tree edits and bounded caches; prove clean/incremental expected facts through `inprocess-provider-lifecycle-check`. |
| WP81 / WP79 | Sidecar preparation exists; state records the ineffective private default `SysInfo`/Query seam. | Install effective retained context and required type/import/xref/subtype surfaces; prove changed settings and delete-last/recreate via `pyrefly-incremental-lifecycle-check`. Resolve the State/Handle integration rather than treating a context hash as a checker setting. |
| WP82 / WP79 | Contained extractor/service and Rust context contracts are retained. | Complete selected private MIR/profile families and actual contained compilation, failure withdrawal and joined process cleanup through `rustc-provider-lifecycle-check`. |
| WP83 / WP80, WP81, WP82 | `production_workspace_startup.rs` still supplies explicit required-input gaps for Pyrefly and rustc. | Connect actual four-provider output and canonical reconciliation to installed two-language genesis; prove `exact-provider-batch-check` and production composition. |
| WP84 / WP83 | Current release deliberately leaves required Python control/dataflow algorithms unavailable; reusable analysis kernels exist. | Install owner-local branches/loops/cleanup/suspension and core transfer semantics with independent expected facts, then `analysis-producer-semantic-check`. |
| WP85 / WP84 | Python derived-analysis code remains starting material; no accepted packet oracle. | Complete object/memory/alias/effect/resource/async families, precision and support, including lettered GEN relationships; prove causal and semantic analysis gates. |
| WP86 / WP85 | `rust_mir_derived_analysis.rs` retains typed MIR analysis APIs. | Complete selected ownership/flow/effect/state analyses and independent real-compiler expectations; separate private loans from approximations. |
| WP87 / WP86 | `common_derived_analysis.rs` retains graph/summary inputs and bounds. | Integrate required graph semantics and affected-SCC summaries, deletions and unknown propagation; prove semantic and fixed-point resource gates. |
| WP88 / WP83 | Startup installs unavailable source-wave effects. | Own live watcher/dirty registry, loss reconciliation and actual source-wave ports; prove same-process lifecycle and Git/capture parity. |
| WP89 / WP87, WP88 | WP78 support relations are a prerequisite; complete update realization is unproved. | Execute positive/negative invalidation, conservative widening and complete owner withdrawals through causal/coverage/unknown gates. |
| WP90 / WP89, WP92 | Startup still declares source-wave and relation-publication capability unavailable. | Publish atomic syntax/semantic successors in the same daemon and reject late work; prove `real_source_to_fastmcp_causal_vertical` with same-PID edits. |
| WP91 / WP83 | `programmatic_relation_delta.rs` still uses `ReplaceAll`; stable-history helpers are retained. | Classify durable/transient outputs, stable roots and unchanged pins; implement unconditional empty-owner replacement and selective publication. |
| WP92 / WP91 | Native schema/Delta boundaries exist; no successor overlay oracle. | Govern durable segments, replacement keys, anti-join/union effective state and bounded equality-proved consolidation. |
| WP93 / WP92 | `ExactDeltaCdfReplayCoordinator` and downstream interface exist. | Stream bounded exact CDF intervals through a real downstream consumer with durable progress/recovery; prove reconstruction and cancellation gates. |
| WP94 / WP93 | Current guarded maintenance explicitly leaves destructive native vacuum unavailable; proposed native-maintenance recipe is absent. | Supply and certify the required reproducible native amendment. Then issue the planned versioned pin/suite/plan-state transition with stable IDs; do not restamp this plan's inputs. |
| WP95 / WP94 | Existing retention/dry-run protections are retained; relation histories still select long preservation intervals. | Prove CDF-interval and lease-safe finite retention with actual eligible deletion, including admission races and restart. Denial alone cannot close this packet. |
| WP96 / WP95 | `programmatic_schema.rs` still excludes `optimize_projections` and `ProjectionPushdown`. | Meet mandatory A04 field-map identity proof with native rules enabled; implement the proposed optimizer-identity recipe and real Delta/view fault matrix. |
| WP97 / WP96, WP87 | Native engine, cache and scan contracts remain; WP79 adds resource plumbing. | Prove measured useful pruning/joins/distribution/encoding and truthful optimizer/resource contracts through scan/statistics/cache/determinism gates. |
| WP98 / WP99, WP90 | `production_query_recipe.rs` still compiles `compiled_find_entities_program` as its available production program. | Implement all eight forms and causally effective operands, mixed-success DAGs and dependent skipping; prove semantic request and authority gates. |
| WP99 / WP97, WP89 | Graph APIs and bounded witness helpers remain in `graph_program.rs`. | Prove demand-root work bounds and correct shortest/all-shortest witnesses before WP98 public composition; preserve multiedges and cancellation. |
| WP100 / WP98 | Modern FastMCP/resource transport and new result ownership are retained. | Complete canonical inline/automatic/resource meaning, source authorization, status/unknown/provenance and cross-language projection evidence. |
| WP101 / WP100 | Existing candidate/proof execution remains; charged retained batches are WP79 prerequisite work. | Prove dependency-aware execution reuse without dropping determinism re-execution, withdrawals or exact publication evidence. |
| WP102 / WP101, WP95 | Installed predecessor vertical tests exist; proposed full conformance recipe is absent. | Prove complete selected profiles/forms and two-language edits in one running installed topology, including real reclamation. |
| WP103 / WP102 | Structured cancellation and current WP79 unit operations are real foundations. | Run full provider/graph/IPC/query/Delta crash, lease, slow-consumer and joined recovery matrix; calibrate combined workload capacity. |
| WP104 / WP105 | Successor capture/check recipes are absent; old measurements are historical only. | Preregister, freeze and capture the completed candidate, then meet real-time/resource targets. Candidate changes require fresh evidence. |
| WP105 / WP103 | No successor cleanup oracle; the recorded broad Clippy baseline remains unclosed. | Remove replaced authorities, finish terminal tooling and pass all required quality gates before WP104 measurement. No baseline waiver. |
| WP106 / WP104 | Initial certifier exists and currently refuses the nonterminal phase. | At the final candidate prove every packet, milestone, DB exit, performance requirement and independent review; do not change phase labels to bypass prerequisites. |

### Milestones and decommission

M18 is reconciled from `not_started` to `in_progress`: its three member implementations exist and focused tests exercise their shared resource/source/release boundary, but full member and integration gates are unresolved. M19–M25 remain `not_started`; each depends on unrealized member outcomes. None is due for a successful completion claim. Resume each through `just real-time-cpg-milestone-check <ID>` only after the members' current evidence is coherent.

All DB24–DB30 remain `not_started`. Historical physical deletion and retained negative tests do not complete a successor batch. The following current positive evidence prevents blanket zero-state claims:

| Batch | Present target/legacy state and required next proof |
|---|---|
| DB24 | Compiled release/feature boundaries remain useful; WP79/WP83/WP105 are not complete. Reprove target provider/fabric/service consumers and the batch's structural/text/build exits. |
| DB25 | WP77 withdraws false complete Python algorithms, but replacement analyses and installed conformance are unfinished. Prove the real owner/control semantics before claiming the batch closed. |
| DB26 | Startup still supplies external-lane gaps and unavailable source-wave/publication effects. The same-PID two-language provider/update positive path remains required. |
| DB27 | `ReplaceAll`, long history preservation and unavailable destructive maintenance remain. Selective updates, protected CDF intervals and actual eligible reclamation must replace them coherently. |
| DB28 | Both projection-rule exclusions remain in the current constructor. Keep the safe boundary until the mandatory identity oracle proves the replacement. |
| DB29 | The production program list remains the narrow entity recipe. Complete forms, graph policies, partial results and delivery before removing corresponding shortcuts. |
| DB30 | Shared resource ownership has advanced, but material native storage/task closure and full proof/performance evidence remain open. No global absence of alternate ownership is established. |

Each batch closes only through `just real-time-cpg-decommission-check <ID>` plus its explicit exit gates, including positive target behavior. Static examples above are observed presence, not an exhaustive absence proof. No decommission suite was run while its dependencies were incomplete.

## Blockers and Invalidated Assumptions

1. **Current consumer mismatch, owned by WP79.** `tests/integration/fact_generation_build.rs:84,96,124,127,128,157` has six compiler errors: missing `ProviderJobSpec.resource_budget`, uncharged context, uncharged source bytes/text/offsets, and missing constructor jobs. The current definitions in `src/provider_contracts.rs` and `src/provider_native_syntax.rs` require charged inputs and job-bound runner construction. This is attributable to the present resource API transition, distinct from the recorded Clippy baseline. Fix the fixture under real ownership and rerun WP77/WP78/WP79; do not add uncharged compatibility constructors.
2. **WP79 oracle is not accepted.** `src/fabric/programmatic_epoch.rs:1262` defines the integrity oracle as a single helper call. The helper has real assertions, but the established oracle-substance checker rejects that body shape. Give the named oracle direct substantive contract assertions, preserving the helper's full behavior and falsifying cases. Do not weaken `_require_exact_definitions` or silently substitute the raw library test for packet acceptance.
3. **WP79 storage/task coverage is still open.** Preserve the execution state's durable Delta/local-object-store ownership obligation and finish the material allocation/read/write/cleanup route. Passing reservation and scheduler tests is insufficient to close it.
4. **Future dependency artifact and proof obligations remain.** WP94's native amendment and subsequent planned authority transition are required; native maintenance, optimizer identity, installed conformance and successor performance recipes are not yet implemented. No present external authorization blocker was discovered that prevents repairing WP79. External distribution authority, if later needed, remains a separate conditional constraint.
5. **Baseline quality is unresolved, not waived.** The state records a broad `root-clippy` failure from the prior execution baseline. This audit did not rerun `ci-fast` or Clippy and does not assert that the older diagnostic set is unchanged. WP105 still owns current all-required-gates closure; the live integration compilation failure already precludes full acceptance.

No accepted semantic design, declared input or packet dependency was found invalidated. The corrections are current implementation/proof reconciliation. The original plan remains the acceptance authority; the mandatory later pin amendment must use its versioned successor process.

## Recommended Resume Order

1. Repair the immediate WP79 integration fixture and integrity-oracle issues while preserving the current working tree; finish the already identified resource/I/O closure.
2. Revalidate stale foundational packets WP77 then WP78; prove WP79, commit coherent task-owned work and reconcile proving commits; run M18 before releasing provider work.
3. Follow the provider dependency branches WP80/WP81/WP82 → WP83 → M19, with shared-file coordination required by the plan.
4. Follow WP84 → WP85 → WP86 → WP87 and the watcher/storage branches WP88 and WP91 → WP92. Join WP87/WP88 into WP89, then WP89/WP92 into WP90.
5. Continue WP93 → WP94 → planned versioned authority amendment → WP95 → WP96; join WP87 for WP97. WP99 precedes WP98; then WP100 → WP101.
6. Complete WP102 → WP103 → WP105 before final WP104 capture. WP106 performs nonmutating certification and independent review closure with every milestone/DB obligation. Do not carry old measurements across candidate changes.

## Exact Next Action

The next executor should inspect the live WP79 diff and read `tests/integration/fact_generation_build.rs` alongside the charged provider contracts, then migrate that fixture to the real process/workspace/job budget and job-bound syntax runner. Make `rt_cpg_wp79_integrity` a directly substantive oracle and retain its assembly-lifetime/policy-drift checks. Finish native durable-I/O/cleanup ownership before claiming WP79 complete.

The focused revalidation sequence is:

```bash
just real-time-cpg-packet-check WP77
just real-time-cpg-packet-check WP78
just real-time-cpg-packet-check WP79
just real-time-cpg-milestone-check M18
just artifacts-check
just plan-status
```

Packet dispatchers own their declared local gates. A passing narrower selection is diagnostic only. Record a current proving commit only after the relevant full acceptance passes, and do not stage unrelated pre-existing work. This audit has not started implementation repair.

## State Reconciliation Summary

The existing schema-2 state was updated after evidence gathering, with a comparison against the pre-audit bytes to prevent overwriting concurrent state changes:

- Overall status remains `executing`; `current_packet` remains WP79, the owner of the necessary repair.
- WP77/WP78 become `stale`; original proving commits, deviations and failed approaches are preserved. Added judgment explains the shared consumer/proof drift and the required revalidation.
- WP79 remains `in_progress` with no proving commit; its existing allocation/Delta obligations are preserved and the immediate consumer/oracle corrections are appended.
- M18 becomes `in_progress`; M19–M25 and DB24–DB30 remain `not_started`.
- Discovered obligations and `next_action` name the exact repair and dependency-safe revalidation. No digests, changed-file census or check-result ledger is added to execution state.

Final verification: `just artifacts-check`, `just plan-status`, direct `validate_review` on this exact new report, and scoped `typos` passed. The audit preservation check permits changes only to this report and the selected execution state. The plan, design and pre-existing implementation contents remain byte-identical to the audit snapshot.

Post-reconciliation `just plan-status` output, reproduced verbatim:

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "df1c50c684a5e005c4b76ada9cd19d8030d9d7dd",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [],
  "declared_input_count": 27,
  "healthy": true,
  "plan_path": "docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md",
  "stale_inputs": [],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

The empty complete lists reflect the reconciled state. `healthy: true` still describes artifact/input/provenance health, not runtime completion.
