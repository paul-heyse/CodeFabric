---
artifact: plan-audit
plan_path: docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v2_2026-08-26.md
verdict: needs-revision
version: v1
date: 2026-08-26
status: complete
---

# Plan Audit: CodeFabric design-principles full-alignment review remediation v2

## Provenance and Scope

This audit re-evaluates the new v2 remediation plan created after the accountable repository
owner rejected `codefabric-golden-v3.0.0-candidate.1`. It audits the complete plan against the
accepted behavioral-conformance design, current source, v1 execution state, SUITE AC-G-78 and
AC-G-79, QRY response/coverage/conformance requirements, GEN Python/Rust fact semantics, FAB
testing doctrine, LIFE consistency/failure invariants, SRV public delivery, and the pinned
library references for canonical JSON, DataFusion/Arrow, gRPC/Protobuf, and FastMCP.

The current real-provider vertical, clean rebuild, candidate/release code, corpus fixtures,
justfile, and plan-assurance tooling were inspected. Two fresh independent lenses challenged the
design and implementation impact. Both agreed that the selected authored-claim plus independent-
evaluator plus black-box-observation architecture is preferable to snapshot capture, descriptor/
hash comparison, upstream-provider differential alone, or a second full compiler.

## Executive Summary

The target design is sound and directly addresses the rejected golden's root defect. It makes
semantic meaning independent, preserves the useful real-provider/integrity vertical, requires
causal faults at producing seams, and supplies a reviewable source-to-claim-to-observation
dossier. The pinned libraries are used at their appropriate boundaries and are not promoted into
expectation authority.

The v2 plan is not executable as written. Its declared-input table was copied from v1 and is
already stale; its baseline does not identify the actual replanning point; and its decommission/
milestone dependencies contain a closeout cycle. The strict-JSON requirement also needs to name
duplicate-key detection rather than relying on ordinary serde DTO parsing. A small closeout
coverage omission must be corrected at the same time.

## Readiness Verdict

**needs-revision**. The architecture does not need redesign. F-001 and F-002 are major plan
integrity/executability defects; F-003 is a major library-boundary ambiguity that could admit a
non-strict expectation contract. F-004 is a minor closure omission. Create a v3 plan rather than
editing this audited v2 artifact.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | major | factuality | frontmatter, §2 | open |
| F-002 | major | sequence | WP08, M04–M05, DB02/DB04/DB05, §8 | open |
| F-003 | major | library | design LD-01, WP09 | open |
| F-004 | minor | impact | WP08, DB05 | open |

## Findings

### F-001 — Planning provenance and declared inputs are stale

**Severity:** major
**Category:** factuality
**Scope:** frontmatter and §2
**Finding:** Frontmatter still declares baseline
`412af14566393c2379ba4e174387361cea5370e8`, while §1.3 says v2 was planned at
`a3efb30d699f84a0d6f190a5ff3c2574bfcf039e`. Executable plan validation reports stale declared
inputs for SUITE, GEN, FAB, LIFE, and `schema-contract-ir.json`. The load-bearing FastMCP,
grpcio, Protobuf, and serde_json references used by WP09/WP11 are also absent from the declared-
input table. Execution from these inputs would not have a trustworthy freshness boundary.
**Required resolution:** In v3, set the current planning baseline, record the dirty-tree identity,
recompute every existing declared input from current bytes, and add the four load-bearing library
references. Do not hand-restamp an executing plan later; this is a new version at planning time.
**Revalidation:** `env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp python -c "from pathlib import Path; from tooling.ci.artifact_contracts import validate_plan; validate_plan(Path('.'), Path('docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v3_2026-08-26.md'), verify_declared_inputs=True, _allow_missing_state=True)"`

### F-002 — Milestone and decommission dependencies are circular

**Severity:** major
**Category:** sequence
**Scope:** WP08, M04–M05, DB02, DB04, DB05, §8
**Finding:** WP08 declares DB01–DB05 as dependencies, DB04 requires WP08 and M05, DB02/DB05
require M05, and M05 requires WP08 plus DB04/DB05. M04 also says DB02 exits even though DB02
requires WP09–WP12, WP07, and M05. The recommended sequence tries to execute DB02/DB05 before
WP07 while their prerequisites require WP07. This cannot be represented as an acyclic execution
transaction and would make completion depend on itself.
**Required resolution:** Treat decommission batches as exit conditions, not WP08 prerequisites.
M04 closes only carried-forward execution/convergence. Run DB02/DB05 after WP12 on their
candidate-side targets and finish their release-side targets after WP07; run WP08; then close
DB04 and M05. Remove M05 from every DB prerequisite and make the normative and linearized graphs
agree.
**Revalidation:** `python3 -c "from pathlib import Path; s=Path('docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v3_2026-08-26.md').read_text(); bad=['**Dependencies.** WP04, WP05, WP07, WP09–WP12; DB01–DB05.','DB02 plus the rebuild half of DB03 exit','**Prerequisites.** WP09–WP12, WP07, M05.','**Prerequisites.** WP07, WP08, M05.']; assert not any(x in s for x in bad)"`

### F-003 — Strict claim ingress does not name duplicate-key detection

**Severity:** major
**Category:** library
**Scope:** design LD-01 and WP09
**Finding:** The plan requires duplicate-key rejection but describes only strict application-owned
serde DTOs. Ordinary `serde_json::from_slice` plus `deny_unknown_fields` does not reject duplicate
object members; the repository's canonicalization reference requires a Visitor-based duplicate-
detecting ingress. A duplicate could silently overwrite an authored predicate or proof-universe
field and change intended meaning while the contract gate remains green.
**Required resolution:** Declare the current serde_json reference as an input and require a
duplicate-detecting Visitor/strict loader before typed claim deserialization. Add an exact
duplicate-key negative oracle for top-level and nested claim, selector, universe, operation, and
query objects.
**Revalidation:** `just functional-golden-contract-check`

### F-004 — Final certification omits the new behavior invariants from its closure text

**Severity:** minor
**Category:** impact
**Scope:** WP08 and DB05
**Finding:** WP08 targets only GI-01–GI-13 although the functional correction is governed by
GI-15–GI-18, and its legacy disposition names DB04 without DB05. The packet's required changes
and acceptance checks mention DB05, so the omission is localized but makes final certification
text internally inconsistent.
**Required resolution:** Make WP08 target GI-01–GI-18 and state that DB04/DB05 jointly remove
false current authority and the self-referential golden path.
**Revalidation:** `rg -n 'GI-01–GI-18|DB04/DB05' docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v3_2026-08-26.md`

## Target-Design Assessment

The selected architecture passes the clean-sheet challenge. If the current Gate B implementation
did not exist, a small authored semantic corpus plus transparent reference laws and black-box
public execution would still be preferable. It is substantially easier to review than a captured
5.9 MB candidate, more independent than a production projection, and much smaller than a second
provider/query stack.

Authority and dependency direction are explicit: humans author intended source meaning; a test-
only evaluator derives limited mathematical consequences; production generates observations;
and the comparator reports semantic differences. Integrity, convergence, delivery, and semantic
meaning remain orthogonal. The design correctly rejects hashes and fingerprints as correctness
oracles while retaining them for identity/provenance.

## Library Capability Assessment

- DataFusion 55.0.0, Arrow/Parquet 59.2.0, delta-rs `43a0cf10`, and petgraph 0.8.3 remain the
  production system under test. The plan correctly prevents them from generating expected
  semantic answers.
- FastMCP 3.4.7 supports explicit `StdioTransport`, programmatic tool calls, structured content,
  fresh-process isolation, and transport-level tests. WP11 correctly requires the actual STDIO
  boundary rather than only `Client(mcp)` in memory.
- grpcio 1.83.0 and Protobuf 7.36.0 generated clients are appropriate public observation
  plumbing; schema/wire equivalence is not mistaken for domain semantics.
- Standard-library ordered collections are sufficient for the bounded independent evaluator.
  Existing proptest is appropriate for laws/counterfactual models. Insta is correctly rejected
  as semantic authority.
- F-003 is the only library-grounding defect: serde_json needs explicit duplicate detection at
  the authored-contract ingress.

## Work-Packet and Impact Assessment

WP09–WP12 form a sensible dependency-closed chain. WP09 owns authored meaning and rejected-
candidate isolation; WP10 owns independent laws/comparison; WP11 owns real execution and causal
seams; WP12 owns mutation closure, reviewability, and candidate production. WP07 remains the
proper external owner checkpoint, and WP08 remains independent certification.

Preflight queries cover the current self-reference, corpus layout, scenario runners, public
clients, provider helpers, release transaction, and assurance gates. Consumer impact includes
canonical tables, lifecycle/rebuild, UDS, artifact, FastMCP, CI, release, and later Waves 8–12.
The design deliberately keeps the Waves revision as a downstream plan activated only after the
functional predecessor M05, avoiding an activation cycle.

## Legacy, Transition, and Decommission Assessment

The legacy matrix is complete: released corpora, accepted records, and rejected candidate bytes
are preserved; self-authorizing comparison, candidate-local normalization/scenario authority,
and captured expectations leave the active path; the governed comparison registry and real
vertical remain. F-002 must correct sequencing before those dispositions are executable.

## Proof and Validation Assessment

The proof model is materially stronger than the rejected golden. It combines exact authored
claims, closed-universe remainder rejection, known-wrong evaluator inputs, producer-seam causal
faults, AC-G-79 rebuild convergence, real UDS/STDIO/artifact equivalence, a decoded dossier, and
separate integrity evidence. Required mutants cover relation direction, cardinality, certainty,
unknown/currentness/completeness, contexts, spans, ACLs, publication, stream, artifact, adapter,
canonical content, and incremental divergence.

The plan correctly refuses to let structural source scans prove independence by themselves;
behavioral falsification must also pass. General cargo-mutants remains out of scope, while the
bounded semantic-mutant registry is a required deterministic gate.

## Doctrine and Anti-Principle Assessment

The target advances executable models/governance, reproducibility, provenance, testability,
semantic observability, and contract-derived tests. It maintains one production semantic
authority, application-owned identity, provider isolation, Arrow/DataFusion/Delta ownership, and
FastMCP presentation-only boundaries. It avoids captured-output authority, silent unknown-to-none
collapse, duplicate semantic engines, and checksum theater.

## Top Required Changes

1. Refresh baseline and complete declared inputs in a v3 plan.
2. Remove the milestone/decommission cycle and align both execution diagrams.
3. Make duplicate-key rejection an explicit strict-ingress obligation and oracle.
4. Close GI-15–GI-18 and DB05 explicitly in WP08.

## Re-Audit Scope

Re-audit v3 only for artifact/input freshness, the corrected acyclic sequence, strict-ingress
library grounding, WP08 closure, and preservation of the selected behavior-first architecture.
If those corrections pass the named commands and no target semantics change, the plan is ready
for approval and activation.
