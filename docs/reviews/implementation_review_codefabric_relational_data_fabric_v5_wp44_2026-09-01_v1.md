---
artifact: implementation-review
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: approved
version: v1
date: 2026-09-01
status: complete
---

# Implementation Review: CodeFabric relational data fabric v5 WP44

## Provenance and Review Scope

This independent, read-only review assesses the completed WP44 working-tree implementation
against WP44 of the v5 plan, the accepted daemon/gRPC/FastMCP boundary review v5, the v2.3
SUITE/LIFE/FAB/SRV/RM authority, and the repository doctrine. The reviewed plan digest is
`fe3259191cf8e90f8593d35ca913145789eed4ca6ba7f7592218a88effb398fb`; the daemon-review-v5
digest is `2c4e819bc416a9fd7fcf5a76928aa17a470a5b296b003e62cf451b170513b7ae`.

Review was completed at repository HEAD `c277952ca83d0045a7f3990634da928d8e0979e7` in a dirty,
shared worktree. WP44 remains correctly `in_progress` with no proving commit while this report
reviews its final candidate bytes. The review covered supervisor policy/grant/session authority,
singleton and durable generation, component-wise filesystem and UDS authority, fd3 transfer,
daemon control, lifecycle and child cleanup, exact activation append/readback reconciliation,
production workspace startup, the WP44 unit and real-process fixtures, and all six named WP44
recipes. No implementation, plan, state, Justfile, or commit was changed; this report is the
review's only repository edit.

The review initially rejected transitional snapshots for unbounded control/rendezvous I/O,
incomplete policy and adapter authority binding, clock-derived generation, path-component and
replacement races, crash-unsafe generation persistence, incomplete adapter cleanup, missing
causal Drain-before-Shutdown, and absent real pre-ready/acknowledgement-loss faults. A later real
startup probe also exposed a release-pin mismatch and missing transient-result/object-store
authority. The implementation owner repaired those items. The verdict below is based only on the
final hash-stable snapshot and fresh reruns after the repairs.

## Executive Summary

WP44 now realizes one supervisor-owned startup and lifecycle path. The public launcher grammar is
closed to launch/activate/cancel/release/status operations; daemon administration remains on the
private authenticated control boundary. Operator policy identity, revision, validity, revocation,
principal, workspace, executable, distribution, process-start, grant, session, and generation
authority are bound and revalidated. Component-wise descriptor authority prevents ancestor
symlink or root-replacement redirection, and owned sockets retain the exact directory and inode
identity needed for safe cleanup.

Supervisor generations are durable and monotone across clock faults and interrupted writes.
Rendezvous and daemon-control I/O are bounded; daemon-control failure closes admission, invalidates
the generation, and drives owned-child recovery. Abandoned adapters receive bounded TERM, identity
polling, KILL when required, and pid-gone confirmation before capacity is released. Production
signal handling causally performs Drain before Shutdown and joins owned endpoints.

Fresh startup stays closed until the recovered command actor appends and reads back one exact
activation horizon. Real-process evidence covers direct success, post-durable-append
acknowledgement loss, pre-ready failure, restart, drain, and abrupt exit. The production workspace
probe additionally prepares and physically executes an exact semantic request to nonempty Arrow
output and returns one typed guarded-input requirement, confirming that release pins, transient
result bindings, and the reduced child's exact object-store capability compose at runtime.

## Verdict

**Approved.** No blocker, major, or minor WP44 finding remains on the final reviewed snapshot. The
implementation satisfies WP44's startup, exact fresh-activation readiness, supervisor authority,
failure containment, and real-process proof obligations and may proceed to the proving-commit and
state-completion transaction.

This approval is fingerprint-sensitive. Any subsequent change to the reviewed WP44 source,
fixtures, or recipe selection requires the affected oracle and review evidence to be rerun. It
does not certify the WP45 wire contract, WP46 FastMCP presentation behavior, or the full WP47
installed-wheel vertical.

## Outcome and Invariant Matrix

| WP44 obligation | Independent evidence | Assessment |
|---|---|---|
| One strict supervisor/daemon startup authority | closed process grammar, named policy store, singleton identity, peer/generation hello, full executable/distribution/process-start binding | satisfied |
| Safe private runtime and policy paths | component-wise descriptor traversal, retained root/parent identity, symlink/replacement/cross-device/live-socket faults | satisfied |
| Durable monotone supervisor generation | clock-fault and interrupted-inactive-slot tests plus exact generation/session rejection | satisfied |
| Bounded launch and control lifecycle | silent-peer deadline, control-timeout escalation, capacity reservation, bounded TERM-to-KILL cleanup, pid-gone assertion | satisfied |
| Exact fresh activation readiness | real empty-head startup reaches Ready only after exact durable append/readback; seed/latest/hash/receipt substitutions fail closed | satisfied |
| Unknown-outcome reconciliation | unit ambiguity contract and real post-append/pre-readback acknowledgement-loss process recover the exact horizon | satisfied |
| Causal drain, restart, and cleanup | real daemon rejects Shutdown before Drain; restart, pre-ready failure, abrupt exit, endpoint join, and replacement-inode cases pass | satisfied |
| Production query-runtime coherence | fresh production workspace executes an exact request to nonempty Arrow and yields one guarded requirement | satisfied |
| Packet recipe reachability | INT, BEH, NEG, OPS, contract, and platform recipes all select nonzero target tests | satisfied |

## Findings

No open findings.

## Gate and Evidence Assessment

| Evidence | Fresh result | What it proves |
|---|---:|---|
| `just fastmcp4-startup-contract-integrity-check` | pass; 7/7 library and 2/2 real-process cases | strict binary/library/settings grammar, policy/session authority, exact activation transaction, and project-venv distribution/process identity |
| `just fresh-activation-ready-reconciliation-check` | pass; 1/1 real process | empty-head startup reaches Ready only after durable exact activation readback |
| `just supervisor-startup-boundary-rejection-check` | pass; 28/28 library and 1/1 direct-daemon process | unsafe policy/socket/path, substitution, replay, generation, control, cleanup, and direct-start faults fail closed |
| `just supervisor-restart-join-operations-check` | pass; 3/3 library and 5/5 real processes | exact ambiguity reconciliation, restart, causal Drain, pre-ready cleanup, abrupt-exit capacity recovery, and joined endpoints |
| `just supervisor-launch-contract-check` | pass; 114/114 tests; 5 claims, 10 fixtures, 77 negative scenarios | retained supervisor policy/singleton/control/fd3/restart acceptance contract remains executable and fault-complete |
| `just supervisor-launch-platform-check` | pass; 39/39 library cases | every current WP44 INT/BEH/NEG/OPS unit selector is platform-reachable, including real directional fd3 behavior |
| exact production workspace execution probe | pass; 1/1, 795 filtered | release-bound startup reaches physical execution, returns nonempty Arrow, and produces one guarded-input requirement |
| `cargo check --locked --lib` | pass | final production library compiles on the locked graph |
| targeted `rustfmt --check` over seven WP44 production sources | pass | reviewed production source formatting is clean |
| `git diff --check` | pass | the shared working diff has no whitespace errors |
| final SHA-256 comparison | pass | the eight released WP44 production/recipe fingerprints remained fixed throughout the fresh gate reruns |
| targeted `validate_review` contract | pass | this report has the required implementation-review schema and approved verdict vocabulary |

Concurrent WP47-only additions changed the shared integration-test file after the first review
sweep without changing the WP44 selector bodies or the released WP44 production/recipe
fingerprints. The four integration-bearing WP44 recipes were therefore rerun after that change;
all passed with the same counts. Because WP47 remains active in the same file, this review
deliberately does not bind approval to the whole-file digest: a later edit to any WP44 selector
body requires its affected recipe to be rerun. At review time the seven WP44 production files and
the complete shared integration file passed rustfmt directly, and `git diff --check` passed
repository-wide.

The aggregate `just artifacts-check` was also attempted, but its prerequisite
`governance-tooling-lint` stops on concurrent formatting drift in the WP48-owned
`tooling/ci/fastmcp4_production_evidence.py` and its test. It does not reach artifact validation.
The targeted review-contract validator passes, and the unrelated WP48 formatting diagnostics are
not a WP44 finding.

## Legacy and Decommission Assessment

The repaired path retains the recovered command actor, activation writer/fence/readback,
supervisor, session, process, and owned-socket primitives behind target-owned APIs. It does not
restore ontology, bootstrap, model, static-schema, or digest-as-proof authority. Startup readiness
is derived from one coherent exact activation event and table-version vector, while ambiguous
outcomes reconcile by durable transaction identity rather than blind append retry or latest-value
assembly. The WP44 recipes do not claim the later gRPC/FastMCP surface.

## Residual Proof Boundary and Safe Next Action

The safe next action is to preserve the reviewed fingerprints in the WP44 proving commit, rerun
the six named recipes from that commit, run the executor's cumulative root-format and retained
WP29/WP30 zero-state gates once concurrent WP45-WP47 edits are quiescent, and atomically record the
proving commit and WP44 completion in v5 execution state. Installed-wheel no-source-import proof,
wire evolution, adapter presentation, purge, and performance certification remain owned by later
packets and must not be inferred from this approval.
