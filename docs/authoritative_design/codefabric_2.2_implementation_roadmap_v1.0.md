---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-roadmap
suite_id: codefabric-relational-data-fabric
suite_version: 2.2.0
artifact_tag: RM
artifact_version: 1.0.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_2.1_implementation_roadmap_v1.0.md
---

# CodeFabric 2.2 relational data-fabric implementation roadmap

## 0. Authority and boundary

This roadmap has stable artifact identity
`codefabric-relational-data-fabric-roadmap`. It is subordinate to SUITE, ONT,
GEN, FAB, QRY, LIFE, and SRV. It orders capability and decommission work but
cannot weaken, reinterpret, or certify their contracts. The active approved v4
implementation plan owns exact packet dependencies, acceptance checks, proving
commits, and decommission batches.

V2.2 is a forward-only design target. It preserves the present-state fact and
query capabilities but does not preserve v1 runtime operability, predecessor
bootstrap/model authority, dormant handoff machinery, or compatibility routes.
Historical suites, descriptors, allocation records, plans, and reviews remain
immutable history and never become production inputs.

## 1. Sequencing invariants

1. Issue the synchronized v2.2 suite and independently authored expectations
   before dependent implementation claims. WP28 and M01 are not dependencies.
2. One suite-version-neutral `CompiledSemanticRelease` with immutable
   `SuiteIdentity` owns all production semantic construction.
3. Operational callers provide workspace roots, policy, credentials, and bounds
   only; they cannot inject catalogs, schemas, transformations, query programs,
   proof closures, or release vectors.
4. One phase-typed startup path owns empty-head genesis, warm recovery, exact
   activation readback, and atomic `ActiveWorkspace` installation.
5. DataFusion child-session/schema/plan closure and exact Delta history/recovery
   precede provider, analysis, query, and lifecycle release.
6. One `WorkspaceSupervisor` owns one daemon, one safely acquired and live-probed
   singleton, one private control socketpair mapped to daemon stdin, grants,
   generations, process groups, exact socket cleanup, and joined shutdown per
   workspace.
7. `codefabric mcp serve` is attach-only and launches one presentation-only
   adapter from one safely loaded operator-owned `AgentLaunchPolicy`; direct
   host STDIO, bounded stderr, and allowlisted fd 3 grant delivery are structural,
   and the launcher never starts a daemon or proxies MCP bytes.
8. `codefabric.cpgd.v2` is the sole production RPC package. V1 source and
   descriptors remain history only; live v1 generation, package data, services,
   clients, translators, and profiles are deleted.
9. Every accepted query pins one immutable epoch and streams into bounded,
   independently decodable Arrow IPC pages whose manifest is sealed last.
10. Fresh activation is target-only, genesis is `ExpectedHead::Empty`, uncertain
    outcomes fail closed, and repair is forward-only.
11. Every packet leaves its immediate consumers coherent and removes attached
    displaced authority at the earliest dependency-safe boundary.
12. Final release requires independent decoded expectations, committed causal
    faults, resource/security/recovery proof, and target-only physical zero
    state at one trusted HEAD.

## 2. Capability stages

### 2.1 Stage 0 -- successor authority and independent expectations (`WP33`)

Issue exactly one synchronized terminal v2.2 suite. Freeze independently
reviewed semantic, lifecycle, wire-v2, supervisor, resource, security, recovery,
and zero-state expectations plus committed negative fixtures. Supervisor
expectations cover safe policy-root selection, peer UID and supported PID/start
identity, singleton live probing, control-record bindings, direct STDIO and fd 3
inheritance, bounded stderr, the exact one-shot fallback condition, partial-spawn
reclamation, and wrong owner/workspace/operation/generation/socket negatives. Exit capability:
later work is judged against an expectation the production system did not
author.

### 2.2 Stage 1 -- honest production kernel and predecessor removal (`WP29`--`WP30`)

Build the real daemon from `CompiledSemanticRelease`, explicit operational
inputs, phase-typed workspace state, one command supervisor, one lifecycle
projection, one query coordinator, and one atomic active-workspace slot. Prove
cold genesis and exact warm recovery, then delete bootstrap/model replay,
generated schema/catalog authority, dual epoch pins, migration wrappers, default
backends, test-only seed routes, and displaced build/package surfaces.

### 2.3 Stage 2 -- DataFusion and Delta closure (`WP31`--`WP32`)

Derive schema contracts from typed plans, retain optimizer-visible native
expressions and plans at the highest viable extension rung, close recursive
authorized child catalogs over bound provider/function/object-store authority,
and make every query use one bounded governed runtime. Publish through exact
delta-rs versions with zero blind retry, reversible activation vectors,
readback, fencing, uncertain-outcome reconciliation, and two successive
in-process activations.

### 2.4 Stage 3 -- exact providers, analyses, queries, and lifecycle (`WP34`--`WP36`)

Run exact provider-native Arrow relations and explicit gaps; execute one
application-owned producer for every derived family; compile all eight request
forms as typed DataFusion transformations; and drive invalidation, update,
publication, activation, and capability from the same authoritative relations.
Unknown and incomplete coverage remain explicit and never become empty proof.

### 2.5 Stage 4 -- supervisor through FastMCP vertical (`WP37`)

Implement one real path:

```text
operator policy -> WorkspaceSupervisor -> codefabricd
source image/change -> provider batches/gaps -> typed transformations/proof
-> exact Delta publication + activation -> atomic ActiveWorkspace
-> authorized scheduled DataFusion query -> sealed Arrow page package
-> codefabric.cpgd.v2 over owned UDS -> strict FastMCP projection
```

Prove singleton and descriptor ownership on Linux and macOS, single-use launch
grants, same-UID plus supported PID/start peer checks, policy-root no-follow/
owner/mode/type/device/inode enforcement, generation/revocation/replay denial,
attach-only multi-agent operation, direct STDIO inheritance, bounded stderr,
and joined/reaped process lifecycle. Compile- and process-probe exact
`command-fds` 0.3.3 with Tokio support as the safe fd 3 candidate on Linux and
macOS; never clear `CLOEXEC` around spawn. If and only if a supported platform
cannot satisfy fixed-fd inheritance, prove the private-root, no-follow,
owner-verified `0600` one-shot fallback, immediate unlink, single read, and
path-substitution fault. Also prove control/rendezvous loss, partial spawn,
unsafe socket, wrong owner/workspace/operation/generation, content-bound watch
resume, and accepted-work survival leave no credential or unowned child.

### 2.6 Stage 5 -- causality, durability, and operational closure (`WP38`--`WP40`)

Demonstrate that changing or removing compiled declarations changes behavior;
prove independent decoded semantic expectations and fault sensitivity; prove
durable exact reconstruction, temporal-state isolation, bounded scheduling,
journals, pages, leases, cleanup, cancellation, and forward recovery; and remove
remaining predecessor authorities and comparators.

### 2.7 Stage 6 -- fresh activation and full release (`WP41`--`WP42`)

Activate target-only from an empty head, restart from the exact selected horizon,
prove one daemon/writer/UDS owner, reconcile every uncertain boundary, execute
physical legacy and v1-runtime zero-state checks, rebuild from clean inputs, and
run the complete milestone/final gate matrix. No dormant handoff or fallback
machinery survives.

## 3. Decommission order

1. Preserve immutable v2.1/v2.0/v1.3 suite and wire allocation history.
2. Remove selectable ontology/bootstrap/model/generated-schema authority as
   soon as the compiled release and production consumer exist.
3. Remove duplicate catalogs, schema hashes, proof/vector inputs, test-only
   seeds, default barriers, and caller-defined production query programs.
4. Remove eager whole-result collection, unbounded journals/maps/resources,
   disconnected readiness, and unsafe UDS cleanup as the real v2 vertical lands.
5. Remove v1 generation, live bindings, services, clients, translator/profile,
   adapter package payloads, and compatibility tests in the v2 contract
   transaction.
6. Remove predecessor cutover controllers, bridge/reboot/rollback vocabulary,
   model release tooling, history comparators, and all temporary transition
   surfaces before final certification.

## 4. Proof and release

Each stage exits only through its plan-named executable oracles at a proving
commit. Identity digests authenticate inputs and packages but never establish
semantic correctness. Semantic proof uses independently authored decoded
expectations and committed faults; governance and absence claims use the
strongest available construction, relational, structural, and textual tiers
with stated coverage. A v2.2 release is complete only when every packet,
milestone, decommission batch, final gate, clean reconstruction, restart, and
target-only zero-state oracle is green at the same HEAD.
