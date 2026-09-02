---
artifact: plan-audit
date: 2026-09-01
version: v1
status: complete
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: ready
---

# Plan Audit: execution-proved relational data fabric implementation plan v5

## Provenance and Scope

This independent pre-activation audit reviewed the complete v5 plan, its FastMCP 4 design v1 and
accepted v2 amendment, the composed daemon/gRPC design v5, v4 status review v2, active v4 plan, all
declared-input identities, current v2.2 suite authority relevant to the successor boundary, the
P1--P36 doctrine, the exact FastMCP/Pydantic, grpcio/Protobuf, and Tonic references, and the current
dirty tree. It challenges plan readiness; it does not certify behavior that WP43--WP52 have not yet
implemented.

Baseline `6e74cfbbe23da73dd110a2adb232276e00f9a3ad` equals current `HEAD` and is ancestral. The
shared working tree was already extensively dirty. This audit changed no plan, design, state,
production, test, configuration, or lock file and did not commit.

Validation evidence:

- every declared-input SHA-256 matches the current file;
- direct `validate_review` accepts design v2 as `aligned`, and direct `validate_plan` accepts the
  draft, all declared inputs, ten packets, five milestones, four decommission batches, and four
  unique oracle names per packet;
- `just plan-dependency-check <v5-plan>` reports ten packets and zero disjoint-phase overlaps;
- the corrected literal WP44 `ast-grep outline` pipeline exits 0 over both live activation modules;
- isolated exact-stack probes on Python 3.14.7 resolve FastMCP 4.0.0, MCP 2.1.1, and Pydantic
  2.13.4; confirm `tasks=False`, `InputRequiredResult`, `RequestStateSecurity`, request context,
  completion types, and operation middleware; distinguish modern `2026-07-28` from legacy
  `2025-11-25` before tool dispatch; and observe an empty application extension registry plus the
  sole framework-owned `io.modelcontextprotocol/ui: {}` modern capability;
- `just artifacts-check` passes the currently active v4 artifact corpus and its 15 contract tests;
  the inactive v5 draft is instead validated directly because its state is intentionally created
  only by activation; and
- `git diff --check` and `git diff --cached --check` pass.

No broad implementation gate was run: this is a documentation-only pre-activation audit, and the
plan correctly assigns not-yet-existing successor recipes and real-process behavior to its packets.

## Executive Summary

V5 is dependency-closed and executable. It preserves the proven relational substrate while
replacing invalidated FastMCP 3 serving work with a synchronized v2.3 design release, exact fresh
activation repair, one atomic daemon start outcome, a modern-only FastMCP 4 presentation cell, a
real installed-process vertical, independent evidence, multidimensional purge, measured budgets,
FreshActivation proof, and terminal certification.

The audit found two activation blockers and one minor factual defect while the draft was still
being challenged. All three were corrected before this report: the impossible literal-empty
FastMCP extension expectation now reflects the exact native framework behavior; the design
amendment now uses a schema-valid verdict and a fresh declared hash; and WP44 now names live
activation modules. Revalidation passes. No open blocker, major, or minor finding remains.

## Readiness Verdict

**Verdict: `ready`.** The exact audited v5 plan is ready for approval and atomic activation. Its
future state should be created by the repository activation transaction; v4 completion labels must
not be copied into it. This verdict does not claim implementation completion or serving
certification.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | library grounding / executability | design v1; plan I5-08, LD5-05, WP46, WP49 | closed |
| F-002 | blocker | artifact contract | design v2 frontmatter; plan declared inputs | closed |
| F-003 | minor | factuality / impact preflight | WP44 | closed |

## Findings

### F-001 — Literal empty extension advertisement was impossible on FastMCP 4.0.0

**Finding:** The original target required no extension advertisement, but an otherwise bare
`FastMCP("probe", tasks=False)` has an empty application registry while
`fastmcp.server.low_level.LowLevelServer.get_capabilities` unconditionally adds the empty
framework-owned `io.modelcontextprotocol/ui` capability on the modern protocol. The pinned release
has no supported public disable switch. The former clause was therefore unimplementable without a
private monkey patch or replacement protocol stack.

**Required resolution:** Distinguish application/custom extension and UI-component zero state from
the one inert framework advertisement; reject any other identifier, non-empty settings,
CodeFabric registration, UI component, or semantic consumption. Design v2 and the revised plan do
exactly this and assign separate live-discovery, application-registry, and catalog observations.

**Revalidation:** An offline isolated exact-stack probe confirmed versions, `server._extensions ==
{}`, and modern `server._mcp_server.get_capabilities(protocol_version="2026-07-28").extensions ==
{"io.modelcontextprotocol/ui": {}}`. The revised declared-input hash and plan validation pass.

### F-002 — The first amendment verdict was outside the review schema

**Finding:** Design v2 initially used `targeted-revision-accepted`, while the repository permits
only `aligned`, `aligned-with-findings`, `revision-required`, or `redesign-required` for an
`interface-design-review`. Direct validation failed, so the plan's primary design input was not a
valid review artifact.

**Required resolution:** Use the schema-valid accepted-target verdict `aligned`, refresh the
immutable design digest in v5, and validate both artifacts. This correction is present.

**Revalidation:** Direct calls to `validate_review(root, design_v2)` and
`validate_plan(root, plan_v5)` both exit 0; the current design digest is
`202329441a517e097ac3a045cbf1022bf05242c8de2e8f2e2f58d5ecd3b9ee6f` and matches the plan.

### F-003 — WP44 named a nonexistent activation source path

**Finding:** WP44 originally included `src/activation_control.rs`. The live implementation is under
`src/fabric/activation_control_delta.rs` and `src/fabric/activation_transaction.rs`. Because the
same command also scanned all of `src/fabric`, coverage was not ambiguous, making this a mechanical
preflight defect rather than a packet-design gap.

**Required resolution:** Name the two live files while retaining the directory scan. The revised
WP44 does so.

**Revalidation:** The literal revised `ast-grep outline ... | sed -n '1,320p'` pipeline exits 0 and
the stale path is absent from the plan.

## Target-Design Assessment

The target is coherent and clean-sheet defensible. One workspace supervisor and Rust daemon own
launch authority, source/fabric/query/resource state, admission, cancellation, recovery, and
durability. One per-agent STDIO FastMCP process owns presentation only through an application-owned
`DaemonPort`. Atomic `Accepted | InputRequired | Rejected` start semantics remove the validate/start
race; sealed FastMCP request state carries only a bounded daemon continuation, while every leg is
reauthorized by the daemon. Public resource handles are daemon-minted after lease creation, and
completion remains capped, advisory, authorization-filtered, and revalidated at use.

The v2 amendment improves rather than weakens the design: it records unavoidable native metadata
truthfully without treating advertisement as registration, behavior, authorization, or proof.

## Library Capability Assessment

The selected FastMCP 4 APIs exist on the exact stack. `FastMCP` accepts `tasks=False`, strict input
validation, request-state security, middleware, and completion configuration; modern guard result,
context input responses, sealed state, and completion types are present. Operation middleware can
read the negotiated protocol version and reject a legacy tool call before the tool body. The plan
uses native guard/completion/context facilities and rejects sessions, task workers, cache, auth,
gateway, provider, transform, App, and UI-component authority.

The generated Protobuf boundary, one long-lived `grpc.aio` channel, explicit deadlines and
cancellation, oneof failure closure, Tonic UDS process boundary, and application-level resumable
watch identity align with the pinned references. No library-incompatible mandatory API remains.

## Work-Packet and Impact Assessment

The ten-packet chain is intentionally serialized where the dirty tree and contract changes
overlap: issue authority (WP43), repair startup (WP44), revise wire/daemon ports (WP45), build the
adapter (WP46), prove the installed vertical (WP47), author independent evidence (WP48), purge
predecessors (WP49), measure post-purge behavior (WP50), prove FreshActivation (WP51), then certify
one trusted HEAD (WP52). Each packet names outcome, dependencies, invariants, preflight, known
touches, required work, legacy disposition, exactly four substantive oracle categories, commit
boundary, and replan triggers. The current-tree impact probes cover source, binaries, contracts,
generated output, adapter package/lock, tests, recipes, CI, rules, fixtures, and deployment census.

## Legacy, Transition, and Decommission Assessment

V2.2 remains immutable history until WP43 issues the synchronized v2.3 suite. The target has no
FastMCP 3 or older MCP runtime profile, no Validate-then-Start ordinary path, Python lease map,
duplicate freshness/identity, phantom prompts, adapter semantic authority, dormant handoff, or
fallback. WP49 proves multidimensional zero state only after target consumers exist; WP51 performs
the read-only deployment census and stops for a separate AuthorityHandoff design if a real
predecessor is discovered. This is forward-only and dependency-correct.

## Proof and Validation Assessment

The plan distinguishes structural identity from behavioral proof. Packet oracles require nonzero
selectors and discriminating faults across integrity, positive behavior, negative/failure, and
operations/recovery/resource/performance. The installed-process vertical cannot pass through an
in-memory server or injected daemon fake. Independent expectations, cross-process cancellation,
unknown-outcome activation readback, bounded resources, extension/component distinctions,
decommission envelopes, performance budgets, and one-HEAD terminal aggregation are explicit.

## Doctrine and Anti-Principle Assessment

V5 aligns with P1--P36: authority remains singular and inward; native libraries own their real
boundaries; guarded interaction is causal and replay-aware; advertisement is not enforcement;
expected outcomes are independently authored; dirty-tree and legacy discovery are computed; one
mutation path and forward repair remain mandatory; and governance is executable. It introduces no
new static runtime census, digest-as-correctness claim, Python semantic owner, second daemon, hidden
compatibility profile, or self-authored proof.

## Top Required Changes

1. Approve and atomically activate the exact validated v5 artifact, creating only the v5 state
   universe through the repository activation transaction.
2. Begin WP43 only after confirming declared-input freshness and current-tree ownership; stop on any
   named replan trigger rather than weakening the target.

## Re-Audit Scope

No further re-audit is required for F-001--F-003. Re-audit if a declared input changes, activation
changes plan text, the exact FastMCP capability observation differs, a required host cannot execute
modern guarded input, the library exposes materially different supported behavior, or deployment
census finds a real predecessor/distributed profile.
