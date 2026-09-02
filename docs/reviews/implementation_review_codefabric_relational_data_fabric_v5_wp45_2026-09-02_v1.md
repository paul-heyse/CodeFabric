---
artifact: implementation-review
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: changes-required
version: v1
date: 2026-09-02
status: complete
---

# Implementation Review: CodeFabric relational data fabric v5 WP45

## Provenance and Review Scope

This independent, read-only review assesses the current WP45 candidate against the approved v5
plan, the accepted FastMCP 4 presentation-boundary review v2, the composing daemon/gRPC boundary
review v5, the current Protobuf source and generated Rust/Python clients, and the repository's v2
data-fabric doctrine. The reviewed plan digest is
`fe3259191cf8e90f8593d35ca913145789eed4ca6ba7f7592218a88effb398fb`; the FastMCP review-v2
digest is `202329441a517e097ac3a045cbf1022bf05242c8de2e8f2e2f58d5ecd3b9ee6f`; and the daemon
review-v5 digest is `2c4e819bc416a9fd7fcf5a76928aa17a470a5b296b003e62cf451b170513b7ae`.

Review was performed at repository HEAD `c277952ca83d0045a7f3990634da928d8e0979e7` in the
shared dirty worktree. Execution state remains `executing`, with WP44 `in_progress` and WP45
`not_started`; WP45 has no proving commit. That is consistent with reviewing candidate bytes
before their proving transaction and is not itself a finding. No production, test, tooling,
Justfile, state, or plan file was edited. This report is the review's only repository change.

The reviewed surface includes `contracts/rpc/cpg_query_service.proto`, the committed descriptor,
generated Rust and Python bindings, `src/query_service.rs`, query coordination and resource
registries, lifecycle/session/RPC authority, the Python daemon client, the Rust/Python UDS
interoperability drivers, and the exact four WP45 recipes. The review specifically traced whole
operation budgets, transport/admission bounds, reconnect session renewal without start
resubmission, bounded release tombstones, durable policy/revocation/sharing/cancellation state,
lifecycle acceptance/drain linearization, typed challenge replay/expiry/output, projection
removal, and safe correlation/error types.

## Executive Summary

The candidate has a strong target-only core. There is one generated `codefabric.cpgd.v2` service;
atomic start is a closed oneof; validation is pure; Protobuf numbers and the removed projection
vocabulary are reserved; public resource handles are distinct from internal lease tokens; resource
reads/releases reauthorize every authority binding; result and release retention are bounded;
cancellation identities and policy/revocation/sharing authority survive journal recovery; and
lifecycle admission is linearized against drain. The generated Rust and installed-wheel Python
clients both traverse the real Tonic service over UDS. The Python reconnect path performs a new
handshake and resumes observation without issuing another `StartQuery`. Safe error details and
correlation identifiers are typed and allowlisted.

Three material gaps prevent approval. First, the nominal remaining budget is not one budget across
the complete operation: Python reconnect may spend a new readiness budget after the watch deadline,
Rust streaming checks the deadline only before potentially blocking awaits, and queued query work
can wait in `await_running` without its execution deadline. A live probe exceeded a 50 ms watch
budget by four times during reconnect. Second, challenge expiry classification depends on whether
an unrelated start-state prune ran first: pruning removes the only record that distinguishes an
expired continuation, after which the same token is reported as a generic invalid continuation.
Third, the exact four WP45 recipes do not exercise that ordering or the existing challenge replay/
expiry/output fault tests. They also omit the final causal regression which proves that private
`PublicationPending` journal state cannot create gaps in public event ordinals. The implementation
now projects contiguous public ordinals and cursor checksums correctly, and focused final-byte
tests pass, but the packet recipes would have remained green before that repair.

## Verdict

**Changes required.** IR-001 through IR-003 are major correctness/operations and contract-proof
findings. The accepted design remains valid; all findings are local implementation and oracle
repairs. WP45 must not receive its proving commit or state-completion transition until the repairs
and focused re-review pass.

## Gate and Evidence Assessment

| Evidence | Fresh result | What it establishes -- and what it does not |
|---|---:|---|
| `just fastmcp4-daemon-wire-contract-check` | pass on final bytes | Source/descriptor/generated consistency, proto evolution, two real generated-client UDS traversals, and selected closed-oneof/client negatives. |
| `just fastmcp4-atomic-start-check` | pass on final bytes; 2 UDS interop, 4 Rust, 3 Python cases | Pure validation, atomic reservation/replay, closed oneof decoding, and generated-client reachability. It does not select challenge expiry or private-event projection regressions. |
| `just fastmcp4-resource-authority-check` | pass on final bytes; 2 UDS interop, 7 Rust cases | Per-use handle authority, bounded release tombstones, package/resource release, restart cleanup, reissue, and expiry cleanup. |
| `just fastmcp4-daemon-security-recovery-check` | pass on final bytes | Durable cancellation/recovery, policy/revocation/sharing authority, lifecycle admission, reserved control, safe metadata, identity rejection, and reconnect/no-resubmit cases. Its budget case tests only `RpcBudget` itself, and its selector omits the challenge and projected-sequence faults. |
| focused projected-sequence/cursor selection | pass on final bytes; 3/3 | Private publication intent is excluded, public ordinals remain contiguous, projected cursor checksums verify, and policy/revocation cursor authority remains exact. |
| focused guarded-input Rust selection | pass; 5/5 | Three rounds, stable rejection, direct expiry/replay, output bound, and live catalog input all pass in the tested ordering. |
| full `test_daemon_client.py` | pass; 14/14 | Strict Python decoding, generated wire use, reconnect/no-resubmit, malformed requirement rejection, and safe status handling pass. |
| delayed reconnect runtime probe | reproduces defect; `DaemonRpcError elapsed=0.202 budget=0.050`, `start_legs=['initial', 'continuation']` | Reconnect correctly avoids a third start leg, but consumes more than the caller's entire watch budget. |
| scoped structural/text search | pass except explicit reservations/tests/internal Rust leases | No live `ProjectionSelector`, projection resource kind, Python lease registry, public attempt IDs, repeated expected-generation assertions, `Any`, or challenge JSON remains. Internal Rust result-lease tokens do not cross the wire. |
| `ast-grep scan --config sgconfig.yml` over the WP45 source/proto/client scope | pass | No current structural-governance finding in the reviewed scope. |
| scoped `git diff --check` | pass | The reviewed WP45 diff has no whitespace errors. |

## Finding Index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | major | operations | Remaining budgets do not bound the complete queued/streaming/reconnect lifetime. |
| IR-002 | major | correctness | Typed challenge expiry is prune-order-dependent. |
| IR-003 | major | tests | Exact WP45 recipes omit challenge and private-journal/public-sequence faults that have exposed real failures. |

## Findings

### IR-001 — Remaining budgets do not bound the complete operation lifetime

**Severity:** major
**Dimension:** operations
**Design/Plan refs:** WP45 required change 7; I5-13, I5-17; FastMCP design v1 §8; Tonic
reference §§18.1--18.6, 35.7
**Evidence:** `CpgDaemonClient.watch_query` creates an absolute deadline at
`client.py:1564-1565`, but on transport loss calls `_reconnect` at `client.py:1642` without passing
the remaining duration. `_reconnect` gives `next_settings` the full readiness timeout and then
`connect` gives channel readiness and Handshake fresh readiness timeouts (`client.py:1145-1160`,
`1043-1070`). A live UDS probe injected a 200 ms replacement-settings delay into a watch with a
50 ms total budget and observed `elapsed=0.202`, while `start_legs` remained exactly
`['initial', 'continuation']`. On the Rust side, `execute_accepted_query` waits in
`QueryCoordinator::await_running` without a deadline (`query_service.rs:2253-2257`,
`query_coordinator.rs:877-907`). `watch_next` and `read_next` check `budget.remaining()` before
session/coordinator/resource awaits, but do not wrap those awaits in `timeout_at`
(`query_service.rs:2467-2524`, `2723-2786`). The named test at `query_service.rs:4762-4775` proves
only that `RpcBudget::run` times out one sleeping future; it never drives queueing, a stream
iteration, or reconnect.
**Failure mode:** a request can exceed its advertised remaining budget while renewing a session,
waiting for a query slot, waiting on stream authority/state, or reading a resource. Expired queued
work may retain task/result reservations until unrelated retention activity, and a blocked stream
may retain its data-admission permit after the declared operation deadline. This breaks the one
relative-budget contract and makes reserved control insufficient to bound overall resource
occupancy.
**Remediation:** compute one absolute operation deadline before connection or observation work and
thread only its remaining duration through settings renewal, channel readiness, Handshake, Watch,
and every retry. In Rust, make queue admission/execution deadline-aware, close expired queued work
durably, release its reservations, and wrap each stream iteration's asynchronous authorization,
cursor/event/read work under the same deadline. Preserve an explicit smaller cleanup reserve rather
than giving execution the entire outer timeout.
**Focused re-test:** after adding causal blocked-queue, blocked-stream, and delayed-reconnect cases,
run:

```bash
cargo nextest run --locked --lib -E 'test(/wp45_(queued_execution_deadline_releases_reservations|watch_iteration_deadline_releases_admission|read_iteration_deadline_releases_admission)/)' --no-tests=fail
uv run --frozen --project codefabric-cpg-mcp pytest -q codefabric-cpg-mcp/tests/test_daemon_client.py::test_watch_reconnect_consumes_one_entire_lifetime_budget_without_resubmitting_start
just fastmcp4-daemon-security-recovery-check
```

### IR-002 — Typed challenge expiry is prune-order-dependent

**Severity:** major
**Dimension:** correctness
**Design/Plan refs:** WP45 required changes 2--3; I5-09; WP45 `PC-WP45-OPS` fault contract
**Evidence:** `prune_start_state` deletes every challenge at its expiry
(`query_service.rs:1122-1138`) and is invoked by unrelated issue/replay/reservation paths
(`query_service.rs:748-755`, `782-784`, `824-830`). `consume_challenge` can emit the typed
`CHALLENGE_EXPIRED` result only after finding the retained record; a missing record returns generic
`CHALLENGE_CONTINUATION` before inspecting expiry (`query_service.rs:873-900`). Consequently, the
existing `wp45_challenge_expiry_and_replay_are_distinct_typed_outcomes` case passes only because it
consumes the expired token before any prune.
**Failure mode:** the same authentic continuation at the same age can produce either typed
`ContinuationExpired` or generic invalid-continuation behavior depending on unrelated request
ordering. That makes the closed recovery contract nondeterministic and violates the typed
challenge recovery contract.
**Remediation:** retain a strictly bounded, expiry-aware used/expired challenge tombstone or use an
authenticated continuation envelope that preserves enough non-secret classification data after
the live preparation record is reclaimed. Authorize and classify that record before falling back
to generic invalid-continuation handling. Bound tombstone count and retention explicitly.
**Focused re-test:** after adding the ordering fault, run:

```bash
cargo nextest run --locked --lib -E 'test(/wp45_(challenge_expiry_remains_typed_after_unrelated_prune|challenge_expiry_and_replay_are_distinct_typed_outcomes|guard_invalid_answer_closes_and_replays_stable_rejection|challenge_output_is_bounded_after_typed_projection)/)' --no-tests=fail
just fastmcp4-daemon-security-recovery-check
```

### IR-003 — Exact WP45 recipes omit causal faults that have exposed real failures

**Severity:** major
**Dimension:** tests
**Design/Plan refs:** WP45 packet-local gates; `PC-WP45-BEH`; `PC-WP45-OPS`; P25, P27, P30
**Evidence:** the four plan-named WP45 recipes do not select
`wp45_challenge_expiry_and_replay_are_distinct_typed_outcomes`,
`wp45_guard_invalid_answer_closes_and_replays_stable_rejection`, or
`wp45_challenge_output_is_bounded_after_typed_projection`; those cases appear only in the later
`fastmcp4-guard-roundtrip-check` recipe (`justfile:679-683`), while the WP45 selectors at
`justfile:655-677` omit them. The final coordinator repair added
`wp45_beh_private_journal_events_do_not_create_public_sequence_gaps`, which proves private
`PublicationPending` state is excluded while public ordinals and cursor checksums remain exact
(`query_coordinator.rs:346-397`, `2304-2383`). A focused final-byte run of that test plus the
operation/cursor and policy/revocation cursor tests passed 3/3, and all four named recipes were
rerun successfully. However, the new causal regression is not selected by any of those four
recipes. The shared generated-client UDS fixture and existing selectors were green before this
private-event sequence defect caused a real installed-wheel vertical failure.
**Failure mode:** the packet can be marked green while typed challenge fault behavior or the
private-journal/public-stream boundary regresses. Because both omitted cases have concrete causal
branches and the latter already escaped to a real consumer, later vertical tests become the first
line of detection instead of WP45's governed acceptance evidence.
**Remediation:** add the private-public sequence/cursor regression to the appropriate BEH recipe and
the challenge expiry/replay/output faults to the OPS or atomic recipe. Keep each selector explicit
and require nonzero selection. The four WP45 recipes, not a later WP46/WP47 recipe, must own these
wire/daemon authority faults.
**Focused re-test:** after updating the named selectors, run:

```bash
just fastmcp4-atomic-start-check
just fastmcp4-daemon-security-recovery-check
just gate-filter-census
```

## Outcome and Invariant Matrix

| WP45 outcome or invariant | Executable oracle | Assessment |
|---|---|---|
| Sole generated v2 proto/descriptor/Rust/Python contract | `just fastmcp4-daemon-wire-contract-check`; `just proto-check`; `just proto-repro-check` | satisfied |
| Closed atomic Start outcome and pure Validate | `just fastmcp4-atomic-start-check` | satisfied |
| Typed, bounded, daemon-authored challenge with replay and expiry | focused five-test guarded-input selection; required new prune-order test in IR-002 | changes required: IR-002 |
| Private journal state projects to contiguous public events and bound cursors | focused 3-test projected-sequence/cursor selection; required named-recipe selection in IR-003 | implementation satisfied; packet oracle changes required: IR-003 |
| Daemon-minted handles and per-read/per-release reauthorization | `just fastmcp4-resource-authority-check` | satisfied |
| Bounded release tombstones and exact cleanup/reissue | `just fastmcp4-resource-authority-check`; `just fastmcp4-daemon-security-recovery-check` | satisfied |
| Authorized, capped, non-enumerating reference completion | shared generated-client UDS interop inside each packet recipe; focused completion tests already reachable through `just fastmcp4-completion-authorization-check` | satisfied in WP45 bytes |
| Durable policy/revocation/sharing/cancellation authority | `just fastmcp4-daemon-security-recovery-check` | satisfied |
| Separate bounded transport/data/control admission | `just fastmcp4-daemon-security-recovery-check`; `missing_or_mismatched_identity_is_rejected_before_handler_dispatch` | satisfied for current tested saturation model |
| One remaining budget across operation lifetime | required blocked-queue/stream/reconnect tests in IR-001 | changes required: IR-001 |
| Reconnect renews Handshake/Watch and never resubmits Start | two reconnect tests in `just fastmcp4-daemon-security-recovery-check`; delayed reconnect probe | no-resubmit satisfied; lifetime bound fails under IR-001 |
| Ready admission linearizes before drain | `wp45_ready_admission_linearizes_acceptance_before_drain` through security-recovery recipe | satisfied |
| Safe typed status, diagnostics, progress, and correlation | wire and security-recovery recipes; full daemon-client tests | satisfied |
| Projection/secret/public-attempt legacy is absent | proto tests plus scoped `rg`/`ast-grep` search | satisfied |

## Architecture and Doctrine Assessment

The target authority split is sound. Rust remains the only query, challenge, DataFusion/Arrow,
resource, lifecycle, and durable recovery authority; Python maps strict presentation DTOs onto one
generated daemon port and holds only channel/session transport state. Public handles are capability
references to Rust authority, not Python-generated identity. Atomic start, exact coordinator
records, per-use denied cases, real generated-client UDS construction, and durable cancellation
provide substantive P13/P25/P27 evidence rather than name-only proof.

IR-001 violates P23's explicit lifecycle/resource-state obligation and weakens P13's failure
contract because timeout is not a decisive transition at every owned layer. IR-002 violates P23:
the externally observed challenge state depends on unrelated pruning order. IR-003 violates
P25/P27/P30 because the governed recipes omit causal expected-output faults which have already
changed a downstream installed-wheel result. None of these defects
requires reopening the accepted topology or restoring a legacy authority.

## Library Leverage Assessment

Protobuf is used well: closed oneofs, typed enums and constraints, binary continuation/error data,
field reservations, generated clients, and hermetic descriptor equivalence replace ad hoc parsing.
Tonic/grpcio provide real UDS transport, generated service/client paths, message bounds, streaming,
standard statuses, and server-visible timeouts. Pydantic's strict discriminated models prevent
generated Protobuf objects from becoming public FastMCP schemas.

The remaining budget implementation does not yet use those timeout facilities to their full
contract. A deadline must be computed once, propagated as remaining time to grpcio calls, and
enforced around Tonic-owned asynchronous stream/queue work. The DataFusion/Arrow execution path
does receive the absolute execution deadline and checks it while sealing bounded result pages, but
the pre-execution queue wait is outside that enforcement. No additional processing library or
custom timeout framework is needed.

## Legacy and Decommission Assessment

The removed projection selector/resource kind survives only as reserved Protobuf history and
negative assertions. No live Python `ProjectionSelector`, public lease-token projection,
`_resource_leases`, `mcp_call_id`, `rpc_attempt_id`, repeated expected-generation assertion, `Any`,
or challenge JSON path was found. Internal Rust lease tokens remain appropriately behind the
daemon boundary. The sole current service is v2; no v3 compatibility service or v1 runtime service
participates. WP45 does not reintroduce static-schema, ontology/bootstrap, or digest-as-semantic-
proof authority.

## Test and Operational Assessment

The four named recipes all pass on the final bytes and each crosses both generated clients through a real Tonic UDS
service. That is meaningful interoperability evidence, not a direct-call substitute. Resource,
durable cancellation, authority, lifecycle, safe-error, and reconnect-no-resubmit evidence is
substantive. The findings are precisely false-green boundaries: the budget test isolates a helper
rather than the operation lifetime, the direct challenge expiry test omits the prune ordering that
changes behavior, and both challenge faults and the repaired private/public event-sequence fault
sit outside the four WP45 recipe selectors.

The delayed-reconnect probe is particularly diagnostic: it preserves the positive no-resubmit
property while independently falsifying the budget claim. Green challenge tests remain useful but
do not negate IR-002 because their fixture never causes the prior pruning transition.

## Plan Deviations and Diff Hygiene

No design-changing plan deviation was found. The implementation deliberately revises the
unreleased v2 target in place and does not preserve legacy operability. The execution state has not
prematurely claimed WP45 completion. The shared tree contains extensive concurrent v5 work, so this
review is fingerprint-sensitive; the key reviewed digests are:

| Surface | SHA-256 |
|---|---|
| `contracts/rpc/cpg_query_service.proto` | `0cc74ca10e8ee0d9ed78aeb28c6ef36dbc6c587932f281f2ae53b385b3b798d0` |
| `src/query_service.rs` | `3ba40479721cb59159dfdb98805b7a8e7be811c07f9a19fc7cbebb6e1de03ded` |
| `src/fabric/query_coordinator.rs` | `65590c00cbf63dd07fab09f961035219d8937cc3af6675007372816b063006d9` |
| `src/fabric/streamed_result_registry.rs` | `1bd13e22e4434401e5e673db5f28f453165dcd5478619a3b5482476530ab26c6` |
| Python daemon client | `e8bcb398759cd0c0e01e8e8e36cd5b59839d827f8372dcf7ad20e6966b0e053e` |
| Python daemon-client tests | `0cf157f60a158a53a6e4765112e9e948038ec7ecd6191c7d9a4b63f2f9dd8fd1` |
| `tests/integration/rpc.rs` | `c69d72115b0cb9c52521f241532814cd66ee8eee1aa5ac8de4e9bb9ecdbabbf8` |
| `justfile` | `47ea8f185b60cce415721350388770568c5ac2321962d61d42dc1fcb0e38f86e` |

## Required Remediation Order

1. Establish one absolute budget across Python connect/reconnect/watch and Rust queue/stream work;
   add causal blocked-operation tests and reservation/admission release assertions.
2. Preserve bounded typed expiry/replay classification after live challenge reclamation and add
   the unrelated-prune fault.
3. Put the challenge expiry/replay/output and private-journal/public-sequence faults into the exact
   four WP45 recipes, rerun all four recipes plus the focused tests, and obtain a focused re-review
   before any proving commit.

## Focused Re-Review Scope

Re-review may be limited to `src/query_service.rs`, `src/fabric/query_coordinator.rs`, the Python
daemon client and its tests, challenge/budget/projected-sequence selectors in `justfile`, and any
generated contract surface only if remediation changes it. It must rerun the exact four WP45
recipes, the new blocked-budget probes, the prune-before-expiry/replay/output cases, the private-
journal/public-sequence cursor case, full daemon-client tests, and scoped projection/error/
correlation zero-state searches. Resource tombstones, durable
policy/revocation/sharing/cancellation, lifecycle linearization, and generated UDS interop need only
be rerun if their reviewed bytes or selectors change.
