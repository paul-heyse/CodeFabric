---
artifact: library-capability-research
topic: sccache agent build acceleration
version: v1
date: 2026-08-29
status: superseded
superseded_by: docs/library_ref/sccache_for_Rust_Advanced_Configuration.md
supersession_reason: Rust 0.17 basedirs and global incremental assumptions were corrected
---

# sccache agent build acceleration

## Decision Summary

Keep sccache as a hard repository prerequisite. Replace reliance on its default
localhost TCP auto-start behavior with one supervised, pre-started per-user service over
an exact Unix-domain socket (UDS), and use sccache 0.17.0 client-side mode for normal
local compilation. Codex should receive access to that socket and read-only access to the
dedicated cache through a least-privilege permission profile.

For CodeFabric, use a dedicated 40 GiB local cache, disable Cargo incremental compilation
for cache-bearing builds, normalize known checkout/worktree roots, pin the CI sccache
binary to 0.17.0, and make cache effectiveness a checked condition rather than a status
line. Do not enable silent compiler fallback when the cache service is unavailable.

This is a supported-context contract, not a claim that every rustc invocation can be
cached. Link-producing crates, incremental invocations, Miri/tool-owned wrappers, stdin
compilation, and some other compiler forms are documented sccache exclusions.

## Capability Requirements

- Raw `cargo`, `just`, Bacon, human shells, fresh non-interactive agent shells, stable
  root, dated-nightly extractor, and sidecar commands must resolve the same wrapper.
- Codex must reach the cache without general localhost or arbitrary UDS access.
- The daemon must already exist before a sandboxed compiler starts; sandboxed auto-start
  is not a reliable lifecycle mechanism.
- Concurrent builds and worktrees must share exactly one local-cache owner.
- Cache hits must be possible across isolated target directories and, when registered,
  across checkout roots.
- CI must retain the GitHub Actions backend without inheriting workstation-only paths.
- Service failure, cache saturation, wrong version, disabled client-side mode, missing
  path normalization, cache errors, and a failed second-run hit probe must be observable.

## Environment and Version Baseline

- Host: macOS arm64, 258 GiB free at review time.
- Rust: repository stable toolchain 1.98.0; dated-nightly extractor is a separate Cargo
  root.
- sccache: 0.17.0 from `~/.cargo/bin/sccache`.
- Codex CLI inspected: 0.151.0; local permission profiles are documented as beta.
- Review-start repository: `.cargo/config.toml` committed `rustc-wrapper = "sccache"`;
  CI already set `CARGO_INCREMENTAL=0` and `SCCACHE_GHA_ENABLED=true`.
- Local authority: `tooling-ref §13`, especially §13.1 through §13.7.

## Current Code and Custom Infrastructure

At review start, the repository proved only that `sccache` was installed and reported
aggregate statistics. The active daemon used the default TCP endpoint `127.0.0.1:4226`,
the default macOS cache, the default 10 GiB ceiling, server-side compilation, and no
`basedirs`.

At observation time the cache held 10,709,016,844 of 10,737,418,240 bytes. Advanced
statistics showed Rust 252 hits and 949 misses (about 21% over cacheable Rust outcomes),
while the bootstrap's 60.93% aggregate headline was raised by 1,017 assembler/C-family
hits. The aggregate percentage therefore did not describe Rust effectiveness.

The Codex workspace sandbox denied the default TCP connection and also denied sccache's
auto-start child process. `SCCACHE_NO_DAEMON=1` did not remove IPC; it only kept a newly
started server in the foreground. Bypassing `RUSTC_WRAPPER` made the same Rust command
work, which isolated sccache lifecycle/transport as the failure rather than compilation.

## Candidate Matrix

| Candidate | Sandbox-safe | Concurrent local ownership | Performance posture | Decision |
|---|---:|---:|---:|---|
| Default TCP plus auto-start | No | One owner | Existing behavior | Reject |
| Make `rustc-wrapper` optional | Yes | N/A | Loses shared reuse | Reject |
| One daemon per worktree on one disk cache | Transport-dependent | No; upstream warns of races | Unreliable | Reject |
| One supervised UDS daemon, server-side mode | Yes with exact socket rule | Yes | Avoids TCP denial but retains daemon bottleneck | Superseded |
| One supervised UDS daemon, client-side mode | Yes with exact socket rule | Yes | Upstream-recommended 0.17.0 path | Adopt locally |
| GHA backend through pinned sccache action | CI only | Managed by job | Already aligned to CI | Retain and pin |

## Verified Capability Findings

1. The local tooling reference correctly requires a committed wrapper and measured hit
   rate, but does not specify sandbox-safe IPC or daemon supervision.
2. sccache's default client/server connection is TCP on localhost. `SCCACHE_SERVER_UDS`
   selects a UDS, `SCCACHE_IDLE_TIMEOUT=0` keeps a server alive, and
   `SCCACHE_NO_DAEMON=1` controls foregrounding rather than eliminating the server.
3. sccache 0.17.0 introduced `SCCACHE_CLIENT_SIDE=1` / `client_side_mode = true` as the
   recommended architecture. Compilation and hashing stay in each CLI process; the
   daemon remains the storage gateway and statistics owner.
4. Client-side local-cache hits may return a disk path through `StorageGetPath`; the
   sandboxed client therefore needs read access to the cache, while the daemon alone
   needs write access.
5. An executable probe pre-started an isolated sccache server on a UDS outside the Codex
   sandbox. Two direct Rust compilations inside `codex sandbox -P :workspace` with only
   that socket allowed produced one Rust miss followed by one Rust hit and zero cache
   errors. This proves both exact-socket transport and client-side cache retrieval.
6. `SCCACHE_BASEDIRS`/`basedirs` strips absolute checkout prefixes and is the upstream
   mechanism for cross-worktree hits. The daemon owns this metadata; newly created
   worktrees are not normalized until the supervised service configuration is refreshed.
7. Upstream requires Rust incremental compilation to be disabled for cacheability and
   documents link-producing and other non-cacheable compiler forms.
8. Local disk storage supports one server per cache. A dedicated CodeFabric daemon/cache
   can coexist with another default sccache daemon without sharing storage and racing.

## Library Decisions

- **Adopt sccache 0.17.0 client-side mode locally.** It is the upstream-recommended
  architecture and avoids funneling all compiler work through the daemon.
- **Use a dedicated CodeFabric cache and UDS.** Keep machine-specific paths out of Cargo
  configuration; generate them during workstation setup.
- **Keep the wrapper fail-closed.** Do not set `SCCACHE_IGNORE_SERVER_IO_ERROR`; a build
  that silently stops using the mandatory accelerator hides environmental drift.
- **Set normal cache-bearing Cargo builds to non-incremental.** A separate, explicitly
  named human hot-loop command may be benchmarked later, but it must not redefine agent
  or gate behavior.
- **Retain the GHA backend in CI and pin `with.version: v0.17.0`.** The action commit pin
  does not by itself pin the downloaded sccache binary.
- **Do not add a remote workstation cache yet.** No cross-host reuse requirement or
  credential/lifecycle design justifies it.

## Custom Code Displaced or Retained

Retain only thin repository integration:

- a compiler wrapper that selects the preconfigured local UDS outside CI, selects the GHA
  path in CI, enables client-side mode, and emits an actionable service error;
- a cross-platform service setup/status command (launchd on macOS, systemd user service
  on Linux) that generates machine-local configuration without committing absolute paths;
- cache doctor and hit-probe recipes; and
- bootstrap context that reports Rust-specific effectiveness and configuration health.

Do not implement cache storage, eviction, compiler hashing, daemon auto-start, or a custom
IPC proxy. Those remain sccache responsibilities.

## Upgrade/Migration

1. Add the wrapper and make `.cargo/config.toml` reference its config-relative path.
2. Add service setup/status commands and generate a dedicated 40 GiB cache, permanent
   supervised daemon, stable UDS, and current worktree `basedirs`.
3. Add the exact UDS and cache-read grants to the user's CodeFabric Codex permission
   profile; do not allow all Unix sockets.
4. Set non-incremental behavior for normal repository Cargo paths and remove contrary
   documentation.
5. Pin CI to sccache 0.17.0 and enable client-side mode unless CI error logging forces the
   documented server-side fallback.
6. Replace aggregate-only telemetry with advanced JSON checks and a two-run cache canary.
7. Start the new service, verify the canary from host and Codex sandbox, then let the old
   default cache age out. Do not run two servers against the same disk cache.

Rollback is to point the committed wrapper back to the prior endpoint and service, not to
remove the hard wrapper. Cache data is disposable and can be left in place during rollback.

## Risks/Open Validation

- A clean controlled CodeFabric timing comparison was intentionally deferred because an
  unrelated full DataFusion/delta-rs proof build was consuming the host during review.
  Upstream reports a workstation benefit for client-side mode, and the functional cache
  hit is proved, but this report does not claim a CodeFabric wall-clock improvement yet.
- The 40 GiB default is host-specific capacity policy: it is justified on this workstation
  by a saturated 10 GiB cache and 258 GiB free, and must remain configurable elsewhere.
- Active worktree registration needs a safe refresh point. Missing registration reduces
  cross-worktree reuse but does not affect correctness or within-worktree caching.
- Codex permission profiles are beta. The exact-socket sandbox probe must be rerun after
  Codex upgrades, and the profile must not use the broad all-UDS escape hatch.
- Client-side mode is ignored when `SCCACHE_ERROR_LOG` or distributed compilation is in
  use; the doctor must flag either condition instead of assuming the mode is active.

## Recommended Design Integration

Treat sccache as a small local build service with a repository-owned contract:

```text
Cargo -> repo sccache wrapper -> exact per-user UDS -> supervised sccache 0.17 daemon
              |                         |
              |                         +-> dedicated 40 GiB disk cache (daemon writes)
              +-> client-side hash/compile and direct cache reads

Codex profile: workspace access + exact UDS allow + cache read only
CI: same committed wrapper -> pinned 0.17.0 GHA backend, no workstation path
```

The operational success condition is not merely “sccache is installed.” It is: correct
version; reachable supervised endpoint; client-side mode active; non-incremental Cargo;
cache not saturated; no cache errors/timeouts; known checkout roots normalized; and a
second identical isolated compile recorded as a Rust hit.

## Evidence/Reproduction Commands

```bash
just lib-outline docs/library_ref/rust_development_environment_tooling_agent_reference_2026-08-19.md \
  --match 'sccache|Sccache|Compilation cache' --view expanded
sccache --version
sccache --show-adv-stats --stats-format json
lsof -nP -iTCP:4226 -sTCP:LISTEN
git worktree list --porcelain
codex sandbox -P :workspace -C /Users/paulheyse/CodeFabric \
  --allow-unix-socket /absolute/path/to/server.sock -- env \
  SCCACHE_SERVER_UDS=/absolute/path/to/server.sock SCCACHE_CLIENT_SIDE=1 \
  sccache "$(rustup which rustc --toolchain stable)" -vV
```

Primary external references:

- [sccache 0.17.0 release](https://github.com/mozilla/sccache/releases/tag/v0.17.0)
- [sccache configuration](https://github.com/mozilla/sccache/blob/v0.17.0/docs/Configuration.md)
- [sccache architecture](https://github.com/mozilla/sccache/blob/v0.17.0/docs/Architecture.md)
- [sccache Rust limitations](https://github.com/mozilla/sccache/blob/v0.17.0/docs/Rust.md)
- [sccache local storage](https://github.com/mozilla/sccache/blob/v0.17.0/docs/Local.md)
- [Cargo configuration paths and rustc-wrapper](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper)
- [Codex permission profiles](https://learn.chatgpt.com/docs/permissions)
