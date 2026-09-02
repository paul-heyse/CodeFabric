---
artifact: implementation-review
date: 2026-09-02
version: v2
status: complete
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: approved
---

# Implementation Review: CodeFabric relational data fabric v5 WP48

## Provenance and Review Scope

This is an independent review of the five-entry append-only candidate transaction
`contracts/evidence/relational-fabric-v5/wp48-production-evidence-v2.jsonl` through entry
`e330bf3a880ea0929dbb3bf8822273bf9d770834ba37a83b6f1a676044d5af1d`. The reviewer identity is
`codex-wp48-r2-independent-evidence-reviewer`; this reviewer is neither the WP48 implementation
owner nor the WP43 expectation author.

The review covers exactly: `causality`, `fixture_independence`, `real_process_coverage`,
`exclusions`, `fault_discrimination`, and `limitations`. The accepted FastMCP 4 v1 design and v2
framework-UI amendment, plan v5 WP43/WP47/WP48, the r3 expectation release, and the relevant v2.3
SUITE/FAB/QRY/LIFE/SRV authority were used as acceptance authority. Production source was inspected
only to establish evidence provenance, topology, observation seams, and fault placement.

Candidate commit `14de8d7de13631e542820a03f38427b82c942902` exists, is ancestral to review HEAD, and resolves to
the recorded tree `7afc80528fd87719c797bf090b3303f0c89b5538`. The transaction's five entry digests and
`previous_entry_sha256` chain recompute through the reviewed tip. Its complete candidate-snapshot
input set, runner, runner tests, direct run specifications, and transitive recipe closure match the
recorded bindings. The transaction bytes were not changed by this review.

## Executive Summary

The candidate provides sufficient WP48 development evidence. Sixteen independently issued r3
claims match decoded observations; all sixteen paired negative fixtures are rejected with their
specified mismatch paths and typed error; the eleven plan-required layer faults have nonzero,
successful executions; and clean reconstruction uses a fresh Cargo target, fresh runtime roots,
a newly built installed wheel, target-only descriptor regeneration, a repeated source mutation,
and repeated process restart behavior.

The real presentation observations traverse the production supervisor, copied `codefabric` and
`codefabricd` binaries, attach-only launcher, freshly built non-editable FastMCP wheel under
isolated Python, direct STDIO, generated Tonic client, daemon durable readback, and OS process
census. They observe one workspace daemon, two adapter processes, two channels, distinct sessions
and STDIO streams, guarded input, atomic start, resource and completion authority, cancellation,
reconnect, security denial, and redaction. Component probes supplement this topology only for
intrinsic adapter behavior; they do not replace its positive installed-process evidence.

No blocker, major, minor, or observational finding remains within WP48's declared scope.

## Verdict

**ACCEPTED** for `WP48-development-evidence-not-final-release`.

The implementation-review verdict is `approved`. This does not certify WP49 physical predecessor
purge, WP50 resource/performance results, later FreshActivation/certification packets, another OS,
or the final repository release.

## Gate and Evidence Assessment

| Evidence question | Independent assessment | Executable oracle |
|---|---|---|
| Candidate identity, ancestry, exact tree, bound inputs, runner closure, and append-only chain | Pass; the candidate commit is ancestral, its Git tree matches, every required candidate-snapshot binding recomputes, and all five hashes chain to the reviewed tip. | `PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/fastmcp4_production_evidence.py review-candidate --transaction contracts/evidence/relational-fabric-v5/wp48-production-evidence-v2.jsonl` |
| Sixteen decoded target observations | Pass; every comparison is `matched`, has no mismatch path, and is traced leaf-by-leaf to an executed source class. | `just fastmcp4-production-behavior-check` after independent review finalization |
| Sixteen negative fixtures and eleven required layer faults | Pass; each paired fault is rejected as `RFV5_OBSERVATION_DRIFT`, and the provider, transformation, DataFusion, Delta, activation, start, guard, resource, completion, cancellation, and MCP-projection runs are nonzero and exit zero. | `just fastmcp4-causal-fault-check` after independent review finalization |
| Clean target-only reconstruction | Pass; a fresh `CARGO_TARGET_DIR`, fresh runtime roots, `SCCACHE_RECACHE=1`, installed-wheel-only execution, five real-topology cases, one source-mutation case, and descriptor reproduction are recorded; the ephemeral root is removed. | `just fastmcp4-clean-reconstruction-check` after independent review finalization |
| Independent expectation release | Pass; r3 expected values reside in frozen YAML and the generic expectation validator imports no production adapter, daemon, generated wire, or predecessor acceptance implementation. | `just fastmcp4-independent-expectation-review-check` |
| Candidate integrity and release drift | Pass; exact r3 release bytes, immutable source inputs, runner and recipe closure are bound to the candidate commit rather than descendant working-tree bytes. | `just fastmcp4-production-evidence-integrity-check` after independent review finalization |

The four post-review packet recipes intentionally require a sixth `review_accepted` transaction
entry. This review authors the external review artifacts only; it does not append to or otherwise
modify the candidate transaction.

## Finding Index

No findings.

## Findings

No findings.

## Outcome and Invariant Matrix

| Outcome or invariant | Evidence assessment | Executable oracle |
|---|---|---|
| Exact v2.3 suite and FastMCP 4/MCP/Pydantic/Python identity | Matched, with predecessor suite/pin restoration rejected. | `just fastmcp4-production-evidence-integrity-check` |
| Modern-only admission and exact catalog/schema projection | Real installed admission plus installed introspection match r3; legacy dispatch and forbidden components are discriminated. | `just fastmcp4-production-behavior-check` |
| Guarded input and atomic start | Generated-client and durable-journal observations prove no first-leg acceptance, daemon-owned challenge semantics, one resumed acceptance, pure explicit validation, and one ordinary Start path. | `just fastmcp4-production-behavior-check` |
| Completion, resources, security, and redaction | Installed MCP observations, generated-client authority probes, durable readback, and component sinks match the independent relations and reject cross-authority/leak faults. | `just fastmcp4-causal-fault-check` |
| Cancellation, reconnect, and two-agent isolation | Installed process census and daemon observations show one daemon, two adapter/channel cells, distinct authority, one cancellation, and resume without Start resubmission; corresponding shared/resubmit faults are rejected. | `just fastmcp4-causal-fault-check` |
| Source-to-Delta-to-DataFusion causal layers | Provider, transformation, programmatic query, exact Delta, activation readback, and their layer faults all run with nonzero selection. | `just fastmcp4-production-behavior-check`; `just fastmcp4-causal-fault-check` |
| Target-only reconstruction | Fresh build/package/runtime roots, source mutation, restart, and descriptor reproduction are present without a predecessor model, static adapter schema, stale descriptor, cached epoch, or source-tree adapter import. | `just fastmcp4-clean-reconstruction-check` |
| Independent expectations govern observed behavior | The comparator reads frozen r3 expectations separately from executed observations; normal/fault parity is enforced at the source seam. | `just fastmcp4-independent-expectation-review-check`; `just fastmcp4-negative-fixture-independence-check` |

## Required Scope Assessment

### causality

Observed values are not inferred from command success alone. The transaction embeds decoded normal
observations with per-leaf source records. The real observer produces normal and fault rows from
installed MCP, generated Tonic, durable readback, focused component, and OS process seams. Eleven
separate fault runs cover every layer named by WP48, while all sixteen claim-level fault
observations are compared to independent r3 expectations. Earlier unsuccessful evidence attempts
against commits `70db221d...` and `c516b53c...` remain preserved and were not promoted; the accepted
candidate is the later `14de8d7...` commit and reruns the closed command set.

### fixture_independence

The r3 release records a distinct expectation author and independent falsification reviewer. Its
validator is generic and production-import-free. The r3 correction was prompted by a prior target
execution gap, but issuance and independent review explicitly record static adjudication from the
accepted design, released RPC contract, and pinned references, with no target output copied or used
as expected-value authority. R3 was frozen before this WP48 candidate capture, and its six artifact
hashes are bound from the candidate commit snapshot.

### real_process_coverage

Positive presentation evidence builds and installs one fresh, non-editable adapter wheel, isolates
imports with `python -I`, copies the Cargo-built production executables, starts the production
supervisor and its direct `codefabricd` child, launches the attach-only MCP boundary, and drives
modern JSON-RPC over direct STDIO. `/proc` census confirms one daemon and one adapter/channel per
agent for the two-agent case. Generated Tonic and SQLite readback cover hidden daemon authority that
must not be inferred from the public projection.

### exclusions

Historical acceptance inputs are empty and predecessor expectations are not comparator authority.
Immutable history is classified separately from live candidate zero state. No skipped or unparsed
candidate is recorded. The unrelated user-owned `Untitled` file is outside the candidate tree and
was not inspected or modified. WP49 purge, WP50 performance/resource measurement, non-Linux hosts,
network/cross-user profiles, and final release certification are explicitly outside this verdict.

### fault_discrimination

All sixteen r3 negative fixtures produce the expected mismatch paths and
`RFV5_OBSERVATION_DRIFT`. Normal and fault observations share the same source seam, while fault
behavior is applied before observation rather than by editing captured normal output. The eleven
required layer-fault runs select at least one test each and exit zero; the independent negative
fixture gate selects all sixteen. This establishes that the evidence can fail for the named
defects rather than merely authenticate files or count executions.

### limitations

The accepted limitations are material and correctly bound the verdict: one Linux local-workstation
environment is observed; WP49 owns physical predecessor purge; and WP50 owns representative
resource/performance evidence. The transaction is development evidence, not final release
certification. Recorded stdout/stderr digests authenticate bounded run captures but do not preserve
their full text; the review relies on reproducible closed commands and decoded semantic
observations, consistent with repository evidence policy.

## Architecture and Doctrine Assessment

The evidence advances P2/P27 because typed expectations and declared source seams causally govern
comparison and fault rejection; P3/P5 because presentation observations remain projections of
daemon-owned authority; P18 because hashes establish identity only; P19 because clean
reconstruction re-executes; P20/P25/P36 because capability claims have named executable oracles;
P30 because expected values remain independently authored; and P31 because drift is recomputed
rather than locally restamped. No history comparator, fake positive topology, or self-authored
golden decides the verdict.

## Library Leverage Assessment

The evidence exercises the accepted native boundary rather than substituting a custom protocol:
FastMCP 4 owns modern MCP framing/guard/completion presentation; `grpc.aio` and generated Tonic own
the private UDS client/server boundary; the Rust daemon owns Arrow/DataFusion/Delta semantics and
durable authority. The permitted empty framework-owned UI advertisement remains distinct from an
empty CodeFabric application-extension registry and component catalog.

## Legacy and Decommission Assessment

WP48 proves that predecessor expectations and runtime fallbacks do not authorize this candidate,
and it discriminates restored predecessor surfaces. It does not claim physical DB15--DB17 closure:
that belongs to WP49 after this evidence passes. The limitation is therefore accurate rather than
an incomplete WP48 acceptance claim.

## Test and Operational Assessment

Evidence includes positive, negative, failure, restart, reconnect, two-agent, redaction, stdout,
clean-root, source-mutation, and descriptor-reproduction coverage. Run selection is closed and
nonzero. Temporary observation and reconstruction roots are private, symlink-confined, removed on
success, and not accepted after a failed command. The transaction records environment, package,
tool, process, resource, command, exit, selection, and bounded-output identities needed to
reconstruct this result.

## Plan Deviations and Diff Hygiene

No acceptance-affecting deviation was found. The candidate transaction is untracked because the
independent review has not yet closed it; this review leaves those bytes unchanged. The only other
dirty path observed was user-owned `Untitled`, which remained untouched. The two requested review
artifacts are collision-free additions.

## Required Remediation Order

None for WP48 evidence acceptance. Subsequent execution remains dependency ordered: finalize the
review transaction without changing its first five entries, then proceed to WP49 and WP50 under
their own acceptance gates.

## Focused Re-Review Scope

No re-review is required for this candidate tip. Any change to the candidate commit, r3 release,
runner, observer, recipe closure, or first five transaction entries invalidates this approval and
requires a new append-only evidence attempt and independent review.
