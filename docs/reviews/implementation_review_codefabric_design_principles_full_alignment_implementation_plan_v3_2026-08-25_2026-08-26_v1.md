---
artifact: implementation-review
plan_path: docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v3_2026-08-25.md
verdict: changes-required
version: v1
date: 2026-08-26
status: complete
---

# Implementation review: design-principles full-alignment plan v3

## Provenance and Review Scope

This independent, read-only review assesses the complete implementation of the approved v3
plan against remediation proposal v2, the governing design corpus, the schema-2 execution
state, current code and generated contracts, legacy cutovers, and executable behavior. The
review point is `412af14566393c2379ba4e174387361cea5370e8`; the accepted baseline
`dd3c0056ce2c01d04c28605b043a9316a6c26383` is an ancestor. The committed implementation
universe is 307 changed paths, 44,304 insertions, and 11,964 deletions from that baseline.

`just plan-status` independently reported a healthy derivation: all 23 packets, M09-M14, and
DB10-DB12 were trusted complete with no stale declared input. That establishes provenance,
not correctness. The implementation status report and execution state were used only as
evidence locators; their completion claims were re-tested against source, generated schemas,
focused gates, and adversarial probes.

Four pre-existing repository-owner paths were excluded from the implementation diff and left
untouched: the modified DataFusion reference skill and index, the deleted legacy
`docs/library_ref/datafusion_rust.md`, and untracked
`docs/library_ref/apple-rust-linker.md`. This review changes no production code, tests, plan,
state, or design document; its only repository write is this report.

## Executive Summary

The implementation is not complete enough to certify M14. The repository is mechanically
healthy and several substantial parts of the design are well implemented: the identity and
digest-domain split, generated registries, Arrow provider boundary, candidate-publication
foreign-key enforcement, truthful provider contracts, phase/error closure, keyed RPC boundary,
presentational adapter, deterministic checksum, model-plane repair, and legacy removals all
have current executable evidence.

The central product and release claims do not hold, however:

1. All eight query forms are advertised, but the public request type cannot express the
   normative semantics of relationship traversal, connecting paths, conjunctive patterns,
   deterministic set operations, objective summaries, or source-context selection. The
   executors implement reduced lookalikes. The generated JSON schema also rejects five of the
   eight registry-authoritative form names.
2. Gate B does not execute the required vertical golden slice. Candidate outputs for its eleven
   planes are copied from independently constructed descriptor expectations, and the released
   verifier checks those descriptors against files and registry literals rather than running
   source capture through providers, publication, serving, semantic query, RPC, MCP, streaming,
   and artifact retrieval.
3. Failed and cancelled governed executions persist an execution envelope with no plan
   artifacts or metrics; some post-execution persistence failures do not persist a failed bundle
   at all. Durable provenance therefore does not close for every execution.
4. AC-G-79's true clean-rebuild proof does not cover the full sixteen-scenario corpus. The
   corpus runner still records a same-wave reconciliation comparison, while the true rebuild
   test uses a separate five-stage fast-syntax-only fixture.

The focused gates pass because their assertions encode these reduced behaviors. This is an
oracle-substance failure, not evidence that the findings are hypothetical.

## Verdict

**Changes required.** IR-001 and IR-002 are blockers: required public capability and a required
release gate are unavailable despite being advertised and certified. IR-003 and IR-004 are
major gaps in provenance and convergence proof. The accepted design remains implementable;
the defects require implementation and proof repair rather than a design change.

## Gate and Evidence Assessment

| Evidence | Current result | Assessment |
|---|---:|---|
| baseline ancestry and `just plan-status` | pass; healthy, no stale inputs | packet trust is reconstructible, but does not prove outcomes |
| `just governance` at review HEAD | pass | structural/governance suite is current but does not detect the findings below |
| `just semantic-query-conformance-check` | pass; 6/6 | counts eight form variants and exercises reduced operators; does not validate QRY form schemas or semantics |
| QRY §16 JSON-schema falsification probe | **fail** | a normative connecting-path request is rejected by `cpg-semantic-query-request.schema.json` |
| registry-slug/schema cross-check | **fail** | schema accepts only 3 of the 8 registry-authoritative form slugs |
| `just gate-b-check` | pass | verifies release/digest chains and descriptor contracts; it does not execute the vertical slice |
| `just query-artifact-single-execution-check` | pass; 4/4 | its cancellation oracle explicitly accepts an empty `plan_artifacts` collection |
| `just wp72-acceptance-check` | pass twice; 4/4 each | true rebuild and full corpus are tested in separate, non-composing fixtures |
| DB10 textual/structural census | pass | no SQL builder on `semantic_query.rs`/`query_service.rs`; custom DataFusion extensions are rejected |
| DB11 textual census | pass | legacy observation/fact/extractor protocol names are absent from domain code; wire DTO use is confined to `rpc_adapter` |
| DB12 textual census | pass | twin generated Rust registry and handwritten registered state enums are absent |
| `git diff --check` | pass | no whitespace error in the reviewed artifact/diff |

The schema probe used the generated public schema and a request shaped exactly like QRY §16:
`starting_from`, `ending_at`, `through`, `path_policy`, `maximum_length`, and `return`. It was
rejected at `queries/0` because none of those form-specific fields exists. A second probe used
the eight generated registry slugs; `retrieve facts about code`, `follow code relationships`,
`find connecting fact paths`, `summarize objective facts`, and
`retrieve source and syntax context` were all rejected by the public schema.

## Finding Index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | blocker | outcome / correctness / architecture | the advertised eight-form semantic query contract is not implemented |
| IR-002 | blocker | tests / outcome / operations | Gate B releases descriptor assertions instead of an end-to-end vertical result |
| IR-003 | major | correctness / operations / provenance | failed and cancelled executions lose the required plan-artifact bundle |
| IR-004 | major | tests / correctness | AC-G-79 is not proved by true clean rebuild over the full golden scenario corpus |

## Findings

### IR-001 — The advertised eight-form semantic query contract is not implemented

**Severity:** blocker

**Dimension:** outcome / correctness / architecture

**Design/Plan refs:** QRY §§4.2-4.10, 15-17, 21, 30, 33, 106-107; proposal R1/R2;
plan outcomes 1.1.2-1.1.4; GI-1, GI-2, GI-4, GI-8, GI-10; WP56, WP62, WP75,
WP63; M11; LD-01 and LD-11.

**Evidence:**

- The only per-query public fields are `query_id`, `request`, `label`, generic `input`, two
  kind-code predicates, and `limit` (`src/semantic_query.rs:175-230`). There is no typed
  representation for QRY's `relationship`, `direction`, `distance`, `stop_when`, distinct
  path `starting_from`/`ending_at`, `through`, `path_policy`, `maximum_length`, named pattern
  `bindings` and `relationships`, set operator, grouping/aggregation objective, or requested
  source span/context projection.
- The generated public schema repeats the reduced shared shape for every form and hard-codes
  shortened names such as `retrieve facts`, `follow relationships`, `find paths`,
  `summarize facts`, and `fetch source context`
  (`contracts/schema/cpg-semantic-query-request.schema.json:94`, `:139`, `:184`, `:319`,
  `:364`). The registry authority instead defines the normative eight slugs, including
  `retrieve facts about code` and `find connecting fact paths`
  (`contracts/registry/enum-registry.yaml:15`; `src/generated/registries.rs:714-749`). The
  executable schema cross-check accepted only 3/8 authoritative names.
- `follow code relationships` lowers to an ordinary relation-table projection filtered only by
  fact IDs/relation kind; it has no traversal direction or distance contract. The application
  graph kernel finds one BFS path only between consecutive members of a single entity list,
  with a fixed depth of 64 (`src/semantic_query.rs:1285-1335`, `:1405-1440`). It cannot express
  start/end sets, relationship-family restrictions, or the QRY path policies.
- `match a code fact pattern` selects every edge touching an optional entity set; it has no
  named bindings or conjunctive relationship clauses (`src/semantic_query.rs:1442-1461`).
  `combine result sets` unconditionally maps the union of IDs into singleton groups, and
  `summarize objective facts` emits one count-like row without grouping or aggregation semantics
  (`src/semantic_query.rs:1463-1482`). Source-context retrieval returns all manifest context IDs,
  not the source/syntax records selected by inputs (`src/semantic_query.rs:1484-1493`).
- `wp75_behavioral_acceptance` calls those private reduced kernels and asserts row counts; the
  production conformance request supplies none of the normative form-specific fields
  (`src/semantic_query.rs:2288-2350`; `src/fabric/serving.rs:3288-3356`). Passing that test proves
  eight enum dispatch branches, not QRY §107 conformance.

**Failure mode:** clients cannot issue the questions the v1.3 public contract specifies. Valid
QRY requests are rejected, while advertised support returns answers with materially different
semantics: a relation scan can be labeled traversal, arbitrary adjacent IDs become path
endpoints, a one-edge filter becomes conjunctive pattern matching, and all contexts can be
returned for a selected path. This violates conservative capability advertisement and can make
incorrect facts appear authoritative.

**Remediation:** make the query clause a generated/tagged union with a form-specific typed model
for all eight QRY forms and derive its Rust, public JSON Schema, Proto/Python, capability, and
test projections from the query-form authority. Implement semantic phrase resolution and each
form's required operator fields, typed prior-result roles, coverage/absence behavior, and
canonical result shape. Relationship traversal and paths must preserve fact IDs, direction,
distance, relationship family, certainty, ordered witnesses, and context boundaries. Pattern
matching must execute the declared named conjunctive bindings. Advertise a form only when its
normative conformance row passes; otherwise mark it unsupported before snapshot work.

**Focused re-test:** add a QRY-spec-derived corpus with at least one positive and negative case
for every form-specific field and all allowed prior-result roles, then run a gate such as
`cargo nextest run --locked -E 'test(/qry_v13_form_contract_conformance/)' --no-tests=fail`
through `ProductionQueryService` over UDS. Add a generator cross-check asserting the eight
registry slugs exactly equal the JSON Schema, Rust serde, Proto, Python, and capability slugs.

### IR-002 — Gate B releases descriptor assertions instead of a vertical golden result

**Severity:** blocker

**Dimension:** tests / outcome / operations

**Design/Plan refs:** SUITE Gate B; proposal R9; plan outcome 1.1.10; GI-7, GI-10;
WP71, WP76; M14.

**Evidence:**

- `derive_expectations` constructs eleven small descriptor objects in code—for example, a list
  of provider names, table names, transport labels, and only three query-form names
  (`src/gate_b_candidate.rs:1040-1085`). They are not expected rows, response bytes,
  publications, streamed chunks, or artifact payloads derived independently from the design.
- After checking only that some IDs exist, expected table names were observed, and Tree-sitter
  plus Ruff appeared, `candidate_contracts` returns `expectations.clone()` unchanged
  (`src/gate_b_candidate.rs:1088-1123`). It does not require a rustc-MIR observation even though
  SUITE Gate B requires one Rust MIR owner and the static descriptor lists `rustc-mir`.
- The expected-vs-candidate diff therefore compares the descriptor map to its own clone
  (`src/gate_b_candidate.rs:1125-1165`). The owner accepted a cryptographically sound bundle,
  but that bundle did not contain independent produced outputs for the eleven vertical planes.
- Released execution canonicalizes each expected JSON descriptor and dispatches simple
  file/registry/literal checks (`src/golden_corpus.rs:618-660`, `:687-833`). It never constructs
  a `ProductionQueryService`, starts the daemon UDS endpoint, invokes the FastMCP adapter,
  observes streaming, reads a result artifact, or compares actual canonical result rows to the
  accepted files.
- `check_released_gate_b` regenerates the same candidate descriptors and then calls the
  descriptor verifier (`src/gate_b_release.rs:667-684`). Thus `just gate-b-check` can remain
  green even when the vertical query behavior or adapter/RPC integration is absent or wrong.

**Failure mode:** M14 can release and trust a golden corpus without proving SUITE Gate B's one
Python owner, one Rust MIR owner, unknown/property/relation/derived facts, hot update, durable
publication, semantic query, streamed result, and artifact result in one end-to-end execution.
The accepted answers authenticate assertions about what the system should have done, not the
bytes it did produce.

**Remediation:** replace each descriptor with immutable, independently derived expected output
and execute one coherent vertical fixture through source capture, Tree-sitter/Ruff/rustc (and
the applicable sidecar), reconciliation, candidate FK validation, durable Delta publication,
snapshot activation, production query UDS, real adapter/FastMCP surface, streamed response, and
artifact readback. Candidate generation must capture actual IDs, canonical table rows,
publication/version records, snapshot manifest, query response bytes/checksum, RPC events, MCP
payload, diagnostics, and artifact bundle. The diff must compare those produced bytes to an
independent expectation, not a clone. A new owner decision and superseding immutable corpus
version are required after the corrected candidate is generated.

**Focused re-test:** add an executable `gate_b_vertical_slice_produces_all_eleven_planes` test
that fails when any provider, publication, query/RPC/MCP stream event, or artifact is stubbed or
omitted; select it with `--no-tests=fail` from `just gate-b-candidate-check` and
`just gate-b-check`. Perturb one expected row or suppress rustc-MIR execution and prove both
candidate and released gates fail.

### IR-003 — Failed and cancelled executions lose the required plan-artifact bundle

**Severity:** major

**Dimension:** correctness / operations / provenance

**Design/Plan refs:** proposal R4; plan outcomes 1.1.5 and 1.1.8; GI-5, GI-6;
WP61, WP65, WP66; M12; LD-06.

**Evidence:**

- When backend planning/execution returns any error, `execute_accepted_query` persists a failed
  terminal bundle with `plan_artifacts: Vec::new()` and only a public error code
  (`src/query_service.rs:1204-1213`). The backend result type has no failure payload capable of
  returning a partial plan/metrics artifact.
- Cancellation paths likewise persist an empty plan list (`src/query_service.rs:851-870`,
  `:1907-1924`). The packet's own negative oracle asserts that the empty list is correct
  (`src/query_service.rs:2764-2815`), opposite WP65's acceptance sentence requiring partial
  metrics and phase on failure, cancellation, and stream drop.
- If result-artifact insertion fails after successful semantic execution, the service emits a
  failed terminal event but persists no corresponding failed query-artifact bundle
  (`src/query_service.rs:1254-1287`). Canonical snapshot encoding and artifact-persistence
  failures also contain early-return paths that cannot preserve the complete failure context.

**Failure mode:** an execution ID can resolve only to a phase/error shell, or to no durable
bundle, after the system has already planned or executed work. Operators cannot recover the
logical/optimized/physical plan, partial metrics, pinned inputs, or exact failing phase, and
`explain_version` cannot close the promised durable provenance chain for all governed
executions.

**Remediation:** make planning/execution return a failure artifact containing every stage and
pin captured before failure, plus metrics from the same physical plan instance when one
exists. Persist the terminal bundle before emitting the terminal event for success, failure,
cancellation, stream drop, result insertion failure, and artifact-store failure; define and
test a recoverable fallback if the primary artifact store itself is unavailable. Cancellation
before planning may legitimately have no plan, but must explicitly record that lifecycle
boundary rather than using the same empty representation as cancellation during execution.

**Focused re-test:** inject faults after binding, logical planning, physical planning, first
batch, stream drop, result insertion, and artifact persistence. For every allocated execution
ID, assert a readable terminal bundle with the exact phase, complete available pins, the
partial plan/metrics set appropriate to that phase, and no diagnostic re-execution. Run with
`cargo nextest run --locked -E 'test(/query_failure_artifact_closure/)' --no-tests=fail` and
`just query-artifact-single-execution-check`.

### IR-004 — AC-G-79 is not proved by true clean rebuild over the full scenario corpus

**Severity:** major

**Dimension:** tests / correctness

**Design/Plan refs:** AC-G-79 §79.2; proposal R9; plan outcome 1.1.10; GI-7;
WP72; M14.

**Evidence:**

- The sixteen-scenario candidate runner's `clean_rebuild_equal` helper clones the current wave,
  changes its state, and re-runs `FastSyntaxReconciler` over the same already captured wave
  (`src/gate_b_candidate.rs:412-423`). This is the exact same-wave comparator WP72 says must
  retire; it does not re-walk inventory, recapture bytes, rebuild operational state, or compare
  serving sessions.
- The actual zero-state implementation exists and `wp72_behavioral_acceptance` uses independent
  operational stores and `CanonicalState::from_serving_session`, but only over a separate
  hand-written five-stage fixture (`src/fabric/serving.rs:2223-2260`, `:2282-2433`). It does not
  load the sixteen released `scenario.json` definitions.
- Both the candidate scenario engine and the WP72 fixture set
  `semantic_capabilities_required: false` (`src/gate_b_candidate.rs:266-278`;
  `src/fabric/serving.rs:2203-2218`). The proof therefore covers the fast syntax lane, not the
  provider-complete effective state implied by the released Gate B corpus and AC-G-79.
- `just rebuild-equivalence-check` selects only the four `wp72_*` tests; no oracle composes the
  full scenario loader, the true zero-state rebuild, semantic/provider lanes, and exact serving
  schema/bag comparison.

**Failure mode:** convergence regressions specific to overflow, multi-file saves, context
change, capability withdrawal, watcher loss, hot-overlay flush, ACL redaction, provider
withdrawal, or semantic facts can pass the released scenario suite and the true-rebuild suite
independently. The full AC-G-79 claim is therefore unproved.

**Remediation:** make the released scenario runner invoke a fresh zero-generation engine and
operational store for every terminal checkpoint, re-walk and recapture the actual workspace,
run the same required providers/capability policy, activate independent serving snapshots, and
compare complete effective state with `prove_serving_rebuild_equivalence`. Delete the
same-wave `FastSyntaxReconciler` equality claim or label it only as a local determinism check.
Include the semantic-capability success and withdrawal paths rather than disabling the semantic
lane for the corpus.

**Focused re-test:** add one selector that iterates all sixteen released scenario definitions
and, after every terminal state, compares incremental and independent zero-state serving
sessions for schema metadata, governed-key uniqueness, row counts, and exact multiplicities.
Run it with `cargo nextest run --locked -E 'test(/full_golden_scenario_clean_rebuild_equivalence/)' --no-tests=fail`
from `just rebuild-equivalence-check`; mutate a tombstone, provider fact, context, duplicate
multiplicity, and one semantic-lane result to prove rejection.

## Outcome and Invariant Matrix

| Design move / invariant | Status | Executable oracle or falsification |
|---|---|---|
| R11 / GI-10 — accepted normative ownership and current detector truth | pass for authority setup; certification reopened | `just design-principle-traceability-check`; `just alignment-detector-check`; IR-001/IR-002 prevent M14 certification |
| R1 / GI-1 — one identity, fingerprint, and registry authority | **fail at query public schema** | registry-slug versus JSON-Schema equality probe from IR-001 |
| R2 / GI-2, GI-4, GI-8 — typed semantic multi-backend DAG | **fail** | QRY §15-17 request-schema probe and form-specific production KAT required by IR-001 |
| R3 / GI-12 — deterministic result and separated identities | pass in reviewed scope | `just query-determinism-check` plus WP64 adversarial checksum tests |
| R4 / GI-6 — artifact/provenance closure | **fail on non-success paths** | phase-fault artifact-closure matrix required by IR-003 |
| R5 / GI-3 — one Arrow fact contract and two transports | pass in reviewed scope | `just provider-protocol-check`; DB11 zero-state search |
| R6 / GI-13 — truthful providers and candidate-state contracts | pass in reviewed scope | `just publication-referential-integrity-check`; `just id16-extension-contract-check`; `just provider-statistics-contract-check` |
| R7 / GI-5 — phase/error/guard truth | pass in reviewed scope | public-error closure inside `just governance`; WP61 phase and guard oracles |
| R8 / GI-9 — one hardened boundary and thin adapter | pass in reviewed scope | `just adapter-ci-fast`; `just adapter-stdio-test`; WP67 forgery/expiry tests |
| R9 / GI-7 — contract-derived Gate B, convergence, and parity | **fail** | corrected vertical Gate B oracle (IR-002) and full-corpus true-rebuild oracle (IR-004) |
| R10 — authoritative model compiler and derived outputs | pass in reviewed scope | `just model-plan-check`; `just model-repro-check`; `just model-release-check` |
| GI-11 — one pinned Arrow/DataFusion universe | pass | `just stable-graph-check`; final `features-each` evidence applies because later commits are docs/state only |

Every matrix row has a named executable oracle. Rows marked pass are bounded to the reviewed
claim; they are not blanket approval of modules affected by a blocker.

## Architecture and Doctrine Assessment

The implementation preserves several important architectural decisions. Relational planning
uses DataFusion built-ins and explicitly rejects `LogicalPlan::Extension`; no custom execution
plan, physical expression, UDF, or planner was introduced. Graph work remains in an
application-owned typed family. Python stays presentational, provider wire types are confined
to adapters, and the direct-batch/IPC split respects process boundaries.

IR-001 nevertheless violates model-first semantics and conservative claims: a typed node named
after a QRY form is not that semantic model when its defining fields and result obligations are
absent. IR-002 and IR-004 violate contract-derived proof: a descriptor check and two disjoint
partial fixtures cannot establish the end-to-end contract. IR-003 violates lifecycle and
provenance closure because the system discards the artifacts needed to explain non-success
executions. These are failures of HOL pass contracts and PRIN P1/P2/P10/P20/P25, not stylistic
disagreements.

## Library Leverage Assessment

The selected versions and responsibility split remain sound. The implementation makes
appropriate use of `LogicalPlanBuilder`, `Expr`, snapshot-scoped DataFusion sessions, Arrow
`RecordBatch`, `StreamDecoder`, fallible Id16 construction, `arrow-row`, physical-plan metrics,
Delta history/commit properties, and petgraph/application graph infrastructure. No finding
requires a version movement or a lower DataFusion extension level.

The query defect is application-model incompleteness, not a missing DataFusion capability:
DataFusion can continue to own relational subplans while the application graph plan represents
the full QRY traversal/path/pattern semantics. The Gate B and convergence defects likewise
require composing existing production boundaries, not adopting another library.

## Legacy and Decommission Assessment

DB10-DB12 are substantively complete in the reviewed tree:

- semantic query code contains no legacy SQL string builder or static registered-state fields;
- `ObservationMessage`, `CanonicalFact`, `encode_selected`, and `--extract-json` are absent from
  domain paths, with `ProviderJobSpec` confined to the RPC adapter decode seam;
- the twin generated registry module, orphan Python registry, and handwritten registered
  state enums are retired; and
- no custom DataFusion extension was added as a replacement escape hatch.

The remediation must not restore any of those paths. IR-001 should be fixed by extending the
typed public and graph models, not by reviving SQL text or introducing opaque DataFusion nodes.

## Test and Operational Assessment

Ordinary tests and repository gates are stable, including current focused query, Gate B,
artifact, and convergence selectors. Their green state is valuable for regression safety but
does not close the findings because the asserted contracts are narrower than the accepted
sentences. In particular:

- the query gate proves dispatch and determinism of simplified forms;
- the Gate B gate proves immutable acceptance and descriptor consistency;
- the artifact gate proves the current empty-on-cancel behavior; and
- the rebuild gate proves a real five-stage fast-syntax rebuild plus separate full-corpus
  same-wave checks.

The owner's waiver of further mutation execution is not a finding. Mutation is Tier C and the
bounded campaigns already produced useful assertion repairs. No first-party unsafe code was
introduced, so the Miri deferral is also proportionate. The findings above were exposed by
contract-level falsification and code-path inspection, not by a missing expensive assurance
tool.

## Plan Deviations and Diff Hygiene

The execution state records the mutation and Miri decisions accurately, and the history retains
packet-focused implementation commits even where later trust reconciliation moved several
packets to a shared recertification commit. That shared current proof is not itself a finding.

The material deviation is functional: WP75, WP65, WP71/WP76, and WP72 were marked complete
against reduced oracles. Because those packet outcomes are load-bearing for M11, M12, and M14,
the state should be superseded or corrected through the normal execution-state workflow after
the implementation author accepts these findings; neither this review nor the immutable plan
should be edited to manufacture completion.

The current worktree's four repository-owner paths are unrelated to the review and remain
unstaged. `git diff --check` is clean.

## Required Remediation Order

1. Reopen WP56/WP57/WP62/WP75/WP63 and repair the one-authority, form-specific query contract;
   withdraw unsupported advertisements until each normative form passes.
2. Rebuild WP71's candidate generator as a real vertical execution, then obtain a new WP76
   accountable-owner decision and publish a superseding immutable corpus version.
3. Reopen WP65/WP66 and persist phase-appropriate partial plan artifacts for every terminal
   path before claiming provenance closure.
4. Replace the corpus same-wave comparison with true independent rebuilds across all sixteen
   scenarios and semantic-provider configurations; re-prove WP72.
5. Re-run the complete v3 final gate matrix, update execution state/status through their normal
   versioned workflow, and request focused independent re-review before restoring M14.

## Focused Re-Review Scope

A follow-up review may remain focused on IR-001-IR-004 and their direct consumers:

- generated query authorities and schemas, `semantic_query`, serving/query-service activation,
  and capability advertisement;
- Gate B candidate/release/corpus execution and the newly accepted corpus bytes;
- failed/cancelled execution artifact persistence and `explain_version` closure; and
- full-corpus incremental-versus-clean rebuild execution.

The identity/checksum substrate, provider direct/IPC boundary, publication FK enforcement,
error registry, hardened RPC/adapter boundary, model compiler, and DB10-DB12 need only regression
gates unless remediation touches them.
