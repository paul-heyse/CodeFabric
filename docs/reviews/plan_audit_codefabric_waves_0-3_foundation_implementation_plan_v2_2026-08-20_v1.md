# Plan Audit: CodeFabric Waves 0-3 Foundation Implementation Plan v2

## Provenance and Scope

- **Audited plan:** [`docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v2_2026-08-20.md`](../plans/codefabric_waves_0-3_foundation_implementation_plan_v2_2026-08-20.md), SHA-256 `fd2edf3d8bfdf5e2e9f2e3425c8a80005e2db8828cb612122b1908be4a882970`.
- **Declared design:** [`codefabric_1.3_implementation_roadmap_v1.0.md`](../upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md), SHA-256 `06f3a9d530db83d514b6ef2c148cce87482dccff277d3eee628c1994e20ea2c3`.
- **Baseline checked:** commit `e14e175071df5c98dfcdeba81dd8bcca3fe91fb0` plus the current working tree on 2026-08-20. All eight design digests recorded by the plan match the current files.
- **Authority reviewed:** the roadmap; governance manifest; ontology, generation, fabric, query, lifecycle, and serving specifications; repository/tooling specification; holistic doctrine; the exact-version DataFusion/Arrow, delta-rs, gix, FastMCP, and Pydantic references; the pinned delta-rs source at rev `9f9223197469897ef05ae4369eb4fd1390174e65`.
- **Repository evidence:** `just --list`; `git status --short`; SHA-256 recomputation; targeted outlines and source reads; exact-source API inspection; `uv run pyrefly check`; and the required pre-edit `just ci-fast` run.
- **Independent challenge:** one fresh-context design challenger and one focused library/API specialist were used. Their claims were independently checked against the permanent owners before inclusion.
- **Mutation boundary:** the repository was treated as read-only except for this new audit report.

The current `just ci-fast` baseline is not clean. Rust formatting/check/Clippy, Ruff, nine nextest tests, two doctests, Maturin development install, and twelve pytest tests passed. The command exited `2` because Typos found pre-existing findings in the prior audit and delta-rs recommendation documents. Pyrefly exited `0` while warning that `python/**/*.py` was excluded, so its green result did not prove coverage of the current Python package.

## Executive Summary

The plan is substantially stronger than a normal first-production implementation plan. Its clean cutover from the PyO3 seed, process/build-domain isolation, explicit authority boundaries, exact storage/query version family, fault-point discipline, multi-table publication model, and pinned-query proof are sound directions. The underlying 1.3 architecture remains viable; this audit does **not** recommend replacing it.

The plan is not executable as written. Four blockers must be closed before implementation:

1. Gate A is made green through locally invented contracts and a `deferred-mapping` exception that the governing manifest does not permit.
2. Wave 2 reaches `WorkspaceLifecycle.READY` without the first active snapshot, an explicitly illegal transition.
3. WP24 activates a `ServingSnapshot` before its immutable exact-version provider/catalog set exists.
4. WP17 requires successful SHA-256 repository behavior while gix is compiled with only `sha1`.

The most important design improvement is to make `TableSpec` express three orthogonal concerns—durable mutation, overlay mutation, and materialization/catalog role—instead of resolving their current conflict by mapping query-time-derived state to `OPERATIONAL_ONLY`. The most important library correction is to use delta-rs's actual application-transaction API as the primary idempotency candidate; the plan's statement that the pinned revision lacks that API is false.

## Readiness Verdict

**Verdict: `needs-revision`.**

Execution should not begin until F-001 through F-004 are closed and the revised dependency/order graph is re-audited. The target architecture is not materially invalidated, so `needs-redesign` is not warranted. Several corrections require accepted edits to permanent design owners before the implementation plan can be restamped.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | design | WP07-WP11, M02, A-01-A-56 | open |
| F-002 | blocker | sequence | WP14, WP18, M03, WP24 | open |
| F-003 | blocker | sequence | WP19, WP23-WP25, M04 | open |
| F-004 | blocker | library | LD-06, WP17, M03 | open |
| F-005 | major | factuality | WP01, WP05, M01, WP17, WP19 | open |
| F-006 | major | design | A-11, WP09, WP21, WP23, WP25 | open |
| F-007 | major | library | LD-04, A-33, R-02, WP21 | open |
| F-008 | major | library | WP15, LD table | open |
| F-009 | major | proof | LD-10, WP05, WP14 | open |
| F-010 | major | impact | WP16, M03, G-33 | open |
| F-011 | major | impact | WP12, WP14, M03, G-62 | open |
| F-012 | major | proof | WP06-WP10, WP14, WP21-WP24, M02-M04 | open |
| F-013 | minor | factuality | plan section 3, section 14 | open |
| F-014 | minor | context-efficiency | all packets, section 15, state path | open |
| F-015 | minor | library | WP25 | open |
| F-016 | minor | library | LD-08, WP13 | open |

## Findings

### F-001 — Gate A is weakened and missing normative contracts are delegated to implementation

**Severity:** blocker  
**Category:** design  
**Scope:** WP07-WP11, M02, A-01-A-56  
**Claim:** The plan cannot truthfully close Gate A because it both permits implementation-time invention of missing normative values and treats non-executable phrase mappings as executable.
**Evidence:** Plan section 1.3 authorizes `AUTHOR` dispositions; A-02-A-05, A-08-A-09, A-11, A-15-A-20, A-27, A-35-A-36, A-38, A-40, A-44-A-45, A-52-A-53, and A-55 assign missing contract values during packets. WP08b and WP11 say `deferred-mapping` records keep AC-G-04 green. The governance manifest AC-G-04 instead requires CI failure for every query phrase with no executable mapping, and Part V states that an implementation agent shall not invent a missing gate contract. Roadmap section 28 requires ambiguities to return to their owning 1.3 specification. Ontology AC-G-70 makes completed machine registries code-generation authority; it does not authorize an implementer to invent missing registry schemas, identity encodings, protocol names, or cross-spec semantics.
**Impact:** Gate A can report success while normative choices remain unowned, making generated identifiers and protocols appear stable before design acceptance. Later correction would be a breaking contract migration, not an implementation fix.
**Required resolution:** Amend the owning permanent documents or accept separately reviewed machine-contract source artifacts before implementation. Limit Wave 1 authoring to enumerating values inside already-complete, accepted registry schemas. Remove the `deferred-mapping` exception, or amend AC-G-04 explicitly with lifecycle/version semantics for that state. Make accepted design/contract amendments an entry gate for the affected packets.
**Revalidation:** Every `AUTHOR`, `DEV`, and `ISSUE` row has an accepted owner disposition; `codefabric-contracts verify --profile released` rejects a phrase without a real executable mapping; no packet instructs an agent to choose a normative value absent from its owner.

### F-002 — Wave 2 reaches `READY` before a valid snapshot exists

**Severity:** blocker  
**Category:** sequence  
**Scope:** WP14, WP18, M03, WP24  
**Claim:** WP18's required `bootstrap -> READY` transition is illegal under the lifecycle contract because Wave 2 explicitly has `NO_SNAPSHOT` and the first snapshot cannot activate until WP24.
**Evidence:** WP18 defines `active_snapshot = NO_SNAPSHOT` yet requires three register/enable/bootstrap flows to reach `READY`; WP14 also tests a state walk through `READY`. Lifecycle section 154 says an active snapshot must be constructed before a worktree is `READY`. AC-G-25 defines the mandatory transition `BOOTSTRAPPING -- first valid snapshot activated --> READY` and requires illegal transitions to fail rather than be coerced.
**Impact:** The machine-checked lifecycle either rejects M03 or the implementation weakens the governing transition table. A client could observe readiness while no query pin exists.
**Required resolution:** Keep `WorkspaceLifecycle` in `BOOTSTRAPPING` (or add a design-owned pre-ready state) through M03. Expose source-control-plane health as an orthogonal status. Move the only first `READY` transition into WP24 after complete candidate-snapshot construction and activation.
**Revalidation:** Model checking proves `READY => active_snapshot exists and is ACTIVE`; an M03 restart never reports `READY`; the WP24 activation event is the only path from initial bootstrap to `READY`.

### F-003 — Snapshot activation precedes construction of the snapshot-owned provider set

**Severity:** blocker  
**Category:** sequence  
**Scope:** WP19, WP23-WP25, M04  
**Claim:** WP24 activates and swaps a `ServingSnapshot` before WP25 constructs the exact-version Delta providers and private DataFusion catalog that the fabric specification defines as immutable members of that snapshot.
**Evidence:** WP24 builds and activates an empty-overlay manifest, then WP25—dependent on WP24 and WP23—builds the exact-version providers and catalog from a leased snapshot. Fabric sections 12.6 and 91 require exact versions to resolve, providers to be created and registered, overlay wrappers and integrity checks to run, and the catalog/provider set to freeze **before** atomic activation; every lease then reuses those provider objects. Fabric section 98.1 also requires every Delta handle to have exactly one declared access profile, including `QUERY_SERVING` with `skip_stats = false`; the plan does not assign those profiles across WP19-WP25.
**Impact:** An active snapshot can exist without the object graph that makes table-version/provider drift impossible. Later provider construction can fail or observe a different engine state after the authoritative pointer has moved.
**Required resolution:** Split provider/catalog substrate from serving views. Build an exact-version `DeltaBaseCatalog`, empty-overlay wrapper, access-profile-aware Delta handle factory, and private catalog before WP24 activation; run integrity checks and freeze them into the candidate snapshot. WP23 may then construct a replacement snapshot with a populated overlay. Keep user-facing views and the pinned-query proof in the final packet.
**Revalidation:** A deterministic ordering test proves `resolve versions -> construct providers -> wrap -> validate -> freeze -> durable activate -> ArcSwap`; each lease receives pointer-identical provider objects; no provider is rebound; all handle construction requires an access-profile enum and query providers retain statistics.

### F-004 — The SHA-256 gix acceptance fixture cannot pass with the declared feature set

**Severity:** blocker  
**Category:** library  
**Scope:** LD-06, WP17, M03  
**Claim:** WP17 requires a SHA-256 object-format repository to yield correct DTOs, but LD-06 enables gix `sha1` and omits the compile-time `sha256` feature.
**Evidence:** WP17's positive fixture roster includes SHA-256. The pinned gix 0.86 reference section 1A and section 7 state that hash algorithms are feature-gated and `sha256` is opt-in. Lifecycle section 39.1 allows `sha256` for compatibility testing but warns that the feature alone does not prove complete SHA-256/reftable parity.
**Impact:** The declared positive acceptance test is unsupported by the selected build and is likely to fail before application behavior is reached.
**Required resolution:** Either enable `gix/sha256` and treat repository support as an explicit compatibility probe with a fail-closed replan trigger, or make SHA-256 a typed unsupported-format negative fixture. Do not claim full parity from feature presence.
**Revalidation:** Build the exact feature graph, open real SHA-1 and SHA-256 repositories through `GitStateAdapter`, and assert algorithm-tagged IDs and widths without string assumptions; or prove the negative fixture returns the registered capability error.

### F-005 — M01 does not resolve the stable production graph it claims to establish

**Severity:** major  
**Category:** factuality  
**Scope:** WP01, WP05, M01, WP17, WP19  
**Claim:** Wave 0 can close with an empty stable root graph even though the roadmap requires Wave 0 to pin and validate the real Arrow/Parquet/DataFusion/object_store/delta-rs/gix/Tokio compatibility domain.
**Evidence:** Roadmap Wave 0 names those dependencies and requires duplicate-family rejection. WP01 instead requires zero dependencies and no feature table; gix arrives in WP17 and the storage/query graph in WP19. M01 admits that duplicate-family enforcement is only a synthetic fixture until WP19. This also contradicts plan section 1.2, which says WP01 declares `local-workstation`/`s3-storage`, and Fabric section 2.1, which requires that feature table. WP05 discovers system versus vendored `protoc` but does not require one exact compiler identity even though generated files must be byte-identical.
**Impact:** The highest-risk pre-release dependency, MSRV, transitive kernel, feature, and generator interactions remain unknown until after two milestones and substantial contract work.
**Required resolution:** Add a Wave-0 stable-stack compatibility slice using the actual production manifest and lock: exact Fabric pins, resolver 3, the required feature table, gix profile, and an exact Protobuf generator/toolchain identity for Rust and Python. Use a production-boundary compile smoke or explicitly time-bounded dependency-hygiene exceptions rather than a synthetic second graph. Add a metadata validator for the actual resolved family/kernel invariants.
**Revalidation:** A clean locked build resolves the complete graph; `cargo tree -e features` proves default/S3 isolation and gix features; the actual graph has one approved family/kernel line; exact-version provider/session/schema/gix probes compile; two clean generations produce byte-identical stubs with recorded generator versions.

### F-006 — `TableSpec` conflates durable mutation, overlay mutation, and catalog role

**Severity:** major  
**Category:** design  
**Scope:** A-11, WP09, WP21, WP23, WP25  
**Claim:** The proposed `TableSpec` cannot represent the fabric design faithfully because it uses one overlay field to absorb concepts from three independent taxonomies and maps query-time-derived state to `OPERATIONAL_ONLY`.
**Evidence:** Fabric section 68 defines durable table mutation classes. AC-G-21 separately defines five overlay mutation policies and says `OPERATIONAL_ONLY` is absent from `ServingSnapshot` effective fact tables. Fabric section 91 additionally requires an explicit `query-time derived` policy for some non-owner/global surfaces. Plan A-11 resolves this by adding `overlay_policy`, retaining a separate owner policy, and mapping query-time-derived to `OPERATIONAL_ONLY`/view-backed surfaces; WP09 also describes `current-singleton` as though it were an overlay policy.
**Impact:** Valid query-visible derived surfaces can be classified as non-snapshot operational state, while singleton/durable-write behavior leaks into overlay semantics. Generators cannot enforce legal combinations without ad hoc exceptions.
**Required resolution:** Amend the fabric owner to define three orthogonal fields: durable mutation class, overlay mutation policy, and materialization/catalog role (including a distinct query-time-derived role). Define a validity matrix and mappings for every table before WP09 generates code.
**Revalidation:** Every table has exactly one value on each applicable axis; `OPERATIONAL_ONLY` never backs a query-visible effective fact; generated negative fixtures reject invalid cross-products; WP21, WP23, and WP25 consume only their owned axis.

### F-007 — The delta-rs idempotency premise is false and bypasses native functionality

**Severity:** major  
**Category:** library  
**Scope:** LD-04, A-33, R-02, WP21  
**Claim:** The plan incorrectly states that pinned delta-rs has no application transaction API and therefore overweights a custom outcome-reconciliation fallback.
**Evidence:** LD-04, A-33, and R-02 say no app-id/version transaction exists at rev `9f922319...`. The exact pinned source exposes `CommitProperties::with_metadata`, `with_application_transaction`, and `with_application_transactions`; `Transaction::new(app_id, version)` is documented as enabling idempotency, and `Snapshot::transaction_version` reads the committed version. The same source contains conflict tests for duplicate application transactions. See the [pinned transaction source](https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/transaction/mod.rs) and [application transaction tests](https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/transaction/application.rs).
**Impact:** WP21 may build avoidable custom idempotency machinery and still fail to use the transaction marker that participates in delta-rs conflict detection and history.
**Required resolution:** Correct LD-04/A-33/R-02. Make application transactions plus commit metadata the primary candidate, with an explicit CodeFabric mapping from stable application identity to monotonic `i64` transaction version. Retain external operation records only for multi-table orchestration and recovery evidence that Delta's per-table transaction action cannot own.
**Revalidation:** Exact-revision compile and behavior probes cover first commit, duplicate retry, concurrent duplicate, reload/restart, monotonic advance, and metadata persistence; the fallback is deleted or its residual responsibility is narrowly documented.

### F-008 — The secure-open packet lacks a safe syscall implementation decision

**Severity:** major  
**Category:** library  
**Scope:** WP15, LD table  
**Claim:** WP15 requires `openat2`, `openat`, and `fstatat` semantics while preserving `unsafe_code = deny`, but the plan selects no library that exposes those operations safely and its static proof checks only one forbidden read API.
**Evidence:** WP15 requires Linux resolve flags, a fallback walk, macOS directory-relative opens, and denial across devices. The Rust standard library does not provide the complete `openat2` interface. `rustix` 1.1.4 is a verified candidate: it exposes safe `openat2`/`openat` returning `OwnedFd`, and `ResolveFlags::{BENEATH, NO_MAGICLINKS, NO_SYMLINKS, NO_XDEV}`. See [rustix `openat2`](https://docs.rs/rustix/1.1.4/rustix/fs/fn.openat2.html), [`openat`](https://docs.rs/rustix/1.1.4/rustix/fs/fn.openat.html), and [`ResolveFlags`](https://docs.rs/rustix/1.1.4/rustix/fs/struct.ResolveFlags.html). WP15's proposed governance rule only bans `std::fs::read`; gix later performs advisory path opens internally.
**Impact:** The packet is not dependency-closed and may either require forbidden first-party unsafe code or silently weaken confinement. Its literal “every filesystem read” proof is not established.
**Required resolution:** Add an exact-version syscall-wrapper library decision and use descriptor-relative APIs; use `NO_XDEV` plus fallback device checks. Narrow the invariant to authoritative source-byte reads, keep gix reads advisory, and revalidate identity after any unavoidable library reopen. Expand static rules to all forbidden direct open/read surfaces.
**Revalidation:** Platform compile probes and adversarial rename/symlink/mount swaps pass on Linux and macOS; AST positive/negative fixtures prove the rule's coverage; first-party unsafe remains absent.

### F-009 — The UDS library probe omits mandatory peer identity

**Severity:** major  
**Category:** proof  
**Scope:** LD-10, WP05, WP14  
**Claim:** The tonic/prost compatibility probe can pass without proving the OS peer-credential check that the plan and AC-G-61 require before handler dispatch.
**Evidence:** LD-10 acknowledges mandatory peer credentials, but WP05 probes UDS transport, `oneof`, message limits, and Python interoperability only. WP14 then reuses that result for administrative IPC. Tokio exposes peer credentials on Unix streams, but tonic integration must carry the accepted connection identity into request handling; mere UDS connectivity does not prove this.
**Impact:** The selected transport can compile and round-trip while lacking the same-user authorization boundary assumed by later packets.
**Required resolution:** Extend WP05 to prove peer credential extraction and propagation through the exact selected tonic incoming-stream mechanism. Fail closed when credentials are missing or mismatched. Assert encode and decode size limits on both sides.
**Revalidation:** Same-UID succeeds; different-UID fails where the platform permits the fixture; missing credentials fail before RPC dispatch; handler instrumentation proves rejected requests never enter application code.

### F-010 — Source-blob lease lifetime and reclamation have no owner

**Severity:** major  
**Category:** impact  
**Scope:** WP16, M03, G-33  
**Claim:** WP16 issues and persists source-snapshot leases but assigns no packet responsibility for release, restart orphan recovery, or garbage collection.
**Evidence:** WP16 implements capture and lease issuance and proves blob immutability, but its outcome and acceptance criteria contain no lease terminal states or deletion behavior. Fact Generation AC-G-33 requires runtime source blobs to remain while any provider, snapshot, or source-artifact lease exists and to be removed after all such leases release.
**Impact:** Implementations can leak all captured source bytes indefinitely or delete bytes still needed by a live consumer; neither behavior is caught by M03.
**Required resolution:** Add a dependency-closed lease-lifecycle portion to WP16 or a successor before M03: holder kinds, acquire/renew/release, crash orphaning, grace policy, idempotent bounded GC, and atomic delete eligibility. Coordinate but do not conflate this with WP24's serving-snapshot lease table.
**Revalidation:** A leased blob cannot be deleted; concurrent release/GC is race-safe; all holders released leads to eventual deletion; restart recovers or safely orphans holders; repeated cleanup is idempotent.

### F-011 — G-62 is reported complete without its command and drain contract

**Severity:** major  
**Category:** impact  
**Scope:** WP12, WP14, M03, G-62  
**Claim:** The completion inventory marks G-62 fully realized even though the packets omit mandatory daemon status/stop/drain, credentials, service integration, and drain-and-restart behavior.
**Evidence:** WP12 supplies `serve`, `check-config`, discovery, singleton lease, and a shutdown skeleton; WP14 supplies workspace administration. Lifecycle AC-G-62 additionally requires `codefabric daemon status|stop|drain`, `codefabric contracts verify`, `codefabric credentials ...`, service integration, and a drain procedure that rejects ingress, lets work terminate, handles overlay state, checkpoints SQLite, and exits. Plan section 16 marks G-62 fully realized at M03.
**Impact:** Machine traceability will claim a permanent contract is complete while major operational and upgrade behavior remains absent.
**Required resolution:** Prefer honest staged conformance: mark G-62 partial and name exact later-wave owners for credentials/service/fully populated drain, while implementing the Wave-appropriate status/stop/drain shell and no-work drain semantics now. If full realization is retained, add all obligations and tests before M03.
**Revalidation:** The G-* inventory agrees with executable commands and tests; every deferred AC-G-62 clause has one named packet; drain rejects new work, checkpoints state, and exits within a tested deadline.

### F-012 — The proof plan omits risk-triggered fuzzing and mutation testing for new protocol/state surfaces

**Severity:** major  
**Category:** proof  
**Scope:** WP06-WP10, WP14, WP21-WP24, M02-M04  
**Claim:** Section 14.1 incorrectly concludes that no Wave 0-3 surface justifies fuzzing or mutation testing despite introducing decoders, serializers, a grammar, Protobuf protocols, and critical state transitions.
**Evidence:** WP06-WP10 add JCS and CBEF decoders/serializers, an EBNF grammar, registry parsers, JSON Schema, and four Protobuf packages. WP14 and WP21-WP24 add lifecycle, publication, retry, activation, and lease state machines. The repository evidence policy names parser, decoder/serializer, protocol, and compact state-machine inputs as `fuzz/` triggers and recommends focused mutation testing for core validation/state-transition logic. The plan substitutes property tests only and postpones fuzzing to real providers in Wave 4.
**Impact:** Malformed-input reachability and assertion strength at the foundational contract and recovery boundaries remain unmeasured; property cases alone do not explore parser state space or prove tests detect plausible validation faults.
**Required resolution:** Add `fuzz/` when the first production decoder lands, targeting production JCS/CBEF/registry/protocol decode paths and seeded from KAT/negative corpora. Add focused mutation campaigns for canonicalization, transition validation, publication retry, and activation ordering. Keep campaigns bounded and risk-triggered, not universal gates.
**Revalidation:** Corpus replay is deterministic; bounded milestone/scheduled fuzz runs retain crashes; targeted modules meet an accepted mutation outcome with every survivor classified; coverage shows the error paths are reached.

### F-013 — The recorded clean baseline is factually stale and Pyrefly can pass without checking sources

**Severity:** minor  
**Category:** factuality  
**Scope:** plan section 3, section 14  
**Claim:** The plan's “no pre-existing failures” and enumerated working-tree identity do not describe the current tree.
**Evidence:** `git status --short` contains modified and untracked files absent from section 3's list, including skill files, a delta-rs recommendation, and the replacement delta reference. `just ci-fast` exits `2` on pre-existing Typos findings. `uv run pyrefly check` exits `0` but warns that `python/**/*.py` is excluded. The plan's eight design digests and baseline commit still match, so this is execution provenance drift rather than design-source drift.
**Impact:** Packet attribution can incorrectly label inherited failures as plan-caused, and a green type gate can be a false negative.
**Required resolution:** Commit or explicitly snapshot the intended baseline, restamp the inventory/failure fingerprints, and remove the “none” claim. For the new adapter project, add a coverage sentinel proving an included file with a known type error makes the configured Pyrefly recipe fail.
**Revalidation:** Clean-checkout `just ci-fast` is green or every accepted baseline failure is fingerprinted; `git status` matches the recorded identity; the Pyrefly sentinel fails and passes as expected.

### F-014 — The four-wave artifact overstates packet parallelism and weakens resumability

**Severity:** minor  
**Category:** context-efficiency  
**Scope:** all packets, section 15, state path  
**Claim:** The plan's declared parallel packets are not file-disjoint, and one 2,705-line artifact/state stream spans four change-heavy waves despite the roadmap's per-wave planning guidance.
**Evidence:** WP02 and WP03 both edit `scripts/bootstrap.sh` while section 15 calls WP02-WP04 disjoint. WP08b, WP09, and WP10 all touch generator/verifier infrastructure or generated consumers while section 15 calls their outputs disjoint. Roadmap section 28 recommends a separate detailed document with four to eight packets for each wave. This plan declares 25 packets and a state path that does not yet exist.
**Impact:** Parallel execution can create merge/regeneration conflicts, and a late spec/probe change forces re-reasoning across unrelated completed waves.
**Required resolution:** Split the executable artifact into four chained versioned plans, retaining a small Waves 0-3 program map, or explicitly justify the variance. Serialize shared-file integration or define isolated ownership plus a merge-and-regenerate packet. Initialize durable execution state when the first plan is accepted.
**Revalidation:** The packet DAG's parallel branches have disjoint write sets; shared generation runs after merge; each executable plan has a bounded context, independent baseline, and resumable state.

### F-015 — The read-only SQL gate leaves DataFusion statements enabled

**Severity:** minor  
**Category:** library  
**Scope:** WP25  
**Claim:** Disabling only DDL and DML with `SQLOptions` does not implement the stated read-only SQL surface because statement commands remain allowed by default.
**Evidence:** WP25 says DDL/DML are disabled. The pinned DataFusion 54 reference documents `allow_statements` as a separate option and shows that read-only posture also requires `with_allow_statements(false)`; it also cautions that `SQLOptions` is not a complete sandbox.
**Impact:** `SET`/`SHOW`/`RESET`-class statements can remain admitted contrary to the packet outcome, and later caller-controlled SQL could access unapproved providers/functions without a plan lint.
**Required resolution:** Add `with_allow_statements(false)` and retain a logical-plan allowlist for providers/functions and direct-file scans whenever caller-controlled SQL exists.
**Revalidation:** Positive `SELECT` passes; DDL, DML, statements, direct-file references, and unauthorized providers/functions fail before execution.

### F-016 — SQLite online backup is not feature-closed

**Severity:** minor  
**Category:** library  
**Scope:** LD-08, WP13  
**Claim:** WP13 requires rusqlite online backup, but LD-08 identifies only bundled SQLite and does not require rusqlite's separate `backup` feature.
**Evidence:** WP13 mandates an online backup before migration. Rusqlite exposes its backup module behind the `backup` Cargo feature; `bundled` controls how SQLite is supplied, not whether backup APIs compile. See the [official rusqlite feature manifest](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml).
**Impact:** The selected dependency can satisfy the documented LD row yet fail to compile WP13 or prompt an unsafe file-copy substitute against a live WAL database.
**Required resolution:** At exact-version adoption, enable `bundled` and `backup`, use `rusqlite::backup` and `TransactionBehavior::Immediate`, and record the feature graph in LD-08.
**Revalidation:** Back up a live WAL database with an active reader, restore into a fresh database, and prove migration failure leaves the source and restored logical state coherent.

## Target-Design Assessment

The target design is directionally sound and should be preserved: one Rust authority process, a thin generated-contract Python/FastMCP adapter, isolated compiler/type-checker domains, immutable source images, a single canonicalization authority, exact durable table versions, a distinct current-snapshot pointer, and lease-pinned query execution. The seed's PyO3/Maturin surface is correctly treated as disposable rather than evolved into the production architecture.

Four design clarifications are required before coding: the Gate-A contract owners and deferred-mapping semantics (F-001), the pre-snapshot workspace state (F-002), the snapshot-owned provider lifecycle (F-003), and the orthogonal `TableSpec` model (F-006). These are amendments to the existing architecture, not reasons to replace it.

## Library Capability Assessment

The Arrow 58.4.0, DataFusion 54.1.0, Parquet 58.4.0, object_store 0.13.2, delta-rs `9f922319...`, and kernel 0.25.x family is internally aligned. The plan also correctly prefers DataFusion `ViewTable`/`MemTable`, bounded memory plus spill, delta-rs exact-version providers, and gix's narrow non-default profile.

Required capability corrections are concentrated and actionable:

- delta-rs already provides commit metadata and application transactions; use them before custom retry state (F-007);
- gix needs either `sha256` or an explicitly negative SHA-256 contract (F-004);
- a safe syscall wrapper such as rustix closes WP15 without first-party unsafe (F-008);
- the tonic probe must cover OS peer identity, not merely UDS transport (F-009);
- DataFusion statements need their own deny switch (F-015);
- rusqlite online backup needs its `backup` feature (F-016).

One additional leverage improvement is recommended but is not readiness-blocking: WP04 should use FastMCP 3.4.7's in-memory `Client(mcp)` to prove initialize/ping/list-tools through the real protocol pipeline, while retaining the subprocess test for STDOUT isolation. `attrs`/`cattrs` should remain out of WP04 unless a bounded internal model seam justifies them; Pydantic already owns the public/settings boundary.

## Work-Packet and Impact Assessment

The packets are unusually explicit about outcomes, dependencies, change surfaces, negative evidence, rollback, and replan triggers. WP22's conditioned-current-pointer protocol, WP23's overlay/rebase equality, and WP24's crash injection are especially strong.

Impact coverage is incomplete at three boundaries: Wave 0 postpones the actual stable dependency graph (F-005), source-blob lease reclamation has no owner (F-010), and the G-62 completion claim outruns its packets (F-011). The packet DAG also needs a shared-file integration correction and should be decomposed by wave for reliable execution state (F-014).

## Legacy, Transition, and Decommission Assessment

The legacy matrix is strong. It names PyO3, Maturin, the private native module, the mixed Python package, old tests, and stale documentation; it supplies negative removal proofs and preserves the one-package/one-integration-target repository constraints. The planned native-daemon plus adapter boundary is a genuine replacement, not a second implementation beside the seed.

No additional production legacy needs preservation. The only transition correction is procedural: do not decommission against a falsely clean baseline, and ensure the replacement adapter's type checker demonstrably includes its sources (F-013).

## Proof and Validation Assessment

The plan has strong deterministic, cross-language, negative, crash, and adversarial evidence. It distinguishes nextest from doctests, dev installs from artifact proof, synthetic fixtures from later providers, and operational metrics from SLO claims.

The proof model nevertheless has four material holes: Gate A's verifier semantics are weakened (F-001); peer identity is absent from the transport probe (F-009); decoder/state-machine fuzz and mutation evidence are postponed despite current triggers (F-012); and current baseline/type-check coverage is overstated (F-013). F-003's activation-order proof and F-010's lease/GC races must also become explicit before M04/M03 respectively.

## Doctrine and Anti-Principle Assessment

The plan advances the doctrine's narrow authority, ports-and-adapters, functional-core/imperative-shell, deterministic artifacts, explicit provenance, and deliberate legacy cutover principles.

The open findings map to four anti-principles:

- implementation-owned invention weakens explicit versioned contracts and governed transformation boundaries (F-001);
- `READY` without a snapshot and a snapshot without its providers permit impossible state combinations (F-002/F-003);
- conflated `TableSpec` axes overload one field with different meanings rather than making invalid combinations unrepresentable (F-006);
- custom retry/security machinery is proposed before exhausting precise native library capabilities (F-007-F-009).

## Top Required Changes

1. Return every missing Gate-A value and deferred-mapping rule to its permanent owner; restamp the plan after acceptance.
2. Keep workspaces pre-`READY` until WP24 activates the first fully constructed snapshot.
3. Move exact-version provider/catalog construction and access profiles inside candidate-snapshot construction before activation.
4. Correct the gix SHA-256 feature/fixture contract.
5. Make M01 resolve and probe the actual stable dependency graph and exact generator identities.
6. Redesign `TableSpec` as three orthogonal axes and update WP09/WP21/WP23/WP25.
7. Replace the false delta-rs premise with application-transaction behavior probes and a narrowed custom recovery layer.
8. Close the secure-open, peer-credential, source-lease, and G-62 ownership/proof gaps.
9. Add bounded fuzz/mutation evidence and restamp a clean, coverage-proven execution baseline.
10. Split or serialize the oversized/shared-file packet graph before plan execution.

## Re-Audit Scope

A focused re-audit is sufficient if the architecture remains otherwise unchanged. It should verify:

- disposition and closure evidence for F-001 through F-016;
- accepted owner edits and new SHA-256 digests for every changed permanent design artifact;
- the revised lifecycle and snapshot/provider ordering by machine-checkable packet dependencies;
- exact resolved Cargo features for the stable graph, gix, rustix-equivalent, rusqlite, and delta-rs;
- behavior probes for delta application transactions, UDS peer identity, secure opens, and provider reuse;
- source-lease cleanup and G-62 staged-coverage ownership;
- new fuzz/mutation obligations and current `ci-fast`/Pyrefly coverage evidence;
- a conflict-free, resumable per-wave execution plan or an explicit justified alternative.
