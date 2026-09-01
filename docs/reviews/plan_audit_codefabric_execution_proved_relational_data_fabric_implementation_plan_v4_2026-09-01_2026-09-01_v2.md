---
artifact: plan-audit
date: 2026-09-01
version: v2
status: complete
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
verdict: ready
---

# Plan Audit: execution-proved relational data fabric implementation plan v4

## Provenance and Scope

This focused re-audit reviewed the complete revised plan against accepted interface-design review
v5 and its incorporated v4/v3 target. It re-evaluates the prior v1 audit's F-001 operational
supervisor gap and F-002 compiled-release naming observation, then checks induced plan, dependency,
proof, platform, legacy, and doctrine drift. It does not re-open unaffected architectural choices
or certify implementation that the plan has not yet executed.

The planning baseline is committed `HEAD`
`6a76b5cff3d84e8249e5bedaa52a17f2abb816dd`. Current changes in scope are the revised plan, new v5
design amendment, and prior audit report; no production-code change has occurred since that
baseline. The deliberately absent future v4 state remains valid for a draft inactive plan.

Validation evidence:

- direct `validate_plan` and `validate_review` calls from
  `tooling/ci/artifact_contracts.py` both exited 0, including all declared-input digest checks;
- `just plan-dependency-check
  docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md`
  exited 0 with 14 packets and zero disjoint-phase overlaps;
- `just stable-graph-check` exited 0;
- `cargo info command-fds@0.3.3` exited 0 and confirmed the exact crate plus its `tokio` feature;
  the plan still requires supported-platform compile and process probes before adoption; and
- a scoped search of the plan found `CompiledSemanticRelease` bound to `SuiteIdentity` and no
  `CodeFabricV21Release` occurrence.

The disclosed `root-fmt` and `root-check` baseline failures remain assigned to WP29. This
documentation-only revision neither changes nor conceals them.

## Executive Summary

The earlier activation blocker is closed. Design v5 now names one same-package
`WorkspaceSupervisor` as the per-workspace daemon and grant authority, one attach-only
`AgentStdioLauncher` per agent, an operator-owned policy source, an authenticated private
rendezvous, an unnamed daemon-control socketpair, fixed-FD adapter capability delivery, and joined
restart/revocation/shutdown behavior. The plan carries those decisions through I4-23, WP33, WP37,
M05, execution order, replan policy, and the final gate matrix. Direct STDIO inheritance replaces
a user-space proxy loop and makes protocol-byte preservation and OS backpressure structural.

The naming observation is also closed. `CompiledSemanticRelease` is version-neutral and bound to
the immutable `SuiteIdentity` selected by WP33; suite `2.2.0` is collision-checked again before
issue.

No blocker or major finding remains. One minor traceability correction is open: four downstream
packets still end their explicit target-invariant ranges before newly added I4-23, although their
dependencies, required work, packet oracles, milestones, and final gates already exercise the
supervisor boundary. This is mechanical and does not make execution unsafe or dependency-open.

## Readiness Verdict

**Verdict: `ready`.** The plan is ready for the mechanical F-003 correction, approval, and atomic
activation. No further design review or broad re-audit is required before execution. This verdict
does not claim that future packet recipes already exist or that production behavior already passes;
the plan explicitly creates and executes those proofs in WP33/WP37 and at WP42.

### Prior-audit disposition

| Prior ID | Prior severity | Disposition | Closure evidence |
|---|---|---|---|
| v1 F-001 | blocker | closed | Design v5 sections 0--6; plan I4-23, WP33 required changes 4--7, WP37 required changes 3--7 and 12--14, M05, and the supervisor final-gate family. |
| v1 F-002 | observation | closed | Plan outcome 1.1, I4-01, WP29, and WP31 use `CompiledSemanticRelease` plus immutable `SuiteIdentity`; the stale v2.1-labelled type is absent from the plan. |

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-003 | minor | proof | WP38, WP40, WP41, WP42 | open |

## Findings

### F-003 — Downstream target-invariant ranges omit the new supervisor invariant

**Severity:** minor  
**Category:** proof  
**Scope:** I4-23; WP38, WP40, WP41, WP42

**Finding:** I4-23 is correctly owned by WP33 and WP37 and is substantively proved by M05 and the
final supervisor gate family. However, WP38 and WP40 still declare `I4-01--I4-22`, WP41 stops its
second range at I4-21, and WP42 declares `I4-01--I4-22`. Those ranges predate I4-23. The surrounding
requirements already rerun WP37 and exercise real supervisor/launcher processes, so this is a
traceability omission rather than a missing behavior or oracle.

**Required resolution:** Add I4-23 to the explicit target-invariant list of WP38, WP40, WP41, and
WP42. In WP38's evidence-family wording, name the supervisor policy/singleton/multi-agent/descriptor
claims so the independent evidence transaction cannot accidentally omit the newly accepted trust
root.

**Revalidation:**

```bash
python3 -c 'from pathlib import Path; t=Path("docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md").read_text(); [(lambda s: (_ for _ in ()).throw(AssertionError(w)) if "I4-23" not in s.split("**Design and library references.**", 1)[0] else None)(t.split(f"### {w} —", 1)[1].split("\n### ", 1)[0]) for w in ("WP38", "WP40", "WP41", "WP42")]'
```

## Target-Design Assessment

The revised target now has an explicit operational root rather than an inherited socketpair with no
owner. Its dependency direction is coherent: operator policy authorizes the Rust supervisor;
supervisor control registers claims with the daemon; a per-agent launcher conveys only a
single-use capability to presentation; the daemon remains sole semantic/session authority; and
FastMCP remains presentation only. Singleton, rendezvous, control-channel loss, adapter exit,
accepted-work survival, generation change, stale recovery, shutdown, and supported-platform
behavior all have named ownership and fail-closed outcomes.

The clean-sheet challenge passes. A shared workspace daemon with an attach-only operational shell
is preferable to per-agent daemons, reusable environment credentials, or Python-owned authority
even without the current implementation. The amendment adds no Cargo root or semantic process.

## Library Capability Assessment

The DataFusion/Arrow/Delta, Tonic/grpcio/Protobuf, and FastMCP/Pydantic decisions are unchanged and
remain grounded in the live pinned graph. The only induced library candidate is `command-fds`
0.3.3. The crates.io index confirms that version and its Tokio feature exist. Design v5 and WP37
treat it as a candidate, not an unproved mandate: exact Linux/macOS compile and process probes must
pass, first-party unsafe remains denied, multithreaded `CLOEXEC` toggling is forbidden, and the
bounded no-follow `0600` one-shot-file path is the sole accepted fallback. Failure of both paths is
a design-reopen trigger.

Safe standard-library `OwnedFd`/`Stdio` ownership covers the daemon-control stdin mapping, while the
fixed-FD adapter case is isolated behind the probed launcher capability. No fabricated or
version-incompatible required API was found.

## Work-Packet and Impact Assessment

The 14-packet DAG remains dependency-closed. WP33 specifies suite, policy, singleton, launch,
inheritance, restart, and negative fixtures before implementation. WP37 then owns the entire real
subprocess transaction and cannot complete through an injected backend or stub daemon. Its impact
probe now includes the administrative binary, daemon binary, supervisor/launcher implementation,
policy/rendezvous/lease/restart surfaces, host configuration, manifests/locks, adapter package, CI,
and platform behavior.

The four focused supervisor checks are correctly subordinate to WP37's four substantive packet
oracles rather than inflating the packet category count. The positive oracle launches one
supervisor, one daemon, and two independently authorized installed adapters; negative and
operational oracles cover descriptor leakage, wrong authority, generation restart, and joined
cleanup. F-003 is the only induced packet-text discrepancy.

## Legacy, Transition, and Decommission Assessment

The revision introduces no compatibility or predecessor regression. V1 runtime generation,
services, clients, package payload, translators, and operability tests remain deletion targets;
only exact non-live protocol/allocation history survives. WP28/M01 remain absent. DB14 still deletes
dormant predecessor-handoff machinery after target-only FreshActivation proof. The new supervisor
is a target lifecycle shell and does not preserve any displaced semantic authority.

## Proof and Validation Assessment

The trust root now has independent positive, negative, operational, and platform proof obligations.
The plan distinguishes specification-time contract checks from implementation-time real subprocess
oracles and carries both into the final matrix. It also preserves nonzero selection, committed
faults, decoded expectations, clean reconstruction, multidimensional zero state, and one trusted
HEAD. Future recipe absence before WP33/WP37 is expected plan scope, not a readiness defect.

F-003 should be corrected so the independent evidence and terminal packet declarations explicitly
name the same invariant their executable gates already prove.

## Doctrine and Anti-Principle Assessment

The amendment strengthens P3/P13/P23 by locating launch, policy, process, session, and semantic
authority explicitly; P25/P30/P36 by requiring denied cases and platform faults; P27/P31 by making
policy and grants causally load-bearing rather than descriptive; P32/P33/P35 by keeping validated
policy and lifecycle types in a same-package Rust operational shell; and P34 by preserving the
single daemon mutation path. It introduces no hand-maintained live census, digest-as-correctness,
self-authored expectation, Python semantic authority, per-agent daemon, or ambient credential.

## Top Required Changes

1. Apply the bounded F-003 target-invariant-range correction before approval or activation.
2. Approve and activate the exact validated plan through the repository transaction; do not migrate
   v3 packet completion or create WP28/M01 state.

## Re-Audit Scope

No further audit is required for F-003 after its executable revalidation passes. Re-audit only if
the correction changes more than the named packet scopes, a declared input drifts, the selected
descriptor-delivery and fallback paths both fail, operator-owned policy cannot be supplied, or a
real predecessor/distributed deployment is discovered.
