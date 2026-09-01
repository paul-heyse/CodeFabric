---
artifact: interface-design-review
date: 2026-08-31
version: v3
status: complete
supersedes: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-08-30_v2.md
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
serving_specification: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
lifecycle_specification: docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md
fabric_specification: docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md
reviewed_head: f12329f05e3678698ff9a43ec4f69f95f42db12f
working_tree: dirty-pre-existing-and-in-progress
baseline: intentionally-not-taken
verdict: revision-required
target_status: accepted
---

# Production daemon composition through gRPC and FastMCP: end-to-end design review

## 0. Executive decision

The relational core is not blocked by Arrow, DataFusion, or delta-rs. It is blocked by a missing
application-owned composition and lifecycle authority.

The current tree contains substantial, valuable programmatic foundations: exact provider-batch
admission, native DataFusion logical plans, immutable epoch construction, exact Delta version
selection, a fenced command actor, activation append/readback, authorized child sessions, and one
published-result registry. Those foundations should be retained. The production binary cannot use
them: `codefabricd` calls `daemon::serve`, and that function returns
`ProgrammaticCompositionRequired`; the real `serve_programmatic` path has no production caller.

Trying to repair only that call would preserve two circular dependencies:

1. workspace construction requires an existing selected activation before it creates the command
   actor that is supposed to own activation, so a lawful first epoch cannot be created; and
2. the daemon starts its query transport, reports workspaces as ready, and then tries to recover
   command/cutover authority, while handshake and status require a query snapshot that must not be
   available until that recovery is complete.

The preferred target is a **phase-typed production startup coordinator around one staged daemon
kernel**:

```text
exact operational inputs + compiled CodeFabric release
  -> acquire daemon/writer authority
  -> recover the command journal with semantic admission closed
  -> genesis if the activation head is empty, otherwise reopen the exact selected Delta vector
  -> build and prove the programmatic epoch and its query authority
  -> atomically install one ActiveWorkspace
  -> safely bind UDS endpoints and expose liveness + honest bootstrapping status
  -> observe and commit sole target authority for this fresh deployment
  -> open query admission and publish Ready
  -> DataFusion stream -> bounded Arrow IPC pages -> one sealed result package
  -> Tonic control/resource service -> one grpc.aio channel -> presentation-only FastMCP
```

This review also changes two conclusions from the 2026-08-30 v2 review:

- **Do not introduce `codefabric.cpgd.v2`.** `SRV §§0, 5` now explicitly preserve the released
  `codefabric.cpgd.v1` method names and meanings. Evolve v1 additively and negotiate new features.
- **Do not build a permanent predecessor cutover subsystem for this deployment.** The user has
  confirmed that the displaced design was never an operational production authority, and the
  current tree has no production-serving predecessor route. Use a fresh-activation profile and
  prove target-only physical zero. If an actual deployed predecessor is discovered later, that is
  a replan trigger for a separate one-shot authority-handoff tool, not a reason to embed dormant
  predecessor state in the steady-state daemon.

Hashes remain useful for identity and integrity. They are not semantic proof. A stored digest,
count, plan text, or agreement with the displaced implementation must never replace typed
construction, exact readback, relational execution, or an independently authored behavioral
oracle (`PRIN` A.4, P18, P25, P30, P32).

### 0.1 Decision summary

| Area | Decision |
|---|---|
| Production owner | Add one compiled `CodeFabricV21Release` and one `ProductionStartupCoordinator`; no caller supplies production semantics piecemeal. |
| Construction | Replace the broad synchronized DTO graph with phase-typed `ExactWorkspaceInputs -> CandidateFabric -> SealedEpoch -> SelectedEpochRecord -> ActiveWorkspace`. |
| Genesis | Start the writer lease and recovery-only command actor before an epoch exists; the first activation uses `ExpectedHead::Empty` through the same command path. |
| Recovery | Read one exact activation-control horizon containing event, reversible table-version vector, fence, and control horizon; never independently select “latest.” |
| Readiness | One lifecycle authority drives discovery, health, handshake, status, validation, and query admission. Bound transport may honestly report bootstrapping. |
| Atomic serving | Install one immutable `Arc<ActiveWorkspace>` through an atomic slot; every query retains one epoch/authorization/resource lease to terminal cleanup. |
| Provider contracts | Replace name heuristics, fake provider execution, and duplicate schema fingerprints with exhaustive typed relation/field descriptors. |
| Query programs | Released eight-form semantics are private exhaustive Rust constructors, not caller-supplied “production” definitions. |
| DataFusion | Preserve typed `Expr`/`LogicalPlan` construction, reduced child catalogs, optimizer visibility, one governed runtime, memory reservations, and streamed execution. |
| Delta | Preserve exact-version providers, activation chain, writer fence, readback, and uncertain-outcome reconciliation; Delta table atomicity does not replace fabric activation. |
| Results | Replace `Vec<RecordBatch>` plus retained whole IPC buffers with bounded, independently decodable Arrow IPC pages in one sealed result package. |
| Query control | Reserve scheduler/journal capacity before returning an accepted handle; use bounded/coalesced events and one retention supervisor. |
| Wire | Preserve the nine released v1 methods; add only compatible fields/features and, after spec revision, one additive `GetReference` method. |
| Sessions | Mint an opaque expiring session at handshake and require it as binary metadata, bound to daemon generation, peer, principal, workspaces, profile, operations, and expiry. |
| UDS | Use an owned-socket guard with no-follow/type/owner checks, live probing, inode ownership, and ownership-checked unlink. |
| FastMCP | Complete channel readiness and handshake before lifespan yield; retain four stable tools, strict Pydantic envelopes, daemon-derived live references, and STDIO purity. |
| Cutover | Replace WP41’s predecessor state machine with fresh target activation and target-only authority/zero-state proof; retain optional handoff only behind a real-deployment trigger. |
| Plan | Reopen WP29, WP37, WP41, and WP42 acceptance wording; adjust WP31/WP32 where construction and activation APIs change. WP28 remains excluded. |

The verdict is a focused design revision, not a rejection of the relational architecture. The
lowest-level fabric implementation should not be rolled back. Implementation should resume only
against this composed target, because incremental repairs to the current startup sequence would
cement the wrong ownership boundaries.

---

## 1. Review basis and boundary

### 1.1 Scope

Unlike the v2 review, this review covers the complete production path whose seams must agree:

- operational workspace discovery and exact typed input construction;
- provider contracts, provider batch admission, normalization, authority, unknowns, and derived
  transformations;
- programmatic epoch construction, proof, exact Delta persistence, activation, recovery, commands,
  resources, cancellation, and shutdown;
- daemon process composition, readiness, UDS ownership, query scheduling, event retention, result
  publication, and resource reads;
- Protobuf/Tonic and generated grpcio compatibility;
- the `grpc.aio` client, FastMCP lifespan, four public tools, resources, progress, and strict
  Pydantic presentation models;
- decommission of stale predecessor/cutover infrastructure; and
- the resulting changes to the remaining v3 implementation-plan scope.

This is a design review. It does not certify the current implementation and does not resume plan
execution.

### 1.2 Governing sources

The primary normative sources are:

- `SUITE §§2–3, 7–9, 11–13` for programmatic authority, invariants, durable state, serving,
  transition, and release completion;
- `FAB §§1, 4, 6–7, 9–15` for the one programmatic session, provider IPC, native DataFusion
  compilation, exact Delta histories, commands, resources, reconstruction, and acceptance;
- `LIFE §§1, 8–12, 14–15` for topology, the sole mutation path, activation, admission, resource
  governance, failure recovery, startup, shutdown, security, and proof;
- `SRV §§0–15` for released compatibility, topology, negotiation, the nine service methods,
  accepted handles, the four tools, reconnect, result resources, public errors, lifespan, fairness,
  and acceptance; and
- `PRIN` A and P1–P36, especially P3, P11, P14–P20, P22–P25, and P27–P36.

The plan integration target is plan v3 WP29, WP31, WP32, WP37, WP41, and WP42. WP28/M01 remains
outside execution by explicit instruction.

The requested pinned-library lenses are:

- `rust_grpc_daemon_advanced_reference_tonic_0.14.6.md` (`tonic-ref`), especially §§16, 18–24,
  27, 30, 34–35, 37, and 40;
- `grpcio_python_advanced_reference_1.83.0.md` (`grpcio-ref`), especially §§8, 14, 18, 21, 23,
  35, 37, and 39;
- `protobuf_python_advanced_reference_7.36.0.md` (`protobuf-ref`), especially §§12, 26, 37,
  and 44;
- `fastmcp_python_advanced_reference_3.4.7.md` (`fastmcp-ref`), especially §§2, 5–7, 9, 12,
  and 33;
- `pydantic_python_advanced_reference_2.13.4.md` (`pydantic-ref`), especially §§4, 9–10, 21,
  34, 40, and 48;
- `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`
  (`datafusion-ref`), especially §§43.21, 44, 53–55;
- `arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md` (`arrow-ref`), especially
  §§5.17, 6.11–6.16, 10.3–10.5, 10.18, and 28;
- `deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md`
  (`delta-ref`), especially §§3.8, 3.15, 5.16–5.17, 6.25, 7.1–7.2, 10, and 13; and
- `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md`
  (`datafusion-arrow-ref`) and
  `deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md`
  (`delta-align`). Dependency versions are not repeated as authority; `FAB §2.1` remains the pin
  ledger.

### 1.3 Evidence method and dirty-tree boundary

No behavioral baseline was taken. The displaced design was not an accepted production oracle, and
agreement with it would not prove the new design. The review instead used:

- current authoritative-spec outlines and targeted section reads;
- an export inventory over the named Rust boundary with `ast-grep outline ... --items exports`;
- bounded textual searches for production callers, readiness projections, session ownership,
  socket operations, result materialization, and FastMCP behavior;
- direct inspection of source and generated wire inputs; and
- independent read-only reviews of the Rust lifecycle, the DataFusion/Arrow/Delta composition, and
  the gRPC/Python/FastMCP path.

The reviewed tree was already extensively dirty. This document adds one new review artifact and
does not claim ownership of the existing implementation changes. HEAD identifies the repository
lineage, not a clean baseline or proof of current behavior.

---

## 2. Non-negotiable architecture

1. **One application-owned semantic release.** Provider relation contracts, transformations, the
   eight query programs, proof construction, and query-authority construction are compiled Rust
   behavior owned by the CodeFabric release. Operational inputs select sources, policy, limits, and
   credentials; they do not define production semantics (`PRIN` P1–P3, P27, P32–P35).
2. **One exact state selector.** An activation read returns the selected event and complete
   reversible table-version vector from one control horizon. No component performs an independent
   latest lookup (`FAB §§9–11`; `PRIN` P11, P23).
3. **One mutation path.** Genesis, source waves, provider publication, activation, maintenance,
   rollback/repair, and administration go through the same fenced, idempotent command actor. Test
   seeding is never a production route (`LIFE §8`; `PRIN` P34).
4. **One immutable query world.** Each accepted query retains one active-workspace `Arc`, one exact
   epoch, one authorization, and one resource lease. Activation cannot change its world underneath
   it (`FAB §§4, 12–13`; `SRV §6`).
5. **One readiness authority.** Discovery, Tonic health, handshake, status, validate, start, and
   admin status are projections of one typed lifecycle state. No Boolean or path-exists probe can
   author readiness (`LIFE §§11–12`; `SRV §§4, 11, 13`; `PRIN` P16, P24).
6. **One result authority.** Query execution seals one result package. Events, resource reads,
   inline presentation, and cleanup all refer to that package; none stores a second semantic
   result (`SRV §§9–10`; `PRIN` P3, P23).
7. **Python is presentation only.** It does not decode Arrow into semantic rows, build plans,
   choose epochs, maintain capability state, or reconstruct references (`SRV §§1–3, 7, 13`).
8. **Unknown is explicit.** Missing provider output or incomplete coverage materializes as an
   unknown/remainder/capability gap, never empty success (`SUITE §§3, 6`; `FAB §8`; `PRIN` P20).
9. **Identity is not correctness.** Fingerprints protect identity, compatibility, integrity, and
   cursor continuity. Correctness comes from construction, execution, exact readback, and
   independent oracles (`PRIN` A.4, P18–P19, P25, P30, P32).
10. **Steady-state code models real states only.** A predecessor lifecycle is not retained merely
    because an older plan named one. Historical artifacts stay historical; executable branches
    require a real deployment fact (`PRIN` A, P26–P28, P31).

---

## 3. Current end-to-end reconstruction

### 3.1 Production entry is deliberately disconnected

The actual binary route is:

```text
src/bin/codefabricd.rs::run
  -> daemon::serve(config)
  -> Err(ProgrammaticCompositionRequired)
```

The intended route exists only as parts:

```text
admit_production_provider_relations
  -> production derived-analysis construction
  -> ProductionSemanticQueryRecipe
  -> ProgrammaticWorkspaceRuntimeFactory::build_daemon
  -> daemon::serve_programmatic
```

There is no non-test caller that closes this chain. `DaemonConfig` correctly contains process and
operational settings rather than semantic definitions, so adding dozens of semantic TOML fields is
not the answer. A compiled release recipe and production startup shell are missing.

### 3.2 First activation is circular

`ProgrammaticWorkspaceRuntimeFactory::build` requires a current snapshot and non-empty activation
head before it constructs the workspace and command runtime. Yet the sole command actor is supposed
to own activation. The vertical test solves this by directly seeding a writer generation and first
activation. That is valid fixture setup, not a production design.

The necessary missing state is **pre-epoch command authority**: the daemon can own the writer lease,
recover the command journal, and submit a genesis transaction with `ExpectedHead::Empty` before an
`ActiveWorkspace` exists.

### 3.3 Startup and readiness contradict each other

The current `serve_programmatic` path:

- constructs claims with `WorkspaceReadiness::Ready`;
- can build a staged backend;
- binds the query socket;
- publishes discovery;
- only then retries command recovery and cutover admission; and
- finally flips a separate backend `AtomicBool`.

Meanwhile, `Handshake` and `GetStatus` call `public_snapshot`, which bypasses that staged execution
gate, and both hardcode Ready. FastMCP yields from lifespan before it performs the handshake, making
the channel and compatibility check lazy. The result is not “transport ready while semantic state
bootstraps”; it is a reachable service that can falsely report semantic readiness.

### 3.4 Construction synchronizes parallel inputs

`ProgrammaticWorkspaceConstruction` accepts a builder, table-version vector, activation authority,
two semantic catalogs, producer closure, authorization, resource policies, result limits, Delta
ports, command factory, and release pins. Validation detects some disagreement, but the type still
allows callers to assemble mutually derived production objects independently.

The same issue appears in the outer production recipes:

- provider field meaning and coordinate roles are inferred from field-name substrings;
- a synthetic `pass\n` provider run is used to obtain native syntax schemas;
- the provider recipe defines a second schema fingerprint based partly on `Debug` formatting;
- derived analyses remain caller-declared; and
- `ProductionSemanticQueryRecipeInput` accepts the complete definitions of the eight supposedly
  released production query programs.

These are synchronization points and caller-defined authority, not typed release construction
(`PRIN` P27, P31–P33).

### 3.5 The epoch and Delta core are stronger than their shell

The following should be preserved:

- programmatic epoch construction uses native DataFusion catalogs, schemas, session state, a
  governed `RuntimeEnv`, and a Delta planner;
- sealing historicizes observations, and reopen takes an exact `TableVersionSet` without selecting
  latest;
- exact Delta providers support version-pinned table reads;
- activation performs prove, close admission, revalidate, append/readback, atomic swap, cache,
  reopen, and acknowledge;
- activation recovery has no append port; and
- authorized child sessions rebuild a reduced provider graph rather than hiding names in a parent
  catalog.

The activation-control authority does require one repair: after a successful in-process activation,
its long-lived provider horizon must advance atomically. A locally refreshed provider that is not
installed leaves later activations capable of reading an old head.

### 3.6 Query control is bounded in some layers and unbounded in others

The epoch resource coordinator is substantive, but `ProductionQueryService` still owns unbounded
maps for sessions, handles, idempotency keys, tasks, and a `Vec<QueryEvent>` per handle. `StartQuery`
spawns before one application scheduler has reserved all necessary capacity, while advertised queue
state and `soft_query_quota` are not the enforcing authority.

The RPC service also constructs a disconnected `FreshnessBarrier::default()` instead of consuming
the source/update watermark that governs activation. A query can therefore satisfy a transport-
local freshness state that is not the workspace lifecycle’s state.

Idempotency lookup occurs before full normalized-operation comparison. Reusing a key with a
different request can therefore return the old handle rather than a typed conflict. The event
checksum covers only query ID plus sequence, not the event payload or session/generation context.

### 3.7 The apparent Arrow stream materializes whole results

The current result path is effectively:

```text
DataFusion execution
  -> collected Vec<RecordBatch>
  -> clone/to_vec
  -> Arrow StreamWriter into Vec<u8>
  -> Arc<[u8]> retained in ArrowResultResourcePackage
  -> registry range reads
  -> grpc server-streaming method that emits one chunk
  -> Python collects that one chunk
  -> ArrowResourcePresenter joins ranges into whole bytes
  -> FastMCP resource returns a materialized value
```

Hard bounds make this finite, but peak memory still scales with the complete result and includes
multiple representations. It underuses DataFusion’s `SendableRecordBatchStream`, Arrow’s IPC
stream writers, and the object-store/buffered-write substrate.

### 3.8 Wire and FastMCP are close but not yet authoritative

Useful foundations already exist:

- one generated `codefabric.cpgd.v1` Tonic/grpcio service with nine methods;
- one long-lived `grpc.aio` channel;
- strict, frozen, extra-forbid Pydantic models and reused `TypeAdapter`s;
- exactly four FastMCP tools;
- STDIO-safe server configuration; and
- one shared production Arrow result registry beneath the RPC layer.

The remaining boundary defects are material:

- negotiated sessions are keyed only by caller-supplied agent ID and are not bound to peer identity,
  daemon generation, expiry, or operation scope;
- callers repeat identity in request bodies, and read/release do not share one session authority;
- `ReadResult` is declared streaming but sends exactly one range;
- public errors expose arbitrary internal detail, while Python parses prose for released/expired;
- FastMCP lifespan handshakes lazily;
- `get_code_graph_reference` synthesizes current references locally and never calls the daemon;
- progress events are not projected with `Context.report_progress`; and
- the adapter collapses distinct MCP call and daemon query correlation identities.

### 3.9 The current forward-cutover subsystem models a deployment that does not exist

`fabric/forward_cutover.rs` and `forward_cutover_controller.rs` implement a large predecessor-aware
state machine, durable journal, reboot/revocation evidence, rollback vocabulary, and daemon/admin
coupling. The binding has no production caller, and the cutover cannot advance through the current
startup path. It also creates an inward dependency cycle: the fabric command factory imports an
outer daemon/deployment adapter that itself imports fabric and daemon types.

For this deployment, the valuable invariants are one UDS owner, one writer, one activation head,
one target package, no fallback route, fail-closed unknown outcomes, and target-only repair. The
predecessor-specific states are falsely static baggage.

---

## 4. Findings and required dispositions

| ID | Priority | Finding | Required disposition |
|---|---:|---|---|
| IR3-01 | Blocker | `codefabricd` has no production composition caller. | Add a release-owned factory and startup coordinator; delete the error-only route after cutover. |
| IR3-02 | Blocker | Genesis requires an activation head before the sole activation actor exists. | Add pre-epoch writer/command authority and `ExpectedHead::Empty` genesis through the same actor. |
| IR3-03 | Blocker | Admission opens while command recovery and sole-authority recovery are incomplete. | Construct installed state closed; only the startup coordinator may open it after exact readback. |
| IR3-04 | Blocker | Handshake/status hardcode Ready and call a query snapshot while staged. | Split lifecycle/control projection from the semantic query backend and derive every readiness surface from it. |
| IR3-05 | High | Provider schema roles are inferred from names and a fake provider run. | Make provider relation enums exhaustively construct one typed field/schema descriptor. |
| IR3-06 | High | A duplicate schema hash uses `Debug` formatting. | Use the application-owned schema identity boundary once; prove schemas by construction and round trip. |
| IR3-07 | High | Callers define production transformations and query programs. | Move the released transformation and eight-form query sets behind private exhaustive `CodeFabricV21Release` constructors. |
| IR3-08 | High | Workspace construction accepts separately derived catalogs, vector, proof, and builder. | Replace with phase-typed aggregates and one `SelectedEpochRecord`. |
| IR3-09 | High | Activation authority can retain a stale in-process control horizon. | Atomically replace the exact provider/read horizon after successful append/readback. |
| IR3-10 | High | Result execution collects and copies all record batches and IPC bytes. | Stream into bounded, independently decodable IPC page objects and seal one manifest/package. |
| IR3-11 | High | Query/session/handle/task/idempotency maps and event vectors lack one bounded retention owner. | Add one scheduler/journal/retention supervisor with byte/count/time quotas and durable temporal state. |
| IR3-12 | High | Accepted handles can precede real admission reservation; queue fields are decorative. | Reserve scheduler and journal capacity before acceptance; queue state comes from the enforcing coordinator. |
| IR3-13 | High | Idempotency keys are not bound to the normalized operation. | Store a typed operation identity and reject mismatched reuse with a stable code. |
| IR3-14 | High | Negotiated session authority is keyed by untrusted agent ID. | Mint an expiring opaque session, transport it as binary metadata, and bind it to peer/generation/principal/workspaces/profile/operations. |
| IR3-15 | High | UDS cleanup unlinks paths without live-owner and inode checks. | Introduce an `OwnedUnixSocket` lifecycle guard with safe stale-probe and owned-inode unlink. |
| IR3-16 | High | FastMCP reports availability before handshake and derives live references locally. | Handshake before lifespan yield; route live references/status/capabilities to the daemon. |
| IR3-17 | Medium | v2 review’s new `cpgd.v2` package conflicts with accepted v2.1 compatibility. | Preserve v1 methods; make additive field/profile changes and add only the missing reference operation after spec revision. |
| IR3-18 | Medium | `ReadResult` server streaming currently produces one item and Python requires one. | Preserve the released RPC, but make it stream bounded chunks/pages; FastMCP still exposes bounded page resources. |
| IR3-19 | Medium | Error contracts use prose and can expose internal detail. | Use standard status plus stable structured metadata/details; map to strict public records and keep logs private. |
| IR3-20 | Medium | Progress and correlation are not preserved end to end. | Bind MCP call, RPC attempt, daemon query, session, epoch, and resource IDs; project coalesced progress through FastMCP context. |
| IR3-21 | Blocker | WP41/LIFE predecessor cutover is stale for a never-deployed predecessor. | Version the design/plan to a fresh-activation profile and delete dormant predecessor machinery after target-only zero-state proof. |
| IR3-22 | High | Shutdown contains declaration-only steps and releases process authority before all owned state closes. | Use a joined shutdown state machine; resources and endpoints close before writer and daemon leases release last. |
| IR3-23 | High | RPC freshness uses a disconnected default barrier. | Feed validation/start from the same lifecycle event watermark, activation head, and source-generation authority used by the workspace. |

---

## 5. Alternatives considered

### 5.1 Alternative A — thin production facade over the current constructors

Add a `CodeFabricV21ProductionRecipe` that discovers workspaces, builds every existing DTO, calls
`build_daemon`, and enters `serve_programmatic`.

This is the smallest patch, and it would make the binary run. It is rejected as the target because
it leaves genesis circular, retains independently supplied catalogs/proofs/vectors, preserves a
separate staged Boolean, and makes the new production root responsible for synchronizing objects
that should be impossible to construct inconsistently.

### 5.2 Alternative B — early-bound daemon with the current permanent cutover controller

Bind UDS early, fix bootstrapping status, build the current workspace DTOs, and retain the complete
predecessor cutover state machine as the admission gate.

This repairs readiness but retains a nonexistent deployment premise, an outer-to-inner dependency
cycle, and a second circular startup condition. It would spend substantial implementation and proof
effort simulating predecessor restart, reboot, rollback, and revocation that cannot happen in the
actual target deployment.

### 5.3 Alternative C — phase-typed active workspace plus fresh activation

This is the preferred target. It keeps the proven relational/transactional mechanisms while
changing their composition:

```text
CodeFabricV21Release + ExactWorkspaceInputs
  -> PreEpochWorkspace
  -> CandidateFabric
  -> SealedEpoch
  -> SelectedEpochRecord
  -> ActiveWorkspace
  -> DaemonKernel
```

The startup coordinator is the imperative shell; the release recipe and phase transitions are the
functional core. A one-shot deployment handoff is an optional outer adapter only when an external
census proves a real predecessor.

Alternative C best satisfies P3, P16, P23, P27, P31–P35 and removes the most synchronization work
instead of adding validators around it.

---

## 6. Preferred target architecture

### 6.1 Component topology

```text
codefabricd
  |
  +-- ProductionDaemonFactory
  |     +-- CodeFabricV21Release                compiled semantic authority
  |     +-- OperationalWorkspaceRegistry        explicit paths/policy/credentials only
  |     +-- ProductionStartupCoordinator        phase owner
  |
  +-- DaemonKernel
        +-- LifecycleAuthority                  one typed process/workspace projection
        +-- WorkspaceSlot<Arc<ActiveWorkspace>> atomic install/swap
        +-- CommandSupervisor                    sole mutation/recovery/genesis path
        +-- QueryCoordinator                     admission/fairness/journal/cancellation
        +-- ResultPackageStore                   one immutable resource authority
        +-- OwnedUnixSocket(admin)
        +-- OwnedUnixSocket(query)
        +-- AdminService
        +-- CpgQueryService v1
        +-- tonic health                         process/service liveness only

ActiveWorkspace
  +-- Arc<FabricEpoch>
  +-- SelectedEpochRecord
  +-- WorkspaceEpochQueryAuthority
  +-- AuthorizedChildSessionFactory
  +-- FabricAdmissionRuntime
  +-- EpochResourceCoordinator
  +-- Delta/activation authority
  +-- command handle and source/update lifecycle
```

`DaemonConfig` remains an operational process contract. It names roots, endpoints, local limits,
and policy locations. It does not contain relation schemas, query plans, producer closures, or
provider semantics.

### 6.2 Compiled semantic release

`CodeFabricV21Release` is a zero-sized or immutable release object whose code exhaustively owns:

- provider relation and field contracts for native syntax, Pyrefly, and rustc lanes;
- canonical identity-input, provider-local identity, coordinate, raw-kind, diagnostic, retention,
  and provenance roles;
- normalization, authority/conflict, explicit unknown/remainder, and derived transformations;
- all released application analyses;
- the eight semantic request programs and their return/selection/consumer-slot definitions;
- proof construction and producer-closure resolution; and
- query-authority construction from an actual sealed/reopened epoch.

Production inputs may supply only values that can genuinely vary: source/repository identity,
explicit policy, access grants, resource limits, deployment roots, credentials, and provider
availability. Adding a provider field or query form without a release-owned exhaustive case must
fail compilation or a construction oracle.

### 6.3 Phase types and legal transitions

| Phase | Owns | Cannot do |
|---|---|---|
| `PreEpochWorkspace` | workspace ID, safe source authority, writer lease, recovery-only command actor, Delta roots, explicit policy | query, advertise semantic capability, select latest |
| `CandidateFabric` | admitted provider batches, typed transformations, candidate tables, explicit gaps, construction provenance | serve, mutate active head directly |
| `SealedEpoch` | immutable programmatic epoch, query programs derived from that epoch, proof observations | claim active/current |
| `SelectedEpochRecord` | exact activation event, table-version vector, writer fence, control horizon, proof reference | select another version independently |
| `ActiveWorkspace` | one epoch `Arc`, query authority, admission/resources, command/update runtime | open admission unless startup authority says Ready |
| `DaemonKernel` | atomic workspace slot, lifecycle, scheduler, results, transports, joined shutdown | own semantic definitions |

Digests inside these types bind identities; they do not make two separately built values
semantically equal. Where a value is derivable from an earlier phase, the transition derives it.

### 6.4 Startup state machine

The outer coordinator owns these states:

```text
Configured
  -> DaemonLeased
  -> WriterFenced
  -> CommandRecovered
  -> GenesisRequired | SelectedEpochRecovered
  -> EpochBuiltAndProved
  -> WorkspaceInstalledClosed
  -> EndpointsBoundBootstrapping
  -> SoleTargetAuthorityObserved
  -> SoleTargetAuthorityCommitted
  -> Ready
  -> Draining
  -> Stopped
```

Every transition has a typed error and an observable, non-authoritative status projection. A
failure before Ready leaves semantic admission closed. A contradictory durable observation moves
to `FailedClosed`, never to an inferred empty state.

The UDS may bind at `EndpointsBoundBootstrapping`. At that point:

- standard health may report the process/service as serving;
- handshake may negotiate compatibility and return a bootstrapping workspace claim;
- status and live reference may report safe lifecycle/capability facts; and
- validate/start return a stable typed bootstrapping outcome without consulting an uninstalled
  semantic backend.

Only `Ready` enables semantic query admission.

### 6.5 Lawful genesis

When the exact activation-control read yields `Empty`:

1. retain the writer lease and recovered command actor in pre-epoch mode;
2. run provider lanes from exact source images and explicit inputs;
3. admit all required relations or explicit remainders;
4. build native transformations, seal the candidate epoch, and execute its proof obligations;
5. submit `ActivateGenesis { expected_head: Empty, candidate: ... }` through the same command actor;
6. append and read back the exact activation event and reversible table-version vector;
7. construct `SelectedEpochRecord` from that readback;
8. reopen the exact vector and reconstruct the query authority from the epoch; and
9. install the `ActiveWorkspace` with admission still closed.

There is no direct production seed function. Duplicate genesis converges through command identity
and exact head readback; an uncertain result is reconciled without blind append retry.

### 6.6 Warm recovery and clean reconstruction

Ordinary restart must be fast and exact:

1. recover commands/fence;
2. read one selected activation-control horizon;
3. reopen every Delta table at its selected version;
4. rebuild the programmatic session/query authority from the compiled release and exact durable
   observations;
5. verify schema/proof/producer closure by execution; and
6. install atomically before admission.

Ordinary recovery does not rerun providers merely to rediscover the selected state. A separate
clean-reconstruction oracle deletes non-authoritative temporal/cache state, reruns providers from
the declared source/input pins, and proves the same semantic outcome by decoded relations and
invariants—not by digest agreement alone.

### 6.7 Activation while running

Activation remains an application transaction around Delta’s per-table atomicity:

```text
build/prove candidate
  -> close new admission
  -> drain or retain old query leases according to policy
  -> revalidate expected activation head and writer fence
  -> append once
  -> exact readback
  -> atomically advance activation-control horizon
  -> atomically install new ActiveWorkspace
  -> reopen admission
  -> acknowledge command
```

Existing queries retain their old epoch/resource leases. New queries cannot fall back to the old
epoch. SQLite may record command/query temporal progress, but deleting SQLite must not change the
activation head selected from Delta.

### 6.8 Shutdown

Shutdown is a joined state machine, not a list of reported labels:

1. publish `Draining`/`Stopping` from the lifecycle authority;
2. stop accepting new semantic work;
3. close workspace admission;
4. cancel or drain queries under a bounded policy;
5. join result writers, provider processes, watchers, and query tasks;
6. drain and stop command actors;
7. flush/close temporal databases and result stores;
8. close servers and unlink only sockets whose recorded inode/generation is still owned;
9. release workspace writer leases; and
10. release the daemon singleton lease last.

Each completed step is evidenced by its actual owner. Placeholder `Ok(())` steps are removed.

---

## 7. DataFusion, Arrow, and Delta design

### 7.1 Provider boundary

Each provider relation enum produces an application-owned typed descriptor:

```text
ProviderRelationDescriptor
  relation_id
  provider lane and native symbol
  exact Arrow fields
  per-field semantic role
  identity/coordinate/retention/provenance rules
  required/optional coverage contract
```

The same descriptor constructs the Arrow `SchemaRef`, validates provider IPC, and feeds
programmatic admission. Schema extraction never executes a fake source. There is one schema
identity implementation at the released boundary. Missing provider data becomes a typed remainder;
new raw kinds remain representable.

Prepared admission should retain its all-lanes preflight and transactional registration. Native
Arrow batches and DataFusion `MemTable` remain appropriate for bounded provider products (`FAB
§§6–8`; `datafusion-arrow-ref` P7–P15).

### 7.2 Transformation and query compilation

Continue to lower typed semantic programs into native `Expr` and `LogicalPlan` with
`LogicalPlanBuilder`. Preserve aliases, field identity, bounds, and optimizer visibility. Use an
extension node only where native operators cannot retain required semantics, and preserve the
current high-rung evidence hierarchy (`datafusion-ref` §§43.21, 44; `PRIN` P14–P15).

The production query recipe is invoked from the actual sealed/reopened epoch. A caller cannot pass
an alternative eight-form catalog. The producer-closure resolver either finds exactly one producer
for each required family or emits an explicit remainder/ambiguity.

### 7.3 Session and catalog ownership

Use one governed `RuntimeEnv` per workspace authority domain, with explicit memory pool, spill/temp
policy, object stores, and scheduler settings. Build one root epoch session, then create a fresh
reduced child catalog/session for each authorized query or safely reusable authorization closure.

The reduced child catalog must reconstruct provider/view dependency closure. Filtering names in a
parent catalog is insufficient because views can retain concrete provider `Arc`s. Retain the
current child-provider proof and `IdentityPreservingViewTable` until an executable fault proves the
native DataFusion type preserves the required identity through analyzed, optimized, physical, and
batch schemas.

### 7.4 Streamed execution and result sealing

Replace eager collection with:

```text
DataFusion SendableRecordBatchStream
  -> cancellation/deadline/resource-metered consumer
  -> bounded page accumulator
  -> Arrow StreamWriter for one independently decodable page
  -> buffered object/file writer
  -> page seal + identity/integrity metadata
  -> ordered manifest seal
  -> atomic ResultPackage publication
```

Each Arrow page is a complete IPC stream containing its schema and a bounded number/size of record
batches. Repeating schema/dictionary state is an intentional price for independent FastMCP resource
reads and bounded retry. Page boundaries are a physical policy, not semantic identity. Batch/page
layout changes must preserve decoded rows, schema, ordering, coverage, and provenance.

The exact buffered object-store writer API must be compile-probed against the pinned
`object_store`/Arrow release before implementation. If that API cannot provide atomic seal for the
local path, use a private temporary file plus no-replace/rename publication behind the same
`ResultObjectSink` port. Do not fall back to a whole-result `Vec<u8>`.

### 7.5 One logical response and one package

The daemon authors one canonical semantic response envelope containing:

- execution, availability, freshness, completeness, and limit states;
- exact snapshot/epoch identity;
- per-query status and safe errors/notices;
- coverage and provenance projections; and
- references to the sealed relation-page resources.

The result package owns that envelope, the Arrow page manifest, page objects, leases, and cleanup
state. Its descriptor digest is an integrity identity; it is not automatically the semantic
response digest.

For a small response, Rust may create a bounded canonical JSON row projection from the same sealed
package. The adapter performs one bounded read. That projection is presentation data with a strict
size ceiling, not a second query result authority. For large relations, the MCP response links to
manifest/page resources. Python never assembles or interprets a complete Arrow relation.

### 7.6 Exact Delta authority

Continue to use high-level delta-rs table providers and writes where they preserve the contract,
with exact version loading and one application-owned version selector (`delta-ref` §§3.8, 6.25,
7.1–7.2). Retain application-owned:

- multi-table epoch activation;
- writer fencing;
- zero-blind-retry append policy;
- exact commit/readback;
- uncertain-outcome reconciliation;
- CDF gap handling and explicit rebuild;
- retention/maintenance coordination with active epoch/result leases; and
- atomic active-workspace publication.

Delta transaction atomicity is table-local. It cannot replace the activation chain that binds a
complete fabric epoch (`delta-align` §§0.2–0.4, 2.1–2.3; `FAB §§9–11`).

---

## 8. Query scheduler, journal, and retention

One `QueryCoordinator` owns capacity before handle acceptance:

- concurrent running queries;
- queued handles and queue classes;
- per-principal/workspace fairness;
- journal bytes/events;
- result bytes/pages and lease duration;
- deadlines and cancellation;
- idempotency records;
- terminal replay/tombstone windows; and
- task joining.

`StartQuery` performs authentication, compatibility, workspace authorization, canonical request
validation, normalized operation identity, idempotency comparison, and capacity reservation before
returning an accepted handle. The queue position/class is observed from that same coordinator.
Freshness evaluation uses the lifecycle authority’s event watermark, source generation, activation
head, and remaining request budget; the RPC layer does not instantiate a second barrier.

Query events are control-only in production: pinned snapshot, bounded/coalesced progress, artifact
ready, and terminal. Response bytes are resources. A bounded journal may retain every semantic
transition while coalescing replaceable progress. SQLite is appropriate for temporal handles,
cursors, sessions, idempotency, terminal status, and cleanup work; it is not epoch or semantic
result authority.

An event cursor binds event payload, query, session/principal, daemon generation, negotiated
profile, sequence, and expiry. Its MAC/digest proves integrity and continuity only. Restart either
reopens a sealed retained result/terminal record or marks an in-flight query Lost with a typed
reason; it never silently reruns under a new epoch.

---

## 9. UDS, Tonic, Protobuf, and grpcio

### 9.1 Preserve the released v1 service

Keep these method meanings:

```text
Handshake
GetStatus
ValidateQuery
StartQuery
StreamQuery
AttachQuery
CancelQuery
ReadResult
ReleaseResult
```

`StreamQuery` is the initial follow operation and `AttachQuery` is resumable attachment. That
distinction is now accepted and should not be replaced with the v2 review’s `WatchQuery`.

Use ordinary Protobuf additive evolution: new fields, new enum values handled as unknown by older
clients, reserved tombstones for removals, descriptor equivalence, and forward-unknown fixtures
(`protobuf-ref` §§12, 26, 37, 44).

### 9.2 Necessary additive wire changes

After versioning `SRV`, add:

1. an opaque session grant in `HandshakeResponse`, carried in fixed binary gRPC metadata on every
   later RPC;
2. a negotiated minor/feature bit for session metadata, content-bound cursors, typed errors, and
   paged result resources;
3. typed queue, lifecycle, terminal, cleanup, release, and error-code fields alongside legacy
   strings during transition;
4. a relative execution budget, preferably a Protobuf duration, while retaining the released wall
   deadline for the old profile;
5. a payload-bound opaque resume cursor while retaining legacy sequence/checksum fields; and
6. one additive `GetReference` RPC, because live authorized program/capability/reference relations
   do not fit honestly in `GetStatus` and must not be synthesized by Python (`SRV §7`).

Do not change the package solely for aesthetic cleanliness. A future v2 package requires an actual
incompatible meaning change and a separately accepted migration design.

### 9.3 Authentication and session authorization

The local security chain is layered:

```text
private socket directory and mode
  -> same-user UDS peer credentials
  -> initial capability/grant proof at Handshake
  -> daemon-minted opaque session
  -> per-RPC operation/workspace/handle/resource authorization
```

The minted session binds daemon instance/generation, authenticated peer UID and where supported PID,
agent principal, authorized workspaces, negotiated versions/features, host profile, allowed
operations, issue/expiry time, and anti-replay identity. Tonic request extensions carry the
verified peer identity into session checks. Repeated body IDs remain compatibility assertions and
must match the session; they never grant authority.

On reconnect the Python client creates one replacement channel and performs a new handshake. A
session from an old daemon generation is invalid. Capability material never appears in logs,
status, MCP output, or error detail.

### 9.4 Honest health and readiness

Add standard Tonic health for process/service liveness only (`tonic-ref §30`; `grpcio-ref §21`).
Do not overload it with workspace freshness or semantic readiness. The richer state remains in
Handshake/GetStatus and the lifecycle authority.

Reflection stays disabled in production unless a development profile explicitly enables the
bounded descriptor service. Compression, keepalive, HTTP/2 windows, and other tuning stay at safe
defaults until representative measurements justify a change.

### 9.5 Owned socket lifecycle

`OwnedUnixSocket` must:

1. validate a private, non-symlink parent;
2. inspect an existing endpoint without following symlinks;
3. reject wrong type/owner/mode;
4. probe a socket that may be live and refuse to unlink a responsive owner;
5. unlink only a proven stale socket under the daemon lease;
6. bind and set final permissions;
7. record device/inode plus daemon generation; and
8. on shutdown unlink only if the path still identifies the owned inode.

Path existence is neither liveness nor readiness (`tonic-ref §§22–24, 40`). The same guard pattern
applies to admin and query endpoints.

### 9.6 Deadlines, errors, and streaming

The Python call timeout, gRPC deadline, queue budget, freshness wait, DataFusion execution,
resource write, and read budgets derive from one remaining-budget model. Use monotonic time within
each process; absolute wall time is compatibility/observation, not the sole cancellation clock.

Errors use:

- standard gRPC status codes for transport class;
- a stable application error code and safe structured fields in trailing metadata or probed rich
  status details;
- private correlated diagnostics in Rust logs; and
- strict public Pydantic error records in MCP.

Python never parses prose. Adopt `tonic-types`/`grpcio-status` only after an exact Rust/Python
interop probe; fixed trailing metadata is sufficient meanwhile (`tonic-ref §§17, 35`; `grpcio-ref
§§8, 23, 39`).

Keep `ReadResult` server-streaming for compatibility, but let it produce multiple bounded chunks
for a requested resource/page. FastMCP resource functions still return one complete bounded page,
because FastMCP materializes resource return values; gRPC streaming must not be presented as
unbounded MCP backpressure.

---

## 10. FastMCP and Pydantic boundary

### 10.1 Lifespan

FastMCP lifespan performs:

1. strict settings construction;
2. one `grpc.aio` channel creation;
3. channel-readiness wait with a bounded connection budget;
4. handshake and compatibility/session validation; and
5. only then yields the lifespan context.

A compatible daemon may return a bootstrapping workspace. The MCP process can then serve status and
reference while fact queries return the stable bootstrapping result. This is not a lazy handshake.
On shutdown, release owned result leases best-effort and close the one channel.

### 10.2 Stable public catalog

Retain exactly four tools:

- `query_code_graph`;
- `validate_code_graph_query`;
- `get_code_graph_status`; and
- `get_code_graph_reference`.

Do not dynamically register one tool per relation or query program. Released MCP tool schemas are
static compatibility artifacts; live capabilities and authorized program/reference relations are
daemon observations. The reference tool may construct its own Pydantic-derived tool-output schemas,
but current program, capability, recipe, guide, and semantic reference content comes from
`GetReference`.

Use `Context.report_progress` for coalesced daemon progress with a stable phase mapping. Preserve
MCP call ID, RPC attempt ID, daemon query ID, session, epoch, result package, and resource IDs as
distinct correlation fields.

### 10.3 Resource presentation

FastMCP resources return materialized values (`fastmcp-ref §7`). Therefore:

- every resource exposed through MCP has a hard independent byte bound;
- an Arrow page resource is independently decodable;
- the manifest gives ordered page URIs, schema/coverage metadata, lengths, and integrity identities;
- the adapter forwards bytes without Arrow interpretation;
- cancellation/release calls the daemon; and
- Python never joins every page into one relation before returning.

If a client needs continuous Arrow streaming beyond bounded pages, that is a future direct client
or Arrow Flight decision, not something to emulate inside FastMCP.

### 10.4 Pydantic posture

Preserve module-scoped strict/frozen/extra-forbid models and reused `TypeAdapter`s. Validate at the
correct Python-versus-JSON seam, generate/snapshot both validation and serialization schemas, and
keep discriminated unions exhaustive (`pydantic-ref` §§9–10, 21, 34, 40, 48).

Do not add dynamic models or `orjson`. Protobuf and canonical JSON volumes at this presentation
boundary do not justify a second serialization policy, and Rust remains the canonical semantic
response author.

---

## 11. Fresh activation and legacy disposition

### 11.1 Deployment profiles

The successor design distinguishes two real deployment profiles:

| Profile | Trigger | Mechanism |
|---|---|---|
| `FreshActivation` | No deployed predecessor ever owned the workspace UDS, writer lease, serving package, or activation head. This is the current authorized case. | Target-only genesis/recovery, safe endpoint ownership, one writer, one activation head, package/route zero state, and forward repair. |
| `AuthorityHandoff` | A read-only deployment census proves a real predecessor is or was authoritative and must be revoked without downtime/data loss. | Separately designed one-shot outer controller with exact supervisor/lease/UDS evidence; removed after completion. |

Source search alone does not prove external deployment state. Here the profile is justified by the
user’s operational statement plus the absence of any current production-serving predecessor route.
If contrary deployment evidence appears, stop and replan; do not silently activate a dormant
handoff branch.

### 11.2 What fresh activation must prove

- the production package/binary is the only installable serving target;
- no default/bootstrap/ontology/predecessor backend can be selected;
- only the target owns the daemon and writer leases;
- stale sockets are handled safely and no second owner can bind;
- the exact activation head and epoch are reconstructed after process restart;
- an empty head can create genesis only through the command actor;
- target mutation is forward-only after activation;
- an uncertain command/activation result closes admission until reconciled; and
- physical zero scans cover source, features, binaries, packages, recipes, service configuration,
  generated includes, and hidden governance paths while excluding immutable history explicitly.

### 11.3 Delete rather than preserve dormant decommission machinery

After replacement behavior and target-only proof exist, remove:

- `src/fabric/forward_cutover.rs`;
- `src/forward_cutover_controller.rs`;
- `with_forward_cutover` and its inward dependency;
- cutover admin commands/status fields that have no fresh-activation meaning;
- predecessor release, revocation, reboot-simulation, rollback-to-predecessor, and temporary-bridge
  vocabulary;
- deployment fixtures/contracts/recipes that exist only for the stale predecessor state machine;
  and
- the error-only `daemon::serve` path after `codefabricd` owns real composition.

Retain the narrow reusable primitives by moving them inward only where they are independently
required: one-writer lease, monotonic generation, expected-head checks, command idempotency,
uncertain-outcome reconciliation, target-only physical observations, and fail-closed repair.

Historical designs, plans, reviews, released wire allocations, and tombstones remain immutable
history and compatibility evidence; they are never runtime inputs.

### 11.4 Required authoritative-design revision

Version, do not silently edit, the current authorities so they express deployment profiles:

- `SUITE §9` should require sole-authority transition and select fresh activation or handoff from
  an explicit deployment fact;
- `LIFE §13` should move predecessor fencing/rollback under the conditional handoff profile and
  define target-only fresh activation;
- `FAB §14` should separate clean reconstruction, fresh genesis, and optional authority handoff;
- `RM` should route the current successor through fresh activation; and
- `SRV §7` should authorize the additive live-reference RPC while retaining the stable four-tool
  catalog.

The underlying invariants in `SUITE §§3, 13`, `LIFE §§8–12`, and `FAB §§9–13` remain unchanged.

---

## 12. Library decisions

| ID | Decision | Rationale |
|---|---|---|
| LD3-01 | Keep native DataFusion plans and the highest viable extension rung. | Preserves optimizer/validator visibility and avoids a parallel evaluator (`datafusion-ref` §§43–44; P14–P15). |
| LD3-02 | Use `SendableRecordBatchStream` end to end until bounded page encoding. | Avoids eager `Vec<RecordBatch>` collection and lets resource governance meter execution (`datafusion-ref` §§53–54; `arrow-ref` §§5.17, 6.11–6.16). |
| LD3-03 | Encode independently decodable Arrow IPC stream pages. | Matches FastMCP’s materialized-resource reality while retaining Arrow-native schemas and cross-language interoperability (`arrow-ref` §§10.3–10.5, 10.18, 28). |
| LD3-04 | Use buffered object/file sinks behind an application port and atomic seal. | Separates storage strategy from semantic identity and bounds memory; exact pinned API is compile-probed. |
| LD3-05 | Preserve exact-version delta-rs providers and application activation. | delta-rs owns table snapshots/commits; CodeFabric owns multi-table epoch selection, fencing, and swap (`delta-ref` §§3.8, 5.16–5.17, 6.25). |
| LD3-06 | Preserve reduced authorized DataFusion child catalogs. | Views can retain provider objects; name filtering is not authorization. |
| LD3-07 | Preserve static generated Tonic/grpcio bindings and `cpgd.v1`. | Current accepted compatibility makes additive v1 evolution cheaper and more honest than gratuitous package replacement. |
| LD3-08 | Add Tonic health for liveness only; keep semantic readiness in the application service. | Standard ecosystem behavior without collapsing distinct state (`tonic-ref §30`). |
| LD3-09 | Reuse one `grpc.aio` channel and explicitly renegotiate after reconnect. | Matches async client lifecycle and avoids per-call channel overhead (`grpcio-ref §§14, 18, 37). |
| LD3-10 | Keep FastMCP tools static and resources bounded. | Dynamic catalog generation and whole-relation resources would duplicate authority or defeat backpressure (`fastmcp-ref §§5–7, 9`). |
| LD3-11 | Keep strict Pydantic envelopes and reused adapters. | Boundary validation/schema generation stay explicit and efficient (`pydantic-ref` §§21, 34, 40, 48). |
| LD3-12 | Do not add `orjson`. | It supplies no missing authority or throughput capability on this presentation-only path. |
| LD3-13 | Keep hashes/MACs for identity, integrity, and authentication only. | Semantic proof remains construction/execution/readback (`PRIN` A.4, P18, P25, P30). |
| LD3-14 | Use `arc-swap`/equivalent for one atomic active-workspace slot. | Readers pin one immutable authority without lock-coupled epoch mutation; lifecycle still owns valid transition order. |

---

## 13. Plan integration

The current plan remains useful as a traceability container, but several acceptance clauses are
stale. Preserve packet IDs and issue a versioned amendment/successor rather than rewriting proving
history.

### 13.1 WP29 — replace broad composition with phase-typed startup

Add or revise WP29 to require:

- `CodeFabricV21Release` and exhaustive provider/transformation/query construction;
- pre-epoch command recovery and lawful genesis;
- one `SelectedEpochRecord` from an exact control horizon;
- atomic installation with admission closed;
- real `codefabricd` entry into the composed kernel; and
- cold genesis, warm exact recovery, partial rollback, multi-workspace/worktree isolation as
  applicable to the deployment topology, and joined shutdown.

Remove any pass condition satisfied by a factory hash, static census, or test-only direct seed.

### 13.2 WP30 — continue rapid ontology/bootstrap decommission

WP30 remains directionally correct and should not be delayed by a compatibility baseline. Land the
positive `CodeFabricV21Release`/phase-typed consumer first, then delete the displaced ontology,
bootstrap/model compiler, generated-schema authority, dual-epoch types, migration command, and
runtime readers as already scoped. Preserve only released wire allocations, intrinsic identity
primitives, and immutable history. No adapter, daemon, test helper, feature, recipe, or fallback may
select the old authority merely to support comparison.

### 13.3 WP31/WP32 — narrow construction and advance activation authority

WP31 should make released query/analysis programs private compiled constructors and remove caller-
defined production catalogs. WP32 should return the activation event, exact reversible vector,
fence, and control horizon together; prove two successive in-process activations and selection of
an older exact version on restart.

### 13.4 WP37 — one real production vertical

WP37 should implement this review as one dependency-closed vertical:

```text
real source image/change
  -> exact provider batches or explicit gaps
  -> native transformations and proof
  -> Delta publication + genesis/activation
  -> atomic ActiveWorkspace install
  -> scheduled authorized DataFusion query
  -> streamed Arrow result package
  -> safe UDS + generated v1 gRPC
  -> eager-session grpc.aio client
  -> strict FastMCP response/resource
```

The vertical must use the production binary and package, not an in-process or injected backend.

### 13.5 WP41 — retain ID, replace stale semantics

Rename/redefine WP41 for traceability as **Execute fresh successor activation and prove sole target
authority**. Replace predecessor phases and rollback proof with:

- target package/service/feature/route zero state;
- daemon/writer/UDS/activation ownership before and after restart;
- genesis idempotency;
- target-only mutation and forward repair;
- uncertain-outcome fail-closed reconciliation; and
- absence of temporary handoff machinery.

Do not reintroduce a predecessor binary or synthetic deployment merely to make the old oracle pass.

### 13.6 WP42 — certify the revised authority, not stale packet text

WP42 should rerun the versioned packet oracles that still represent the accepted target. It must
not require a stale WP41 predecessor-reboot oracle after that obligation has been superseded.
Certification remains blocked on a real binary start/serve/cancel/restart/reconstruct path, strict
four-domain compatibility, legacy zero state, and independent implementation review at one trusted
HEAD.

---

## 14. Executable proof obligations

### 14.1 Composition and genesis

- Real `codefabricd serve` reaches the production kernel; `ProgrammaticCompositionRequired` is
  unreachable outside a removed/explicit diagnostic test.
- Empty activation head creates exactly one genesis through the command actor; duplicate delivery
  converges and an injected unknown append result reconciles without a blind retry.
- A direct test-seed analogue is rejected in production configuration.
- Missing required operational inputs fail before semantic admission and leave no leaked lease,
  socket, task, or partial workspace.

### 14.2 Compiled semantic authority

- Adding a provider field without an exhaustive semantic role fails construction or a compile-time
  match/test.
- Missing Pyrefly/rustc/native output emits explicit remainder/capability rows, not empty success.
- Mutating a typed transformation operand changes the decoded derived relation.
- All eight query forms are constructed only by the release recipe and execute through native
  DataFusion plans.
- An unauthorized provider retained through a view is rejected by child-catalog closure proof.

### 14.3 Delta and activation

- Two table versions containing different rows prove restart opens the activation-selected version,
  including an older version.
- Two successive in-process activations prove the control horizon advances.
- Faults after close, append, readback, swap, reopen, and acknowledge prove no early admission and
  correct uncertain-outcome reconciliation.
- Deleting SQLite temporal state does not change the semantic activation head reconstructed from
  Delta.

### 14.4 Readiness and lifecycle

- A bound query UDS during bootstrap returns bootstrapping from handshake/status and a stable query
  rejection; it never reports Ready.
- Every readiness surface changes causally from the same lifecycle transition.
- FastMCP refuses lifespan readiness on channel/handshake incompatibility but may start against an
  honestly bootstrapping daemon.
- Shutdown faults prove every worker/store/socket closes before writer and daemon leases release.

### 14.5 Sessions, UDS, and wire

- Wrong UID, wrong capability, expired session, old daemon generation, wrong workspace, wrong
  operation, wrong handle owner, and wrong resource owner all fail with stable codes.
- Reusing a session body ID without its metadata grant fails; body/metadata identity disagreement
  fails.
- A live foreign socket is never unlinked; a proven stale owned socket is recovered; shutdown does
  not unlink a replacement inode.
- Rust and Python generated descriptors remain equivalent; old v1 clients ignore additive fields;
  new clients reject missing required feature negotiation.
- Unknown enum/field fixtures survive forwarding as required by the compatibility policy.

### 14.6 Scheduling, result streaming, and retention

- Capacity is reserved before acceptance; queue position comes from the enforcing scheduler.
- Same idempotency key plus same normalized operation converges; any differing bound field returns
  typed conflict.
- Slow consumers keep resident memory within the declared pool plus bounded page buffers.
- Partition count, batch size, and page size variations preserve decoded rows, order, schema,
  coverage, and provenance.
- Cancellation during planning, execution, page write, gRPC read, and MCP resource read joins work
  and releases permits.
- Sessions, journals, handles, tasks, idempotency entries, results, and tombstones expire under one
  observable retention policy.

### 14.7 FastMCP presentation

- The server registers exactly four tools and no dynamic semantic catalog.
- Live program/capability/reference content changes when daemon authority changes and cannot be
  changed by editing only Python static data.
- Progress reaches a FastMCP test client with bounded/coalesced updates.
- Python never imports Arrow/DataFusion/Delta processing libraries and never decodes relation
  semantics.
- STDOUT remains protocol-only under success, bootstrapping, errors, reconnect, cancellation, and
  shutdown.

### 14.8 Fresh-activation zero state

- No predecessor/default/bootstrap/ontology serving or mutation route is selectable in source,
  features, binaries, packages, recipes, service configuration, generated includes, or hidden
  governance paths.
- One target owns UDS, writer, serving, and activation before/after process restart.
- No cutover bridge/controller remains in the production dependency graph.
- Immutable historical artifacts are excluded by exact class/path and have no live reader.

---

## 15. Performance and measurement obligations

The target removes known scaling defects but does not claim benchmark results. Measure after the
vertical exists:

- daemon cold genesis and warm exact recovery;
- time to transport-bound bootstrapping and time to semantic Ready;
- DataFusion execution throughput with representative eight-form queries;
- peak memory across collect-based current code versus streamed page sealing;
- page size/batch count tradeoffs and FastMCP resource latency;
- concurrent fairness, cancellation latency, and queue accuracy;
- adapter channel reconnect/handshake latency;
- result-store write/read throughput on the supported local filesystem; and
- shutdown/drain time with active queries and leased resources.

Do not tune compression, HTTP/2 windows, keepalive, multipart thresholds, page sizes, spill limits,
or retention duration from folklore. Record the workload, environment, distribution, resource
summary, and semantic equality oracle for every accepted tuning change.

---

## 16. Replan triggers

Reopen the relevant design boundary rather than weakening it if:

- a provider schema cannot be constructed without executing a provider;
- query programs cannot be reconstructed deterministically from the release plus explicit inputs;
- one activation read cannot return event and reversible table vector from the same control
  horizon;
- recovery requires a process-local candidate, stored catalog clone, or digest-as-correctness;
- native DataFusion types cannot preserve required field identity or authorization closure at the
  selected extension rung;
- the pinned object-store/Arrow writer cannot seal bounded pages without a whole-result copy;
- FastMCP introduces a genuinely streaming resource contract that changes the page decision;
- a real deployed predecessor is discovered;
- a released v1 field meaning must change incompatibly rather than additively; or
- representative measurement shows the chosen process/page/session topology violates the declared
  resource envelope.

Each trigger identifies missing authority or an incompatible capability. It does not authorize a
silent fallback, a second semantic implementation, or restoration of the displaced ontology
system.

---

## 17. Acceptance conditions for this design target

This target is ready to plan and implement when all of the following are accepted:

1. `CodeFabricV21Release` owns production provider, transformation, proof, and query semantics.
2. Startup includes lawful pre-epoch genesis and exact warm recovery.
3. One lifecycle authority controls all readiness and admission projections.
4. Active workspace installation is atomic and query leases pin immutable epoch authority.
5. DataFusion output remains streamed through bounded Arrow IPC page sealing.
6. One result package owns canonical response, manifest/pages, leases, and cleanup.
7. The released v1 service is evolved additively, with a session grant and live-reference route.
8. UDS lifecycle, peer/session authorization, errors, deadlines, and retention are bounded and
   explicit.
9. FastMCP handshakes eagerly, remains presentation-only, exposes four stable tools, and reads live
   references from the daemon.
10. Fresh activation replaces the stale permanent predecessor state machine for this deployment,
    and the corresponding authoritative specs and plan packets are versioned.
11. Proof is based on typed behavior, decoded relations, exact durable readback, causal faults, and
    independent expectations—not hashes, counts, plan text, or legacy agreement.
12. The real source-to-FastMCP vertical and target-only restart/zero-state proof are the terminal
    implementation gates.

Until those revisions are integrated, the correct next action is to avoid further local repair of
`daemon::serve`, the staged Boolean, or the existing forward-cutover controller. Those patches would
make the current ownership cycle harder to remove.
