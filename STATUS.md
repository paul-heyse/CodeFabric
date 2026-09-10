# CodeFabric status

Updated 2026-09-10 from the canonical `/home/paul/CodeFabric` working tree on `master`.
Implementation has resumed from `b2a97b9c` in the detailed plan's P01–P14 package order, as requested.
P01's initial captured dependency/context vertical and phase costs are committed in `a31e2a3a`.
P02's initial canonical semantic vertical now publishes diagnostics, Python modules, Python/Rust
imports, references and structural types with scoped public retrieval and exact reopen.
The Rust type and HIR reference slices are committed in `ba35b4c7` and `1b515b7b`.
P02 public family selection is committed in `f7adca03`. P03 repeated-block output isolation is
committed in `7eefd2ea`, and typed prior-entity result consumption in `bd7f3752`.
Semantic-reference/import traversal is committed in `2ca3d45b`. First-class occurrence
selection and typed outgoing traversal are committed in `fe0d17fe`.
The occurrence/module source continuation now passes mixed native query/reopen validation;
retained source revocation also passes after the block-provenance correction.
The preceding production milestone is `4cc74d7c` (typed native Rust diagnostic details).
Package boundaries cross outcomes 4–8; completing the first package does not complete an outcome.

## Current handoff

**Outcomes 1–3 are implemented for the current Linux workflow. Outcomes 4 and 5 are partially
implemented. Outcome 6 has a partial production update loop; outcomes 7–8 remain open with selected implemented
prerequisites. No outcome from 4 through 8 is complete.**

Follow the [production backlog](docs/plans/codefabric_pragmatic_production_implementation_plan.md)
and its [detailed outcomes 4–8 execution plan](docs/plans/codefabric_pragmatic_production_outcomes_4_8_detailed_implementation_plan_2026-09-09.md).
The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md)
and [selected design](docs/spec_index/README.md) retain the full Python/Rust CPG, all eight forms,
composition, truthful incomplete scope and sustained operation. The detailed plan separates
implemented portions, unfinished acceptance and the next work for every slice, with library-grounded
design enhancements integrated into the same delivery progression.

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
`5964e5ff`. The full plan remains unfinished. No outcome from 4 through 8 is closed. Implementation
originally stopped at the diagnostic checkpoint. The subsequent user instruction resumes execution
of the cross-cutting packages in §3.3 of the detailed plan.

## P03 in progress: block composition and first-four completion

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

This is the initial compiler-failure vertical. Runtime stream/planning failures, early phrase/input
and authorization failures, all-failed request result envelopes, ready-block concurrency, prior
FindEntities scopes and broader first-four semantics remain P03 work. An all-failed request retains
the existing request-level failure. P03 and subsequent packages remain open.

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
byte-safe compiler argv, retained providers/caches and measured shared scheduling remain in their
4A–4C/P04/P06/8E slices. P02's initial canonical semantic vertical and scoped public retrieval are now delivered.

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

Later committed slices add custom/default/disabled build inputs, Cargo-configured platforms/rustflags,
library linkage combinations and captured feature/default-feature/profile selections with workspace
inheritance and package override. Raw compiler source paths and ordinary structured diagnostic
messages/details retain exact source binding. Fresh readiness and live updates publish source/syntax
before semantic convergence. Their independent clean/reopen checks are recorded above.

Remaining: registry/git materialization; generated `OUT_DIR`/build-script/proc-macro input closure;
complete actual per-unit/unified-feature/environment and host-target contexts; retained compatible
build caches and bounded parallel scheduling; raw compiler argv and external tool-change invalidation;
full generated/hygiene diagnostic mapping, compatibility-report semantics and canonical/public
consumers. Existing failed-target, source-stage, cancellation and stale-completion checks cover their
selected scenarios; broader configuration/dependency races remain.

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

The committed Python configuration slice admits captured project configuration when the effective manifest accounts for
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

The committed source/stub slice owns retained modules by input/file identity rather than import name.
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
complete canonical type/member/import/reference coverage beyond the initial P02 clusters above;
additional declared/expected/narrowed propositions; full overload/descriptor/decorator semantics;
retained checker updates and context invalidation. Initial normalization does not close those families.

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
recorded mixed source scenario above. Raw Rust source-manifest/local-file paths also pass the
separate compiler-path scenario; complete byte-safe compiler argument handling remains open.

Remaining: full source/lexical/CST feature census; retained parsers/query packs and incremental trees;
complete trivia/index/coordinate handling and further source codecs; reversible compiler paths
and additional source presentation; rename/case-collision semantics and broader incomplete-edit
behavior during actual live updates. Exact declaration spans, function definitions/bodies, bounded
surrounding lines and explicit UTF-8/UTF-16 columns are implemented. Remaining subjects and complete
syntax/coordinate contexts stay open.

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

Remaining: full module/class/lambda/callable entities; full exports/reference census beyond P02's
initial Python/Rust cluster; complete structural types and propositions beyond the initial shapes; members/signatures/argument binding; complete
candidate/dispatch and executable-instance relations; external endpoints and generated/lowered correspondence;
full per-proposition authority/conflict retention; identity continuity and owner replacement under edits.
Raw provider coverage is not complete canonical-family coverage.

### 4E — public query forms: partial

| Form | Demonstrated current behavior | Remaining |
|---|---|---|
| FindEntities | Canonical declarations, modules, call/reference/import occurrences and complete admitted syntax nodes; reusable IDs/priors, literal names, text properties, captured path bounds and source context selection | Remaining kinds, configured context defaults, broader boundaries, ambiguity and directives |
| RetrieveFacts | Canonical declarations, syntax properties and initial module/import/reference/type/diagnostic families; typed entity priors and literal subjects; native semi joins preserve occurrence identity | Full signatures/members/arguments, point filters, broad family expansion and fact/instance subjects |
| FollowRelationships | Python/Rust one-step calls, semantic references/imports, syntax parent links and Python lexical references; both directions, typed entity priors and scoped unknowns | Full distance/stop/filter behavior, remaining endpoint roles and dependency scope |
| RetrieveSourceContext | Exact declaration, module, occurrence and syntax-node spans; function definitions/bodies and surrounding lines; independent disclosure, original-byte/UTF-8/UTF-16 coordinates, truncation and retained revocation | Source-location/fact/instance subjects, outlines, related contexts and full history selection |

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
include the selected context's potential callers or referring files. Exact outgoing Rust caller
partitions are implemented; full reverse-dependency and all-family owner scope remain open.

Remaining: all family/owner dimensions; authorization-scoped efficient status
scans; incoming reference/import and negative dependency/frontier propagation; shared live pending/running
state; provider precision and actionable retry details; a clean distinction between terminal semantic
unknowns and runnable pending work for convergence. Empty results alone never establish complete absence.

### 5B — typed delivery and source/semantic barriers: partial

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
| 6D | Mixed live/clean comparison; Python version/platform/roots/stubs/paths/decoding, Cargo build/platform/feature/profile selections, failed compilation repair, obsolete completion and source-only restart | Full semantic/identity/coverage edit corpus, external/generated dependencies, complete context and rename cases |

The installed live test is distinct from startup-versus-restart validation. The mixed comparison retains canonical
identity and relationships; the wider edit and rename-continuity corpus remains open.

## Outcome 7: full analyses and all eight forms — open

| Slice | Implemented prerequisite | Remaining delivery |
|---|---|---|
| 7A Python language semantics | Owned Ruff bindings/references/call syntax and selected Pyrefly call definition anchors | Complete scope/binding/import/type/member/call/decorator/pattern/comprehension and dynamic-semantics rows, canonical consumers and invalidation |
| 7B Python CFG/dataflow | Substantial native Ruff owner CFG/evaluation builders, typed analysis code and prepared source expectations | Integrate the explicit native graph, remove implicit production ordinal fallthrough, qualify normal/exception/cleanup/suspend edges and complete real dataflow/public/update wiring |
| 7C Python advanced state | Existing analysis structures | Finite memory/points-to, effects/resources/exceptions, capture/generator/async/concurrency and unknown propagation, built on 7B |
| 7D Rust source/types/MIR | Real typed compiler publication, stable declaration keys, selected canonical calls and ordinary native diagnostic messages/children/spans/suggestions/edits | Full types/generics/traits/instances/MIR payloads, generated/hygiene/coroutine/CTFE/FFI facts, separate compatibility diagnostics and canonical/public coverage |
| 7E Rust derived/private borrow | Existing MIR analysis modules and contained compiler seam | Real typed inputs, finite dataflow/state/ownership analyses, exact private loans/regions, drop/unwind/coroutine and changed-body replacement |
| 7F Common graphs/summaries | Existing petgraph/analysis integration and canonical calls | Demand-rooted projections, correct dominance/SCC/reachability, structural facts and bounded interprocedural fixpoints with precision/frontier scope |
| 7G Complete forms/composition | Eight-form request/ingress infrastructure; four limited public forms | FindPaths, MatchPattern, CombineResults and SummarizeFacts; finish first four; real typed multi-block DAGs, fan-out/fan-in, repeated forms, references, authorization, negatives, ordering/limits and cancellation |
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
| 8F Performance | Native canonical joins, schema-preserving scan pushdown, streaming lookahead and shared immutable input storage | Qualify structured scan/statistics/physical properties through all consumers, pruning/file-size tuning, workload-based parallelism/caches and measured before/after improvements; overlays/CDF/Rayon/orjson only with a concrete need |

`delta_guarded_maintenance.rs` still returns `OptimizeCommitIdentityAndRetryControl` and
`AtomicVacuumApprovalBinding`. No optimize commit or destructive vacuum/reclamation is claimed.
No representative benchmark or sustained bounded-storage acceptance has been completed.

## Validation and limits at this checkpoint

These are attributable implementation runs from 2026-09-09. The final diagnostic rows include
checks run while reaching this stopping point; earlier rows preserve evidence for unchanged scopes.
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
these scenarios do not exercise full outcomes 4–8. Latest affected Clippy retains 952 library/36 integration warnings (988 total), with no new findings
in the diagnostic-detail slice. Strict lint remains open; the latest run was not a `-D warnings` pass. The last older aggregate root result at `0cc7242`
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
All validation processes and fixture daemons launched for this slice have finished; no continuation
is scheduled. A separate terminal-owned nextest run was observed during handoff; it was left
untouched and its results are not included here.

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
disk copies, and one charged toolchain capture is shared per semantic pass. External tool changes, retained
build state, cache reclamation and representative cost validation still need work.

Use self-contained `just` recipes; keep stable/sidecar shared `target/` and the extractor's separate
dated-nightly target. Real root provider tests require current `CODEFABRIC_RUSTC_EXTRACTOR_BIN` and
`CODEFABRIC_PYREFLY_SIDECAR_BIN`. Rebuild a changed sidecar through the repository shell or the existing
golden setup; `just sidecar-check` checks/lints rather than installing a fresh executable. No routine
`cargo clean`, independent worktrees, source-edit artifacts or new approval cycle is required.

## Active package order

Implementation resumed under the user's package-order instruction after the 2026-09-09 planning
revision. The entire outcomes 4–8 scope remains required; none is complete. The detailed plan
§3.3 defines P01–P14 and §10 records the current entry point. P01's initial vertical is described above.

Continue in this order:

1. Preserve P01's captured dependency roots and phase instrumentation, extending its full context,
   cache and source-fidelity variants in the owning later slices.
2. P03–P05: completed first-four meanings, remaining query coverage and typed block bindings,
   target freshness, retained live providers/version reuse and broader clean/incremental comparison.
3. P06–P11: every remaining language/analysis family, explicit native CFG/MIR/private inputs,
   common graphs/summaries, all eight forms/full DAG semantics and modern delivery.
4. Begin P12 native maintenance/finite retention after P04's ownership prerequisites while fact
   scope grows; finish P13–P14 integrated recovery and representative workload optimization.

Do not redo per-publication parser reuse, contained Cargo startup, supported captured selections,
source-first activation, one-step call/reference queries or the completed source-context meanings.
Cross-edit provider retention and complete context/analysis/query acceptance remain missing.
No representative performance, native optimize/destructive vacuum or sustained reclamation result
was produced in this continuation.
