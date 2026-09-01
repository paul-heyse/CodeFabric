---
artifact: interface-design-review
date: 2026-09-01
version: v5
status: complete
supersedes: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
serving_specification: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
lifecycle_specification: docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md
fabric_specification: docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
reviewed_head: 6a76b5c
working_tree: clean-before-review
baseline: intentionally-not-taken
verdict: aligned
target_status: accepted
---

# Production supervisor and launch authority: operational target amendment

## 0. Decision

The forward-only target in design v4 remains accepted. This amendment closes its one unresolved
operational boundary: the component that owns the inherited supervisor-control socketpair, the
single shared daemon, and per-agent FastMCP launches is now an explicit production responsibility.

Use the existing same-package Rust `codefabric` administrative binary as the operational surface:
`codefabric supervisor serve` owns one `WorkspaceSupervisor` per workspace, and `codefabric mcp
serve` owns one `AgentStdioLauncher` per attached MCP host. The supervisor is the sole daemon launch and
grant-registration authority. It owns one long-lived `codefabricd` child, one private authenticated
supervisor rendezvous, and the daemon-side control socketpair. A separate same-package
`AgentStdioLauncher` invocation attaches to the existing supervisor, requests one policy-bound launch,
starts exactly one installed FastMCP presentation process, delivers its one-shot bootstrap
capability through an inherited descriptor, and gives the child direct ownership of the host's MCP
stdin/stdout descriptors.

This adds no Cargo root, semantic service, second data plane, compatibility path, or Python
authority. The supervisor and launcher are operational shells around the topology accepted by v3/v4.
All v4 daemon, gRPC v2, session, Arrow-resource, and four-tool FastMCP decisions remain unchanged.

## 1. Named production topology

```text
operator-owned launch policy store
             |
             v
WorkspaceSupervisor (one per workspace, same Rust package)
  |-- private runtime directory + singleton lease/live probe
  |-- private supervisor rendezvous UDS
  |-- unnamed CLOEXEC control socketpair -> one codefabricd child
  |-- grant registry/revoke/generation acknowledgements
  `-- daemon process group, restart generation, drain, and reap ownership

agent host STDIO
  <-> AgentStdioLauncher (one per attached agent, same Rust package)
        |-- requests a named operator policy from the existing supervisor
        |-- receives registered, single-use launch capabilities
        |-- starts one installed Python FastMCP adapter
        |-- passes capability bytes only through inherited fd 3
        `-- directly inherits host stdin/stdout + bounds diagnostic stderr
                    |
                    v
          FastMCP presentation process
                    |
                    v
          owned query UDS -> shared codefabricd
```

The long-lived supervisor is started by the repository's deployment/service boundary or the
explicit `codefabric supervisor serve --config <path>` command. The host-registered
`codefabric mcp serve --config <path>` invocation is attach-only: absence of a
live authenticated supervisor is a typed startup failure, never permission to spawn another
daemon. Later agents attach to the same supervisor and receive independent adapters, grants,
sessions, watches, resources, and cleanup scopes.

The supervisor rendezvous is distinct from the public query UDS and the unnamed daemon-control
socketpair. It carries bounded launch/revoke/status operations only and never semantic query,
result, Arrow, Delta, or catalog data. `OwnedUnixSocket` rules apply to it as strictly as to admin
and query endpoints.

## 2. Policy and identity authority

`AgentLaunchPolicy` is an immutable, operator-owned Class 1 input loaded by the supervisor from an
authorized configuration root. It binds an opaque policy identifier to:

- principal identity and allowed workspace set;
- permitted RPC operations and semantic profiles/features;
- MCP host/resource/deadline bounds;
- issue/not-before/expiry and revocation generation;
- the expected adapter distribution and executable identity where supported; and
- any deployment-specific maximum concurrent launches.

The agent invocation supplies only the opaque policy identifier and presentation/runtime details
that genuinely vary. It cannot supply or override principal, workspace ACLs, operations, semantic
profiles, resource ceilings, expiry, or revocation generation. Policy files are owner-verified,
mode-restricted, opened without following symlinks, parsed strictly, and never selected from the
workspace repository or adapter package. Configuration drift creates a new policy revision; it
does not mutate an accepted grant.

The supervisor authenticates an attaching launcher through the owned rendezvous, kernel same-UID peer
credentials, supported PID/start identity, the selected policy, and bounded anti-replay request
identity. Same UID alone is necessary but not sufficient to choose claims. The daemon derives
authority only from the registered grant and verified query-UDS peer; body identifiers remain
correlation assertions.

## 3. Daemon control and singleton lifecycle

Before starting `codefabricd`, `WorkspaceSupervisor`:

1. creates or safely opens a private per-user runtime directory;
2. acquires a per-workspace singleton lease and performs a live, generation-aware probe;
3. constructs daemon services without exposing a query socket;
4. creates one unnamed Unix socketpair with close-on-exec on every unrelated descriptor;
5. maps the daemon endpoint to `codefabricd` standard input through safe `OwnedFd`/`Stdio`
   ownership (the daemon has no STDIO protocol), without a pre-exec hook or ambient descriptor;
6. starts `codefabricd` in an owned process group and closes the parent copy of the daemon end; and
7. completes generation/control acknowledgement before permitting adapter launches.

The supervisor retains the other endpoint and is the only producer of the length-bounded
`RegisterLaunchGrant`, `RevokePrincipal`, `AdvanceSupervisorGeneration`, and `Acknowledgement`
records accepted by v4. Each record binds workspace, daemon/supervisor generation, monotonic
sequence, operation identity, expiry, and content integrity. Gap, replay, changed duplicate,
unknown record, wrong generation, or control-channel replacement fails closed.

The singleton lease records enough identity to reject a live second supervisor without trusting a
PID file alone. Stale recovery requires lease ownership plus failed live probe and exact owned
socket/type/owner/device/inode checks. A losing racer does not unlink, signal, or replace the
winner. Linux and macOS implementations may observe different peer-PID detail, but both require
same UID, supervisor generation, policy authorization, and fail-closed absence of an optional
observation.

Loss of the supervisor-control channel closes new handshake/renewal authority and moves lifecycle
truth to a typed degraded/draining state. Already accepted queries retain their pinned workspace
and bounded cleanup policy; they are not silently canceled or restarted merely because an adapter
or launcher exits. The supervisor owns daemon drain, joined shutdown, signal propagation, timeout
escalation, child reap, singleton release, and owned-socket cleanup. Orphaned daemon operation is
not a supported steady state.

## 4. Per-agent launch and direct STDIO inheritance

For each authorized attach, the supervisor resolves the named policy, reserves its launch slot,
mints a high-entropy capability, sends only the capability digest and immutable claims to the
daemon, and waits for acknowledgement before releasing the raw capability to the launcher. A failed
registration creates no adapter and consumes no reusable credential.

`AgentStdioLauncher` creates a dedicated capability channel before adapter spawn. The child
inherits the host's stdin/stdout directly, a bounded diagnostic stderr pipe, and the allowlisted
capability channel at fixed fd 3. All unrelated descriptors remain close-on-exec. Capability bytes never
enter argv, ordinary environment variables, the repository, logs, process listings, or MCP
traffic. The adapter reads one bounded, generation-labelled grant frame at a time and erases each
consumed capability buffer as far as safe-language ownership permits. The descriptor remains a
unidirectional launcher-to-adapter launch channel only so the supervisor can deliver a replacement
single-use grant after daemon generation changes; the adapter cannot mint claims or write control
records through it. Descriptor EOF forbids further handshake authority.

The daemon control descriptor uses the standard library's safe stdin mapping above. The adapter's
additional descriptor first uses a safe, maintained descriptor-inheritance API; the initial
candidate is exact `command-fds` 0.3.3 with its Tokio support, with fd 3 as the only extra mapping
while preserving the repository's
`unsafe_code = "deny"` boundary. WP37 must compile-probe and process-probe the selected API on Linux
and macOS before adoption. Temporarily clearing `CLOEXEC` around spawn in the multithreaded Tokio
supervisor is forbidden because a concurrent child can inherit the capability. If no supported launcher can
provide that contract, the only permitted fallback remains v4's private-runtime, no-follow,
owner-verified `0600` one-shot file, unlinked immediately after the child opens it. The fallback is
an explicit supported-platform decision with the same replay, logging, cleanup, and fault tests;
it is not an environment/argv token.

The launcher never reads or writes MCP stdin/stdout: direct inheritance makes byte identity and OS
backpressure structural and removes a protocol buffer/copy loop. Adapter stderr alone is piped and
forwarded to the host diagnostic stream through a bounded policy with truncation/accounting;
secrets and grant bytes are redacted at their source and never depend on log scrubbing. Host EOF is
observed by the adapter directly. Signals and deadlines propagate through the owned child process
group, and launcher exit waits join stderr/lifecycle tasks and reap the adapter. Partial spawn or
launcher failure revokes the unused grant and closes every created descriptor.

Adapter/launcher exit revokes its session authority and releases launch-scoped resources, but an
already accepted daemon query follows its durable query/retention contract. Reconnection creates a
new grant, gRPC channel, and daemon session in the surviving adapter when its inherited fd 3 launch
channel remains valid, then resumes `WatchQuery` by accepted query identity and bound cursor; it
never resubmits `StartQuery`. Proxy/adapter replacement instead creates a wholly new launch.
Daemon restart advances generation, invalidates every volatile grant/session/cursor tied to the old
generation, reconstructs only durably proved terminal/package state, and requires fresh grants.

## 5. Security and platform contract

The production implementation must prove:

- private runtime directories, policy files, leases, and UDS paths reject symlinks, wrong owners,
  wrong modes, wrong types, cross-device replacement, and replacement-inode cleanup;
- one workspace admits one live supervisor/daemon and any bounded number of policy-authorized
  adapters without duplicate semantic state;
- child inheritance contains no descriptor outside the explicit allowlist on Linux or macOS;
- wrong UID, optional-PID mismatch when available, wrong policy, workspace, operation, generation,
  expiry, revocation, replay, or launch capacity is denied before session authority;
- control/rendezvous loss, partial spawn, early adapter exit, supervisor restart, daemon restart,
  signals, timeouts, and stale artifacts leave no live credential or unowned child;
- accepted-work survival and reconnect semantics are distinct from adapter-process lifetime; and
- the narrow `0600` fallback, if selected for a platform, has a committed distinguishing fault and
  cannot be read twice or reached through a substituted path.

The local same-user design does not claim protection from a fully compromised same-UID process.
Cross-user, network, container-broker, or multi-host deployment reopens identity and transport
design rather than extending these assumptions.

## 6. Suite, implementation, and proof consequences

The successor SUITE/LIFE/SRV/RM transaction must name `WorkspaceSupervisor`, `AgentLaunchPolicy`,
the private rendezvous, `AgentStdioLauncher`, and their lifecycle/authority ownership. The runtime
topology statement remains one central Rust daemon per workspace and one presentation-only FastMCP
process per agent; the Rust supervisor/launcher are lifecycle shells and own no independent CPG,
semantic catalog, Arrow relation, query scheduler, or durable fabric state.

Use a suite-version-neutral `CompiledSemanticRelease` with immutable `SuiteIdentity` instead of
`CodeFabricV21Release`. A newly selected suite changes the identity value and compiled constructor
set without making callers or operational inputs version-specific.

WP33 independently specifies and fixtures the policy, singleton, launch, inheritance, launcher,
restart, revocation, and attach-only behavior before implementation. WP37 implements them in the
same root package and proves the real supervisor -> daemon plus launcher -> installed adapter path.
The existing four WP37 substantive oracles remain the completion set; the focused
`supervisor-launch-contract-check` and `supervisor-launch-platform-check` are mandatory edit-local
and final-gate controls feeding those oracles, not additional packet-completion categories.

Reopen design only if safe allowlisted child-descriptor delivery and its bounded fallback both fail
on a supported platform, a deployment cannot provide operator-owned policy authority, or the
one-supervisor/one-daemon topology cannot provide joined process ownership. Do not answer any of
those by allowing adapter-declared claims, per-agent daemons, reusable credentials, live v1
operability, or Python semantic authority.

## 7. Principle alignment

- P1--P3: semantic authority remains compiled in Rust; launch policy and lifecycle each have one
  explicit owner.
- P4--P5: suite, workspace, supervisor, daemon generation, principal, session, query, and resource
  identities remain distinct behind typed boundaries.
- P7--P8: every adapter consumes the one shared workspace fabric and canonical resource plane.
- P9--P10: grants and sessions bind provenance/closure without pretending their digests prove
  semantic correctness.
- P11--P12: queries pin immutable epochs and reconnect without mixed or implicit latest state.
- P16--P17: singleton ownership, bounded subprocesses, revocation, and joined shutdown are explicit.
- P18--P22: operational policy is accountable Class 1 authority; acceptance fixtures remain
  independent and decoded.
- P23--P25: the same runtime and daemon serve every agent; only transport/presentation differ.
- P27--P31: unknown/loss states are explicit and security dependencies fail closed.
- P32--P36: lifecycle ports, restart generations, ownership, and measured resource bounds are
  first-class rather than ambient launch-script behavior.
