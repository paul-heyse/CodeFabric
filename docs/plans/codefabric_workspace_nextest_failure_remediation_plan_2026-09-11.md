# CodeFabric workspace nextest failure remediation plan

Date: 2026-09-11. Reviewed implementation: `a26c00a79f88b97ea9c85f1000d5f25775197c98`
(`a26c00a7`, clean canonical `master` before the run).
Status: **planned; no production or test fixes implemented by this review**.

This is a qualification and repair supplement to the
[remaining outcomes 4–8 implementation plan](codefabric_pragmatic_production_outcomes_4_8_detailed_implementation_plan_2026-09-09.md),
particularly P04/P05 and its E05/E06/E10/E12/E21/E25/E26 boundaries. It does not replace that
backlog, reopen delivered P01–P03 boundaries wholesale, or declare any outcome complete.
The user requested the full workspace run, failure review and this new implementation plan.
The implementation sequence below is ready for a subsequent execution instruction.

## 1. Result and scope

The requested command completed with exit status **100**:

```sh
cargo nextest run --workspace --no-fail-fast
```

| Observation | Result |
|---|---|
| Selected tests | 1,235 across four binaries |
| Passed | 1,164 |
| Failed assertions/startup cases | 68 |
| Timed out | 3 |
| Explicitly ignored | 2 |
| Test execution duration | 134.287 seconds; compilation separately took 2 minutes 39 seconds |
| Nextest run ID | `b1bc0a36-e41a-48a0-9ce9-ccbf5f46c796` |
| Test profile | Existing `default`; no filter, retries or thread-count override |
| Host | Linux; 32 logical CPUs; approximately 188 GiB physical RAM |
| Build mode | Stable Rust 1.98.0, test profile, default `local-workstation` features, supervised sccache, `CARGO_INCREMENTAL=0` |
| Runner | cargo-nextest 0.9.143 |

The root is one Cargo package with an implicit one-member workspace. `--workspace` does not
include the separately managed `rustc-extractor/` and `pyrefly-sidecar/` test suites or the Python
adapter suite. Native provider binaries were supplied to the root integration tests. Nextest does
not run doctests. No ignored test was silently enabled, deleted or added to the skip list.

The two existing ignored cases are the source-image 10,000-attempt race campaign in
`src/source_image.rs:2423` and the adapter-wheel cross-domain oracle in
`tests/integration/rpc.rs:1395`. The **10,000-file complete-capture test was selected and timed out**;
it is a different test from the ignored race campaign.

### 1.1 Reproduction environment and evidence

The exact executed shell command supplied the already-built providers and the delegated cgroup
ancestry required by contained execution on this host:

```sh
env CARGO_INCREMENTAL=0 \
  DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus \
  XDG_RUNTIME_DIR=/run/user/1000 \
  CODEFABRIC_RUSTC_EXTRACTOR_BIN=/home/paul/CodeFabric/target/extractor/debug/codefabric-rustc-extractor \
  CODEFABRIC_PYREFLY_SIDECAR_BIN=/home/paul/CodeFabric/target/debug/codefabric-pyrefly-sidecar \
  systemd-run --user --scope --quiet --property=Delegate=yes \
  cargo nextest run --workspace --no-fail-fast
```

These environment values are host-specific reproduction information, not new repository pins.
The uv executable remains the system-installed CLI. No manifest, lock, source, test, toolchain,
watch profile, sysctl or timeout was changed for this run. No concurrent Cargo/nextest run was
present when it started; unrelated desktop/editor processes remained running.

Local evidence is under `target/nextest-review/2026-09-11-workspace/`:

- `full.log`: original stdout/stderr, every failure and final summary.
- `run-context.json`: revision, command, allowed environment values and workspace membership.
- `failures.json`: all 71 unsuccessful test identities, log locations and captured failure output.
- `resources-during-failures.json`: inotify limits, bounded process/descriptor observations and a
  failing direct `inotify_init1` probe.
- `sequential-diagnostics.log`: the five-case sequential diagnostic run in §1.2.

These local logs are supporting diagnostics and may be removed by later build cleanup. The
observations, named tests and remediation decisions required for implementation are retained here.
Do not add these logs as runtime dependencies or introduce a new evidence registry.

Review handoff checks: local-link/navigation checks for this document and STATUS pass (two files,
zero errors); focused spelling and whitespace checks pass. The §7 inventory was mechanically
compared with the original run: all 71 identities, result kinds, durations and log lines match,
with no duplicate rows. These checks validate the handoff, not product behavior.

### 1.2 Sequential diagnostic run

To test whether parallelism concealed other defects, five representative cases were rerun with
unchanged code, binaries, default profile and containment environment. The command added:

```sh
--test-threads 1 --success-output immediate --no-tests=fail \
-E 'test(rt_cpg_wp79_complete_ten_thousand_file_governed_capture) | test(wp47_fresh_production_workspace_prepares_exact_and_guarded_semantic_requests) | test(pragmatic_repeated_first_four_blocks_keep_independent_results_and_reopen) | test(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation) | test(explicit_poll_profile_publishes_nested_source_changes_and_reopens_exactly)'
```

Nextest run ID: `a21f25c0-f929-43aa-afb4-acc81ee10ff3`.
Result: **four passed, one failed**, 336.110 seconds, exit status 100. The 1,232 filtered/ignored
cases in this diagnostic invocation are not counted as acceptance.

| Diagnostic case | Full-run outcome | Sequential outcome and implication |
|---|---|---|
| Direct production backend exact/guarded preparation | 120.704 s timeout | Failed at 36.304 s with an admitted-snapshot/freshness mismatch; confirmed stale test setup, discussed in R04 |
| Complete 10,000-file governed capture | 120.047 s timeout | Passed in 77.404 s; retain its complete inventory, lease and cleanup assertions |
| Repeated first-four blocks/reopen | 120.205 s timeout | Passed in 101.742 s; retain composition/public/reopen assertions and qualify aggregate scheduling |
| Explicit polling/background source/reopen | 127.966 s startup failure | Passed in 84.040 s; the original startup failure is not proof of a polling semantic defect |
| Real supervisor durable fresh readiness | Watch creation failure | Passed in 36.614 s; its durable source-first activation assertions remain valid |

A sequential pass is a diagnosis, not a replacement for the required full command. The original
71 unsuccessful cases remain the baseline until the configured full suite passes after repairs.
The other 61 daemon cases that failed before readiness were not individually rerun in this review;
their semantic assertions remain unexercised by this full run.

The passing readiness case also emitted a provider warning: `structured-task scope is closed:
daemon/retained-pyrefly`. This fixture deliberately holds semantic publication and stops after
source-first assertions. The warning is not an assertion failure or proof of a leaked process;
R05 includes tracing this shutdown boundary before classifying it as expected cancellation or a
real join-order defect. The suite result alone does not establish successful background semantics.

## 2. Failure classification and confidence

| Group | Full-run cases | Observed failure | Disposition |
|---|---:|---|---|
| G1 | 50 | `source-watch-install: Too many open files (os error 24)`; supervisor exits before ready | Retain tests. Correct test scheduling and improve watcher capacity diagnostics; do not rewrite product expectations |
| G2 | 13 | Daemon hello or `DaemonControlRead` deadline during startup, about 128 s including teardown | Retain tests. Requalify with bounded fixture concurrency, then trace any remaining real startup delay |
| G3 | 3 | Nextest kills tests at their 120 s default bound | Two subsequently pass; the direct backend timeout hides the stale freshness setup in R04 |
| G4 | 1 | Provider relation count is 73, expected 72 | Update stale census assertions without reducing the current provider family surface |
| G5 | 3 | Synthetic MIR fixture constructors panic on `DataType::Binary` | Update fixtures to the real current native schema; preserve graph/identity assertions |
| G6 | 1 | Test expects output-row execution failure during `assembly.seal()` | Retire that obsolete pre-execution assertion; replace its composition coverage with an actual-read assertion |
| **Total** | **71** | **68 failures + 3 timeouts** | Every unsuccessful test is listed in §7 |

### 2.1 Watch capacity is distinct from ordinary descriptor exhaustion

During the failures, the host had `fs.inotify.max_user_instances=128`,
`max_user_watches=65536` and `max_queued_events=16384`. The caller and sampled daemon processes had
`RLIMIT_NOFILE=1048576`; sampled daemons held approximately 31–34 file descriptors. A direct
`inotify_init1` call returned errno 24 while the run was active and succeeded after it ended.
Linux documents both the per-user inotify-instance limit and the per-process descriptor limit as
possible `EMFILE` causes. The observations identify inotify-instance capacity as this run's
immediate watcher-construction constraint. See [inotify_init(2)](https://www.man7.org/linux/man-pages/man2/inotify_init.2.html).

The diagnostic scan observed 172 inotify **descriptor references** across accessible processes,
including desktop/editor processes. Inherited descriptors can refer to the same instance, and nine
processes were inaccessible. That count is not a count of distinct kernel instances and must not
be compared directly to 128 as if it were exact allocation accounting.

The source path is concrete: `WorkspaceObservation::watch` in
`src/fabric/workspace_updates.rs:340` calls `new_debouncer_opt`; resolved notify 8.2.0
`INotifyWatcher::from_event_handler` calls `Inotify::init` at `src/inotify.rs:539`.
The current nextest file contains only a commented test-group example. Its live configuration does
not limit concurrent native workspace fixtures. Lowering the daemon's 16-worker/large-memory
operating profile would not address the shared per-user instance limit.

No OOM, full disk or native Delta executor panic was observed in this run. Sampled available memory
remained about 140 GiB and sampled free disk reached 329 GiB; these are samples, not continuous peak
measurements. The earlier native cleanup cascade recorded in the parent plan remains unresolved
and was not reproduced here.

### 2.2 Readiness failures are not 63 separate semantic defects

All G1 and G2 cases fail through `RunningSupervisor::wait_ready` at
`tests/integration/daemon.rs:403`, before their public query/fact assertions. G2 receives the
supervisor's 120-second daemon hello/read deadline (`src/supervisor.rs:52,1821`), rather than the
fixture's 180-second `PROCESS_DEADLINE`. G3 uses a third deadline: nextest's default 120 seconds.
Changing just one of those bounds does not repair the others.

The sequential poll case passes without code changes, and two G3 cases pass sequentially. This
supports aggregate scheduling/I/O contention as a contributor, but does not prove that all G2 cases
share only that cause. R01 makes the aggregate executable; R05 requires a new full run and concrete
phase evidence for any survivors. None of these tests is obsolete because it has an old `wp` name.

## 3. Design and library constraints for the fixes

The parent plan §3.3, §6A–6D, §8A/8C–8F and §9.1 remain the delivery authority. The
[parent production backlog](codefabric_pragmatic_production_implementation_plan.md), Outcome 2,
explicitly removes generalized proof execution and mandatory double execution while preserving
schema, source/context, authorization and publication checks.

| Boundary | Required design and concrete library use |
|---|---|
| Test concurrency | Use nextest's process-wide test groups and per-test weights. An in-process mutex cannot coordinate nextest's separate test processes. Keep ordinary unit tests parallel. [Native nextest test-group documentation](https://nexte.st/docs/configuration/test-groups/) |
| Watch ownership | Preserve notify's pruned registrations, selected Git/common-dir observation, retained rescan obligation and explicit native/poll choice. Keep construction, registration and asynchronous backend failures distinguishable. Use `Debouncer::stop` under the owned blocking lifecycle; do not substitute non-joining Drop. [notify reference](../library_ref/notify_debouncer_full_rust_reference.md), §8, Watch roots and lifecycle; §12, Error model; §30, Shutdown |
| Source identity | Binary raw compiler paths coexist with display paths and exact captured file/digest/byte coordinates. Populate typed native Arrow fields; do not cast raw Unix paths to lossy text. [Rust MIR reference](../library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md), §17, Source spans, files, macro expansion, and source anchoring; current `src/rustc_relation_schema.rs` is the executable contract |
| DataFusion execution | Install typed plans and validate schemas/properties without collecting their output. Enforce row/memory/spill bounds on actual streams, with one fresh execution state per physical read and shared counters across partitions. Preserve resource contracts through authorized view reconstruction. [DataFusion/Arrow reference](../library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md), §47, Planner metadata; resolved behavior is in `programmatic_schema.rs` |
| Freshness and identity | `PreparedSemanticExecution` already owns the admitted epoch and projected snapshot. Derive execution freshness from that snapshot, preserve target barriers and source/semantic distinctions, and keep old readers on their exact lease. [Lifecycle specification](../authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md), §9, Admission, freshness, and epoch pinning |
| Delta persistence | Preserve exact selected versions, the logical/storage type restoration boundary, command-owned sessions, zero hidden retries and uncertain-write reconciliation. No latest-version substitution, eager proof pass or raw-Parquet production shortcut. [Data-fabric specification](../authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md), §9, Durable Delta relations; [Delta reference](../library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md), §3, snapshots/time travel and §7.2, session/runtime integration |
| Lifetimes and failures | Child cancellation and dropping a future do not establish that native work has joined. Keep service/control headroom, awaited workspace shutdown and owner-level cleanup. [Rust daemon reference](../library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md), §27.2, Select pattern and §37, Graceful shutdown |

The installed native baseline remains Arrow 59.2.0/DataFusion 55.0.0, delta-rs `43a0cf10`,
notify 8.2.0/debouncer 0.7.0 and Tokio 1.53.1. No library upgrade, new crate, Python data-plane
component, universal allocator admission layer, proof registry or unrelated protocol redesign is
needed to resolve the demonstrated failures.

## 4. Dependency-ordered implementation packages

Execute R01, R02, R03, R04, R05 and R06 in the canonical working tree. R02/R03 do not depend on a
new scheduler design; keep each correction small and attributable. These packages repair the
current test/implementation boundary before broader P04 execution resumes.

### R01. Make the full native suite fit its actual shared host resources

**Addresses:** G1/G2 and contention-sensitive G3. **Parent scope:** P04/E10/E12/E21/E26.
**Surfaces:** `.config/nextest.toml`, the existing environment/doctor entry points where useful,
`tests/integration/daemon.rs`, and narrowly `src/fabric/workspace_updates.rs` for error context.

1. Add a named nextest group for tests that launch real workspaces or exercise native watchers.
   Start qualification at two simultaneous expensive workspace fixtures. Include
   `integration::daemon::`, the direct production-startup unit test and native watch tests; audit
   actual daemon/watch constructors so a heavy consumer cannot evade the group through a different
   module name. Keep pure library, wire and graph tests outside it.
2. Treat the 10,000-file capture as heavy durable I/O. Give it the group's full two-slot weight so
   it does not overlap full daemon fixture startup. Use the same mechanism for a case that owns
   simultaneous independent clean/live daemons when measurement warrants exclusivity. The group
   limits test fixtures; it does not reduce production CPG capability or native worker defaults.
3. Use `cargo nextest show-config test-groups` and `cargo nextest list` to inspect the effective
   default/CI selections. Override precedence is per setting: an existing timeout override should
   not accidentally exclude a test from the group. Avoid a broad first-match timeout rule that
   masks the existing longer multi-generation deadlines.
4. Preserve the explicit polling tests and native-watch tests. Do not silently switch all test
   configurations to polling, ignore environment failures, or retry them until they disappear.
5. Add focused capacity diagnostics to the existing startup/error path: watcher construction versus
   registration, selected backend, original OS error, and relevant inotify/descriptor limits when
   available. Keep raw error categories until presentation instead of flattening them at each layer.
   Sampling inability must not become a second startup failure. Do not create a new public wire
   schema solely for this diagnosis or print environment secrets.
6. Do not change workstation sysctls or close unrelated editor watchers as part of a test repair.
   If a host has insufficient capacity even for the qualified group, report the specific
   environment prerequisite. A deliberate future host-capacity change is separate from repository
   correctness; it must not be an undocumented requirement of the raw command.
7. Review failed-fixture cleanup: `RunningSupervisor::Drop` currently allows five seconds before
   killing the supervisor, whereas accepted production drain has its own budget. Use the ordinary
   stop/join path when possible, preserve the original failure, and verify owned descendants and
   endpoints have gone. Bound forced teardown; do not hide leaked work with broad process killing.

**Exit:** all selected tests still run through the original full command; no G1 failures under the
recorded usable host conditions; group membership includes all actual scarce-resource consumers;
no new ignored tests or retries; owned fixture cleanup restores its resources. Use small concurrent
samples to justify any later increase above two. Per-user descriptor references are diagnostic
hints, not a universal allocation accounting contract.

### R02. Reconcile the current provider census and typed MIR fixtures

**Addresses:** G4/G5. **Parent scope:** P02/P06/E06/E07/E14/E15/E25.
**Surfaces:** `src/production_provider_recipe.rs:2629`,
`src/programmatic_derived_analysis.rs:9444–9600`, `src/rustc_relation_schema.rs`,
`src/pyrefly_relation_schema.rs` and the existing provider-census tests.

1. The current release has 73 provider relations. `PyreflyRelation::MemberObservation` was added
   with real native/public consumers in `9a5d8e0b`; the assertion still expects 72. Retain the
   five-lane and eight-form boundaries, and verify both relation and identity-transformation census.
   `compile_current_v23_release` creates one identity transformation per admitted relation, so the
   next hard-coded `transformations == 72` assertion is stale too, although the first assertion
   prevents it from running in this baseline.
2. Replace magic aggregate totals with meaningful census correspondence against the closed current
   native relation sets, retaining an explicit assertion for the newly admitted member relation and
   its exact typed schema. Reuse
   `wp34_int_compiled_relation_census_and_schema_contracts_are_exhaustive`, which already passes.
   Do not add an expected list generated only from the same result under test or declare eight-form
   production completion merely because ingress has eight forms.
3. Extend `rust_control_batch` and `rust_structural_batch` with deliberate `BinaryArray` construction
   for the current `span_file_bytes` fields. Use coherent raw byte paths alongside their fixture
   display/file identity; preserve null for explicitly absent/unmappable source as allowed by that
   field. Keep unexpected types/fields detectable instead of filling every new field with arbitrary
   defaults. Inspect the full selected schemas so fixing the first panic does not conceal a second
   type/nullability mismatch.
4. Preserve independent controller edges 8/9/99, optional-predicate null behavior, unwind edges,
   exact changed-input IDs and ownership/alias/resource/async/unsafe structural rows. Those are
   current semantic checks; they must survive fixture modernization. No production Binary field
   should be removed or recast to Utf8 to accommodate a stale fixture.
5. Run the four failures plus the existing provider-schema/census and related control/structural
   tests. Inspect any downstream assertion newly reached after the fixture panic. A synthetic
   structural pass does not close real MIR or private-borrow integration in P06/P08.

**Exit:** G4/G5 pass against the current native schema with meaningful semantic assertions intact;
no producer scope reduction, provider-local identity promotion or lossy raw-path conversion.

### R03. Replace the obsolete sealing-time execution expectation

**Addresses:** G6. **Parent scope:** delivered Outcome 2 and P04/P06/E25 resource ownership.
**Surfaces:** `src/programmatic_derived_analysis.rs:11277`,
`src/fabric/programmatic_schema.rs:1729,2184,2828,5530`.

The failing test is named `execution_bound_aborts_seal_without_returning_partial_epoch`. The
current implementation intentionally seals plan-backed views without executing transformations.
`validate_transformation_physical_contract` plans/validates properties; the actual stream invokes
`validate_streamed_transformation_batch` and emits `TransformationOutputRowsExceeded` at read time.
The passing `actual_transformation_reads_enforce_row_and_memory_bounds_without_preexecution`
already checks that contract, including reconstructed views and fresh counters on repeated reads.

1. Remove the requirement that `assembly.seal()` evaluate row limits. Do not restore pre-execution,
   duplicate collection, mandatory result checksums or generalized proof histories.
2. Preserve a meaningful derived-composition regression: compose with the small output budget,
   seal successfully, then execute the relevant derived output through the installed provider and
   assert its actual streamed failure. Ensure the error is the intended row-bound failure and
   not an unrelated schema/admission error. Choose known independently specified nonempty fixture
   rows so the bound is causally exercised.
3. Keep missing/duplicate relation, dependency, schema and policy errors at installation time.
   Keep the distinction between streaming a permitted prefix and publishing a successful result:
   an eventual bound failure must terminate the query and must not publish a successful partial
   package. Reuse existing result-publication failure coverage for that consumer.
4. Retire the old test name/assertion after its current replacement is exercised. If the existing
   schema-bound test already covers every useful obligation, consolidate overlapping low-level
   checks there, while retaining a composition-level check that resource contracts are forwarded.
   Remove obsolete imports and search exact test-name selectors/recipes before deleting them.

**Exit:** sealing is side-effect free with respect to transformation execution; actual reads still
fail at the configured boundary; repeated physical reads receive independent counters; no success
publication is fabricated after execution failure.

### R04. Make direct-backend tests use admitted snapshot freshness

**Addresses:** the hidden failure behind one G3 timeout. **Parent scope:** P03/P04/P11/E09/E18/E21.
**Surfaces:** `src/daemon.rs:1976–2230`, `src/query_backend.rs:132,320`,
`src/fabric/programmatic_query_backend.rs:981,1108`, `src/query_service.rs:2805`.

The direct test requests `best_available_snapshot`, directly admits an epoch, then calls `execute`
with hard-coded `FreshnessState::Current`. Admission projects actual observation freshness into
`PreparedSemanticExecution::snapshot`; execution rejects a different caller value at
`programmatic_query_backend.rs:1142`. The real query service passes the admitted snapshot value.
This is a stale test caller, not evidence that the production equality check should be removed.

1. Preserve exact/guarded preparation against the source-first catalog and assert it does not
   create a separate execution or invent semantic completeness.
2. For the positive function-row assertion, request `require_semantic_current` and use the existing
   `admit_fresh_execution_request` path under a finite deadline. Avoid a sleep or a query that merely
   happens to overlap completion. A source-only best-available test should assert pending/unknown
   scope rather than positive semantic rows or complete absence.
3. Pass the freshness carried by the admitted snapshot through execution and retain exact
   workspace/epoch/source authorization checks. Verify the test waits on the required semantic
   boundary before its `total_rows > 0` assertion, which was not reached by this diagnostic run.
4. Simplify the internal execution interface in the same bounded change: derive freshness inside
   the backend from the owned `PreparedSemanticExecution` instead of accepting a redundant separate
   freshness argument. Update the real service and test backend implementations together. Retain
   workspace routing, epoch capability equality, nonempty snapshot identity and admitted-snapshot
   validation; do not replace these with a mutable lookup of the latest workspace.
5. Exercise admitted old-snapshot execution during successor publication, source-first pending
   selection and semantic-current positive selection using existing query/lifecycle fixtures.
   Ensure explicit shutdown is reached on success and cleanup remains owned on failure.

**Exit:** direct and service-mediated execution share one immutable freshness source; the corrected
case passes, and known pending/stale semantics cannot be relabeled current. This does not implement
all target/family freshness or historical query selectors in the parent plan.

### R05. Resolve remaining startup and timeout failures without masking work

**Addresses:** remaining G2/G3 and any latent failures exposed after R01–R04.
**Parent scope:** P04/P05/E12/E21/E26, with measured persistence improvements in P12/P14.
**Surfaces:** `.config/nextest.toml`, `tests/integration/daemon.rs`, relevant cases below it,
`src/supervisor.rs`, `src/fabric/production_workspace_startup.rs`, source capture/publication owners
and existing preparation reports.

1. Run all original failed/timed-out identities after R01–R04, then the full command. Do not treat
   the representative sequential passes as proof for the other cases. Preserve all independently
   expected language facts, query fields, source disclosure, clean/live comparisons and restart
   behavior while reaching previously blocked assertions.
2. For a remaining startup failure, retain compact stage information and durations for source census,
   capture, parser/provider work, relational planning/writes, watch installation and ready handshake.
   The existing source/semantic preparation reports and structured operation owners are the starting
   point. A timeout error should identify the last stage and whether owned work is advancing.
3. Distinguish the 120-second supervisor startup deadline, 180-second fixture process wait, per-query
   freshness deadline and per-test nextest bound. Set finite test envelopes from the actual sequence
   of startup/edit/query/reopen operations and observed admitted concurrency; account for teardown.
   The repeated-block case's isolated 101.742 seconds leaves little room under 120 seconds, so
   qualify a focused longer outer bound if the unchanged legitimate sequence requires it. Do not
   change the production ready deadline merely because a test was launched in a saturated group.
4. If startup still exceeds its product bound under admitted load, fix the measured blocking path.
   Reuse retained source/parser/provider inputs and exact versions; preserve bounded Delta write
   concurrency and source-first readiness. Do not repeat semantic extraction before source readiness
   or skip family tables to reduce elapsed time. Native CPU allocation remains shared-scheduler
   work in P04, not one unconstrained pool per new helper.
5. Preserve the full 10,000-file capture test. Its isolated 77.404-second result proves the current
   complete capture/lease/release checks can finish; it is not an obsolete allocator receipt test.
   First prevent heavy startup overlap. If measured capture cost remains a bottleneck, inspect
   per-file durable registration/lease transaction work in `source_image.rs` and `inventory_capture.rs`
   before batching; preserve failure rollback, capture fences and lease ownership. Do not reduce
   10,000 to a small fixture or revive universal allocation accounting to obtain a pass.
6. If the earlier native Delta executor cancellation/cleanup cascade reappears, trace runtime and
   pending-operation ownership before changing any library. The resolved kernel executor uses a
   spawned future plus synchronous receive; dropping its Tokio runtime while native work waits can
   panic. This is a known unresolved risk, not a reproduced cause of this run's G1/G2 failures.
   Follow the existing joined scope and exact uncertain-write recovery; keep any native patch narrow
   and qualify interruption/reopen behavior. Do not catch and suppress the panic as a test fix.
7. Trace the observed readiness-case retained-Pyrefly warning through the closed task scope and
   process join. Preserve cancellation as cancellation when shutdown is expected; if joining work
   requires an already-closed spawning scope, repair the owner/order and retain cleanup on failure.
   Do not merely suppress warnings or count source-ready as completed semantics.
8. Retain separate native and explicit poll scenarios. `Debouncer::stop` joins the debounce worker;
   do not infer complete joining of every backend thread solely from that call. Full poll-backend
   lifecycle qualification remains a parent P13 obligation unless this repair touches it.

**Exit:** every originally unsuccessful scenario reaches its intended product assertions and passes
under the qualified full-run configuration; startup/timeout survivors have an explained and tested
fix, not a blanket timeout multiplier or a success-by-retry result.

### R06. Close the full workspace baseline and hand off remaining product scope

**Depends on:** R01–R05. **Surfaces:** the changed implementation/tests/configuration, this plan,
STATUS and the parent plan's validation/handoff notes.

1. Run relevant Rust checks/affected Clippy and focused tests as each boundary changes. Do not
   rebuild unaffected native domains solely to modify a fixture, but rebuild any changed provider
   executable before consumer tests. Preserve the stable/nightly/sidecar/adapter separation.
2. Run `cargo nextest run --workspace --no-fail-fast` with the qualified checked-in default profile
   and recorded provider/containment environment. No filtering, new ignored cases, retry-based
   green, reduced corpus or temporary configuration is the terminal result. Require zero failures
   and zero timeouts. Keep the two existing intentional ignores explicit and verify no useful
   selected test disappeared except the documented obsolete assertion replaced in R03.
3. Reconcile test identities before/after; renaming or consolidating one obsolete assertion can
   change the total legitimately. Map each old failed identity to its corrected/replacement case.
   A falling test count without an explained replacement is not acceptance.
4. For claims beyond this command, separately run `just root-doctest`, changed provider-domain tests
   and adapter/protocol checks when their inputs change. The separate ignored stress/cross-domain
   scenarios retain their existing command routes. Do not call a nextest pass a four-domain or
   full-outcomes certification.
5. Update STATUS and the parent plan with command, revision/date, configuration, counts, remaining
   failures and implementation limits. Mark this repair complete only after the full baseline is
   actually green. Then return to the unmet P04 sequence, followed by P05–P14.

**Exit:** a reproducible green selected workspace suite with useful assertions preserved, no hidden
resource prerequisites, and an honest remaining product backlog.

## 5. Explicit keep, update and retirement decisions

| Tests/behavior | Decision | Reason |
|---|---|---|
| Fifty watcher-construction failures | Keep all product assertions; repair execution conditions and diagnostics | They failed before the behavior they are meant to protect |
| Thirteen ready-handshake failures | Keep; diagnose after resource admission is fixed | Native/poll, language and public-query cases remain selected target behavior |
| Repeated-block and 10,000-file timeout cases | Keep; group/qualify finite deadlines | Both passed unchanged sequentially |
| Direct-backend fresh workspace case | Modernize freshness/admission and retain positive/guarded semantics | Its hard-coded execution freshness conflicts with source-first observation |
| Provider census test | Update aggregate assumptions and preserve exact family/schema checks | Real member observations increased the native surface |
| Three MIR fixture failures | Modernize typed inputs, retain expected edges/state/identity behavior | The fixture cannot currently construct the production schema |
| Sealing-time row-bound assertion | Retire that obsolete assertion/name; preserve actual-read/composition coverage | Eager execution during sealing was deliberately removed |
| Other old `wp`, `proof`, `governed` names | No blanket deletion or rename project | Names alone do not establish obsolete behavior; many still protect current semantics/ownership |
| Existing two ignored cases | Preserve explicit separate scope; do not add more ignores | Not failures in the requested default command |

Before deleting a legacy test or helper, inspect its consumers with repository-wide bounded `rg`,
including `justfile`, tooling selectors and docs. Retain schema/authorization/unknown/epoch/cleanup
obligations even if their original wrapper or assertion name is retired. Do not restore deleted
proof machinery to satisfy a historical test, and do not delete a semantic test because it is costly.

## 6. Remaining uncertainty and implementation risks

- The review ran all selected tests but only five representative sequential diagnostics. More
  semantic or lifecycle defects may appear after startup capacity and fixture errors are repaired.
  R05 is an explicit dependency of full closure, not permission to waive newly exposed failures.
- Host inotify headroom varies with unrelated applications. A static two-fixture group is an initial
  measured qualification choice, not a guarantee for every host state. Report unavailable host
  capacity and keep profile/resource limits observable.
- A source-only epoch can be a lawful ready state. Test changes must preserve pending semantics,
  not turn source readiness into a claim that checker/compiler work is complete.
- Changing the internal execution freshness interface affects real service and test backends.
  Inspect all callers and preserve accepted snapshot identity, epoch leases and cancellation.
- Full-run elapsed time will increase when formerly failing startups actually execute long edit/
  clean/reopen sequences. That cost is real qualification work; choose useful concurrency from
  observed CPU/I/O/watch headroom and do not silently drop scenarios to fit the old failed duration.
- P04 retained Cargo target/fact reuse, full dependency/tool validity, all-owner retention, native
  maintenance, remaining language analyses and all eight complete query forms stay open under
  the parent plan. This repair does not claim those outcomes.

## 7. Complete unsuccessful-test inventory

The following inventory is generated from the original run's first result record for each test,
not from the repeated final failure summary. Group labels refer to §2; the sequential diagnostics
change the interpretation of G3 but do not erase its full-run result. The tables retain exact
qualified names for nextest selectors; durations and log lines refer to `full.log`. Each group has
one shared causal diagnosis and repair owner above, rather than 71 unrelated speculative fixes.

### G1. Watcher construction capacity (50 cases)

Repair owner: **R01, then R05**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric::integration` | `integration::daemon::block_queries::pragmatic_prior_entity_results_fan_out_and_union_through_installed_clients_and_reopen` | FAIL, 11.702s | 2150 |
| `codefabric::integration` | `integration::daemon::branch_queries::pragmatic_failed_query_branches_preserve_independent_results_and_exact_reopen` | FAIL, 11.805s | 2170 |
| `codefabric::integration` | `integration::daemon::literal_queries::pragmatic_literal_identifiers_and_property_filters_survive_public_reopen` | FAIL, 11.264s | 2106 |
| `codefabric::integration` | `integration::daemon::live_updates::captured_python_site_packages_survive_public_queries_and_reopen` | FAIL, 11.135s | 2085 |
| `codefabric::integration` | `integration::daemon::live_updates::custom_cargo_build_input_changes_context_and_matches_clean_public_results` | FAIL, 10.730s | 2022 |
| `codefabric::integration` | `integration::daemon::live_updates::live_mixed_decoded_sources_equal_original_bytes_and_independent_clean_queries` | FAIL, 10.475s | 2064 |
| `codefabric::integration` | `integration::daemon::live_updates::live_mixed_function_definitions_and_bodies_equal_exact_clean_source` | FAIL, 11.018s | 2127 |
| `codefabric::integration` | `integration::daemon::live_updates::mixed_semantic_publication_shutdown_joins_workspace_owners` | FAIL, 4.226s | 2043 |
| `codefabric::integration` | `integration::daemon::live_updates::processing_remainder_pages_keep_exact_scope_across_reopen_and_updates` | FAIL, 3.800s | 2002 |
| `codefabric::integration` | `integration::daemon::live_updates::provider_deployment::selected_provider_redeployment_fences_delayed_facts_and_reconciles_reopen` | FAIL, 3.347s | 2191 |
| `codefabric::integration` | `integration::daemon::live_updates::shutdown_during_semantic_delta_publication_joins_workspace_owners` | FAIL, 3.292s | 2233 |
| `codefabric::integration` | `integration::daemon::live_updates::source_current_publication_fences_delayed_semantics_and_resumes_after_restart` | FAIL, 3.559s | 2296 |
| `codefabric::integration` | `integration::daemon::live_updates::source_line_windows_and_hard_limits_survive_public_delivery_and_reopen` | FAIL, 6.103s | 2446 |
| `codefabric::integration` | `integration::daemon::live_updates::timed_out_client_during_publication_drains_and_reopens_exactly` | FAIL, 6.250s | 2488 |
| `codefabric::integration` | `integration::daemon::location_queries::pragmatic_source_location_facts_do_not_require_source_disclosure` | FAIL, 6.029s | 2467 |
| `codefabric::integration` | `integration::daemon::location_queries::pragmatic_source_locations_resolve_captured_bytes_lines_and_zero_width_nodes` | FAIL, 6.312s | 2551 |
| `codefabric::integration` | `integration::daemon::ordering_queries::pragmatic_semantic_ordering_precedes_limits_and_survives_prior_reuse_and_reopen` | FAIL, 6.178s | 2572 |
| `codefabric::integration` | `integration::daemon::pragmatic_all_rust_targets_failed_retains_diagnostics_and_source` | FAIL, 0.721s | 2212 |
| `codefabric::integration` | `integration::daemon::pragmatic_live_python_edits_converge_without_restart` | FAIL, 5.742s | 2509 |
| `codefabric::integration` | `integration::daemon::pragmatic_public_source_context_is_exact_and_separately_authorized` | FAIL, 5.649s | 2530 |
| `codefabric::integration` | `integration::daemon::pragmatic_python_chunked_inventory_publishes_cross_module_semantics` | FAIL, 0.422s | 2254 |
| `codefabric::integration` | `integration::daemon::pragmatic_python_implicit_calls_qualify_call_coverage` | FAIL, 0.502s | 2275 |
| `codefabric::integration` | `integration::daemon::pragmatic_python_public_declaration_kinds` | FAIL, 5.388s | 2595 |
| `codefabric::integration` | `integration::daemon::pragmatic_python_public_lexical_references` | FAIL, 5.699s | 2657 |
| `codefabric::integration` | `integration::daemon::pragmatic_python_semantics_publish_real_call_targets` | FAIL, 5.531s | 2615 |
| `codefabric::integration` | `integration::daemon::pragmatic_rust_semantics_publish_captured_path_dependency` | FAIL, 0.456s | 2317 |
| `codefabric::integration` | `integration::daemon::pragmatic_rust_semantics_publish_locked_directory_dependency_and_reopen` | FAIL, 0.462s | 2338 |
| `codefabric::integration` | `integration::daemon::pragmatic_rust_semantics_publish_real_call_targets` | FAIL, 0.478s | 2359 |
| `codefabric::integration` | `integration::daemon::pragmatic_rust_target_failure_retains_other_targets` | FAIL, 5.055s | 2636 |
| `codefabric::integration` | `integration::daemon::pragmatic_rust_virtual_workspace_inherits_package_settings` | FAIL, 0.472s | 2380 |
| `codefabric::integration` | `integration::daemon::relationship_queries::distance::bounded_call_walks_preserve_witnesses_filters_cycles_priors_and_exact_reopen` | FAIL, 5.523s | 2743 |
| `codefabric::integration` | `integration::daemon::relationship_queries::pragmatic_semantic_relationships_through_installed_clients_and_reopen` | FAIL, 5.243s | 2877 |
| `codefabric::integration` | `integration::daemon::semantic_references::pragmatic_python_canonical_modules_imports_references_and_reopen` | FAIL, 2.171s | 2405 |
| `codefabric::integration` | `integration::daemon::semantic_references::pragmatic_rust_canonical_imports_references_and_reopen` | FAIL, 2.003s | 2425 |
| `codefabric::integration` | `integration::daemon::source_boundary_queries::pragmatic_source_boundaries_narrow_first_four_forms_and_survive_reopen` | FAIL, 4.556s | 2925 |
| `codefabric::integration` | `integration::daemon::syntax_queries::pragmatic_syntax_nodes_properties_parents_and_source_survive_public_reopen` | FAIL, 4.228s | 2947 |
| `codefabric::integration` | `integration::daemon::types::pragmatic_python_canonical_structural_types_and_reopen` | FAIL, 1.101s | 2678 |
| `codefabric::integration` | `integration::daemon::types::pragmatic_rust_canonical_structural_types_and_reopen` | FAIL, 0.951s | 2699 |
| `codefabric::integration` | `integration::daemon::wp44_beh_real_project_venv_launch_preserves_distribution_authority` | FAIL, 0.927s | 2720 |
| `codefabric::integration` | `integration::daemon::wp44_beh_real_supervisor_ready_requires_durable_fresh_activation` | FAIL, 1.033s | 2765 |
| `codefabric::integration` | `integration::daemon::wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback` | FAIL, 1.085s | 2828 |
| `codefabric::integration` | `integration::daemon::wp44_ops_real_launch_capacity_recovers_pending_failure_and_abrupt_exit` | FAIL, 1.026s | 2787 |
| `codefabric::integration` | `integration::daemon::wp44_ops_real_signal_orders_drain_before_shutdown_and_joins_owned_endpoints` | FAIL, 1.012s | 2807 |
| `codefabric::integration` | `integration::daemon::wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints` | FAIL, 0.902s | 2853 |
| `codefabric::integration` | `integration::daemon::wp47_beh_real_installed_wheel_guard_query_resource_and_completion` | FAIL, 3.999s | 3030 |
| `codefabric::integration` | `integration::daemon::wp47_int_real_installed_wheel_modern_contract_observation` | FAIL, 3.737s | 2967 |
| `codefabric::integration` | `integration::daemon::wp47_neg_real_agent_scope_legacy_framing_and_secret_denial` | FAIL, 3.743s | 3009 |
| `codefabric::integration` | `integration::daemon::wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation` | FAIL, 3.701s | 3051 |
| `codefabric::integration` | `integration::daemon::wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent` | FAIL, 3.404s | 2988 |
| `codefabric::integration` | `integration::daemon::wp63_ops_installed_restart_reconstructs_only_exact_activation_authority` | FAIL, 3.570s | 3072 |

### G2. Startup readiness deadlines (13 cases)

Repair owner: **R01, then R05**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric::integration` | `integration::daemon::fact_families::members::pragmatic_native_members_preserve_ownership_empty_scopes_types_and_reopen` | FAIL, 128.060s | 3150 |
| `codefabric::integration` | `integration::daemon::fact_families::pragmatic_canonical_fact_families_through_installed_clients_and_reopen` | FAIL, 128.681s | 3275 |
| `codefabric::integration` | `integration::daemon::first_release::pragmatic_plan_corpus_first_four_forms_and_independent_expectations_survive_reopen` | FAIL, 128.265s | 3191 |
| `codefabric::integration` | `integration::daemon::live_updates::cargo_configured_platforms_and_flags_converge_with_clean_public_queries` | FAIL, 128.301s | 3212 |
| `codefabric::integration` | `integration::daemon::live_updates::cargo_feature_and_profile_selections_keep_partial_contexts_and_equal_clean_queries` | FAIL, 128.344s | 3254 |
| `codefabric::integration` | `integration::daemon::live_updates::cargo_library_linkage_kinds_survive_live_queries_and_clean_reopen` | FAIL, 128.139s | 3170 |
| `codefabric::integration` | `integration::daemon::live_updates::explicit_poll_profile_publishes_nested_source_changes_and_reopens_exactly` | FAIL, 127.966s | 3233 |
| `codefabric::integration` | `integration::daemon::live_updates::live_python_context_and_negative_imports_equal_independent_clean_queries` | FAIL, 128.718s | 3296 |
| `codefabric::integration` | `integration::daemon::live_updates::live_python_namespace_stub_precedence_equals_independent_clean_queries` | FAIL, 129.006s | 3317 |
| `codefabric::integration` | `integration::daemon::live_updates::live_python_raw_paths_and_root_initializer_keep_exact_source_identity` | FAIL, 128.960s | 3359 |
| `codefabric::integration` | `integration::daemon::live_updates::live_python_search_paths_preserve_all_sources_and_equal_independent_clean_queries` | FAIL, 128.716s | 3380 |
| `codefabric::integration` | `integration::daemon::live_updates::mixed_live_updates_equal_independent_clean_public_queries` | FAIL, 128.485s | 3338 |
| `codefabric::integration` | `integration::daemon::live_updates::mixed_raw_path_inventory_keeps_rust_calls_across_updates_and_clean_reopen` | FAIL, 127.963s | 3401 |

### G3. Outer test timeouts (3 cases)

Repair owner: **R01/R04/R05**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric` | `daemon::tests::wp47_fresh_production_workspace_prepares_exact_and_guarded_semantic_requests` | TIMEOUT, 120.704s | 3111 |
| `codefabric` | `source_image::inventory_capture::tests::rt_cpg_wp79_complete_ten_thousand_file_governed_capture` | TIMEOUT, 120.047s | 3120 |
| `codefabric::integration` | `integration::daemon::block_queries::pragmatic_repeated_first_four_blocks_keep_independent_results_and_reopen` | TIMEOUT, 120.205s | 3135 |

### G4. Provider census (1 case)

Repair owner: **R02**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric` | `production_provider_recipe::tests::current_v23_release_compiles_all_exact_provider_relation_schemas` | FAIL, 0.020s | 1469 |

### G5. Native Binary fixtures (3 cases)

Repair owner: **R02**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric` | `programmatic_derived_analysis::tests::rust_control_input_executes_native_joins_and_preserves_controller_semantics` | FAIL, 0.013s | 1504 |
| `codefabric` | `programmatic_derived_analysis::tests::rust_control_input_faults_are_explicit_and_causal` | FAIL, 0.014s | 1522 |
| `codefabric` | `programmatic_derived_analysis::tests::rust_structural_producers_execute_native_plans_and_change_with_exact_inputs` | FAIL, 0.014s | 1541 |

### G6. Obsolete seal-time execution assertion (1 case)

Repair owner: **R03**.

| Binary | Exact test identity | Baseline result | Log line |
|---|---|---|---:|
| `codefabric` | `programmatic_derived_analysis::tests::execution_bound_aborts_seal_without_returning_partial_epoch` | FAIL, 0.226s | 1568 |
