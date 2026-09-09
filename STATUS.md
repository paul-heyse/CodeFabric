# CodeFabric status

Updated 2026-09-09 from the canonical `/home/paul/CodeFabric` working tree on `master`.
Last production commit: `734821db` (`Publish canonical Python call occurrences and checker-resolved edges`).
The uncommitted continuation is identified below; documentation updates do not certify that code.

## Current handoff

**Outcomes 1–3 are implemented for the current Linux workflow. Outcomes 4 and 5 are partially
implemented. Outcomes 6–8 remain open, with reusable infrastructure and some prerequisite work
already present. No outcome from 4 through 8 is complete.**

Follow the [production backlog](docs/plans/codefabric_pragmatic_production_implementation_plan.md)
and its [detailed outcomes 4–8 execution plan](docs/plans/codefabric_pragmatic_production_outcomes_4_8_detailed_implementation_plan_2026-09-09.md).
The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md)
and [selected design](docs/spec_index/README.md) retain the full Python/Rust CPG, all eight forms,
composition, truthful incomplete scope and sustained operation. The detailed plan now separates
implemented portions, unfinished acceptance and the next work for every slice.

Fresh daemon startup captures real Python/Rust inputs, runs contained semantic providers and
publishes exact Delta versions. Canonical declarations, Python lexical references and Python/Rust
call occurrences exist. Installed FastMCP clients have exercised function search and declaration
fact retrieval with scoped processing and observed result truncation. Full public call traversal
has **not** been demonstrated. A running daemon does **not yet** continuously update the graph;
startup still waits for semantic work before publication. The first useful release remains open.

All work in this implementation has used the canonical tree and small commits. No new independent
worktrees or subagents were created. At this checkpoint no implementation build/test process remains
running. Existing production edits are preserved while the user-requested documentation update is made.

## Uncommitted continuation: call scope and public call queries

There are two successive validation levels in the current source changes:

1. **Call-specific processing: implemented and focused tests passed, not committed.**
   Requested provider/input partitions are retained in `system.requested_processing_scope`.
   A native DataFusion transformation produces `system.entity_processing_scope`, keeping
   `function-declarations` separate from `call-targets`. Call partitions combine terminal Ruff,
   Pyrefly or rustc family coverage with unresolved targets, absent source/caller identities and
   Pyrefly calls without explicit Ruff callee syntax. Semantic gaps can mark a completed provider
   partition partial without changing declaration completeness. The implicit-call detection path
   exists, but a dedicated property/decorator scenario has not exercised it.
2. **Public call-query wiring: implemented to compile, behavioral validation pending, not committed.**
   `fact.code_call_selector` uses native joins to canonical entities and incoming/outgoing projections.
   `query.result.call-facts` reuses the declaration query's typed subject semi join. The recipe accepts
   exact canonical entity subjects, `calls`/`call relationships`, incoming/outgoing direction and
   explicit `one relationship step`/`one step` distance. Omitted direction projects as outgoing.
   Public endpoint/call-site IDs, deterministic ordering, language/context scope and call-family
   processing are wired. These are intended behaviors awaiting execution through the real service.

Changed surfaces: startup `processing.rs`, `canonical.rs`, `canonical/processing.rs` (new),
`canonical/call_selector.rs` (new), `canonical/python_calls.rs`, startup assembly,
`src/fabric/processing_status.rs`, `programmatic_ingress_port.rs`, `programmatic_query_backend.rs`,
`src/production_query_recipe.rs`, `production_query_recipe/facts.rs` and the daemon integration test.

Four focused tests passed before the call-selector/query changes: three processing-scope tests and
`pragmatic_python_semantics_publish_real_call_targets` (6.59 seconds). They verify family isolation,
remainder paging/context handling and real Python declaration-complete/call-partial publication.
The subsequent whole dirty library passes `just root-check-fast` (66 warnings; 7.13 seconds).
**That check does not execute DataFusion plans or validate the new public form.** No test or Clippy
result is claimed for the final call-selector/query changes. Resume here, preserving both new files.

## Outcome 4: real inputs, canonical facts and the first four forms

### 4A — Rust contexts and production compilation: partial

Implemented in `eba6f19a`, `baf533f4`, `52477365` and `b6a7d777`:

- Contained locked/offline Cargo metadata and compilation during fresh startup, bound to captured
  manifests, source generation, context, compiler/sysroot and the extractor source manifest.
- Captured path dependencies, multiple compilation units, package and virtual workspaces with
  inherited package settings; libraries, binaries, examples, tests and benchmark targets.
- Distinct target contexts and sequential execution. A failed target retains other targets' facts
  and records its unavailable state in `system.rust_target_progress`.
- Reusable immutable dependency/sysroot blobs shared between provider views. Mutable installed
  files are copied and verified before reuse; per-edit full sysroot disk copies are avoided.
- Raw compiler Arrow publication and per-family partial admission. Missing units remain unknown;
  excess units or substituted source/context pins are rejected. Target `processed` means output
  returned, not that every compiler family or semantic proposition is complete.

Remaining: registry/git dependency materialization; build-script/proc-macro and generated `OUT_DIR`
input closure/source mapping; effective Cargo configuration/environment and selectable feature,
profile/target combinations; host-versus-target build separation; retained compatible compiler
build caches; bounded parallel context scheduling; byte-safe compiler path handling; structured
compiler diagnostics; update-time invalidation, cancellation and obsolete-completion scenarios.

### 4B — Python contexts and semantic extraction: partial

Contained Pyrefly startup honors selected Python version/platform and uses the pinned embedded
bundles. Its private input view verifies captured bytes, no-follow regular files and exact source
bindings before checker mutation. Both direct and contained real-process tests pass.

`1301df5a` removes the old 64-module ceiling with ordered descriptor chunks under one complete
checker inventory. Sequence/end counts, duplicate/missing members, deadline/cancellation and input
bounds are checked before run acceptance. The producer and uploader advance together over a bounded
channel; chunks do not create independent checkers. A real 70-module daemon fixture resolves
`extra_69.chosen` among 69 distinct same-name function declarations.

`92bb153d` extends the selected Pyrefly Query seam with checker-selected definition coordinates.
Function metadata resolves through the definition index into the target module's declaration;
imported aliases and bound methods are tested. The sidecar maps coordinates to captured file/digest
pins, and the daemon independently validates file, digest and range. Synthesized/unavailable or
out-of-inventory definitions remain explicit gaps; qualified display names are not identity.

Remaining: project configuration and ordered external import roots; namespaces/re-exports and
`.pyi` precedence across dependencies; external distribution/stub materialization and identity;
canonical structural type/member/import/reference output and all declared/computed/expected/narrowed
propositions; full overload/descriptor/decorator semantics; retained checker updates and context
invalidation. Current bulk raw output and selected call resolution do not close those families.

### 4C — source and syntax: partial

`11a61909` publishes six Rust Tree-sitter source-context relations independently of successful
compilation, including exact-byte CST recovery for malformed Unicode/CRLF source. Rust schemas do
not invent Python version fields. Python retains syntax/parse diagnostics while failed Ruff semantic
families report unknown coverage. `734821db` adds owned Ruff callable, call-site and callable-syntax
observations: native syntax has 28 relations total, including 22 Ruff relations. Provider-local
syntax/binding IDs remain observations within their admitted source/context/run.

Remaining: full source/lexical/CST feature census; retained parsers/query packs and incremental trees;
complete trivia/index/coordinate handling; non-identity decoded source mappings (for example BOM or
non-UTF-8 Python encodings); reversible compiler paths and source presentation; rename/case-collision
semantics; incomplete-edit behavior during actual live updates. No source-context public form is
implemented for the canonical production release yet.

### 4D — canonical normalization: partial

The committed canonical catalog includes:

| Relation | Implemented behavior and limits |
|---|---|
| `source.code_file` | Captured input identity, raw path bytes, content/generation and capture disposition |
| `fact.code_entity`, `fact.code_declaration` | Python bindings and Rust stable compiler keys mapped through application identity recipes; declarations retain separate occurrence identity, exact source range, context and provenance |
| `fact.code_entity_selector` | Canonical function selectors for Python, Rust or both |
| `fact.code_reference` | Python lexical read/write occurrences joined to bindings/declarations through exact source/context/run pins; unresolved targets retained; project-aware semantic and Rust references remain open |
| `fact.code_call_site` | Python and Rust call occurrences, caller/target identity when established, resolution/dispatch, exact or explicitly unavailable source mapping, raw provider provenance |

Native DataFusion joins/projections construct these relations. Rust uses actual stable crate/definition
keys and kind; missing stable keys yield identity gaps. Direct calls, repeated same-callee occurrences,
function-pointer unknowns, captured dependency targets and macro/lowered source-mapping gaps are tested.
Python uses exact Ruff caller/call syntax and checker-selected target definitions. Repeated calls remain
distinct; dynamic targets remain unknown. Module/lambda calls retain application-owned source occurrences,
with unavailable public caller entities explicit. Implicit property/decorator calls are not fully normalized.

`d7487f39` permits a native non-null refinement of a declared nullable field while rejecting the unsafe
reverse; buffers and schema metadata remain intact. Exact Delta reopen restores logical fixed-width IDs
and numeric types from storage representations. `068e8fd4` fixes empty metadata-rich IPC schema validation
to use the admitted allocation bound instead of encoded page length.

Remaining: full module/class/lambda/callable entities; semantic imports/exports and references for both
languages; canonical structural types and propositions; members/signatures/argument binding; complete
candidate/dispatch and executable-instance relations; external endpoints and generated/lowered correspondence;
full per-proposition authority/conflict retention; identity continuity and owner replacement under edits.
Raw provider coverage is not complete canonical-family coverage.

### 4E — public query forms: partial

| Form | Demonstrated current behavior | Remaining |
|---|---|---|
| FindEntities | Installed client returns canonical Python/Rust functions; language/context filters precede limits; stable name/entity ordering | Other kinds/representations, source boundaries, semantic name/ambiguity resolution and full directives |
| RetrieveFacts | Explicit canonical entity IDs; `declarations` or `declaration locations and provenance`; native semi join prevents repeated subjects duplicating occurrences; partial and empty cases tested | Types, members, call/derived families, point filters, broad family expansion and phrase/fact/prior-result resolution |
| FollowRelationships | Canonical call data is committed; one-step public call recipe is uncommitted and compile-checked only | Execute/fix new call path; references/imports, candidates, full direction/distance/stop/filter behavior and composition |
| RetrieveSourceContext | Existing source storage/lease infrastructure only | Canonical production form, exact selected bytes after disk changes, separate disclosure authorization, coordinate and truncation delivery |

Unsupported subject meanings are explicitly rejected; they do not fall back to names. The generalized
pragmatic expectation corpus is not fully connected to all public forms. The static four-form mixed-language
acceptance and the live first-useful-release acceptance are both still open.

## Outcome 5: processing, incomplete scope and freshness

### 5A — requested/query scope: partial

`system.provider_run_scope` and `system.provider_family_progress` retain requested inputs, provider/context
pins and terminal family state. Committed entity processing uses requested Python files and selected Cargo
targets, independently of emitted fact rows. Public function/declaration queries select language/context
before limits and summarize corresponding partitions. Failed Rust targets do not make a Python-only query
incomplete. Missing capture, unsupported work, failure, deadline, cancellation and resource bounds retain
reason categories; coverage and row truncation are distinct.

The first remainder page is bounded to 64 rows with `next_offset`; raw path bytes, optional display path,
target/kind and known context survive projection. The summary is retained with the exact result package.
The uncommitted call-specific extension is described above and conservatively includes the selected
context's potential callers. It does not yet derive exact owner/reverse-dependency scope.

Remaining: all family/owner dimensions; public remainder pagination; authorization-scoped efficient status
scans; incoming reference/import and negative dependency/frontier propagation; shared live pending/running
state; provider precision and actionable retry details; a clean distinction between terminal semantic
unknowns and runnable pending work for convergence. Empty results alone never establish complete absence.

### 5B — wire delivered in part; freshness barriers open

`b865c6b2` adds typed Protobuf processing fields, strict Pydantic projections and retained manifest/reopen
support. Presence distinguishes unobserved result exhaustion from observed false. Streaming observes one
authorized lookahead row, seals only N requested rows and reports actual truncation; at a grant ceiling,
exactly N rows leave exhaustion unknown. `e65bdbeb` fixes released diagnostic enum projection in the adapter.

Remaining: actual selection/barriers for `best_available_snapshot`, `await_latest`,
`require_current_for_targets`, `require_source_current` and `require_semantic_current`; historical/newer
workspace observations; public pagination; generation/context/family convergence waits and compile-failure
quiescence. The backend still supplies `FreshnessState::Current` on the inspected path; this does not
implement those policies or prove currentness against live disk edits. Strict freshness is a priority
before claiming continuous or fully current service behavior.

## Outcome 6: continuous updates — open

| Slice | Existing foundation | Remaining delivery |
|---|---|---|
| 6A | Secure capture and repository/source-wave components | Daemon-owned notify/gix input loop; watch-before-census; bounded dirty queue/coalescing; overflow/rescan, polling, rename/delete/root/config recovery |
| 6B | Source/context identities, exact admitted provider pins and owner/command seams | Immediate semantic invalidation; conservative context dependencies including negative imports; deletion/replacement; generation fences and stale-completion rejection during updates |
| 6C | Working startup composition, exact version vectors and immutable dependency blobs | Reuse startup for updates; source/syntax publication before semantics; retained Tree-sitter/Pyrefly/Cargo state; changed-version reuse and fair scheduling |
| 6D | Prepared independent expectations, edits and comparison helpers | Real persistent-daemon versus independent-clean adapters, observable quiet convergence and semantic/identity/coverage comparisons over actual edit sequences |

No startup-versus-restart test is being counted as live incremental convergence. Python canonical identity
continuity across unrelated edits also needs the actual incremental/clean corpus; current identities are
not evidence that all continuity requirements hold.

## Outcome 7: full analyses and all eight forms — open

| Slice | Implemented prerequisite | Remaining delivery |
|---|---|---|
| 7A Python language semantics | Owned Ruff bindings/references/call syntax and selected Pyrefly call definition anchors | Complete scope/binding/import/type/member/call/decorator/pattern/comprehension and dynamic-semantics rows, canonical consumers and invalidation |
| 7B Python CFG/dataflow | Typed analysis code and prepared source expectations | Correct owner-scoped control/evaluation semantics, normal/exception/cleanup/suspend edges, reaching definitions/liveness and real production input wiring; replace ordinal/sequential approximations |
| 7C Python advanced state | Existing analysis structures | Finite memory/points-to, effects/resources/exceptions, capture/generator/async/concurrency and unknown propagation, built on 7B |
| 7D Rust source/types/MIR | Real typed compiler publication, stable declaration keys and selected canonical calls | Full types/generics/traits/instances/MIR payloads, macro/hygiene/generated spans, coroutine/CTFE/FFI facts, structured diagnostics and canonical/public coverage |
| 7E Rust derived/private borrow | Existing MIR analysis modules and contained compiler seam | Real typed inputs, finite dataflow/state/ownership analyses, exact private loans/regions, drop/unwind/coroutine and changed-body replacement |
| 7F Common graphs/summaries | Existing petgraph/analysis integration and canonical calls | Demand-rooted projections, correct dominance/SCC/reachability, structural facts and bounded interprocedural fixpoints with precision/frontier scope |
| 7G Complete forms/composition | Eight-form request/ingress infrastructure; two limited public forms, third in progress | FindPaths, MatchPattern, Compare and Summarize; finish first four; real typed multi-block DAGs, fan-out/fan-in, repeated forms, references, authorization, negatives, ordering/limits and cancellation |
| 7H Modern presentation | Installed FastMCP transport/resources, guarded-input scenarios, typed processing and diagnostic correction | All-form presentation, full paging/cursors and source permissions; replay/expiry/reconnect/slow-reader/TTL integration for new workflows; one consistent daemon-authored response |

Substantial existing algorithms and fixtures are reusable, but fixture-fed or schema-only families are
not delivered production analyses. Every row in the detailed plan's full ontology coverage map remains
required through canonical facts, public retrieval, precision/unknowns and update replacement. Dynamic
unknowns are legitimate terminal facts; unfinished implementation is a separate remaining task.

## Outcome 8: sustained operation — open

| Slice | Existing foundation/progress | Remaining delivery |
|---|---|---|
| 8A Persistence | Exact Delta publication/reopen, removed proof-only histories, immutable provider-input blob reuse | Consumer-based durable/cache split, unchanged version/owner reuse, fewer redundant history/intermediate writes and measured growth over edits |
| 8B Maintenance | Native checkpoints and retention-aware vacuum dry runs | Native compaction and destructive vacuum remain unavailable; fix actual commit-properties/transaction/retry seams, replace `AtomicVacuumApprovalBinding` with writer/reader ownership, protect files plus reconstruction logs and test real reclaim/recovery |
| 8C Retention | Store budgets, headroom and existing leases/handles | Coordinated history/result/source/context/build/cache/diagnostic TTL and eviction; maintenance scheduling; finite retained state through real cycles while protecting readers and uncertain writers |
| 8D Recovery | Linux containment, joined subprocess/native cleanup, exact restart and focused cancellation/lost-ack tests | Failures/races introduced by updates, retained providers, new forms and native maintenance; pressure/expiry recovery; explicit unsupported behavior for unimplemented deployment profiles |
| 8E Measurement | RSS/cgroup/headroom signals, benchmark harness and small real scenario timings | Correlated phase metrics, representative small/medium/large and real-repository CPG workloads, distributions, convergence/first-batch/retention/recovery measurements |
| 8F Performance | Native canonical joins, valid schema refinement, streaming lookahead and shared immutable input storage | Safe scan pushdown/statistics/physical properties, pruning/file-size tuning, workload-based parallelism/caches and measured before/after improvements; overlays/CDF/Rayon/orjson only with a concrete need |

`delta_guarded_maintenance.rs` still returns `OptimizeCommitIdentityAndRetryControl` and
`AtomicVacuumApprovalBinding`. No optimize commit or destructive vacuum/reclamation is claimed.
No representative benchmark or sustained bounded-storage acceptance has been completed.

## Validation and limits at this checkpoint

These are attributable implementation runs from 2026-09-09, not tests rerun for this documentation
refresh. Counts below overlap; they must not be added into a full-suite total.

| Code scope | Command/observation | Result and boundary |
|---|---|---|
| Through canonical Rust calls (`45421cff`) | Focused canonical/provider tests and real mixed/path-dependency daemon scenarios | Direct, indirect, repeated and macro/unmapped calls; exact installed restart pass; seven final selected cases pass |
| Public declaration facts (`64ce1acf`) plus IPC/diagnostic corrections | Focused query/scope/resource tests, installed mixed-client query, exact restart | 42 selected tests pass; repeated subjects, failed Rust target, completed empty Python scope and unsupported references exercised; exact reopen about 15.2 s |
| Chunked Python inventory (`1301df5a`) | 16 selected root Pyrefly/daemon tests; sidecar check/tests; protocol and adapter checks | Pass, including contained cross-chunk imports and 70-module fresh Delta publication; 29 sidecar and 95 adapter tests, adapter lint/types and `just proto-check` pass |
| Checker definition anchors (`92bb153d`) | `just sidecar-test`, `just sidecar-check`; selected root provider/recipe/daemon cases | 30 sidecar and 26 root cases pass; imported alias/bound method, wrong file/digest/range and real 70-module target anchors |
| Canonical Python calls (`734821db`) | Affected native/canonical/recipe tests plus real Python/mixed Rust and installed restart | Initial stale relation-count assertions were corrected; final selected rerun passes; repeated, module and dynamic calls and cross-module exact call range tested; reopen 15.6 s |
| Compiler-input governance (`fe51b1bd`) and committed call work | `just governance-scan`, root library Clippy, docs/whitespace checks | 30 rule cases and scan pass. Configured compiler/extractor readers have narrow exceptions; workspace source capture rules remain. Clippy completes with the existing warning backlog, not strict cleanliness |
| Uncommitted processing extension, before public call wiring | `just root-test-incremental -E 'test(processing_scope) \| test(pragmatic_python_semantics_publish_real_call_targets)'` with both real provider binaries selected | Four pass: three scope tests and real Python publication, 6.59 s runtime |
| Final uncommitted call-query library | `just root-check-fast` | Pass, 66 existing warnings; public call execution, affected tests and Clippy still pending |

Local observations for resumption include `/tmp/codefabric-call-processing-tests.log`,
`/tmp/codefabric-public-calls-check.log`, `/tmp/codefabric-python-canonical-calls-final-regression.log`
and `/tmp/codefabric-python-call-anchors-*`. They are optional local logs, not required runtime
artifacts or a new certification mechanism. Git and named behavioral tests retain the useful history.

The existing four golden scenarios have passed during earlier slices (startup, installed Python
serving, exact reopen and cancellation). They were not rerun on the final dirty call-query code and do
not exercise full outcomes 4–8. Root Clippy retains a large warning backlog (958 in the recent recorded
library run after new warnings were addressed). The last older aggregate root result at `0cc7242`
reported 1,038 passed, 13 failed and two skipped; it is historical, not a current verdict. No new
four-domain aggregate, full-root green result or universal product completion is claimed here.

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
findings are resolved; uv is aligned to the actual host update, 0.12.11 (`fe5615f`).

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
disk copies, but toolchain verification cost, retained build state and cache reclamation still need work.

Use self-contained `just` recipes; keep stable/sidecar shared `target/` and the extractor's separate
dated-nightly target. Real root provider tests require current `CODEFABRIC_RUSTC_EXTRACTOR_BIN` and
`CODEFABRIC_PYREFLY_SIDECAR_BIN`. Rebuild a changed sidecar through the repository shell or the existing
golden setup; `just sidecar-check` checks/lints rather than installing a fresh executable. No routine
`cargo clean`, independent worktrees, source-edit artifacts or new approval cycle is required.

## Next action

1. Finish the preserved call-query continuation: exercise real installed-client incoming/outgoing,
   repeated-subject, unknown-target, empty/partial, direction-default, distance/limit and language/context
   cases; verify query compilation, exact reopen and declaration-query regressions; fix issues, run
   affected Clippy/governance, then commit the coherent slice. Do not treat the current compile pass as
   completion of FollowRelationships.
2. Finish 4A–4D effective/external/generated inputs and canonical families needed by the first four forms;
   complete 4E and 5A–5B, especially public remainder paging and real freshness barriers. Retain the broad
   workstation allowances and honest partial semantics.
3. Connect 6A–6D continuous observation, invalidation, two-speed publication, retained providers and the
   independent clean/incremental corpus. Only then claim the first useful release.
4. Continue 7A–7H and 8A–8F to the full plan acceptance: every family and all forms/composition, finite
   retention, actual native maintenance, recovery and representative performance. Add phase metrics and
   persistence improvements while integrating updates; optional performance mechanisms remain conditional.

The user's current request is to reconcile these documents. Production implementation remains authorized
but is left at the explicit checkpoint above for the next implementation turn.
