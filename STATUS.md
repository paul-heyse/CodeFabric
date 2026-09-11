# CodeFabric status

Updated 2026-09-11 from the canonical `/home/paul/CodeFabric` working tree on `master`.
The user has paused pursuit of full workspace test remediation and authorized completing the
remaining P04 scope, then P05, in the detailed outcomes plan. Sound design and feature delivery
take priority over arbitrary performance thresholds or a 100% suite result. P04 is active.

Remediation through `00f90066` preserves the completed fixture, admission and native lifetime fixes.
The isolated unfiltered run at clean `50d86d82` was stopped at the user's direction: **1,181 passed,
eight failed, one SIGINT interruption, 50 not started**, out of 1,240 selected; the same two intentional
ignores remain. Run `e4ec8cf5-8f74-42df-bd16-8e129ede3bd9` took 4,691.738 s. This is partial evidence,
not full qualification. Seven survivors exhausted semantic fixture setup/query clocks; one exposes
shutdown command authentication delayed behind owned cleanup. The complete Python context/negative
import corpus passed. Logs and context: `target/nextest-remediation/2026-09-11/full-3*`.

The latest 48 focused native/processing checks, including the complete 131-file retained-page case,
pass. All-target Clippy completes with existing warnings, featureless checking passes, and 232
tooling tests pass. R06 remains deferred; the supplement records original and subsequent evidence.
Relevant functional failures will be addressed in the product slices they affect. No broad suite
rerun is a prerequisite to resuming P04.

P01/P02 initial vertical exits and P03's first-release query boundary are delivered. P04 retained
continuous operation is partial; P05–P14 remain open. The detailed plan §10 and the package handoff
below identify the next unmet work. Package boundaries cross outcomes 4–8; an initial package exit
does not complete an outcome or the first useful release.

## Current handoff

**Outcomes 1–3 are implemented for the current Linux workflow. Outcomes 4 and 5 are partially
implemented. Outcome 6 has a partial production update loop; outcomes 7–8 remain open with selected implemented
prerequisites. No outcome from 4 through 8 is complete.**

The 2026-09-11 `cargo nextest run --workspace --no-fail-fast` baseline is **not green**:
1,164 passed, 68 failed, three timed out and two were explicitly ignored (1,235 run, 134.287 s;
build separately 2m39s). The clean reviewed revision is `a26c00a7`; run ID
`b1bc0a36-e41a-48a0-9ce9-ccbf5f46c796`. The default profile used installed native providers and a
delegated user-systemd scope, with no test filter, thread override or retries. This command covers
the stable root workspace, not separate provider/adapter suites or doctests.

The [workspace test remediation plan](docs/plans/codefabric_workspace_nextest_failure_remediation_plan_2026-09-11.md)
records all 71 unsuccessful identities, source/library evidence, exact reproduction context and
six ordered repair packages. Fifty failures stop at native watcher creation under exhausted
per-user inotify instance capacity; 13 stop at startup readiness deadlines. The remaining failures
are a stale provider census, three stale Binary-field fixtures and an obsolete seal-time execution
expectation. Three tests hit nextest's 120-second bound. Five sequential diagnostics produce four
passes and expose a stale direct-backend freshness argument behind one timeout; they do not replace
full-suite acceptance. The original passing readiness case also emitted a retained-Pyrefly shutdown
warning; the subsequent typed cancellation and ownership corrections are recorded above and in §0
of the supplement. These counts describe the original review baseline.

Local logs are in `target/nextest-review/2026-09-11-workspace/`; the durable findings and full test
inventory are in the new plan. Scheduling, typed fixture/census updates, actual-read resource
assertions, admitted-snapshot freshness and native lifetime fixes are implemented. Full-run qualification is deferred by the user; the supplement remains open.
The selected product work is P04 followed by P05. It supports the current outcomes plan and does not replace its
backlog or declare P04/P05 complete. Repair progress and commits are in the supplement’s §0.

Follow the [production backlog](docs/plans/codefabric_pragmatic_production_implementation_plan.md)
and its [detailed outcomes 4–8 execution plan](docs/plans/codefabric_pragmatic_production_outcomes_4_8_detailed_implementation_plan_2026-09-09.md).
The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md)
and [selected design](docs/spec_index/README.md) retain the full Python/Rust CPG, all eight forms,
composition, truthful incomplete scope and sustained operation. The detailed plan separates
implemented portions, unfinished acceptance and the next work for every slice, with library-grounded
design enhancements integrated into the same delivery progression.

Fresh daemon startup captures real Python/Rust inputs and publishes exact source/syntax Delta
versions before readiness; the owned coordinator publishes coherent semantic successors with
requested/completed/unknown scope. Canonical diagnostics, Python modules, Python/Rust declarations,
imports, semantic references, structural types and call occurrences are queryable within their
implemented meanings. P03 adds typed repeated blocks and prior results, branch isolation, scoped
family retrieval, bounded call walks, exact source/outline/related contexts and the first-release
plan corpus. Exact reopen preserves IDs, facts and source disclosure rules.

P04 now retains workspace-owned Tree-sitter/Ruff, contained Pyrefly and immutable Rust toolchain
inputs; observes pruned native/poll source topology and selected Git metadata; reuses exact source/
line and empty Arrow pins; bounds Delta writes; and detects selected provider executable replacement
through live work and restart. Broader dependency validity, Cargo target/fact reuse, shared native
CPU scheduling, full inclusion/topology, first-release edit coverage and sustained operation remain.
The current checkpoint is detailed below; older dated entries retain attributable history and do
not select an obsolete P02/P03 continuation.

The uv CLI pin is removed in `a644b295` at the user's request. Local tooling accepts the installed
uv; CI prefers an existing executable and uses an unpinned setup fallback only when absent. Python,
`uv_build` and application dependency selection remain separate. On 2026-09-10, system uv 0.12.13
passes `just tools-doctor`, `just tool-version-contract-check`, shell syntax and workflow parsing.
`just tooling-test tooling/product/test_harness.py` passes all 228 selected tooling tests in 3.36 s
(`/tmp/codefabric-system-uv-consumer.log`); the CLI report is `/tmp/codefabric-system-uv-report.log`.
Earlier historical uv reconciliation entries below no longer prescribe a CLI version.

## P04 retained inputs and syntax — partial, active

The resumed inclusion slice uses one capture/watch policy. Captured root `pyrefly.toml` and
`[tool.pyrefly]` search/site-package candidates can select subtrees beneath normally pruned
`.venv`/build directories, with ancestor observation, sibling pruning, no-follow source capture
and unconditional `.git` exclusion. Configuration changes invalidate watch topology; changed
policy during traversal requires reconciliation. Context discovery retains precedence/conflict
validation. Thirteen focused inventory/native/poll/recovery cases pass in 15.039 s
(`/tmp/codefabric-p04-inclusion-focused.log`). Installed `.venv` site-package queries through all four public forms and exact reopen pass
in 113.704 s (run `c363235c-e4e6-45b6-8bcd-e59204aacf18`,
`/tmp/codefabric-p04-inclusion-installed.log`; isolated target, installed native providers,
delegated user-systemd scope). Physical roots outside the registered workspace and complete Git classification
remain in P04; this slice does not imply their completion.

P03's related-context integration is committed in `75687368`. P04's immutable-input pin reuse and
workspace-owned syntax checkpoint is committed in `55c69cdd`. The retained Pyrefly service is
committed in `1f590cdc` and now passes the installed eight-state clean/update corpus. P04 remains open: retained Cargo contexts,
complete invalidation/topology and shared update scheduling still follow. P05–P14 remain open.

Exact source bytes and line indexes now declare identities over their producer revision, workspace,
generation, captured inventory and actual admitted image/line-index identities. Matching descriptors
and identities reuse selected exact Delta pins without executing a comparison scan or writing a new
table. Changed/ineligible relations retain candidate-owned physical histories, preserving isolation
from abandoned writes. Missing identities do not qualify; no generation or provenance is relabeled.
The optional identity lives in the existing canonical Delta descriptor and survives exact reopen.

Before the parser-cache integration, the installed source/semantic edit, obsolete-completion and
pending-stage restart scenario passes in 792.455 s, alongside three exact Delta tests (four total,
794.051 s; `/tmp/codefabric-p04-source-pin-final-native.log`). It verifies two reused source pins,
separate pins for changed generations and exact reuse after restart. The initial run stopped at an
outdated 60-second checkpoint wait while semantic writes were active; the helper now waits up to
180 seconds and this multi-stage fixture has a 15-minute nextest bound. The product wrapper allows
1200 seconds for staged/function-source sequences. Production deadlines are unchanged. The observed
four-file/219-byte initial semantic pass spent 25.689 s in Cargo/rustc and 104.311 s in relational
execution/writes; these are small-fixture costs, not representative performance qualification.

The parser continuation uses one native Python or Rust runner per retained file/context. Tree-sitter
receives a UTF-8-safe bounding edit derived from old/new captured text and parses with the edited
old tree. All current CST/fact rows are reprojected. Ruff reuses its AST, tokens and native indexes
only for equal text/decoding, Python version, ceilings and exact CST evidence; semantic projection
still uses the current admitted module/context. Cache entries are removed during mutation and dropped
on failure, so an advanced tree cannot remain paired with a failed Ruff update. Candidate-specific
syntax run IDs distinguish independently executed observations while canonical identities remain
application-owned.

Workspace cache eviction uses retained native reservations, shared memory/generation headroom and
last use. The existing joined census worker expires idle entries and capture removes absent files;
cache eviction never removes published Arrow/Delta facts. Counters distinguish retained entries,
parser reuse, Ruff parse reuse and evictions. Native reservation bytes remain declared capacity,
not measured RSS. The existing coarse envelopes still need representative calibration.

The combined parser/cache implementation passes all 21 focused ownership and adapter tests in
1.086 s (`/tmp/codefabric-p04-retained-syntax-focused.log`), including Unicode/disjoint edits against
fresh native trees, Ruff cache admission, eviction with held Arrow facts and cancellation recovery.
Final default/featureless `just root-check` passes in
`/tmp/codefabric-p04-retained-syntax-root-final.log`; affected Clippy reports zero changed-line
diagnostics in `/tmp/codefabric-p04-retained-syntax-clippy-final.jsonl`. Existing root warnings remain.
All 40 product-harness tests pass in 0.74 s with focused Python lint/format clean. The installed
live/clean function definitions, bodies and outlines case passes in 899.664 s against the syntax
checkpoint; the separate race/restart case passes in 814.946 s (two total, 1714.614 s;
`/tmp/codefabric-p04-retained-syntax-native-v2.log`). Root checks overlapped parts of that native
execution, so this is correctness evidence and an observed duration, not an isolated benchmark.
No full CI, doctest or outcome-completion claim is made.

The current Pyrefly continuation retains one contained checker under the workspace task scope.
A stable read-only mount contains only captured Python input views; the sidecar's existing writable
checker view receives digest-verified generations and native categorized changes serially. Its
existing complete inventory fallback reconstructs native state after deletion. Configuration,
provider executable metadata or ceiling changes retire the process; failed/partial runs, removal of
the last Python file and ten-minute idle expiry also retire it. Failed joins retain ownership and
block replacement. Census skips a busy checker so source observation can still cancel obsolete work.

The process owner now retains the private socket-directory descriptor through actual cleanup,
including cancelled construction. It samples aggregate cgroup CPU and peak memory every 250 ms;
between-run rotation after 600 accumulated CPU seconds avoids consuming a fresh job's allowance
from a one-shot lifetime limit. The contained service has an 1800-second cumulative CPU guard;
per-run wall deadlines and memory/process containment are unchanged. One private writable output
root is cleared only before launch or after a proved join; captured input/history is separate.
Counters report starts, reuse attempts, retirement and retained generations. These changes do not
claim complete shared CPU scheduling or representative memory calibration.

Initial Pyrefly wiring passes default/featureless checks. The two updated actual process-owner
cases pass (0.111/0.113 s) in `/tmp/codefabric-p04-pyrefly-retained-native.log`; cancelled construction
keeps the socket descriptor and residency until join. The initial installed Python corpus stops
at its 216.524-second reuse assertion: adding a module
changes the canonical manifest's module map, so this is a new effective context. The corrected
corpus tests real retained-checker call-target changes and restoration with a stable module map;
inventory/configuration changes require fresh contexts and still compare with independent clean
queries. The final cleanup/ownership selection passes all three cases in 0.238 s
(`/tmp/codefabric-p04-pyrefly-cleanup-focused.log`), including symlink-safe private-output removal.
Default/featureless checks pass in `/tmp/codefabric-p04-pyrefly-root-final.log`; final affected
Clippy is clean in `/tmp/codefabric-p04-pyrefly-cache-clippy-v4.jsonl`, including the extracted
context-transition assertion helper. The final installed corpus passes in 1528.600 s
(`/tmp/codefabric-p04-pyrefly-retained-final-native.log`, nextest run
`3388436d-bb08-463b-8788-1133a11685bc`). Both source-only call-target changes reuse the native
checker; module creation/deletion, Python version/platform and unsupported configuration follow
their expected retirement paths. All seven updates agree with separate clean query states.
Source-only generation 2 records one process start, one reuse and two completed generations. The expanded eight-state context corpus now has an 1800-second nextest bound
and 2100-second product-wrapper bound: the first one-file semantic pass alone spends 60.974 s in
relational execution/Delta writes, while the kernel sample reports 245 ms of provider-group CPU. The
initial failed invocation had a 900-second bound. These are observed
small-fixture costs, not representative performance results. The final 40-case harness run passes in 0.70 s with the revised timeout expectations,
and focused Python lint/format and documentation navigation pass.


The current storage continuation admits four exact Delta table writes at a time through
`FuturesUnordered`, within the existing native operation and shared session/memory pool. It stops
new admission after an observed failure and drains started writes before returning. Exact pins,
zero-retry native commits, descriptor restoration and candidate-local isolation are preserved.
The four focused Delta/storage cases pass in 1.607 s, and the installed first-four-form mixed
query/reopen corpus passes in 189.160 s. Default/featureless checks and affected Clippy pass for
that slice. Logs: `/tmp/codefabric-p04-bounded-delta-{focused,check,native-corpus}.log` and
`/tmp/codefabric-p04-bounded-delta-clippy-final.jsonl`.

The longer storage update comparison was interrupted by a user-confirmed `cargo clean` across
active codebases: Pyrefly disappeared during the platform transition and nextest could not launch
its now-missing restart binary (`/tmp/codefabric-p04-bounded-delta-updates.log`, 965.842 s).
That run is not acceptance or a valid before/after performance comparison. The selected extractor
and sidecar were rebuilt without dependency changes; `just extractor-identity` passes
(`/tmp/codefabric-p04-extractor-rebuild.log`, 13.24 s), and the sidecar binary build passes
(`/tmp/codefabric-p04-sidecar-rebuild.log`, 52.54 s). The subsequent update/restart selection passes as recorded below.

The next P04 slice prunes native watch registration using the inventory walker's directory
exclusions. Each selected directory receives a non-recursive watch before child enumeration;
symlink subtrees and build/cache trees are excluded. Directory/rename/loss hints rebuild the
bounded topology under the existing blocking owner, with old registrations held until replacement
and native `Debouncer::stop` on all owned exit paths. Forced repairs survive the coalescing window;
periodic reconciliation also requests topology repair. Registered/excluded directory counts are
traced. These are observation hints: secure capture still enforces source/root authorization.
Recreating a watched directory does not automatically authorize a substituted workspace root.

Six native watcher/ownership cases pass in 1.559 s before the polling addition
(`/tmp/codefabric-p04-pruned-watch-focused-final.log`), including silence for excluded generated
writes, symlinks, nested directory creation, root recreation and held repair requests. The installed
Python sequence initially reaches its old 60-second freshness deadline during initial preparation
(`/tmp/codefabric-p04-pruned-watch-live.log`, 101.527 s). Its saved source/semantic write phases are
32.682/56.826 s. The fixture now uses the existing 120-second freshness allowance and a 900-second
nextest bound; `python-live` has a 1200-second product-wrapper bound. Production defaults are
unchanged; startup latency remains unqualified. Its immediate retry was stopped before execution
because the provider binaries had been removed.

A restart-required `static_config.source_watch_profile` now explicitly selects `native` (the
compatible default) or `poll`. Polling uses native `PollWatcher` with a two-second interval
and independent 150 ms debounce/50 ms tick. Content comparison is disabled: the recovery-parent
registration must not hash unrelated sibling files, and secure census already verifies source bytes. It shares the pruned topology and
owned recovery path; it is not a silent fallback. Polling metadata IO and detection latency remain to be measured. Selected Git metadata topology subsequently lands below. Full inclusion/conflict policy, external
roots, retained Cargo contexts, dependency-aware owner reuse and shared scheduling remain open in P04.

The assembled storage/watch code passes the native Python edit/delete/atomic-save/empty/reopen
sequence in 605.277 s and the mixed source/semantic obsolete-completion/restart sequence in
796.409 s (`/tmp/codefabric-p04-watch-storage-installed.log`, nextest
`61720db4-5daa-488f-879e-ed49237f163e`, two concurrent tests). The initial polling case in that
selection fails at its 30-second query deadline while a newly captured nested source snapshot is
still being written. The corrected fixture observes durable source publication independently
before querying, holds semantic publication explicitly, and passes nested-file background convergence
and exact pending-stage reopen in 86.014 s (`/tmp/codefabric-p04-poll-installed-final.log`, nextest
`b942f219-90c9-482c-b21d-5accc7aa616f`). This distinguishes background publication from a query-forced
census; it does not isolate polling from periodic reconciliation or certify detection latency.

All nine focused watcher/configuration/failure-drain cases pass in 10.142 s
(`/tmp/codefabric-p04-watch-profile-focused-v2.log`). The final default/featureless root check passes
(`/tmp/codefabric-p04-watch-profile-root-final.log`), and affected Clippy has zero diagnostics
(`/tmp/codefabric-p04-watch-profile-clippy-accepted.jsonl`). Existing root warnings remain. All 44
product-harness cases pass in 0.76 s (`/tmp/codefabric-p04-watch-harness-final.log`); focused Python
lint/format and documentation checks pass. No full CI/doctest or performance claim is made.

## P04 retained Rust deployment inputs — accepted checkpoint

The pruned native/poll watcher and bounded exact-write checkpoint is committed in `2e71be9e`.
The retained Rust toolchain implementation is committed in `44053b64` and passes the installed
clean/update/reopen scenario. One immutable captured Rust toolchain bundle lives under the existing
workspace resource owner. Every semantic pass resolves the dated compiler/host and extractor and freshly
captures the host C driver/search selection. Reuse additionally checks canonical targets, inode,
size, mode and modification/change times for every captured file and directory. A changed selection,
missing input, directory transition or failed validation discards the cache entry. These metadata
observations detect ordinary deployment changes; context identity still derives from captured bytes.

Bounded capture now checks file-descriptor metadata before/after reads, checks cancellation in 64 KiB
chunks and validates the complete directory/file observation set before retaining the bundle. A
charged immutable lease spans each active compiler pass, so idle/headroom eviction cannot release
its capacity early. The existing census worker performs opportunistic idle eviction without waiting
behind capture. Last-Rust-file removal clears retention. Compact preparation reports record captures,
reuse, eviction, retained entries and charged bytes. This is toolchain-input reuse; private Cargo
output and extractor fact production remain fresh for each requested run. Automatic tool-change
invalidation without another semantic pass and compatible Cargo-unit output retention remain open.

The first four focused cache/context tests pass in 0.020 s
(`/tmp/codefabric-p04-toolchain-cache-focused.log`). The final five focused cases pass in
`/tmp/codefabric-p04-toolchain-cache-native-final.log`. Its installed scenario fails after 167.724 s
at the first adapter query timeout: Cargo/toolchain preparation completes in 26.579 s, followed by
98.182 s of still-running relational execution/writes. The captured bundle retains 1,693,175,554
charged bytes. This is a failed invocation, not a performance or product acceptance result.

Shutdown after that timeout also produces native `buoyant_kernel_engine` executor cancellation
panics and exhausts the daemon's two-second structured-task cleanup allowance, leaving writer
retirement unproved. This failure remains open. The native executor's blocking-return task is
cancelled during private-runtime destruction; no dependency/runtime changes have been made for
that path. Resolve real publication cancellation/drain behavior before claiming sustained recovery.

The fixture now waits for the exact selected source bytes in a semantic activation before issuing
its public comparison, reusing the existing 180-second bounded input-readiness helper. The final
selection passes all six tests in 522.443 s (`/tmp/codefabric-p04-toolchain-cache-native-v2.log`,
nextest `eeb8316f-860a-43bc-9040-1eb2849f7e98`). The installed scenario takes 522.423 s: the Rust
call-target edit reuses one captured toolchain, all four public forms agree with independent clean
state, and exact reopen preserves the result. The clean daemon records its own first capture.

The observed initial/warm Cargo-and-toolchain phases are 28.379/7.809 s; relational execution/writes
remain 102.652/99.017 s. Both states retain one 1,693,175,554-byte charged bundle. Root checks
and Clippy overlap parts of this run, and no isolated performance or representative scale claim is
made. Final default/featureless root checks pass (`/tmp/codefabric-p04-toolchain-cache-root-v3.log`).
Affected Clippy is clean (`/tmp/codefabric-p04-toolchain-cache-clippy-v3.jsonl`), including modified
function headers after extracting pending-target setup and the test cost reader. The read buffer
is bounded on the heap and pointer assertions are explicit. Documentation checks pass.

The publication-shutdown continuation corrects a separate confirmed control-protocol timeout:
the daemon drains accepted queries for up to ten seconds before acknowledging `Drain`, while the
supervisor previously used its ordinary two-second IO allowance. `Drain` now uses the existing
30-second accepted-work shutdown allowance, with ordinary control deadlines unchanged. A delayed
authenticated acknowledgement regression, ordinary timeout/generation retirement and cooperative
cleanup ownership all pass (three cases, 0.063 s; `/tmp/codefabric-p04-drain-focused.log`). Compact
cleanup-deadline diagnostics name at most eight remaining owned tasks without changing join policy.

Three installed probes stop after real, unselected semantic Delta data commits and require clean
supervisor joins, same-source-generation reopen and independently expected public facts. Python-only
publication passes in 126.463 s (`/tmp/codefabric-p04-publication-shutdown-diagnostic.log`); mixed
Python/Rust publication passes in 278.169 s
(`/tmp/codefabric-p04-mixed-publication-shutdown-diagnostic.log`). Those two probes predate the
control correction. The final mixed probe additionally forces a 0.5-second installed-client transport
timeout during publication and passes in 273.319 s with the correction
(`/tmp/codefabric-p04-client-timeout-publication-shutdown.log`, nextest
`3bfc2f01-3645-43ef-855a-a932c06d5270`). Its captured success stderr contains no native executor
panic. These checks did not reproduce the earlier failing cleanup cascade; its cause remains open.

The runs use `just root-test-incremental -j 1` with exact nonempty selectors, the installed extractor
and sidecar, and a delegated user-systemd scope on Linux, against `90e372ca` plus this continuation.
`just golden --case publication-shutdown`, `mixed-publication-shutdown` and
`client-timeout-shutdown` expose the cases. Default/featureless `just root-check` passes
(`/tmp/codefabric-p04-publication-drain-root.log`), and final affected Clippy is clean, including
modified function headers (`/tmp/codefabric-p04-publication-drain-clippy-final.jsonl`). All 48
product-harness tests pass in 0.79 s with focused Ruff checks clean
(`/tmp/codefabric-p04-publication-drain-harness.log`). No full CI or doctest claim is made.
P04 input topology/invalidation and retained-context scheduling remain open with P05–P14; the final
user-requested pause and next entry point are recorded in the package handoff below.

The subsequent Git observation slice (`20230937`) resolves the selected worktree's actual Git/common directories
with isolated, worker-local gix 0.86 handles. It adds at most six non-recursive metadata/parent
registrations, deduplicates source/metadata registrations and filters callbacks to the selected
index, configuration, `.git`/administrative pointers and `info` policy inputs. Objects, refs, logs
and other worktrees are not recursively observed. In-place pointer changes request topology repair;
an installation-time topology comparison retains repairs missed before registration. Malformed or
absent Git metadata retains marker observation and never supplies source authority.

All 11 focused native/poll/topology/ownership cases pass in 15.042 s
(`/tmp/codefabric-p04-git-watch-focused-final.log`, nextest
`1f0047eb-38ad-4f7f-b1db-d0d6bffdce30`; `just root-test-incremental -j 2 --no-fail-fast` with
`test(workspace_updates::tests::) | test(git_state::watch_topology::tests::)`, `--no-tests=fail`).
The first run exposed missing repair for in-place `.git` edits; the correction passes. The poll
fixture also initially assumed subsecond metadata detection: notify 8.2 compares whole-second
modification times in the selected metadata-only profile. Its existing-file case now uses a known
observable timestamp change. Periodic reconciliation remains necessary for missed hints; this is
not a complete Git classification/inclusion or installed semantic-update acceptance claim.
Git-selected path classification, conflict provenance, explicitly configured external policy files
and selected dependency-root observation still follow. The source inclusion policy is unchanged.
Default/featureless `just root-check` and affected Clippy pass, including modified function headers
(`/tmp/codefabric-p04-git-watch-root.log`, `/tmp/codefabric-p04-git-watch-clippy.jsonl`).
Documentation navigation, focused spelling and diff checks pass. This continuation is based on
`8a92d019`; no full CI, doctest, semantic Git acceptance or P04 closure is claimed.

The census hashing continuation replaces a repeated linear search through all known directories
with a `BTreeSet` of discovered ancestors. Once an ancestor exists, its parents already exist,
so ancestor discovery stops there. Bottom-up directory hashing, byte ordering and framing are
unchanged; the separate vector of all leaf paths is removed. A 16,386-leaf fixture including
shared ancestors and a non-UTF-8 path preserves its frozen pre-change digest and reversed-input
digest. Its measured hashing step is 521.957 ms before and 27.748 ms after, in the local test profile
with one test worker (`/tmp/codefabric-p04-inventory-merkle-before.log`,
`/tmp/codefabric-p04-inventory-merkle-after.log`). This is a synthetic hashing sample, not a
whole-census or representative CPG throughput claim.

All seven inventory cases pass in 0.292 s, including real secure capture, source-generation fences,
metadata exhaustion and byte-native identities. The existing installed `python-poll-live` case
now also uses a real separate Git administrative directory outside captured source. Background
nested-source publication and exact-generation reopen pass in 85.998 s
(`/tmp/codefabric-p04-git-poll-merkle-installed.log`, nextest
`4df82f22-3395-4774-8bd2-b79d7eb14e04`). The invocation uses the exact nonempty test selector under
`just root-test-incremental -j 1`, installed providers and a delegated Linux user-systemd scope,
against `20230937` plus this continuation. Root checks overlap that installed run; it is correctness
evidence, not an isolated performance result. Default/featureless `just root-check` passes
(`/tmp/codefabric-p04-inventory-merkle-root.log`).
Final affected Clippy is clean, including modified function headers after extracting the Git fixture
setup (`/tmp/codefabric-p04-inventory-merkle-clippy-final.jsonl`). Documentation, focused spelling
and diff checks pass. P04 input validity, full topology, Cargo-unit reuse and shared scheduling remain
open; the hashing change does not alter any source/context selection or close a package.

The accepted P04 persistence continuation gives fully materialized, zero-row Arrow inputs an
immutable content identity. Native `MemTable` validation still checks partitions/schema; an exact
empty input then uses DataFusion's read-only `EmptyTable`. Every batch must be empty, and arbitrary
providers gain no identity from statistics or estimates. The existing Delta path requires full
executable-descriptor equality and validates the selected exact version before reuse. Provider
coverage and current processing remain independently published; empty fact storage is not an
absence/completeness assertion. Native provider admission, Rust syntax and input-observation tables
use the constructor. Nonempty data and changed descriptors retain candidate-owned writes.

All 22 focused schema/admission/Delta cases pass in 1.689 s
(`/tmp/codefabric-p04-empty-arrow-focused-v3.log`). The initial selection exposed old admission
fixtures missing the current Pyrefly/Rust binary fields; those fixtures now construct the selected
schemas. Focused cases cover actual empty-version reuse without a new table, empty→populated→empty
replacement, old readers, exact reopen, malformed schemas/partitions and explicit incomplete scope.
All three installed cases pass in 804.945 s (`/tmp/codefabric-p04-empty-arrow-installed.log`,
nextest `3b1fb102-ee05-40c6-8805-b5fa9927a1c3`): mixed first-four-form/reopen takes 214.848 s,
staged invalidation/restart takes 804.940 s and client-timeout/publication/reopen takes 265.048 s.
The exact nonempty selectors run with `just root-test-incremental -j 2 --no-fail-fast`, explicit
installed provider paths and a delegated Linux user-systemd scope, against `7e4ba9d9` plus this
continuation. The independent uv-only commit lands while these binaries run. Root checks overlap
execution; these durations are correctness evidence, not isolated performance measurements.
Default/featureless `just root-check` passes (`/tmp/codefabric-p04-empty-arrow-root.log`). Final
affected Clippy reports no changed-line diagnostics (`/tmp/codefabric-p04-empty-arrow-clippy-final.jsonl`);
existing root warnings, including the long staged integration fixture, remain. Documentation,
focused spelling and diff checks pass. No full CI or doctest claim is made.
The staged case now checks a real Ruff comment's empty reuse, creation and deletion alongside public
pending/semantic results. Shutdown probes wait for eight real candidate writes because empty-version
reuse reduces the number of new tables; no fixed total table count is a correctness assumption.

P04 selected provider deployment invalidation is committed in `722b57d4` and passes installed
qualification on 2026-09-10.
One small observation over the selected Pyrefly/rustc-extractor executable paths, canonical targets,
inode, size, mode, ownership and nanosecond modification/change times is persisted in the existing
source inventory state relation. Missing executables are observed explicitly; other observation
errors leave freshness unavailable. This witness is independent of retained cache lifetimes and
is an invalidation trigger, not a content identity or replacement for captured provider contexts.
Capture, source reconciliation and candidate publication compare it alongside inventory/generation;
old exact epochs without a witness require reconciliation. Source bytes remain separate. Changed
executables trigger conservative source/pending/semantic replacement, including after restart.

The same selected-path resolver feeds both production launch paths and observation. Periodic
reconciliation now continues during long provider/relational work; an input mismatch cancels the
private build scope and awaits its existing owner. Rust daemon reference §27.2, Select pattern,
guides retaining/joining the build rather than assuming a dropped future stops native work.
This observes two provider executables only. Complete sysroot/linker/runtime-image and external
input observation, finer invalidation and shared CPU/context scheduling remain open.

The initial ten focused deployment/watch tests pass in 15.046 s
(`/tmp/codefabric-p04-provider-deployment-focused.log`). The added legacy-state mismatch and
executable mutation cases also pass in the first installed selection. Its product case first fails
because the fixture omitted independent source disclosure permission (107.757 s,
`/tmp/codefabric-p04-provider-deployment-installed.log`, nextest
`dd8ae54c-6d11-437e-a36e-db09b0dd2c93`). The second attempt passes initial four-form answers but fails
in the independent raw-storage reader (105.650 s,
`/tmp/codefabric-p04-provider-deployment-installed-v2.log`, nextest
`3ba73612-c7f2-4eaf-b7ab-adb88d201ee6`): raw Delta stores logical UInt64 as Decimal128 and fixed IDs as
Binary. The fixture now grants disclosure before launch, reads generation from selected activation
pins and reads raw digest arrays as Binary. Production schema restoration and authorization were
not weakened.

The final installed case **passes in 443.603 s**
(`/tmp/codefabric-p04-provider-deployment-installed-v3.log`, nextest
`225ab70b-5d09-4915-b5be-8ad8bf7c1e48`). It replaces private installed provider copies while preserving
length/mtime, observes a background successor without a query or source edit, rejects a completed
obsolete deployment, and compares independently expected declarations/facts/calls/source after
repair and changed/unchanged restart. Unchanged restart preserves the selected generation; captured
source bytes/inventory remain unchanged throughout. Removal/symlink/raw-path variants are unit
coverage; the real installed redeployment uses Pyrefly, while both production launch paths share
the same observation resolver. This is scoped acceptance, not all-tool/all-platform deployment
qualification or a full legacy schema migration exercise.

The exact final command, against `096c6db7` plus the changes committed as `722b57d4`, is:

```sh
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus XDG_RUNTIME_DIR=/run/user/1000 \
CODEFABRIC_RUSTC_EXTRACTOR_BIN=/home/paul/CodeFabric/target/extractor/debug/codefabric-rustc-extractor \
CODEFABRIC_PYREFLY_SIDECAR_BIN=/home/paul/CodeFabric/target/debug/codefabric-pyrefly-sidecar \
systemd-run --user --scope --quiet --property=Delegate=yes \
just root-test-incremental -j 1 --no-fail-fast --success-output immediate \
-E 'test(selected_provider_redeployment_fences_delayed_facts_and_reconciles_reopen)' --no-tests=fail
```

Default/featureless `just root-check` passes
(`/tmp/codefabric-p04-provider-deployment-root-final.log`). Final all-target affected Clippy
completes with no changed-line or modified-function-header findings
(`/tmp/codefabric-p04-provider-deployment-clippy-final.jsonl`); existing root warnings remain.
The initial large-future finding is corrected with an owned boxed build future. The final installed
run includes that correction and the corrected raw-storage helper. All 50 product-harness cases
pass in 0.80 s (`/tmp/codefabric-p04-provider-deployment-harness.log`), with focused Python lint/format
clean. New Rust modules pass focused rustfmt checking. The product selector is
`just golden --case provider-deployment-live`, with a 600-second bound; its native case and wrapper
are validated separately. These timings include concurrent static checks and are not isolated
performance measurements. Full CI and doctests were not run for this slice.

## P03 first-release query boundary delivered

The plan corpus is committed in `47b0c225`; the shared native schema-identity correction is
committed in `445bcbda`. The final mixed installed run passes both cases in 418.109 s
(`/tmp/codefabric-p03-related-and-corpus-final.log`): the source-authored plan corpus takes
190.916 s and related-occurrence/relationship/source/reopen acceptance takes 227.189 s.

Related source contexts use native joins over canonical calls, references and imports. A distinct
capability role and anchor meaning preserve selection through resolution/replay. Output retains
the requested anchor separately from the actual occurrence, with relationship, resolution and
unknown reason. Descriptor union/deduplication precedes the exact byte-buffer join. Authorized
boundaries apply to returned files; in-file lexical occurrences remain visible when outside callers
are excluded. Bounded native descriptor lookup narrows fully known explicit anchor language/context
sets while keeping incoming occurrence files broad. Unknown/excluded/disjoint anchors preserve
conservative coverage. Native Python reference and Rust lexical/reference gaps stay visible.

The installed case verifies repeated subjects, typed prior reuse, source-location equivalence,
exact bytes, requested boundaries, scoped processing and exact reopen. Arrow IPC metadata confirms
that the returned occurrence has the reusable public-identity role; the anchor has a separate role.
Disclosure/snapshot-bound source handles are validated and compared separately from canonical
facts and bytes. Neither empty subsets nor absent exhaustion observations assert complete absence.

The plan's `pragmatic_cpg/workspace` and `expectations.json` now drive the `first-release-queries`
case: all four forms, Python/Rust declaration names, independently constructed primitive type IDs,
imports, resolved and unknown calls, definition bytes/positions and canonical/processing equality
on exact reopen. `just golden --case first-release-queries` selects it; the measured run used the
same exact nextest selector inside the delegated user-systemd fixture. All 40 product-harness tests
pass (`/tmp/codefabric-p03-corpus-harness.log`), with focused Python lint/format checks clean.

Fourteen compiler/metadata tests and 64 affected session/catalog/compiler/source tests pass.
Seven source-scope cases, including incomplete/disjoint anchor selection, also pass in
`/tmp/codefabric-p03-related-and-corpus.log`. That earlier combined run failed only because its
new corpus test assumed optional `additional_rows` was present; the corrected final run passes.
Default/featureless checks pass in `/tmp/codefabric-p03-related-anchor-context-check.log`;
changed-line Clippy is clean in `/tmp/codefabric-p03-related-final-clippy.jsonl`. The final native
outline/revocation and walk-metadata regressions pass (two tests, 118.586 s) in
`/tmp/codefabric-p03-native-metadata-regression.log`.
Global baseline lint/format issues remain separate; no full CI or doctest closure is claimed.

This delivers P03's first-release query integration boundary. Full first-four meanings/subject roles
continue with their P06/P10 producers; target/family convergence and explicit historical query
selection remain in P04/P11. P04 has since delivered scoped retained source/provider state, exact
source/empty pin reuse and provider executable invalidation; its complete boundary remains open. P05 still owns the assembled edit/
clean/restart first-useful-release boundary. No outcome from 4 through 8 is complete.

Named traversal stops are committed in `f0a70a03`. The P03 syntax-outline slice now passes validation.
A new source-context capability role advertises outline anchors only on supporting snapshots.
Functions use their full exact CST owner; other captured subjects use their source spans. Native
joins select CST rows wholly contained in an anchor's exact workspace/file/digest/generation,
retaining node/parent identity, raw/normalized kinds, parser flags, ordinal/depth, source positions,
and both semantic-anchor and syntax-provider contexts. The output is a flat source-range outline,
with deterministic node ordering before limits. Whole-file bytes and source-text payloads are
removed before outline expansion. A volatile native scalar function checks live source disclosure;
retained results continue to carry their source dependency.

The installed mixed Python/Rust function scenario passes in 811.901 s: exact definitions/bodies,
full CST outlines, edits/restoration and both independent clean comparisons agree. The installed
syntax-only case passes in 105.342 s, including explicit row truncation, every advertised page,
exact reopen and retained/new-query disclosure revocation. Both compiler return-action cases also
pass (four total in 917.262 s, `/tmp/codefabric-p03-outline-explicit-and-live.log`). Six focused
source-processing checks pass in 0.031 s, including the distinct semantic/function-owner/parser
context dependencies (`/tmp/codefabric-p03-outline-processing-final.log`). The exact occurrence
validity/provenance check passed earlier. The final installed run also verifies `additional_rows: true` for each explicit three-row outline
limit and passes in 105.768 s (`/tmp/codefabric-p03-outline-observed-limit.log`).

Default/featureless checks pass in `/tmp/codefabric-p03-outline-root-check.log`. Final Clippy reports
no diagnostics on changed lines in `/tmp/codefabric-p03-outline-final-clippy-v2.jsonl`; existing
warnings in large surrounding functions remain. Documentation/navigation and changed-line spelling
checks pass. Whole-file spelling still flags existing escaped Latin-1 fixture bytes; these are
preserved. No full CI or doctest claim is made. Validation uses the existing installed providers,
shared incremental target and delegated user-systemd scope. Commands were `just root-check`,
`cargo clippy --locked --all-targets --message-format=json` through `cargo-check-mode.sh`, and
`just root-test-incremental -j 1` with the exact test selectors shown in the logs, plus
`just docs-check`, changed-line `typos` and `git diff --check`.

The initial private-byte binding failure and missing explicit truncation binding were corrected.
The first mixed fixture run exceeded the adapter's 120-second timeout before its first entity query
finished. The fixture now waits for a semantic activation containing the exact edited Python/Rust
bytes and allows 15 minutes for its five preparations; production timeouts are unchanged. The
four-file/462-byte initial snapshot measured 30.872 s for source relational execution/writes,
25.919 s in Cargo/rustc and 96.547 s for semantic execution/writes across 132 relations. These are
small-workload observations for P04/P12/P14, not representative optimization evidence. Earlier
failures remain attributable in `/tmp/codefabric-p03-outline-*.log`.

That outline checkpoint continued into the accepted related-occurrence and plan-corpus work above.
P04–P14 remain open; broader meanings and roles follow their producers in P06/P10.

The bounded-call slice is committed in `69beaec3`. The next P03 slice adds named stopping
conditions through the existing canonical name selector and native DataFusion left anti joins.
An arriving witness remains visible; expansion from a stop entity is excluded, including an
initial subject that is itself a stop. Joins bind public entity, context and workspace IDs.
Repeated/multiple names form a stop set; an absent name excludes nothing. Literal names remain
separate from evaluative request wording. No application path enumeration or new native dependency
is introduced (DataFusion reference §52, join planning decision model, and resolved 55.0.0 sources).

The installed Python/Rust stopping scenarios passed in 211.809 s, including prior bounded walks,
filters, source boundaries, cycles and exact reopen (`/tmp/codefabric-p03-stopping-native.log`).
Two literal-name checks also passed. Default/featureless checks and affected Clippy pass.
The correction reports unsupported stopping phrases as block-level semantic-unavailable
results and blocks their dependents while preserving independent work; explicitly installed
controlled stopping meanings remain supported by their catalog resolutions. All 12 ingress checks
pass in `/tmp/codefabric-p03-stopping-native-final.log`. Its installed case exposed a fixture dependency-role error (`entities` from a `facts` producer); after correcting
the test operand, final installed Python/Rust traversal, unsupported-stop branch isolation, all
advertised pages and exact reopen pass in 206.443 s
(`/tmp/codefabric-p03-stopping-native-roles.log`). Default/featureless checks pass in
`/tmp/codefabric-p03-stopping-root-check-final.log`; final affected Clippy reports zero diagnostics
in `/tmp/codefabric-p03-stopping-guard-clippy.jsonl`. Docs/navigation, spelling and diff checks pass.
Validation uses the existing installed providers and delegated user-systemd scope; no full CI or
doctest claim is made.
Other stopping meanings, transitive closure, precise visited-owner processing, source outlines/
related contexts, remaining first-four scopes/directives and P04–P14 remain open.

The P03 bounded-call continuation after `04b48f22` passes installed public validation. Exact
walks of two through eight steps return the final-hop witnesses; cumulative walks return distinct
witnesses from every included hop. Incoming and outgoing programs use DataFusion frontier
projections and left semi joins, retaining separate call sites without enumerating paths. Filters
apply to every edge, and source boundaries prune every traversal input, including paths that leave
and re-enter a selected file. Each program has its own result authority and field namespace.
Repeated projection IDs require identical epoch lineage. Older one-step contracts remain available
when their snapshots lack the fields needed for walks. Multi-step processing keeps broader scope
until all visited owners can be accounted for.

Storage scans now preserve their declared metadata with the existing schema identity execution
node, including native statistics, ordering and partitioning delegation. Native projections at
aggregate inputs after logical optimization preserve internal metadata through composed plans;
type/nullability checks remain enabled. A direct aggregate probe reproduced the omitted-metadata
failure before the fix. Eight focused metadata/provider checks now pass, including exact native
walk edges and logical field identity metadata. Nine compiler/processing checks passed in 1.359 s
(`/tmp/codefabric-p03-distance-focused.log`). Default/featureless checks, affected Clippy (zero
affected diagnostics), docs/navigation, spelling and diff checks pass.

The installed Python/Rust walk, per-edge filters, repeated/empty priors, cycles, source-boundary
leave/re-entry case, every advertised page and exact-version reopen pass in 191.384 s
(`/tmp/codefabric-p03-distance-native-expected.log`). This fixture explicitly selects resolved
call observations for its independent expected edge sets. Earlier catalog/field-namespace,
metadata and fixture expectation failures remain in the `/tmp/codefabric-p03-distance-*.log`
logs; the current walk passes. Validation uses the existing installed provider binaries and a
delegated user-systemd scope. The existing first-four-form public scenario passes in 176.262 s,
and all 11 child-session checks pass (12 total in 178.160 s), in
`/tmp/codefabric-p03-distance-legacy-regression.log`.
Stop conditions, transitive closure, further relationship families/directives, precise multi-step
processing, remaining P03 source/query scope and P04–P14 remain open.

The P03 owner-processing continuation after `9a5d8e0b` passes public validation. Native
DataFusion groups associated member observations by exact class owner/workspace/context/source
generation, counts class census rows and local unknowns, and preserves pending/failed provider
states. Explicit class-ID member queries select those owned partitions; broad and older-epoch
fallbacks remain available. Retained remainders use class public IDs and the selected member family.
Structural error types retain valid IDs, so a bounded linear native reverse-dependency pass now
propagates error/unknown/unsupported/missing components to affected declared/computed member
roots. Associated-name census completion remains independent of type completeness.

Four focused owner/family/pending/wrong-pin/retained-selection checks pass in 0.088 s. All 39
processing/canonical checks pass in 12.733 s (`/tmp/codefabric-p03-member-owner-integrated-focused.log`).
All 42 sidecar cases pass in 2.07 s (`/tmp/codefabric-p03-member-precision-sidecar-tests.log`),
including direct errors, nested tuples and method parameters without widening the known class.
Strict sidecar check/Clippy and the rebuilt provider pass. Final installed empty/incomplete class
queries and exact reopen pass in 86.056 s (`/tmp/codefabric-p03-member-owner-native-precision.log`):
the empty class is complete, the error-bearing class retains one owner remainder with its actual
class public ID, and every fact page is read. Default/featureless checks, final affected root Clippy
(including changed function headers), local Pyrefly package Clippy, docs, spelling and diff checks
pass. The earlier installed run exposed the error-type coverage issue and remains attributable
in `/tmp/codefabric-p03-member-owner-native.log`. Validation uses the rebuilt sidecar and delegated
user-systemd scope. Prior-result/descriptive-subject processing refinement remains open; their
facts remain correctly selected by native query plans. Remaining P03 traversal/directive/context
scope and P04–P14 stay open.

The P03 native member continuation after `71353d60` passes installed public validation. Native
class declarations supply bounded class/member DTOs from exact `KeyClassField` bindings, with
separate declared/computed type indices into the shared graph, source anchors, final/class-variable/
property facts and native descriptor setter/deleter hooks. Missing binding/answer fields remain
nullable. Associated membership census completion is separate from type completeness; inherited
or effective receiver lookup is not asserted. An additive Arrow member stream (family 149)
replaces rendered-name candidate discovery; the legacy display relation derives only known native
values. Canonical `fact.code_member_observation` joins exact workspace/context/source/generation/
run-scoped owners and structural type IDs. Member declaration mappings remain optional; implicit
associated fields retain their native evidence. Class census rows distinguish known empty scopes
from missing native answers. Public `associated member observations` supports canonical and prior
entity subjects with conservative per-file member-processing coverage.

All 41 sidecar cases pass in 2.04 s (`/tmp/codefabric-p03-members-transport-tests.log`), including
native descriptor hooks, Arrow anchors/nullable flags and census limits. Strict sidecar check/Clippy
passes; the final transport binary is rebuilt. All 29 canonical/query cases pass in the combined
initial run; its native case exposed a missing closed processing-family registration, now fixed.
The next run exposed differing producer/consumer module digest order after the additive relation.
Sorting after all native relations are assembled fixes it, with a focused contract regression.
The final installed Unicode/nested/empty/final/property/class-variable/prior/reopen scenario passes
in 84.109 s (`/tmp/codefabric-p03-members-native-transport-fixed.log`), reading every advertised
page and independently constructing the expected Python primitive type identity. Default and
featureless checks, affected root Clippy, docs, spelling and diff checks pass. The local Pyrefly
package Clippy invocation succeeds using its cached checked artifact. Validation uses the rebuilt
sidecar, stable root and delegated user-systemd scope; failed earlier runs remain in `/tmp`.
Precise owner processing, full signatures, inherited/dispatch/member normalization, Rust member
production and the remaining P03–P14 scope stay open.

The P03 continuation after `695a51e2` adds a validated declaration-type census inside the existing
Pyrefly expression-type transaction. Native Ruff AST visitation reaches unused and nested function/
class declarations; exact native `Key::Definition`/`Answers::get_type_at` lookups seed the same
bounded structural graph without inventing expression occurrences or repeating positional AST searches. This prepares native class-member discovery and improves callable evidence for unused
definitions. Sidecar check/Clippy passes. Both focused native declaration/structural tests pass in 0.33 s
(`/tmp/codefabric-p03-declaration-census-sidecar-tests.log`), including unused nested functions,
methods and classes plus bounded graph/unchanged occurrence census behavior. The sidecar binary
is rebuilt for the final indexed lookup. All 40 sidecar cases pass in 2.40 s
(`/tmp/codefabric-p03-declaration-census-indexed-sidecar-all.log`); sidecar check/Clippy and
affected root Clippy pass. The final installed public unused-function/prior/reopen case passes
in 181.706 s (`/tmp/codefabric-p03-declaration-census-native-final.log`), including independent
primitive type identities and exact-version facts. The earlier positional prototype also passed
(196.511 s); those timings are not a controlled performance comparison. Docs, spelling and diff
checks pass. Member extraction, full signature variants and P03 completion remain open.

The callable-type continuation after `b6fa5af8` passes native validation. A new canonical
`fact.code_callable_type` relation binds Python callable components through Pyrefly's native
function-definition anchors and exact run/context/source keys, and Rust parameter/return types
through admitted MIR owners and slots. It preserves component order, Python parameter kind/name/
requiredness, native evidence kind and unknown reasons. Public `parameter and return type
observations` retrieval selects the exact canonical owner; existing prior-entity consumers apply.

A native anti join emits an explicit unknown for canonical functions/methods with no callable
evidence. Those gaps qualify the existing type-processing family. A body slot is identified as
MIR evidence; observed Python function types are not asserted to be a complete declaration-signature
census. The new relation leaves existing persisted type schemas and provider frames unchanged.
Default/featureless root checks pass. All 29 canonical/recipe cases pass in 13.681 s
(`/tmp/codefabric-p03-callable-focused-all.log`), including native unknown-owner/context/workspace
isolation and absent-provider behavior. Final affected Clippy has zero diagnostics. The first
installed run passes all initial family retrievals and independent Python primitive/parameter
checks, then exposes a fixture lookup that omitted Rust qualified names; the lookup is corrected.
The final installed family/prior/reopen run passes in 167.659 s
(`/tmp/codefabric-p03-callable-native-final.log`): independently constructed Python `int` and Rust
`u8` identities match parameter/return facts, prior FindEntities subjects select only their owners,
and all canonical facts remain identical after exact-version reopen. The mixed fixture uses the
existing 180-second preparation helper. Logs retain earlier compile and plan-dependency failures,
both corrected before this run. Final affected Clippy, docs, spelling and diff checks pass. Full
declaration signatures, members, update/clean comparisons and the remaining P03–P14 scope stay open.

The semantic-reference scope continuation after `1cba8112` adds a catalog-selected FindEntities
program for references targeting prior or named canonical entities. DataFusion 55 left semi joins
bind canonical target/reference IDs, exact analysis contexts and workspace identity before existing
filters, ordering and limits. Repeated subjects/candidate rows do not multiply occurrences; empty
prior results remain empty. Unsupported semantic scopes receive typed block failures. Ordinary
census and captured-location queries keep their selected meanings.

Scoped Find results retain the entity-role schema for downstream facts, relationships and source.
Materialized consumers accept distinct producer relation names only with identical ordered semantic
fields and roles; runtime still requires exact Arrow names, types, nullability and metadata. Inline
composition retains its relation constraint. Field renaming now tracks bindings produced by each
subtree, preserving same-ID prior inputs and carrying nested projection/aggregate values into their
actual downstream uses. Ingress also supplies the documented one-step relationship default.

All 53 focused ingress/recipe/compiler/native-program/retention cases pass in 1.907 s
(`/tmp/codefabric-p03-find-scope-unit-verified.log`). Native binding cases cover null-bearing input,
independent filtered/sorted values, sums and repeated intermediate projections. Default/featureless
root checks and affected Clippy pass with zero affected diagnostics. Installed prior-result
fan-out/source/reopen passes in 74.788 s; independent/all-unavailable branches/reopen passes in
81.350 s (`/tmp/codefabric-p03-find-scope-native-final.log`). The final mixed Python/Rust scope,
traversal, source and exact-version reopen case passes in 165.151 s
(`/tmp/codefabric-p03-find-scope-native-reopen-final.log`). Its comparator validates fresh
snapshot/disclosure-bound source handles and compares every canonical/provenance field and
delivered source byte/range. Final affected Clippy, navigation, spelling, diff and tool-version
contract checks pass. No full-suite or doctest result is implied.

An earlier mixed run did not converge within its 120-second semantic preparation wait:
Cargo/rustc took 27.34 s and relational writes were unfinished after 93.22 s; source publication
wrote 84 relations in 24.83 s (`/tmp/codefabric-p03-find-scope-tests-6.log`). The fixture now uses
the existing separate 180-second preparation helper before queries. Startup within 120 seconds
remains unqualified for P04/P14. The workstation `uv` drift to 0.12.12 was reconciled to the pinned
0.12.11 before the final rerun. Broader Find scopes, owned parameter/return/member facts and the
remaining first-four meanings remain P03 work; no package or outcome exit is claimed.

The ingress continuation after `d0c437b8` retains typed unavailable blocks without inventing a
program or relation binding. Missing released-form programs, prior-input consumer slots and
selection/return/input targets,
unsupported quoted entity meanings/source-location interpretations and incompatible fact-family
combinations now remain local to their query blocks. Partial projections and guard requirements
from failed branches are removed; descendants receive typed dependency failures. Existing exact
catalog, global scope, guard-answer, request-shape and DAG validation still applies.

Native petgraph `DiGraph`/`toposort` validates the unavailable dependency graph, including cycles,
unknown/active predecessors and configured edge/fan-in/fan-out bounds. Private indices never escape.
The existing manifest-only path can retain a request with no executable ingress blocks at all.
Twenty-two ingress/compiler cases pass in 0.12 s
(`/tmp/codefabric-p03-unavailable-ingress-tests-1.log`); all 26 broader compiler/recipe cases pass.
The expanded public fixture initially omitted the summary form's required `group_by`; this is
corrected. All 16 final focused cases pass. The older fan-out fixture had a stale ordering
assumption: the released default selects `helper` first by source position, while that scenario
expects `first`. It now explicitly requests name-ascending ordering for its intended one-row
producer. Final installed fan-out/facts/calls/source/reopen passes in 75.60 s; the expanded
independent/all-unavailable branch and reopen case passes in 80.99 s
(`/tmp/codefabric-p03-unavailable-ingress-public-final.log`). Default/featureless checks and final
affected Clippy (zero diagnostics) pass, as do navigation, affected spelling and diff checks.
Full first-four meanings/scopes and later packages remain open.

The preparation continuation after `f5311963` changes canonical subject/property checks and source
disclosure denial from request-wide errors to typed block failures. Unsupported captured-location
scope also stays local to its block. Failed branches are removed before query-relevant processing,
request-input materialization and native planning. The compiler's validated dependency order carries
failure to descendants while preserving independent results and original public outcome order.
Source grants are still required for admitted source execution and retained source reads.

Twenty focused ingress/backend/outcome tests pass in 0.11 s
(`/tmp/codefabric-p03-preparation-tests-1.log`). Installed mixed-success/all-failed/reopen passes
in 81.01 s and mixed Python/Rust literal/property reopen in 153.84 s. The source grant/revocation/
reopen/edit case reaches the pinned-read edit barrier, then hits its old 120-second outer test bound
during generation 2 publication (`/tmp/codefabric-p03-preparation-native-1.log`). The source stage
writes 78 relations in 23.99 s; semantic relational writes are still running after 13.01 s at
termination. The case now uses the same five-minute outer nextest bound as the other native
reopen scenarios. A focused rerun then reaches `FRESHNESS_DEADLINE`: generation 2 source writes
take 24.32 s and semantic writes are still running after 32.80 s
(`/tmp/codefabric-p03-preparation-source-final.log`). Its positive convergence request now allows
90 seconds; 30-second barrier deadlines and source authorization assertions remain unchanged.
The focused rerun passes in 152.46 s (`/tmp/codefabric-p03-preparation-source-90s.log`), including
revocation, restart and the pinned old source read after an edit. This does not qualify convergence
within 60 seconds. Default/featureless checks, final affected Clippy (zero diagnostics), all 218
tooling cases (3.19 s), navigation and affected spelling/diff checks pass. This remains a P03 slice;
the unavailable catalog/form/consumer-slot projection gaps at that checkpoint are addressed by
the ingress continuation above. Malformed structure and inconsistent global authority remain
request-wide errors.

The scheduling continuation after `12b95a1b` uses a native petgraph dependency graph and a
deterministically ordered ready set. Completed producers unlock their consumers immediately;
unrelated running roots do not create a wave barrier. Query-owned `FuturesUnordered` work is bounded
by the admitted child's target partition count (16 in the current workstation profile). Producer
results retain one canonical Arrow materialization and shared-pool ownership. Completion order
cannot change sealed relation order, per-query outcome order or prior-input order.

Child plans now defer native `execute_stream` until their consumer polls. Preparing leaf plans
therefore starts no unconsumed native execution; the owned stream wrapper still supplies native
task enrollment, cancellation and destruction scope. DataFusion §21.4, petgraph §16.10 and the Rust
daemon reference §25.3 inform this use of native streaming, DAG validation and query-owned work.
The scheduler updates only outgoing dependency counts instead of rescanning every pending block.

Eleven deferred-stream/native-owner tests pass in 0.37 s. All 24 expanded runtime/graph/ownership
cases pass in 0.43 s (`/tmp/codefabric-p03-ready-tests-2.log`), including controlled native scans
that prove two-block overlap, bounded admission, immediate refill and cancellation of all pending
planners. The initial concurrency fixture mistakenly retained its one-partition configuration;
the corrected test binds both its workspace resources and child authority to two partitions.
Installed-client branch/reopen passes in 82.80 s and ordering/prior/reopen in 157.19 s; all 24
package/client cases pass (`/tmp/codefabric-p03-ready-native.log`). Default/featureless root checks,
navigation and affected spelling/diff checks pass. Final affected Clippy reports zero diagnostics
(`/tmp/codefabric-p03-ready-clippy-final.jsonl`); the final 13-case runtime/deferred-stream selection
passes in 2.52 s (`/tmp/codefabric-p03-ready-tests-final.log`). The earlier preparation cost limitation remains visible
below; these timings do not establish a comparative performance improvement.

The runtime continuation after `bb059db5` isolates native computation errors in both reusable
producers and leaf streams. Failed producers skip their dependents; independent blocks keep their
real Arrow results. Failed leaves stop native production, delete their trailing private pages and
reconcile the exact durable cleanup checkpoint before sealing. If deletion or checkpoint replacement
fails, the request fails and the previous recovery checkpoint retains ownership. A crash before
replacement leaves an idempotently deletable page set. Successful output observations and processing
summaries exclude the failed blocks; typed outcomes retain original request order.

DataFusion reference §33.2–33.3 and the resolved 55.0.0/Arrow 59.2.0 error enums ground the boundary:
native execution and arithmetic/cast/compute errors can fail a block. Resource, storage, schema,
internal, source hard-limit, cancellation and deadline failures remain fatal. Native `find_root`
preserves classification through contextual/external wrappers. Leaf results remain streamed, and
retained producer rows retain their native shared-pool reservation and owned lifetime.

The first ten fault/recovery cases pass in 0.20 s, including a real provider-column division by zero,
dependent skipping, partial-page deletion, exact reopen, stale-checkpoint rejection, cleanup failures
and resource release (`/tmp/codefabric-p03-runtime-branches-tests-2.log`). The expanded test also
covers a failing leaf and passes during the integrated run. That run passes 56 of 57 selected cases,
including the installed branch/reopen scenario in 121.72 s; the ordering scenario fails with a client
`MCPError` at the query step in 178.57 s, without an underlying public error
(`/tmp/codefabric-p03-runtime-branches-regression.log`). The serial rerun passes the source-window/
hard-limit/reopen scenario in 108.63 s and all five selected runtime/outcome cases, but ordering again
expires before query admission. Its semantic preparation records 26.62 s in Cargo/rustc and an
unfinished 95.39 s in relational execution/Delta writing at teardown. The ordering fixture now waits
for exact semantic activation within a separate 180-second preparation bound before its queries.
The final installed ordering/prior/reopen scenario passes in 207.03 s
(`/tmp/codefabric-p03-runtime-branches-ordering-ready.log`). Completed semantic preparation publishes
129 relations, spending 27.83 s in Cargo/rustc and 101.15 s in relational execution/Delta writes;
the source epoch's 84 relations take 47.63 s in that phase. These are observed costs, not comparative
performance qualification. Initial mixed-input readiness within the adapter's 120-second operation
deadline remains unqualified; preparation performance stays in P04/P14.
Default/featureless root checks and final affected Clippy pass. All
218 tooling cases pass in 6.58 s. The workstation uv mismatch
was reconciled from 0.12.12 to the required 0.12.11; `just tool-version-contract-check` passes.
Catalog/form/consumer-slot projection failures are now block-local as described above.
Prior FindEntities scopes and broader first-four semantics remain P03 work. No package or outcome exit is claimed.

The continuation after `82bda01b` now retains an all-failed compiler request as a typed result
manifest with zero relations, pages and data rows. The request operation finishes successfully;
each failed/skipped block keeps its actual execution outcome. No empty fact relation is fabricated.
Ordinary empty query results retain their typed Arrow schema, and the existing generic transaction
constructor still rejects undeclared empty output sets. That checkpoint left early request/input
and authorization errors request-wide; the preparation continuation above isolates the supported
canonical subject/property, captured-location scope and source-disclosure cases.

The durable publication checkpoint can own the manifest alone. Exact registration/reissue and
restart cleanup accept that shape while preserving checksums, paths, owner/lease checks and zero
data counts. Shared typed outcome validation uses the already-enabled petgraph 0.8.3 `DiGraph`
and iterative `toposort` for linear dependency-cycle checks; indices remain private and public
request order stays unchanged. Missing/complete dependencies and cycles cannot explain a skipped
block. The expanded installed branch/reopen case passes in 79.65 s
(`/tmp/codefabric-p03-outcomes-only-native-1.log`), including the all-failed request. Package
reissue/cleanup, undeclared-empty rejection and dependency-chain tests pass. The ordering scenario
also passes in 149.98 s; all 14 selected native/registry cases pass. Default/featureless root checks,
218 tooling cases, navigation, affected spelling and diff checks pass. Final Clippy has no new-file/
changed-line findings (`/tmp/codefabric-p03-outcomes-only-clippy-final.jsonl`). All 32 expanded
coordinator/package/runtime regression cases pass in 0.38 s
(`/tmp/codefabric-p03-outcomes-only-regression.log`). Full-suite and baseline lint closure are not claimed.

The independent compiler-branch continuation after `c7012005` isolates unavailable return meanings
and invalid return ordering to their owning block. Valid dependents of failed blocks receive
`NOT_EXECUTED_DEPENDENCY`; independent compiled outputs still execute and publish real Arrow
results. Structural execution-catalog inconsistencies remain fatal. Rust publishes typed per-block
execution outcomes and bounded issues in original request order, separately from processing
coverage. A completed block can still have incomplete semantic knowledge.

The sealed manifest validates successful block/relation correspondence and failed-dependency
identities. Fresh registration and exact retained-package reissue derive identical outcomes from
that manifest. The generated Protobuf response uses a presence-bearing message; older responses
without outcomes remain distinguishable from reported outcomes. Strict Pydantic models reject
inconsistent states, empty/duplicate identities and malformed dependencies. Reconnect identity
includes block outcomes while excluding reissued resource authority.

Five focused compiler cases and 35 compiler/package/registry regression cases pass. The installed
five-block branch/reopen scenario passes in 76.55 s (`/tmp/codefabric-p03-branches-native-4.log`),
and the preceding Python/Rust ordering regression passes in 150.49 s. All 120 adapter cases, adapter lint/types,
Protobuf generation and compatibility, 218 tooling cases, default/featureless root checks and
affected Clippy pass. Validation logs use `/tmp/codefabric-p03-branches-*`.
The native fixture first requested an invalid
fact-to-entity role, then exposed the still-unimplemented FindEntities prior-result `within` slot;
the supported entity-to-facts dependency is the acceptance path for this slice. The native comparison
also caught null-versus-absent related IDs; Rust now omits unset related IDs consistently with the
generated wire and public adapter. `query-branches` selects the scenario. Navigation, affected
spelling and diff checks pass; full-suite/strict baseline lint closure is not claimed.

This is the initial compiler-failure vertical. The runtime continuation above handles native
computation failures; other early phrase/input
and authorization failures, prior FindEntities scopes and broader
first-four semantics remain P03 work. The subsequent continuation above adds all-failed request
result envelopes. P03 and subsequent packages remain open.

The semantic-ordering continuation after `b81ef8c9` adds admitted `return.order_by` meanings to
all first-four production programs. Native sort keys support ascending/descending order and retain
the program's remaining deterministic tie breakers. Unknown keys, duplicate aliases and keys
outside the selected schema fail explicitly. Typed return actions are part of the release identity,
compiler dependencies and block manifest; they remain inside composed producer plans.

Entity discovery now carries a deterministic exact captured source anchor and defaults to source
path/position, kind, name and identity/context ordering. Native DataFusion `DISTINCT ON` selects
one intact anchor per entity/context/file/workspace; it cannot combine unrelated minimum values
into a fabricated span. All source/scope filters run below the final sort and limit, preserving
native sort/top-k planning. Historical schemas advertise only their available ordering keys.

The twelve-block installed-client Python/Rust scenario passes all four forms, ordering before
location-scoped truncation, prior reuse, unavailable/duplicate keys and exact reopen in 147.43 s
(`/tmp/codefabric-p03-ordering-native-2.log`). The independent native whole-span/context test and
compiler action/schema test also pass. The first run hit the old 120-second test bound; the new
case now uses the same finite five-minute override as other expanded public cases. Fixture Rust
name expectations retain their existing qualified-name representation. The initial 22 canonical/
recipe cases pass in 4.17 s (`/tmp/codefabric-p03-ordering-units-1.log`). Default/featureless root
checks pass (`/tmp/codefabric-p03-ordering-root-check.log`); final Clippy has no new-file/changed-line
findings (`/tmp/codefabric-p03-ordering-final-clippy.jsonl`). All 218 tooling cases and affected
Python lint pass. `semantic-ordering` selects the native scenario. All 28 final regression cases
pass in 86.86 s (`/tmp/codefabric-p03-ordering-regression.log`), including installed-client syntax
(85.48 s) and source locations (86.86 s) with exact reopen. Documentation/navigation, affected
spelling and diff checks pass. Source preparation in this fixture
spent 23.97 s executing/writing 84 relations; this is an observation, not a comparative performance
claim. Other return projections/groups/deduplication, broader first-four meanings and independent
branch failure remain open.

The source-location continuation after `8c0d9494` now accepts typed captured-file points and
half-open ranges in all first-four forms. Original byte offsets and one-based line/zero-based byte
columns remain distinct inputs; CRLF, lone CR, Unicode and zero-width syntax nodes retain their
captured coordinates. Optional controlled meanings select declaration/call/reference/import/module
or syntax candidates without collapsing ambiguity. Unknown meanings and mixed coordinate bases
fail explicitly. Source-location facts do not require a source-disclosure grant.

Native DataFusion predicates and identity/context semi-joins share the existing named-subject
selection path. A captured Arrow line-index relation supplies coordinates to exact source-backed
canonical entity locations; the location relation contains metadata, not source text. FindEntities
applies its location scope before limits and probes, and Find/Source processing intersects the
addressed captured files with authorized source boundaries. Other dependency scopes remain
conservative. Historical epochs without the location relation do not advertise the capability.

The twelve-block installed-client Python/Rust scenario passes initial publication and exact reopen
in 73.99 s, including byte/line subjects, all four forms, range selection before truncation and empty
Rust syntax. A separate metadata-only scenario passes in 60.44 s
(`/tmp/codefabric-p03-locations-native-3.log`). The final explicit metadata-only policy assertion
also passes in 57.77 s (`/tmp/codefabric-p03-locations-metadata-final.log`). This slice is committed
in `b81ef8c9`. The first added Find-within case exposed physical
column names differing from released semantic field IDs; selection now uses the released IDs.
The existing ten-block syntax/named/prior/source scenario passes in 69.86 s
(`/tmp/codefabric-p03-locations-named-regression.log`). Fifty-five affected Rust cases pass in
4.19 s (`/tmp/codefabric-p03-locations-final-units.log`). Default/featureless root checks pass
(`/tmp/codefabric-p03-locations-root-check.log`), as do all 218 tooling cases and affected Python
lint. Final all-target Clippy has no new-file/changed-line findings
(`/tmp/codefabric-p03-locations-final-clippy-2.jsonl`). Documentation/navigation and affected
spelling/diff checks pass. Whole-file spelling still flags the pre-existing truncated UTF-8
test bytes in `daemon.rs`; that unrelated fixture is preserved. Product selectors
`source-locations` and `source-location-metadata` name the public scenarios. These checks use the
existing delegated user-systemd scope and installed provider binaries. Full source outlines,
configured context defaults, directives, precise dependencies and independent branch failure
remain P03 work; no package, outcome, full-suite or doctest closure is claimed.

The syntax continuation after `2f7dc7cb` exposes the complete admitted Python/Rust CST as
canonical source-context entities. Native DataFusion grouping, Arrow structs/lists and an immutable
bounded identity fold replace provider-local node numbers with application-owned parent/sibling
identities. Raw kinds, fields, spans, anonymous/trivia/recovery flags and provider provenance remain
available. Exact captured digest/generation joins reject stale syntax. No Cargo target is needed
for Rust syntax. Requested file partitions govern syntax coverage; a missing canonical root cannot
turn a completed native run into a complete canonical census.

All first-four forms now accept syntax nodes: FindEntities, `syntax node properties`, incoming/
outgoing `syntax parents`, and exact/surrounding source. Typed entity priors and quoted raw kinds
are reusable subjects. Source context mode selects the reserved source identity; syntax/semantic
representation selection keeps those layers distinct. Existing `explicit` context selection remains
a compatibility spelling of `selected`. New retained schema profiles advertise the added syntax
capability; historical profiles do not gain meanings they cannot serve. Broader configured-default
contexts, source outlines, directives and independent branch failure remain open. Typed source
locations are delivered by the continuation above.

The ten-block installed-client Python/Rust scenario passes initial publication and exact reopen
in 67.73 s. It reads every advertised Arrow page, checks all nodes and parent edges, CRLF/Unicode
source bytes, anonymous/extra/recovery nodes and complete syntax scope without a Cargo target.
The failed-Rust-target diagnostic/source regression also passes in 87.52 s
(`/tmp/codefabric-p03-syntax-native-5.log`). The `syntax-nodes` product selector names the new case.
All 36 focused Rust checks pass (`/tmp/codefabric-p03-syntax-final-units-2.log`), including invalid
parents, duplicate/zero-width siblings, provider renumbering, stale generation, retained profile
capability and Python/Rust boundary continuation. The public run exposed and corrected an inner
parent-join nullability declaration; subsequent fixture corrections covered envelope freshness,
multiple Arrow pages and hexadecimal case. All 218 tooling cases pass in 3.00 s.
Default/featureless root checks pass (`/tmp/codefabric-p03-syntax-root-check.log`). Final all-target
Clippy has no new-file/changed-line findings (`/tmp/codefabric-p03-syntax-final-clippy-2.jsonl`);
existing warnings remain. The extracted native assertion helper also passes its focused case.
Affected Python lint, documentation/navigation, spelling and diff checks pass. Native validation
uses the same delegated Linux user-systemd scope and installed provider binaries recorded above.
No package, outcome, full-suite or doctest closure is claimed.

The source-boundary continuation after `2a90b5eb` accepts the closed
`{"kind":"path","root":"selected"}` descriptor over captured workspace-relative paths.
Native DataFusion binary range predicates preserve literal path components and non-UTF8
children; balanced predicates respect the existing 256-boundary request limit. An exact file-ID
semi-join narrows file-anchored outputs before result limits/probes. The manifest records resolved
boundaries. Invalid traversal/unknown descriptors and unanchored result families fail explicitly;
no filesystem lookup or source registration occurs. Python processing uses the same captured path
selection, including retained continuation; Rust target/owner scope remains conservatively broad
where file dependency ownership is unavailable. Dotted Python declaration names now reject missing
qualification semantics, while checker-qualified Python module names remain supported.

The eight-block mixed Python/Rust installed-client scenario passes initial publication and exact
reopen in 111.41 s (`/tmp/codefabric-p03-boundaries-native-3.log`). It checks the first four forms,
literal source bytes, pre-limit selection, truncation, complete empty scope, excluded incomplete
Python files and non-retryable invalid-path errors. Native binary-path and retained 101-partition
remainder/page comparisons also pass. Run 1 found an incorrect fixture expectation for Pyrefly's
qualified call-target name; run 2 found a fixture closure type mismatch. Both are corrected.
`source-boundaries` selects this scenario in the existing product harness.

All 218 tooling cases and affected Python lint pass (`/tmp/codefabric-p03-boundaries-tooling-2.log`).
The initial broader tooling run exposed an existing false positive for the daemon's native
`tracing_subscriber::registry()` diagnostic sink; that precise library call is now distinguished
from semantic registries. The harness failure test uses explicit cases rather than duplicating
its growing catalog. Default/featureless root checks pass
(`/tmp/codefabric-p03-boundaries-root-check.log`). All 29 final affected Rust cases pass in 0.34 s
(`/tmp/codefabric-p03-boundaries-final-units.log`). Final all-target Clippy has no new-file or
changed-line findings (`/tmp/codefabric-p03-boundaries-final-clippy.jsonl`); existing warnings remain.
Documentation/navigation, spelling and diff checks pass. No full-suite or doctest closure is claimed.
Full source/syntax/context scopes, broader references, directives and independent block failure
remain P03 work. No package or outcome exit is claimed.

The named-subject continuation passes installed-client validation after `ff89d115`. Facts,
relationships and source accept supported backtick-quoted declaration/module names, including
structured `semantic_reference` objects. The compiler selects canonical entities by kind and
literal name, then uses native identity/context semi-joins and distinct unions with explicit/prior
subjects. Existing entity-subject inputs are filtered directly. Absent named input compiles to
false; ambiguous namespace candidates remain separate. Source processing retains the named
language/family, and manifests record the resolved subject predicate.

The expanded 18-block positive/negative installed-client and exact-reopen scenario passes in
109.83 s; canonical fact-family retrieval passes in 113.87 s (`/tmp/codefabric-p03-named-native-7.log`,
sequential execution). Existing prior-entity reuse (58.09 s), relationships/occurrence source
(145.90 s) and the eight-form authorized-child scenario also pass
(`/tmp/codefabric-p03-named-native-6.log`). That parallel run exposed a fixture expectation for the
Rust exact span (its provider span is the signature) and hit the fact-family runner's 120-second
bound. The corrected authored-span expectation passes; fact-family validation now has the same
finite five-minute runner bound as the other expanded public-query scenarios.

Initial runs 1/2 exposed duplicate selector declarations in diagnostic-fact programs; the new path
now reuses them. Runs 3/4 then found schema-level metadata drift after native empty-union branch
elimination; all output fields matched. A focused native reproduction also caught physical
projection pushdown discarding an identity metadata projection. The child retains the compiler's
logical metadata envelope and applies native `ProjectionExec::try_new_with_schema_metadata` after
physical optimization. Field metadata/types/nullability and final schemas remain exact; buffers
remain native Arrow. The reproduction passes (`/tmp/codefabric-p03-named-metadata-unit-3.log`).
Compact recipe/port failure diagnostics now expose activation composition causes in daemon logs.

All 43 affected cache/child-authority/recipe/ingress/source-scope cases pass (0.40 s,
`/tmp/codefabric-p03-named-final-units.log`). Default/featureless root checks pass
(`/tmp/codefabric-p03-named-final-root-check.log`). Affected Clippy has no new-file/changed-line
findings (`/tmp/codefabric-p03-named-final-clippy.jsonl`); the existing backlog remains. Documentation,
spelling and diff checks pass. Broad reference and source-location meanings, directives, source/
syntax scope and independent branch failure remain P03 work; no package exit is claimed.

The literal/filter continuation now separates quoted identifiers from entity-kind phrases. For
example, a Rust function request with a backtick-quoted `target` becomes a declaration meaning and a
literal name predicate; all namespace-qualified matches remain candidates. Native DataFusion
`LIKE` with an escaped suffix handles retained Rust qualification, without rewriting canonical
names or accepting public patterns. Explicit qualified-name predicates remain exact. Text `where`
objects use `property`, `operator` (`equals` / `does not equal`) and literal `value`; each compiled
program carries its own allowed semantic property-to-field map. Unknown fields/operators and
malformed quoted identifiers are rejected. Code identifiers that resemble evaluative terms stay
literal operands; actual judgment requests remain rejected. Entity results also expose existing
qualified-name evidence, and each manifest block records its resolved selections/predicates.

The ten-block installed-client/reopen case passes in 107.44 s, alongside native percent/underscore/
backslash/Unicode suffix and intent checks (`/tmp/codefabric-p03-literals-native-2.log`). The first
native run passed 20 units but used `resolved` instead of the call family's actual
`resolved_declaration` filter value (`/tmp/codefabric-p03-literals-native-1.log`). All 37 affected
compiler/recipe/ingress and native regressions pass in 108.72 s
(`/tmp/codefabric-p03-literals-regression.log`), including manifest resolved meanings (108.70 s)
and prior-entity reuse with the extended schema (54.32 s). The initial 26 canonical/recipe/parser
cases passed (`/tmp/codefabric-p03-literals-units-1.log`). A final availability guard prevents
nullable, unimplemented Python declaration qualification from yielding a false complete-empty
answer. The final five-case run passes in 107.31 s, including installed-client positive/negative
queries and exact reopen (`/tmp/codefabric-p03-literals-final-native-3.log`). Earlier final runs
corrected an immediate-error test assumption and exposed terminal validation failures being
collapsed to `INTERNAL`; the daemon now preserves the existing non-retryable `VALIDATION_REJECTED`
wire classification. `literal-identifiers` resolves to exactly one native case.
Default/featureless root checks and focused tooling tests/lint pass. Final affected Clippy
(`/tmp/codefabric-p03-literals-final-clippy-2.jsonl`) reports no new-file/changed-line findings;
the existing repository backlog remains. Documentation/link, spelling and diff checks pass.
Broader reference subjects, source/syntax meanings, scope/projection/directive behavior and independent branch failure remain P03 work.

The source continuation extends exact captured source descriptors to call/reference/import
occurrences, lexical references and Python modules. Native exact-pin joins and span/provenance
checks exclude invalid mappings. Candidate multiplicity is deduplicated within a provider family;
lexical and semantic witnesses remain distinct. Resolved subject families now filter both source
rows and processing, and mixed-family remainders expose an optional `fact_family` through Rust,
protobuf and Pydantic. Retained single-family selections and absent historical wire fields still
read. A distinct source role prevents older retained profiles from claiming occurrence/module
source support. Existing source byte bounds and independent disclosure authorization still apply.

The 70-block installed-client scenario passes initial publication and exact reopen in 120.41 s
(`/tmp/codefabric-p03-occurrence-source-native-4.log`). It covers both languages' occurrence source,
Python module extents, mixed-family line windows, authored bytes/coordinates, prior entity subjects
and earlier relationship cases. The first run lacked required line-window bounds; the second
exposed lexical witnesses entering semantic-reference source rows; the third hit the runner's
120-second bound. These are retained in the corresponding `native-1/2/3.log` files. The native
family filter fixes the witness mismatch; this expanded test now has a finite five-minute bound
with the existing one-minute slow signal. All 16 canonical regressions passed in run 2, and the
independent invalid-pin/range/provenance/deduplication case passes (0.38 s in run 3).

All 37 affected recipe/processing/package unit cases pass
(`/tmp/codefabric-p03-source-unit-1.log`). The source regression run passed 34 of 35 cases,
including repeated blocks (58.72 s) and prior-result reuse/reopen (58.43 s), but found a retained
source revocation bug: the read check still recognized the retired form-wide output name.
It now follows sealed exact-source input provenance, retaining the historical name fallback;
all 29 registry/package and installed disclosure cases now pass (107.77 s;
`/tmp/codefabric-p03-source-disclosure.log`). The source case verifies denied access, authorized
Unicode/bounded bytes, live revocation of a retained page, exact reopen and revocation again.
Default/featureless `just root-check` passes (`/tmp/codefabric-p03-source-root-check.log`).
`just proto-check` passes, including 28 compatibility/generator/wire cases
(`/tmp/codefabric-p03-source-proto-check.log`). All 109 adapter tests, adapter lint/types and
37 Rust units pass. Final all-target Clippy completes with the existing backlog and no diagnostics
on new modules or changed lines (`/tmp/codefabric-p03-source-clippy-final.jsonl`). Document
navigation, focused spelling and diff checks pass. The final four scope/retained-manifest cases
pass after cleanup (`/tmp/codefabric-p03-source-final-units.log`). Source processing
still uses conservative file/context scope when exact owner dependencies are unavailable.

The occurrence continuation adds calls, semantic references and imports to the existing canonical
entity universe. Candidate/namespace multiplicity stays in the fact relations. Calls have an
occurrence label independent of resolved target names. Six Python/Rust FindEntities meanings
return reusable public IDs and use their actual family coverage. The `call targets` relationship
meaning selects first-class call subjects through the same native target-kind templates. An
explicit occurrence-capable selector role prevents older retained entity profiles from admitting
meanings whose census they do not contain.

The extended installed-client/reopen case passes in 107.67 s
(`/tmp/codefabric-p03-occurrences-native-2.log`): both languages find all three occurrence kinds,
then use the typed results for outgoing traversal. It compares the occurrence ID sets and every
canonical witness column, while retaining the preceding guarded/bidirectional/unknown/truncation
cases. The retained-profile test passes (0.31 s;
`/tmp/codefabric-p03-occurrences-profile-test.log`). All 24 affected canonical/recipe/processing
and public regressions pass in 104.39 s (`/tmp/codefabric-p03-occurrences-regression.log`), including
all eleven fact families (104.38 s) and repeated first-four blocks/source reads (47.72 s).
Default/featureless `just root-check` passes (`/tmp/codefabric-p03-occurrences-root-check.log`).
The first native run's malformed test envelope placed `languages` outside `scope`; the service
correctly rejected it (`/tmp/codefabric-p03-occurrences-tests-1.log`, with 11 unit cases passing).
The corrected fixture uses the released scoped envelope. All-target Clippy completes with the
existing backlog and no diagnostics on new modules/changed lines
(`/tmp/codefabric-p03-occurrences-clippy-final.jsonl`). Document navigation, focused spelling
and diff checks pass.

The source continuation above extends this occurrence slice. Full source/syntax meanings,
remaining P03 scope/directive/block work and P04–P14 remain open.

The preceding reference/import continuation adds four immutable native query templates over the existing
canonical relations, covering incoming denotations and outgoing source occurrences. Exact
workspace/context joins establish target kinds; unresolved targets retain null public IDs and
provider explanations. A shared Arrow scalar formats application-owned binary IDs. Typed
projections carry their explicit output labels, nullability and kind evidence in program identity.
Target-kind grouping prevents name aliases from multiplying occurrence witnesses. Existing typed
entity-result slots also feed incoming references/imports. Processing uses the selected family and
conservative potential dependency scope; owner/frontier narrowing remains open.

Guarded selection resolves family and direction together, deduplicates aliases by execution
meaning and preserves valid directions across replay. Labels remain readable while submitted
choices stay opaque and catalog-bound. A native thirteen-block installed-client case checks both
languages/directions, aliases, repeated subjects, unresolved imports, empty results, truncation,
prior-entity fan-out and guarded import selection, then checks exact reopen. All 56 affected
compiler/ingress/runtime/ownership/cache and native cases pass (93.41 s;
`/tmp/codefabric-p03-relationships-regression.log`): the new case takes 88.93 s, the eleven-family
regression 93.38 s and the prior-entity regression 31.73 s. The earlier eleven-block native case
passes in 69.96 s alongside all nine ingress tests
(`/tmp/codefabric-p03-relationships-tests-2.log`).

The first run passed 21 compiler cases but exposed a private capability mismatch: DataFusion's
`ScalarUDF::call` allocates another outer Arc. The query compiler now constructs the native
`ScalarFunction` with the exact registered Arc, preserving existing strict child authorization.
The failed run is retained at `/tmp/codefabric-p03-relationships-tests-1.log`. Resolved DataFusion
55 `ScalarUDF::call`/`ScalarFunction::new_udf` source, native projection/aggregate/join APIs, and
Pyrefly reference §26 informed the implementation. Default/featureless `just root-check` passes
(`/tmp/codefabric-p03-relationships-root-check.log`). Focused tooling lint, golden harness,
document navigation and diff checks pass. Golden names for the recent P02/P03 submodule cases
now include their actual Rust module paths; `semantic-relationships` selects the new case.
All-target Clippy completes with the existing backlog; the new import/style diagnostics were
corrected (`/tmp/codefabric-p03-relationships-clippy-final.jsonl`). Native nextest discovery finds
exactly the expected test for all eight corrected/new selectors
(`/tmp/codefabric-p03-relationships-test-list.json`).

P03 is still open: complete literal/scope/representation meanings, first-class occurrence and
source subjects, type/member facts, bounded distance/stop/filter directives and independent
branch execution/failure remain. No outcome 4–8 closure is claimed.

The preceding continuation adds explicit materialized consumer slots and a shared-pool Arrow result
owner. Completed producer rows retain their exact schema and are reused across consumers; native
semi joins preserve entity/context pairs. Explicit subject rows and prior dependencies are separate.
Result-limit probes are excluded from downstream inputs, while the producer's published observation
retains truncation. The canonical manifest records dependency edges, and consumer provenance names
the actual producer block relations. Source-level schema metadata is replaced by the transient
input's declared envelope; column types, nullability, names and semantic metadata match exactly.
Arrow buffers and their shared reservation stay owned through consumer and publication streams.

The nine-block installed-client case passes initial execution and exact reopen in 30.12 s
(`/tmp/codefabric-p03-prior-native-6.log`). It checks a limited producer reused for facts/calls/source,
two producer sets feeding one consumer, repeated subjects, typed empty inputs, and a block ID equal
to a real entity ID that must never become a scalar entity subject. `prior-entities` selects it.
All 51 compiler/ingress/runtime/ownership/public-family regression cases pass (75.25 s;
`/tmp/codefabric-p03-prior-regression.log`), including the updated native case in 31.90 s and all
eleven canonical families after reopen. Ownership cases verify one source poll across consumers,
probe exclusion, unchanged field semantics, and reservation release on completion/row-limit/
memory/cancellation/deadline failures. Default/featureless `just root-check` passes
(`/tmp/codefabric-p03-prior-root-check.log`). All-target Clippy completes with the existing backlog
and no diagnostics in the new modules/native tests (`/tmp/codefabric-p03-prior-clippy-final.jsonl`).
The cleanup uses narrower native error types and retains an annotated coherent template rewrite.
The final 34-case compiler/ingress/runtime/ownership run passes after that cleanup
(`/tmp/codefabric-p03-prior-final-unit-tests.log`).
Golden selection, focused Python lint, documentation navigation, spelling and diff checks pass.

Native integration exposed and corrected rejection of repeated producer references, UNION's loss
of qualifiers before source joins, and inherited provider-level schema metadata at the transient
input boundary. The existing independent repeated-block case passes after the UNION correction
(31.43 s; `/tmp/codefabric-p03-prior-tests-4.log`). Query rejection stages now reach the existing
supervisor-owned warning sink with private diagnostic detail; the public error projection remains
closed. Earlier failed runs are retained in `/tmp/codefabric-p03-prior-tests-2.log`,
`/tmp/codefabric-p03-prior-tests-3.log`, `/tmp/codefabric-p03-prior-tests-4.log` and
`/tmp/codefabric-p03-prior-native-5.log`; an initial test compile needed a longer-lived snapshot binding.

This is the first typed entity-result vertical. Independent branch scheduling/failure, occurrence
roles, full scopes/meanings and the broader P03 completion remain open. Eager legacy compiler
fixtures retain their previous composition mode; modern production uses owned streaming execution.
DataFusion planning §52/§54.8, resolved native UNION/alias/MemTable/MemoryReservation APIs, and Arrow
59 RecordBatch schema/array sharing informed this implementation. No outcome 4–8 closure is claimed.

Repeated forms now receive output relation/field bindings derived from the exact request,
program catalog, query block and template schema authority. The typed field rewrite preserves
canonical epoch inputs, source-disclosure parameters, row predicates, sorting and limits. Existing
request-owned Arrow relations remain isolated per output execution. The canonical response manifest
maps query IDs to their distinct output relations, so sorted resource pages can be associated with
the correct block. Legacy direct epoch outputs retain their existing binding path.

`just root-check-fast` and default/featureless all-target `just root-check` pass
(`/tmp/codefabric-p03-block-output-check.log`, `/tmp/codefabric-p03-block-output-root-check.log`).
The native installed-client case with two independent blocks of each first-four form passes
in 30.36 s (`/tmp/codefabric-p03-block-output-native-2.log`); it checks distinct subjects, an empty
caller, source text, page association and exact reopen. The first run correctly denied source
access; the fixture now explicitly grants its private workspace's disclosure policy. All 19
runtime/compiler/public-family regression cases pass (93.08 s;
`/tmp/codefabric-p03-block-output-regression.log`). `repeated-first-four` selects the new native case.
All-target root Clippy completes with its existing backlog and no diagnostics in the new modules
or native test (`/tmp/codefabric-p03-block-output-clippy-final.jsonl`). Golden selection, focused
Python lint, documentation navigation, spelling and diff checks pass. Full-suite and doctest
completion are not claimed.
The typed entity-result vertical above extends this initial output-isolation slice. Broader prior
roles, independent branch scheduling/failure and remaining first-four meanings remain open.
The rest of P03–P14 remains in package order.

## P02: initial canonical semantic vertical delivered

The Python continuation publishes `fact.code_module`, `fact.code_semantic_reference` and
`fact.code_import`. Checker-selected modules are application-owned semantic entities without
invented declaration spans. The native Pyrefly query seam resolves names, attributes and import
aliases in one transaction and AST walk, retaining every definition candidate and explicit
unresolved observations. A semantic import mode disables the editor's unresolved-import landing
fallback. Module targets preserve native module identity even when their definition range is empty.
The sidecar maps source/definition anchors to original captured bytes and emits the typed
`provider.pyrefly.reference.v1` Arrow relation (family 143). Both build domains use the updated
schema bundle; old sidecars fail its existing exact digest handshake.

Native DataFusion joins require matching analysis context, source file, digest and generation.
Declaration targets require exact anchors; module targets require exact captured module membership.
Distinct canonical matches remain candidates, and stale targets retain unknown denotations.
Import syntax joins checker names only at the same alias start within the Ruff alias range, with
the join method recorded. Raw syntax names never establish semantic resolution. Entity union
branches carry common output field metadata before optimization, including an empty module branch.

`modules`, `semantic-references` and `imports` now have requested processing partitions. An accepted
native per-file/family census refines aggregate provider remainder, and canonical target gaps can
only downgrade completion. Imports also require their syntax family. An unresolved import in one
file does not make another file's complete imports incomplete. Source-only epochs disclose pending
semantic work. Builtins/external targets outside the captured inventory and unsupported semantic
anchors remain unknown; the broader reference census and external normalization are still required.

Validation on 2026-09-10: all 38 sidecar tests pass, including aliased imports, attributes, module
endpoints and absent imports (`/tmp/codefabric-p02-references-sidecar-tests-3.log`). Six canonical
tests and a real daemon/reopen case pass together (23.29 s;
`/tmp/codefabric-p02-references-tests-3.log`). Independent Arrow inputs cover candidate multiplicity,
another analysis context, stale source/target digests, stale generation, invalid ranges and stable
IDs across run/generation changes. The expanded real case additionally checks a separate broken
import file, healthy-file complete coverage and exact reopen of all new relations/coverage
(23.45 s; `/tmp/codefabric-p02-references-native-negative.log`).
`canonical-python-references` selects it through `just golden`. Native tests use the delegated
user-systemd scope described under P01, with the rebuilt current sidecar and existing extractor.
Fourteen provider-recipe/processing tests pass, including the expanded exact schema census
(`/tmp/codefabric-p02-references-provider-tests-final.log`). Tooling golden selection, focused
lint/format, documentation navigation, spelling and diff checks pass.
Default/featureless `just root-check` and strict `just sidecar-check` pass. All-target root Clippy
completes with the existing warning backlog; full-suite, doctest and public family-selection
completion are not claimed by this slice. The code-facts and DataFusion reference skills and exact
local sources guided the bulk resolver, typed Arrow boundary, joins, aggregates and alias metadata.

The public family selection required at this cluster checkpoint is delivered below.
P03–P14 remain required in package order.

### Public canonical family selection

Family-specific immutable query templates now consume the canonical module/import/reference,
type/observation/component and diagnostic hierarchy relations directly. Native DataFusion semi
joins select exact workspace/context and explicitly named entity/file scopes without multiplying
facts for repeated subjects. All canonical fields retain their schema lineage; no new persisted
wide selector relation or serialized fact payload is introduced.

The production catalog is indexed by program identity within each form. An explicit program-selection
binding chooses the typed plan without inventing a fact column. Catalog-bound guarded choices span
installed family meanings and retain opaque submitted IDs with readable labels. Python module
selection joins the existing canonical entity universe. Query and retained processing selections
use the actual selected family; compiler diagnostic children have their own requested census, and
structured detail rows carry language for scope filtering. Missing Python structured diagnostics
remain unsupported. Older retained detail schemas lacking the new public scope field are not
advertised as executable templates.

The available meanings state their scope: module metadata; imports and type observations/components
in declaring files; semantic references to entities; structural types and diagnostics in analysis
contexts. These do not assert Python declaration-owned type roles or diagnostic ownership by an
arbitrary function. Narrower ownership, broad mixed-family requests and full block-local composition
continue in P03/P06. A request spanning different family schemas uses separate blocks; P03's
initial output isolation preserves their distinct bindings and resource association.

On 2026-09-10 the 16 focused dispatcher/compiler/processing checks pass
(`/tmp/codefabric-p02-family-programs-tests.log`). The real mixed native installed-client case passes
all eleven family queries, guarded import selection, module discovery, typed field comparison and
exact Delta reopen in 74.25 s (`/tmp/codefabric-p02-family-programs-native-2.log`). The initial run's
guard fixture omitted its answer; the corrected fixture consumes the readable authorized choice.
The final 47-case canonical/compiler/ingress/processing regression run passes, including the
preceding public Python declaration scenario (27.72 s;
`/tmp/codefabric-p02-family-regression-tests.log`). `canonical-fact-families` selects the new case.

`just root-check` passes default and featureless all-target checks
(`/tmp/codefabric-p02-family-root-check.log`). Root Clippy completes with the existing warning
backlog (`/tmp/codefabric-p02-family-clippy.jsonl`); the new wildcard imports, guard/test length
annotations and needless clone were addressed, followed by the passing regression compile/run.
Golden harness selection, focused Python lint, documentation navigation, spelling and diff checks
pass. No full-suite, doctest, first-four completion or outcome 4–8 closure is claimed.
DataFusion planning §52 (join planning), exact DataFusion 55 builder sources, FastMCP 4 §7.5–7.6
(binary resources) and §38 (guarded replay), and Pydantic §21 (reused typed validation) informed this
path. The existing adapter owns presentation and binary resource delivery; Rust owns family meaning,
execution and coverage.

### Initial Rust imports and semantic references

One native HIR traversal visits every item-like and its bodies, exporting paths, type-relative
associated paths, method references and each import namespace. It uses resolved HIR `Res` and
type-checker dependent definitions rather than rendered names to select targets. Typed Arrow
relations `provider.rustc.hir_reference.v1` and `provider.rustc.hir_import.v1` carry source anchors,
stable compiler definition keys, raw/normalized target kinds, local-owner/index provenance,
aliases, glob/public flags and namespace distinctions. Lowering-only list stems do not invent
imports; compiler-injected prelude imports retain their generated provenance. The shared bundle
and current extractor were rebuilt; the provider recipe census now has 72 relations.

Canonical references require exact accepted run/context/generation and captured compilation-owner
bytes. Each HIR location independently binds its actual captured file/digest/range; unmapped,
generated or invalid locations keep unknowns and null canonical coordinates. Target joins also
require current captured declaration bytes and context. Multiple canonical declarations remain
candidates. Import resolution joins native reference ordinals within the same run, compilation
unit and owner. Syntax-only provider/observation fields remain null for the compiler branch.
Namespace alternatives retain separate rows sharing their source import occurrence. Field metadata
is aligned before native unions through a common helper shared with the type relations.

Requested `imports` and `semantic-references` processing partitions are Cargo-target/context scoped.
Canonical gaps downgrade completion. Native anti joins also detect accepted observations excluded
by source validation; null target ordinals are compared explicitly. Unrepresented local bindings,
primitive/self types, full module/member/external normalization, glob expansion, macro/hygiene
correspondence and the broader reference census remain P06. Positive resolved imports/methods
remain usable beside those unknowns. Rust lexical references retain their separate unsupported scope.

Validation on 2026-09-10: all 22 extractor tests pass, including real alias resolution, distinct
type/value namespace targets, local binding provenance, method byte positions, generated imports
and distinct source ranges for nested imports (`/tmp/codefabric-p02-rust-references-native-tests-3.log`).
Strict extractor check/Clippy passes (`/tmp/codefabric-p02-rust-references-extractor-check-final.log`).
The real mixed daemon case verifies canonical function/method targets, aliases/public imports,
namespace rows, unknown coverage and exact Delta reopen (62.67 s), alongside the existing Python
reference scenario (26.25 s; `/tmp/codefabric-p02-rust-references-integration-tests.log`).
The expanded 26-case canonical/provider/processing/native regression run passes (64.58 s;
`/tmp/codefabric-p02-rust-references-final-tests.log`). Independent Arrow cases cover stale owners,
locations and target digests, generations, candidate multiplicity, context/compilation-unit isolation,
null coordinates and identity stability. The final four Arrow/type regression cases also pass,
including the anti join's missing-owner and null-target behavior
(`/tmp/codefabric-p02-rust-references-final-arrow-tests.log`). `canonical-rust-references` selects
the native scenario. Root default/featureless checks pass
(`/tmp/codefabric-p02-rust-references-root-check-final.log`). Root Clippy completes with the
existing warning backlog (`/tmp/codefabric-p02-rust-references-clippy-final.jsonl`); new slice
warnings were addressed. Full-suite, doctest and all-family public-query completion are not claimed. The code-facts and DataFusion
references and exact local HIR/type-checker/source-map/plan APIs guided the implementation.

### Initial Python structural types

The native Pyrefly query now returns both the existing presentation type table and a bounded typed
graph from one checker transaction and AST occurrence traversal. The graph preserves native
constructors, captured class/function anchors, builtin identities, literal scalar values, tuple
layout, and callable parameter kinds, call-significant names and requiredness. It retains every
native type discriminant, including explicitly unsupported shapes. Local graph indices and native
hashes are response-local; neither display strings nor provider hashes establish canonical type IDs.

Three shared Arrow relations carry type nodes, components and source observations. Byte literals
use an explicitly identified scalar Binary field; opaque semantic carriers remain rejected. The
shared schema bundle and its logical-type metadata changed, and the current sidecar was rebuilt.
DataFusion binds graphs to exact accepted runs, source digests/generations and unique canonical
definition anchors, then groups their typed records for normalization. An owned graph normalizer
uses petgraph's iterative SCC traversal and the application TypeInterner; it encodes each term once.

`fact.code_type`, `fact.code_type_observation` and `fact.code_type_component` publish structural
identity, occurrence roles and native component evidence. The internal canonical graph preserves
local correspondence and normalization gaps. Primitive/nominal/generic, class/type objects,
union/intersection, tuples, basic callables/functions, literals, Any, Error, Unknown, None and Never
are distinct. Unresolved/ambiguous definitions, recursion and unsupported advanced constructors
remain explicit unknowns; dependent shapes cannot silently become complete. Checker-computed
observations retain the `checker-observed` role, without inventing declared/expected/narrowed roles.
The `types` processing family combines native per-file coverage with canonical normalization and
location gaps, including pending source-only publication.

Validation on 2026-09-10: all 39 sidecar tests pass, including typed literals, parameter semantics,
native anchors and a deliberately bounded graph (`/tmp/codefabric-p02-types-sidecar-tests-3.log`).
Three graph-normalization tests and the existing interner adapter case pass. Independent Arrow plan
cases verify stale bytes/generations, context separation, ambiguous nominal anchors, missing nodes,
invalid locations and stable IDs across runs/generations. The real daemon case checks an independently
specified integer type key/ID, native constructor distinctions and callable parameters, unknown
coverage and exact Delta reopen of all type relations (25.07 s). The affected 14-case run passes,
including provider schema admission and retained opaque-carrier rejection
(`/tmp/codefabric-p02-types-integration-tests-2.log`). `canonical-python-types` selects this scenario.
The final 21-case canonical/processing/interner/native regression run passes (27.33 s), including
the preceding module/import/reference case and the type/reopen case with final field metadata
(`/tmp/codefabric-p02-types-final-tests.log`). Default/featureless root checks and strict sidecar
checks pass (`/tmp/codefabric-p02-types-root-check-final.log`,
`/tmp/codefabric-p02-types-sidecar-check-2.log`); all 39 sidecar tests also pass after the final schema
metadata change (`/tmp/codefabric-p02-types-sidecar-tests-final.log`). All-target root Clippy
completes with the existing warning backlog; its one new test allocation warning was corrected
(`/tmp/codefabric-p02-types-clippy-final.jsonl`). Tooling golden selection, focused lint/format,
document navigation and diff checks pass. Focused spelling passes; whole-file spelling still flags
six unchanged escaped byte fragments in existing encoding fixtures. Full-suite/doctest and
strict repository-wide lint completion are not claimed.
The code-facts, DataFusion and petgraph reference skills and exact resolved sources guided this slice.
Full recursive/binder/alias/overload/ParamSpec/TypedDict normalization, expanded type observation
roles and members remain P06; narrower public type ownership/directives remain P03/P06. No full type-universe or
public-query completion is claimed by this slice.

### Initial Rust structural types

The dated compiler emitter now carries primitive kinds, native definition kinds, array lengths,
generic-argument and binder counts, region classes, and function-pointer ABI, safety, variadic and
explicit unwind flags as typed Arrow fields. Rust ABI has no invented unwind flag. Native graph
keys use the compiler's stable type hashing without TypeId's region erasure; static and erased
reference observations cannot merge before normalization. Provider keys remain provenance, never
canonical type identity. Both build domains share the changed schema and exact bundle handshake.

DataFusion groups each exact accepted compiler owner/context/source graph for the shared iterative
SCC normalizer and application TypeInterner. `system.canonical_rust_type_graph` feeds the same
canonical type, observation and component relations as Python. Supported initial shapes include
primitives, never, tuples, arrays/slices, pointers/references, function pointers and canonically
resolved nominal/function definitions with type-only arguments. Array length, ABI, safety, explicit
unwind and region distinctions participate in structural identity. Erased regions retain their
identity and precision gap. Non-type generic arguments, binders, unresolved nominal definitions,
cycles and advanced shapes retain unknowns, including dependent types.

Compiler item observations and MIR local/argument/return type observations remain distinct. MIR
slot indices never mint source occurrence IDs. Native owner, compilation unit, type key and
component ordinal remain available beside canonical identities. Locations require exact captured
owner-file bytes and source-authored spans. Requested `types` coverage is target/context scoped;
missing canonical graphs and normalization/location gaps can only downgrade completion. Python
coverage remains per-file, and source-first epochs preserve pending semantic scope.

The real mixed-language daemon case passes independently specified primitive/array identities,
function-pointer distinctions, MIR argument/component provenance, explicit unknown coverage and
exact Delta reopen of all type relations (60.89 s;
`/tmp/codefabric-p02-rust-types-joins-tests.log`). Independent Arrow inputs in that same passing run
cover stale digests/generations, unaccepted runs, context separation, identical native keys in
different compiler owners, missing children and stable IDs after run/generation changes.
`canonical-rust-types` selects the native scenario. All 21 extractor tests pass, including actual
native static/erased region-key separation (`/tmp/codefabric-p02-rust-types-extractor-tests-3.log`);
strict extractor check/Clippy passes after the final native changes
(`/tmp/codefabric-p02-rust-types-extractor-check-verified.log`). All 29 affected canonical,
normalizer, processing, provider-recipe and real Python/Rust type/reopen cases pass with final
schemas and metadata (62.01 s; `/tmp/codefabric-p02-rust-types-final-tests.log`). Default/featureless
`just root-check` passes (`/tmp/codefabric-p02-rust-types-root-check-verified.log`). Tooling golden
selection, focused Ruff/format/spelling, documentation navigation and diff checks pass. All-target
root Clippy completes with the existing warning backlog and no warnings in the new normalization
modules (`/tmp/codefabric-p02-rust-types-clippy-verified.jsonl`); the new independent-availability
boolean warning has an explicit rationale. No full-suite/doctest or strict repository-wide lint
claim is made.

Validation exposed duplicate dependencies in the combined-language observation plan; those are
now deduplicated. It also exposed a preparation retry defect: identical source/context inputs reused
private Cargo output paths, obscuring the original publication error with `File exists`. Attempts
now use fresh private output names while semantic identity remains stable. The daemon initializes
the resolved tracing-subscriber 0.3.23 formatter on supervisor-owned stderr, making CodeFabric
warnings and dependency errors visible without a daemon log queue or retained log-file history. This addresses
the immediate E26 diagnostic sink gap; correlated metrics, finite provider-output retention and
integrated retry/recovery acceptance remain in their packages.

The code-facts, DataFusion, petgraph and Rust daemon references and exact local sources guided this
slice. Complete binders/regions, const generics, aliases, trait objects, nominal/member census and
additional type propositions remain P06/P08. Public family retrieval remains P02. No outcome is closed.

### Canonical diagnostics (`cf42d574`)

The daemon now publishes `fact.code_diagnostic` for accepted Python and Rust messages, plus
`fact.code_diagnostic_child`, `fact.code_diagnostic_span`, `fact.code_diagnostic_suggestion` and
`fact.code_diagnostic_edit` for Rust's native hierarchy. Raw provider relations remain available.
Native DataFusion joins bind messages to exact provider runs, compilation/module owners, captured
source digests and generations. Application CBEF diagnostic IDs exclude provider-run and generation
identities; identical content under the same effective context retains its ID.

Diagnostic ownership and source locations are separate. A Rust span or edit gains canonical source
coordinates only when its location file, digest, generation and byte bounds match captured source.
Unmapped/invalidated locations keep native paths/ranges and an explicit state; they never borrow the
compilation owner's location. Python's current checker API supplies rendered messages, so severity,
code, locations and suggestions are not parsed or invented from text. Structured availability and
provider authority stay visible.

Requested processing includes separate `diagnostic-messages`, `diagnostic-locations` and
`diagnostic-suggestions` partitions. Source-only epochs disclose pending messages. Python structured
details are explicitly unsupported. A failed Cargo target retains accepted positive diagnostic rows
and failed/incomplete target coverage; those rows cannot establish the absence of further diagnostics
or completion of declarations/MIR. The processing snapshot's closed family validator accepts these
new families on activation and reopen.

Validation on 2026-09-10 includes a real failed Rust target plus a Python type error, independently
expected `E0425` and source bytes 22–38, canonical/raw message equality, explicit coverage and exact
Delta reopen of all five diagnostic relations (46.95 s). Synthetic Arrow inputs independently cover
stale owner bytes/generations, a location in another file, invalidated and out-of-bounds locations,
unchanged IDs under different run/generation identities, and empty-provider installation. Use the
`canonical-diagnostics` product-golden selector for the real scenario. The final affected run passes
12 canonical/processing and real-provider cases, including partial Rust target failure (94.11 s
total; `/tmp/codefabric-p02-diagnostic-final-tests.log`). It uses `just root-test-incremental` with
selectors `production_workspace_startup::canonical::tests::`, `processing_status::tests::`,
`pragmatic_all_rust_targets_failed_retains_diagnostics_and_source` and
`pragmatic_rust_target_failure_retains_other_targets`, under the P01 delegated user-systemd test
scope and the existing sidecar/extractor binaries. The same native/reopen diagnostic case took
62.15 s during that parallel run; timings are not performance comparisons.

Tooling golden selection passes (1 case, 215 deselected); focused tooling lint/format, document
navigation, spelling and `git diff --check` pass. Default and featureless `just root-check` pass
(`/tmp/codefabric-p02-diagnostic-root-check.log`); all-target Clippy completes with the existing
repository warning backlog and no warnings in the new diagnostic modules or modified processing
paths (`/tmp/codefabric-p02-diagnostic-clippy-final.jsonl`). Strict repository lint is not claimed.
Initial checks caught and corrected empty-plan
dependency declarations, the closed processing-family validator and a test that incorrectly expected
complete coverage for a failed Cargo target. No full-suite, doctest or all-family public-query
completion is claimed.

The DataFusion and code-facts reference skills guided typed logical joins and native diagnostic
authority. P02's remaining scope and the subsequent Python cluster are recorded above.
No outcome is closed by these slices.

## P01: captured dependency contexts and initial phase costs

Contained Cargo now loads the captured ancestor `.cargo/config` or `.cargo/config.toml` files
explicitly, in native precedence order. Its writable working directory previously prevented native
configuration discovery through `--manifest-path` alone. Cargo's directory-source replacement now
resolves locked dependency material in the captured universe, preserving registry/git source
identity in the lock and native checksum handling. Target discovery excludes directory-source
packages from independent top-level target selection; their actual dependency units still run
through the compiler wrapper. A dependency's unselected tests do not become analysis targets.

Captured native Pyrefly `site-package-path`/`site_package_path` configuration selects authorized
dependency roots. Bounded, digest-verified `py.typed` markers enter the identity-bearing context
and the checker's private view. Dependency modules bind relative to their package root; explicit
workspace root ordering remains intact. The existing one-checker bulk API supplies real calls,
types and source anchors. No interpreter probing or import execution is added. Existing manifests
without markers retain their previous serialized shape; older sidecars reject unfamiliar context
fields rather than silently ignoring them. Unresolved dependency locks remain unavailable.

Each workspace retains only its latest `source-preparation-costs.json` and
`semantic-preparation-costs.json`, with bounded phase names, source file/byte counts and relation
version counts. These record preparation and relational execution/write costs, including ordinary
failure/drop state. They are operational diagnostics, separate from semantic coverage; abrupt
process death can leave the preceding report. They do not yet measure every compiler subphase,
query cost, CPU/RSS or retention cost required by P14/8E.

Validation on 2026-09-10: 24 affected root tests pass, including the real locked directory dependency
and exact reopen (53.18 s) and installed Python dependency declaration/call/source queries and reopen
(32.84 s in the preceding equivalent integrated sample). All 37 sidecar tests and strict sidecar
check/Clippy pass. Default and featureless root checks pass. The broader context selection caught
and resolved an overlapping-root precedence regression. Native tests ran in a transient delegated
user-systemd scope: the initial login shell lacked the required cgroup ancestry and correctly
reported containment unavailable. No containment checks were weakened.

Logs: `/tmp/codefabric-p01-integrated-tests.log`, `/tmp/codefabric-p01-sidecar-tests.log`,
`/tmp/codefabric-p01-sidecar-check.log`, `/tmp/codefabric-p01-root-check.log`.
After a behavior-preserving marker-helper extraction, all 14 Python context checks pass; all seven
target checks also pass after keeping invalid directory-source configuration in per-target
preparation scope. Final root check output is `/tmp/codefabric-p01-root-check-final.log`.
Affected Clippy inspection retains existing root warnings; strict root lint is not claimed clean.
The golden selector harness and both document navigation/spelling checks pass.
The integrated command used `just root-test-incremental --no-tests=fail --success-output immediate`
with selector `test(python_context::tests::) | test(production_workspace_startup::rustc::targets::tests::) | test(rust_selected_settings_causally_prepare_contained_arguments_and_environment) | test(pragmatic_rust_semantics_publish_locked_directory_dependency_and_reopen) | test(captured_python_site_packages_survive_public_queries_and_reopen)`.
It was wrapped by `systemd-run --user --scope --quiet --property=Delegate=yes`, with
`DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`, `XDG_RUNTIME_DIR=/run/user/1000`, and both
`CODEFABRIC_RUSTC_EXTRACTOR_BIN`/`CODEFABRIC_PYREFLY_SIDECAR_BIN` selecting the current repository
builds. `just golden --case cargo-directory-source` and `--case python-site-packages` select the
same native cases; the same delegated environment is required on this login-shell host.
The measured small inputs contain 4 Python-context files/146 bytes and 10 mixed-context files/846
bytes. Native Pyrefly takes 0.523 s in the first case; Cargo/rustc takes 25.915 s in the second.
Relational execution plus Delta writing takes 5.600/9.944 s. These are initial samples, not a
before/after performance claim or representative scale qualification.

This delivers P01's first external-input vertical using deliberately captured dependency material
inside the authorized workspace. Separate external filesystem-root registration/fetching, full
package/distribution identity, generated `OUT_DIR` freezing, effective Cargo unit/config closure,
byte-safe compiler argv and measured shared scheduling remain in their 4A–4C/P04/P06/8E slices.
P04 subsequently delivers retained parser/checker/toolchain-input owners; retained Cargo target/fact
state and full input/retention qualification remain open. P02's initial canonical semantic vertical and scoped public retrieval are now delivered.

## Detailed plan expansion, 2026-09-09

The planning revision preserves all 25 slices and the complete ontology coverage map. It adds:

- Source-grounded usage of all eight requested library reference skills, including exact API/pin
  constraints and conditional adoption of overlays, CDF, Rayon and orjson.
- Seven architecture decisions covering validity/version reuse, retained providers, native semantic
  ownership, typed request DAGs, durable layout, maintenance exclusion and shared scheduling.
- Twenty-six identified code enhancements mapped to their owning slices and fourteen implementation
  packages ordered by dependency, with concrete per-slice steps, compatibility/removal rules and
  behavioral acceptance for all eight forms, edits, recovery and sustained retention.

Current-code inspection corrects two earlier plan assumptions: native Ruff already has substantial
owner-scoped CFG construction, and the DataFusion schema wrappers already implement native pushdown.
Their remaining work is production integration/qualification and measured improvement. Other
concrete enhancements include replacing whole-path shortest-path queueing, retaining checker state
with a safe generation update view, reusing unchanged Delta pins, and fixing native optimize/vacuum
commit properties before enabling maintenance. Optimized-MIR source-call completeness remains a
qualification task, not a newly reproduced failure.

Only this file and the detailed plan change. `just docs-check` reports two files and zero navigation
errors; `typos` on both files and `git diff --check` pass. A one-off comparison confirms all 25 slices,
29 ontology rows and historical plan validation evidence are preserved. No implementation or new native/product/performance
validation is claimed. The detailed plan §3.3 defines P01–P14; §10 records the future entry point.

## Typed Rust diagnostic locations, notes and edits

The extractor now retains ordinary `DiagInner` child messages, labeled primary/secondary spans,
native suggestion applicability/style, every alternative and each multipart replacement. Empty
alternatives and disabled/sealed suggestion state remain distinct. Four application-owned Arrow
relations carry the details; typed local formatter and SourceMap APIs stay inside the dated-nightly
adapter. Primary compiler/lint codes still come from the native JSON emitter because the pinned
compiler keeps lint names private. Cargo's artifact, timing, unused-extern and future-breakage report
hooks continue to delegate to its native emitter; the separate compatibility report is outside this
ordinary-diagnostic detail profile.

Each location retains its raw local path independently of any remapped display name. Original byte
ranges use rustc's BOM/CRLF normalization map. Captured source locations carry their own file identity
and content digest, separately from the compilation owner's source fields. Dummy/non-file spans,
unavailable local paths, uncaptured generated files and locations outside the captured universe
retain explicit unmapped states. Changed or aliased captured inputs and invalid captured ranges
remain hard failures. No external display filename is promoted to a source identity.

The retained primary/detail capture shares an 8 MiB payload accounting limit, with separate finite
primary/detail row limits. One pending detail bundle is independently bounded. Overflow drops detail
bundles without orphaning their parent message, and all affected diagnostic families retain unknown
capture scope. Exact detail locations can be unmapped even when the native diagnostic capture closes.

Validation on 2026-09-09 for the code committed in `4cc74d7c`: all 20 extractor tests and
strict check/lint pass, including native remapped diagnostic paths, multipart/empty alternatives and
captured-source mutation rejection. The actual successful-warning and failed-no-MIR contained
compiler scenarios pass (7.80/7.71 s in the final selection), including second-file identity/digest and BOM/CRLF byte ranges;
the successful lint retains its note and exact replacement. All ten selected provider/schema checks
pass with the four new relations. Default/featureless root checks and full governance pass; Clippy
retains exactly 952 library/36 integration warnings, with no new findings. The expanded installed
live/clean/repair/exact-reopen scenario passes (350.01 s), comparing all four detail relations through
replacement and failed-epoch reopening. All-failed startup also passes (58.77 s). The final
four-case native selection passes completely. Canonical/public diagnostic consumers and the
remaining outcomes are still open. These are correctness samples, not performance comparisons.

The raw compiler bundle now has 22 relations; the compiled release has 66 provider relations and
transformations. Its schema-bundle digest changes, while the Protobuf fingerprint remains
`8eddc258dc2129ec0f1580f1936ea4213898c8dd903dcdf36dadbb9451163bfe`. Exact reopening within
this candidate is tested; migration of retained epochs from an older provider bundle was not tested.

## Primary Rust diagnostics and failed compilation observations (`f1e44d80`)

The dated-nightly extractor installs the native diagnostic emitter and retains typed primary
messages, severity, compiler/lint code and invocation-local ordinal. Its additional capture is
bounded to 8 MiB and 20,000 primary diagnostics; overflow, unfinished frames and unavailable sink
initialization leave explicit unknown scope. Messages without a native code retain an empty code.
The compiler's display paths are not source identities. Diagnostic owners bind the exact captured
compilation root. The later typed-detail slice above adds per-message locations, child notes
and suggestions; full generated/hygiene mapping and public consumers remain open.

Closed diagnostic owners now survive ordinary compiler failure even when no MIR callback runs.
Closed successful units within a failed Cargo invocation can also retain positive facts. Canonical
declaration, call and caller-body projections now select their native prerequisites independently,
so diagnostic-only output does not require nonexistent MIR tables. Receipt
admission still requires exact plan/input binding, proved containment, complete kernel accounting,
process samples, an empty process group and a complete output manifest. Failed targets remain
unavailable and every requested target family remains unknown; retained diagnostic rows do not
establish semantic completion. Cancelled, timed-out, resource-limited or corrupt work remains
inadmissible. The existing provider contract uses a `Failed` terminal for this qualified failure.

Validation on 2026-09-09: both real contained compiler cases pass (successful warning/direct call
8.21 s; unresolved-function diagnostic without MIR 8.12 s). Installed live/clean failure, repair and
exact persisted reopening pass on the final production candidate (374.04 s), including removal of
the old diagnostic after repair and unchanged activation history on failed-epoch reopen. All-failed
fresh startup passes (48.36 s), with exact captured Rust bytes and syntax still present. Staged
source/semantic cancellation, obsolete completion and restart pass (188.20 s).
The mixed-target check now explicitly selects completed contexts for complete-call expectations and
separately verifies retained calls in a failed parent context with an unavailable remainder; it passes
(76.48 s). Previously selecting the first context by ID made that expectation dependent on which
parent target was selected. The shared Python call/reopen helper also passes (26.21 s). Public source identity checks remain intact.
All 19 extractor tests and strict build/lint/identity checks pass. Eighteen Rust service checks pass,
including corrupted streams, pin drift, cancellation, ordinary failure retention and failed-stream/
successful-receipt rejection. Four canonical checks and the launcher-evidence check pass together.
Launcher proof checks reject missing accounting samples/output manifests, surviving processes and
degraded accounting. Default/featureless root checks, full governance and all 216 tooling tests pass.
Clippy retains 952 library/36 integration warnings, with no new code/file diagnostics; three previously
oversized functions remain oversized. Changed-file formatting and document navigation pass. The repository-wide spelling check retains
existing escaped-source fixture and vendored/historical-text findings; the new text passes separately.
These delegated-cgroup fixture samples are correctness evidence, not comparative latency measurements.
Full diagnostic mapping, canonical/public diagnostic consumers and the other outcome scope remain open.

## Source-first fresh readiness

Fresh startup now selects a durable source/syntax epoch with checker/compiler scope pending and
starts the existing owned update coordinator to publish its semantic successor. Source-current
queries can read that initial epoch. Semantic-current requests retain their freshness barrier and
deadline; no compiler result is inferred from readiness. Exact reopening of a source-only epoch
uses the same semantic-resumption path as interrupted live updates.

The final installed staged-publication scenario passes on 2026-09-09 (186.06 s), including initial Python
source queries and named pending Rust target scope before the first semantic publication, strict
semantic deadline failure, cancellation of a completed obsolete candidate, and source-only restart
at the same epoch/generation followed by semantic convergence. Initial durable readiness also passes
(8.85 s); its exact-genesis assertions now hold the background successor to avoid timing assumptions.
Installed semantic serving (39.66 s), exact restart (28.67 s), the 70-module Python inventory
(26.12 s), Python calls (29.10 s), failed Rust targets (94.18 s) and guard/resource delivery (22.27 s)
pass. Cargo selections/clean/reopen pass (212.76 s), as do retained processing pages (74.63 s).
Semantic test requests now explicitly wait for semantic-current data and inspect its exact successor
instead of requiring compiler facts in the first activation. Separate source-only tests retain pending
scope assertions. These samples are correctness evidence, not a comparative latency measurement.
Default/featureless root checks, full governance, all 216 tooling tests, documentation navigation and
changed-file formatting pass. Clippy retains 952 library/36 integration warnings, with no new code/file
diagnostics; three previously oversized test functions remain oversized. Strict lint and global
formatting retain their recorded backlog.
Retained provider state, selective persistence and the other remaining outcomes stay open.

## Call scope and public call queries: implemented limited production slice

`system.requested_processing_scope` retains requested provider/input partitions, and native
DataFusion derives `system.entity_processing_scope`. Declaration and call families stay separate.
Terminal provider coverage combines with unresolved targets, missing source/caller identities and
unmatched implicit Pyrefly calls. A real property/decorator fixture verifies that unnormalized implicit
calls leave call coverage partial while declarations remain complete.

`fact.code_call_selector` joins canonical entities and projects incoming/outgoing subjects without
multiplying call occurrences. `query.result.call-facts` uses an exact subject semi join. Installed
modern clients exercise Python and Rust incoming/outgoing calls, omitted-direction default, the two
one-step distance phrases, repeated subjects/sites, dynamic targets, empty results, observed limits,
language/context selection and a failed Rust target. Python calls pass after exact persisted reopen.
Unsupported distances enter the existing clarification path; broader traversal remains unimplemented.

Validation on 2026-09-09: the installed Python/call/reopen scenario passes (16.02 s); the mixed Rust
failed-target/declaration/call scenario passes (60.89 s). Sixteen focused processing/recipe/ingress
and implicit-call tests pass. Default and featureless `just root-check` and `just governance-scan`
pass. Strict `just root-clippy` fails on the existing broad lint backlog (960 library and 1,084
library-test errors in that run, before fixing two new processing findings). A subsequent library
Clippy run completes with 959 warnings; this is not strict lint cleanliness. `just root-fmt` also
fails on pre-existing formatting in seven untouched files; all twelve changed Rust files pass focused
format checks. Docs navigation and `git diff --check` pass. Five final processing/implicit-call/exact
reopen regression cases pass after the processing refactor (reopen 16.65 s).

This login shell lacks delegated provider cgroups. Runtime checks pass inside
`systemd-run --user --scope --quiet --property=Delegate=yes env` with the two provider binary
variables below. The initial direct contained-provider run fails `SandboxUnavailable`; no production
containment bypass or host configuration change was introduced.

## Declaration selection and reusable public subjects

FindEntities now exposes the canonical declaration kinds actually produced today: Python functions,
classes, parameters, bindings, imports, type aliases and type parameters; Rust functions, constants,
statics and constructor kinds. New entity results include a reusable public entity ID. Exact older
epochs without that optional field remain readable. Guarded selections present readable labels while
submitting the same opaque choice IDs; the installed driver selects by a unique live label.

Real installed-client Python class/parameter/binding/import/type-parameter/type-alias and Rust
constant/static scenarios verify independent expected names/kinds, scoped processing and public
FindEntities-to-RetrieveFacts subjects. The mixed failed-target case retains its unavailable Rust
partition. Final installed checks pass: Python kinds/fact subjects 12.95 s, readable guard 11.04 s,
and Rust kinds/facts/failed-target calls 63.91 s. Exact-reopen regression passes (17.94 s).
`just root-check-fast` and 15 focused processing/recipe/ingress tests pass.
The adapter fast checks pass (95 tests plus lint/types), and the label-selection harness test passes.
Library Clippy completes with the existing warning backlog; strict lint remains open as recorded above.
Changed-file formatting, Python Ruff, docs navigation and `git diff --check` pass.
Canonical coverage for additional kinds does not establish complete Python/Rust type/member semantics.

## Public lexical-reference traversal

A native `fact.code_relationship_selector` combines call witnesses with Python lexical references.
The `lexical references` meaning follows one step from a reference occurrence to its target or, in
reverse, from a target to each referring occurrence. Repeated request subjects do not multiply rows.
Reference write/read/call/type/import kinds and lexical precision remain visible. Public occurrence
and source/target IDs support subsequent traversal. Call-specific fields are null on reference rows;
existing call facts retain their provider details. Older call-only catalogs retain their original path.

Requested reference coverage comes from captured inputs and actual Ruff reference-family outcomes.
Unresolved/candidate targets qualify otherwise completed partitions; Rust reference scope is explicitly
unsupported. Incoming selection conservatively includes every requested file in the selected context.
This is lexical coverage, not project-aware checker reference or import/export completeness.
Validation on 2026-09-09: installed Python references pass across complete and unknown-target source,
exact reopen, repeated subjects, occurrence-ID reuse, independent source positions/kinds, limits and
empty results (30.39 s). The mixed Rust call/declaration/reference-unsupported scenario passes (64.48 s).
Three focused processing cases pass; Python calls/exact reopen pass with the combined relation (17.49 s).
Default and featureless `just root-check`, governance, docs navigation and changed-file formatting pass.
Library Clippy completes with 958 existing warnings and no new findings; strict lint remains open.
Full semantic references remain open.

## Exact source-context queries and live disclosure authorization

`RetrieveSourceContext` now selects canonical declaration subjects with the `exact source span`
meaning. Captured bytes are stored once per file in `source.exact_source_bytes` and joined through
workspace/generation/file/digest pins at query time. `fact.code_source_context` stores descriptors.
The bounded native projection removes whole-file bytes from public output and returns a source-context
identity, lossless UTF-8 or binary, half-open delivered byte positions, one-based line numbers,
zero-based byte columns and exact returned/omitted byte counts. `return.maximum_source_bytes` is an
independent per-span limit in 1..=1048576. Python declaration spans currently identify declaration
names; Rust spans retain the compiler's declaration range. The later source-context slices below
add function definition/body selection and surrounding lines.

Workspace registration defaults to metadata disclosure. `WorkspaceRegistry::set_source_disclosure`
explicitly grants/revokes source access and advances the policy revision. Preparation, native batches
and every result-resource chunk recheck live policy. Query-local scalar capabilities retain exact
function identity, stay within the private child session and bypass shared plan caches.

Installed-client evidence on 2026-09-09: Unicode/CRLF positions, a limit splitting UTF-8, disk changes,
exact reopen, metadata access while source is denied and same-session revocation of an unread
published page pass (21.04 s after the final coordinate refinement). The mixed Python/Rust source/empty/failed-target scenario, including
the previous declarations/calls/reference checks, passes (67.56 s). Nineteen focused compiler,
request-authority and source-materialization tests pass. Default/featureless root checks, 95 adapter
tests, 186 tooling tests, docs navigation and governance pass. Strict lint remains open on the
existing backlog; final library Clippy completes with 958 warnings and no new findings. This does not close source
syntax coverage, complete public-form semantics, composition, freshness or outcomes 4–8.

## Function definition and body source contexts

`RetrieveSourceContext` now selects `function definition` and `function body` alongside the exact
canonical declaration span. Native DataFusion joins the Python binding to its exact Tree-sitter name
child and the Rust declaration header to the exact function-item start. Body selection follows the
same parser run's named body child. File/digest/generation and parser owner keys prevent cross-file
or nested-function substitution. Results identify the source-mapping method. Byte limits, lossless
text/binary output and live source authorization use the existing materialization path; the actual
context kind now participates in source-context identity. Older catalogs expose only the meanings
supported by their schema.

A separate `function-source-context` processing family starts from requested declaration partitions
and marks unmapped function owners partial. Missing syntax, stale digests and different contexts
cannot establish body absence; pending provider scope remains pending. Public summaries retain the
source-context family label, and private remainder paging retains the selected internal family.
The native scope regression passes for good/missing owners, stale digests, two Rust contexts and
pending work. The initial 29-case source/canonical/processing selection passes, including the installed
source-disclosure scenario (38.20 s). The mixed live/clean function scenario passes (206.25 s on 2026-09-09): nested Python definitions,
CRLF/Unicode and byte truncation, Rust braces in strings/comments, edits and exact restoration all
match independent source expectations and clean daemons. `just golden --case function-source-live`
selects it. Default/featureless root checks, all 202 tooling tests, tooling lint, governance, docs
navigation and changed-file formatting pass. Root library Clippy retains the 955-warning baseline;
combined library/integration Clippy reports no findings on changed lines.
Syntax nodes determine definition/body boundaries; separate decorator or attribute nodes need broader
context selection. Full syntax outlines, surrounding-line selection, other source subjects and
composition remain open.

## Decoded source mappings

Capture, Python syntax and the Pyrefly executable share an allocation-free selection rule for UTF-8,
UTF-8 BOM, ASCII and Latin-1 inputs. Python coding cookies apply before UTF-8 detection, only in legal
first/second-line comments; conflicting BOMs, unknown codecs and invalid bytes are unavailable.
Syntax admission verifies both decoded text and every original-byte boundary against captured bytes.
Ruff converts all source-bearing semantic observations after decoded-text analysis. Pyrefly reads a
private UTF-8 view and projects located types, calls and cross-file definition anchors through indexed
original-byte mappings. Daemon admission uses the selected encoding when checking target boundaries.
Captured bytes and their digests remain authoritative for source disclosure.

The rustc extractor now uses the pinned compiler's `SourceFile::original_relative_byte_pos` map for
item, MIR and local spans, including BOM stripping and CRLF normalization. Provider line/column
observations retain compiler coordinates; public source context derives coordinates from raw bytes.
Strict sidecar and extractor checks pass, with 36 sidecar tests and 14 extractor tests. The new
checker case resolves a Latin-1 caller to a BOM/UTF-8 definition with independently checked original
byte slices. The root decoding/admission cases pass, including forged-map rejection. Installed
mixed live/clean acceptance passes (203.40 s on 2026-09-09), with exact declarations, cross-encoding
call targets, authorized lossless source, encoding changes/restoration and BOM/CRLF Rust spans.
`just golden --case decoded-source-live` selects the case with a 600-second bound. All 204 tooling
tests, tooling lint, full governance and documentation navigation pass. All 45 affected root source
tests pass, including the 10,000-file governed capture case (63.30 s). Default/featureless root
checks pass; library Clippy retains 955 baseline warnings, with no library/integration findings on
changed lines. The packaging guard now excludes vendored package manifests while checking activated
Rust dependency graphs; an application-file negative probe still fails as expected (`e9743daf`).
Further codecs, full coordinate semantics, syntax outlines, broader context selection and retained
parser/checker state remain open.

## Source-context text columns

Source context now adds zero-based `start_utf8_column`, `end_utf8_column`, `start_utf16_column` and
`end_utf16_column` beside original byte columns and one-based lines. UTF-8 columns count bytes in
decoded text; UTF-16 columns count code units. Both endpoints share one allocation-free scan of the
selected decoding. BOM bytes and partial characters have no text position; byte coordinates remain
available for lossless truncated output. The checks cover astral characters, Latin-1, BOM, CRLF,
empty input and missing final newlines. The mixed installed source/call scenario passes with
independent UTF-8/UTF-16 column expectations (202.48 s), including an astral character, Latin-1,
BOM/CRLF Rust, encoding changes/restoration and clean daemons. The function definition/body
scenario also passes (204.34 s), including public null text columns for a split-character byte limit.
Default/featureless root checks, full governance, docs navigation and changed-file formatting pass.
Library/integration Clippy adds no diagnostics over the previous slice; the library retains its
955-warning baseline.

## Surrounding-line source context and hard-limit failures

Source context now offers `surrounding lines` for declaration subjects with explicit
`return.source_lines_before` / `return.source_lines_after` counts (0–4096 per side; an omitted side
is zero). Catalog semantic-role metadata distinguishes epochs that contain line anchors. Queries
expand complete physical lines from captured bytes, retain CRLF and file edges, and expose anchor,
requested-window and delivered-byte ranges separately. No requested byte limit now means no semantic
truncation limit; exceeding the service byte envelope returns non-retryable
`QUERY_HARD_LIMIT_EXCEEDED` through the appended Protobuf enum and strict adapter projections.
The generated descriptor identity is `b3:9012381600dcae6ba4e347a9b376ff36c16c00dc17d78ac2350c884cd24dd7be`.

On 2026-09-09 the installed `source-lines-live` scenario passes in 33.70 s: CRLF/astral Unicode,
zero/large windows, missing final newline, split-character binary output, exact reopen and an
oversized body with and without an explicit 128-byte limit. Seven affected line/source/ingress/error
checks pass. Default/featureless root checks, full governance, all 109 adapter tests with lint/types,
206 tooling tests with lint, docs navigation and changed-file formatting pass. Library Clippy keeps
its 955-warning baseline; an added test-only ownership warning was corrected. Full root strict lint
and global formatting retain the previously recorded unrelated failures. Broader source subjects and
syntax outlines remain open.

## Raw compiler source paths

Compiler manifests now retain exact relative path bytes and read the prior UTF-8 map format.
Both formats reject duplicate paths; paths with traversal, aliases or duplicate file identities
remain invalid. An unrelated captured non-UTF-8 Python pathname no longer fails Rust manifest
construction. The compiler's pinned `RealFileName::local_path` supplies an optional binary
`span_file_bytes` coordinate alongside the display-only `span_file`. Owner verification and Rust
call-source matching consume exact paths. Non-file/compiler-virtual locations remain unmapped.
Only the released path field and its logical type are admitted as binary at the provider boundary.

On 2026-09-09 the installed `rust-paths-live` scenario passes in 138.00 s: a Unicode Rust crate-root
path with non-UTF-8 and display-colliding Python siblings retains declarations, calls and exact source
through an edit, independent clean comparison and exact reopen. The 16 extractor tests and strict
extractor checks pass, including a real remapped-path compiler round trip. Six affected root
manifest/schema/provider-boundary tests, default/featureless root checks, full governance and 208
tooling tests pass. Changed-file formatting and docs navigation pass. Five newly exposed Clippy
match-arm findings were merged; final library/integration Clippy adds no diagnostics over the
955-library/36-integration baseline, and the provider schema census passes again. This does not remove rustc's
UTF-8 invocation-argument constraint or add dependency/generated source ownership.

## Contained custom Cargo build inputs

The selected C compiler is captured into the owned dependency view, with its selected read-only
`/usr` library prefix supplied through a wrapper. This resolves Debian's `cc` alias through
`/etc/alternatives` and GCC's library search after relocation without widening sandbox mounts.
Compiler bytes and the search selection enter context dependency identity. Default, custom and
disabled `package.build` settings are resolved from captured manifests; explicit missing or escaping
scripts fail preparation. Build-script declarations remain observable alongside target declarations.

On 2026-09-09 `cargo-build-live` passes in 124.86 s through installed clients: changing a custom
script's cfg output changes the effective context, selected declaration, direct call target and
exact function body, matching an independent clean daemon. Two focused build-input tests and all
210 tooling tests pass; tooling lint, changed-file formatting and docs navigation pass.
Default/featureless root checks pass. Library/integration Clippy retains its 955/36-warning baseline
with no new code/file diagnostics. The compiler-capture governance exception includes only the
new host compiler helper, with a positive rule fixture; full governance passes and workspace
source rules remain enforced.
Earlier native attempts exposed the linker alias/library
search failures and the existing target-wide call scope. Calls to uncaptured standard-library
functions in the build script correctly retain an unresolved target remainder for that broad scope.
The following slice adds per-caller outgoing processing. Distinct host/target contexts, generated/
proc-macro input closure, the full host C SDK closure and Cargo cache/configuration coverage remain open.

## Rust outgoing-call owner scope

The additive processing projection now joins explicit Rust declaration owners to admitted MIR
bodies and exact source revisions. Caller-local unresolved targets and source locations remain
partial; an unowned call qualifies its whole context. Missing/stale MIR bodies and unfinished
provider work cannot establish an empty call set. Queries can narrow explicit outgoing subjects
only when every selected owner has a retained partition. Unknown subjects, incoming queries,
Python and older persisted schemas keep the existing conservative scope. Private continuation
selection retains owner IDs and avoids counting owner and context partitions together.
On 2026-09-09 six focused recipe/selection/processing/continuation cases pass, including stale MIR,
unowned calls, repeated subjects, unknown subjects and exclusion of overlapping context rows.
The installed `cargo-build-live` case passes with typed caller IDs and exact reopen (152.97 s):
direct and known-empty outgoing callers are complete, while build-script and incoming queries keep
their relevant remainders. Live results equal an independent clean daemon. The mixed failed-Rust-target
public queries pass (67.73 s), preserving unrelated failures for incoming calls while narrowing
known outgoing callers. Retained 130-partition processing pages pass across repair/restart (56.71 s).

All 109 adapter tests, default/featureless root checks, all 210 tooling tests, tooling lint and full
governance pass. Final affected Clippy retains the 955 library/36 integration warning baseline with
no new code/file diagnostics; the final pure owner-selection refactor passes the six focused tests
again. Changed-file formatting and docs navigation pass. Python owner scope, incoming dependency/
frontier precision, other families, efficient status indexing and distinct host/target contexts remain open.

## Captured Cargo platform and flag selection

Requested Cargo targets now expand over the captured workspace `build.target` string or array.
The dated compiler resolves `host-tuple`; repeated effective platforms are deduplicated before
execution. Extensionless `.cargo/config` takes precedence over `.cargo/config.toml`, consistent
with the contained Cargo working directory. Each pending or failed partition carries an optional
typed target platform through exact persistence, remainder paging, Protobuf and the adapter.
Missing platforms remain unavailable while a valid selected platform can publish useful facts.
Malformed or empty selections fail discovery explicitly.

On 2026-09-09 the installed `cargo-platforms-live` case passes in 198.82 s: missing platform,
mixed valid/missing selections, host alias deduplication, cfg flag changes, configuration precedence,
ignored configuration edits, independent clean reconstruction and exact reopen. It checks exact
Rust declarations, outgoing call targets, source bodies, context changes and processing remainders.
Four focused target/processing tests pass, along with 109 adapter tests, 212 tooling tests and
lint/types. Default/featureless root checks and full governance pass. Six final focused target/processing/continuation cases pass after simplifying the processing
and progress helpers (0.12 s). Affected Clippy retains the 955/36-warning baseline with no new
code/file diagnostics. Changed-file formatting, docs navigation and whitespace checks pass. Full feature/profile selection, custom target-spec
closure, registry/git/generated inputs, host/target separation and build caching remain open.

## Shared captured Rust toolchain preparation

Every semantic publication pass now lazily captures one immutable compiler/sysroot/extractor/host-C
bundle shared across selected Cargo targets. A failed capture is shared too, while requested targets
retain separate failure explanations. The workspace budget owns the retained bytes through the pass;
the initial capture reservation shrinks to measured retained capacity. The extractor read now shares
the complete toolchain byte bound. Compact capture diagnostics report bytes, files, elapsed time
and the input digest.

Actual captured toolchain contents now enter effective context identity. Changing runtime bytes
changes the context; relocating identical bytes or advancing only source generation does not.
A previously captured view remains immutable after an installed file changes. Four focused capture/
context/selection tests pass. The installed multi-platform live/clean/exact-reopen case passes in
205.79 s on 2026-09-09. The preceding single sample was 198.82 s, so this scenario establishes
correctness with shared capture, not an end-to-end speedup. Default/featureless root checks,
governance scan, docs navigation and changed-file formatting pass. Library/integration Clippy
retains its 955/36-warning baseline with no new code/file diagnostics. Broader performance measurements,
retained build caches, parallel scheduling and observation of external toolchain changes remain open.

## Cargo library linkage selection

Captured manifests now supply typed linkage selections for libraries, proc macros and library
examples. Metadata admission compares the complete selected crate-type set and Cargo target role,
accepting `rlib`, `dylib`, `cdylib`, `staticlib` and combined library outputs without admitting a
substituted linkage or target role. The extractor preserves repeated and comma-separated rustc
crate-type flags in its existing raw identity field. Linkage changes remain context changes.

On 2026-09-09 `cargo-linkage-live` passes through installed clients (175.53 s): `cdylib`, `dylib`
and combined `rlib`/`cdylib`/`staticlib` targets retain exact declarations, outgoing calls and source
bodies across manifest edits, independent clean reconstruction and exact reopen. Seven focused
context/capture/metadata cases pass; five affected cases pass again alongside the native scenario.
All 17 extractor tests and strict checks pass, including the real multi-linkage compiler IPC round
trip. Default/featureless root checks and all 214 tooling tests plus lint pass. Affected Clippy has
954 library and 36 integration warnings, with no new code/file findings; the metadata refactor
removed one existing length warning. Full governance, changed-file formatting and documentation
navigation pass.
Full feature/profile choices, host/target separation, dependency/generated/proc-macro closure,
custom target specifications, caches and scheduling remain open.


## Captured Cargo feature and profile selections

Captured `package.metadata.codefabric.rust_contexts` now selects features, default features,
profiles and optional platforms, with fallback to the owning workspace metadata. Package entries
replace inherited entries. Effective duplicates coalesce; malformed entries, missing features and
missing profiles retain unavailable scope alongside valid contexts. Contained Cargo metadata must
confirm the inherited workspace. Its manifest and effective settings participate in context identity.
Public processing remainders carry an optional typed `rust_build` selection; empty features and
`default_features=false` remain distinct from an absent selection on old snapshots.

The installed `cargo-selections-live` scenario passes on 2026-09-09 (192.07 s): default/explicit/
disabled features, a custom profile's debug-assertion behavior, inheritance, overrides, duplicates,
three independent preparation failures, exact calls/source, clean reconstruction and exact reopen.
Six focused context/owner/paging cases pass (0.13 s), as do two storage/paging regressions (0.014 s).
All 109 adapter tests with lint/types and all 216 tooling tests with lint pass. The native check found
and fixed Delta list storage naming: the shared storage policy now uses the kernel's `element`
representation and restores the logical list name, metadata and width. Kernel/Arrow round trips
cover regular, large, view and fixed-size lists with null values. All 16 final schema/context/paging
checks and full governance pass after locating the kernel integration test inside the fabric boundary.
Default/featureless root checks pass; Clippy reports 952 library and 36 integration warnings, with
no added findings over the preceding slice. Strict lint/global formatting retain their recorded
unrelated backlog. This does not establish optimized
release-profile source-call coverage, external/generated build closure or full context scheduling.

## Live source reconciliation and current query selection

The daemon now owns a bounded, coalesced notify queue, a native watcher with an owned blocking
lifetime, and a serialized update operation. Watches precede the first census. Events are hints;
secure descriptor-relative inventories and captured bytes remain authoritative. Queue overflow retains
a reconciliation watermark, and periodic censuses recover missed events. A durable single-row
`source.input_inventory_state` relation permits unchanged exact reopen and avoids needless publication.
Updates reuse startup capture, contained providers, normalization and exact activation. Whole-context
replacement removes deleted owners; event/capture fences abandon obsolete candidates.

Current-required policies request an authoritative census and wait for publication. Admission retries
an event racing snapshot selection within a deadline. Best-available queries retain the selected epoch
and its actual freshness. Typed snapshot events expose freshness/context selection; a separate typed
workspace status reports observation/reconciliation watermarks, watch health, rescan and runnable work.
Unavailability and freshness deadlines have distinct safe errors. Old source pages keep their exact
captured bytes across newer publications. Epoch retirement no longer closes the shared workspace
admission gate; shutdown owns that transition.

Validation on 2026-09-09: 49 selected query-service/coordinator, watcher/barrier and installed runtime
checks pass. The extended live Python scenario includes complete source removal/recreation, public
status and unchanged exact reopen (68.53 s); the old-source-page/live-edit/reopen scenario passes
(31.45 s). The async challenge-expiry regression found by the first integrated run is corrected and
its regression passes. Adapter lint/types and 100 tests, generated protocol compatibility, governance,
docs navigation and diff checks pass. Default/featureless root checks and isolated `data-fabric`
checking pass. Library Clippy completes with 957 existing warnings and no findings on changed lines;
strict lint remains open. Changed Rust files pass formatting except pre-existing layout elsewhere in
`activation_transaction.rs`. The final installed live regression passes after the future-ownership
refactor (67.12 s). This is a limited live slice, not closure of outcomes 5 or 6.

A persistent mixed Python/Rust daemon now passes independent clean-state comparison for the four
implemented forms through call-target edits, compiler failure and repair (259.56 s on 2026-09-09).
Each clean daemon uses separate durable state and provider caches with the same authorized source
identity. Comparison retains canonical IDs, relationships, facts, positions, precision, ordering,
source bytes and coverage; it excludes generation/run/observation IDs and the explicitly snapshot-bound
source-context handle. Independent expected names and call pairs prevent equal empty results from
passing. Compiler failure removes current Rust declarations/calls and reports one incomplete target
while Python queries remain usable. The repaired result also matches the original semantic result.
`just golden --case mixed-clean-live` selects this case; its 600 s bound covers the repeated contained
builds. The accompanying harness checks pass with all 188 tooling tests.

Remaining live work includes broader clean/edit/context cases, retained provider/parser/compiler
state, external roots, Git inclusion, an explicit polling
profile, root replacement recovery and broader configuration/negative-dependency and delayed-completion cases.
Source-current queries can select the source stage; other strict policies conservatively await the
complete workspace provider pass. Target/family-specific barriers and all-family scope remain open. Reconciliation currently rebuilds all selected relations;
selective persistence and finite historical/candidate reclamation remain outcome 8 work.

## Source-first live publication and semantic convergence

Live updates publish a source/syntax epoch with pending Python checker and named Rust target scope,
then a semantic successor at the same source generation. Source-current requests use their own census
barrier and can select the first epoch; semantic-current/await-latest requests wait for the terminal
provider pass. A terminal provider failure remains incomplete without leaving runnable work pending.
The durable input-inventory relation records the publication stage, so exact reopen resumes unfinished
semantic work. Older epochs without that field retain their terminal-provider interpretation.

Public status separately exposes source reconciliation/freshness and whether the selected epoch awaits
semantics, with optional-field compatibility across Rust/Python. Captures invalidated by concurrent
edits retry as pending work. Candidate attempts have distinct physical namespaces; activation recovery
preserves an unresolved candidate before admitting a newer publication.

The debug-only `hold_semantic_update_publication` fixture pauses actual completed providers before
activation, without bypassing containment or publication fences. Captured bytes retain their leases
while releasing the exclusive operational writer before checker/compiler work. This permits source
censuses during semantic execution; capture/release/census writes share one short-lived writer gate.

Validation on 2026-09-09 against this continuation of `6b6abffb`: default/featureless `just root-check`
passes. The final delegated-cgroup `just root-test-incremental` selection passes all 43 affected cases:
mixed paused source/semantic publication, named target scope, strict deadline, obsolete completion and
pending restart (168.05 s); complete Python edit/remove/atomic-save/empty/recreate/reopen (114.60 s);
four-form mixed independent-clean comparison and compiler failure/repair (294.29 s); old-source-page
and live disclosure checks (41.57 s); processing, query-service, watcher, barrier and capture ownership
regressions. The capture check independently opens a writer while exact bytes/leases remain live,
then verifies lease release. Native commands use the delegated scope and provider binaries below.
Adapter lint/types and 106 tests, 190 tooling tests, generated protocol compatibility, governance,
docs navigation, changed-file formatting and diff checks pass. Library Clippy completes with the same
957 baseline warnings and no added findings; strict root lint and unrelated formatting remain open.

Retained checker/parser/Cargo state, selective relation-version reuse, full scope, historical/candidate
reclamation and the remaining outcomes 4–8 requirements are open. No outcome is closed by this slice.

## Public processing remainder continuation

`get_code_graph_processing` accepts the durable daemon query ID, query block ID and next offset.
It returns a typed 64-row page from the original exact processing relation, with the same snapshot,
source generation, family, language/context selection and total counts. Complete row ordering uses
raw paths and the remaining scope keys. The private result manifest retains the table/version and
selection; public manifests and wire messages omit storage addresses. DataFusion filters and pages
the selected Delta version under the shared workspace budget and owned native lifetime.

A separate resource keeps processing continuation alive after fact and manifest reads. After restart,
the daemon authorizes the accepted query and reissues its retained resource; it does not resubmit the
query. Principal, workspace, policy/revocation generation, expiry, block and offset checks precede reads;
resource/session authority is checked again before delivery. The adapter releases the continuation
when the final page arrives. Older results without a retained processing selection remain readable
but cannot acquire a new continuation. Full historical query selection remains separate unfinished work.

Validation on 2026-09-09: the installed 130-partition Python case passes across fact/manifest
consumption, exact restart, whole-workspace repair, two remaining pages and invalid block/range/released
resource requests (53.38 s). All 130 old paths retain their order and original incomplete scope.
The broader run passes 57 processing/coordinator/service/retention/source regressions; its new
negative-case failure exposed missing typed resource errors and an incorrect test expectation, both
corrected in the final installed run. Adapter lint/types and 108 tests pass, including reconnect
with renewed handles and rejection of changed processing facts. All 192 tooling tests, tooling lint,
governance, docs navigation and default/featureless root checks pass. Library Clippy completes with
955 existing warnings and no added lint categories; strict lint and baseline formatting remain open.
Generated protocol compatibility passes (28 tests and generator verification).

All-family/owner/dependency scope, indexed status aggregation, target/family-specific barriers and
the full outcomes 4–8 target remain open.

## Outcome 4: real inputs, canonical facts and first-four queries — partial

P01/P02 initial semantic verticals and P03’s initial query boundary are delivered. Complete effective contexts, native unit/family coverage and the first useful release remain open.

| Slice | Current implementation | Remaining scope |
|---|---|---|
| 4A | Partial; captured contexts/diagnostics and retained immutable toolchain inputs | Full external/generated/effective units, Cargo target/fact reuse, scheduler and complete deployment invalidation |
| 4B | Partial; captured selected contexts and retained contained checker | Complete effective external Python contexts and semantic output; retain chunking and definition anchors |
| 4C | Partial; retained parser/Ruff state and selected live/clean source behavior | Full syntax/codec/coordinate/path census, external/generated inputs and remaining incomplete edits |
| 4D | Partial, committed | Remaining canonical families, authority/unknowns and external/generated/edit-time identity |
| 4E | P03 initial four-form query boundary delivered | Remaining meanings/subject roles/directives, last-four inputs and full composition with P06/P10 |

## Outcome 5: processing, incomplete scope and freshness — partial

Typed requested/completed/remainder partitions, block-local scope, source-current barriers and retained processing continuation exist. All-family dependency closure, historical selection and efficient authorized status remain open.

| Slice | Current implementation | Remaining scope |
|---|---|---|
| 5A | Partial; selected canonical/query families and block-local scope demonstrated | Complete query dependency/owner scope, all families and efficient authorized live status |
| 5B | Partial live source/semantic barriers and typed status | Target/family-specific convergence and historical query selection |

## Outcome 6: continuous updates — partial

P04 has accepted retained-provider, observation, pin-reuse and selected live/reopen continuations. Complete 6A–6C and P05’s assembled first-release corpus remain required.

| Slice | Current implementation | Remaining scope |
|---|---|---|
| 6A | Pruned native/poll watches, selected Git metadata observation and faster compatible Merkle hashing | Git inclusion/conflict stages, external roots, complete topology/recovery and watch-cost qualification |
| 6B | Whole-context replacement/fences; persisted provider-executable observation passes live/reopen acceptance | Full positive/negative/external/tool dependency closure, owner manifests and finer valid replacement |
| 6C | Source/semantic stages; retained parser/checker/toolchain owners and scoped version reuse | Cargo unit/target/fact retention, allocated native CPU shares and bounded fair backlog |
| 6D | Limited mixed clean/live comparison and deterministic publication pause | Broader language/context/edit corpus and all-family comparisons |

## Outcome 7: full analyses and all eight forms — open

The native schemas/algorithms and P02/P03 foundations below do not establish full production analyses or complete query algebra. P06–P11 cover every remaining family and form.

| Slice | Current implementation | Remaining scope |
|---|---|---|
| 7A | Selected module/import/reference/type/call normalization delivered; full slice open | Full Python families, member/dispatch/type propositions, external semantics and replacement |
| 7B | Open; substantial native Ruff CFG and analysis algorithms exist | Integrate explicit native CFG/evaluation events, remove implicit derived fallthrough and qualify production behavior |
| 7C | Open | Python memory/effects/resources/exceptions/capture/async/concurrency on 7B |
| 7D | Partial raw foundation and ordinary native diagnostic details | Full typed Rust family census, canonical types/instances/lowering, generated/hygiene mappings and diagnostic consumers |
| 7E | Open | MIR analyses, exact private borrow facts and advanced state on real bodies |
| 7F | Open | Demand-rooted graph algorithms, structural facts and interprocedural summaries |
| 7G | P03 repeated first-four blocks, typed priors and branch isolation delivered; full slice open | Last four forms, remaining first-four meanings and full mixed-form algebra/directives |
| 7H | Partial foundation; full slice open | All-form modern delivery, cursors/permissions, replay/reconnect/expiry and Rust-owned retention |

## Outcome 8: sustained operation — open

P04 supplies selected caching, bounded writes and exact reuse foundations. P12–P14 still own finite state, native maintenance, integrated recovery and representative measurements.

| Slice | Current implementation | Remaining scope |
|---|---|---|
| 8A | Exact source/empty-Arrow pin reuse and bounded native writes delivered | Consumer-based persistence, nonempty owner replacement and measured finite edit-time growth |
| 8B | Open; checkpoint/dry-run only | Fix native commit-properties seams and enable owned compaction/destructive vacuum |
| 8C | Retained provider caches have owned idle/headroom cleanup; full slice open | Coordinated source/result/snapshot/build/log/diagnostic retention and real reclamation cycles |
| 8D | Partial Linux baseline | New update/provider/query/maintenance failure and recovery behavior |
| 8E | Open; signals/tooling only | Correlated runtime metrics and representative performance/retention measurements |
| 8F | Native pushdown and scoped reuse; synthetic Merkle improvement observed | Representative all-consumer/property and workload optimization, preserving correctness and coverage |

All 25 slices remain selected in the detailed plan, with E01–E26 library-grounded enhancements
integrated into P01–P14. The table is the current aggregate; earlier per-slice records above preserve
commands and scoped acceptance at their stated revisions. No preparation, schema-only or selected
fixture result closes the full product.

## Validation at the stopping point

**Current package validation at the stopping point (2026-09-10, through `722b57d4`).** These are
attributable scoped runs, not a summed full-suite result. Native cases used rebuilt installed
providers, stable root/dated-nightly extractor separation and a delegated Linux user-systemd scope.
Some static checks overlapped native runs; durations are correctness observations, not isolated
benchmarks. The preceding sections retain exact selectors, configuration, nextest IDs and failure logs.

| Scope/revision | Check/evidence | Result and limit |
|---|---|---|
| P01/P02/P03 initial exits | Captured dependency, canonical family and installed first-four query/reopen cases in the detailed plan §3.3 and this file; P03 corpus `47b0c225` with schema fix `445bcbda` and related contexts `75687368` | Initial vertical/query boundaries delivered; P05 first useful release and full outcomes remain open |
| Retained syntax/checker/toolchain (`55c69cdd`, `1f590cdc`, `44053b64`) | Installed syntax live/clean + staged restart; eight-state Pyrefly corpus; six retained Rust toolchain cases | Pass at recorded revisions; Pyrefly 1,528.600 s, Rust six-case run 522.443 s; full Cargo output/dependency/CPU scheduling remains open |
| Watch/Git/Merkle (`2e71be9e`, `20230937`, `7e4ba9d9`) | Native/poll/Git metadata ownership cases; seven inventory cases; installed poll/nested source/external Git metadata/reopen | Pass; final installed poll case 85.998 s; 16,386-leaf hash 521.957 → 27.748 ms is synthetic only |
| Drain and empty exact reuse (`8a92d019`, `096c6db7`) | 22 exact Arrow/provider cases; installed mixed first-four, comment empty/populated/empty + stale/restart, client-timeout/publication/reopen | 22 pass; final three installed cases pass in 804.945 s; no full retention/maintenance or resolution of the earlier native cleanup cascade |
| Provider executable invalidation (`722b57d4`) | Ten focused watch/witness cases, two deployment/legacy cases; final installed delayed replacement and changed/unchanged restart | Pass; installed case 443.603 s, `/tmp/codefabric-p04-provider-deployment-installed-v3.log`; final code includes both fixture corrections and boxed future |
| Final stable checks (`722b57d4`) | `just root-check`; `./scripts/cargo-check-mode.sh cargo clippy --locked --all-targets --message-format=json`; focused rustfmt | Default/featureless pass; no affected changed-line/header Clippy findings; existing root warnings remain, no strict global lint/full-root/doctest claim |
| Product wrapper and system uv | Focused pytest/Ruff; `just tools-doctor`, `just tool-version-contract-check`, shell/config parsing (`a644b295`) | 50 product-harness cases pass in 0.80 s; earlier uv consumer selection 228 pass; system uv 0.12.13 accepted without CLI pin |

The failed installed native run `/tmp/codefabric-p04-toolchain-cache-native-final.log` remains an
unresolved cancelled-executor/cleanup case. Later targeted shutdown probes pass without reproducing
it. Documented fixture fixes and the Drain acknowledgement correction do not establish that native
failure's cause or resolution. No four-domain aggregate, destructive maintenance, sustained state
or representative end-to-end performance acceptance is claimed.

## Historical validation before package execution

These are attributable implementation runs from 2026-09-09 through the diagnostic-detail checkpoint.
They predate the P01–P04 continuations recorded above and do not describe the current final checks.
Counts overlap and must not be added into a full-suite total.

| Code scope | Command/observation | Result and boundary |
|---|---|---|
| Through canonical Rust calls (`45421cff`) | Focused canonical/provider tests and real mixed/path-dependency daemon scenarios | Direct, indirect, repeated and macro/unmapped calls; exact installed restart pass; seven final selected cases pass |
| Public declaration facts (`64ce1acf`) plus IPC/diagnostic corrections | Focused query/scope/resource tests, installed mixed-client query, exact restart | 42 selected tests pass; repeated subjects, failed Rust target, completed empty Python scope and unsupported references exercised; exact reopen about 15.2 s |
| Chunked Python inventory (`1301df5a`) | 16 selected root Pyrefly/daemon tests; sidecar check/tests; protocol and adapter checks | Pass, including contained cross-chunk imports and 70-module fresh Delta publication; 29 sidecar and 95 adapter tests, adapter lint/types and `just proto-check` pass |
| Checker definition anchors (`92bb153d`) | `just sidecar-test`, `just sidecar-check`; selected root provider/recipe/daemon cases | 30 sidecar and 26 root cases pass; imported alias/bound method, wrong file/digest/range and real 70-module target anchors |
| Canonical Python calls (`734821db`) | Affected native/canonical/recipe tests plus real Python/mixed Rust and installed restart | Initial stale relation-count assertions were corrected; final selected rerun passes; repeated, module and dynamic calls and cross-module exact call range tested; reopen 15.6 s |
| Compiler-input governance (`fe51b1bd`) and committed call work | `just governance-scan`, root library Clippy, docs/whitespace checks | 30 rule cases and scan pass. Configured compiler/extractor readers have narrow exceptions; workspace source capture rules remain. Clippy completes with the existing warning backlog, not strict cleanliness |
| Historical processing extension, before public call wiring | `just root-test-incremental -E 'test(processing_scope) \| test(pragmatic_python_semantics_publish_real_call_targets)'` with both real provider binaries selected | Four pass: three scope tests and real Python publication, 6.59 s runtime |
| Call-query continuation | Installed Python/call/reopen and mixed Rust/declaration/call tests; 16 focused scope/recipe/ingress/implicit-call tests; five final scope/implicit-call/reopen tests | Pass; full traversal, composition and live updates remain open |
| Declaration kinds/public subjects (`1a60e748`) | Installed Python kinds/fact subjects, guard choices, mixed Rust constants/statics/facts, exact reopen; adapter fast and focused root checks | Pass: 12.95 s Python, 11.04 s guard, 63.91 s Rust, 17.94 s reopen; 95 adapter and 15 focused root cases; default root check and affected Clippy complete |
| Lexical references (`5964e5ff`) | Installed Python complete/partial/reference/reopen and mixed Rust calls/declarations/unsupported-reference checks; root default/featureless check; affected Clippy/governance/docs/format | Pass: 30.39 s Python, 64.48 s Rust and three processing cases; Python call/reopen regression 17.49 s; Clippy retains 958 warnings |
| Source/disclosure and richer source contexts (`46e260a9`, `10e577d5`, `33b2c2a5`) | Installed exact-span/disclosure/revocation, function definition/body live/clean and surrounding-line/reopen scenarios | Pass: 21.04 s source/revocation, 206.25 s function/body, 33.70 s lines/hard limits; remaining subjects/directives stay open |
| Decoding/columns/raw paths (`55e50782`, `d07d81e4`, `4516a5a7`, `fcd62fd9`) | Real source/call/live/clean/reopen scenarios and strict provider checks | Pass: mixed decoding 203.40 s, text columns 202.48/204.34 s, Python paths 81.85 s, Rust paths 138.00 s; further codecs/argv/rename behavior open |
| Python configuration/roots/stubs (`1c913767`, `6e339a3e`, `38629d50`) | Installed independent clean/live comparison with configuration changes and source/stub/root replacement | Pass: 165.86/72.87/73.85 s respectively; external distributions/roots and full semantic output open |
| Rust custom build/caller/platform/toolchain/linkage/selections (`41438e2e` through `09988b9d`) | Actual contained targets, installed live/clean/reopen fixtures, typed scope/schema/adapter checks | Selected custom build, exact caller scope, platform flags, shared toolchain capture, linkage and feature/profile cases pass; feature/profile final scenario 192.07 s, rerun after source-first startup 212.76 s; complete generated/external/unit closure open |
| Live lifecycle and source-first fresh startup (`a6d8569a`, `99b77ec0`, `56d2a36d`) | Running mixed daemon, independent clean comparisons, current barriers, obsolete completion and source-only restart | Final staged scenario 186.06 s; after primary diagnostics 188.20 s; public source precedes semantic successor; retained providers/full corpus open |
| Retained processing continuation (`730a346d`) | Installed 130-partition paging through restart and a live successor, invalid ranges/blocks/released handles | Pass 53.38 s, later source-first regression 74.63 s; all-family/efficient status open |
| Primary Rust diagnostics (`f1e44d80`) | Two contained compiler cases, mixed target/context, all-failed startup, live/clean repair and exact failed-epoch reopen; Rust service/launcher checks | Pass: failed-no-MIR 8.12 s, successful warning 8.21 s, mixed contexts 76.48 s, all-failed 48.36 s, live/clean/reopen 374.04 s; 18 service and 19 extractor tests; tooling 216; no containment weakening |
| Typed Rust diagnostic details (`4cc74d7c`) | `just extractor-check`, `just extractor-test`, `just extractor-identity`; provider/schema cases; final four-case contained/installed native selection; `just root-check`, `just governance`, affected Clippy | Pass: 20 extractor and ten provider/schema tests; successful warning 7.80 s, failed-no-MIR 7.71 s, all-failed startup 58.77 s, live/clean/repair/exact reopen 350.01 s; 988 existing Clippy warnings, no new findings; no full-suite or cross-upgrade migration claim |

Local observations for resumption include `/tmp/codefabric-call-processing-tests.log`,
`/tmp/codefabric-public-calls-check.log`, `/tmp/codefabric-python-canonical-calls-final-regression.log`
and `/tmp/codefabric-python-call-anchors-*`. The current slices also retain
`/tmp/codefabric-outcomes-declarations-final.log`, `/tmp/codefabric-outcomes-references-final.log`
and `/tmp/codefabric-outcomes-references-check-final.log`. They are optional local logs, not required runtime
artifacts or a new certification mechanism. Git and named behavioral tests retain the useful history.

The original four golden scenarios passed during earlier slices (startup, installed Python
serving, exact reopen and cancellation). Exact reopen was rerun on the call-query slice (16.65 s);
these scenarios do not exercise full outcomes 4–8. The diagnostic-detail checkpoint retained 952
library/36 integration Clippy warnings (988 total), with no new findings in that slice. Strict lint
remains open; that historical check was not a `-D warnings` pass. The last older aggregate root result at `0cc7242`
reported 1,038 passed, 13 failed and two skipped; it is historical, not a current verdict. No new
four-domain aggregate, full-root green result or universal product completion is claimed here.

Final diagnostic logs remain locally under `/tmp/codefabric-diagnostic-details-*`: `native2.log`
contains the four final behavior checks; `extractor-tests2.log` contains the 20 tests;
`root2.log`, `clippy2.jsonl` and `governance1.log` contain the integrated static evidence.
`native1.log` contains the ten provider/schema cases plus the two earlier contained runs.
The previous `/tmp/codefabric-rust-diagnostics-*` logs belong to `f1e44d80`.

The final native selection used the two provider binary paths described below and a temporary
`systemd-run --user --scope --quiet --property=Delegate=yes env` scope, followed by:

```sh
just root-test-incremental -E 'test(mixed_live_updates_equal_independent_clean_public_queries) | test(pragmatic_all_rust_targets_failed_retains_diagnostics_and_source) | test(contained_cargo_extracts_real_selected_rust_call) | test(contained_cargo_retains_structured_diagnostics_without_mir)' --test-threads 2 --no-tests=fail
```

Affected Clippy used `./scripts/cargo-check-mode.sh cargo clippy --locked --lib --test integration
--message-format=json`; the comparison to the preceding 988-warning candidate found no new findings.
Changed Rust formatting, new-text spelling, document navigation and `git diff --check` pass.
At that diagnostic checkpoint, its validation processes and fixture daemons had finished. A separate
terminal-owned nextest run was left untouched and its results are not included here. The current
user-requested stopping point is the P04 checkpoint above.

## Runtime profile and completed foundations

The [nonproduction preparation plan](docs/plans/codefabric_pragmatic_delivery_nonproduction_preparation_plan_2026-09-08.md)
was completed in `79c5d52`: pragmatic skills/instructions, selected design alignment, retired process
machinery, focused command/CI/environment checks, and independent product fixture/tooling preparation.
That readiness did not establish production completion; the startup failure recorded during preparation
was subsequently repaired in outcome 1.

Preparation and outcomes 1–3 remain implemented for the Linux workflow: owned control/candidate
publication, exact activation/readback/reconciliation, removal of the generalized proof-program and
allocation-receipt paths, one-pass catalog/schema validation, real execution bounds and joined cleanup.
Useful exact histories, provider coverage, source identity and operation outcomes remain. Historical
`proof_receipt` compatibility fields do not reinstate a proof evaluator. Previous source-read/governance
findings are resolved. The earlier exact uv reconciliation (`fe5615f`) is superseded by the system
uv policy (`a644b295`); uv 0.12.13 is the installed executable at this checkpoint.

The workstation profile is 16 physical cores/32 threads and 192 GB RAM: 64 GiB managed workspace budget,
32 GiB shared DataFusion pool, 16 data workers/partitions, RSS pause/resume at 112/96 GiB, system-memory
headroom up to 16 GiB, and a 128 GiB shared disk allowance including 64 GiB spill. These replace the old
3/2.5 GiB RSS thresholds. Actual free disk and physical process memory matter; reservations are not RSS.
Linux containment combines Bubblewrap, seccomp and cgroups. Provider cgroups do not impose a one-core
quota or virtual-address-space limit. Pyrefly uses 16 checker threads, two transport workers, up to
16 blocking workers and a negotiated 16 GiB memory profile. Other platforms must report unavailable
observations/containment honestly; sampling does not guarantee immunity from OOM.

Current source/transport ceilings: 1 GiB mixed captured source set; Pyrefly 16,384 modules, 64 descriptors
per chunk, 32 MiB per file, 512 MiB source bytes and 32 MiB aggregate descriptors per run; individual RPC
frames remain 4 MiB. Context opening remains unary/bounded and external roots remain unfinished. These
are configured ceilings, not measured optimal workload sizes. Dependency blobs avoid repeated sysroot
disk copies; one charged immutable toolchain bundle now survives compatible semantic passes with
owned idle/headroom eviction. The two provider executable selections are observed across live work
and reopen. Full tool/library/dependency validity, retained Cargo target/fact state, coordinated
all-owner reclamation and representative cost validation still need work.

Use self-contained `just` recipes; keep stable/sidecar shared `target/` and the extractor's separate
dated-nightly target. Real root provider tests require current `CODEFABRIC_RUSTC_EXTRACTOR_BIN` and
`CODEFABRIC_PYREFLY_SIDECAR_BIN`. Rebuild a changed sidecar through the repository shell or the existing
golden setup; `just sidecar-check` checks/lints rather than installing a fresh executable. No routine
`cargo clean`, independent worktrees, source-edit artifacts or new approval cycle is required.

The final handoff documentation passes `just docs-check STATUS.md
 docs/plans/codefabric_pragmatic_production_outcomes_4_8_detailed_implementation_plan_2026-09-09.md
 tooling/product/README.md` (three files, zero navigation errors), focused `typos` and
`git diff --check`. The plan preserves all 25 slice headings, all 29 ontology requirements and the
D1–D7/E01–E26/P01–P14 registers. The navigation log is
`/tmp/codefabric-p04-stopping-point-docs.log`. Only the two handoff documents change after the
validated implementation commit `722b57d4`.

## Package handoff — P04 active, P05 next

**Execution resumed at the user's request on 2026-09-11.** The provider executable invalidation
checkpoint `722b57d4` and subsequent native ownership repairs remain implemented. Complete P04's
remaining inclusion/validity, retained Cargo fact state, scheduling and reuse/ownership boundaries,
then P05's first-release corpus. The detailed plan §3.3 and §10 retain the full progression.

Current execution order:

1. Continue **P04**: complete captured source/Git/external inclusion and recovery topology; complete
   positive/negative/tool/sysroot/linker/runtime dependency validity and owner manifests; establish
   native Cargo unit/fact census before retaining target outputs; allocate shared native CPU shares
   and bounded fair context backlog; extend nonempty owner reuse and remaining retention ownership.
2. Complete **P05**: assemble the first-release external/config/negative/deletion/race corpus with
   independent clean comparisons, restart and leased old source/facts. P03's initial query boundary
   is delivered; the first useful release is still open.
3. Continue **P06–P11**: complete language normalization, Python control/advanced analyses, Rust
   MIR/private analyses, common graphs/summaries, all eight query forms/full DAG semantics and modern
   delivery. Each new family must become publicly queryable with actual coverage and update cases.
4. Start **P12** native maintenance/finite retention after P04's ownership prerequisites and before
   large full-ontology corpora; finish **P13–P14** integrated recovery and representative optimization.
   Keep optional overlays/CDF/Rayon/orjson contingent on a demonstrated consumer/cost benefit.

Do not recreate delivered parser/checker/toolchain-input retention, scoped exact pin reuse,
pruned source/Git metadata watching, first-four typed composition or source-context meanings.
Retained Cargo outputs are different from retained immutable toolchain inputs: Cargo `Fresh` can
skip the fact-producing wrapper, so output reuse needs complete unit/fact admission. Existing
job-count admission is also different from shared native CPU allocation; Pyrefly still uses 16
checker threads and Rust targets are serialized.

The unresolved native Delta executor/cleanup cascade in
`/tmp/codefabric-p04-toolchain-cache-native-final.log` (nextest
`efd361ae-de4d-4363-90c0-898b5c1e50c9`) remains a P04/P13 recovery item. Subsequent targeted
publication/client-timeout/shutdown/reopen cases pass without reproducing it; they do not prove a
fix. Full poll-backend thread joining, all-owner reclamation and other-platform deployment recovery
also remain open. No new full-root/four-domain aggregate, destructive vacuum, sustained-retention or
representative performance acceptance is claimed at this stopping point.
