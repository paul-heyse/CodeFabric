---
artifact: plan-audit
date: 2026-09-01
version: v1
status: complete
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
verdict: needs-revision
---

# Plan Audit: execution-proved relational data fabric implementation plan v4

## Provenance and Scope

This audit independently reviewed the complete 1,925-line plan against the accepted v4 boundary
amendment and its incorporated v3 design, committed `HEAD`
`6a76b5cff3d84e8249e5bedaa52a17f2abb816dd`, the eight-artifact authoritative v2.1 suite, the
repository specification and `AGENTS.md`, the complete P1--P36 data-fabric doctrine, and the pinned
DataFusion/Arrow, delta-rs, Tonic, grpcio/Protobuf, FastMCP, and Pydantic references. The current
source, manifests, lockfiles, generated-contract surfaces, command inventory, and execution-artifact
validators were inspected independently of the plan's claims.

The plan records baseline `f12329f05e3678698ff9a43ec4f69f95f42db12f`; current `HEAD` is its
one-child committed successor and contains the deliberate pivot (660 changed files). The plan and
accepted amendment are in that commit, so this drift is declared planning context rather than
unattributed implementation evidence. The worktree was clean before this audit; this report is the
only audit-owned change.

Validation evidence:

- direct plan and review-artifact validation passed; the plan yields 14 unique packets, milestones
  M02--M07, decommission batches DB09--DB14, and fresh declared-input digests;
- `just plan-dependency-check <plan>` passed with 14 packets and zero disjoint-phase overlaps;
- `just stable-graph-check` passed against the live pinned dependency graph;
- `just root-check-fast` passed; `just root-fmt` failed on the pre-existing `src/daemon.rs:776`
  formatting difference already disclosed in plan section 1.3;
- the ordinary `just artifacts-check` passed for the still-active v3 plan. A targeted whole-artifact
  check of inactive v4 exited 1 because the intentionally absent future v4 state file is opened
  before draft-plan validation; direct v4 plan/design validation passed, so that tool limitation is
  not used as a plan finding.

No earlier audit of this exact v4 plan was found.

## Executive Summary

The plan is structurally disciplined and largely faithful to the accepted forward-only target. It
correctly excludes WP28 and M01, migrates no v3 completion, restores no v1 operability, preserves v1
wire material only as non-live allocation history, and makes DB09--DB14 physical decommission and
target-only FreshActivation release obligations. Its DataFusion/Arrow/Delta, Tonic/grpcio/Protobuf,
and FastMCP/Pydantic choices are grounded in the live pins and use the libraries in their intended
roles.

It is not ready for activation. The accepted design makes an inherited supervisor-control
socketpair and operationally authenticated single-use launch grant the root of every daemon session,
but neither the design nor WP33/WP37 names the launcher/supervisor authority or closes its
one-daemon/many-adapter process lifecycle. The plan defers discovery of that load-bearing security
and deployment architecture until the omnibus production vertical and calls failure a replan
trigger. Execution would therefore have to invent an authority boundary that determines process
topology, authorization input, descriptor delivery, restart, revocation, and supported-platform
behavior. A same-Cargo-package Rust launcher/supervisor can close this without a new Cargo root, but
that decision and lifecycle must be accepted before activation.

One non-blocking naming observation remains: the plan allocates a successor suite after v2.1 while
naming the sole compiled semantic authority `CodeFabricV21Release` throughout the new target. The
runtime owner should be version-neutral or be named and identity-bound to the version WP33 actually
allocates.

## Readiness Verdict

**Verdict: `needs-revision`.** Do not activate this plan as written. The overall target architecture
does not require redesign: a focused accepted-design amendment plus a revised plan can resolve the
operational trust root. The compiled-release identity observation can be clarified in the same
revision but does not independently block activation. Forward-only implementation remains the
required direction; neither finding authorizes v1 operability, WP28, M01, or a predecessor handoff
path.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | design/operations/sequence | design v4 section 1.2; WP33, WP37, M05 | open |
| F-002 | observation | design/factuality | outcome 1.1; WP29, WP31, WP33 | open |

## Findings

### F-001 — The launch-grant trust root has no executable operational owner

**Severity:** blocker  
**Category:** design, operations, sequence  
**Scope:** accepted design v4 section 1.2; plan WP33 required change 4; WP37 required change 3 and
replan trigger; M05

**Finding:** The accepted design requires an operationally authenticated grant flow through an
inherited supervisor-control socketpair, followed by a single-use bootstrap descriptor and a
session that reauthorizes every RPC. It does not identify the supervisor/launcher, the authority
from which principal/workspace/operation grants are derived, or how the repository's one central
daemon per workspace and one FastMCP process per agent are parented and reconnected. The plan adds
only the supervisor-control record schema in WP33, then assigns the socketpair, grant minting,
daemon, Python adapter, packaging, and real vertical to WP37; inability to deliver the grant by a
supported launcher is deferred to a late replan trigger.

Current code confirms that no inherited operational authority can simply be reused:
`codefabricd` exposes only `serve`/`check-config`, the Python module is the direct STDIO entrypoint,
and there is no `RegisterLaunchGrant`, `RevokePrincipal`, `AdvanceGeneration`, control socketpair,
or bootstrap-descriptor implementation. Existing forward-cutover `SupervisorObservation` values
are deployment evidence scheduled for DB14 removal, not a parent process or grant authority.

A wrapper launched once per agent that starts its own daemon would violate the one-daemon-per-
workspace invariant. Conversely, an unnamed socketpair held by a long-lived workspace supervisor
cannot accept unrelated future launcher processes without an explicitly designed rendezvous or a
supervisor that itself launches each adapter. This is therefore not a detail that WP37 can safely
infer while implementing the terminal vertical. It is the root of the security and process
architecture and blocks I4-13/session denial and the real M05 proof.

**Required resolution:** Amend the accepted target, then revise the plan, to name a same-package
Rust launcher/supervisor command (or another explicitly accepted authority) as the sole grant root.
No new Cargo root is required. The amendment and packet impact must specify:

1. private runtime-directory ownership and a per-workspace singleton lease/live probe, including
   how later per-agent invocations attach without creating a second daemon;
2. the non-self-declared policy input that authorizes principal, workspace set, operations,
   profiles, bounds, expiry, and revocation generation;
3. creation of a close-on-exec unnamed control socketpair before `codefabricd` spawn, exact endpoint
   ownership, descriptor allowlisting, daemon/supervisor-generation binding, ordering,
   acknowledgement, loss, and fail-closed partial-spawn cleanup;
4. a separate one-shot inherited capability descriptor for each Python adapter, immediate close and
   capability erasure after consume, byte-for-byte STDIO proxying with protocol-only stdout,
   bounded stderr, EOF/signal/deadline propagation, and joined child reaping;
5. adapter exit/revocation, accepted-work survival, reconnect-with-new-grant, daemon restart/
   generation invalidation, shared-daemon shutdown, and stale lock/socket recovery semantics;
6. Linux and macOS behavior for peer UID and optional PID, descriptor inheritance/CLOEXEC, process
   groups/signals, private-directory modes, no-follow owned UDS lifecycle, singleton races, and the
   design's narrowly allowed owner-verified `0600` fallback; and
7. explicit source/package/service/configuration touch surfaces plus real subprocess positive and
   negative proof. If an independently launched adapter must contact an existing supervisor, name
   and authenticate that rendezvous; an unnamed socketpair alone is not such a rendezvous.

**Revalidation:**

```bash
just supervisor-launch-contract-check && just supervisor-launch-platform-check && just session-uds-presentation-boundary-rejection-check
```

### F-002 — The successor runtime owner is still named for suite v2.1

**Severity:** observation  
**Category:** design, factuality  
**Scope:** plan outcome 1.1, planned authority evolution 2.1, WP29, WP31, WP33

**Finding:** WP33 must allocate the next collision-free synchronized suite after v2.1 and make it
the sole current authority, but the outcome and production packets continue to require
`CodeFabricV21Release` as the sole compiled semantic owner. That name came from the incorporated v3
design when v2.1 was current. Once WP33 selects a successor, retaining a v2.1-labelled production
owner obscures which suite is compiled and invites either stale semantic authority or a misleading
permanent type/API. This conflicts with the plan's exact successor-identity and one-authority
posture even if the implementation behind the name happens to be updated.

**Required resolution:** No independent readiness action is required. When revising for F-001, use
a version-neutral compiled owner whose immutable `SuiteIdentity` is the exact WP33-selected suite,
or allocate the concrete versioned owner only after WP33 selects the collision-free successor.
Prove exhaustive provider, transformation, analysis, proof, and query construction against that
exact selected suite identity.

**Revalidation:**

```bash
just compiled-release-suite-identity-check && just successor-authority-expectation-integrity-check
```

## Target-Design Assessment

Apart from F-001, the target is coherent: one compiled semantic authority, phase-typed construction,
one atomic active workspace, exact Delta selection, one bounded query coordinator, immutable result
packages, clean v2 control/resource transport, and presentation-only FastMCP form an inward-pointing
authority graph. The forward-only amendment is explicit and consistently propagated through the
plan. F-001 is a missing root node, not evidence that the chosen daemon/gRPC/FastMCP architecture is
wrong; a named same-package supervisor/launcher is sufficient if it satisfies the lifecycle above.

## Library Capability Assessment

The live graph and manifests confirm DataFusion 55.0.0, Arrow/Parquet 59.2.0, `object_store`
0.13.2, exact delta-rs revision `43a0cf10`, Tonic 0.14.6/Prost 0.14.4, grpcio 1.83.0,
protobuf 7.36.0, FastMCP 3.4.7, and Pydantic 2.13.4. The plan correctly uses streaming DataFusion
execution, fresh independently decodable Arrow IPC pages, manifest-last create-only publication,
exact Delta versions, generated bilateral Protobuf, Tonic UDS request extensions and health,
one `grpc.aio` channel, bounded FastMCP resources, and strict/frozen Pydantic presentation models.
No unavailable selected library API was found. The Tonic reference's platform warning that peer PID
is optional reinforces F-001's Linux/macOS requirement; PID cannot be the portable principal root.

## Work-Packet and Impact Assessment

The parsed 14-packet DAG is dependency-closed and its planned parallelism has no declared
disjoint-phase overlap. Current-tree observations in section 1.3 are accurate: the actual daemon
entry still reaches the error-only `serve` path, the alternate composition is not the production
entry, and query/session/result ownership retains displaced structures. WP37 is intentionally one
contract transaction and has a useful internal order, but its impact inventory omits the concrete
launcher/supervisor binary or subcommand, singleton/control rendezvous, adapter-launch policy,
platform configuration, and MCP-host invocation surface required by F-001. Those surfaces must be
added in the revised plan.

## Legacy Transition Decommission Assessment

The legacy disposition is ready. V1 operability, translation, dual service registration, runtime
generation, client/server tests, and package payload are deletion targets; only immutable proto,
descriptor, fixture, and allocation history may remain non-live. WP28 and M01 are absent from the
packet, state, milestone, and oracle universe. DB14 deletes dormant predecessor handoff authority
after target-only FreshActivation proof. Nothing in this audit requests compatibility restoration,
predecessor resurrection, or old behavior as an oracle.

## Proof and Validation Assessment

The plan allocates four substantive oracle categories to each packet, discriminating faults,
nonzero selection, exact decoded semantic expectations, real process verticals, bounded resource
evidence, clean reconstruction, and multidimensional zero state. The final gate matrix is
appropriately broader than routine compilation. WP42's certification implementation must preserve
the current non-recursive pattern: execute predecessor packet oracles and independently implement
the four WP42 terminal surfaces rather than invoking `relational-fabric-v4-certification` from
itself. F-001 is the only proof blocker because the session denial and terminal process oracles lack
a defined authority to exercise.

## Doctrine and Anti-Principle Assessment

The plan substantially satisfies P1--P36, especially one semantic authority (P3), proof at the
authority boundary (P13/P25), native execution and staticness (P26--P31), construction over
attestation (P32), one semantic mutation path (P34), inward acyclic dependencies (P35), and
executable governance (P36). F-001 leaves the operational authentication root outside that
one-authority/executable-governance model; F-002 weakens exact conceptual identity after suite
versioning. No fact/judgment collapse, absence-as-proof, Python semantic authority, opaque payload,
whole-result materialization, digest-as-semantics, or compatibility-first anti-principle is planned.

## Top Required Changes

1. Accept and plan the concrete same-package supervisor/launcher authority and its one-daemon,
   many-adapter, inherited-FD, restart/revocation, owned-UDS, and Linux/macOS lifecycle.
2. Re-run plan/review artifact validation, dependency closure, F-001's revalidation surfaces, and a
   focused independent re-audit before activation. Do not create v4 state or change the active-plan
   pointer until then. The F-002 naming clarification may be included in that revision.

## Re-Audit Scope

A follow-up audit can be focused on the accepted design amendment, the revised WP33/WP37 process
and impact contracts, the compiled-release/suite identity correction, the updated DAG/oracles, and
input freshness. Recheck that the correction adds no Cargo root, reusable env/argv credential,
per-agent daemon, v1 runtime path, WP28, M01, or predecessor-handoff authority; the remaining target,
library, legacy, doctrine, and proof assessments need only be rerun for induced drift.
