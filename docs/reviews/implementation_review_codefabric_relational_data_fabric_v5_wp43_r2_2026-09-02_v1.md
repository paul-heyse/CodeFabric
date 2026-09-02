---
artifact: implementation-review
date: 2026-09-02
version: v1
status: complete
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: approved
---

# Implementation Review: CodeFabric relational data fabric v5 WP43 r2

## Provenance and Review Scope

This independent review assesses the forward-corrected expectation release at
`contracts/acceptance/relational-fabric-v5-r2` against WP43 and WP48 of the v5 plan, the accepted
FastMCP 4 presentation-boundary reviews v1 and v2, QRY v2.3 §§13.1–13.5, the relevant SRV v2.3
contracts, the FastMCP 4 reference, and data-fabric principles P18, P25–P31, and P36.

The reviewed author candidate is exactly
`25e10b66453e4d665ffa05e36ec95247691f846f`. The later ancestral descendant `d57c62d` changes WP49
assurance only and was excluded from expectation-authoring evidence. This review did not import
production modules, inspect WP48 observer output, use target execution as expected-value authority,
or reuse the r1 acceptance decision.

The review covered all 16 expectations, all 16 causal fixtures, all 16 negative fixtures, issuance
and correction provenance, r1 byte preservation, the inherited performance method, JSON-pointer
fault paths, duplicate-key/path/hash controls, and the release-specific validator/tests.

## Executive Summary

The r2 release is approved. It corrects the synthetic r1 observations without replacing them with
implementation-shaped literals:

- claim 003 uses a valid reverse-DNS custom-extension fault and the correct escaped JSON pointer;
- claim 004 records decoded outer schema shape separately from Rust-daemon semantic authority;
- claim 005 records stable ownership, safe presentation, selected-choice equality, and exactly-once
  admission without freezing a semantic field ID or answer;
- claim 009 records daemon ownership plus accepted/resumed identity equality without freezing a
  daemon query ID;
- claim 015 points at the r2 release while inheriting the r1 performance method byte-for-byte; and
- claim 016 binds the corrected six-artifact and five-source release sets.

No blocker, major, minor, or observational finding remains. The acceptance transaction records a
distinct reviewer and one disposition per claim, and the strengthened validator rejects duplicate
YAML keys, duplicate or malformed JSON pointers, selector rebinding, unexpected hash entries,
duplicate source paths, incomplete source coverage, non-distinct review, incomplete dispositions,
and byte drift.

## Verdict

**Approved.** The r2 expectation release is independently authored, candidate-bound,
causally discriminating, candidate-neutral where performance is concerned, and suitable as the
expectation authority for a new WP48 evidence attempt.

Frozen accepted identities are:

| Artifact | SHA-256 |
|---|---|
| `causal-fixtures.yaml` | `77842bb7a33c6608e582337c84c88c484cda84ce0bcbdce9f172f6f4e78f6714` |
| `expectations.yaml` | `c1c1bb8a394ab02e53a2c91a205630e54dd3357ab1039f9f605fdc1ff65de751` |
| `independent-review.yaml` | `1d42ccdc723f13c9fc17bbb5551ba612a66a33175b93158cb0a5a0982a81e70d` |
| `issuance.yaml` | `8c27a22dcf06365cde7c159b73d41bb8de64760b2b5b74d66700c46b80054c3c` |
| `negative-fixtures.yaml` | `471e0e99573ffd59b814d2ffc9165c8755bb46c7389b2d98451a866fd209d60c` |
| `performance-method.yaml` | `ceb48efae08732a452bbbafa9642f1130eb81cffefcd4e7b2869925d2be5c6df` |

## Gate and Evidence Assessment

| Check | Result | What it establishes |
|---|---:|---|
| focused successor-expectation pytest | 51 passed | release parsing, both release lineages, all fixture exactness, acceptance independence, duplicate/path/hash/selector/source falsification |
| `just fastmcp4-successor-authority-integrity-check` | exit 0; 8 selected | synchronized v2.3 suite identity, predecessor linkage, and required authority clauses |
| `just fastmcp4-independent-expectation-review-check` | exit 0; 16 selected | distinct candidate-bound acceptance and all causal fixtures |
| `just fastmcp4-negative-fixture-independence-check` | exit 0; 16 selected | exact RFC 7396 fault patches, exact escaped mismatch paths, and typed failure |
| `just fastmcp4-expectation-drift-check` | exit 0; 11 selected | exact six-file set, exact five-source set, frozen bytes, and performance-method identity |
| focused Ruff format/check | exit 0 | validator and falsification-test hygiene |
| `cmp` and r1 SHA-256 checks | exit 0 | r1 remains byte-for-byte frozen and r2 performance method equals r1 exactly |

The independent-review recipe was also run before acceptance and failed with
`RFV5_REVIEW_PENDING`, confirming the handoff could not masquerade as acceptance. It passed only
after this distinct review record and its frozen hashes were installed.

## Finding Index

No findings.

## Findings

No blocker, major, minor, or observation was identified.

## Outcome and Invariant Matrix

| Claim | Independent disposition | Governing outcome | Executable oracle |
|---|---|---|---|
| RFV5-FM4-001 | accepted | sole v2.3 authority, exact stack, modern era, bridge off | `fastmcp4-successor-authority-integrity-check` |
| RFV5-FM4-002 | accepted | modern admission and legacy pre-dispatch rejection | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-003 | accepted | four tools, two resource families, one completion, bounded framework UI advertisement, no application extension | `fastmcp4-independent-expectation-review-check` |
| RFV5-FM4-004 | accepted | strict outer presentation schemas and separate daemon semantic authority | `fastmcp4-independent-expectation-review-check` |
| RFV5-FM4-005 | accepted | daemon-authored bounded guard, re-entry, reauthorization, exactly-once acceptance | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-006 | accepted | one closed atomic start path and pure explicit validation | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-007 | accepted | live capped authorized advisory completion and read-time reauthorization | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-008 | accepted | daemon-minted bounded resources and per-operation authorization | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-009 | accepted | explicit cancellation and identity-preserving reconnect without start replay | `fastmcp4-independent-expectation-review-check` |
| RFV5-FM4-010 | accepted | two isolated presentation cells over one workspace daemon | `fastmcp4-independent-expectation-review-check` |
| RFV5-FM4-011 | accepted | typed fail-closed grant/session/challenge/resource/generation matrix | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-012 | accepted | allowlisted errors, secret redaction, and protocol-only STDOUT | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-013 | accepted | no Python semantic, session, task, cache, lease, or data-plane authority | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-014 | accepted | predecessor presentation authority is live-zero while history stays immutable | `fastmcp4-negative-fixture-independence-check` |
| RFV5-FM4-015 | accepted | preregistered candidate-neutral control, samples, distributions, and budgets | `fastmcp4-expectation-drift-check` |
| RFV5-FM4-016 | accepted | exact release/source sets and fail-closed drift | `fastmcp4-expectation-drift-check` |

## Architecture and Doctrine Assessment

The release advances P30 by keeping expected values independent of target execution and by
replacing invented identities with ownership/equality relations. It maintains P3 and P13 by leaving
semantic, query, challenge, resource, completion, cancellation, and authorization authority in the
Rust daemon. It maintains P18 by using hashes only to bind the exact reviewed Class-1 release bytes,
never as evidence of correctness. It advances P25, P27, and P36 through a causal fixture and a
committed negative fault for every claim. Exact release/source membership and recomputation maintain
P26, P28, and P31 without permitting a silent restamp.

The r2 correction is a legitimate forward release rather than an in-place rewrite of r1. That is
the clean-sheet outcome: preserve immutable evidence history, correct the target authority forward,
and never force production code to reproduce a synthetic legacy literal.

## Library Leverage Assessment

The corrected extension fault follows FastMCP 4 §39's reverse-DNS identifier contract. Guard claims
follow §38's re-entrant `InputRequiredResult` and sealed request-state model while retaining the
daemon token as semantic authority. Completion claims follow §41's resource-template-only,
100-candidate, separately authorized surface. The expectation release does not adopt FastMCP
sessions, tasks, auth, cache, providers, transforms, proxying, or application extensions.

## Legacy and Decommission Assessment

All six r1 release files retain their frozen hashes. The r2 performance method is byte-identical to
r1 and explicitly names r1 as its source release, so preregistration remains before candidate
results. R1 acceptance is not reused for r2. R2 changes no historical bytes and creates no live
predecessor fallback; WP49 retains responsibility for the physical post-evidence purge.

## Test and Operational Assessment

All 16 negative patches are exact RFC 7396 mutations whose declared JSON-pointer sets equal the
actual structural differences. All 16 causal fixtures change both controlled input and expected
observation. No expected observation freezes a daemon-generated query, challenge, field, package,
or resource identity. The fixed public resource handle in claim 008 is explicitly controlled test
input; the expectation asserts its owner and authorization relations rather than its literal value.

The performance method remains candidate-neutral: three warmups, thirty samples, interleaved
candidate/control execution on one host, raw samples, distribution summaries, bootstrap confidence
intervals, ten separately measured workloads, and immutable preimplementation budgets. No local
relaxation is permitted.

## Plan Deviations and Diff Hygiene

The r2 release is a beneficial execution-time correction authorized by WP48's forward-repair rule.
Target execution disclosed that r1 contained synthetic observations, but neither execution output
nor production code supplied the corrected expected values. The changed values derive from the
accepted design, QRY/SRV contracts, and relational properties. Concurrent WP48 observer files,
WP49 assurance work, and `Untitled` were not read as expected-value authority or modified by this
review.

## Required Remediation Order

None.

## Focused Re-Review Scope

Any change to the six accepted r2 files, the five immutable design inputs, selector bindings, or
performance method invalidates the frozen hashes and requires a new forward-versioned expectation
release plus distinct review. WP48 may now bind a new evidence transaction to this accepted r2
release and compare actual observations through the generic observation validator.
