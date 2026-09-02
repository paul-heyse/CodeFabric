---
artifact: implementation-review
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
verdict: changes-required
version: v1
date: 2026-09-02
status: complete
---

# Implementation Review: CodeFabric relational data fabric v5 WP48

## Provenance and Review Scope

This independent review assesses the unreviewed five-entry transaction at
`contracts/evidence/relational-fabric-v5/wp48-production-evidence-v1.jsonl`, the WP43 expectation
release, the WP48 capture/validation harness, its bound recipes and tests, the preserved unfavorable
attempt, and the production tests needed to judge what each recorded run actually proves. The
reviewer identity is `codex-wp48-independent-production-evidence-reviewer`; it is distinct from
`wp48-production-evidence-executor`, the WP43 expectation author, the implementation owner, and the
WP43 expectation reviewer.

The reviewed transaction contains exactly five append-only entries and ends at
`70e0d2a0dab50a3f1254eb2d6b6282a2bdc9ee49a28308fa34debdbf32c0bd63`. It binds candidate
commit `01258b9e4220abebf91c63210deefff1ae8ddde7` and tree
`fb8adc1fe622217e559858b0c255788f341e285c`. The read-only `review-candidate` validator passed with
16 selected claims and reproduced that exact tip. The transaction has not been finalized or
appended by this review.

The earlier unfavorable attempt remains at
`contracts/evidence/relational-fabric-v5/wp48-production-evidence-failed-20260902T110948704172Z.jsonl`.
It records `accepted_as_success: false` and the failed `clean-real-topology` observation at candidate
`70db221d3949a4cf73a70a4ebda2928030475e0e`. The successor candidate added serialized execution
for the shared process tests and reran the complete closed command set. The failed attempt was not
deleted, rewritten, or reclassified.

## Executive Summary

The transaction is strong on integrity, independent expectation provenance, append-only failure
retention, bounded command metadata, clean reconstruction, and exclusion of predecessor authority.
Its WP43 expectations were committed before WP44--WP47 production implementation, and the current
validator binds every input, runner, test, recipe dependency, candidate commit/tree, tool/package
identity, and chain entry.

It is not yet acceptable evidence of the claimed behavior. The claim entries store a digest of each
independent expected observation beside broad passing run IDs, but the capture never records a
complete decoded actual observation and never invokes the independent structural comparator on
that actual value. Several mappings therefore cover only a subset of their frozen clauses. The
clearest examples are RFV5-FM4-002 through RFV5-FM4-004: the capture omits both
`fastmcp4-modern-protocol-check` and `fastmcp4-public-surface-check`; the real installed
`modern-contract`/`security-boundaries` tests do not assert the exact legacy error/public code and
zero business dispatch, nor the full component/schema observation.

The same root defect affects negative evidence. `distinguished: true` and each negative fixture's
production-run association are constructed by the recorder and revalidated against the recorder's
own constants. The 16 fixture tests prove that a JSON merge-patched *expected value* is rejected by
the generic comparator, while the production runs prove selected normal or hostile-input behavior.
The harness does not execute the corresponding controlled production mutation, decode its actual
observation, and demonstrate the independently declared mismatch paths/error. A regression in an
unasserted field can therefore leave every recorded run green while the transaction still claims
the unchanged expected-observation digest and causal discrimination.

## Verdict

**Changes required.** IR-001 is a blocker to independent acceptance and the sixth transaction
entry. No `wp48-independent-review-v1.json` has been created, and the five-entry transaction has not
been appended.

## Gate and Evidence Assessment

| Evidence | Fresh result | Assessment |
|---|---:|---|
| `fastmcp4_production_evidence.py review-candidate` | pass; 16 claims; exact tip reproduced | Proves chain, inputs, runner/recipe closure, environment, identities, run metadata, and schema. It does not prove the semantic completeness of claim-to-run mappings. |
| `tooling/ci/test_fastmcp4_production_evidence.py` before review | 25 pass; the one complete-transaction test fails because the required review entry is intentionally absent | Falsifies structural drift and append/failure mechanics. Its behavior test mutates stored digests, not actual decoded production observations. |
| candidate commit/tree and input bindings | pass | Exact candidate and immutable WP43 inputs are bound. |
| preserved failed attempt | pass | Unfavorable clean-topology result remains explicit and non-successful. |
| final clean reconstruction record | pass | Fresh Cargo target/cache root, installed wheel, fresh runtime/activation, serial real-process tests, source mutation, restart, descriptor reproduction, and ephemeral-root removal are recorded. |
| claim observation map | incomplete | Expected digests and passing run IDs are associated, but no complete actual observation is captured or compared. |
| causal fault/negative fixture map | incomplete | Hostile-input tests and independent fixture comparison exist, but declared production-fault discrimination is not executed and observed end to end. |

## Required Scope Assessment

## causality

The substrate portion is materially causal: provider-row content-pin drift, changed transformation
inputs, unresolved DataFusion authority, unknown Delta reader features, substituted activation
versions, guarded-input rejection, resource reissue denial, completion non-disclosure,
cancellation, and MCP denial all execute substantive code paths. The fresh vertical also proves a
real source reaches provider/activation/query/presentation behavior.

The transaction nevertheless does not establish causality for each frozen claim. A claim row is
produced by hashing WP43's expected value and assigning static `run_ids`; the corresponding run
does not return a canonical observation consumed by `validate_observation`. Thus the expected hash
can remain stable while a field outside the selected test assertions changes. Causality is partial,
not complete.

## fixture_independence

This scope is satisfied. WP43 commit `13c4bbd0314033b51168c9fa4bd00fbec0593fa7` introduced the
16 expectations, causal fixtures, negative fixtures, performance method, and independent review
before the WP44--WP47 implementation commits. It is ancestral to the reviewed candidate. The
release validator imports no adapter, daemon, generated wire, predecessor evidence, or production
observation module; accepted design/plan inputs are hash-bound, target execution is declared false,
and author/reviewer identities are distinct. The current transaction binds those exact bytes.

## real_process_coverage

The process topology itself is genuine. The recorded WP47 cases build and install a wheel into an
isolated virtual environment, copy the production Rust executables, start the real supervisor and
`codefabricd`, launch attach-only per-agent FastMCP processes, communicate over STDIO and private
UDS gRPC, and exercise fresh activation, guarded query/resource/completion, denial, cancellation,
restart, reconnect, and two-agent isolation. Clean reconstruction repeats those cases from one
fresh Cargo/runtime root with serialized execution.

Coverage of the frozen observations is not complete. In particular:

- RFV5-FM4-002 expects the exact legacy JSON-RPC error, stable public code, and zero business
  dispatch. The bound installed-process test only requires that *some* error exists and checks
  secret absence; it does not assert those exact fields or dispatch count.
- RFV5-FM4-003 expects the complete provider/transform/session/UI/application-extension/task
  census. `modern-contract` checks the public tool/resource/prompt/extension subset, but not the
  whole frozen observation.
- RFV5-FM4-004 expects complete strict/frozen/extra-forbid schema policy, required/optional fields,
  types, aliases, and hidden dependency exclusion. The installed test checks only a subset of query
  properties, while the broader component recipe is not captured.
- The exact WP46 oracles `fastmcp4-modern-protocol-check` and
  `fastmcp4-public-surface-check` are absent from `BEHAVIOR_RUN_SPECS` and from the transaction.

Real processes are therefore present, but their recorded assertions cannot support the complete
claim hashes currently attached to them.

## exclusions

This scope is satisfied within the declared WP48 boundary. The opening entry has
`historical_acceptance_inputs: []`; runner and transitive recipe validation reject the forbidden
WP38, FastMCP 3, predecessor-comparator, and source-tree-adapter edges. The clean run uses an
installed wheel, fresh Cargo/runtime roots, no cached epoch, no predecessor model, no static adapter
schema, no stale descriptor, and no source-tree adapter import. WP49's physical repository purge is
correctly not claimed here.

## fault_discrimination

The 11 layer tests are useful hostile-input and state-substitution oracles, and the 16 independent
negative fixtures are structurally discriminating against their frozen expected objects. What is
missing is the bridge between them. `_fault_payload` sets every layer and fixture mapping to
`distinguished: true` after its mapped commands exit zero; `_validate_fault_map` checks that static
shape. It does not apply a fixture's fault to the production probe, capture the changed actual
observation, and require `RFV5_OBSERVATION_DRIFT` at the declared mismatch paths.

For example, a legacy request could regress from the expected `-32600`/
`unsupported_protocol_era` rejection to another error after unintended business dispatch, yet the
current installed negative assertion could still see an error and pass. The transaction would
continue to record RFV5-FM4-002's unchanged expected digest and `distinguished: true`. That is the
exact false-positive state the independent evidence packet is supposed to rule out.

## limitations

The three recorded limitations are accurate and appropriately bounded: WP48 observes one Linux
local-workstation environment; WP49 owns physical predecessor purge; and WP50 owns representative
resource/performance evidence. `skipped_candidates` and `unparsed_candidates` are both empty, and
the verdict scope is explicitly development evidence rather than final release.

Those limitations cannot absorb IR-001. The missing actual-observation and controlled-fault
comparison is inside WP48's declared development-evidence outcome, not an environment, purge, or
performance limitation.

## Finding Index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | blocker | tests | Claim and fault maps attest static associations instead of comparing complete decoded production observations and controlled faults to the independent WP43 values. |

## Findings

### IR-001 — Claim and fault mappings are not executable semantic comparisons

**Severity:** blocker
**Dimension:** tests
**Design/Plan refs:** WP48 required changes 2, 4, and 6; `PC-WP48-BEH`;
`PC-WP48-NEG`; FastMCP 4 design §16; SUITE §§5.1--5.2; P25, P27, P30
**Evidence:** `claim_observation_map` stores `expected_observation_sha256` plus static `run_ids`.
`_claim_payload` never accepts actual observations and `_validate_claim_map` only recomputes the
expected digest and static mapping. `validate_observation` is used by WP43 fixture tests, not by
WP48 capture against production output. `causal_fault_map` similarly records
`distinguished: true`; `_validate_fault_map` validates the declaration and command exits without
executing the corresponding production mutation and comparing its decoded observation. The
runner omits `fastmcp4-modern-protocol-check` and `fastmcp4-public-surface-check`, and the mapped
WP47 assertions cover only subsets of RFV5-FM4-002--004.
**Failure mode:** a production regression in an unasserted expected field can leave all mapped
commands green while the transaction still carries the independent expected digest and claims the
fault was distinguished. The sixth entry would then convert an integrity-valid but semantically
incomplete record into accepted evidence.
**Remediation:** issue a new append-only candidate attempt whose observation record contains, for
every claim, a bounded canonical decoded actual observation (or a closed set of canonical slices)
and an executed generic comparison to the exact WP43 expected object. Cover every frozen field with
the real installed topology where the claim is process-facing; include the exact modern-protocol
and public-surface oracles or an equivalent stronger installed-process probe. For every negative
fixture and each claimed layer fault, execute the controlled intervention against the same
observation path and record the independently declared stable error and exact mismatch paths. Do
not infer `distinguished` from a normal test's zero exit. Preserve both the unfavorable attempt and
the current five-entry attempt as non-authorizing history; do not rewrite or restamp either.
**Focused re-test:** add causal tests which fail when any decoded expected field is omitted and when
any mapped production fault is replaced by a normal run, then run:

```bash
PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest -q \
  tooling/ci/test_fastmcp4_production_evidence.py -k 'beh_exact_decoded_observation_binding or neg_each_production_fault_reports_expected_mismatch_paths'
PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python \
  tooling/ci/fastmcp4_production_evidence.py review-candidate
```

The new candidate is ready for independent review only when both commands pass and direct review
confirms the recorded actual observations and fault deltas cover every frozen clause.

## Outcome and Invariant Matrix

| WP48 obligation | Executable oracle | Assessment |
|---|---|---|
| Exact append-only five-entry chain and candidate/input/runner binding | `fastmcp4_production_evidence.py review-candidate` | satisfied |
| Independently authored WP43 expectations precede production implementation | WP43 release validators plus Git ancestry | satisfied |
| Every positive clause is decoded and compared to its independent expected value | no current executable oracle; IR-001 | blocker |
| Real installed supervisor/daemon/launcher/wheel/modern-host topology | WP47 installed vertical cases and `clean-real-topology` | satisfied for exercised paths; incomplete claim coverage under IR-001 |
| One causally discriminating production fault per claimed layer | current fault tests plus static map; no actual-observation mutation comparison | blocker: IR-001 |
| Independent negative fixtures affect production observations at declared paths | WP43 merge-patch comparator only | blocker: IR-001 |
| Clean target-only reconstruction, mutation, and restart | `clean-real-topology`, `clean-source-mutation`, `clean-descriptor` | satisfied |
| Failed attempts remain unfavorable append-only evidence | preserved failed transaction with `accepted_as_success: false` | satisfied |
| No skipped/unparsed verdict candidate | `review-candidate`; limitations entry | satisfied |
| Environment/purge/performance boundaries are explicit | limitations entry | satisfied |

## Architecture and Doctrine Assessment

The overall evidence architecture has the correct separation: WP43 owns expectations, WP48 owns
execution, and a third identity owns review. Candidate identity and digests are used as integrity,
not explicitly described as semantic proof. The defect is the missing executable boundary between
independent meaning and production observation. Static claim/fault association violates P25/P27/P30
and SUITE §§5.1--5.2 because a producer-authored mapping is being asked to stand in for a decoded,
independently discriminating result.

The required repair does not reopen the target runtime architecture or library choices. It makes
the evidence pipeline actually perform the comparison its types currently describe.

## Library Leverage Assessment

No load-bearing library choice is invalidated. The real topology correctly uses the installed
FastMCP 4 wheel, generated Protobuf/grpcio client, Tonic UDS daemon, Arrow/DataFusion query path,
and exact Delta reconstruction. The missing capability is application-owned evidence composition:
canonical observation DTOs and the existing generic WP43 comparator must be connected. No new
third-party dependency is required.

## Legacy and Decommission Assessment

The WP48 candidate does not use historical acceptance results or a predecessor comparator as
verdict authority. Forbidden live recipe fragments are rejected, the clean run reports no
predecessor model/static adapter schema/stale descriptor/cached epoch, and the earlier failed
attempt remains immutable. Physical live-surface purge remains correctly assigned to WP49.

## Test and Operational Assessment

Run execution is bounded, output lengths/digests are recorded, command failure is fail-closed, and
the clean suite now serializes shared real-process cases. The runner also enforces nonzero selectors
and hashes transitive recipe definitions. Those are good operational controls. They cannot detect
semantic assertion omissions, which is why actual observation capture and faulted comparison must
become first-class evidence rather than a reviewer inference over broad test names.

## Plan Deviations and Diff Hygiene

The preserved failed attempt and new candidate are a beneficial application of WP48's rollback
policy. The substantive adverse deviation is that required changes 2 and 4 were implemented as
claim-to-run and fixture-to-run declarations rather than complete decoded comparisons. No
acceptance JSON or sixth transaction entry was written by this review.

## Required Remediation Order

1. Define the bounded canonical actual-observation record for all 16 claims and map every frozen
   field to an observing production probe.
2. Close RFV5-FM4-002--004 immediately with the exact modern protocol and full public
   surface/schema/component evidence; audit the remaining 13 mappings to the same standard.
3. Execute each controlled production fault through the same observation path and record the exact
   independent mismatch/error, rather than recorder-authored `distinguished` flags.
4. Add mutation-sensitive harness tests, commit the revised candidate, and capture a wholly new
   append-only attempt from that commit.
5. Request a fresh independent review of the new five-entry tip. Only that reviewer may author an
   acceptance document and only the execution owner may append it.

## Focused Re-Review Scope

Re-review IR-001 only after a new candidate tip exists. Verify every claim's actual decoded
observation against the frozen WP43 value, every production fault's exact mismatch paths/error,
real-process coverage for all process-facing fields, preservation of both earlier attempts, and
unchanged limitation boundaries. A green schema validator without those semantic records is not
sufficient.
