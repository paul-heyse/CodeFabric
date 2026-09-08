---
artifact: implementation-status
plan_path: docs/plans/codefabric_real_time_cpg_implementation_plan_v2_2026-09-04.md
state_path: docs/plans/state/codefabric-real-time-cpg_v2_state.json
version: v2
date: 2026-09-07
status: complete
---

# Real-time CPG execution: native resource decision reopening

## Judgment and scope

**The remaining plan is not complete. WP79 requires reopening the native resource/library decision before its production resource contract can be closed.** Tested ownership improvements remain useful, but no fixed native memory reservation has been established that covers the pinned Delta/kernel decoder and retained snapshot state. All WP80–WP106 packets transitively depend on WP79. No CPG semantics or required work packet is being removed.

This report follows execution authorized from the [v1 implementation-status review](implementation_status_codefabric_real_time_cpg_implementation_plan_v2_2026-09-04_2026-09-07_v1.md). It records implementation progress and a concrete decision request, not an independent certification of the root agent's changes. Separate agents implemented bounded native/store primitives and independently challenged the native-library feasibility and dependency classification. The user explicitly authorized discretionary sub-agent use.

The root candidate is `e24627a29ee44bc3f288eeb07197b66e6dec7c7e` plus the shared working tree. Sub-agent delta commits are private implementation checkpoints, not root proving commits. The initial overlapping changes are preserved in `/tmp/codefabric-rt-exec-20260907-8hy8_m98/`; no reset, broad cleanup, dependency source modification, remote publication or plan/design rewrite was performed. The active-plan pointer remains unchanged.

## Implemented work and its limits

- Migrated the fact-generation integration consumer to charged source/context/provider contracts; corrected its semantic-release resource-owner fixture; made the WP79 integrity oracle directly substantive without weakening the alias rejection.
- Moved bounded Arrow encoding into a feature-independent fabric module. The `data-fabric` feature no longer imports the daemon-only result package. Added the already-resolved `bytes` crate as an explicit optional dependency for retained `Bytes::from_owner` ownership; this changes the root edge, not the resolved dependency version.
- Shared the workspace DataFusion runtime and injected its root-addressed object store into selected native Delta create/reopen paths. Native Delta retains transaction and table-state authority. Centralizing the existing filesystem backend does **not** yet make production native storage fully budgeted.
- Added traced DataFusion descendant/stream ownership, finite task bookkeeping, typed capacity/cleanup outcomes, and reservation retention through native termination. A cleanup deadline reports pending cleanup; it does not release capacity while work survives.
- Added a native execution lane and workspace read/mutation wrapper that construct and poll the complete operation on an owned multithread runtime, drain cleanup and destroy that runtime before publishing a terminal result. This joins started hidden blocking workers. Typed failures survive cleanup composition; opaque error and panic payloads are disposed in the native context before the join barrier. The explicit memory profile is an admission reservation, **not** an allocator limit or a proved production cost envelope.
- Added a finite local-store owner for read buffers, ranges, listings, physical occupancy and multipart mutation state. Runtime-bound mutation grants prevent concurrent read lanes from borrowing write authority. Reconciliation follows an unforgeable joined-runtime token; physical census never selects Delta semantic state.

These primitives do not close WP79. Production still needs complete phase integration, charged retained native outputs, a justified native decode/working-state bound, cleanup/recovery integration, packet gates and a coherent proving commit. Failed native multipart completion can leave `#<digits>` staging paths which the native ObjectStore path API rejects; those bytes remain charged. Automated physical recovery remains an explicit implementation obligation.

## Evidence for reopening

The live lock selects delta-rs revision `43a0cf10`, `buoyant_kernel` 0.25.1 and `buoyant_kernel_engine` 0.25.0, retaining DataFusion 55 / Arrow 59.2. The following source paths are relative to those exact dependency source roots; line references identify the inspected source rather than a moving upstream branch.

| Native surface | Current source evidence | Consequence |
|---|---|---|
| Retained eager snapshot | delta-rs `crates/core/src/kernel/snapshot/mod.rs:1283,1404`: `EagerSnapshot` retains a private `Arc<Snapshot>` and its snapshot accessor is crate-private. `Snapshot::files` at :609 can expose original cached batches on a plain snapshot; this does not expose the loaded eager backing. | No supported complete traversal of the installed eager cache was found. Replayed/projected batches are different allocations and cannot establish ownership of the original buffers. |
| Hidden original backing | delta-rs `kernel/snapshot/log_data.rs:59` and `iterators.rs:108` retain private batches. Materialized files also retain metadata/protocol and other non-Arrow state. | Counting active files, claiming a public projection or serializing a second snapshot does not account the retained original. |
| CRC decoding | kernel `src/crc/mod.rs:173` directly calls `serde_json::from_slice`; `src/crc/reader.rs:37` turns errors into optional absence using `.ok()`. | A delegated JSON engine handler does not govern this allocation path; a future resource failure must not silently become optional CRC fallback. |
| Schema decoding | kernel `src/actions/mod.rs:349` parses a metadata schema string directly with serde. | Row limits and post-decode Arrow checks do not bound schema construction before work. |
| Checkpoint and replay decoding | kernel JSON/Parquet handler interfaces return decoded `EngineData`; the current APIs provide no shared fallible allocation guard before every decoder/replay expansion. | Compressed object-size bounds alone do not prove decoded page, dictionary, value, map or accumulated-batch bounds. |
| Misleading configuration candidates | delta-rs `DeltaTableConfig.log_batch_size` appears only in definition/default/equality; `log_buffer_size` is consumed by `commit_infos`, not main snapshot decoding. Kernel engine `src/lib.rs:85` defaults both prefetch and batch rows to 1000. | Neither the table batch setting nor DataFusion target partitions establishes the proposed aggregate native bound. |
| Writer fanout | delta-rs `operations/write/execution.rs:39–56,523`: channel size accepts any positive environment-provided `usize`; workers follow actual physical output partitions. | A production profile must validate inherited channel settings and actual physical fanout before admission. Configured target partitions alone is insufficient. |

The primary agent independently inspected the eager accessor, cached-file path, direct CRC/schema decoders, configuration consumers and writer channel parser. The independent reviewer also challenged whether a bounded-input envelope could satisfy the existing contract without modifying dependencies. Such a proof is possible in principle, but none is established here. It would need full mediation of immutable exact objects, cumulative log/action bytes and counts, schema structure, CRC collections, decoded checkpoint geometry, replay accumulation, retained metadata and actual task/channel fanout. A permanently JSON-only history profile or refusal of required checkpoint/recovery histories would change supported guarantees or leave mandatory retention work unfinished. A guessed multiplier or post-decode rejection is not that proof.

The accepted [D-RT05 §3.5](../designs/codefabric_real_time_cpg_planning_contracts_design_v1_2026-09-04.md#35-d-rt05-budgets-ownership-and-exhaustion) requires admission before work and material retained-memory ownership; it explicitly says a missing large allocation path blocks the claim. It permits measured overhead and supplemental containment and does not demand exact RSS accounting. WP79's explicit trigger therefore fires because the proposed aggregate budget excludes material native paths, not merely because an API is inconvenient.

## Proposed decision: narrow native resource amendment

Accept an additive native resource-admission amendment, then issue the required versioned design/plan/pin-authority transition. Preserve the current CPG meanings, native Delta planning/transactions, Arrow/DataFusion universe, four first-party build domains and every WP77–WP106 obligation. This is a proposed resolution, not a claim that it is the only theoretically possible implementation.

The amendment should require these native capabilities; names below are proposed contracts, not current APIs:

1. **Original retained backing access.** Expose borrowed materialized batches through the eager snapshot/log-data boundary, preserving the materialized-versus-lazy distinction. No projection, replay or serialization may substitute for original buffers. CodeFabric can then attach shared Arrow allocation ownership for the actual lifetime.
2. **Fallible decode/replay admission.** Carry a shared resource guard through JSON/CRC/schema/checkpoint decoding and replay accumulation. Reserve before growing material decoded buffers, maps, collections or schemas; enforce cumulative bytes, individual values/pages, rows/actions and structure limits. Resource exhaustion must propagate as a typed failure, including optional CRC paths. Establish whether the actual allocation boundary also needs an Arrow/Parquet patch before selecting an artifact.
3. **Retained non-Arrow ownership.** Provide native allocation receipts or complete ownership traversal for retained schema/configuration/CRC/log-path/replay state. Clones share charges until the last owner; a partial heap estimator does not substitute for omitted state.
4. **Finite native execution configuration.** Bind kernel prefetch, batch geometry, actual physical partitions, channel capacity and blocking nesting to the reviewed execution profile. Validate ambient configuration before native OnceLock initialization. Join and reconcile all admitted work on rejection, cancellation and deadline.

Required falsifiers include highly compressed checkpoints with large decoded values, oversized/nested schema and CRC collections, cumulative small-action replay growth, exhausted admission before decoding, optional-CRC exhaustion propagation, eager-cache clone/drop lifetimes, cancelled native decode/write workers, concurrent candidate/query/retained-epoch pressure, and reserved control responsiveness. Instrument the native allocation boundary; measuring a returned batch alone cannot prove reserve-before-work.

Artifact preparation should start from the exact pinned native source in an isolated local checkout, preserve licensing and provenance, produce a reviewable patch and immutable reproducible artifact, and inspect the dependency diff. Do not modify the Cargo cache or switch the root dependency to an unversioned local path. The authority amendment must enumerate FAB §2.1, manifests/locks, exact graph/pin validators, reference routing, compatibility probes, policy identities and declared plan inputs affected by the final artifact. Preserve Arrow/DataFusion type-universe checks and all four-domain compatibility obligations. External publication is a separate action; none is needed to approve this local direction.

The successor must explicitly retain WP77/WP78 revalidation, place the resource amendment before WP79 closure, preserve WP80–WP106 and M18–M25 dependencies, and retain WP94's distinct native maintenance contract plus WP95 reclamation. Native resource work does not satisfy maintenance START/END properties, approved deletion, zero retry, leases or CDF protection by implication. Give every unfinished packet and DB24–DB30 obligation an explicit successor disposition using the existing stable IDs.

## Why acceptance is required

Plan §2.1 requires an accepted versioned pin/suite-authority amendment and a successor plan before execution against changed declared inputs. Its §9.2 says: “Obtain acceptance of the revised design before continuing that path.” The existing LD-RT08 exception is narrowly for native maintenance; it does not already select decoded-allocation admission changes.

The applied [impl-plan-exec skill](../../.claude/skills/impl-plan-exec/SKILL.md) §“Design reopening required” also says to “record evidence and a design-reopening request” and “continue only unrelated packets.” This is an explicit rule, not an inferred confirmation requirement. WP77/WP78 revalidation remains independent work; the later dependency graph does not.

## Validation and handoff

The following complete commands passed during this execution:

- `just real-time-cpg-packet-check WP77`, including its four oracles and all packet-local gates.
- `just real-time-cpg-packet-check WP78`, including its four oracles and all packet-local gates.
- `just feature-architecture-check data-fabric`, including independent Cargo compilation and the structural boundary fixture.
- `just stable-graph-check` and all 20 feature-architecture validator tests. The synthetic provider fixture and stable graph census now include the charged-input `arrow-buffer` edge; forbidden dependency boundaries remain enforced.
- The corrected store passed all 16 focused tests in both its isolated implementation tree and the root tree. Independent source review verified fixes for long listing paths, readers holding deleted files open, late multipart registration during cleanup, and per-range allocation charges.
- Final merged native/store/wrapper/encoding/activation validation passed **53 tests**: `just root-test-incremental --lib -E 'test(native_lane_) | test(workspace_native_) | test(owned_local_) | test(wp79_native_) | test(bounded_encoding::tests::) | test(native_capacity_preserves_) | test(exact_delta_activation_authority_split)'`. Independent source review also verified the classified-error ownership/composition corrections and native-context panic disposal.
- Final `just root-check` passed both the default all-target and featureless builds. `just root-fmt`, `just artifacts-check`, `just plan-status`, and `git diff --check` also passed. Plan-state health is not behavioral certification.

The complete WP77/WP78 dispatchers ran after the original integration/oracle/feature fixes. The later native primitive follow-up has its separate 53-test root validation; neither observation is terminal certification. Temporary raw logs are in `/tmp/codefabric-rt-exec-20260907-8hy8_m98/`.

No new root proving commit or complete packet/milestone status is asserted. WP77/WP78 retain their historical proving commits until the changed consumer boundary has coherent committed lineage; WP79 and downstream acceptance require the reopening disposition. Full WP79/M18 and terminal certification were not run as closure gates. The inherited full-quality failures remain WP105 obligations.
