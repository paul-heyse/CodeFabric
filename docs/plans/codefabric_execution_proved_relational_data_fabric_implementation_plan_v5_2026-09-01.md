---
artifact: implementation-plan
plan_id: codefabric-execution-proved-relational-data-fabric
version: v5
date: 2026-09-01
status: approved
design_path: docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v2.md
design_version: v2
baseline_commit: 6e74cfbbe23da73dd110a2adb232276e00f9a3ad
working_tree_digest: ba4dc8a168b1dcaf377625bad779a9176825e10093e6d5803027ea49dc255c19
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v5_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
---

# CodeFabric execution-proved relational data fabric -- implementation plan v5

This draft converts the accepted FastMCP 4 presentation-boundary design and the reconciled v4
implementation status into one dependency-closed successor for the remaining program. It preserves
the proved relational-fabric substrate through v4 WP36, explicitly revalidates the stale WP29/WP30
composition outcomes, and replaces the invalidated FastMCP 3-era WP37--WP40/WP42 tail. It does not
reopen the ontology/bootstrap/model authority removed by v4, restore a hash where the target now
proves a fact programmatically, or require agreement with an unvalidated predecessor design.

The work-packet identities continue at WP43 so that v4 history remains unambiguous. This file does
not create its declared state file, alter `docs/plans/active-plan.json`, mutate v4 state, or authorize
implementation. Independent audit, approval, and the confirm-gated activation transaction are
required before execution.

## 1. Outcome, boundaries, and execution law

### 1.1 Outcome

At completion:

1. A synchronized authoritative-suite successor, expected to be
   `codefabric-relational-data-fabric@2.3.0`, expresses one modern MCP product profile while keeping
   every v2.2 artifact immutable history.
2. One stable Rust daemon per workspace remains source, provider, DataFusion, Delta, lifecycle,
   query, challenge, result, resource, authorization, and recovery authority. One FastMCP STDIO
   process per agent remains a reconstructible presentation cell.
3. Fresh workspace startup reaches `Ready` only after the command actor creates or reconciles one
   exact activation event, table-version vector, writer fence, and control horizon. An unknown
   append outcome is read back and reconciled; it is never blindly retried, seeded, selected by
   latest-state lookup, or accepted because a digest agrees.
4. The unreleased `codefabric.cpgd.v2` contract exposes one atomic `StartQuery` closed outcome:
   `Accepted`, `InputRequired`, or `Rejected`. The explicit validation operation remains a pure
   dry-run projection and never becomes the ordinary start preflight.
5. Typed daemon-authored input challenges carry bounded constraints and an opaque daemon
   continuation token. FastMCP 4 maps them to native guarded input and seals request state, while
   the daemon independently reauthorizes every leg and enforces expiry, replay, round, principal,
   workspace, argument, policy, and generation binding.
6. The daemon mints public resource handles only after creating authoritative resource/lease state.
   Every read and release is reauthorized; Python retains no handle-to-secret or resource-lease
   registry and exposes neither filesystem paths nor secret lease tokens.
7. FastMCP 4.0.0 and MCP SDK 2.1.1 serve only protocol era `2026-07-28`, with the compatibility
   bridge disabled. Older eras fail before business dispatch and have no alternate result path.
8. The MCP catalog contains exactly four stable tools, two bounded resource families, one narrow
   authorized reference-completion surface, and no prompts, tasks, application-owned/custom server
   extensions, sessions, FastMCP auth policy, cache authority, providers, transforms, proxy,
   gateway, Tool Search, Code Mode, apps, sampling, or root-listing surface. Modern discovery may
   contain only FastMCP 4.0.0's unavoidable empty framework-owned
   `io.modelcontextprotocol/ui` advertisement; no CodeFabric component consumes it.
9. Verified fd3 launch settings construct one application through a pure factory. A hidden typed
   `DaemonPort` contains generated gRPC types; public strict Pydantic models and FastMCP `Context`
   carry only presentation inputs and request-local behavior.
10. Host cancellation after acceptance invokes daemon `CancelQuery` once under a separate cleanup
    budget. Observation loss never resubmits `StartQuery`; reconnect re-handshakes and resumes a
    watch by daemon query identity when still authorized.
11. Reference completion is daemon-backed, authorization-filtered, capped, advisory, and
    revalidated by the eventual resource operation. It never enumerates result handles, repository
    paths, source/entity inventories, hidden capabilities, principals, or denied existence.
12. Built-in FastMCP request spans receive only allowlisted correlation attributes. No request
    payload, answer, source content, session/challenge/resource token, path, gRPC metadata, or
    diagnostic byte reaches telemetry or STDOUT.
13. The real supervisor -> `codefabricd` -> fd3 launcher -> installed FastMCP 4 wheel -> modern MCP
    host path proves guarded input, atomic acceptance, progress, bounded resources, completion,
    cancellation, reconnect, restart, two-agent isolation, and protocol silence against real
    source-to-Delta-to-DataFusion behavior.
14. FastMCP 3 pins and APIs, the camel-case compatibility path, normal-path duplicate validation,
    top-level duplicate freshness, Python public-ID/resource-lease authority, static adapter schema
    authority, phantom prompts, unused settings dependencies, stale gates, and any unused Python
    identity/canonicalization implementation reach physical zero state.
15. `src/bin/codefabric.rs` and `src/bin/codefabricd.rs` remain only because they own real process
    boundaries. They are thin command/bootstrap shells over library-owned typed policy and runtime
    ports; no static schema/model generation or ad hoc semantic configuration returns to production
    binaries.
16. Post-purge package, resource, and performance evidence is measured on the target topology. The
    final candidate passes FreshActivation, all successor packet oracles, the retained v4 substrate
    proofs, four-domain release gates, and an independent implementation review at one trusted
    HEAD.

### 1.2 Non-goals

- No legacy FastMCP 3 operability, pre-modern MCP product profile, `ctx.elicit()` fallback, dual
  result shape, old-client equivalence suite, or shipped-v2 compatibility fiction.
- No rollback to ontology/bootstrap/model/generated-schema authority, no old-design comparator as
  correctness evidence, and no reintroduction of hashes, fingerprints, counts, plan text, receipts,
  or caches as semantic/activation proof.
- No Python Arrow, DataFusion, Delta, semantic planning, request normalization, authorization,
  canonical identity, workflow, task, result, cache, or resource-lease authority.
- No arbitrary SQL, public physical catalog names, serialized logical plans, filesystem/object
  paths, secret lease tokens, or dynamic tool generation from relations or entity kinds.
- No FastMCP HTTP/shared-edge profile, framework auth, Docket/Redis worker, MCP gateway, upstream
  aggregation, response cache, custom server extension, or detached MCP task until a separately
  accepted functional outcome requires it.
- No new Cargo root, Python service, semantic process boundary, or library added only to organize
  code. No `src/bin` proliferation; the two operational binaries reuse the root library.
- No speculative Tonic/HTTP2/keepalive/compression/rich-status tuning and no folklore performance
  thresholds. Capability and tuning changes require pinned-stack evidence and representative
  measurements.
- No state creation, active-plan switch, implementation mutation, decommission, package publish,
  or deployment in this plan-authoring turn.

### 1.3 Baseline and inherited trust posture

Baseline HEAD `6e74cfbbe23da73dd110a2adb232276e00f9a3ad` is the ancestral v4 WP36 state
commit. The frontmatter working-tree digest is SHA-256 over `git status --porcelain=v2 -z` before
this plan file was created; it identifies a dirty planning snapshot and proves neither ownership nor
correctness.

The v4 status review established the following execution facts:

- WP31, WP32, WP33, WP34, WP35, WP36, M02, M04, DB09, and DB10 retain ancestral proving commits;
  the current named packet oracles for WP31--WP36 pass.
- WP29 and WP30 are stale only because later working-tree recipe edits coupled their behavior gates
  to the unfinished serving vertical. Their target construction and physical ontology/bootstrap/
  model decommission remain. V5 revalidates those outcomes without restoring predecessor paths.
- WP37--WP40 and WP42 are invalidated by the accepted FastMCP 4 target. WP41 FreshActivation remains
  required but moves behind the successor release candidate.
- The dirty tree contains reusable supervisor, owned-UDS, process-runtime, session, generated gRPC
  v2, client, adapter, launcher, and real-process test work. Existence earns no packet completion;
  WP44--WP47 rediscover ownership, retain target-conforming bytes, and replace stale assumptions.
- Current root, adapter, proto, supervisor, and retained WP31--WP36 focused checks are green, but the
  real vertical fails closed during fresh activation with `ReadbackUnavailable` and no reconciliation
  probe evidence. One wire selector is empty, one recovery recipe names a deleted test, and two
  planned serving recipes do not exist. These are successor work, not reasons to weaken proof.

V5 state, when activated, contains only WP43--WP52, M08--M12, and DB15--DB18. It does not copy v4
completion labels into a new state file. The inherited foundation is trusted through ancestry and
rerun evidence, then explicitly re-certified by WP52.

### 1.4 Execution law

- The accepted FastMCP 4 review is immediate design authority and composes with the accepted daemon/
  gRPC review. The versioned v2.3 suite issued by WP43 becomes normative before design-bearing code
  changes complete; released v2.2 files are never edited in place.
- A packet starts only after all named v5 dependencies are complete at ancestral proving commits,
  declared inputs are fresh, current-tree impact is rediscovered, and overlapping paths are
  reserved or serialized. Packet numbering is traceability, not permission to skip dependencies.
- Each packet completes only when exactly four unique substantive executable oracles pass at its
  proving commit and candidate HEAD: integrity (`INT`), positive behavior (`BEH`), negative/failure
  (`NEG`), and operations/recovery/resource/performance (`OPS`). Every selector reports a nonzero
  selected count and a committed discriminating fault.
- An aggregate gate preserves each child exit code, selected count, and evidence artifact. A green
  structure, hash, descriptor generator, schema fingerprint, package inventory, or mock-only test is
  never a behavioral oracle.
- Native Arrow/DataFusion/Delta, generated Protobuf/gRPC, FastMCP, and Pydantic capability is used at
  the highest viable target rung. Custom behavior records the rejected native rung, the full
  replacement contract, resource/cancellation behavior, and an executable replan trigger.
- Digests bind immutable identity and integrity only. Whenever the program can construct, execute,
  read back, decode, or causally discriminate the claimed fact, that behavior is the proof; no packet
  may restore a digest merely to satisfy stale wording.
- Target consumers land before predecessor deletion. Zero state covers source, exports, generated
  outputs, features, targets, locks, packages, recipes, workflows, services, rules, fixtures,
  installed artifacts, hidden live paths, and compatibility switches. Unreachable, deprecated,
  ignored, feature-disabled, or absent from one grep is not deletion proof.
- After target-format mutation, recovery is forward repair through the target command/contract.
  Before an activation append, discard the private candidate and keep admission closed. After an
  unknown append result, exact coherent readback reconstructs authority or the daemon remains
  failed-closed; no blind retry or predecessor restoration is permitted.
- Every packet records implementation, tests, four oracle recipes, fixtures/faults, and evidence in
  one proving commit without staging, resetting, overwriting, or attributing unrelated dirty work.

## 2. Source design, declared inputs, and library decisions

The following planning inputs are immutable. Any unaccepted drift makes this draft stale and
requires a revised plan before activation or dependent execution.

| path | sha256 |
|---|---|
| docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v1.md | 21c957501ca2607c98067ace58ac07f49eddb72154bd2debff72d7f28d122084 |
| docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v2.md | 202329441a517e097ac3a045cbf1022bf05242c8de2e8f2e2f58d5ecd3b9ee6f |
| docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md | 2c4e819bc416a9fd7fcf5a76928aa17a470a5b296b003e62cf451b170513b7ae |
| docs/reviews/implementation_status_codefabric_execution_proved_relational_data_fabric_v4_2026-09-01_v2.md | 726dcffedf9307ee02ff967d755ea498f633e10dd430d63620c76d0c08146308 |
| docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md | a11bd07e7e2533df8fc62e84acddb65258ba9cd3a60b03bd4f32b3749ba09ced |
| docs/library_ref/full_data_fabric_design_principles_v2.md | eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5 |
| docs/library_ref/fastmcp_python_advanced_reference_4.0.0.md | e9ee398e2d0e8ef63995cf38368fdf20c00a21a138dad64ea3bdf4a7c5c53f86 |
| docs/library_ref/pydantic_python_advanced_reference_2.13.4.md | 4f66f29a9fde6feed03a0755942db9bb9fb0834f57ff49ab80ab448d65d6a477 |
| docs/library_ref/grpcio_python_advanced_reference_1.83.0.md | e01fd5483b679cb62ef09e2c50228ab74eab298c2d559774f1f4c7ddd3320f78 |
| docs/library_ref/protobuf_python_advanced_reference_7.36.0.md | 2b9a2151f25e610ef75a43739b23852fd5faac3b183bbe1c374ff9923001798e |
| docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md | 6dd8665f9c33e70181c292b91f6376fd76b12d7e3073a956e48ba9542d9adf32 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.2.md | a11b496912e98767d8e7e7c11ca8ce49a05fe53e9755dfbe8a9e1fcb51e985ab |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.2.md | 4e92c43769165b75e5c8e6f37d10571de9607f39f1344c9c43a7590457c9827e |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.2.md | ffd340a227e7c55f99b6cd4c4b0406e59ae9ed52702d50425ff121185afaeec2 |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.2.md | efb447cbd0e444afeacd14c31828535ee46147c010d6ab62e3e5383c07b09fc2 |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v2.2.md | cd4e5c4c991f3afad081d09febf94dce3d106596e638947c762ebfb15abd038b |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.2.md | 137bd77c05874ccd458679bbcb95c27a28b066ae585f2d2ed81f14fe69928a98 |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md | 6b8ac7e546079553360a47a277bd1e11a74e1ea4581a14bd8d1ea4ab6b6b6864 |
| docs/authoritative_design/codefabric_2.2_implementation_roadmap_v1.0.md | b48499c56d99622071fd98a85e86073f2010f6d06a5a93314955cbc8e8f51546 |

### 2.1 Planned design-authority evolution

WP43 allocates the collision-free synchronized suite version `2.3.0` with `v2.3` filenames and a
`codefabric_2.3_implementation_roadmap_v1.0.md` roadmap. All eight suite roles receive versioned
successors so membership remains coherent. SUITE, SRV, QRY, and RM receive the accepted substantive
changes. ONT, GEN, FAB, and LIFE carry forward with explicit predecessor linkage unless cross-domain
review proves a genuinely new invariant belongs there; serving mechanics are not duplicated merely
to make every file change.

The successor SRV selects FastMCP 4.0.0, MCP 2.1.1, protocol `2026-07-28`, four tools, two resource
families, guarded input, authorized completion, explicit cancellation, and adapter-authority zero
state. QRY owns typed preparation/input requirements and the atomic start outcome. SUITE owns the
cross-artifact release identity and no-fallback rule. RM owns dependency/decommission order.
Derived indexes are updated only after their normative sources, and cite those sources rather than
becoming authority.

### 2.2 Live pin and capability posture

Execution rederives every pin from the issued successor FAB, Cargo metadata, and the adapter lock.
The retained Rust universe remains Arrow/Parquet 59.2.0, DataFusion 55.0.0, `object_store` 0.13.2,
delta-rs revision `43a0cf10`, Tonic 0.14.6/Prost 0.14.4, grpcio/grpcio-tools 1.83.0, and protobuf
7.36.0. The target adapter universe is FastMCP 4.0.0, MCP SDK 2.1.1, Pydantic 2.13.4, and Python
3.14.7. `just stable-graph-check`, `just proto-check`, the locked adapter gates, and installed-wheel
inspection outrank copied version prose.

DataFusion stays the programmatic query/catalog/expression/planning authority; Arrow stays the
typed columnar and independently decodable page boundary; exact Delta versions stay durable table
authority. Python neither decodes Arrow nor reconstructs a semantic request. Generated Protobuf
messages terminate in the adapter port, while strict Pydantic models own only public presentation
validation and schema publication.

### LD5-01 — FastMCP modern presentation cell

**Decision:** adopt
**Version basis:** FastMCP 4.0.0 on Python 3.14.7, exact locked server/client feature resolution.
**Displaces:** FastMCP 3.4.7 construction/decorators, import-time constitutional server shape, and
pre-modern result branches in `codefabric-cpg-mcp/`.
**Risk:** a first major-version target may expose unstable guard or protocol-admission seams; pin the
exact version and require installed-STDIO behavior plus discriminating faults before release.
**Validation:** `just fastmcp4-dependency-contract-check` and
`just fastmcp4-modern-protocol-check`.

### LD5-02 — MCP SDK protocol types

**Decision:** adopt
**Version basis:** `mcp==2.1.1` as a direct adapter dependency with one governed public import style,
`from mcp import types as mcp_types`, and `FASTMCP_MCP_CAMELCASE_COMPAT=false` in every gate.
**Displaces:** transitive protocol-type reliance, mixed `mcp_types`/`mcp.types` imports, camel-case
Python SDK attributes, and compatibility-bridge behavior.
**Risk:** wire JSON remains protocol-defined camel case while Python attributes are snake case;
live schema/inspection and bridge-off tests must distinguish the two.
**Validation:** `just fastmcp4-contract-observation-check`.

### LD5-03 — Pydantic presentation contracts

**Decision:** retain-current
**Version basis:** Pydantic 2.13.4 with strict, frozen, extra-forbid module-scoped models and reused
`TypeAdapter` instances.
**Displaces:** live static `contracts/adapter/*.schema.json`, dynamic Pydantic model generation, and
duplicate settings/schema registries.
**Risk:** generated JSON Schema and fingerprints can be mistaken for correctness; they remain
observations checked against independently authored public clauses and behavior.
**Validation:** `just fastmcp4-public-surface-check`.

### LD5-04 — Native guarded input and completion

**Decision:** wrap
**Version basis:** FastMCP 4 `InputRequiredResult`, `RequestStateSecurity`, request-local `Context`,
and completion handler APIs under the modern protocol.
**Displaces:** accept-then-cancel ambiguity, adapter-authored semantic questions, phantom prompts,
and tool-simulated completion.
**Risk:** FastMCP request-state sealing is integrity defense, not daemon authority; an independent
daemon token and per-leg reauthorization remain mandatory.
**Validation:** `just fastmcp4-guard-roundtrip-check` and
`just fastmcp4-completion-authorization-check`.

### LD5-05 — FastMCP stateful and gateway capabilities

**Decision:** reject application adoption; observe the unavoidable inert framework advertisement
**Version basis:** FastMCP 4.0.0 sessions, tasks, auth, cache, extension, provider, transform,
proxy/gateway, Tool Search, Code Mode, and app capabilities reviewed against the local STDIO target.
An exact-stack probe established that modern discovery unconditionally includes the empty
framework-owned `io.modelcontextprotocol/ui` capability even when the application extension
registry and component catalogs are empty; the pinned release exposes no supported disable switch.
**Displaces:** no accepted target capability; explicit rejection prevents a second identity,
workflow, cache, policy, or upstream-composition authority.
**Risk:** transitive defaults or future refactors may register forbidden components; set
`tasks=False`, omit related dependencies, and inspect live discovery, the application extension
registry, and resolved component catalogs separately. Any extra extension identifier, non-empty UI
settings, registered UI component, or CodeFabric consumer fails closed.
**Validation:** `just fastmcp4-adapter-authority-zero-state-check`.

### LD5-06 — Generated local gRPC boundary

**Decision:** retain-current
**Version basis:** Tonic 0.14.6, Prost 0.14.4, grpcio/grpcio-tools 1.83.0, protobuf 7.36.0, one
committed descriptor IR, and private owned UDS transport.
**Displaces:** handwritten wire DTOs, JSON/`Any` semantic envelopes, reflection-dependent clients,
and parallel v1/v3 services.
**Risk:** source/descriptor/Rust/Python drift or a shipped-contract fiction; revise the unreleased v2
surface atomically and prove exact generated interoperability.
**Validation:** `just proto-check`, `just proto-repro-check`, and
`just fastmcp4-daemon-wire-contract-check`.

### 2.3 Current-tree known impact

Known touch points seed packet preflight but are not frozen must-touch manifests:

- authority and planning: eight `docs/authoritative_design/*v2.3*` successors,
  `docs/spec_index/**`, `AGENTS.md`, independent acceptance contracts/reviews, artifact/governance
  tooling, `justfile`, and CI;
- startup and process topology: `src/supervisor.rs`, `src/process_runtime.rs`,
  `src/owned_unix_socket.rs`, `src/session_authority.rs`, `src/daemon.rs`, lifecycle/activation
  modules, `src/bin/codefabric.rs`, `src/bin/codefabricd.rs`, Cargo targets/features, and integration
  cases;
- daemon contract: `contracts/rpc/cpg_query_service.proto`, descriptor/baseline/census artifacts,
  `tooling/proto/**`, generated Rust/Python bindings, `src/rpc.rs`, `src/query_service.rs`, query
  coordinator/result/resource ports, authorization/session code, and interoperability fixtures;
- adapter: `codefabric-cpg-mcp/pyproject.toml`, `uv.lock`, package entry points, `server.py`,
  `settings.py`, `daemon/**`, `contracts/**`, tests, wheel resources, and STDIO harnesses;
- decommission and proof: `contracts/adapter/**`, old evidence/fixtures, structural rules,
  recipe selectors, package manifests, release scripts, service configuration, and hidden live
  consumers.

The only current production files under `src/bin/` are `codefabric.rs` and `codefabricd.rs`. Their
target role is now operational and justified. Packets must keep their parsing/bootstrap bodies thin,
move semantic defaults and policy into typed library-owned contracts, and reject any resurrection of
static schema/model generation in those binaries.

## 3. Global target invariants and dependency graph

- **I5-01 -- Sole relational authority.** Installed providers, typed programmatic transformations,
  application analyses, exact Delta histories, and authorized DataFusion plans remain the only live
  semantic substrate.
- **I5-02 -- Programmatic proof.** Construction/execution/readback/decoded causality proves semantic
  behavior; hashes and fingerprints establish identity only.
- **I5-03 -- Atomic present state.** Every admitted request pins one immutable proved FabricEpoch and
  never mixes source, provider, analysis, policy, or table generations.
- **I5-04 -- Exact activation.** Genesis, append, readback, unknown-outcome reconciliation, and
  restart use one coherent exact event/vector/fence/horizon.
- **I5-05 -- One lifecycle owner.** Startup, readiness, admission, drain, shutdown, and recovery read
  one lifecycle authority; only `Ready` admits semantic work.
- **I5-06 -- One supervisor/daemon.** One operator-policy-owned supervisor and one daemon exist per
  workspace; per-agent adapters attach and never start or own a daemon.
- **I5-07 -- Owned local endpoints.** Every UDS/rendezvous endpoint is private, no-follow,
  peer-credential-bound, generation-recorded, and replacement-inode-safe at cleanup.
- **I5-08 -- Atomic start.** Ordinary query admission has one start operation and one closed outcome;
  validation is pure data and cannot race acceptance.
- **I5-09 -- Daemon-authored challenge.** Semantic missing-input requirements, constraints,
  continuation, replay defense, and round limits are Rust authority.
- **I5-10 -- Dual guard integrity.** FastMCP seals request state while the daemon independently
  authorizes its opaque token on every re-entry.
- **I5-11 -- Sessionless edge.** Python retains only a lifespan channel/session transport and
  bounded in-flight cleanup records; no durable user/workflow/resource state exists there.
- **I5-12 -- Daemon resource authority.** Public handles are daemon-minted and per-operation checked;
  internal lease tokens and storage paths never cross the wire.
- **I5-13 -- Bounded streaming.** Rust keeps query execution streaming and seals bounded independent
  Arrow pages; Python materializes at most one bounded presentation response/resource unit.
- **I5-14 -- Modern-only MCP.** Protocol `2026-07-28` is the sole product era; older eras fail before
  business dispatch and no compatibility bridge participates.
- **I5-15 -- Fixed public surface.** Exactly four tools, two resource families, and one authorized
  reference-completion handler are live; no prompts or rejected FastMCP components are registered.
  The application extension registry and UI component catalogs are empty. Modern discovery contains
  only the exact empty framework-owned `io.modelcontextprotocol/ui` advertisement and no other
  extension identifier or settings.
- **I5-16 -- Presentation-only schemas.** Strict Pydantic DTOs describe public presentation; generated
  Protobuf types stay inside the daemon adapter and do not become public tool models.
- **I5-17 -- Explicit cancellation.** Host intent after acceptance reaches `CancelQuery` once under a
  cleanup budget; watch loss and transport reconnect do not alter work authority.
- **I5-18 -- Safe completion.** Completion is capped, live, authorized, non-enumerating, advisory, and
  revalidated by the resource operation.
- **I5-19 -- Redacted observability.** Correlation IDs remain observational and distinct from semantic,
  query, challenge, epoch, package, resource, session, and lease identities.
- **I5-20 -- STDOUT purity.** Direct inherited STDIO carries MCP bytes only; diagnostics and telemetry
  use bounded STDERR/configured sinks.
- **I5-21 -- Physical decommission.** Replaced dependencies, state, schemas, tests, rules, recipes,
  package payload, and runtime fallbacks reach zero state after target consumers pass.
- **I5-22 -- Thin operational binaries.** The two root binaries select commands/ports and delegate to
  library authority; no semantic registry, schema generator, or hidden default lives in them.
- **I5-23 -- Measured resource posture.** One channel per adapter, one daemon for all agents, bounded
  candidates/pages/rounds/queues, and representative startup/RSS/latency/fairness evidence are
  required before release tuning.
- **I5-24 -- One trusted terminal HEAD.** Retained v4 substrate, all v5 packets, FreshActivation,
  post-purge packages, four domains, and independent review agree at one candidate commit.

The v5 packet DAG is deliberately linear at the public-boundary seam because the dirty startup,
proto, generated Python, adapter, recipes, and real-process tests overlap:

```text
WP43 -> WP44 -> WP45 -> WP46 -> WP47 -> WP48 -> WP49 -> WP50 -> WP51 -> WP52
```

Within a packet, disjoint documentation, Rust, Python, and expectation work may proceed concurrently
only after path reservation. Shared generated outputs, locks, proto sources, `justfile`, Cargo
manifests, process endpoints, and target directories force serialization.

## 4. Dependency-closed work packets

### WP43 — Issue v2.3 authority and independent FastMCP 4 expectations

**Outcome.** One synchronized v2.3 suite records the accepted modern-only presentation boundary,
and one independently reviewed expectation release defines the protocol, catalog, guarded-input,
atomic-start, completion, resource, cancellation, security, performance-method, and zero-state
truth that later code must satisfy. Production output never authors its own expected values.

**Dependencies.** None. Activation preflight must confirm the v4 retained proving commits remain
ancestral, but they are not v5 packet dependencies or migrated state.

**Target invariants.** I5-01--I5-24.

**Design and library references.** FastMCP 4 design §§0--18; daemon review v5 §§0--7; current SUITE
governance/release sections; current QRY typed request/admission sections;
current SRV package/catalog/resource/lifecycle/release sections; RM §0; principles P1--P5,
P9--P10, P13--P14, P18--P22, P25--P31, P36; LD5-01--LD5-06.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
just authoritative-design-conformance-check
just spec-outline docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.2.md --match '^(0|4|8|9|10|11|12|13)[.]'
just spec-outline docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md --match 'FastMCP|protocol|prompt|resource|query|release'
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' '2[.]2[.]0|v2[.]2|fastmcp==3[.]4[.]7|MCP 1[.]29|prompt|InputRequired|2026-07-28' docs/authoritative_design docs/spec_index contracts tooling/ci justfile AGENTS.md
```

Known touch is the eight new v2.3 suite files, derived spec indexes, an independently owned
`contracts/acceptance/relational-fabric-v5/` expectation/negative-fixture release, focused review
and validation tooling, artifact-contract tests, `AGENTS.md`, `justfile`, and CI. V2.2 and earlier
artifacts are read-only.

**Required changes.**

1. Allocate all eight synchronized successor files with predecessor links and a unique terminal
   suite identity. Change SUITE, SRV, QRY, and RM substantively; carry ONT, GEN, FAB, and LIFE
   forward unless review identifies a domain-owned invariant rather than serving restatement.
2. In SRV, freeze the exact FastMCP/MCP/Pydantic profile, modern-era admission, fd3 factory,
   hidden daemon port, four tools, two resources, no prompts, guarded input, daemon public handles,
   authorized completion, cancellation/reconnect, telemetry redaction, bounds, and rejected-feature
   list.
3. In QRY, freeze one `StartQueryOutcome` sum type, typed challenge/continuation semantics, pure
   validation, idempotency/replay rules, resource-handle projection, and completion-authority input.
4. In SUITE/RM, order startup repair before wire change, target consumer before purge, evidence
   before deletion, performance after purge, and FreshActivation before certification. State that
   v4 WP37--WP40/WP42 are superseded rather than silently relabeled.
5. Update `docs/spec_index/library-routing.md` to route FastMCP 4 and only then refresh other derived
   indexes. Make the current-suite selector derive the v2.3 terminal identity without mutating an
   ancestor's issuance-time bytes.
6. Issue expected clauses and fixtures independently from production imports for modern/legacy
   negotiation, exact catalog/schema, all guard legs, atomic start, completion allow/deny, resource
   authorization, cancellation/reconnect, two-agent isolation, secret redaction, and every
   decommission target.
7. Pre-register the performance workload, environment record, minimal FastMCP 4 control, measured
   dimensions, sample/distribution rules, and candidate-neutral budget source. Do not invent a
   threshold from the implementation result or permit a later local relaxation.
8. Obtain an independent review of suite causality, expectation independence, negative-fixture
   discrimination, and the absence of history/comparator authority.

**Legacy disposition and decommission.** V2.2 and all earlier suites/evidence remain immutable,
non-live history. FastMCP 3 runtime/dependency/schema/prompt/gate residue becomes DB15--DB17 scope;
nothing live is deleted in this packet.

**Acceptance checks.**

Executable oracle: `fastmcp4-successor-authority-integrity-check`
Governed criterion: `PC-WP43-INT`

Executable oracle: `fastmcp4-independent-expectation-review-check`
Governed criterion: `PC-WP43-BEH`

Executable oracle: `fastmcp4-negative-fixture-independence-check`
Governed criterion: `PC-WP43-NEG`

Executable oracle: `fastmcp4-expectation-drift-check`
Governed criterion: `PC-WP43-OPS`

**Oracle category fault contract.** `INT` corrupts suite identity, membership, predecessor linkage,
or a version/pin clause; `BEH` changes a controlled semantic/protocol input and requires the
independently authored decoded expectation to distinguish it; `NEG` imports a production expected
value or allows a forbidden authority; `OPS` drifts one frozen input/selector/performance method and
requires dependent execution to stop rather than restamp it.

**Edit-local gates.** Run `just spec-outline` for every changed authority artifact, targeted
artifact-contract and expectation-tool tests, `just authoritative-design-conformance-check`,
`just artifacts-check`, and targeted `typos`. Prove all v2.2 bytes unchanged.

**Packet-local gates.** Add and run the four packet recipes above. Each records nonzero selected
cases, the independent reviewer identity, its committed fault, and v2.3 sole-current selection.

**Integration milestone.** Completes M08 and opens design-bearing code work.

**Replan triggers.** Stop if `2.3.0` collides before issuance, synchronized versioning cannot express
the successor, any required host cannot support `2026-07-28` guarded input, modern admission cannot
precede business dispatch through a supported FastMCP hook, or independent expected values cannot
be authored without target imports.

**Rollback and recovery.** Before selection, remove only unselected new candidate artifacts and
leave v2.2 current. After selection, correct forward with another suite version; never rewrite v2.3
or reactivate v2.2 serving behavior.

### WP44 — Reconcile startup work and close exact fresh-activation readiness

**Outcome.** The reusable dirty-tree supervisor/daemon foundation becomes one owned, tested target
startup path. A truly empty workspace reaches `Ready` through exact command-actor genesis/readback;
an unknown outcome reconciles from durable evidence or remains failed-closed. WP29/WP30 composition
and decommission outcomes pass again without restoring any ontology/bootstrap/model authority.

**Dependencies.** WP43.

**Target invariants.** I5-01--I5-07, I5-19--I5-22, I5-24.

**Design and library references.** Daemon review v5 startup/supervisor/session/UDS sections;
successor SUITE/LIFE/FAB/RM; principles P3--P5, P9--P10,
P13, P18--P19, P25--P31, P36; Tonic UDS, peer-credential, Tokio lifecycle, cancellation, and
shutdown chapters.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
git diff --name-status -- src tests contracts/rpc tooling/proto codefabric-cpg-mcp justfile Cargo.toml Cargo.lock
ast-grep outline src/bin src/supervisor.rs src/process_runtime.rs src/owned_unix_socket.rs src/session_authority.rs src/daemon.rs src/fabric/activation_control_delta.rs src/fabric/activation_transaction.rs src/fabric | sed -n '1,320p'
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ReadbackUnavailable|AwaitingReconciliation|ExpectedHead|Ready|serve_programmatic|bootstrap|ontology|model|schema generator|src/bin' src tests justfile Cargo.toml
just lifecycle-production-vertical-check
```

Known touch includes the listed Rust startup/lifecycle modules, activation command/readback ports,
`src/bin/codefabric.rs`, `src/bin/codefabricd.rs`, Cargo targets/features, integration process
fixtures, supervisor recipes, and the WP29/WP30 gate definitions. The expected current vertical
failure is diagnostic input, not a baseline to preserve.

**Required changes.**

1. Attribute every dirty target path before editing. Retain target-conforming supervisor, owned
   socket, process runtime, session, fd3 launcher, generated-v2, and test work; preserve unrelated
   changes and record discarded approaches rather than replacing whole files.
2. Repair the fresh-start activation path through the existing recovered command actor. On an
   uncertain append, probe by durable command/transaction identity, read one coherent exact event,
   table-version vector, fence, and control horizon, then reconstruct or remain
   `AwaitingReconciliation`/failed-closed. Do not seed a head, retry append blindly, combine latest
   values, trust a receipt, or introduce a digest as proof.
3. Make `ProductionStartupCoordinator` and one lifecycle projection decisive for discovery,
   bootstrapping, health/status, admission, drain, shutdown, and restart. Liveness may be healthy
   during bootstrapping, but semantic operations remain closed until exact active authority exists.
4. Finish the target supervisor singleton, private runtime root, owned rendezvous/query endpoints,
   daemon control hello, bounded restart generation, process-group drain, joined cleanup, stale
   recovery, and peer/generation propagation required before the public contract changes.
5. Keep `src/bin/codefabric.rs` as the thin administrative/supervisor/attach command shell and
   `src/bin/codefabricd.rs` as the thin authenticated daemon bootstrap. Move launch bounds, policy,
   semantic profile, workspace settings, and defaults into strict library-owned typed contracts;
   reject static schema/model generation and ad hoc semantic construction in either binary.
6. Decouple the retained WP29/WP30 recipes from the unfinished FastMCP vertical. Revalidate honest
   programmatic production composition, lifecycle, compiled-release consumption, model-free
   restart, and bootstrap/ontology/model zero state at the Rust process boundary.
7. Replace empty/deleted selectors with target startup cases where they belong, but do not claim
   the later wire/FastMCP surface. Preserve each gate's nonzero-selection and fault contract.

**Legacy disposition and decommission.** Retain only target lifecycle, writer/fence,
reconciliation, supervisor, owned-socket, session, and process primitives behind target-owned APIs.
Ontology/bootstrap/model/static-schema execution stays absent. Historical v4 serving tests may
remain non-live until WP49 only when no recipe, package, rule, or production selector reaches them.

**Acceptance checks.**

Executable oracle: `fastmcp4-startup-contract-integrity-check`
Governed criterion: `PC-WP44-INT`

Executable oracle: `fresh-activation-ready-reconciliation-check`
Governed criterion: `PC-WP44-BEH`

Executable oracle: `supervisor-startup-boundary-rejection-check`
Governed criterion: `PC-WP44-NEG`

Executable oracle: `supervisor-restart-join-operations-check`
Governed criterion: `PC-WP44-OPS`

**Oracle category fault contract.** `INT` corrupts phase/command/readback or binary-to-library port
contracts; `BEH` starts from no activation head and reaches ready only after decoded exact readback;
`NEG` covers direct daemon start, unsafe/replaced sockets, unauthorized attach, seed/latest/hash/
receipt substitution, and restored model authority; `OPS` faults append acknowledgement, readback,
child readiness, restart, drain, and replacement-inode cleanup.

**Edit-local gates.** Run targeted Rust unit/integration tests, `just root-fmt`,
`just root-check-fast`, `just root-check`, `just supervisor-launch-contract-check`, all retained
WP29/WP30 zero-state checks, and structural rules for production composition and owned sockets.

**Packet-local gates.** Add and run the four packet recipes plus the reusable
`supervisor-launch-platform-check` component recipe. The positive and operations gates use real
`codefabric`/`codefabricd` subprocesses and exact durable activation state; injected services may
exercise lower layers but cannot satisfy packet completion.

**Integration milestone.** Advances M09; WP45 completes that milestone.

**Replan triggers.** Stop if one coherent durable read cannot recover the exact activation horizon,
if readiness needs a second mutable owner, if target startup requires a predecessor model/catalog,
if safe operator-owned launch policy cannot be provided, or if supported-platform descriptor/socket
ownership cannot be implemented without first-party unsafe code.

**Rollback and recovery.** Before append, discard private candidates and keep admission closed.
After an unknown result, reconcile or fail closed. Startup failure revokes unused grants, joins
children, and removes only owned endpoints. After target state exists, repair forward through the
target command path; never restore bootstrap authority.

### WP45 — Revise the unreleased gRPC v2 daemon contract and authority

**Outcome.** One generated `codefabric.cpgd.v2` service exposes atomic start outcomes, typed input
challenges, daemon-minted public resource handles, authorized reference completion projection, and
explicit cancellation over the existing owned UDS/session boundary. Source proto, descriptor IR,
Rust/Python bindings, service/client code, and interoperability fixtures change as one transaction;
there is no v3 compatibility service for an unshipped v2 contract.

**Dependencies.** WP44.

**Target invariants.** I5-03--I5-13, I5-16--I5-19, I5-24.

**Design and library references.** FastMCP 4 design §§5--8, §§10--11, §§14--17; daemon review v5
wire/session/resource/error sections; successor
QRY/SRV/LIFE; Tonic §§3--8, 12--29, 34--40, 43; grpcio §§8--10, 13--19, 21, 23, 26--30, 35--37;
protobuf §§4--18, 26--30, 37--44; LD5-06.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline contracts/rpc tooling/proto src/query_service.rs src/session_authority.rs src/fabric/child_session codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon | sed -n '1,360p'
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'ValidateQuery|StartQuery|ReadResource|ReleaseResource|GetReference|CancelQuery|ResourceLease|session|cursor|freshness_policy' contracts/rpc tooling/proto src codefabric-cpg-mcp tests
just proto-check
just proto-repro-check
```

Known touch is the v2 proto, descriptor/census/baseline policy, hermetic generator, generated Rust
and Python files, RPC exports, query service/coordinator, challenge and resource registries, session
authorization, adapter daemon port, and exact Rust/Python wire fixtures.

**Required changes.**

1. Replace the live v2 `StartQueryResponse` with a closed Protobuf `oneof` for accepted query,
   input challenge, and validation rejection. Absence or an unknown variant fails closed. Preserve
   a separate `ValidateQuery` request/response as pure preparation data with no acceptance,
   challenge side effect, capacity reservation, task, or resource creation.
2. Define bounded typed challenge messages: stable semantic field IDs, allowlisted input kinds,
   strict scalar/enum/collection constraints, safe presentation keys, optional authorized choices,
   opaque daemon continuation bytes, expiry, challenge/round identity, remaining rounds, and a safe
   explanation code. Reject opaque JSON, `Any`, prose-parsed constraints, or adapter-authored
   semantic defaults.
3. Bind the daemon continuation to principal, workspace, semantic request identity and immutable
   normalized fields, session/daemon generation, challenge/round, issue/expiry, policy/revocation
   generation, allowed answer shape, and original start idempotency identity. Valid continuation
   can accept exactly once; replay/tamper/cross-context use fails without creating work.
4. Add daemon-minted public resource handles distinct from internal lease tokens. The authoritative
   record binds package/resource identity, workspace, owner or sharing class, policy/revocation and
   daemon generation, expiry, allowed selectors/ranges, and release state. Read/release reauthorize
   every request; restart invalidates stale-generation handles and recovery can mint a new handle
   only from a retained authorized package.
5. Extend the typed `GetReference` authority with a bounded completion projection for approved
   reference-template variables. It accepts prefix/selector/cap, returns only authorized safe values
   plus bounded `total`/`has_more` metadata, and leaks no denied existence. Do not create a generic
   search/enumeration RPC.
6. Wire explicit `CancelQuery` through coordinator authority with one idempotent cancellation
   identity and typed acknowledgement/terminal observation. Reserve control capacity so cancel,
   release, handshake, and status cannot be starved by result streams.
7. Preserve one relative remaining-budget model, bounded messages/pages, standard status plus a
   small allowlisted typed detail/trailing-metadata code, verified peer identity in request
   extensions, and per-method/session/object authorization. Never parse prose or expose tokens.
8. Regenerate descriptor IR and both languages hermetically, update exact census/history treatment,
   and prove binary interoperability and unknown-field/oneof behavior. Preserve the displaced v2
   descriptor only as explicitly non-live issuance evidence if governance requires it; do not make
   it a runtime compatibility baseline.

**Legacy disposition and decommission.** The old normal-path Validate-then-Start flow, Python-
generated handles, secret lease projection, repeated top-level freshness, public attempt IDs,
overlapping resource routes, and any v1 runtime service become DB15/DB16 deletion targets. The v2
package name, generated transport, owned UDS, session model, bounded streams, and exact result
packages are retained and revised.

**Acceptance checks.**

Executable oracle: `fastmcp4-daemon-wire-contract-check`
Governed criterion: `PC-WP45-INT`

Executable oracle: `fastmcp4-atomic-start-check`
Governed criterion: `PC-WP45-BEH`

Executable oracle: `fastmcp4-resource-authority-check`
Governed criterion: `PC-WP45-NEG`

Executable oracle: `fastmcp4-daemon-security-recovery-check`
Governed criterion: `PC-WP45-OPS`

**Oracle category fault contract.** `INT` corrupts the v2 source/descriptor/generated oneof,
challenge, resource, completion, cancel, session, or error contract; `BEH` races validation state and
proves one atomic start outcome/acceptance; `NEG` covers cross-agent/workspace/generation public
handle reads, secret/path exposure, overrange reads, release replay, and Python-minted handles;
`OPS` faults cancellation, transport loss, session renewal, restart handle invalidation, challenge
expiry, and reserved-control admission.

**Edit-local gates.** Run focused service/coordinator/resource/session tests, `just root-fmt`,
`just root-check`, `just root-test-rust`, `just proto-check`, `just proto-repro-check`, adapter proto
tests, descriptor assertions, and targeted generated-file diff inspection after the deliberate
`just proto-gen` transaction.

**Packet-local gates.** Add and run the four packet recipes. Each must execute generated Rust and
Python clients against the real Tonic service over UDS; direct service tests supplement but do not
replace interop. Prove the old ordinary two-call path is rejected structurally and behaviorally.

**Integration milestone.** Completes M09 with WP44 and opens the FastMCP 4 cell.

**Replan triggers.** Stop if the closed outcome or challenge requires untyped JSON/`Any`, if a public
handle cannot eliminate Python secret state without weakening per-read authorization, if safe
completion cannot reuse live reference authority without existence leakage, if peer identity cannot
reach async authorization, or if generated v2 evolution cannot remain one coherent target-only
transaction.

**Rollback and recovery.** Until all generated/service/client surfaces agree, do not select or
package the candidate descriptor. Failed starts create no accepted work; ambiguous accepted work is
resolved only by daemon idempotency/query identity. Old clients fail clearly; no compatibility
service is restored.

### WP46 — Implement the modern-only FastMCP 4 presentation cell

**Outcome.** The locked adapter constructs one FastMCP 4 server only after verified fd3 settings,
uses a lifespan-owned daemon port and request-local context, publishes exactly the accepted fixed
surface, translates daemon challenges through native guarded input, and contains no independent
semantic/session/task/cache/resource authority. It operates only with the camel-case compatibility
bridge disabled.

**Dependencies.** WP45.

**Target invariants.** I5-08--I5-20, I5-22--I5-24.

**Design and library references.** FastMCP 4 design §§4--13 and §§16--17; successor SRV/QRY;
FastMCP reference construction, tools, resources,
completion, context, dependency injection, request state, middleware, telemetry, transport, testing,
and 3-to-4 migration chapters; Pydantic strictness, aliases, validators/serializers, TypeAdapter,
JSON Schema, error, and performance chapters; grpcio async channel/deadline/cancellation chapters;
LD5-01--LD5-05.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
ast-grep outline codefabric-cpg-mcp/src/codefabric_cpg_mcp codefabric-cpg-mcp/tests | sed -n '1,360p'
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'FastMCP|CurrentContext|ResourceLease|_resource_leases|freshness|mcp_call_id|rpc_attempt_id|ValidateQuery|prompt|session|task|cache|camel' codefabric-cpg-mcp contracts/adapter rules justfile
uv run --frozen --project codefabric-cpg-mcp python -c 'from importlib.metadata import version; print(version("fastmcp"), version("mcp"), version("pydantic"))'
just adapter-ci-fast
```

Known touch includes adapter metadata/lock, entry points, `server.py`, `settings.py`, daemon port,
public models, contract helpers, package resources, tests, live inspection/schema fixtures,
structural rules, adapter recipes, and CI. The current FastMCP 3 test success proves only predecessor
coherence.

**Required changes.**

1. Resolve and lock exactly FastMCP 4.0.0, direct `mcp==2.1.1`, Pydantic 2.13.4, and required
   grpcio/protobuf pins. Do not add direct `fastmcp-slim`, `fastmcp-tasks`, Docket, Redis, `httpx2`,
   or `pydantic-settings` without a separately accepted direct consumer.
2. Use one public protocol import convention and snake-case SDK attributes. Set
   `FASTMCP_MCP_CAMELCASE_COMPAT=false` in every lint/type/test/inspect/wheel/STDIO/release gate so a
   hidden compatibility dependency cannot pass.
3. Replace import-time server publication with a pure application factory invoked after the strict
   fd3 launch descriptor is parsed and validated. Keep fd3 settings in strict Pydantic boundary
   models, not a second settings framework or environment-authored semantic defaults.
4. Define an application-owned typed `DaemonPort`; its production implementation owns one
   lifespan `grpc.aio` channel, stub, and current daemon session. Generated Protobuf types end in
   that module. Inject the port as a hidden dependency; use `CurrentContext` only for MCP request
   identity, progress, cancellation, guard answers, and sealed request state.
5. Construct FastMCP with explicit fail-closed policy including `tasks=False`. Register exactly
   `query_code_graph`, `validate_code_graph_query`, `get_code_graph_status`, and
   `get_code_graph_reference`; exactly result-page/manifest and authorized-reference resource
   families; one narrow reference-template completion handler; and no prompts or rejected
   application components. Register no `ServerExtension` or App/UI component. Observe separately
   that the application extension registry is empty while modern live discovery contains exactly
   FastMCP 4.0.0's unavoidable empty `io.modelcontextprotocol/ui` advertisement and no other
   extension identifier or settings.
6. Implement an ordered operation-aware middleware stack: modern protocol admission, safe
   request/correlation span enrichment, bounded deadline/cancellation projection, and public-safe
   error redaction. Recheck the modern era before operation dispatch; initialize, cancellation,
   completion, and notifications must not enter tool counters or semantic policy.
7. Map the daemon `InputRequired` variant to native `InputRequiredResult`. Use supported
   `RequestStateSecurity` with a process-ephemeral key and a TTL no longer than the daemon challenge
   and launch/session bound. Place only the opaque daemon token plus presentation metadata in sealed
   state; on re-entry pass unsealed state and `ctx.input_responses` back to the daemon without
   interpreting semantic answers.
8. Keep `validate_code_graph_query` pure and return typed preparation information as data. Remove
   the ordinary Validate-then-Start call sequence. `query_code_graph` issues one atomic start per
   idempotency identity and maps the three closed outcomes without accept/cancel compensation.
9. Represent result/reference URIs with daemon public handles and bounded selectors only. Forward
   reads/releases directly; retain no Python handle map, secret lease, filesystem path, package
   registry, or Arrow decoder. Materialize at most one bounded independently decodable page or
   projection per resource call.
10. Implement authorized reference completion by calling the daemon completion projection, applying
    only the protocol cap, and returning advisory values/`total`/`has_more` without local inventory.
    The subsequent resource call performs independent daemon authorization.
11. Translate host cancellation after acceptance into one daemon `CancelQuery` under a separate
    cleanup budget, await the configured acknowledgement/terminal observation, record a redacted
    outcome, and re-raise cancellation. Before acceptance, end the attempt without daemon work.
12. Remove public/random MCP call and RPC attempt IDs; use the framework request ID for leg
    correlation. Keep semantic request, daemon query, challenge, epoch, package, resource, session,
    and lease identities distinct. Allowlist telemetry attributes and route all exporters/
    diagnostics away from STDOUT.
13. Keep strict/frozen/extra-forbid public models and module-scoped adapters. Treat live JSON Schema,
    FastMCP inspection, and fingerprints as observations checked against WP43 expectations, never
    coequal static authority.

**Legacy disposition and decommission.** The target consumer for DB15/DB16 lands here. FastMCP 3
imports/pins, global-server assumptions, compatibility spelling, duplicate freshness/IDs,
Validate-before-Start, Python resource leases, static schemas, phantom prompts, and rejected
framework components may remain only long enough to compare/remove within this packet; none may be
selected by its packet oracles. WP49 proves repository-wide zero state.

**Acceptance checks.**

Executable oracle: `fastmcp4-dependency-contract-check`
Governed criterion: `PC-WP46-INT`

Executable oracle: `fastmcp4-modern-protocol-check`
Governed criterion: `PC-WP46-BEH`

Executable oracle: `fastmcp4-adapter-authority-zero-state-check`
Governed criterion: `PC-WP46-NEG`

Executable oracle: `fastmcp4-public-surface-check`
Governed criterion: `PC-WP46-OPS`

**Oracle category fault contract.** `INT` restores an old/transitive pin, mixed protocol import, or
bridge dependency; `BEH` operates with `2026-07-28` and proves every older era fails before a daemon
business call; `NEG` adds Python semantic/session/task/cache/resource authority or Arrow processing;
`OPS` inspects fresh/reconstructed installed servers and distinguishes any extra/missing tool,
resource, completion, prompt, application extension, task, provider, transform, or UI component;
it also distinguishes drift from the one allowed empty framework-owned UI advertisement.

**Edit-local gates.** Run locked dependency/import probes, adapter Ruff format/lint, Pyright/type
checks, focused pytest, live FastMCP client/inspection, bridge-off schema assertions, STDIO silence,
wheel build/import, and targeted structural rules. Run `just adapter-lint`, `just adapter-type`,
`just adapter-test`, `just adapter-stdio-test`, and `just adapter-wheel-test` as the implementations
become target-aware.

**Packet-local gates.** Add and run the four packet recipes. Also run component recipes
`fastmcp4-guard-roundtrip-check` and `fastmcp4-completion-authorization-check` against a generated
daemon port; WP47 repeats them through real processes.

**Integration milestone.** Advances M10; WP47 completes it.

**Replan triggers.** Stop if FastMCP cannot reject old protocol eras before business dispatch using
a stable supported hook, guarded continuation cannot preserve strict schema/cancellation/STDIO
behavior, the exact stack cannot run bridge-off on Python 3.14.7, or the daemon port cannot contain
generated types without duplicating semantic authority.

**Rollback and recovery.** A partially upgraded lock/server is not packaged or selected. Adapter
startup failure closes the channel, emits bounded STDERR diagnostics, and exits without starting a
daemon. Adapter restart invalidates unfinished request-state guards but cannot lose accepted work;
the daemon remains authority.

### WP47 — Prove the real modern supervisor-to-FastMCP vertical

**Outcome.** The real supervisor, installed `codefabricd`, attach-only launcher, installed FastMCP 4
wheel, and a modern MCP host execute the complete guarded-query/resource/completion/cancellation
path. Two concurrent agents share one daemon while retaining isolated grants, sessions, challenges,
queries, resources, and cleanup. Restart resumes observation without resubmitting accepted work.

**Dependencies.** WP46.

**Target invariants.** I5-01--I5-24.

**Design and library references.** FastMCP 4 design §§5--13 and §16; successor
SUITE/FAB/QRY/LIFE/SRV; daemon review production-vertical and fault sections; FastMCP
client/testing/fingerprinting/STDIO chapters; grpcio async/deadline/cancellation/reconnect chapters;
Tonic UDS/flow-control/shutdown chapters; LD5-01--LD5-06.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'wp37|production_server_entry|ProbeService|fake daemon|stub daemon|supervisor|codefabricd|fd 3|fastmcp|stdio|CancelQuery|completion|InputRequired' tests codefabric-cpg-mcp tooling justfile scripts
ast-grep outline tests/integration codefabric-cpg-mcp/tests src/supervisor.rs src/query_service.rs | sed -n '1,360p'
just supervisor-launch-contract-check
just adapter-stdio-test
just lifecycle-production-vertical-check
```

Known touch includes the unified Rust integration target, real-workspace/source fixtures, process
harness, supervisor/launcher/service tests, installed adapter loader, modern acceptance client,
STDIO capture, fault injection, recipes, CI service permissions, and bounded temporary runtime
roots.

**Required changes.**

1. Launch only production binaries and the installed wheel: operator policy -> supervisor singleton
   -> `codefabricd` control hello -> private query UDS -> attach-only launcher -> one-shot fd3 grant
   -> direct inherited STDIO -> modern client. No in-process FastMCP server, Rust `ProbeService`,
   Python fake daemon, direct factory, or source-tree import may satisfy the positive oracle.
2. Mutate a real workspace source and drive exact providers/gaps, typed transformations and proof,
   Delta activation, atomic workspace publication, scheduled DataFusion execution, independently
   decodable Arrow result pages, daemon v2, and MCP presentation. Decode and compare semantic values
   with WP43 expectations rather than transport fingerprints.
3. Exercise zero, one, and maximum bounded guard rounds. Prove no accepted work exists before
   complete input, every leg re-enters middleware/daemon authorization, valid answers accept once,
   and abandon/tamper/expiry/replay/wrong args/principal/workspace/generation/excess rounds fail.
4. Exercise both resource families through daemon public handles: bounded page/projection reads,
   per-read authorization, release, expiry, revocation, cross-agent/workspace denial, daemon
   generation change, and authorized reissue from a retained package. Assert no path/secret/token
   leaks.
5. Exercise reference completion for allowed kind/version selectors with cap/`total`/`has_more` and
   eventual-operation revalidation. Prove denied/unsupported selectors, entity/source inventories,
   result handles, and repository paths reveal nothing.
6. Cancel before acceptance and after acceptance at queue, execution, watch, page-seal, and resource
   presentation phases. Prove exactly one explicit daemon cancel after acceptance, bounded cleanup,
   terminal observation, and no accidental cancellation from merely dropping a watch.
7. Break the channel after acceptance, recreate one channel/session, resume the watch by daemon
   query/cursor, and prove `StartQuery` was not resubmitted. Advance daemon generation and deliver a
   replacement single-use grant before re-handshake.
8. Run two concurrent launcher/adapter/client cells against one daemon PID/workspace. Verify
   independent principal/grant/session/challenge/query/resource scopes, fair bounded progress, one
   adapter exit/revocation without unrelated cancellation, and joined cleanup of all children and
   endpoints.
9. Capture STDOUT byte-for-byte as MCP framing only. Bound STDERR and inspect logs/spans/errors for
   safe codes/correlation only, with no payload, answer, source, path, metadata, request state,
   launch/session/challenge/resource secret, or stack trace.

**Legacy disposition and decommission.** This packet establishes every target consumer needed to
delete the FastMCP 3 serving vertical and stale WP37 shortcuts. Lower-layer fakes remain only for
unit tests with explicit scope; no fake, direct daemon start, v1 service, source-tree adapter, or
pre-modern client remains in a release/certification recipe.

**Acceptance checks.**

Executable oracle: `fastmcp4-contract-observation-check`
Governed criterion: `PC-WP47-INT`

Executable oracle: `fastmcp4-stdio-vertical-check`
Governed criterion: `PC-WP47-BEH`

Executable oracle: `fastmcp4-security-negative-check`
Governed criterion: `PC-WP47-NEG`

Executable oracle: `fastmcp4-cancellation-recovery-check`
Governed criterion: `PC-WP47-OPS`

**Oracle category fault contract.** `INT` changes a live tool/resource/completion/schema/protocol
clause without its independent expectation; `BEH` replaces any real process/library/domain step or
changes decoded results/guard acceptance; `NEG` covers wrong grant/session/challenge/handle/owner/
workspace/generation, legacy era, unsafe endpoint, enumeration, and secret/STDOUT leakage; `OPS`
faults cancellation, slow reader, channel loss, daemon restart, adapter exit, two-agent fairness,
and joined cleanup.

**Edit-local gates.** Run focused Rust/Python process cases, `just root-check`, `just adapter-ci-fast`,
`just proto-check`, `just supervisor-launch-contract-check`, platform launch checks, live wheel
inspection, and STDIO capture. Verify temporary roots are exact and cleanup is recoverable.

**Packet-local gates.** Run the four packet recipes plus
`just fastmcp4-guard-roundtrip-check`, `just fastmcp4-atomic-start-check`,
`just fastmcp4-resource-authority-check`, and
`just fastmcp4-completion-authorization-check`. The aggregate preserves child exits/counts/faults.

**Integration milestone.** Completes M10 and opens independent production evidence.

**Replan triggers.** Stop if the real host cannot support guarded input, safe completion leaks denied
existence, public handles cannot survive the required authorization model, cancellation cannot be
distinguished from watch loss, reconnect requires start resubmission, or per-agent isolation cannot
share one daemon without cross-principal state.

**Rollback and recovery.** Any failed vertical starts from a new bounded runtime root, revokes grants
and sessions, joins children, and removes only owned endpoints. Accepted work follows daemon recovery
policy; no failed adapter run promotes partial evidence or causes a fallback server to start.

### WP48 — Execute independent first-principles successor evidence

**Outcome.** An append-only evidence transaction, authored from WP43 expectations rather than
production observations, independently proves the complete FastMCP 4 source-to-presentation target,
causal discrimination, negative authority boundaries, and clean reconstruction. Historical design
agreement is explicitly unnecessary.

**Dependencies.** WP47.

**Target invariants.** I5-01--I5-24.

**Design and library references.** Successor SUITE proof/release sections; FAB/QRY/LIFE/SRV
evidence clauses; FastMCP 4 design §16; principles P2--P5, P10, P18--P20, P22,
P25--P31, P36; repository evidence policy and v4 retained evidence boundaries.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'WP38|first-principles|expected|fixture|causal|clean reconstruction|history|comparator|fastmcp' contracts/acceptance tooling/ci tests codefabric-cpg-mcp/tests justfile docs/reviews
ast-grep outline tooling/ci tests/integration codefabric-cpg-mcp/tests | sed -n '1,360p'
just first-principles-production-behavior-check
just causal-fault-discrimination-check
```

Known touch includes the v5 acceptance runner, append-only evidence artifacts, causal-fault fixtures,
clean-build/runtime harness, independent review, recipes, CI, and only those production hooks needed
for deterministic fault injection/observation.

**Required changes.**

1. Bind evidence to the exact v2.3 suite, v5 plan, WP43 expectation release/review, target lock/
   descriptor identities, candidate commit, environment, and real package identities. Identity
   establishes provenance, not behavioral success.
2. Execute every independently issued positive clause through the real installed topology and
   decode provider rows/gaps, transformation/proof outputs, exact activation, all eight query forms,
   guarded input, atomic acceptance, status/reference, resources, completion, cancellation,
   reconnect, restart, and two-agent isolation.
3. Execute negative fixtures for missing capability, ambiguous producer, invalid phase/horizon,
   unsafe socket, wrong grant/session/challenge/handle context, legacy era, forbidden FastMCP
   components, Python authority, secret leakage, unbounded values, and restored predecessor paths.
4. Inject one causal fault per claimed layer and require the corresponding independent expected
   value or rejection to fail: provider row, transformation, DataFusion plan/schema, Delta selection,
   activation readback, start variant, guard token, resource authorization, completion filtering,
   cancellation, and MCP projection.
5. Reconstruct from a clean build/package/runtime root with no predecessor model, generated adapter
   schema, stale descriptor, source-tree adapter import, cached current epoch, or prior evidence
   output. Repeat a representative source mutation and restart.
6. Obtain independent implementation/evidence review of causality, fixture independence, real-
   process coverage, exclusions, skipped candidates, fault discrimination, and limitations. A
   predecessor comparator may be discussed only as historical context and cannot affect verdict.

**Legacy disposition and decommission.** Target evidence replaces invalidated v4 WP38 live
expectations/runners. Preserve old evidence as immutable history only; WP49 removes any live selector,
generated expectation, package resource, or certification dependency on it.

**Acceptance checks.**

Executable oracle: `fastmcp4-production-evidence-integrity-check`
Governed criterion: `PC-WP48-INT`

Executable oracle: `fastmcp4-production-behavior-check`
Governed criterion: `PC-WP48-BEH`

Executable oracle: `fastmcp4-causal-fault-check`
Governed criterion: `PC-WP48-NEG`

Executable oracle: `fastmcp4-clean-reconstruction-check`
Governed criterion: `PC-WP48-OPS`

**Oracle category fault contract.** `INT` drifts a frozen input/runner/selector or imports production
expected values; `BEH` changes a controlled decoded outcome through the real topology; `NEG`
activates each layer fault and proves the corresponding independent clause distinguishes it; `OPS`
removes caches/prior state/builds, restarts processes, and proves clean target-only reconstruction.

**Edit-local gates.** Run expectation/evidence-tool unit tests, targeted process tests, artifact
schema checks, causal mutation controls, clean-root construction probes, and `just artifacts-check`.

**Packet-local gates.** Add and run the four packet recipes. Evidence records exact commands/exits,
selected counts, candidate/environment/resource context, faults, exclusions, and limitations in the
packet artifact rather than execution state.

**Integration milestone.** Advances M11 and opens physical purge.

**Replan triggers.** Stop if expected values depend on production output, a claimed layer cannot be
causally faulted/observed, the real topology must be replaced by a fake, clean reconstruction needs
predecessor state, or any skipped/unparsed candidate can affect a verdict.

**Rollback and recovery.** Failed evidence does not modify production authority. Correct the target
or expectation design forward, issue a new append-only evidence attempt, and retain failed attempts
with limitations; never restamp or delete an unfavorable result.

### WP49 — Purge FastMCP 3, duplicate adapter authority, and stale live surfaces

**Outcome.** After every FastMCP 4 target consumer and independent evidence path passes, all
displaced serving, adapter-authority, static-schema, package, gate, and historical live reachability
is physically removed. The retained repository exposes only the modern target and the two justified
thin operational binaries.

**Dependencies.** WP48.

**Target invariants.** I5-01--I5-02, I5-08--I5-24.

**Design and library references.** FastMCP 4 design §§9, §§14--16; successor SUITE/SRV/RM
zero-state clauses; principles P3--P5, P18--P22, P25--P31, P36;
repository package/feature/evidence doctrine.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' -g '!docs/authoritative_design/*v2.2*' 'fastmcp.?3|3[.]4[.]7|mcp.?1[.]29|CAMELCASE_COMPAT|ctx[.]elicit|_resource_leases|ResourceLease|mcp_call_id|rpc_attempt_id|ValidateQuery.*StartQuery|pydantic-settings|test_arrow_resources|prompt|TasksExtension|UserSession|SessionProvider|Docket|contracts/adapter' .
rg --files src/bin | sort
ast-grep outline src/bin codefabric-cpg-mcp/src/codefabric_cpg_mcp rules tooling/ci | sed -n '1,360p'
just remaining-legacy-zero-state-check
```

Known touch includes adapter code/tests/lock/wheel resources, `contracts/adapter/**`, old evidence and
proto runtime outputs, structural rules, recipes/workflows, package/service configuration,
administrative scripts, generated registries, and both production binaries.

**Required changes.**

1. Remove FastMCP 3/MCP 1.x pins and lock entries, camel-case Python compatibility use, mixed model
   imports, old constructor/decorator assumptions, pre-modern functional cases, `ctx.elicit()`
   fallback, and any live v1/v3 alternate service/client.
2. Remove ordinary Validate-before-Start, presentation/RPC freshness duplicated outside the
   canonical semantic request, public/random MCP/RPC attempt IDs, Python-generated public handles,
   `_resource_leases`, secret lease projection, and any adapter request rewriting beyond strict DTO
   mapping.
3. Remove live static adapter schema files/fingerprints as authority or package payload, phantom
   prompt declarations/tests, the unused direct `pydantic-settings` requirement/import/configuration,
   deleted-test recipe references, stale
   FastMCP 3 AST rules, mock/direct-factory shortcuts from release gates, and invalidated WP37/WP38
   live selectors.
4. Remove direct/extra FastMCP session/task/auth/cache/application-extension/provider/transform/proxy/gateway
   dependencies, registration, storage, workers, configuration, and package extras. A transitive
   package needed by the exact FastMCP resolution is not application adoption, but no CodeFabric
   import or live registration may depend on the rejected capability. Prove `tasks=False`, an
   empty application extension registry, no task/UI components, and exactly the bounded inert
   framework advertisement from live modern discovery, not merely a source keyword.
5. Inspect Python identity/canonicalization modules and every consumer. Delete application-owned
   identity/canonical request semantics with no presentation consumer; retain only minimal strict
   boundary validation that cannot alter Rust-owned identity or meaning.
6. Remove stale generated gRPC runtime outputs/services/clients, compatibility baselines, fixtures,
   and package resources that can still be selected. Keep only explicitly governed non-live
   descriptor history and the sole generated v2 target.
7. Audit `src/bin/`. Retain exactly `codefabric.rs` and `codefabricd.rs` as thin operational shells
   if their real process consumers remain. Move any semantic settings/defaults into typed library
   policy; remove any static schema/model generation, registry emission, or ad hoc semantic
   execution from production binary reachability. Keep hermetic model/proto generators only in their
   explicitly feature-separated tooling paths when still required by repository governance.
8. Remove retired features, optional dependencies, exports, modules, recipes, workflows, services,
   rules, fixtures, installed entry points, wheel resources, and hidden live consumers. Update
   `AGENTS.md`/README operational documentation to describe only the actual target.
9. Run structural and textual coverage with explicit exclusions for immutable history, vendor,
   caches, secrets, and generated non-live evidence. Record skipped/unparsed/overlapping candidates;
   zero matches without a validated coverage envelope is not acceptance.

**Legacy disposition and decommission.** Completes DB15, DB16, and DB17. V2.2 designs, previous
plans/states/reviews, and append-only evidence remain history but cannot be generated, packaged,
selected, imported, served, or used by a current gate. Target Rust semantic/resource authority,
generated v2 bindings, strict presentation DTOs, owned endpoints, and thin operational binaries are
retained.

**Acceptance checks.**

Executable oracle: `fastmcp4-post-purge-surface-check`
Governed criterion: `PC-WP49-INT`

Executable oracle: `fastmcp4-retained-target-behavior-check`
Governed criterion: `PC-WP49-BEH`

Executable oracle: `fastmcp4-decommission-zero-state-check`
Governed criterion: `PC-WP49-NEG`

Executable oracle: `fastmcp4-package-build-check`
Governed criterion: `PC-WP49-OPS`

**Oracle category fault contract.** `INT` restores a removed source/export/pin/package/rule/recipe
or changes the fixed target surface; `BEH` reruns representative guard/resource/completion/cancel
behavior after deletion; `NEG` seeds every predecessor class in each covered live location and
requires zero-state failure; `OPS` builds/installs/inventories all retained domains/features/wheels
and proves no removed artifact is reachable.

**Edit-local gates.** Run targeted `rg` and ast-grep rules, dependency/lock/package inspection,
adapter and Rust focused tests, `just root-check`, `just adapter-ci-fast`, `just proto-check`,
`just stable-graph-check`, `just governance-scan`, and target package builds after each deletion
cluster.

**Packet-local gates.** Add and run the four packet recipes. The zero-state recipe validates its
coverage census and seeded faults; the package recipe inspects compiled targets, features, generated
bindings, wheel contents, installed entry points, recipes, and services rather than source alone.

**Integration milestone.** Advances M11 and completes DB15--DB17; WP50 completes the milestone.

**Replan triggers.** Stop if a predecessor surface still has a real target consumer, if a historical
exclusion is generated/packaged/selectable, if zero-state reaches secrets/vendor content or skips an
unclassified live path, or if deleting Python identity semantics would remove a genuine strict
presentation requirement rather than duplicate Rust authority.

**Rollback and recovery.** Restore only a deleted target consumer proven necessary, then redesign
its ownership before continuing; do not restore an entire predecessor subsystem or pin. After lock/
package changes, repair forward and rerun clean installation rather than copying old artifacts.

### WP50 — Re-execute post-purge release and measured resource evidence

**Outcome.** The purged target passes clean four-domain/package behavior and a pre-registered,
representative FastMCP 4 resource/performance study. Startup, memory, calls, guards, completion,
resources, cancellation, reconnect, and N-agent behavior are measured separately; tuning is accepted
only when attribution and semantic equality justify it.

**Dependencies.** WP49.

**Target invariants.** I5-01--I5-24.

**Design and library references.** FastMCP 4 design §§12, 16--17; successor SUITE/SRV/RM
performance/release clauses; principles P2--P5, P10, P18--P20, P22,
P25--P31, P36; repository performance/evidence doctrine.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'daemon-boundary-bench|performance|startup|RSS|latency|throughput|fairness|N-agent|history-independence|post-purge' justfile scripts tooling/ci tests codefabric-cpg-mcp docs/reviews
just post-purge-package-build-operations-check
just clean-incremental-recovery-performance-check
just stable-graph-check
```

Known touch includes the versioned benchmark/evidence harness, real-workload fixtures, resource
samplers, environment record, minimal FastMCP 4 control, post-purge release matrix, CI opt-in job,
and only measured target code changes supported by attribution.

**Required changes.**

1. Rebuild every retained Rust/Python domain, feature, generated binding, binary, and wheel from
   clean controlled roots. Install the adapter and repeat representative semantic/guard/resource/
   cancel/restart behavior before measuring.
2. Execute the WP43 pre-registered method and candidate-neutral budget. Record environment, sample
   count, warm/cold classification, distributions and uncertainty, workload/data scale, process
   topology, dependency identities, resource ceilings, exclusions, and limitations. A failed result
   cannot be fixed by editing the budget in the same candidate.
3. Measure installed-adapter cold readiness; idle/active RSS per adapter; status/no-op and atomic
   query-acceptance overhead; one/max guard rounds; authorized completion latency/cardinality;
   resource first-byte/throughput; cancellation acknowledgement/cleanup; reconnect/resume; and
   N-agent fairness plus aggregate Python/daemon memory.
4. Compare the target against a minimal FastMCP 4 control only to attribute framework/application
   overhead, never against FastMCP 3 or legacy semantic outputs as correctness authority. Preserve
   identical semantic requests, decoded outputs, bounds, and process topology across meaningful
   comparisons.
5. Prove structural resource properties independently of timing: one long-lived channel per adapter,
   one daemon per workspace, no synchronous blocking/Arrow decode/threadpool semantic work, capped
   completions, bounded pages/rounds/queues/logs, no per-call server/channel construction, and
   joined cleanup.
6. Tune only a measured dominant target-owned cost while retaining semantic equality and rerunning
   affected packet gates. Do not add cache, tasks, shared HTTP, compression, keepalive, HTTP/2 knobs,
   unsafe code, release-profile folklore, or weaker isolation without a design replan.

**Legacy disposition and decommission.** No predecessor implementation, history comparator, stale
package, or old lock enters the performance verdict. Temporary benchmark products are bounded and
non-authoritative; no benchmark-only path ships. DB15--DB17 must remain closed throughout.

**Acceptance checks.**

Executable oracle: `fastmcp4-release-matrix-integrity-check`
Governed criterion: `PC-WP50-INT`

Executable oracle: `fastmcp4-post-purge-release-check`
Governed criterion: `PC-WP50-BEH`

Executable oracle: `fastmcp4-history-independence-check`
Governed criterion: `PC-WP50-NEG`

Executable oracle: `fastmcp4-performance-check`
Governed criterion: `PC-WP50-OPS`

**Oracle category fault contract.** `INT` drifts the pre-registered method/budget/input/environment
or omits a retained package; `BEH` executes clean post-purge decoded behavior; `NEG` introduces a
history/predecessor/cache dependency and requires the verdict to remain target-only; `OPS` injects
blocking/unbounded/per-call-channel/cleanup faults and requires resource or performance evidence to
distinguish them.

**Edit-local gates.** Run focused benchmark harness tests, package/release checks, semantic equality
controls, resource-bound assertions, and affected packet gates. Use repository performance recipes;
do not embed ad hoc raw benchmark commands in certification.

**Packet-local gates.** Add and run the four packet recipes. Store raw samples and reviewed summary
in versioned packet evidence, not execution state. The OPS recipe must fail on a committed resource
fault even when wall-clock noise would hide it.

**Integration milestone.** Completes M11 and produces the release candidate for FreshActivation.

**Replan triggers.** Stop if no product/independent envelope can be issued, the target misses its
frozen bound, per-agent startup/RSS dominates enough to reopen a separately secured shared-edge
profile, target semantics differ across comparisons, or a proposed optimization crosses authority/
process/security boundaries.

**Rollback and recovery.** Revert only candidate tuning, never evidence or target semantics. Keep
the untuned correct implementation and measured limitation when a tuning experiment fails; issue a
design replan for topology changes.

### WP51 — Execute FreshActivation and prove sole target authority

**Outcome.** From an empty supported workspace with no predecessor head, model, cache, handoff, or
cutover controller, the complete purged release creates, reads back, activates, serves, restarts,
and forward-repairs only the target authority. Dormant predecessor handoff/cutover machinery then
reaches physical zero state.

**Dependencies.** WP50.

**Target invariants.** I5-01--I5-07, I5-12--I5-24.

**Design and library references.** Successor SUITE/FAB/LIFE/SRV/RM FreshActivation, exact
activation, serving, release, and zero-state sections; daemon review
FreshActivation/unknown-outcome/restart sections; principles P2--P5, P9--P10, P18--P20, P22,
P25--P31, P36.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'FreshActivation|AuthorityHandoff|cutover|handoff|ExpectedHead::Empty|unknown.*outcome|forward repair|predecessor' src tests contracts tooling/ci scripts justfile .github
ast-grep outline src tests/integration tooling/ci | sed -n '1,360p'
just fabric-activation-recovery-check
just unknown-cutover-reconciliation-check
```

Known touch includes fresh workspace fixtures, activation fault injection, restart/repair process
harness, dormant handoff/cutover sources/features/recipes/services, zero-state tooling, and release
configuration.

**Required changes.**

1. Provision a new bounded workspace/runtime/storage root with no activation event, semantic model,
   generated predecessor registry, serving cache, handoff artifact, or prior daemon. Start through
   the same production supervisor and installed packages used by users.
2. Submit lawful genesis through the recovered command actor with `ExpectedHead::Empty`; execute
   exact providers, transformations, analyses, proof, Delta writes, activation append/readback,
   workspace reconstruction, ready admission, modern FastMCP guard/query/resource/completion/
   cancellation, and decoded result validation.
3. Restart every process from durable target state, discard caches, and prove exact selected Delta
   versions/event/vector/fence/horizon reconstruct identical authority. No candidate object,
   receipt, digest, latest lookup, model replay, or old schema is allowed.
4. Inject failure before append, acknowledged append, unknown append outcome, delayed/unavailable
   readback, competing writer, corrupted/incoherent horizon, supervisor death, daemon generation
   change, adapter reconnect, and forward repair. Reconcile exact outcome or stay failed-closed;
   never blindly retry or restore predecessor state.
5. Perform target-only forward repair after target mutation and prove serving sessions/resources are
   revoked or reconstructed according to generation/lease policy. Preserve pinned predecessor epoch
   leases until their lawful target retention ends.
6. Run a read-only deployment census for a real predecessor. If one exists, stop and design a
   separate one-shot `AuthorityHandoff`; do not activate dormant generic machinery. If none exists,
   remove handoff/cutover controllers, roles, states, features, recipes, services, fixtures, and
   recovery branches from all live scopes.

**Legacy disposition and decommission.** Completes DB18. Historical descriptions remain immutable;
no executable handoff, predecessor role, dual-authority state, rollback controller, or compatibility
service remains. Target activation/reconciliation/writer/lease primitives remain under the sole
command/lifecycle authority.

**Acceptance checks.**

Executable oracle: `successor-fresh-activation-contract-check`
Governed criterion: `PC-WP51-INT`

Executable oracle: `successor-fresh-activation-authority-check`
Governed criterion: `PC-WP51-BEH`

Executable oracle: `successor-fresh-activation-zero-state-check`
Governed criterion: `PC-WP51-NEG`

Executable oracle: `successor-fresh-activation-reconciliation-check`
Governed criterion: `PC-WP51-OPS`

**Oracle category fault contract.** `INT` corrupts empty-head/command/event/vector/fence/horizon or
sole-profile contracts; `BEH` executes full empty-to-modern-serving behavior and exact restart;
`NEG` seeds every predecessor/handoff/model/cache/receipt/hash/latest route and requires rejection;
`OPS` faults every append/readback/restart/repair edge and proves exact reconciliation or failed-
closed state.

**Edit-local gates.** Run focused activation, lifecycle, Delta, supervisor, session/resource, and
zero-state tests plus target package installation. Re-run affected WP44/WP45/WP47 gates after any
recovery change.

**Packet-local gates.** Add and run the four packet recipes against clean isolated production roots
and real subprocesses. A test-seeded activation head, direct daemon start, injected backend, or
precreated descriptor cannot satisfy BEH/OPS.

**Integration milestone.** Advances M12 and completes DB18; WP52 completes the milestone.

**Replan triggers.** Stop if the deployment census discovers a real predecessor, one coherent exact
readback cannot determine authority, supported storage cannot express lawful empty-head append and
unknown-outcome reconciliation, or forward repair would require predecessor restoration.

**Rollback and recovery.** Before append, discard the candidate and keep admission closed. After an
unknown result, reconcile exact state. After target mutation, repair forward through the target
command actor. Never delete durable target state merely to make a test appear fresh.

### WP52 — Certify the complete successor at one trusted HEAD

**Outcome.** At one committed candidate, every v5 packet oracle, retained v4 relational-substrate
proof, FreshActivation case, decommission batch, clean package, four build domain, modern installed
vertical, resource/performance envelope, and independent implementation review passes. Only then
may governed state mark v5 complete.

**Dependencies.** WP43, WP44, WP45, WP46, WP47, WP48, WP49, WP50, and WP51.

**Target invariants.** I5-01--I5-24.

**Design and library references.** All v2.3 authoritative suite release/proof clauses; accepted
FastMCP 4 and daemon reviews; full data-fabric principles P1--P36; all
LD5 decisions; repository release/evidence doctrine.

**Change surface / Preflight / Known Touch.** Run exactly:

```bash
git status --short --untracked-files=all
just plan-status
just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
just artifacts-check
just oracle-substance-check
just gate-filter-census
just authoritative-design-conformance-check
```

Known touch includes only aggregate certification recipes/tooling, final evidence/review artifacts,
state updates after proving commits, and focused repairs uncovered by the matrix. Any semantic repair
returns to the owning packet and invalidates dependent evidence rather than being hidden in an
aggregate.

**Required changes.**

1. Freeze one candidate commit and derive the packet/oracle universe from this parsed plan. Validate
   exactly WP43--WP52 and forty unique packet oracles without hard-coded stale WP28/v4 counts or
   aliases that discard failures.
2. Rerun every v5 packet oracle at candidate HEAD. Verify proving commits are ancestral, declared
   inputs/suite/expectations are fresh, selectors are nonzero and fault-sensitive, milestones and
   decommission batches are complete, and state contains only schema-permitted judgment fields.
3. Add `relational-fabric-v5-inherited-substrate-check` and revalidate the inherited relational
   substrate. Run the four target-owned oracles for v4 WP31, WP32, WP34, WP35, and WP36 unchanged.
   Run the repaired target-only composition/lifecycle and
   decommission/restart oracles for stale WP29/WP30. Do not restore a stale FastMCP dependency,
   predecessor authority, or hash-based proof to make an old recipe pass.
4. Run the complete real modern vertical, independent production evidence, post-purge package/
   resource/performance matrix, and FreshActivation/restart/reconciliation from clean roots. Inspect
   installed binaries, wheel, descriptor, services, locks, and live server surface.
5. Run stable root, extractor, Pyrefly sidecar, adapter, proto, feature-isolation, dependency graph,
   governance, policy, security, and release gates with no mutating recipe as an implicit dependency.
6. Obtain an independent implementation review against v2.3, this plan, accepted reviews, current
   behavior, library decisions, decommission coverage, evidence causality, and resource results.
   Resolve blocker/major findings through owning packets and rerun dependent gates.
7. Produce a final certification artifact recording exact candidate/proving/input identities,
   commands/exits/selected counts, environment/resources, faults, exclusions, limitations, and
   review verdict. Only a governed state transaction may mark packets/milestones/batches/plan
   complete.

**Legacy disposition and decommission.** DB15--DB18 must be physically closed. V2.2 and v4 remain
immutable history only. Any live runtime/package/gate dependency on FastMCP 3, predecessor semantic
authority, dormant handoff, duplicate Python state, stale schema, or invalidated evidence blocks
certification.

**Acceptance checks.**

Executable oracle: `fastmcp4-successor-provenance-check`
Governed criterion: `PC-WP52-INT`

Executable oracle: `relational-fabric-v5-certification`
Governed criterion: `PC-WP52-BEH`

Executable oracle: `fastmcp4-successor-final-zero-state-check`
Governed criterion: `PC-WP52-NEG`

Executable oracle: `fastmcp4-successor-four-domain-release-check`
Governed criterion: `PC-WP52-OPS`

**Oracle category fault contract.** `INT` corrupts one input, proving ancestry, selector, criterion,
descriptor, lock, package, or evidence reference; `BEH` changes one independently expected decoded
outcome or replaces a real process; `NEG` restores any decommissioned/live legacy class or removes a
required fault; `OPS` fails one domain/package/FreshActivation/restart/resource/performance/review
child and requires the aggregate to preserve failure.

**Edit-local gates.** Run only focused gates for any discovered repair, then return to the owning
packet. Re-freeze a new candidate and repeat all dependent evidence; do not patch generated
certification output or state to mask a failure.

**Packet-local gates.** Run the four packet recipes and the complete final gate matrix in §7. The
certification aggregate lists and preserves every child; it is not a shortcut or substitute.

**Integration milestone.** Completes M12 and the implementation program.

**Replan triggers.** Stop if an accepted design/input drifts, a retained proving commit leaves
history, a packet oracle is missing/empty/non-discriminating, a final gate cannot run in the target
environment, independent review finds a design-level defect, or completion would require weakening
an invariant/zero-state/resource bound.

**Rollback and recovery.** A failed terminal gate leaves v5 executing and the last complete packet
trusted only if its own proof remains ancestral/fresh. Repair through the owning packet, create a new
candidate, rerun all dependents, and never mark completion because time or budget is exhausted.

## 5. Integration milestones

### M08 — V2.3 authority and expectations are frozen

**Entry:** Approved/activated v5 with fresh declared inputs.

**Completion:** WP43 complete; v2.3 is the sole current suite; independent expectations and faults
are accepted.

**Gate:** the four WP43 packet oracles plus `just authoritative-design-conformance-check`.

### M09 — Startup and daemon contract are target-coherent

**Entry:** M08 complete.

**Completion:** WP44 and WP45 complete; empty startup reconciles to Ready, WP29/WP30 outcomes pass,
and the sole generated v2 contract owns atomic start/challenge/resource/completion/cancel behavior.

**Gate:** all WP44/WP45 packet oracles, `just proto-check`, and
`just supervisor-launch-contract-check`.

### M10 — Modern FastMCP 4 delivery is real

**Entry:** M09 complete.

**Completion:** WP46 and WP47 complete; exact dependency/profile/catalog zero state and the real
one-daemon/two-agent installed STDIO vertical pass.

**Gate:** all WP46/WP47 packet oracles plus bridge-off adapter package/STDIO gates.

### M11 — Independent evidence, purge, and measured release candidate are accepted

**Entry:** M10 complete.

**Completion:** WP48, WP49, and WP50 complete; independent causal evidence passes, DB15--DB17 are
closed, clean packages pass, and measured resource/performance evidence meets the frozen envelope.

**Gate:** all WP48--WP50 packet oracles plus post-purge package and governance checks.

### M12 — FreshActivation and terminal certification are complete

**Entry:** M11 complete.

**Completion:** WP51 and WP52 complete; DB18 is closed and the entire target is independently
certified at one trusted HEAD.

**Gate:** all WP51/WP52 packet oracles and the final matrix in §7.

## 6. Cross-packet decommission batches

### DB15 — Remove FastMCP 3 and duplicate Python authority

**Target consumer:** WP46 modern factory/guard/daemon port and WP47 real vertical.

**Delete after:** WP48 independent evidence passes.

**Scope:** old pins/lock entries/bridge/imports, global-server assumptions, duplicate freshness and
attempt IDs, normal Validate-before-Start, Python handles/lease maps, canonical identity/request
rewriting, and any session/task/cache/workflow authority.

**Certified by:** WP49 zero state and WP52 final zero state.

### DB16 — Remove stale wire, schema, prompt, and serving compatibility surfaces

**Target consumer:** WP45 generated v2 contract and WP46 fixed public surface.

**Delete after:** WP47 installed vertical and WP48 evidence pass.

**Scope:** displaced v2 live descriptor/runtime outputs, v1 services/clients, static adapter schemas,
phantom prompts, pre-modern clients/results, old resource routes, stale rules/tests/selectors, and
mock/direct launch shortcuts from release gates.

**Certified by:** WP49 surface/package oracles and WP52 certification.

### DB17 — Remove invalidated evidence, package, dependency, recipe, and history reachability

**Target consumer:** WP48 successor evidence and WP49 target packages/rules/recipes.

**Delete after:** successor evidence proves every claimed outcome.

**Scope:** live v4 WP37/WP38 FastMCP 3 evidence, generated expectations, retired optional
dependencies/features/targets, dead test references, old wheel resources/entry points, workflows,
services, and historical artifacts that are accidentally selectable.

**Certified by:** WP49 package/zero-state oracles and WP50 history independence.

### DB18 — Remove dormant handoff and predecessor cutover authority

**Target consumer:** WP51 sole FreshActivation/forward-repair path.

**Delete after:** read-only deployment census proves no real predecessor and target activation,
restart, unknown-outcome reconciliation, and forward repair pass.

**Scope:** handoff/cutover controllers, roles, states, features, recipes, services, fixtures, and
dual-authority recovery branches; target writer/fence/reconciliation primitives remain.

**Certified by:** WP51 zero state and WP52 final zero state.

## 7. Final gate matrix

Packet recipes supplement repository gates; they do not alias or replace them. Recipes named below
that do not yet exist are explicit plan deliverables of their owning packet.

| Gate family | Exact `just` commands at WP52 | Required truth |
|---|---|---|
| Design/artifact/state | `just authoritative-design-conformance-check`; `just artifacts-check`; `just plan-status`; `just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md`; `just oracle-substance-check`; `just gate-filter-census` | V2.3 sole current; inputs/state/DAG/proving ancestry/selectors/fault contracts coherent; no migrated v4 completion fiction. |
| V5 packet proof | `just successor-all-packet-oracles-check`; parsed `just packet-oracle-check <WP>` for WP43--WP52 | Forty unique substantive packet oracles pass with nonzero selection and discriminating faults. |
| Retained relational substrate | `just relational-fabric-v5-inherited-substrate-check` | Repaired target-only WP29/WP30 and unchanged WP31/WP32/WP34/WP35/WP36 oracles all pass without legacy/hash restoration. |
| Startup/supervisor | `just fastmcp4-startup-contract-integrity-check`; `just fresh-activation-ready-reconciliation-check`; `just supervisor-startup-boundary-rejection-check`; `just supervisor-restart-join-operations-check`; `just supervisor-launch-contract-check`; `just supervisor-launch-platform-check` | One supervisor/daemon, exact ready/reconciliation, safe fd3/UDS/policy/restart/join behavior. |
| Wire/session/resource | `just fastmcp4-daemon-wire-contract-check`; `just fastmcp4-atomic-start-check`; `just fastmcp4-resource-authority-check`; `just fastmcp4-daemon-security-recovery-check`; `just proto-check`; `just proto-repro-check` | Sole generated v2 contract, atomic closed outcome, daemon challenges/handles/completion/cancel, Rust/Python interop. |
| FastMCP adapter | `just fastmcp4-dependency-contract-check`; `just fastmcp4-modern-protocol-check`; `just fastmcp4-adapter-authority-zero-state-check`; `just fastmcp4-public-surface-check`; `just adapter-ci-fast`; `just adapter-wheel-test`; `just adapter-stdio-test` | Exact bridge-off FastMCP 4/MCP/Pydantic profile, fixed surface, strict reconstructible presentation-only package. |
| Real modern vertical | `just fastmcp4-contract-observation-check`; `just fastmcp4-stdio-vertical-check`; `just fastmcp4-security-negative-check`; `just fastmcp4-cancellation-recovery-check`; `just fastmcp4-guard-roundtrip-check`; `just fastmcp4-completion-authorization-check` | Real installed one-daemon/two-agent guarded, resource, completion, cancel, reconnect, restart, security, and STDOUT behavior. |
| Independent evidence | `just fastmcp4-production-evidence-integrity-check`; `just fastmcp4-production-behavior-check`; `just fastmcp4-causal-fault-check`; `just fastmcp4-clean-reconstruction-check` | Independently authored decoded outcomes/faults and clean reconstruction decide correctness; no history comparator. |
| Purge/package | `just fastmcp4-post-purge-surface-check`; `just fastmcp4-retained-target-behavior-check`; `just fastmcp4-decommission-zero-state-check`; `just fastmcp4-package-build-check`; `just remaining-legacy-zero-state-check`; `just post-purge-package-build-operations-check` | DB15--DB17 physically closed across source/generated/package/gates and retained target still works. |
| Performance/resources | `just fastmcp4-release-matrix-integrity-check`; `just fastmcp4-post-purge-release-check`; `just fastmcp4-history-independence-check`; `just fastmcp4-performance-check` | Clean target meets frozen representative envelope and structural bounds with semantic equality. |
| FreshActivation | `just successor-fresh-activation-contract-check`; `just successor-fresh-activation-authority-check`; `just successor-fresh-activation-zero-state-check`; `just successor-fresh-activation-reconciliation-check` | Empty target genesis, exact restart/readback/reconciliation/repair, DB18 zero state, no seed/predecessor/hash/latest path. |
| Stable root | `just root-fmt`; `just root-check`; `just root-clippy`; `just root-test-rust`; `just root-doctest`; `just features-no-default`; `just features-each`; `just stable-graph-check` | Formatting, compiler/lints, ordinary/doc tests, feature isolation, exact dependency universe. |
| Extractor | `just extractor-fmt`; `just extractor-check`; `just extractor-test`; `just extractor-identity` | Exact dated-nightly provider identity and behavior. |
| Sidecar | `just sidecar-ci-fast` | Exact pinned Pyrefly sidecar API/process behavior and routine domain gate. |
| Repository release | `just governance`; `just ci-pr`; `just fastmcp4-successor-provenance-check`; `just relational-fabric-v5-certification`; `just fastmcp4-successor-final-zero-state-check`; `just fastmcp4-successor-four-domain-release-check` | Complete supported release, final zero state, independent review, and one trusted HEAD. |

`relational-fabric-v5-inherited-substrate-check` must preserve the exits and selected counts of the
four target-owned criteria for v4 WP29, WP30, WP31, WP32, WP34, WP35, and WP36. It may repair the
current WP29/WP30 recipe coupling but may not weaken their programmatic composition, zero-state,
exact Delta, DataFusion, provider, analysis, query, page/package, cancellation, retention, or restart
truth. WP31--WP36 recipe semantics remain unchanged.

## 8. Execution sequence, ownership, and state discipline

1. Independently audit this draft. Apply corrections through a versioned artifact if material, mark
   the accepted plan approved, then use `just plan-activate` to create a fresh schema-v2 v5 state and
   atomically replace the active pointer. Preserve v4 plan/state as history.
2. Execute WP43 first and freeze v2.3 plus independent expectations before completing any public
   contract change.
3. Execute WP44 against the dirty startup foundation. Attribute/reserve every path, retain compatible
   work, close exact reconciliation, and revalidate WP29/WP30 before changing the wire.
4. Execute WP45 as one proto/descriptor/Rust/Python transaction. Do not leave a partially generated
   contract selected or packaged.
5. Execute WP46 and WP47 in order: build the bridge-off modern presentation cell, then prove the real
   installed one-daemon/two-agent vertical. Component mocks never close the process packet.
6. Execute WP48, WP49, and WP50 in order. Independent evidence precedes deletion; target consumers
   and evidence precede zero state; purge precedes package/resource/performance certification.
7. Execute WP51 only on the purged release candidate. Remove dormant handoff/cutover code only after
   the deployment census and FreshActivation/repair proof.
8. Execute WP52 at one frozen candidate HEAD. Any repair returns to its owning packet and invalidates
   dependent evidence before a new candidate is frozen.

Before every packet, record current HEAD/input freshness, `git status --short --untracked-files=all`,
structural/textual candidate coverage, Cargo/uv/generated/package/recipe impact, shared-path
reservations, and unrelated dirty changes. Do not stage, reset, overwrite, or attribute other work.
A separate worktree does not waive shared sockets, services, target directories, locks, caches, or
runtime roots.

Each packet has one proving commit containing implementation, tests, exactly four oracle recipes,
fixtures/faults, and a versioned evidence artifact. Evidence records commands/exits/counts,
candidate/environment/resources, impacted/excluded/skipped candidates, limitations, and recovery;
schema-v2 state records only judgment, proving commit, deviations, failed approaches, blockers, and
next action. If packet code changes after proof, rerun its four oracles and every dependent milestone/
terminal gate before updating state.

## 9. Risks and replan policy

| Trigger | Required action |
|---|---|
| V2.3 allocation or accepted input drifts. | Stop dependent work and issue/audit a revised plan or forward suite version; never restamp hashes. |
| Fresh startup cannot read one coherent exact event/vector/fence/horizon. | Reopen durable activation storage/readback; remain failed-closed and do not seed/retry/latest/hash-select. |
| Atomic start/challenge requires opaque JSON, prose parsing, or adapter semantics. | Reopen QRY/wire design; do not duplicate semantic preparation in Python. |
| FastMCP cannot reject pre-modern eras before business dispatch through supported API. | Reopen target host/library profile; do not add a legacy functional branch. |
| Guard continuation cannot retain strict schemas, cancellation, or STDIO behavior. | Reopen the narrow guard adapter with executable library evidence; do not accept work early or use tasks. |
| Daemon public handles cannot remove Python lease state without exposing secrets or weakening authorization. | Reopen resource-handle contract; do not retain `_resource_leases`. |
| Authorized completion reveals denied existence or needs local inventory. | Remove/reopen completion; do not simulate it with a tool or Python cache. |
| Cancellation cannot be distinguished from watch loss or reconnect requires resubmission. | Reopen coordinator/wire recovery semantics; never retry Start blindly. |
| Native Arrow/DataFusion/Delta behavior no longer preserves existing proved semantics/bounds. | Produce the failing native control and reopen the owning substrate packet; no parallel Python/custom evaluator. |
| A real deployed predecessor is discovered. | Stop WP51 and design a separate one-shot AuthorityHandoff; do not enable dormant generic cutover code. |
| Zero-state coverage skips/unparses/unclassifies a live candidate or reaches secrets/vendor paths. | Correct the coverage envelope and rerun seeded faults; empty grep is insufficient. |
| Performance budget is absent, changed after results, or missed. | Issue independent requirements or reopen topology using measured attribution; do not tune by folklore or weaken isolation. |
| Multi-host/network/cross-user deployment enters scope. | Reopen fencing, identity, transport security, supervisor, and FastMCP deployment profiles. |
| Independent review finds a design-level defect. | Return to design/plan versioning; do not bury the correction in certification code or state. |

Primary operational risks are dirty-tree ownership collision, false `Ready` projection, accidental
ordinary Validate-before-Start, treating FastMCP request-state sealing as authorization, secret/
path leakage through handles/completion/telemetry, cancellation by watch drop, old bridge behavior
passing tests, stale generated descriptors, hidden package/rule/recipe reachability, unbounded Python
materialization, and a final aggregate that validates records rather than behavior. Packet order,
four-category faults, installed-process evidence, zero-state coverage, and independent review are
the controls and may not be waived locally.

## 10. Activation and completion boundary

This draft is not active. Activation must:

1. verify this exact file and every declared input;
2. verify the accepted FastMCP 4 and daemon review composition plus the v4 status reconciliation;
3. verify baseline ancestry and inventory the dirty tree without claiming its work complete;
4. create a fresh schema-v2 state at the declared v5 path with WP43--WP52, M08--M12, and
   DB15--DB18 only;
5. migrate no v4 packet/milestone/batch status or proving commit into v5 state;
6. atomically point the active-plan file to this exact approved artifact; and
7. leave the v4 plan/state and all released suite/evidence artifacts immutable history.

Implementation is complete only after M08--M12 and DB15--DB18 close, every v5 packet oracle and the
retained v4 substrate proof pass at one trusted HEAD, the real installed modern STDIO vertical and
FreshActivation/restart/reconciliation pass from clean roots, representative resource/performance
evidence meets its frozen envelope, every decommission coverage census is closed, all four build
domains and packages pass, and the independent implementation review is accepted. Only the governed
state transaction may then mark v5 complete.
