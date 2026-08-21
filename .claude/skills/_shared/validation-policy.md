# Proportional Validation Policy

Validation is continuous, layered, and proportionate. It is not deferred until
the entire plan is implemented, and it is not an excuse to run the full
repository suite after every edit.

## 1. Validation levels

### Edit-local

Run after a coherent micro-change when inexpensive and relevant:

- parser or syntax validation;
- formatter on changed files;
- changed-file lint;
- language-server or narrow type diagnostics;
- one directly affected unit test;
- an import, compile, serialization, or API smoke probe.

Goal: catch cheap mistakes before they propagate.

### Work-packet

Run before declaring a dependency-closed packet complete:

- all tests that prove the packet's contracts and immediate consumers;
- package/crate/module build or check;
- focused type/lint checks;
- structural governance rules;
- negative cutover/decommission checks;
- migration, serialization, or cross-language boundary tests where relevant.

Goal: prove the packet leaves its subsystem coherent.

### Milestone

Run where multiple packets first interact:

- cross-packet integration tests;
- representative end-to-end flows;
- differential/reference tests;
- concurrency, recovery, migration, or performance checks;
- broader affected-package builds.

Goal: detect interaction defects before the final ceremony.

### Final

The gate matrix is a list of `just` recipes — the justfile is the gate
registry per the AGENTS.md command contract, and the plan's declared
obligations select from it. It normally includes:

- repository or policy-defined formatting/linting;
- relevant static/type checks;
- Python tests;
- Rust format, check/clippy, and test/nextest as configured;
- native-extension or cross-language builds;
- governance, architecture, and decommission rules;
- representative end-to-end tests;
- required benchmarks, migration checks, or security scans.

Goal: certify the complete result.

## 2. Failure behavior

A failing check is feedback, not a stopping condition.

1. Record the command and failure.
2. Diagnose whether the failure is introduced, pre-existing, flaky, or caused
   by an intentionally incomplete boundary.
3. Correct the implementation or packet decomposition.
4. Re-run the smallest check that proves the correction.
5. Continue when the required boundary is green.

Do not terminate merely because a validation command failed. Do not suppress or
weaken a gate to make progress unless the design or plan explicitly authorizes
the change and the deviation is recorded.

## 3. Baseline failures

Never assume the repository begins clean.

At execution preflight:

- run or retrieve the relevant baseline checks;
- record pre-existing failures with command, location, and fingerprint;
- require no unexplained regression;
- distinguish repaired baseline failures from plan-caused changes.

A broad final gate may remain non-zero only when every residual failure is
demonstrably baseline, the project policy permits it, and the completion report
states it plainly.

## 4. Gate selection

Discover gates from `just --list` first, and prefer a recipe over
reconstructed tool flags — recipes express intent, so implementations can
change without invalidating what callers know. Re-derive commands from raw
manifests or CI configuration only when auditing the justfile itself, or
when a needed gate has no recipe — in which case propose adding one rather
than embedding flags in an artifact.

Do not hard-code Python-only gates for a mixed-language plan.

## 5. Subagents

Delegated implementers own edit-local and packet-local proof. The lead executor
owns merge, cross-packet milestone, and final proof.

The lead must not accept a subagent's statement that checks passed: a
subagent's proof is the recipe or command name plus its exit status and the
affected scope. Re-run integration-sensitive checks after merging parallel
work.

## 6. Evidence record

Record each required gate as its recipe or command name plus exit status —
`just root-test — exit 0` — nothing more. Check outcomes are derivable by
re-running the named check (`evidence-policy.md` §0), so they are never
hand-written into state; the execution-state schema has no field to hold
them. Prefer machine capture where it already exists (the nextest `ci`
profile emits JUnit under `target/`). Keep a failure log only while it is
needed for diagnosis.
