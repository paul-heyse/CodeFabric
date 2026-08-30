---
artifact: plan-audit
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v1_2026-08-29.md
verdict: needs-redesign
version: v1
date: 2026-08-29
status: complete
---

# Plan Audit: CodeFabric execution-proved relational data fabric v1

## Provenance and Scope

This audit evaluates the implementation plan, its accepted source design, and the design itself
against the current repository, the v2 data-fabric principles, the authoritative feature suite,
and the exact pinned library sources. The plan and design were treated as revisable: fidelity to a
flawed premise was not counted as readiness.

| Evidence | Audited value |
|---|---|
| Plan | `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v1_2026-08-29.md`, SHA-256 `4da2fd00b56ed232434523f3337a2765331d58c75812f3d407ffa6d859c51e99` |
| Source design | `docs/designs/codefabric_execution_proved_relational_data_fabric_design_v1_2026-08-29.md`, SHA-256 `f1d7a451b3dfd103b61aabb9d0a6395d870690f85a8013d2c84a27e4fb85de27` |
| Governing doctrine | `docs/library_ref/full_data_fabric_design_principles_v2.md`, SHA-256 `eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5` |
| Recorded plan baseline | `bcee3f0ae0618231357bfef91ae59403acf61fed` |
| Audited HEAD | `7184b86dc80adedc8a2b8d081179fa52d3dfee20` |
| Baseline drift | Four descendant commits: `e19c67b`, `2f7dc96`, `cc6770b`, `7184b86` |
| Exact resolved stack | DataFusion 55.0.0; Arrow/Parquet 59.2.0; `object_store` 0.13.2; delta-rs 1.0.0 at `43a0cf10a313e5077c48637ad786a05359136bbb`; Pyrefly 1.2.0 at `1933169...`; Ruff 0.0.7; Tree-sitter 0.26.12; dated rustc extractor contract |

The current plan passes the repository's direct `validate_plan(...)` structure, input-digest,
identifier, and oracle-catalog validation: 21 packets, six milestones, seven decommission batches,
21 packet oracle catalogs, and 84 oracle entries were recognized. That proves document shape, not
design correctness.

The audit used current `Cargo.lock`/metadata, exact checked-out library sources, targeted source
and call-site inspection, current-tree drift, and focused probes. In particular:

- candidate `artifacts-check --plan <this-plan>` terminates with an unhandled
  `FileNotFoundError` for the deliberately not-yet-created state file;
- `just doctor` currently fails only because the extractor's dated nightly does not resolve;
- `just stable-graph-check` does not reach graph adjudication because the current Cargo wrapper
  rejects the `+nightly` invocation; and
- the broad `just ci-fast` run was stopped during its initial cold compile at the user's direction.
  No broad-suite failure is inferred from that termination, and broad validation was not rerun.

Existing user changes in the dirty worktree were preserved. This audit modifies no design, plan,
state, implementation, test, configuration, or active-plan pointer.

## Executive Summary

The core architectural direction is substantially better than the predecessor: replayed
relational authority, exact Arrow boundaries, a DataFusion catalog as runtime architecture,
programmatic logical-plan compilation, immutable proved epochs, exact Delta version pins, a
single command path, a presentation-only FastMCP adapter, and explicit legacy deletion are the
right clean-sheet shape.

The plan is nevertheless not safe to execute. Seven blockers invalidate implementation premises
or transition safety:

1. the candidate is expected to implement the governance machinery needed to validate and
   activate itself outside any active packet DAG;
2. WP18 reverses the source design's admission/activation barrier;
3. the proposed post-cutover journal cannot durably revoke an exact legacy binary that never
   reads it;
4. provider-native and application-derived facts are assigned to the wrong authorities, while
   the accepted CFG/dataflow/alias/effect/summary capabilities have no dependency-closed
   implementation program;
5. WP09 assigns Pyrefly facts to `Query` APIs that do not exist;
6. WP10 assigns stable identity, borrowck, and derived dataflow to `rustc_public` surfaces that do
   not exist; and
7. the Rust semantic lane has no enforceable untrusted build-script/proc-macro sandbox contract.

The DataFusion/Arrow target is also under-specified at several load-bearing seams: logical versus
Delta physical schema adaptation, native-versus-extension graph selection, complete custom
physical-plan obligations, bound authorities inside `ViewTable`, the application-owned
`StatisticsRequest` pipeline, heterogeneous Arrow IPC framing, and exact Delta selector/retry
semantics.

The correct outcome is not to retreat from DataFusion, Arrow, or direct current APIs. It is to
revise the design so those APIs are used exactly, move custom analyses into explicit derived
relations, add the missing proof/security owners, and then issue a dependency-closed plan v2.

## Readiness Verdict

**Verdict: `needs-redesign`.** The plan must not be approved or activated in its current form.

| Severity | Open findings | Readiness effect |
|---|---:|---|
| Blocker | 7 | Invalid API, authority, security, or transition premises prevent safe execution. |
| Major | 8 | Material plan, proof, library, and decommission corrections are required. |
| Minor | 1 | The planning baseline and cleanup assumptions must be refreshed. |

The revised design should preserve D-20 through D-29 where they remain sound, amend the provider,
physical-schema, authorized-view, and cutover decisions, and add an explicit derived-analysis
decision. A revised implementation plan should then be audited as a new artifact.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | sequence | WP01, plan §8, plan-governance tooling | open |
| F-002 | blocker | operations | D-26, WP18 | open |
| F-003 | blocker | design | D-26, Stage 5, WP21 | open |
| F-004 | blocker | design | D-22–D-25, WP08–WP12, GEN §§24–66 | open |
| F-005 | blocker | library | D-23, LD-25, WP09, WP11, WP19 | open |
| F-006 | blocker | library | D-23, LD-25, WP10–WP11 | open |
| F-007 | blocker | operations | design §3.12, WP10, WP19–WP20 | open |
| F-008 | major | proof | WP04, WP07–WP10, WP18, WP20 | open |
| F-009 | major | design | LD-17, LD-20, WP04–WP06, WP17 | open |
| F-010 | major | library | D-24–D-25, WP12 | open |
| F-011 | major | design | D-21–D-22, WP14 | open |
| F-012 | major | library | D-26–D-27, WP17 | open |
| F-013 | major | library | LD-20, WP05, WP11 | open |
| F-014 | major | design | D-23, LD-17, LD-26, WP04, WP08–WP10 | open |
| F-015 | major | legacy | Stage 1, L-20–L-22/L-54, DB01, plan §8 | open |
| F-016 | minor | factuality | plan §§1.3, 2.1, WP01 | open |

## Findings

### F-001 — The candidate depends on ungoverned code changes to validate and activate itself

**Severity:** blocker
**Category:** sequence
**Scope:** WP01, plan §8, `tooling/ci/plan_assurance.py`,
`tooling/ci/artifact_contracts.py`, `justfile`
**Finding:** WP01 and plan §8 permit overlap-ledger, inactive-candidate validation,
state-schema, and activation-transaction implementation before this plan is active, but place
that work outside the immutable packet DAG. Current `plan-dependency-check` selects only the
active plan, current activation publishes state and pointer as separate replacements, and the
focused candidate `artifacts-check` crashes because no candidate state exists. A reviewed commit
without an active packet/state/proving-commit chain is not governed execution and cannot prove
the machinery that authorizes this plan.
**Required resolution:** Implement inactive-candidate validation, plan-qualified overlap keys,
predecessor disposition, and crash-recoverable activation under an explicitly approved replan of
the active predecessor or a separate governance-remediation plan with its own state and proving
commit. Only after that remediation is accepted may this successor be audited, approved,
activated, and begin WP01. Remove all self-implementing preactivation work from the successor
DAG.
**Revalidation:** `just plan-candidate-readiness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`

### F-002 — WP18 commits durable activation before closing query admission

**Severity:** blocker
**Category:** operations
**Scope:** D-26, WP18
**Finding:** D-26 requires admission to close before the activation event is committed and read
back. WP18 instead specifies `append activation -> read back -> close admission -> swap`. A new
query can therefore be admitted on the predecessor after durable head selection has changed but
before the barrier closes. The stated no-partial-transition property does not follow from the
packet protocol.
**Required resolution:** Order activation as: stage and prove candidate; build and seal the new
epoch; close new admissions and establish the barrier; revalidate predecessor and writer fence;
append and read back activation; swap the epoch and reconcile the temporal cache; reopen
admission; acknowledge. Existing leases may drain on their already pinned predecessor epoch.
Fault-inject concurrent admissions before and after every step.
**Revalidation:** `just activation-fault-matrix-check`

### F-003 — The proposed journal cannot irreversibly fence the frozen legacy binary

**Severity:** blocker
**Category:** design
**Scope:** D-26, design Stage 5, WP21, frozen legacy daemon
**Finding:** The design and WP21 claim that `NEW_MUTATING` irreversibly fences the legacy writer
through a new deployment journal and higher writer generation. The frozen legacy daemon only
holds an advisory process-lifetime `daemon.lock`; it has no journal or generation check. After
the target daemon stops and releases that lock, the exact old binary can restart and reacquire
it. New code's journal cannot revoke authority from code that never reads the journal.
**Required resolution:** Amend D-26 and Stage 5 with a durable enforcement boundary that the old
release cannot bypass. Viable shapes are a deliberately shipped bridge legacy release that checks
a monotonic retirement generation at every serving/write ingress, or an external deployment,
credential, namespace, or service-entrypoint authority that mechanically revokes the old
binary's access. Specify persistence, ownership, crash/reboot recovery, and binding to the exact
activation event and writer generation. Integrate the selected mechanism into WP21.
**Revalidation:** `just legacy-writer-fence-check`

### F-004 — Provider-native facts and derived analyses have the wrong authorities and no complete implementation program

**Severity:** blocker
**Category:** design
**Scope:** D-22–D-25, WP08–WP12, GEN §§24–66
**Finding:** D-22 places CFG/dataflow observations in `raw_ruff` and dataflow in `raw_rustc`;
WP08 and WP10 call these provider-native outputs. GEN §4 assigns Python CFG and all def-use,
dataflow, alias/points-to, effects, control dependence, summaries, and unknown materialization to
CPG custom analysis, while Rust MIR supplies raw CFG/access/state inputs and CPG/petgraph own
derived analyses. GEN §§24–66 define substantial Python CFG/dataflow/effect/resource work, Rust
ownership/def-use/alias/drop/async work, graph analyses, and interprocedural summaries. WP06 is a
generic plan compiler and WP12 covers graph extensions; neither packet implements these semantic
algorithms or proves complete fact-family coverage. The plan cannot preserve the accepted code
intelligence surface.
**Required resolution:** Amend D-22/D-23 and add a design decision for derived-analysis
authority. Restrict `raw_*` schemas to facts genuinely exposed by each exact provider. Add
dependency-closed packets after raw-provider acceptance for Python CFG and flow analyses, Rust
MIR-derived ownership/dataflow/alias analyses, common graph analyses, effects/resources, and
interprocedural summaries. Each family must name input/output relations, algorithm and precision
version, incremental invalidation, provenance, completeness/unknown semantics, materialization
policy, and independent semantic expectations. Derive a relation proving that every accepted
ontology/query family has exactly one producer or an explicit unsupported remainder.
**Revalidation:** `just derived-analysis-authority-coverage-check`

### F-005 — WP09 assigns Pyrefly semantics to `Query` APIs that do not exist

**Severity:** blocker
**Category:** library
**Scope:** D-23, LD-25, WP09, WP11, WP19
**Finding:** WP09 requires direct pinned `Query` APIs for types, callees, members, imports,
cross-module context, and diagnostics. At exact revision `1933169...`, `Query` exposes file
change/add, attributes, callees, type tables, subtype, and qualified-target helpers; it has no
direct import-resolution, bulk definition/xref, or structured diagnostic surface, and
`add_files` returns rendered diagnostic strings. The pinned Pyrefly reference §16 assigns import
resolution and declared/computed/expected distinctions to TSP/module resolution, optional bulk
definitions/xrefs to Glean, and navigation fallback to LSP. The plan also omits the semantic
environment fingerprint, actual affected-module set, and conservative reverse-importer refresh
required by the pinned state/invalidation behavior.
**Required resolution:** Amend D-23, LD-25, Stage 2, and WP09 with a fact-family/surface matrix:
Query for bulk inferred types, callees, members, and subtype; TSP/module resolver for imports and
declared/computed/expected types; optional exact-revision Glean/internal adapter for bulk
definitions/xrefs; and LSP only where an accepted navigation fallback is required. Unsupported
families must emit explicit remainder/capability rows. Define one long-lived workspace state,
actual Pyrefly configuration and module resolution, `Require::Everything` versus
`Require::Exports` memory tiers, source-coordinate conversion, semantic-environment identity,
and affected-module/reverse-importer invalidation.
**Revalidation:** `just pyrefly-exact-surface-matrix-check && just pyrefly-semantic-environment-invalidation-check`

### F-006 — WP10 attributes private stable keys, borrowck, and derived dataflow to `rustc_public`

**Severity:** blocker
**Category:** library
**Scope:** D-23, LD-25, WP10–WP11
**Finding:** WP10 requires direct `rustc_public` item/MIR/instance/dataflow surfaces and proposes
`StableCrateId + DefPathHash` as the stable identity input. The exact public MIR API does not
provide those stable-key types or the full borrowck/dataflow result graph. GEN §5.2 and the MIR
reference §§27.1 and 37 assign stable compiler keys, exact source mapping, exact loans/regions,
and selected mono/vtable facts to a narrow `rustc_private` adapter; application analyses own
conservative dataflow derived from raw MIR access events.
**Required resolution:** Amend D-23/LD-25 and split WP10 into three explicit authorities:
`rustc_public` raw item/MIR/instance/access observations; a tiny exact-nightly
`rustc_private` enrichment seam for stable keys, source/hygiene, borrowck, and selected
vtable/mono facts where required; and application-derived dataflow with algorithm/version
provenance. If private enrichment is intentionally unavailable, use the documented application
qualified-name key and emit downgraded capability rather than claiming stable compiler identity
or exact borrowck. Compile- and behavior-probe the public and private seams independently; no
borrowed compiler type may escape the callback/process boundary.
**Revalidation:** `just rustc-public-private-authority-check`

### F-007 — The Rust semantic lane lacks an enforceable untrusted compilation sandbox

**Severity:** blocker
**Category:** operations
**Scope:** design §3.12, GEN AC-G-35, WP10, WP19–WP20
**Finding:** Rust semantic extraction may execute repository build scripts and procedural macros.
The MIR reference §53 and GEN AC-G-35 require an explicit trust policy, no network or inherited
credentials, read-only inputs, private bounded outputs, process/resource limits, and fail-closed
platform containment. WP09 names a Pyrefly sandbox check, but WP10 owns no Rust build-script/
proc-macro launcher contract or hostile fixture. Current Rust provider validation accepts a
claimed sandbox digest; it does not establish containment.
**Required resolution:** Add a design-bearing compiler trust contract and a WP10-owned launcher
packet: immutable workspace/dependency views, offline/minimal environment, credential removal,
private target/output, CPU/memory/time/process quotas, process-group termination, and an explicit
build-script/proc-macro policy. Untrusted execution must fail closed where platform containment
cannot be established. Any `TRUSTED_LOCAL` posture must be explicit, separately authorized, and
reported in capability/provenance.
**Revalidation:** `just rustc-untrusted-compilation-sandbox-check`

### F-008 — Independent expectations and the frozen comparator are produced after their consumers

**Severity:** major
**Category:** proof
**Scope:** WP04, WP07–WP10, WP18, WP20
**Finding:** WP03 correctly owns initial model expectations, but WP04 establishes only the
`ProviderBoundaryContract` port, not independently accepted rows for each provider. WP08–WP10
consume such rows without a predecessor packet that authors and accepts them. Query, public,
security, and activation expectations are likewise consumed before WP20 first assigns independent
owners; WP20 also first preserves the frozen comparator after shared implementation work has
begun. The DAG either blocks on undeclared inputs or permits implementers to arrange their own
expectations informally.
**Required resolution:** Add early, separately owned evidence packets: provider boundary rows
before WP08–WP10; query/public/security/activation expectations before WP13/WP18; and the exact
frozen legacy executable/worktree plus decoded comparison contract at the transition start. WP20
must only re-execute and compare preaccepted inputs. Any expectation change must create a
successor candidate epoch and rerun its proof.
**Revalidation:** `just independent-evidence-dag-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`

### F-009 — The logical-to-physical Arrow schema lifecycle has no owner

**Severity:** major
**Category:** design
**Scope:** LD-17, LD-20, WP04–WP06, WP17
**Finding:** The model requires canonical IDs such as Arrow `FixedSizeBinary(16)` plus semantic
metadata. Delta's logical type surface stores binary data as `BINARY` and reconstructs ordinary
Arrow `Binary`; the current `Id16ContractProvider` performs necessary expression and output
adaptation. WP17 proposes native Delta providers and deletion of wrappers without assigning
logical-to-physical cast maps, qualified `DFSchema` construction, projection/filter/statistics
index mapping, fixed-width and metadata restoration, or validation at analyzed, optimized
logical, initial/optimized physical, stream, batch, and sink boundaries. Removing the current
wrapper before an optimizer-visible replacement exists loses an enforced domain contract.
**Required resolution:** Add a model-derived `SchemaContract` decision and owner spanning
WP04/WP05/WP06/WP17. It must carry source schema identity, qualified logical schema,
logical-to-physical mapping, projection/filter/statistics remapping, nullability and extension
metadata, restoration rules, and every phase-boundary validation. Use native projections/views
or one generic transparent adapter; retain `Id16ContractProvider` until replacement proof closes.
Cover empty streams, wrong-width IDs, nested types, column mapping, and deletion vectors.
**Revalidation:** `just relational-schema-lifecycle-check && just delta-provider-contract-check`

### F-010 — WP12 defaults graph families below DataFusion's highest native rung and under-specifies custom execution

**Severity:** major
**Category:** library
**Scope:** D-24–D-25, WP12
**Finding:** WP12 mandates a `LogicalPlan::Extension` for each accepted bounded graph-analysis
family, while D-25 and v2 P14–P15 require native relational nodes where possible. DataFusion 55
already supplies joins, unions, windows, aggregates, and `RecursiveQuery`/
`RecursiveQueryExec` for suitable bounded recursive plans. The exact `ExecutionPlan` contract
also requires more than the packet names: expression visitation, child replacement/property
recomputation, `with_new_children` compatibility, `reset_state`, child statistics requests,
statistics-from-inputs, and physical invariant preservation. Compiler-required stubs can still
hide expressions, retain stale properties, or reuse invalid state.
**Required resolution:** Select the extension rung per compiled operation, not per family:
native relational/recursive plan; scalar/aggregate/window function; planning-time table
function/provider; then custom logical extension only for a proved irreducible relational-child
algorithm. Record and causally prove the selected rung. For each surviving custom node, make the
full DataFusion 55 logical/physical trait behavior, child arity, property recomputation,
statistics precision, repeated execution, memory, cancellation, and invariant checks explicit in
WP12.
**Revalidation:** `just graph-extension-conformance-check && just graph-execution-contract-check`

### F-011 — Reduced child catalogs do not neutralize authorities already bound inside views

**Severity:** major
**Category:** design
**Scope:** D-21–D-22, WP14
**Finding:** Building a fresh reduced `SessionState` is feasible and avoiding
`new_from_existing` is correct. However, DataFusion 55 `ViewTable` stores a prebuilt
`LogicalPlan`, explicitly does not validate/type-coerce it, and plans that stored tree through
the child session. Its `TableScan` nodes already hold provider `Arc`s and function expressions
can hold bound UDF `Arc`s; they are not necessarily re-resolved through the reduced catalog or
function registries. `RuntimeEnvBuilder::from_runtime_env` also retains the prior object-store
registry unless a fresh registry is installed. An allowed view can therefore carry a hidden
provider, function, nested view, extension, or object-store authority into the child.
**Required resolution:** Amend D-21/D-22 and WP14 to either recompile every public view from
model expressions against child-owned catalogs/functions, or recursively validate and seal the
complete bound dependency closure before registration. Install a fresh allowlisted object-store
registry while sharing only explicitly permitted memory/spill resources. Reject hidden providers,
UDF/UDAF/UDWF/table functions, subqueries, nested views, extensions, variables, and store URLs
before planning.
**Revalidation:** `just authorized-view-bound-authority-check && just access-catalog-isolation-check`

### F-012 — WP17 combines inert Delta selectors and leaves an incompatible optimize retry path

**Severity:** major
**Category:** library
**Scope:** D-26–D-27, WP17
**Finding:** At delta-rs revision `43a0cf10`, `TableProviderBuilder::build` uses a supplied
snapshot directly; `table_version` is read only when no snapshot is present. Chaining
`with_snapshot` and `with_table_version` therefore does not cross-check or strengthen the pin,
and `DeltaTable::table_provider()` already seeds the current snapshot. WP17's stated combined
recipe can silently serve the snapshot while the separately supplied version is inert. The
packet also requires zero library-internal retries for every optimize path, but the pinned
`OptimizeBuilder` hard-codes `DEFAULT_RETRIES + commits_made` and exposes no equivalent zero
retry contract at that commit site.
**Required resolution:** Define two mutually exclusive exact-load recipes: verified snapshot
plus session with no version selector, or log store plus version plus session with no snapshot.
Validate and record the observed snapshot root/version before registration. Explicitly forbid
the pinned `OptimizeBuilder` in the command-owned compaction route; use controlled write
primitives whose commit properties set zero retries, then return every conflict to
`FabricCommand` reconciliation. Add a structural guard against hidden optimize/DML retry paths.

**Revalidation:** `just delta-exact-version-reconstruction-check && just fabric-transaction-contract-check`

### F-013 — `StatisticsRequest` has no application producer, response mapping, or consumer

**Severity:** major
**Category:** library
**Scope:** LD-20, WP05, WP11
**Finding:** DataFusion 55 documents `StatisticsRequest` as transport vocabulary that DataFusion
itself neither populates nor consumes. It threads requests from a `TableScan` through `ScanArgs`;
the provider must expose answers through returned-plan `Statistics`, and application logic must
consume them. WP05/WP11 require a production optimizer request to reach each provider but define
no producer rule, response/precision mapping, consumer, unsupported semantics, or application
plan/cache identity. Forwarding an always-empty or semantically inert vector would satisfy the
named path check without proving an optimizer capability.
**Required resolution:** Either remove the claimed query-aware statistics feature and retain
ordinary honest provider statistics, or specify an application-owned logical optimizer producer,
provider-to-plan mapping, physical/optimizer consumer with an observable plan decision, exact/
inexact/unavailable semantics, and plan identity that includes the request set.
**Revalidation:** `just provider-statistics-contract-check`

### F-014 — The heterogeneous Arrow IPC relation protocol is not framed precisely enough to implement

**Severity:** major
**Category:** design
**Scope:** D-23, LD-17, LD-26, WP04, WP08–WP10
**Finding:** Provider run, schema, coverage, remainder, provenance, and fact-family relations
have different Arrow schemas. One Arrow IPC stream has one schema and stream-scoped dictionaries
until end-of-stream; heterogeneous `RecordBatch` schemas cannot be concatenated into the single
unspecified stream envelope. WP04 names an envelope and a Protobuf control frame but does not
define whether each relation receives its own stream, how streams are multiplexed, or how
relation/stream identity, dictionary scope, sequence, end-of-stream, coverage trailers, partial
failure, cancellation, and fingerprint validation compose. Independent provider packets cannot
implement one interoperable protocol from this contract.
**Required resolution:** Amend D-23/LD-26 and WP04 with an outer control protocol containing
relation ID, stream ID, schema fingerprint, sequence, and terminal status, with one independently
framed Arrow IPC stream per relation schema. Specify dictionary scope, bounded flow control,
interleaving rules, truncation, cancellation, trailer ordering, and explicit partial/unknown
coverage. Preserve Protobuf as control only.
**Revalidation:** `just relational-arrow-boundary-check && just provider-protocol-check`

### F-015 — Live importer and static migration-input removal is delayed long after it becomes safe

**Severity:** major
**Category:** legacy
**Scope:** design Stage 1, L-20–L-22/L-54, DB01, plan §8
**Finding:** The design says the one-time importer exits after accepted migration and its bounded
rollback window, and DB01 itself names M01 plus retained replay evidence as prerequisites. Plan
§8 nevertheless forbids every decommission batch until `LEGACY_RETIRED`, placing DB01 after
WP21/M05. The legacy runtime does not depend on the target importer, and a corrected target model
must be a new migration rather than reactivation of old static inputs. Keeping an executable
importer and live reads for the entire provider/query/publication/cutover program prolongs a
second authority and weakens the requested total purge.
**Required resolution:** Split DB01. Immediately after M01 and the explicitly bounded importer
rollback decision, seal the accepted migrations/review, delete the importer executable/build/
runtime route, and remove all live reads of static migration inputs. If immutable predecessor
bytes are still required for frozen comparison or old-binary rollback, move only those bytes to
a non-live archive with no build/runtime/tool reader. Delete that archive at final retention
expiry. Keep later consumer-first DB02–DB07 ordering.
**Revalidation:** `just model-importer-zero-state-check && just frozen-migration-input-live-read-zero-state-check`

### F-016 — The draft baseline no longer represents the finished cleanup state

**Severity:** minor
**Category:** factuality
**Scope:** plan §§1.3, 2.1, WP01
**Finding:** The plan records `bcee3f0...` and describes cleanup as potentially continuing. The
audited HEAD is four commits later, including generic-dispatch fixture and governed-proof
changes, and the user has now declared cleanup finished. The plan anticipates later WP01
reconciliation, but approval should not begin from an intentionally stale planning census when
the bounded cleanup endpoint is now available.
**Required resolution:** Produce plan v2 against the accepted cleanup HEAD, refresh the drift,
known-touch, baseline-failure, and current-tree statements, and explicitly disposition current
working-tree inputs. Do not restamp design/principle digests unless those source artifacts
actually change.
**Revalidation:** `just plan-baseline-freshness-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`

## Target-Design Assessment

The target should be retained as a relational fabric, not replaced with another registry or a
custom graph engine. The following design decisions remain strong:

- **D-20:** immutable migration replay under an exact compiler release is a genuine execution
  authority and correctly bounds unavoidable static bootstrap semantics;
- **D-21/D-22:** immutable epoch ownership and DataFusion's catalog hierarchy are the right
  runtime namespace and discovery architecture, subject to bound-view closure in F-011;
- **D-24:** typed programmatic compilation to visible DataFusion plans is preferable to SQL
  strings, generated plan catalogs, or operation dispatch;
- **D-25:** the native-first relational/petgraph hybrid is sound; WP12, not the design intent,
  selects the wrong default rung;
- **D-26/D-27:** one command path, exact component versions, activation-chain-derived current
  state, and Delta/Arrow physical state are sound, subject to the barrier, fence, schema, and
  exact-API corrections;
- **D-28/D-29:** proof-native governance and semantic-only FastMCP composition are fully aligned
  with the requested product boundary.

The design is not complete enough to remain v1 unchanged. It needs explicit amendments for:

1. raw-provider versus application-derived authority and the complete derived-analysis program;
2. the actual Pyrefly hybrid surface and rustc public/private authority split;
3. Rust compilation trust/sandbox posture;
4. logical-versus-physical schema adaptation across Arrow/DataFusion/Delta;
5. authorized view and object-store dependency closure;
6. relation-scoped Arrow IPC framing; and
7. a legacy fence that the frozen binary cannot bypass.

Clean-sheet challenge: after those amendments, this remains the preferred architecture absent
the current implementation. As written, it would not be selected clean-sheet because it assigns
facts to nonexistent APIs and relies on unenforceable transition/security claims.

## Library Capability Assessment

| Capability | Audit assessment | Required change |
|---|---|---|
| Arrow 59 schemas, arrays, IPC | Correct semantic boundary and one type universe. Strong use of nested/fixed types and zero-copy batches. | Add relation-scoped IPC multiplexing and a model-derived schema lifecycle; never rely on metadata alone. |
| DataFusion catalogs and information schema | Correctly used as runtime namespace and relational closure substrate. | Preserve bound dependency closure when constructing reduced child views and use a fresh object-store registry. |
| DataFusion `Expr`/`LogicalPlanBuilder` and function families | Strong native-first compiler direction. | Make rung selection causal per operation and keep bounded recursive/native forms ahead of custom nodes. |
| DataFusion custom providers | `ScanArgs`, pushdown, constraints, statistics precision, and transparent provider contracts are well chosen. | Define schema remapping and the complete application `StatisticsRequest` producer/consumer contract. |
| DataFusion logical/physical extensions | Appropriate only for irreducible graph algorithms with relational children. | Specify every DataFusion 55 rewrite, expression, reset, properties, statistics, resource, and invariant obligation. |
| delta-rs exact snapshots and plan writes | Exact-version providers and session-required plan writes are the right durable seam. | Use one selector authority, validate observed version/root, and forbid the pinned retrying `OptimizeBuilder` route. |
| Delta activation event log | Correct construction for cross-table atomic visibility under the supported single writer. | Fix admission ordering and add an external/bridge fence for the old binary. |
| Tree-sitter and Ruff | Direct exact-version adapters and raw kind/range/error retention are sound. | Do not label application CFG/dataflow as Ruff-native. |
| Pyrefly | Direct current-revision integration and Arrow IPC process isolation are correct. | Replace Query-only fiction with the exact hybrid surface, semantic environment, and affected-module invalidation. |
| `rustc_public`/MIR | Correct primary raw MIR surface and justified dated-nightly process boundary. | Add the narrow private enrichment seam, keep derived analysis application-owned, and prove sandbox containment. |
| petgraph | Correct transient kernel; canonical external IDs remain relational. | Use only after DataFusion relational reduction and only for irreducible algorithms. |
| FastMCP/Pydantic/gRPC | The retained presentation/control boundary remains appropriate. | Keep all Arrow/DataFusion/domain state in Rust and preserve released protocol compatibility. |

No replacement semantic library is indicated. The best solution is better use of the already
pinned libraries plus bounded application code for genuinely application-owned analysis,
schema, policy, and protocol contracts. The selected sandbox and legacy-fence designs may still
require an explicit platform/deployment capability; that decision must be made and proved rather
than hidden behind an internal compatibility layer.

## Work-Packet and Impact Assessment

| Packet group | Assessment |
|---|---|
| WP01 | Not executable as governed work until external governance remediation closes F-001. |
| WP02–WP03 | Replay, compiler-release identity, importer bijection, and independent model review are strong. Capture independent inputs early and split DB01 so the temporary path exits after M01. |
| WP04–WP07 | Good Arrow/catalog/compiler/proof foundation, but it must own relation-scoped IPC, schema lifecycle, and early independent provider/query/security inputs. |
| WP08–WP11 | Not executable until raw/derived authority is corrected and exact Pyrefly/rustc surfaces, semantic environment, sandbox, and provider-family coverage are dependency-closed. |
| New derived-analysis packets | Required between accepted raw providers and canonical/derived closure. One omnibus WP12 cannot implement GEN §§24–66. |
| WP12 | Retain a graph packet, but compile per-operation native/UDF/UDTF/extension selection and prove the full exact physical-node contract. |
| WP13 | The request-relation compiler is well aligned, but its semantic coverage depends on accepted derived producers and early independent expectations. |
| WP14 | Fresh reduced catalogs are correct; bound views, functions, extensions, variables, and object stores require recursive closure or child-side recompilation. |
| WP15–WP16 | Dynamic Rust delivery and one `FabricCommand` route are sound and well placed. |
| WP17 | Keep exact-version Delta and optimizer-visible overlays, but correct schema adaptation, selector semantics, and compaction retry control. |
| WP18 | Correct concept, unsafe ordering. Move the barrier before durable selection. |
| WP19 | Correct lifecycle/resource integration boundary; add Pyrefly semantic invalidation and the Rust trust launcher rather than assuming provider updates are already exact. |
| WP20 | Strong independent release dossier intent, but expectations and comparator capture must be predecessors, not deliverables first created here. |
| WP21 | A durable state machine is appropriate; the irreversible fence needs a mechanism the old binary cannot bypass. |
| DB01–DB07 | Coverage and zero-state posture are unusually strong. Execute the live importer/static-input teardown earlier, then retain the remaining consumer-first order. |

The known-touch census is broad and correctly includes hidden files, mixed roots, feature/build
targets, fuzz/package surfaces, and generated consumers. The major impact omission is semantic,
not textual: no packet owns much of the accepted analysis graph, and exact provider/security
surfaces are assigned to the wrong packets.

## Legacy, Transition, and Decommission Assessment

The plan's L-20–L-55 disposition matrix, hidden/package inventory union, consumer-first deletion,
positive replacement evidence, structural/textual zero-state proof, skipped-file accounting, and
history-versus-live distinction are strong and should be preserved.

Three transition corrections are mandatory:

- remove the live importer/static input route as soon as M01 and the bounded rollback decision
  permit, while retaining only non-live archival bytes required by explicit commitments;
- close admission before durable epoch selection; and
- replace journal-only legacy retirement with a fence enforced outside or by a bridge version of
  the frozen binary.

Total purge remains the correct completion condition. Historical plans, designs, decisions,
released allocations, and accepted evidence should remain immutable, but nothing in build,
runtime, tooling, tests, packaging, or active routing may consume the retired authorities.

## Proof and Validation Assessment

The plan has strong proof structure on paper: packet-local behavioral/structural/negative/
operational checks, 84 named oracle entries, integration milestones, fault matrices, Miri only at
new concurrency seams, exact-provider fixtures, clean reconstruction, and extensive zero-state
proof. It correctly rejects a broad green suite as a substitute for semantic proof.

The principal proof defects are upstream of test execution:

- several named checks would exercise nonexistent or inert library surfaces;
- independent evidence is first produced after its consumers;
- no oracle covers the missing derived-analysis families;
- no hostile Rust compilation sandbox oracle exists;
- `StatisticsRequest` forwarding can pass without a producer or consumer;
- child-catalog tests do not challenge pre-bound view authorities; and
- the legacy-writer check cannot prove revocation unless it actually restarts the exact frozen
  binary after `NEW_MUTATING`.

The current full test suite was intentionally not rerun. Re-audit does not require ceremonial
broad validation before the plan is corrected; it requires the focused API/semantic/security/
transition probes named by these findings, followed by the ordinary repository gates at the
owning packets.

## Doctrine and Anti-Principle Assessment

The target strongly advances v2 P1–P3, P7–P12, P17–P20, P25–P31, and P33–P36 through executable
relational authority, immutable epochs, replay, proof, provenance, causal declarations, and one
command path.

Open findings currently violate these principles:

| Doctrine | Violation |
|---|---|
| P3 one authority; P9/P10 provenance | F-004 mislabels application derivations as provider-native facts. |
| P4/P20 exact capability truth | F-005/F-006 claim unsupported current API surfaces. |
| P12 executable schema | F-009 leaves logical/physical schema restoration without an owner. |
| P13/P21 least authority | F-011 allows pre-bound view/store authorities to bypass reduced registries. |
| P14/P15 highest visible extension | F-010 defaults native-expressible work to opaque extensions. |
| P22 canonical protocols | F-014 leaves heterogeneous IPC framing ambiguous. |
| P25 executable oracle; P30 independent expectations | F-008 orders proof inputs after consumers. |
| P31 eliminate forgotten synchronization | F-013 creates a request channel with no causal consumer. |
| P34 one replayable mutation path | F-002 and F-012 weaken activation/write causality. |
| P36 executable governance | F-001 governs the successor through work outside any governed DAG. |

The corrections reinforce the v2 pivot; none requires reintroducing static current registries,
generated semantic bundles, opaque compatibility DTOs, or defensive abstraction against future
library changes.

## Top Required Changes

1. Complete candidate-validation and activation remediation under a separately governed active
   plan; then issue successor plan v2.
2. Amend the design with exact raw-provider versus derived-analysis authority and add complete
   Python, Rust, graph, effect/resource, and summary packets.
3. Replace Pyrefly Query-only and rustc-public-only claims with exact surface/authority matrices.
4. Add the untrusted Rust compilation launcher and hostile sandbox proof.
5. Add a model-derived logical/physical schema contract and relation-scoped Arrow IPC protocol.
6. Make DataFusion graph rung selection per operation, specify the full custom physical contract,
   and close bound authorities in child views/runtime stores.
7. Define or remove the application `StatisticsRequest` feature; correct Delta selector and
   optimize retry semantics.
8. Fix the activation barrier and select a durable old-binary fence.
9. Produce independent provider/query/public/security expectations and freeze the comparator
   before their consumers.
10. Split early live importer teardown from final archive deletion and refresh the plan baseline
    to the completed cleanup tree.

## Re-Audit Scope

Re-audit a versioned design successor and implementation plan v2 after all findings have explicit
dispositions. The next audit should verify:

- current plan/design/principle digests and baseline freshness;
- externally governed candidate-readiness machinery;
- exact compile/behavior probes for every selected Pyrefly and rustc public/private surface;
- full accepted fact-family-to-producer closure;
- hostile Rust sandbox behavior;
- Arrow schema/IPC, DataFusion native/extension/view/statistics, and Delta selector/retry probes;
- independent evidence producer dependencies;
- concurrent activation ordering and exact old-binary restart fencing; and
- early and final legacy zero-state boundaries.

Broad repository tests need not be repeated merely to re-establish this audit's design findings.
Run them only after the revised packets and focused closure checks make the target executable.
