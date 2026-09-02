---
artifact: implementation-review
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: approved
version: v2
date: 2026-09-02
status: complete
---

# Implementation Review: CodeFabric relational data fabric v5 WP45 remediation

## Provenance and Review Scope

This independent, read-only follow-up reviews the final WP45 candidate against WP45 of the v5
plan, the accepted daemon/gRPC/FastMCP target in the 2026-08-31 v3 design review and its current
forward-only amendments, the accepted FastMCP 4 presentation target, the v2 data-fabric
principles, and the findings in the WP45 v1 implementation review. The plan digest is
`fe3259191cf8e90f8593d35ca913145789eed4ca6ba7f7592218a88effb398fb`, the accepted daemon-review
v3 digest is `9e53d7fbcad46e718390324e81b0daf15e3dd4a071f8e6d8e89fa9e405edbe4e`, the FastMCP review-v2
digest is `202329441a517e097ac3a045cbf1022bf05242c8de2e8f2e2f58d5ecd3b9ee6f`, and the principles
digest is `eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5`.

Review was performed at repository HEAD `c277952ca83d0045a7f3990634da928d8e0979e7` in the shared
dirty worktree. Baseline `6e74cfbbe23da73dd110a2adb232276e00f9a3ad` and WP43 proving commit
`13c4bbd0314033b51168c9fa4bd00fbec0593fa7` are both ancestors. Execution state remains
`executing`, WP44 remains `in_progress`, and WP45 remains `not_started` with no proving commit.
That is the correct pre-transaction posture for candidate review and is not a finding. This report
is the review's only repository edit.

The focused surface is the shared Rust daemon/query/resource authority, Python reconnecting
daemon client, generated-client UDS boundary, exact four WP45 recipes, gate-filter census, and the
causal tests added to close IR-001 through IR-003. Key final-byte digests include
`src/query_service.rs` `4501cf428346d6a69de7575726f973020a77aee73cec51e80d3ada9e6d23d6d4`,
`src/fabric/query_coordinator.rs`
`65590c00cbf63dd07fab09f961035219d8937cc3af6675007372816b063006d9`, Python daemon client
`95004c7148bbc8cadc7e5030169f1483534ce75101bedfb91a1ae3380641eecc`, and `justfile`
`828cd6528879d539a6db5e4ca00f0a8021cd083649722e04c8b2ff4f34b37358`.

## Executive Summary

All three major findings from the v1 review are closed on final bytes. One absolute operation
deadline now bounds initial Python connection, watch, reconnect settings renewal, replacement
connection, Rust queued execution, and every blocking watch/read stream step. Timeout transitions
close queued work durably and release task/result/admission reservations. Challenge expiry remains
a typed outcome after unrelated pruning because a bounded, authorization-bound tombstone replaces
the expired live record. The exact four WP45 recipes now own the causal queue, stream, reconnect,
challenge-pruning, and private-journal/public-sequence regressions, and the selector census parser
correctly attributes dependency-bearing and parameterized Just recipes.

The final tests drive `execute_accepted_query`, `watch_next`, and `read_next` themselves, with
blockers inside the production bounded closures and outer fail-fast timeouts. They are not helper
tests which could remain green after removal of the production deadline enforcement. The final
governed recipes retain real generated Rust and Python client traversal through Tonic over UDS.
No new blocking correctness, operations, architecture, library-use, decommission, or proof gap was
found in the reviewed WP45 bytes.

## Verdict

**Approved.** IR-001, IR-002, and IR-003 are closed. The current candidate satisfies WP45's
reviewed implementation and packet-local proof obligations. Approval applies to the fingerprinted
dirty-tree candidate; it does not itself create a proving commit or authorize a premature state
transition.

## Gate and Evidence Assessment

| Evidence | Final result | Assessment |
|---|---:|---|
| `just fastmcp4-daemon-wire-contract-check` | pass | The source/descriptor/generated contract and both generated clients traverse the real Tonic UDS boundary. |
| `just fastmcp4-atomic-start-check` | pass | Pure validation and one closed atomic start outcome remain enforced. |
| `just fastmcp4-resource-authority-check` | pass | Daemon handles, per-use authority, bounded tombstones, restart/reissue, and cleanup remain exact. |
| `just fastmcp4-daemon-security-recovery-check` | pass | Final selection includes 2 generated-client UDS cases, 25 Rust cases, 1 interceptor case, and 6 Python cases, including every v1 remediation fault. |
| Exact four recipes in one final-byte chain | pass | No recipe is being inferred from a neighboring WP46/WP47 gate. |
| `just gate-filter-census` | pass; 1 parser regression, 79 recipes, 97 selectors | Live Just attribution equals the committed census and every name-coupled selector fails on zero selection. |
| Full `codefabric-cpg-mcp/tests/test_daemon_client.py` | pass; 15 tests | Strict decoding, safe status handling, reconnect/no-resubmit, and the absolute reconnect budget all pass together. |
| `just root-fmt` and scoped `git diff --check` | pass | The reviewed bytes are formatted and have no whitespace errors. |
| Scoped `ast-grep scan --config sgconfig.yml` over query/resource/proto/client files | pass | No structural-governance finding exists in the final WP45 semantic surface. |
| Scoped textual and structural legacy searches | pass | Removed projection/public-attempt/Python-lease/challenge-JSON authority remains absent; internal Rust lease tokens remain private. |

The exact terminal recipe results were verified on the final candidate after the causal-test and
census repairs. A repository-wide structural scan was not substituted for these packet oracles;
its unrelated in-progress WP44/provider/supervisor diagnostics are outside this follow-up's WP45
acceptance boundary and do not change this verdict.

## Finding Index

| ID | v1 severity | v2 disposition | Closure basis |
|---|---|---|---|
| IR-001 | major | closed | One absolute deadline reaches Python reconnect and actual Rust queue/watch/read state machines; timeout releases owned resources. |
| IR-002 | major | closed | Bounded typed challenge tombstones preserve expiry classification across unrelated pruning. |
| IR-003 | major | closed | All required causal regressions are selected by the exact WP45 recipes, with independently tested census attribution. |

No new open findings were identified.

## Findings

### IR-001 — Closed: one absolute budget now bounds every owned blocking phase

`CpgDaemonClient.watch_query` creates the operation deadline before initial connection work and
passes only remaining time into connection, `Watch`, progress delivery, and `_reconnect`.
`_reconnect` bounds settings renewal, channel closure, replacement readiness, and Handshake under
that same deadline; it resumes observation and never resubmits `StartQuery`.

On the Rust side, `execute_accepted_query` carries its absolute deadline through queued admission
via `await_running_until` and backend execution. A queue timeout durably closes the query and the
coordinator's terminal transition releases queue, task, and result reservations. `watch_next` and
`read_next` wrap authorization, cursor/event lookup, phase waiting, sleep, event conversion, and
result reads with `bounded_stream_step`. Both states own their data-admission permit as an
`Option<OwnedSemaphorePermit>` and take/drop it immediately on timeout or other terminal error.

The three Rust regressions are causally coupled to production code:

- `wp45_queued_execution_deadline_releases_reservations` invokes `execute_accepted_query`, blocks
  real queued work, and proves terminal closure plus reservation reuse.
- `wp45_watch_iteration_deadline_releases_admission` constructs real `WatchState`, injects a
  one-shot blocker inside the exact bounded production closure, calls `watch_next`, and proves
  typed deadline plus permit reuse.
- `wp45_read_iteration_deadline_releases_admission` does the corresponding operation through real
  `ReadState` and `read_next`.

The stream tests also wrap the state-machine call in a 250 ms fail-fast timeout, so deleting or
moving the production deadline wrapper outside the blocked await makes the regression fail rather
than hang. The Python test delays replacement settings beyond a 50 ms lifetime budget, requires a
typed deadline before its outer bound, and asserts that the initial start-leg count is unchanged.
This closes both the lifetime and prompt-release halves of IR-001.

### IR-002 — Closed: expiry survives unrelated pruning under a strict bound

`StartState` now separates live challenge records from minimal `ChallengeTombstone` records with a
closed `ChallengeTombstoneDisposition::{Expired, Used}`. The tombstone retains only the identity
and authorization bindings needed to classify an authentic continuation: challenge, session,
daemon generation, principal/workspace/scope, and request fingerprint. It does not retain the
prepared query/provider payload.

`prune_start_state` first retires old tombstones, then moves newly expired live records one-for-one
into bounded tombstones. `challenge_record_count` covers live and tombstone maps, and issue/round
capacity and collision checks use that combined count. `consume_challenge` authorizes the retained
record before reporting typed expiry; consuming that expired continuation changes it to `Used`, so
the next attempt reports replay rather than expiry again.

`wp45_challenge_expiry_remains_typed_after_unrelated_prune` drives issuance, an unrelated operation
which prunes state, the live-to-expired-tombstone transition, the combined capacity bound, typed
expiry, and the subsequent replay. The outcome therefore no longer depends on unrelated request
ordering, closing IR-002 without retaining unbounded or semantically heavy state.

### IR-003 — Closed: the governed recipes own the causal regressions

`fastmcp4-daemon-security-recovery-check` now explicitly selects all three deadline/resource-
release state-machine tests, the unrelated-prune challenge test, direct expiry/replay, stable
invalid-answer rejection, bounded challenge output, and
`wp45_beh_private_journal_events_do_not_create_public_sequence_gaps`. The latter inserts a private
`PublicationPending` durable event, proves contiguous public ordinals `1, 2, 3`, verifies the
projected cursor checksum and resume, and rejects a nonexistent public sequence `4`. The other
three WP45 recipes continue to own their wire, atomic-start, and resource-authority categories,
and all four depend on the shared real-UDS generated-client traversal.

The review also found and prompted repair of a census attribution defect before this report was
finalized. `scripts/gate_filter_census.py` now recognizes dependency-bearing and parameterized Just
declarations instead of attributing their nextest selectors to a preceding recipe.
`tooling/ci/test_gate_filter_census.py` proves base, dependency-bearing, and parameterized ownership;
the `gate-filter-census` recipe executes that regression before validating the regenerated
79-recipe/97-selector manifest. Thus the committed evidence now proves the four exact recipes,
not merely the presence of the same test names somewhere in the Justfile.

## Outcome and Invariant Matrix

| WP45 outcome or invariant | Final oracle | Assessment |
|---|---|---|
| Sole generated v2 contract and hermetic descriptor/Rust/Python equivalence | wire-contract recipe and shared UDS interop | satisfied |
| Closed atomic Start outcome; Validate remains side-effect-free | atomic-start recipe | satisfied |
| Typed, bounded multi-round challenge and continuation | security-recovery challenge cases | satisfied |
| Expiry/replay classification is independent of unrelated pruning | unrelated-prune causal test | satisfied |
| Private durable journal state projects to contiguous public events/cursors | private-journal/public-sequence causal test | satisfied |
| Daemon-minted handles and per-read/per-release reauthorization | resource-authority recipe | satisfied |
| Bounded release tombstones, restart cleanup, and authorized reissue | resource/security recipes | satisfied |
| Explicit durable cancellation and reserved control admission | security-recovery recipe | satisfied |
| One remaining budget across connection, queue, execution, watch, and read | actual Python/Rust state-machine tests | satisfied |
| Timeout promptly releases queue/task/result/admission resources | causal release/reuse assertions | satisfied |
| Reconnect renews session and observation without resubmitting Start | three Python reconnect tests | satisfied |
| Ready admission linearizes before drain | security-recovery lifecycle case | satisfied |
| Safe typed status, diagnostics, progress, and correlation | wire/security/client tests | satisfied |
| Projection, secret lease, public attempt, `Any`, and challenge-JSON legacy is absent | proto and scoped zero-state searches | satisfied |

## Architecture and Doctrine Assessment

The final bytes preserve the accepted authority split: Rust owns query preparation/execution,
challenge truth, session authorization, public resource records, cancellation, lifecycle, and
durable recovery; Python owns presentation and reconnecting transport only. The repair strengthens
rather than bypasses that split. Deadline exhaustion is now an explicit transition in the owning
Rust state machines, and Python propagates a caller budget instead of inventing a new semantic
timeout. Minimal tombstones preserve an exact external state without resurrecting provider or
prepared-query authority.

This maintains the material obligations behind P3, P11, P13, P16, P20, P23, P25, P27, P30,
P31--P36: typed closed boundaries, bounded work and memory, exact lifecycle transitions, causal
behavioral evidence, and programmatic authority rather than hashes or prose. The implementation
does not restore the displaced Validate-then-Start flow or any compatibility service.

## Library Leverage Assessment

The implementation continues to use Protobuf closed oneofs, generated Rust/Python clients, typed
enums/details, field reservations, and hermetic descriptors instead of ad hoc JSON or prose
decoding. Tonic, Tokio, and grpcio supply the real UDS transport, standard typed status, streaming,
and timeout primitives; the application adds only the domain-owned absolute budget and resource
transition semantics that those libraries cannot infer. Tokio owned semaphore permits make prompt
admission release explicit. No new library or custom scheduling framework is warranted.

Arrow/DataFusion execution remains Rust-owned and receives the same execution deadline after
queue admission. The remediation does not move Arrow pages, relational preparation, or authority
into Python and does not replace programmatic proof with a digest comparison.

## Legacy and Decommission Assessment

Scoped structural and textual searches find no live projection selector/resource kind, Python
resource-lease registry, public MCP/RPC attempt identity, repeated top-level expected-generation
assertion, Protobuf `Any`, or challenge JSON. Removed wire fields remain reserved as evolution
history. Internal Rust lease tokens remain behind the daemon boundary. No v1 runtime or v3
compatibility service participates, and no repair reintroduces the ordinary two-call
Validate-then-Start path.

## Test and Operational Assessment

The important change from v1 is causality, not merely additional green tests. The deadline tests
invoke the exact production state machines and block inside the awaited closures whose bounds they
prove. The challenge test changes pruning order. The journal test inserts the private event which
previously exposed a downstream gap. The Python test makes reconnect settings consume the same
wall-clock budget. Exact Just selectors and the tested census make those cases packet-owned and
zero-selection-safe. Each recipe still performs both generated-client real-UDS traversals, so
direct unit tests supplement rather than replace interoperability.

No test-only semantic implementation or separate mock protocol was introduced. The `cfg(test)`
one-shot blockers are injection seams around the actual bounded awaits; production construction
sets them absent, while mutation of the real bound makes the tests fail under their outer timeout.

## Plan Deviations and Diff Hygiene

There is no adverse WP45 plan deviation. The accepted forward-only choice to revise the unshipped
v2 contract as one transaction remains intact. The current tree is intentionally dirty because
WP44/WP45 implementation is uncommitted; this review does not attribute unrelated files to WP45.
Formatting and scoped diff checks pass. The report's approval is fingerprint-sensitive: a change
to the reviewed query coordinator, query service, Python client, recipes, or census requires the
affected focused oracle to be rerun before proving.

## Required Remediation Order

None for the reviewed WP45 implementation. After this independent approval, the execution owner
may perform the plan's proving-commit and state-update transaction using the final packet evidence.
Those actions are outside this review and have not been performed here.

## Focused Re-Review Scope

No further re-review is required absent byte drift. If the reviewed deadline, challenge,
public-event projection, Python reconnect, Just selector, or census bytes change, rerun the exact
four WP45 recipes plus `just gate-filter-census` and re-review only the affected invariant.
