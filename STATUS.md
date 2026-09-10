# CodeFabric status

Updated 2026-09-09 from the canonical `/home/paul/CodeFabric` working tree on `master`.
Last production commit: `09988b9d` (`Apply captured Cargo feature and profile selections with typed failure scope`);
source-first fresh startup is the current implementation slice.
The completed query slices and their validation are recorded below.

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

Fresh daemon startup captures real Python/Rust inputs and publishes exact source/syntax Delta
versions before readiness. The owned update coordinator runs contained semantic providers and
publishes their exact successor. Canonical declarations, Python lexical references and Python/Rust
call occurrences exist. Installed FastMCP clients have exercised function search and declaration
fact retrieval, one-step incoming/outgoing call traversal and Python lexical-reference traversal with scoped processing and observed
result truncation. Exact declaration source spans now pass through the same client with independent
disclosure authorization. Python call/source queries also pass after exact reopen. A running daemon now reconciles Python edits,
additions, deletions and atomic saves, with current-source query barriers and exact successor epochs.
Live updates now publish source/syntax with semantic pending scope before their semantic successor.
Fresh startup now uses this source-first path too. The first useful release remains open.

The call-query continuation present at session start was preserved, exercised and committed in
`80bc6d18`; declaration kinds/public subjects followed in `1a60e748`, and lexical references in
`5964e5ff`. The full requested implementation remains unfinished. No outcome from 4 through 8 is closed.

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
names; Rust spans retain the compiler's declaration range. Surrounding syntax/body expansion is open.

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

The current change admits captured project configuration when the effective manifest accounts for
all checker settings. Version, platform and ordered workspace search paths are installed from that
manifest; artifact digests remain part of context identity. Unapplied checker settings, unmaterialized
project dependencies and older configurations without a setting census remain unavailable. No ambient
interpreter, imports or checker configuration are consulted. The installed live/clean scenario passes Python 3.14-to-3.12 and Linux-to-Windows selections,
previously missing import creation/deletion, unsupported-setting invalidation and restoration of the
original source/context identities (165.86 s on 2026-09-09). Both call occurrences survive missing
imports; unrelated resolved calls remain usable. Unsupported checker configuration qualifies the
whole selected context. Twelve root discovery/capture tests, all 31 sidecar tests and 194 tooling
tests pass. Sidecar strict lint, default/featureless root checks, governance, docs navigation and
changed-file formatting pass. Root library Clippy retains 955 baseline warnings with no added findings.
`just golden --case python-context-live` selects the installed scenario.

The current sidecar change owns retained modules by input/file identity rather than import name.
A source/stub pair and same-name modules in ordered roots can be checked together. Duplicate file/input
identities and substituted file/name/path bindings are rejected before checker mutation. Deletion
reconstructs the checker so a removed stub cannot survive in its private handles. All 32 sidecar tests
pass, including exact stub target-file selection, deletion/recreation, fully captured ordered roots
and identity-substitution rejection before mutation. Installed namespace/source/stub live/clean
acceptance passes (72.87 s on 2026-09-09): same-name declarations remain distinct, calls select the
stub's exact source owner, removal selects the implementation and recreation restores the original
identities. `just golden --case python-stubs-live` selects it. Sidecar strict checking/lint, all 196
tooling tests, tooling lint, governance and changed-file formatting pass. The unchanged root library
retains the previous 955-warning baseline; integration checking/Clippy completes with no findings on this slice's changed lines. External roots, installed
stub bundles and broader package/import behavior remain open.

Configured search roots now retain Python scripts and other captured sources outside those roots
as explicit checker inputs. The fallback input mapping does not add a resolver search path. Installed
live/clean acceptance verifies all four source declarations, competing same-name imported modules,
search-order reversal and exact restoration (73.85 s on 2026-09-09). An otherwise shadowing root-level
module remains queryable without overriding either configured import root. Twelve context tests and
198 tooling tests pass; default/featureless root checks, tooling lint, governance, docs navigation
and changed-file formatting pass. Library Clippy retains the same 955 warning baseline.
`just golden --case python-roots-live` selects the installed case. Full external roots and package
mapping remain open.

Remaining: additional project configuration settings and ordered external import roots; namespaces/re-exports and
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

Python context discovery now keeps raw relative path bytes through input binding, the contained
provider view and file-URI transport. Root-level `__init__.py` files retain a direct input binding.
Non-Unicode inputs use reversible checker labels while exact paths and application file IDs remain
authoritative; this does not establish surrogate-containing import-name resolution. Local modules
with URL metacharacters, spaces and backslashes are retained. The sidecar consumes escaped local
file URIs and rejects remote/query/fragment forms. Its selected Query seam now returns exact
diagnostic module paths beside rendered text, preventing display-path collisions from assigning
one file's error to another.

Installed live/clean path acceptance passes after the diagnostic-owner refinement (81.85 s on
2026-09-09):
raw and replacement-character paths have distinct canonical owners despite colliding lossy display
strings, with exact local call targets and source spans across deletion/recreation. Root initializer
functions also remain queryable. Thirteen context tests, all 34 sidecar tests (including a real type
error under colliding display paths), strict sidecar checking/lint and all 200 tooling tests pass.
Default/featureless root checks, tooling lint, governance, docs navigation and changed-file formatting
pass. Root library Clippy retains the same 955 warning baseline. The installed scenario is selected by
`just golden --case python-paths-live`. Decoded UTF-8/BOM/Latin-1 mappings now pass the separately
recorded mixed source scenario above. Byte-safe Rust compiler input paths remain open.

Remaining: full source/lexical/CST feature census; retained parsers/query packs and incremental trees;
complete trivia/index/coordinate handling and further source codecs; reversible compiler paths
and source presentation; rename/case-collision
semantics; incomplete-edit behavior during actual live updates. Exact declaration-span source
retrieval and exact function definitions/bodies are implemented; broader syntax/line/source-context
selection remains open.

### 4D — canonical normalization: partial

The committed canonical catalog includes:

| Relation | Implemented behavior and limits |
|---|---|
| `source.code_file` | Captured input identity, raw path bytes, content/generation and capture disposition |
| `fact.code_entity`, `fact.code_declaration` | Python bindings and Rust stable compiler keys mapped through application identity recipes; declarations retain separate occurrence identity, exact source range, context and provenance |
| `fact.code_entity_selector` | Canonical declaration-kind selectors and reusable public entity IDs for Python/Rust |
| `fact.code_reference` | Python lexical read/write occurrences joined to bindings/declarations through exact source/context/run pins; unresolved targets retained; project-aware semantic and Rust references remain open |
| `fact.code_relationship_selector` | Public call and lexical-reference witnesses, native subject selection and reusable occurrence/endpoint IDs |
| `fact.code_source_context`, `source.exact_source_bytes` | Canonical declaration source descriptors and captured bytes selected through exact pins; independent live disclosure checks and bounded native output |
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
| FindEntities | Installed client returns canonical functions and selected additional Python/Rust declaration kinds with reusable public IDs; language/context filters precede limits; stable name/entity ordering | Remaining kinds/representations, source boundaries, semantic name/ambiguity resolution and full directives |
| RetrieveFacts | Explicit canonical entity IDs; `declarations` or `declaration locations and provenance`; native semi join prevents repeated subjects duplicating occurrences; partial and empty cases tested | Types, members, call/derived families, point filters, broad family expansion and phrase/fact/prior-result resolution |
| FollowRelationships | Installed Python/Rust one-step calls and Python lexical references, repeated occurrences, scoped unknowns and limits | project-aware semantic references/imports, Rust references, candidates, full direction/distance/stop/filter behavior and composition |
| RetrieveSourceContext | Exact canonical declaration spans, independently authorized captured bytes, Unicode/CRLF coordinates and explicit byte truncation; disk-change/reopen/revocation/empty/mixed-language cases | Broader syntax/line-bound selection, remaining subjects and composition |

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
The validated call-specific extension and lexical-reference scope are described above and conservatively
include the selected context's potential callers or referring files. It does not yet derive exact owner/reverse-dependency scope.

Remaining: all family/owner dimensions; authorization-scoped efficient status
scans; incoming reference/import and negative dependency/frontier propagation; shared live pending/running
state; provider precision and actionable retry details; a clean distinction between terminal semantic
unknowns and runnable pending work for convergence. Empty results alone never establish complete absence.

### 5B — wire delivered in part; freshness barriers open

`b865c6b2` adds typed Protobuf processing fields, strict Pydantic projections and retained manifest/reopen
support. Presence distinguishes unobserved result exhaustion from observed false. Streaming observes one
authorized lookahead row, seals only N requested rows and reports actual truncation; at a grant ceiling,
exactly N rows leave exhaustion unknown. `e65bdbeb` fixes released diagnostic enum projection in the adapter.

Current-source barriers and typed snapshot/workspace observations now run against the live update
owner, as described above. Remaining: target/family-specific current selection, historical selectors,
full generation/context/family convergence with terminal coverage. Public remainder continuation is
implemented for the currently supported scopes, as described above.
Whole-workspace provider completion is the conservative barrier for strict semantic policies;
source-current uses the separate source publication barrier.

## Outcome 6: continuous updates — partial

| Slice | Implemented foundation | Remaining delivery |
|---|---|---|
| 6A | Watch-before-census, owned native watcher, bounded coalesced queue, retained rescan obligation, periodic secure census and public observation/health | Git inclusion, selected external roots, polling profile, root recreation and ignore/config acceptance |
| 6B | Whole-context replacement, monotonic generations, stale observation, changed-input fences, mixed clean comparison and delayed-completion rejection | Broader negative-dependency/config/context and owner-identity coverage |
| 6C | Source/syntax then semantic publication, separate current barriers, exact pending-stage restart and old source-page retention | Retained Tree-sitter/Pyrefly/Cargo state, changed-version reuse and fair scheduling |
| 6D | Persistent Python edits, mixed clean comparison, Python version/platform/negative imports, obsolete completion and pending restart | Full semantic/identity/coverage edit corpus, external inputs and Rust configuration/dependency cases |

The installed live test is distinct from startup-versus-restart validation. The mixed comparison retains canonical
identity and relationships; the wider edit and rename-continuity corpus remains open.

## Outcome 7: full analyses and all eight forms — open

| Slice | Implemented prerequisite | Remaining delivery |
|---|---|---|
| 7A Python language semantics | Owned Ruff bindings/references/call syntax and selected Pyrefly call definition anchors | Complete scope/binding/import/type/member/call/decorator/pattern/comprehension and dynamic-semantics rows, canonical consumers and invalidation |
| 7B Python CFG/dataflow | Typed analysis code and prepared source expectations | Correct owner-scoped control/evaluation semantics, normal/exception/cleanup/suspend edges, reaching definitions/liveness and real production input wiring; replace ordinal/sequential approximations |
| 7C Python advanced state | Existing analysis structures | Finite memory/points-to, effects/resources/exceptions, capture/generator/async/concurrency and unknown propagation, built on 7B |
| 7D Rust source/types/MIR | Real typed compiler publication, stable declaration keys and selected canonical calls | Full types/generics/traits/instances/MIR payloads, macro/hygiene/generated spans, coroutine/CTFE/FFI facts, structured diagnostics and canonical/public coverage |
| 7E Rust derived/private borrow | Existing MIR analysis modules and contained compiler seam | Real typed inputs, finite dataflow/state/ownership analyses, exact private loans/regions, drop/unwind/coroutine and changed-body replacement |
| 7F Common graphs/summaries | Existing petgraph/analysis integration and canonical calls | Demand-rooted projections, correct dominance/SCC/reachability, structural facts and bounded interprocedural fixpoints with precision/frontier scope |
| 7G Complete forms/composition | Eight-form request/ingress infrastructure; four limited public forms | FindPaths, MatchPattern, Compare and Summarize; finish first four; real typed multi-block DAGs, fan-out/fan-in, repeated forms, references, authorization, negatives, ordering/limits and cancellation |
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
| Historical processing extension, before public call wiring | `just root-test-incremental -E 'test(processing_scope) \| test(pragmatic_python_semantics_publish_real_call_targets)'` with both real provider binaries selected | Four pass: three scope tests and real Python publication, 6.59 s runtime |
| Call-query continuation | Installed Python/call/reopen and mixed Rust/declaration/call tests; 16 focused scope/recipe/ingress/implicit-call tests; five final scope/implicit-call/reopen tests | Pass; full traversal, composition and live updates remain open |
| Declaration kinds/public subjects (`1a60e748`) | Installed Python kinds/fact subjects, guard choices, mixed Rust constants/statics/facts, exact reopen; adapter fast and focused root checks | Pass: 12.95 s Python, 11.04 s guard, 63.91 s Rust, 17.94 s reopen; 95 adapter and 15 focused root cases; default root check and affected Clippy complete |
| Lexical references (`5964e5ff`) | Installed Python complete/partial/reference/reopen and mixed Rust calls/declarations/unsupported-reference checks; root default/featureless check; affected Clippy/governance/docs/format | Pass: 30.39 s Python, 64.48 s Rust and three processing cases; Python call/reopen regression 17.49 s; Clippy retains 958 warnings |

Local observations for resumption include `/tmp/codefabric-call-processing-tests.log`,
`/tmp/codefabric-public-calls-check.log`, `/tmp/codefabric-python-canonical-calls-final-regression.log`
and `/tmp/codefabric-python-call-anchors-*`. The current slices also retain
`/tmp/codefabric-outcomes-declarations-final.log`, `/tmp/codefabric-outcomes-references-final.log`
and `/tmp/codefabric-outcomes-references-check-final.log`. They are optional local logs, not required runtime
artifacts or a new certification mechanism. Git and named behavioral tests retain the useful history.

The existing four golden scenarios have passed during earlier slices (startup, installed Python
serving, exact reopen and cancellation). Exact reopen was rerun on the call-query slice (16.65 s);
these scenarios do not exercise full outcomes 4–8. Root Clippy retains a large warning backlog (955 warnings in the latest affected library run). The last older aggregate root result at `0cc7242`
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

1. Retain the validated one-step call-query slice and extend it with the remaining canonical families
   and traversal semantics. Full FollowRelationships is still open.
2. Finish 4A–4D effective/external/generated inputs and canonical families needed by the first four forms;
   complete 4E and 5A–5B, especially all-family scope and target/family-specific freshness barriers. Retain the broad
   workstation allowances and honest partial semantics.
3. Extend the implemented 6A–6D observation/invalidation/two-speed publication loop with retained
   providers, external/configuration inputs and the full independent clean/incremental corpus.
   Complete first-four semantics before claiming the first useful release.
4. Continue 7A–7H and 8A–8F to the full plan acceptance: every family and all forms/composition, finite
   retention, actual native maintenance, recovery and representative performance. Add phase metrics and
   persistence improvements while integrating updates; optional performance mechanisms remain conditional.

The active user request is implementation of the entire remaining detailed plan. The limited call
slice above is progress; all other stated acceptance remains required.
