# `notify-debouncer-full` in Rust — advanced technical reference

Modeled after the supplied DataFusion advanced-reference style: an exhaustive section map followed by self-contained deep dives that combine mental models, released API contracts, executable Rust patterns, production guidance, failure modes, agent invariants, anti-patterns, and checklists.

> **Release anchor (2026-08-18):** this document targets the latest stable `notify-debouncer-full` release, **0.7.0**, and its direct `notify` dependency line, **8.2.0**. `notify-debouncer-full` 0.8.0 release candidates exist, but this guide deliberately does not silently target prerelease APIs. See §34 for upgrade policy.

---

## Version / source anchors

Use version-pinned release docs when writing deployable code. The stable crate is `notify-debouncer-full 0.7.0`; its crate metadata declares `notify ^8.2.0`, `notify-types ^2.0.0`, `file-id ^0.2.3`, `walkdir ^2.4.0`, and optional channel integrations. The crate's current documented MSRV policy names Rust 1.85 for this release line. [S1][S2]

Primary source-of-truth order used here:

1. **docs.rs for `notify-debouncer-full 0.7.0`** — exact released signatures and source.
2. **docs.rs for `notify 8.2.0`** — underlying watcher/event/config semantics.
3. **released crate source on docs.rs** — implementation details where public prose is insufficient, especially timing, queue coalescing, rename stitching, error delivery, and shutdown behavior.
4. **notify-rs repository documentation** — architecture and release context, but only where it agrees with the pinned stable release.

Do not use examples from `main` or the 0.8 release candidates as if they were guaranteed to compile against 0.7.0.

---

## Feature inventory: what this reference covers

The practical capability surface naturally divides into:

* crate installation and version policy;
* `new_debouncer` and `new_debouncer_opt` construction;
* `Debouncer<T, C>` lifecycle and watch management;
* `DebounceEventHandler` callback/channel integration;
* `DebounceEventResult`, `DebouncedEvent`, and underlying `notify::Event` semantics;
* debounce timeout and tick scheduling;
* per-path event queues, coalescing, event ordering, and duplicate suppression;
* rename matching via tracker IDs and optional filesystem file IDs;
* `RecommendedCache`, `FileIdMap`, `NoCache`, and custom `FileIdCache` implementations;
* recursive/non-recursive watches, multiple roots, and unwatch behavior;
* rescan/overflow semantics and authoritative reconciliation;
* `notify::Config`, runtime configuration, symlink behavior, and polling fallback;
* native platform backends: inotify, FSEvents, ReadDirectoryChangesW, kqueue;
* network/pseudo filesystems and `PollWatcher`;
* synchronous callbacks, channels, Tokio bridging, backpressure, and work queues;
* filtering and ignore policy;
* editor atomic-save patterns and path-state validation;
* code-indexing / code-property-graph integration;
* testing, observability, performance, large repositories, shutdown, security, and upgrades.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and mental model

Define `notify-debouncer-full` as a semantic normalization/debouncing layer on top of `notify`, not a filesystem database, durable journal, guaranteed event log, or async runtime. Establish the pipeline from kernel/backend event to application work.

## 1) Installation, crate selection, and project layout

Pin stable versions; explain the `notify` re-export, optional channel/serde/macOS features, workspace dependency policy, MSRV, and when a direct `notify` dependency is justified.

## 2) First executable Rust watcher

Canonical callback and channel examples, recursive watch registration, lifetime requirements, and the minimum correct production skeleton.

## 3) Core type system and vocabulary

`Debouncer`, `DebouncedEvent`, `DebounceEventResult`, `DebounceEventHandler`, `Event`, `EventKind`, `RecursiveMode`, `WatcherKind`, `RecommendedWatcher`, `RecommendedCache`.

## 4) Debounce timing semantics

Precisely distinguish `timeout` from `tick_rate`; explain eligibility latency, default tick selection, continuous streams, and why this is not a pure trailing-edge quiet-period debounce.

## 5) Event queueing, coalescing, and ordering

Per-path queues, same-kind replacement among expired events, duplicate create suppression, post-create modify suppression, and chronological inter-path sorting while preserving same-path order.

## 6) Rename normalization

`RenameMode::{From,To,Both,Any}`, tracker matching, file-ID matching, chained renames, path rewriting, target overrides, move-in/move-out behavior, and the two-path normalized rename contract.

## 7) File-ID caches

`RecommendedCache`, `FileIdMap`, `NoCache`, `FileIdCache`; platform defaults; recursive cache scans; rescan behavior; memory/IO tradeoffs; custom-cache contract.

## 8) Watch roots and lifecycle

`watch`, `unwatch`, multiple roots, recursive modes, root cache management, deprecated `.watcher()`/`.cache()`, and scope design.

## 9) `DebounceEventHandler` and delivery model

Closure, custom handler, `std::sync::mpsc`, optional crossbeam/flume; handler thread semantics; channel disconnect behavior; fast-handler rule.

## 10) `notify::Event` and event taxonomy

Event hierarchy, paths, attrs, `EventKind`, `ModifyKind`, create/remove kinds, tracker/info/source/flags, and portability limits.

## 11) Rescan and event-loss recovery

`need_rescan`, dropped-event semantics, immediate cache rescan vs delayed application notification, authoritative tree reconciliation, and recovery state machines.

## 12) Error model and failure handling

Construction/watch/config/runtime errors, `DebounceEventResult::Err(Vec<Error>)`, batched errors, handler delivery, path context, classification, retry boundaries.

## 13) `notify::Config` and custom construction

`new_debouncer_opt`, polling interval, content comparison, symlink following, live `configure`, configuration immutability, backend-dependent options.

## 14) Backend selection and portability

`RecommendedWatcher`, `WatcherKind`, compile-time platform aliases, Linux/macOS/Windows/BSD behavior, and capability-based rather than event-shape-based programming.

## 15) Linux / inotify deep dive

Recursive watch cost, inotify watch limits, rename cookies, parent deletion behavior, queue overflow, large directory reliability, deployment sysctls.

## 16) macOS / FSEvents and kqueue

Default FSEvents path, optional kqueue feature, file ownership/security caveat, file-ID cache value, and polling fallback.

## 17) Windows / ReadDirectoryChangesW

Recommended backend, rename/file-ID correlation, directory roots, path semantics, and operational fallback.

## 18) PollWatcher and difficult filesystems

NFS/WSL/pseudo filesystems, poll interval, `compare_contents`, performance, automatic polling with full debouncer, and why manual polling is awkward in 0.7.0.

## 19) Filtering and ignore policy

Application-level path/event filtering, extensions/globs/gitignore, early vs late filtering, generated/build trees, access-event handling, and correctness hazards.

## 20) Async/Tokio integration

Bridge the synchronous handler into async consumers without blocking; unbounded vs bounded channels, task ownership, cancellation, and shutdown.

## 21) Backpressure and work scheduling

Decouple notification ingestion from expensive parsing/indexing; dirty sets, generation counters, superseding work, batching, and overload recovery.

## 22) Editor saves and atomic replacement

Why one user save produces create/modify/rename/remove bursts; semantic invalidation rather than over-trusting exact event kind; state-after-event verification.

## 23) Path normalization, symlinks, and identity

Lexical vs canonical paths, symlink following, rename identity, case sensitivity, non-UTF8 paths, and stable internal keys.

## 24) Code-intelligence / property-graph integration

Recommended event → dirty-file → stat/hash/read → parse → graph-delta pipeline; rename optimization; rescan barrier; atomic graph commits.

## 25) Hot reload / build / process-restart patterns

When to restart, when to incrementally update, coalesced rebuild scheduling, process cancellation, and separation from watchexec-like orchestration.

## 26) Large repositories and performance tuning

Watch-root selection, timeout/tick tradeoffs, file-ID cache overhead, ignored directories, channel pressure, and measurement plan.

## 27) Multiple watchers and heterogeneous backends

Separate native/poll watchers, same sink, watch-class isolation, root sharding, and fallback architectures.

## 28) Observability and diagnostics

Raw vs debounced counters, latency, queue/work depth, rescan counts, errors, backend kind, trace logging, and health signals.

## 29) Testing and correctness

Tempdir integration tests, event-set assertions, timing tolerance, rename tests, atomic-save tests, overflow/rescan simulation boundaries, cross-platform CI.

## 30) Shutdown and resource lifecycle

Drop behavior, `stop`, `stop_nonblocking`, handler/thread ownership, channel closure, deterministic service shutdown.

## 31) Security and resource governance

Untrusted watch roots, symlink traversal, path disclosure, watch-limit exhaustion, poll amplification, file-ID scan cost, and privilege boundaries.

## 32) API design wrapper patterns

Build a stable application facade around the crate; normalize internal events, expose invalidation semantics, keep backend-specific details private.

## 33) Troubleshooting cookbook

No events, too many events, duplicate work, missing rename, `ENOSPC`, network mounts, macOS ownership, delayed delivery, shutdown hangs, unexpected paths.

## 34) Upgrades, prereleases, and compatibility

Stable 0.7.0 vs 0.8 RCs, direct `notify` coupling, deprecations, event serialization, upgrade compile/test matrix.

## 35) Comparison with adjacent crates

Bare `notify`, `notify-debouncer-mini`, `watchexec`, direct inotify, polling, and when full debouncing is or is not the right abstraction.

## 36) Production deployment patterns

CLI, daemon, IDE/indexer, hot-reload server, desktop app, container, multi-workspace service.

## 37) Best-practice rules

Condensed engineering rules for robust deployment.

## 38) Anti-pattern inventory

Cross-cutting mistakes that most often cause correctness or performance problems.

## 39) Production checklist

End-to-end implementation and review checklist.

## 40) API quick reference and canonical recipes

Compact signatures, event predicates, rename extraction, Tokio bridge, dirty-set worker, reconciliation, and shutdown recipes.

---

# Suggested reading / implementation order

1. **§0–4:** mental model, installation, first app, vocabulary, timing.
2. **§5–8:** normalization, renames, caches, watch lifecycle.
3. **§9–13:** delivery, event taxonomy, recovery, errors, configuration.
4. **§14–18:** platform/backends and polling fallbacks.
5. **§19–24:** filtering, async, backpressure, editor behavior, paths, code indexing.
6. **§25–30:** operational patterns, performance, observability, testing, shutdown.
7. **§31–40:** security, wrapper design, troubleshooting, upgrades, deployment, checklists, recipes.

---

# `notify-debouncer-full` Advanced — 0) Scope, versioning, and mental model

## 0.0 Version anchors and documentation stance

This reference is anchored to:

```toml
[dependencies]
notify-debouncer-full = "=0.7.0"
```

The released 0.7.0 crate depends on `notify ^8.2.0`. It re-exports `notify`, so ordinary applications do **not** need a direct `notify` dependency merely to name `Event`, `EventKind`, `RecursiveMode`, or `Config`. Add `notify` directly only when Cargo feature selection or an independent raw watcher requires it. [S1][S2]

There are 0.8.0 release candidates published, but `0.7.0` remains the stable release as of this document's anchor date. Treat prerelease examples as a separate migration target, not as “latest stable.” [S2]

**Agent rule:** every code-generation task must first identify the pinned `notify-debouncer-full` version. APIs around watcher control, watch modes, feature flags, and prerelease branches can drift.

---

## 0.1 Identity: what the crate is / is not

**Definition:** `notify-debouncer-full` is a **stateful semantic debouncing and normalization layer** over `notify` filesystem events. It delays raw events, maintains per-path queues, suppresses selected redundant events, stitches related rename notifications when possible, updates queued paths across renames, and emits batches of `DebouncedEvent`. [S1][S3]

| Axis | It gives you | You still provide |
|---|---|---|
| Event source | `notify` native/poll backends | Filesystem itself; backend reliability |
| Debouncing | time-delayed per-path queues | Application work scheduling/backpressure |
| Normalization | rename stitching, duplicate suppression | Domain-level meaning of “changed” |
| Identity | tracker/file-ID-assisted rename correlation | Stable content/entity IDs for your product |
| Delivery | callback/channel batches | Async bridge, queue capacity, retry policy |
| Recovery | `need_rescan` signal + cache refresh | Authoritative full-tree/application reconciliation |
| Filtering | underlying event classifications | Ignore rules, globs, extension policy, tenant policy |
| Persistence | none | Durable journal/state, graph/database transaction |

Do **not** describe the debouncer as:

* a guaranteed filesystem transaction log;
* a complete substitute for rescanning;
* a “one save = exactly one event” API;
* a durable queue;
* a Tokio-native async stream;
* a content-change detector unless you deliberately use `PollWatcher` content comparison.

---

## 0.2 Value case: why use the full debouncer

Use it when raw backend events contain more detail/noise than the application should react to directly and when rename semantics matter.

High-value leverage:

| Need | Value |
|---|---|
| Re-index source files | avoid repeated parse work for redundant save events |
| Preserve rename semantics | produce a single normalized `RenameMode::Both` when source/target can be matched |
| IDE/workspace watch | update queued paths if a file is renamed before queued events emit |
| Directory delete | avoid a separate emitted remove for every child on inotify |
| Cross-platform application | inherit `notify` backend selection while consuming one event model |
| Hot reload | batch short event bursts before triggering reload/rebuild |
| Code graph | use events as invalidation signals while reconciling authoritative current state |

The crate explicitly advertises single matched rename events, merged renames, path updates across queued rename events, optional file-ID stitching, single directory removal on inotify, no duplicate creates, and suppression of modify events immediately after create. [S1]

---

## 0.3 Canonical pipeline

```text
filesystem mutation
    ↓
OS notification facility
    ├─ Linux: inotify
    ├─ macOS: FSEvents (recommended by default) / kqueue
    ├─ Windows: ReadDirectoryChangesW
    └─ fallback: PollWatcher
    ↓
notify::Watcher backend
    ↓
raw Result<Event, notify::Error>
    ↓
DebounceDataInner
    ├─ per-path event queues
    ├─ rename candidate state
    ├─ watched roots
    ├─ optional FileIdCache
    ├─ pending rescan event
    └─ pending errors
    ↓                 periodic debouncer thread
eligibility by time  ← tick_rate
    ↓
coalesce/sort expired events
    ↓
DebounceEventHandler
    ↓
Result<Vec<DebouncedEvent>, Vec<notify::Error>>
    ↓
application invalidation / work queue
    ↓
authoritative filesystem read + domain update
```

The raw watcher callback and debouncer emission loop are separate. Raw events are inserted into shared state; a dedicated thread sleeps for `tick_rate`, extracts eligible events/errors, then invokes the user handler. [S3]

---

## 0.4 Minimum vocabulary

| Term | Meaning | Application use |
|---|---|---|
| `Debouncer<T,C>` | guard owning a `Watcher`, debouncer thread, shared queues/cache | long-lived watch service object |
| `DebouncedEvent` | `notify::Event` plus occurrence `Instant` | normalized application input |
| `DebounceEventResult` | `Result<Vec<DebouncedEvent>, Vec<notify::Error>>` | handler payload |
| `DebounceEventHandler` | `Send + 'static` handler with `handle_event` | callback/channel adapter |
| `notify::Event` | kind + paths + attributes | underlying event representation |
| `EventKind` | top-level access/create/modify/remove/other taxonomy | coarse classification |
| `ModifyKind::Name` | rename/move event | rename-aware logic |
| `RecursiveMode` | recursive vs non-recursive root watch | watch scope |
| `RecommendedWatcher` | platform-selected native watcher type | default backend |
| `WatcherKind` | runtime/backend classification | telemetry and diagnostics |
| `FileIdCache` | cache contract for filesystem IDs | rename correlation |
| `RecommendedCache` | platform-selected file-ID cache | default cache policy |
| `FileIdMap` | in-memory path → filesystem-ID map | explicit rename assistance |
| `NoCache` | no-op cache | disable file-ID tracking |
| `need_rescan()` | event-loss indicator | reconciliation barrier |

---

## 0.5 The key correctness boundary: events are hints about state transitions

A filesystem notification is not the authoritative current content of a file. Editors can implement a “save” by truncating and writing, writing a temporary file then renaming it over the destination, deleting/recreating, or other sequences. `notify` explicitly warns that exact event shapes differ between editors. [S8]

Therefore the robust application contract is:

```text
DebouncedEvent
    ↓
identify potentially affected path/entity
    ↓
read current filesystem state
    ├─ exists?
    ├─ metadata?
    ├─ file type?
    ├─ hash/content?
    └─ current canonical ownership/scope?
    ↓
compare with indexed/application state
    ↓
apply idempotent delta
```

For code indexing, `Modify(Data)` should generally mean **“this path is dirty; verify it”**, not **“apply a specific low-level mutation to the graph.”** Rename events are a useful optimization because identity can often be preserved, but content/state verification still wins over blind event replay.

---

## 0.6 LLM-agent invariants

**Invariant 1 — Keep the `Debouncer` alive.**
A watcher guard dropped at the end of setup cannot keep delivering events.

**Invariant 2 — `timeout` is not a guarantee of one event per user action.**
Distinct event kinds can survive; continuous activity can produce periodic eligible batches.

**Invariant 3 — A rescan signal overrides incremental confidence.**
Once `need_rescan()` is observed, the application must reconcile authoritative filesystem state.

**Invariant 4 — Event shapes are backend/editor dependent.**
Generate logic against broad semantic categories plus state verification, not one editor's trace.

**Invariant 5 — Handler work must be cheap.**
Use the handler to enqueue work; parse/index/compile elsewhere.

**Invariant 6 — Renames may be matched, but unmatched move-in/move-out remains possible.**
Do not assume every rename has two paths.

**Invariant 7 — Platform default cache behavior differs.**
Linux/Android/WASM use `NoCache`; other supported targets use `FileIdMap` as `RecommendedCache` in 0.7.0. [S7]

**Invariant 8 — `Drop` signals stop; `stop()` is the deterministic join path.**
Use explicit shutdown in services.

---

## 0.7 Anti-pattern inventory

* Treating debounced events as a durable or lossless journal.
* Assuming one Ctrl+S produces one `Modify(Data)` event.
* Using exact event sequences as a cross-platform API contract.
* Parsing/compiling directly inside the debounce callback.
* Ignoring `need_rescan()`.
* Applying delete/create mutations to a code graph without first checking current state.
* Assuming rename `paths.len() == 2` for every name-related event.
* Canonicalizing a deleted old rename path and treating failure as fatal.
* Dropping `Debouncer` immediately after `watch()`.
* Mixing 0.8 prerelease examples into 0.7.0 code.

---

# `notify-debouncer-full` Advanced — 1) Installation, crate selection, and project layout

## 1.0 Minimal manifest

```toml
[package]
name = "workspace-watcher"
version = "0.1.0"
edition = "2024"

[dependencies]
notify-debouncer-full = "=0.7.0"
```

Ordinary code can import the underlying event types through the re-export:

```rust
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult,
};
```

The crate explicitly documents the `notify` re-export and advises adding `notify` directly only when specific `notify` features need to be selected. [S1]

---

## 1.1 Production version policy

Prefer an exact version in infrastructure components where event behavior is correctness-sensitive:

```toml
[workspace.dependencies]
notify-debouncer-full = { version = "=0.7.0" }
```

Why exact pinning matters here:

* debouncer normalization semantics are part of application correctness;
* `notify` event types and backend implementations evolve;
* platform feature defaults can change;
* prerelease 0.8 APIs already exist;
* integration tests often assert semantic event sets rather than simple compilation.

Commit `Cargo.lock` for applications/CLIs/services. Libraries can expose a semver range, but should test the minimum and intended latest resolution.

---

## 1.2 Direct `notify` dependency: when justified

Default:

```toml
notify-debouncer-full = "=0.7.0"
```

Add direct `notify` when:

* you also construct a separate raw `notify` watcher;
* you deliberately select `notify` feature flags independently;
* another module exposes `notify` types as its own public API and you want an explicit dependency contract.

```toml
notify-debouncer-full = "=0.7.0"
notify = { version = "=8.2.0", features = ["serde"] }
```

Avoid resolving two incompatible `notify` versions into interfaces that exchange `Event` or `Config` values.

---

## 1.3 Feature flags

`notify-debouncer-full 0.7.0` documents these forwarding features: [S1]

| Feature | Purpose |
|---|---|
| `serde` | serialization support through `notify-types` |
| `web-time` | time support through `notify-types` |
| `crossbeam-channel` | `DebounceEventHandler` impl for crossbeam sender |
| `flume` | handler impl for flume sender |
| `macos_fsevent` | forwards macOS FSEvents feature |
| `macos_kqueue` | forwards macOS kqueue feature |
| `serialization-compat-6` | old event serialization compatibility |

Example:

```toml
notify-debouncer-full = { version = "=0.7.0", features = ["crossbeam-channel"] }
```

Do not enable both channel ecosystems “just in case.” Pick the queue used by the application.

---

## 1.4 MSRV stance

The stable crate page currently states an MSRV policy with current MSRV **1.85** for `notify-debouncer-full 0.7.0`. [S2]

Agent rule:

```text
Do not infer MSRV from your local stable compiler.
Read the pinned crate metadata/docs.
Run CI at the project's declared MSRV if library compatibility matters.
```

---

## 1.5 Workspace layout for a code-intelligence service

```text
workspace/
  Cargo.toml
  crates/
    fs-watch/
      src/
        lib.rs
        config.rs
        event.rs
        normalize.rs
        service.rs
        recovery.rs
        metrics.rs
    indexer/
      src/
        dirty.rs
        parse.rs
        graph_delta.rs
        reconcile.rs
    app/
      src/main.rs
  tests/
    watch/
      atomic_save.rs
      rename.rs
      directory_delete.rs
      rescan.rs
```

Recommended boundary:

```text
notify-debouncer-full types
          ↓
       fs-watch
          ↓
application-owned WatchChange enum
          ↓
       indexer
```

Do not let every downstream crate match directly on `notify::EventKind`. A small facade makes future backend/version changes much easier.

---

## 1.6 Application-owned event facade

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchChange {
    Dirty(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    ReconcileRequired,
}
```

This is intentionally lower-detail than `notify::Event`. The application can re-stat/re-read paths and avoid coupling graph semantics to editor/backend-specific distinctions.

---

## 1.7 Build/test commands

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features
cargo test -p fs-watch --test atomic_save
```

For event timing tests, avoid asserting millisecond-exact delivery. Use bounded intervals and semantic sets.

---

## 1.8 Installation checklist

```text
[ ] Pin notify-debouncer-full 0.7.0 for this reference.
[ ] Commit Cargo.lock for binaries/services.
[ ] Use notify_debouncer_full::notify re-export by default.
[ ] Add direct notify only for explicit feature/raw-watcher needs.
[ ] Enable only required channel/serde/macOS features.
[ ] Keep watcher code behind an application facade.
[ ] Test on every production OS/backend.
[ ] Include atomic-save + rename + delete integration tests.
[ ] Document Rust MSRV separately from local toolchain.
```

---

# `notify-debouncer-full` Advanced — 2) First executable Rust watcher

## 2.0 Objective

Prove the complete runtime path:

```text
construct debouncer
  → register watch root
  → keep guard alive
  → mutate filesystem
  → receive Vec<DebouncedEvent>
  → handle errors separately
  → shut down deterministically
```

---

## 2.1 Canonical callback example

```rust
use std::time::Duration;

use notify_debouncer_full::{
    new_debouncer,
    notify::RecursiveMode,
    DebounceEventResult,
};

fn main() -> notify_debouncer_full::notify::Result<()> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        |result: DebounceEventResult| match result {
            Ok(events) => {
                for event in events {
                    println!("{event:?}");
                }
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("watch error: {error:?}");
                }
            }
        },
    )?;

    debouncer.watch(".", RecursiveMode::Recursive)?;

    // Keep the guard alive. Replace with your application's normal main loop.
    std::thread::park();

    #[allow(unreachable_code)]
    Ok(())
}
```

`new_debouncer(timeout, tick_rate, handler)` returns `Debouncer<RecommendedWatcher, RecommendedCache>`. With `tick_rate = None`, the crate selects one quarter of the timeout. [S4]

---

## 2.2 Channel-based example

```rust
use std::{sync::mpsc, time::Duration};

use notify_debouncer_full::{
    new_debouncer,
    notify::RecursiveMode,
    DebounceEventResult,
};

fn main() -> notify_debouncer_full::notify::Result<()> {
    let (tx, rx) = mpsc::channel::<DebounceEventResult>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        tx,
    )?;

    debouncer.watch("src", RecursiveMode::Recursive)?;

    for result in rx {
        match result {
            Ok(events) => {
                for event in events {
                    println!("{event:?}");
                }
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("{error:?}");
                }
            }
        }
    }

    Ok(())
}
```

`std::sync::mpsc::Sender<DebounceEventResult>` implements `DebounceEventHandler` directly. [S3]

---

## 2.3 Custom handler object

```rust
use notify_debouncer_full::{DebounceEventHandler, DebounceEventResult};

struct EventPrinter {
    source: &'static str,
}

impl DebounceEventHandler for EventPrinter {
    fn handle_event(&mut self, result: DebounceEventResult) {
        match result {
            Ok(events) => {
                for event in events {
                    println!("[{}] {event:?}", self.source);
                }
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("[{}] {error:?}", self.source);
                }
            }
        }
    }
}
```

The trait requires `Send + 'static` and a mutable `handle_event` method. Closures implementing `FnMut(DebounceEventResult) + Send + 'static` also satisfy it. [S3]

---

## 2.4 Multiple roots

```rust
use notify_debouncer_full::notify::RecursiveMode;

// One recursive source tree.
debouncer.watch("src", RecursiveMode::Recursive)?;

// One non-recursive config directory.
debouncer.watch("config", RecursiveMode::NonRecursive)?;
```

The `Debouncer` automatically tracks registered roots and updates its file-ID cache as `watch`/`unwatch` are called; old patterns that manually manipulated `.watcher()` or `.cache()` are deprecated in 0.7.0. [S5]

---

## 2.5 Deterministic shutdown skeleton

```rust
use std::time::Duration;
use notify_debouncer_full::{new_debouncer, notify::RecursiveMode};

fn run() -> notify_debouncer_full::notify::Result<()> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        |result| println!("{result:?}"),
    )?;

    debouncer.watch("src", RecursiveMode::Recursive)?;

    // ... application blocks/runs here ...

    // Consumes guard, signals stop, joins debounce thread.
    debouncer.stop();
    Ok(())
}
```

`stop()` can wait up to roughly one `tick_rate` for the debouncer thread's sleeping loop to wake and exit. `stop_nonblocking()` signals without joining. [S5]

---

## 2.6 First-app production advisories

* Use 50–200 ms as an **initial measurement range**, not a universal constant.
* Handler: enqueue lightweight application messages; do not parse a repository inline.
* Preserve both success and error arms.
* Keep the guard in the owning service struct.
* Have an explicit shutdown path.
* For indexers, turn event batches into a dirty-path set before scheduling parsing.
* Test the actual editors/build tools that mutate your target tree.

---

## 2.7 First-app anti-patterns

* Creating the debouncer in a helper and returning without returning the guard.
* `unwrap()` on watch registration in a long-lived service.
* `std::thread::sleep(Duration::from_secs(u64::MAX))` as service lifecycle management.
* Expensive synchronous callback logic.
* Ignoring the `Err(Vec<Error>)` arm.
* Assuming a 100 ms timeout means exactly 100 ms delivery.
* Watching the entire repository including `target/`, `.git/`, generated caches, and dependency trees without need.

---

# `notify-debouncer-full` Advanced — 3) Core type system and vocabulary

## 3.0 Type-stack mental model

```text
notify backend
  └─ Result<notify::Event, notify::Error>
        ↓
notify-debouncer-full state
        ↓
DebouncedEvent
  ├─ event: notify::Event
  └─ time: Instant
        ↓
Vec<DebouncedEvent>
        ↓
DebounceEventResult
  = Result<Vec<DebouncedEvent>, Vec<notify::Error>>
        ↓
DebounceEventHandler
```

---

## 3.1 `Debouncer<T, C>`

Conceptual signature:

```rust
pub struct Debouncer<T: Watcher, C: FileIdCache> {
    // private
}
```

Released 0.7.0 owns:

```text
watcher: T
thread: debouncer emission loop
data: shared queues/cache/root state
stop: atomic stop flag
```

The public surface includes instance methods `watch`, `unwatch`, `configure`, `stop`, and `stop_nonblocking`, plus the associated function `kind()`. [S5]

Decision table:

| Method | Effect | Blocking? |
|---|---|---:|
| `watch(path, mode)` | register backend watch + root/cache state | backend dependent |
| `unwatch(path)` | unregister + remove root/cache state | backend dependent |
| `configure(config)` | delegate runtime config change | usually short |
| associated `kind()` | backend kind for `T` | no |
| `stop()` | signal + join debouncer thread | up to tick/scheduling |
| `stop_nonblocking()` | signal only | no join |

---

## 3.2 `DebouncedEvent`

```rust
pub struct DebouncedEvent {
    pub event: notify::Event,
    pub time: std::time::Instant,
}
```

It dereferences to `notify::Event`, so you can write:

```rust
for e in events {
    println!("kind={:?} paths={:?} age={:?}", e.kind, e.paths, e.time.elapsed());

    if e.need_rescan() {
        println!("reconciliation required");
    }
}
```

`time` is the occurrence timestamp captured by the debouncer, not the delivery time. [S6]

---

## 3.3 `DebounceEventResult`

```rust
pub type DebounceEventResult =
    Result<Vec<DebouncedEvent>, Vec<notify::Error>>;
```

Two important consequences:

1. success delivery is **batched**;
2. errors are also **batched**, but separately from events.

The internal loop can invoke the handler once with `Ok(events)` and then again with `Err(errors)` in the same tick when both exist. [S3]

Do not design a handler assuming “one callback invocation equals one debounce cycle with either all data or all errors.”

---

## 3.4 `DebounceEventHandler`

```rust
pub trait DebounceEventHandler: Send + 'static {
    fn handle_event(&mut self, event: DebounceEventResult);
}
```

Built-in implementations in 0.7.0 include:

* `FnMut(DebounceEventResult) + Send + 'static`;
* `std::sync::mpsc::Sender<DebounceEventResult>`;
* crossbeam sender with feature enabled;
* flume sender with feature enabled. [S3]

---

## 3.5 `notify::Event`

```rust
pub struct Event {
    pub kind: EventKind,
    pub paths: Vec<PathBuf>,
    pub attrs: EventAttributes,
}
```

Important convenience methods include `need_rescan`, `tracker`, `flag`, `info`, and `source`. [S9]

`paths` can be zero, one, or multiple at the raw `notify` level. The full debouncer skips a raw event with no path unless it was first recognized as a rescan signal. Normalized matched rename events use two paths in `[from, to]` order. [S3][S10]

---

## 3.6 `EventKind`

```rust
pub enum EventKind {
    Any,
    Access(AccessKind),
    Create(CreateKind),
    Modify(ModifyKind),
    Remove(RemoveKind),
    Other,
}
```

Use broad methods/categories when portability matters. Specific subkinds are not uniformly available from every backend. [S11]

---

## 3.7 Type decision table

| If you need... | Use... |
|---|---|
| long-lived watcher ownership | `Debouncer<...>` |
| normalized event | `DebouncedEvent` |
| callback input | `DebounceEventResult` |
| broad mutation classification | `EventKind` |
| rename source/target semantics | `ModifyKind::Name(RenameMode)` |
| overflow/loss detection | `need_rescan()` |
| backend telemetry | `Debouncer::<...>::kind()` |
| default platform backend | `RecommendedWatcher` |
| rename file-ID assistance | `FileIdCache` / `FileIdMap` |

---

## 3.8 Agent checklist

```text
[ ] Keep Debouncer ownership explicit.
[ ] Match DebounceEventResult as batched Ok/Err.
[ ] Use DebouncedEvent.time for occurrence-time diagnostics.
[ ] Treat Event.paths as variable-length.
[ ] Treat EventKind subkind precision as backend-dependent.
[ ] Check need_rescan before trusting incremental completeness.
[ ] Use WatcherKind for telemetry, not for application semantics.
```

---

# `notify-debouncer-full` Advanced — 4) Debounce timing semantics

## 4.0 The most important timing correction

A common mental model of “debounce” is trailing-edge quiet-period logic:

```text
new event → reset timer
new event → reset timer
new event → reset timer
quiet for timeout → emit once
```

That is **not an accurate model of `notify-debouncer-full 0.7.0`'s core expiration loop**.

Each queued event receives its own `Instant`. On each periodic tick, events whose age is at least `timeout` become eligible. Newer events remain queued. Therefore a continuously changing file does not necessarily wait forever for a quiet period; older events can expire and emit while newer ones remain pending. [S3]

---

## 4.1 Timing pipeline

```text
t0: raw event A arrives, timestamp A=t0
                 ↓
t0 + timeout: A becomes eligible
                 ↓
next debouncer tick after eligibility
                 ↓
A may be emitted (subject to coalescing)
```

Approximate latency envelope under normal scheduling:

```text
minimum ≈ timeout
maximum ≈ timeout + tick_rate
```

With `tick_rate = None`:

```text
tick_rate = timeout / 4
```

so ordinary scheduling adds at most approximately 25% of the timeout before the next scan, not counting OS scheduling delays or a slow handler. [S4]

---

## 4.2 Example: isolated change

Configuration:

```text
timeout   = 100 ms
tick_rate = 25 ms (automatic)
```

Possible trace:

```text
0 ms    MODIFY foo.rs
25      scan: age 25  -> keep
50      scan: age 50  -> keep
75      scan: age 75  -> keep
100     scan: age 100 -> emit
```

If the event arrived just after a tick:

```text
1 ms     event
25       age 24
50       age 49
75       age 74
100      age 99
125      age 124 -> emit
```

Thus “100 ms timeout” is an age threshold, not a precise wake deadline.

---

## 4.3 Example: continuous writes

```text
timeout = 100 ms
writes at: 0, 30, 60, 90, 120, 150, ...
```

A pure trailing-edge debounce might emit only after writing stops. The full debouncer can emit expired write-like events periodically because the 0 ms event becomes old enough even while later events exist.

The per-path queue then coalesces duplicate exact `EventKind`s among the expired events for a batch, reducing repeated output without requiring global silence.

---

## 4.4 `tick_rate` validation

`new_debouncer_opt` rejects a configured `tick_rate` greater than `timeout`. When no tick is supplied, it uses `timeout / 4`. [S3]

```rust
let mut debouncer = notify_debouncer_full::new_debouncer(
    std::time::Duration::from_millis(100),
    Some(std::time::Duration::from_millis(20)),
    handler,
)?;
```

Do not choose a zero/near-zero timeout in production merely to “make it instant.” The implementation runs a sleeping periodic thread; extremely small ticks increase wakeups/lock traffic and defeat the purpose of debouncing.

---

## 4.5 Timeout selection by workload

| Workload | Starting range | Rationale |
|---|---:|---|
| interactive source index | 50–100 ms | prioritize freshness, absorb editor burst |
| hot reload | 75–250 ms | avoid repeated restart/build triggers |
| generated config files | 100–500 ms | writes often occur in grouped bursts |
| large build output tree | preferably ignore | debounce is not a substitute for scope reduction |
| remote/poll filesystem | coordinate with poll interval | source cadence may dominate timeout |

These are **engineering starting points**, not crate defaults or guarantees. Measure actual save/build traces and end-to-end queue latency.

---

## 4.6 Timeout vs downstream batching

Keep two independent controls:

```text
filesystem debounce timeout
  → normalize low-level burst

downstream work aggregation window
  → combine dirty paths into one parse/build/index transaction
```

Example:

```text
notify debounce: 75 ms
indexer gather window: 20 ms
worker supersession: generation-based
```

Do not stretch the filesystem debounce to seconds just because the parser benefits from bigger batches. That delays all change awareness and error/rescan reaction.

---

## 4.7 Timing observability

Capture:

```text
raw/backend event timestamp if available
DebouncedEvent.time
handler receipt Instant
work-queue enqueue Instant
work start/end
index commit Instant
```

Derived metrics:

```text
debounce_delivery_latency = handler_receipt - DebouncedEvent.time
queue_wait                = work_start - enqueue
end_to_end_freshness      = index_commit - DebouncedEvent.time
```

If debounce delivery latency is much larger than `timeout + tick_rate`, investigate slow handlers, thread starvation, filesystem backend behavior, or application scheduling.

---

## 4.8 Timing anti-patterns

* Calling it “trailing-edge debounce” in architecture docs.
* Assuming every new event resets a single path timer.
* Setting timeout equal to the maximum acceptable end-to-end index freshness without accounting for downstream work.
* Using huge timeouts to hide an overloaded parser.
* Using tiny timeouts to compensate for a slow downstream queue.
* Asserting exact 100 ms timing in tests.

---

## 4.9 Agent checklist

```text
[ ] Model timeout as per-event age threshold.
[ ] Model tick_rate as scan cadence.
[ ] Default tick = timeout / 4.
[ ] Keep tick_rate <= timeout.
[ ] Allow timeout-to-timeout+tick scheduling latency.
[ ] Separate filesystem debounce from downstream work batching.
[ ] Measure end-to-end freshness, not only callback delay.
```

---

# `notify-debouncer-full` Advanced — 5) Event queueing, coalescing, and ordering

## 5.0 Internal queue model

The released implementation maintains a `HashMap<PathBuf, Queue>` and each queue holds a `VecDeque<DebouncedEvent>`. [S3]

```text
queues
├─ src/a.rs → [Create, Modify(Data), Modify(Metadata), ...]
├─ src/b.rs → [Modify(Data), ...]
└─ src/c.rs → [Rename..., ...]
```

A queue has special ordering assumptions around remove/move-out and rename handling so that path state can be reconstructed correctly during normalization.

---

## 5.1 Eligibility extraction

At each tick:

1. compute current time;
2. handle pending rescan event if old enough;
3. drain path queues;
4. pop expired events from each queue front;
5. keep unexpired remainder;
6. among expired events, replace earlier event with later event of the exact same `EventKind`;
7. globally sort emitted events with path-sensitive ordering. [S3]

This is richer than “group all paths changed in 100 ms.”

---

## 5.2 Duplicate kind coalescing

Suppose `foo.rs` generates:

```text
Modify(Data(Content)) @ 0 ms
Modify(Data(Content)) @ 20 ms
Modify(Data(Content)) @ 40 ms
```

When all three are expired in the same extraction cycle, the kind-index logic removes the previous expired event of the same kind as later ones are encountered, leaving the newest representative for that kind. [S3]

If the events straddle different expiration ticks, they can still be delivered in separate batches.

---

## 5.3 Duplicate create suppression

Once a path queue indicates the path was created, another `Create` is not appended. [S1][S3]

```text
Create foo
Create foo
    ↓
Create foo
```

This helps backends/editor sequences that surface duplicate creation notifications.

---

## 5.4 Modify-after-create suppression

If a queue begins with create semantics, the debouncer suppresses subsequent `Modify` kinds representing content/data/metadata/other before emission. [S1][S3]

```text
Create foo
Modify(Data) foo
Modify(Metadata) foo
    ↓
Create foo
```

Why this is sensible for invalidation systems: creation already tells the consumer that it must read the new current object; immediate modify noise adds no new obligation.

Why it matters for forensic/audit systems: this crate is **not** preserving every raw transition. Use raw `notify` if every event matters.

---

## 5.5 Create-then-remove inside debounce window

The remove handler detects a path queue that was created and removes the queue rather than emitting a create followed by remove. [S3]

Conceptually:

```text
Create temp-file
Remove temp-file
within pending window
    ↓
possibly no application-visible lifecycle for that transient file
```

This is useful for temporary-file churn, but reinforces that the output is a **normalized change signal**, not a lossless audit trail.

---

## 5.6 Directory removal compression

For a remove event at directory path `dir/`, the debouncer drops queued child paths under that directory and keeps/creates the parent removal queue. This implements the documented behavior of emitting one removal for a deleted directory rather than a flood of inotify child removals. [S1][S3]

Application implication:

```text
Removed("src/generated")
```

should invalidate all indexed descendants under that prefix even if no child remove events arrive.

---

## 5.7 Cross-path ordering

The final sorter groups events by their last path, preserves relative event order for a given path, and interleaves different path groups by event time using a minimum-time heap. [S3]

Practical rule:

```text
Ordering is intentionally improved,
but do not build correctness on a total global filesystem transaction order.
```

Multiple OS producers, backend buffering, and event normalization do not constitute serializable filesystem transactions.

---

## 5.8 Queue semantics decision table

| Raw pattern | Likely normalized intent |
|---|---|
| duplicate create | one create |
| create + immediate data modify | create only |
| create + remove before emission | transient lifecycle may vanish |
| many identical modifies | latest representative per eligible batch |
| directory remove + child removes | parent remove dominates |
| rename chain | merged/rewritten when matchable |
| unrelated paths | roughly chronological interleaving |

---

## 5.9 Application consequence: dedupe again at domain level

Even after full debouncing, an indexer should usually create a domain dirty set:

```rust
use std::{collections::BTreeSet, path::PathBuf};

fn dirty_paths(events: &[notify_debouncer_full::DebouncedEvent]) -> BTreeSet<PathBuf> {
    events
        .iter()
        .flat_map(|e| e.paths.iter().cloned())
        .collect()
}
```

Then enrich with rename/remove semantics separately. The crate's event coalescing is not a substitute for job supersession when several debounce batches arrive before parsing completes.

---

## 5.10 Anti-pattern inventory

* Treating debounced output as every raw event.
* Assuming identical modifications can appear only once over an entire save.
* Assuming create-then-remove must be visible.
* Waiting for per-child directory removal events.
* Using callback arrival order as a total transaction order across the filesystem.
* Scheduling one expensive parse task per `DebouncedEvent` instead of deduplicating dirty paths.

---

# `notify-debouncer-full` Advanced — 6) Rename normalization

## 6.0 Rename mental model

Raw backends may represent a rename as:

```text
From(old, tracker=42)
To(new, tracker=42)
```

or as less precise name events. The full debouncer attempts to connect them and emit:

```text
EventKind::Modify(ModifyKind::Name(RenameMode::Both))
paths = [old, new]
```

The underlying `notify` contract specifies that a `Both` rename's paths are ordered `(from, to)`. [S10]

---

## 6.1 Correlation strategy

When a `From` event arrives, the debouncer stores a pending rename candidate and, if available, the source file ID. When a `To` event arrives, it considers the rename matched when either:

```text
source tracker == target tracker
OR
source cached FileId == target cached FileId
```

Then it constructs/merges the normalized rename. [S3]

This gives two correlation mechanisms:

* **backend tracker/cookie** — preferred where available;
* **filesystem identity** — useful where backends do not supply correlatable rename cookies.

---

## 6.2 `RenameMode` meanings

```rust
pub enum RenameMode {
    Any,
    To,
    From,
    Both,
    Other,
}
```

| Mode | Meaning |
|---|---|
| `From` | source path moved/renamed away |
| `To` | target path arrived |
| `Both` | source + target known, `[from,to]` |
| `Any` | name change known but direction/detail unavailable |
| `Other` | known backend-specific rename kind not otherwise represented |

[S10]

---

## 6.3 Chained renames

The crate advertises merging multiple rename events. Its queue logic finds an existing normalized `Both` rename in a path queue, preserves the original source path/time, removes the intermediate rename, updates pending event paths to the final destination, then emits one merged rename where possible. [S1][S3]

Conceptually:

```text
a.rs → b.rs → c.rs
```

can normalize toward:

```text
rename [a.rs, c.rs]
```

with intermediate queued events rewritten to `c.rs`.

This is especially valuable if work is still waiting in the debounce window when the second rename occurs.

---

## 6.4 Target override semantics

If a rename moves a source over an existing target queue, the implementation can synthesize a target `Remove` with `info = "override"` before merging the source queue into the target path. [S3]

Do not make application correctness depend on the literal info string. Use it as diagnostic context; the authoritative result is still the current target filesystem state plus normalized rename/remove semantics.

---

## 6.5 Move-in and move-out

Not every `From` can be matched to a `To` within the watched scope, and vice versa.

Examples:

```text
watched/old.rs → /tmp/old.rs
  = move out; source may behave like disappearance

/tmp/new.rs → watched/new.rs
  = move in; target may behave like creation/arrival
```

Your application must support unmatched directional name events or their normalized create/remove-like consequences.

---

## 6.6 Rename extraction helper

```rust
use std::path::Path;
use notify_debouncer_full::{
    notify::event::{ModifyKind, RenameMode},
    notify::EventKind,
    DebouncedEvent,
};

fn matched_rename(e: &DebouncedEvent) -> Option<(&Path, &Path)> {
    match e.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if e.paths.len() >= 2 => {
            Some((&e.paths[0], &e.paths[1]))
        }
        _ => None,
    }
}
```

Use `>= 2`, not blind indexing based solely on an assumption that all name events have two paths.

---

## 6.7 Rename-aware index update

Preferred flow:

```text
matched rename old → new
    ↓
lookup indexed entity at old
    ↓
stat/read new
    ├─ if same identity/content lineage plausible:
    │      preserve entity ID + update path + reparse if content changed
    └─ otherwise:
           remove old + index new
```

A rename event is an optimization for preserving identity and avoiding unnecessary teardown/recreate. It is not proof that content remained identical.

---

## 6.8 Rename anti-patterns

* Assuming every rename is `Both`.
* Assuming every `Both` rename preserves content.
* Using path string as the only durable entity identity.
* Canonicalizing old path after it has ceased to exist and treating the failure as an event error.
* Ignoring move-in/move-out across watch boundaries.
* Building custom source/target matching when the full debouncer already correlated the rename.
* Depending on rename tracker IDs as stable across processes/restarts.

---

# `notify-debouncer-full` Advanced — 7) File-ID caches

## 7.0 Purpose

A `FileIdCache` maps paths to stable filesystem-provided file IDs so the debouncer can correlate rename source and target when the notification backend does not provide a usable tracker/cookie. [S7]

```text
old path → FileId X
rename happens
new path → FileId X
        ↓
match rename
```

---

## 7.1 Platform default

In released 0.7.0 source: [S7]

```text
Linux / Android / WASM → RecommendedCache = NoCache
other supported targets → RecommendedCache = FileIdMap
```

On Linux, inotify's rename tracking makes the extra file-ID map unnecessary for the recommended path. On macOS FSEvents/Windows, filesystem ID tracking is valuable for rename stitching.

---

## 7.2 `FileIdCache` contract

Conceptual trait:

```rust
pub trait FileIdCache {
    fn cached_file_id(&self, path: &Path) -> Option<impl AsRef<FileId>>;
    fn add_path(&mut self, path: &Path, recursive_mode: RecursiveMode);
    fn remove_path(&mut self, path: &Path);

    fn rescan(&mut self, root_paths: &[(PathBuf, RecursiveMode)]) {
        // default: add each root again
    }
}
```

Important contract: `cached_file_id` should return only cached data. It should **not** go to disk on a cache miss. [S7]

Why: lookup happens inside the debouncer's event-processing critical path.

---

## 7.3 `FileIdMap`

`FileIdMap` stores `HashMap<PathBuf, FileId>`. When `add_path` is called, it uses `WalkDir`; recursive mode can walk the full subtree, and the released implementation uses `follow_links(true)` during this cache scan. [S12]

Consequences:

* initial watch registration can perform O(number of entries) filesystem work;
* newly created directories can trigger subtree cache population;
* rescan after event loss can repeat full walks;
* symlink traversal in the cache walk deserves explicit security/performance review.

---

## 7.4 `NoCache`

`NoCache` implements the trait but retains nothing. [S7]

Use it when:

* platform tracker semantics are adequate;
* rename stitching by file ID is unnecessary;
* tree scans/memory cost are unacceptable;
* your application treats rename as remove+create anyway.

Explicit custom construction:

```rust
use std::time::Duration;
use notify_debouncer_full::{
    new_debouncer_opt,
    notify::{Config, RecommendedWatcher},
    NoCache,
};

let debouncer = new_debouncer_opt::<_, RecommendedWatcher, NoCache>(
    Duration::from_millis(100),
    None,
    handler,
    NoCache::new(),
    Config::default(),
)?;
```

---

## 7.5 Explicit `FileIdMap`

```rust
use std::time::Duration;
use notify_debouncer_full::{
    new_debouncer_opt,
    notify::{Config, RecommendedWatcher},
    FileIdMap,
};

let debouncer = new_debouncer_opt::<_, RecommendedWatcher, FileIdMap>(
    Duration::from_millis(100),
    None,
    handler,
    FileIdMap::new(),
    Config::default(),
)?;
```

On Linux this is not the recommended default; introduce it only with a measured/semantic reason.

---

## 7.6 Custom cache use case

Implement a custom `FileIdCache` if your application already maintains file IDs as part of an authoritative tree index and can offer O(1) cache reads without disk access.

```text
watch service
    ↕
shared tree metadata cache
    ├─ path
    ├─ file ID
    ├─ size/mtime/hash
    └─ domain entity ID
```

Be careful about lock ordering: the debouncer invokes cache methods while holding its internal data mutex. A custom cache that acquires application locks also acquired in the opposite order by callbacks can deadlock.

---

## 7.7 Rescan behavior

When the raw event reports `need_rescan()`, the debouncer immediately calls:

```text
cache.rescan(&root_paths)
```

before storing the rescan event for delayed emission. [S3][S7]

This refreshes **rename correlation cache state**. It does **not** reconcile your code index/database/application model. Application reconciliation is still mandatory.

---

## 7.8 Cache cost model

| Choice | Memory | Startup IO | Rename correlation | Rescan cost |
|---|---:|---:|---|---:|
| `NoCache` | minimal | minimal | tracker only | minimal |
| `FileIdMap` | O(entries) | tree scan | tracker + file ID | tree scan |
| custom existing metadata cache | application-dependent | potentially near-zero incremental | strong if maintained | application-dependent |

---

## 7.9 Cache anti-patterns

* Doing `metadata()`/file-ID syscalls inside `cached_file_id` on every lookup.
* Using `FileIdMap` on a million-entry tree without measuring startup/rescan cost.
* Forgetting that its released walk follows symlinks.
* Assuming `RecommendedCache` is the same concrete type on every OS.
* Treating cache rescan as application-state reconciliation.
* Sharing a custom cache with unsafe lock ordering.

---

# `notify-debouncer-full` Advanced — 8) Watch roots and lifecycle

## 8.0 Root model

Each call to `Debouncer::watch(path, recursive_mode)` first registers the path with the underlying watcher, then records the root and adds it to the file-ID cache. `unwatch` performs the inverse. [S5]

```text
Debouncer
  roots:
    (repo/src, Recursive)
    (repo/config, NonRecursive)
```

---

## 8.1 Recursive vs non-recursive

```rust
use notify_debouncer_full::notify::RecursiveMode;

// Directory and all descendants, including future descendants.
debouncer.watch("src", RecursiveMode::Recursive)?;

// Only this path/directory level according to backend semantics.
debouncer.watch("Cargo.toml", RecursiveMode::NonRecursive)?;
```

For code repositories, prefer the smallest stable roots that cover desired files. Avoid one root at `/` or the user's home directory.

---

## 8.2 Duplicate root registration

The debouncer's internal root list skips duplicate root entries by path after the underlying watcher call succeeds. Do not depend on repeatedly calling `watch` as an idempotent no-op at the backend layer; maintain your own desired-root set when dynamic registration is complex. [S3]

---

## 8.3 Unwatch semantics

```rust
debouncer.unwatch("src/generated")?;
```

The root-removal helper drops matching root entries under the supplied path and removes cache entries under the path. [S5]

Application rule: if you dynamically shrink scope, also cancel/suppress queued downstream work for paths that are no longer authorized/watched.

---

## 8.4 Parent deletion caveat

`notify` documents that to receive the deletion of a watched folder `b`, you may need to watch its parent `/a`, rather than only `/a/b`. [S8]

Design roots around lifecycle events:

```text
need to know if workspace directory itself disappears?
    watch its parent or manage lifecycle separately

need only internal source-file changes?
    watch workspace/src recursively
```

---

## 8.5 Deprecated access patterns

In 0.7.0, `Debouncer::watcher()` is deprecated because the `Debouncer` exposes watcher methods itself, and `Debouncer::cache()` is deprecated because root/cache management is automatic. [S5]

Do not generate legacy code like:

```rust
// obsolete pattern — do not use for 0.7.0
debouncer.watcher().watch(...);
debouncer.cache().add_root(...);
```

Use:

```rust
debouncer.watch(path, RecursiveMode::Recursive)?;
```

---

## 8.6 Owner-service pattern

```rust
use notify_debouncer_full::{
    notify::RecommendedWatcher,
    Debouncer,
    RecommendedCache,
};

pub struct WatchService {
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
}

impl WatchService {
    pub fn shutdown(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }
    }
}
```

Using `Option` makes ownership-consuming shutdown easy and prevents accidental double-stop logic.

---

## 8.7 Root design for a Rust monorepo

```text
watch:
  Cargo.toml
  Cargo.lock
  crates/*/src/**
  crates/*/build.rs
  config/**

usually ignore:
  target/**
  .git/**
  node_modules/**
  generated caches/**
  editor swap/history dirs/**
```

Whether to watch the whole repository root then filter or register narrower roots depends on backend cost and dynamic directory creation. On Linux, recursive watching consumes inotify watches for entries; reducing root breadth can matter materially.

---

## 8.8 Root lifecycle checklist

```text
[ ] Decide whether parent deletion must be observable.
[ ] Register smallest stable root set.
[ ] Choose RecursiveMode deliberately.
[ ] Keep desired-root state in application if roots are dynamic.
[ ] Unwatch paths no longer in scope.
[ ] Cancel downstream work after unwatch if needed.
[ ] Use Debouncer::watch directly; avoid deprecated watcher/cache APIs.
[ ] Own Debouncer in a long-lived service object.
[ ] Shut down explicitly.
```
# notify-debouncer-full Advanced — 9) Handler delivery and output integration

## 9.0 Delivery mental model

```text
native watcher callback
  → DebounceData::add_event(raw Event)
  → per-path queues + rename state + cache updates
  → debounce loop wakes every tick_rate
  → debounced_events()
      → Vec<DebouncedEvent>
  → DebounceEventHandler::handle_event(
        Result<Vec<DebouncedEvent>, Vec<notify::Error>>
    )
  → application-owned handoff
      queue / actor / task / indexer / logger
```

`notify-debouncer-full` deliberately keeps the output boundary small: one callback receives either a batch of debounced events or a batch of errors. The handler must be `Send + 'static`, because the debouncer owns the background processing thread. [S1] [S3]

---

## 9.1 `DebounceEventResult`

```rust
pub type DebounceEventResult =
    Result<Vec<DebouncedEvent>, Vec<notify::Error>>;
```

Important consequences:

* success is a **batch**, not a single event;
* errors are also batched;
* success and error batches are delivered separately by the loop;
* a callback invocation is not guaranteed to correspond to one OS callback;
* event order inside the success vector is normalized by the debouncer's sorting logic. [S1] [S3]

Agent invariant:

```text
One handler call != one filesystem operation.
One DebouncedEvent != necessarily one raw kernel event.
A handler batch = currently eligible normalized events.
```

---

## 9.2 Closure handler: canonical lightweight path

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
};
use std::time::Duration;

let mut debouncer = new_debouncer(
    Duration::from_millis(75),
    None,
    |result: DebounceEventResult| {
        match result {
            Ok(events) => {
                for event in events {
                    println!("{:?} {:?}", event.kind, event.paths);
                }
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("watch error: {error:?}");
                }
            }
        }
    },
)?;
```

Use a closure when work is fast, nonblocking, and does not need a separately testable handler type.

Do **not** put expensive parsing, graph rebuilding, database transactions, or network requests directly in this callback. The handler runs from the debouncer loop thread; blocking it delays subsequent delivery.

---

## 9.3 Custom handler type

```rust
use notify_debouncer_full::{
    DebounceEventHandler,
    DebounceEventResult,
};

#[derive(Default)]
struct WatchHandler {
    delivered_batches: u64,
}

impl DebounceEventHandler for WatchHandler {
    fn handle_event(&mut self, result: DebounceEventResult) {
        self.delivered_batches += 1;

        match result {
            Ok(events) => {
                println!(
                    "batch={} events={}",
                    self.delivered_batches,
                    events.len()
                );
            }
            Err(errors) => {
                eprintln!("watch errors={}", errors.len());
            }
        }
    }
}
```

Value case:

| Closure | Custom handler type |
|---|---|
| smallest code | independently testable |
| easy captures | explicit state |
| ideal for channel forwarding | reusable instrumentation |
| awkward once logic grows | clearer dependency ownership |

The public trait has one method, `handle_event`, and implementations are provided for compatible closures and supported sender types. [S1]

---

## 9.4 Standard `mpsc` sender as handler

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
};
use std::{sync::mpsc, time::Duration};

let (tx, rx) = mpsc::channel::<DebounceEventResult>();

let mut debouncer = new_debouncer(
    Duration::from_millis(75),
    None,
    tx,
)?;

for result in rx {
    match result {
        Ok(events) => {
            // application thread owns processing
        }
        Err(errors) => {
            // application error policy
        }
    }
}
```

The crate implements `DebounceEventHandler` for `std::sync::mpsc::Sender<DebounceEventResult>`. Optional feature integrations exist for Crossbeam and Flume sender types. [S1]

Critical implementation detail: the provided sender handler implementations intentionally ignore a send failure (`let _ = self.send(event)`). If the receiver disappears, that does **not** propagate as a watcher error or automatically stop the debouncer. [S3]

Therefore:

```text
receiver dropped
  → sender send fails
  → built-in handler ignores send error
  → debouncer can continue running
```

If receiver disappearance should trigger shutdown, use a custom handler or an externally owned lifecycle signal.

---

## 9.5 Bounded-vs-unbounded handoff

`std::sync::mpsc::Sender` is an unbounded queue. It decouples callback latency from downstream work, but a stalled consumer can cause memory growth.

For production indexing systems, make the choice explicit:

```text
unbounded channel:
  + never blocks debouncer thread on queue capacity
  + simplest
  - memory can grow without bound
  - stale work can accumulate

bounded channel:
  + explicit backpressure/memory ceiling
  - blocking send can block debouncer thread
  - dropping events on full queue needs reconciliation semantics
```

Because the crate's direct handler adapters are sender-type-specific, a bounded queue often calls for a custom closure that uses `try_send`, coalesces dirty paths, or signals a higher-level reconciliation worker.

Recommended for code intelligence:

```text
Debouncer callback
  → try_mark_dirty(path) in concurrent set
  → wake one worker/actor
  → worker drains dirty set
  → read current filesystem state
```

This bounds duplicate work without trusting every event as a transaction log.

---

## 9.6 Handler-latency budget

Measure handler duration separately from parsing/indexing duration.

Suggested SLO:

```text
handler p50: < 100 µs
handler p99: < 1 ms
handler work:
  classify
  increment counters
  enqueue/mark dirty
  return
```

The exact budget is application-specific, but the architecture principle is not: **the callback should hand off work rather than become the work executor.**

---

## 9.7 Panic policy

A panic inside the handler runs on the debouncer's background thread. Do not rely on panic as a controlled shutdown mechanism.

Prefer:

```rust
match process_watch_batch(result) {
    Ok(()) => {}
    Err(error) => {
        tracing::error!(?error, "watch batch processing failed");
        // signal supervisor / mark reconciliation required
    }
}
```

For application-level work, propagate errors through your queue/actor/supervisor rather than panicking in the debouncer callback.

---

## 9.8 Handler anti-patterns

* Parsing an entire Rust module in the callback.
* Acquiring a global graph write lock while handling events.
* Doing blocking HTTP/database calls from the callback.
* Assuming every callback contains exactly one logical save.
* Ignoring the `Err(Vec<Error>)` branch.
* Using an unbounded channel with no queue-depth observability.
* Assuming receiver drop stops the debouncer when using built-in sender handlers.
* Calling `unwrap()` on paths or event attributes that are backend-dependent.
* Treating callback order as a durable transaction stream.

---

## 9.9 Handler checklist

```text
[ ] Handler accepts DebounceEventResult.
[ ] Success path iterates every DebouncedEvent in batch.
[ ] Error path records every notify::Error in batch.
[ ] Heavy work is handed off.
[ ] Queue growth/backpressure policy is explicit.
[ ] Receiver-disconnect policy is explicit.
[ ] Handler latency is instrumented.
[ ] No panic/unwrap on ordinary filesystem inputs.
[ ] Downstream processing is idempotent.
```

---

# notify-debouncer-full Advanced — 10) Event taxonomy and semantic interpretation

## 10.0 Event-layer stack

```text
notify::Event
  ├─ kind: EventKind
  │    ├─ Access(...)
  │    ├─ Create(...)
  │    ├─ Modify(...)
  │    │    └─ ModifyKind::Name(RenameMode)
  │    ├─ Remove(...)
  │    ├─ Any
  │    └─ Other
  ├─ paths: Vec<PathBuf>
  └─ attrs: EventAttributes
       ├─ tracker / cookie
       ├─ flag: Rescan
       ├─ info
       └─ source

notify-debouncer-full
  → normalizes/coalesces selected event relationships
  → wraps final Event in DebouncedEvent { event, time }
```

`notify-debouncer-full` does not invent an independent event taxonomy. `DebouncedEvent` dereferences to the underlying `notify::Event`, so `kind`, `paths`, and attributes remain the main semantic API. [S6] [S9]

---

## 10.1 Top-level `EventKind`

```text
EventKind::Any
EventKind::Access(AccessKind)
EventKind::Create(CreateKind)
EventKind::Modify(ModifyKind)
EventKind::Remove(RemoveKind)
EventKind::Other
```

The nested kinds provide progressively more specificity, but the exact specificity varies by backend and OS. Write match statements to tolerate broad variants such as `Any`. [S11]

Robust pattern:

```rust
use notify_debouncer_full::notify::EventKind;

match event.kind {
    EventKind::Create(_) => on_create(&event.paths),
    EventKind::Modify(_) => on_modify(&event.paths),
    EventKind::Remove(_) => on_remove(&event.paths),
    EventKind::Access(_) => {}
    EventKind::Any | EventKind::Other => {
        // conservative fallback or ignore by policy
    }
}
```

---

## 10.2 Match broad semantics before fine semantics

Bad:

```rust
// brittle: assumes one backend/editor emits one exact kind
if event.kind == EventKind::Modify(ModifyKind::Data(DataChange::Content)) {
    reindex();
}
```

Better for code indexing:

```rust
match event.kind {
    EventKind::Create(_) |
    EventKind::Modify(_) => mark_current_path_dirty(),
    EventKind::Remove(_) => mark_removed(),
    _ => {}
}
```

Then narrow only where the distinction genuinely changes application semantics.

---

## 10.3 Rename representation

The most important normalized event is:

```text
EventKind::Modify(
  ModifyKind::Name(RenameMode::Both)
)
paths = [from_path, to_path]
```

For `RenameMode::Both`, `notify` documents the first path as the source and the second path as the destination. [S10]

Safe helper:

```rust
use notify_debouncer_full::notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind,
};
use std::path::Path;

fn rename_paths(event: &Event) -> Option<(&Path, &Path)> {
    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
            if event.paths.len() >= 2 =>
        {
            Some((&event.paths[0], &event.paths[1]))
        }
        _ => None,
    }
}
```

Do not assume `paths[0]` exists for arbitrary events.

---

## 10.4 Event attributes

`notify::Event` carries `attrs`, and convenience methods expose selected attributes such as:

```rust
let tracker = event.attrs.tracker();
let flag = event.attrs.flag();
let info = event.attrs.info();
let source = event.attrs.source();

let needs_rescan = event.need_rescan();
```

These are backend-dependent metadata. A tracker/cookie may help correlate rename halves; the debouncer already uses tracker IDs where available before falling back to file IDs. [S9] [S3]

Agent rule:

```text
Treat event attributes as optional evidence, not universally populated fields.
```

---

## 10.5 `DebouncedEvent::time`

`DebouncedEvent` contains:

```rust
pub struct DebouncedEvent {
    pub event: notify::Event,
    pub time: std::time::Instant,
}
```

The timestamp is used internally for debounce eligibility and ordering. It is a monotonic `Instant`, not a wall-clock timestamp suitable for persistence or audit logs. [S6]

If wall-clock observability is needed, stamp delivery separately:

```rust
let delivered_at = std::time::SystemTime::now();
```

Do not serialize `Instant` as if it were a cross-process event timestamp.

---

## 10.6 Event → application intent mapping

For an incremental source index:

| Debounced semantics | Application intent |
|---|---|
| Create(path) | read current file, create/update index record |
| Modify(path) | read current file, compare digest, reindex if changed |
| Remove(path) | verify absence, remove/tombstone index record |
| Rename(from,to) | verify current state; preferably move identity then reparse target |
| Access | usually ignore |
| Rescan flag | reconcile entire affected watch scope |
| unknown/Other | ignore or conservatively reconcile according to product need |

This table intentionally uses **current-state verification** rather than mechanically replaying event verbs.

---

## 10.7 Why event kind should not be your source of truth

Editors can save through different strategies:

```text
Strategy A:
  open existing file
  truncate
  write
  close

Strategy B:
  write temp file
  fsync
  rename temp over original

Strategy C:
  create sibling backup
  write replacement
  remove backup
```

The same user action therefore produces different low-level event sequences. `notify` explicitly warns that editor behavior differs and that some editors replace files rather than simply modifying them. [S8]

Application invariant:

```text
Filesystem event = invalidation hint.
Filesystem state = source of truth.
```

---

## 10.8 Event taxonomy anti-patterns

* Equating `Modify(Data(Content))` with “the only real save event.”
* Treating `RenameMode::Both` paths in the wrong order.
* Indexing `event.paths[0]` without checking length.
* Assuming tracker/cookie attributes always exist.
* Persisting `DebouncedEvent::time` as UTC/event wall time.
* Dropping `Create` because “the next Modify will arrive”; the debouncer may intentionally suppress redundant modifies after create.
* Treating Access events as content invalidation without need.
* Exhaustively matching non-exhaustive enums without fallback behavior.

---

## 10.9 Event checklist

```text
[ ] Match broad Create/Modify/Remove semantics first.
[ ] Detect RenameMode::Both explicitly.
[ ] Validate path count before indexing.
[ ] Treat attrs as optional.
[ ] Treat DebouncedEvent::time as monotonic process-local time.
[ ] Read/stat/hash current filesystem state before expensive mutation.
[ ] Keep fallback branch for unknown/Any/Other variants.
```

---

# notify-debouncer-full Advanced — 11) Rescan semantics and loss recovery

## 11.0 Core invariant

A file watcher is not a durable event log.

`notify::Event::need_rescan()` reports whether the backend indicates that events may have been missed. `notify`'s documented meaning is that an application maintaining an in-memory representation should refresh it. [S9] [S19]

```text
normal mode:
  native notifications
    → incremental invalidations

loss / overflow / rescan condition:
  event.need_rescan() == true
    → incremental event history no longer sufficient
    → reconcile watched state
```

For a code-property graph, this distinction is fundamental.

---

## 11.1 What `notify-debouncer-full` does internally

When raw processing receives an event marked for rescan, the debouncer:

1. triggers a cache rescan of watched roots;
2. keeps the rescan-indicating event in its delayed queue;
3. returns from normal handling for that raw event;
4. later emits the rescan event once debounce timing permits. [S3]

The cache refresh helps future rename/file-ID correlation, but it does **not** reconcile your application database, AST cache, graph, build cache, or derived state.

Therefore:

```text
Debouncer cache rescan != application state rescan.
```

---

## 11.2 Detecting a rescan event

```rust
for event in events {
    if event.need_rescan() {
        schedule_full_reconciliation();
        continue;
    }

    process_incremental_event(event);
}
```

If a batch contains both a rescan signal and ordinary events, it is generally safer to mark the scope dirty for reconciliation rather than pretending the ordinary events prove completeness.

---

## 11.3 Reconciliation algorithm for code intelligence

```text
1. Enter reconciliation generation N.
2. Enumerate files under canonical watch roots.
3. Apply inclusion/exclusion policy.
4. Compute cheap identity:
     path + metadata
   then content hash when needed.
5. Compare against indexed inventory.
6. Classify:
     unchanged
     added
     content changed
     removed
     probable rename/move
7. Parse/recompute only changed records.
8. Apply graph/index delta atomically.
9. Record successful reconciliation generation N.
10. Resume/merge incremental dirty events that arrived during scan.
```

The key is not “rebuild everything”; it is **re-enumerate authoritative state and calculate a minimal corrective delta**.

---

## 11.4 Generation fence during reconciliation

Events can arrive while a rescan is happening. A robust architecture uses generations:

```text
watch generation = 41
rescan requested
  ↓
start reconcile generation 42
  ↓
new incremental events continue arriving
  → dirty_after_generation_42 set
  ↓
commit reconciliation snapshot/delta
  ↓
process dirty_after_generation_42 against current filesystem state
```

This avoids losing a save that occurs after the directory walk has already inspected that path.

---

## 11.5 Digest-based reconciliation

For each indexed file, maintain enough state to cheaply identify drift:

```rust
struct IndexedFileState {
    path: std::path::PathBuf,
    size: u64,
    modified: Option<std::time::SystemTime>,
    content_hash: [u8; 32],
    parser_version: u64,
}
```

Suggested check cascade:

```text
path absent                         → remove
path new                            → add
size/mtime definitely changed      → hash/read
size/mtime same                    → optionally trust or hash by policy
hash changed                       → parse/reindex
hash same                          → no semantic update
parser/index schema version changed → reindex independent of file event
```

Metadata is a performance hint, not a cryptographic correctness guarantee.

---

## 11.6 Rescan scope

Possible policies:

| Signal scope | Recovery scope |
|---|---|
| backend cannot identify affected root | all watched roots |
| application knows one root generated overflow | that root |
| multiple tenant/workspace watchers | only corresponding workspace |
| uncertain mapping | broaden scope rather than preserve potentially corrupt state |

Correctness systems should prefer a bounded false-positive rescan over silent divergence.

---

## 11.7 Rescan state machine

```text
HEALTHY
  ├─ incremental events → remain HEALTHY
  └─ need_rescan/error implying loss
        ↓
RECONCILE_REQUESTED
        ↓ coalesce duplicate requests
RECONCILING
  ├─ success → HEALTHY
  └─ failure → DEGRADED
                  ↓ retry/backoff/operator action
              RECONCILING
```

Do not launch one full tree walk per rescan signal. Coalesce them.

---

## 11.8 Rescan observability

Capture:

```text
watch_rescan_requested_total
watch_rescan_completed_total
watch_rescan_failed_total
watch_reconcile_duration_seconds
watch_reconcile_files_scanned
watch_reconcile_files_added
watch_reconcile_files_changed
watch_reconcile_files_removed
watch_reconcile_bytes_hashed
watch_degraded_state = 0|1
```

A watcher that “keeps running” while the application state is stale is operationally worse than a watcher that explicitly reports degraded consistency.

---

## 11.9 Rescan anti-patterns

* Ignoring `need_rescan()` because debouncing “should prevent event loss.”
* Assuming cache rescan repairs the application index.
* Rebuilding the entire graph synchronously inside the handler.
* Launching overlapping directory-wide rescans.
* Dropping incremental events that arrive during reconciliation.
* Declaring success before atomically committing the corrected index state.
* Assuming native notification delivery is 100% reliable on very large trees.

---

## 11.10 Rescan checklist

```text
[ ] Inspect need_rescan() on every delivered event.
[ ] Treat it as a consistency signal, not informational logging.
[ ] Coalesce rescan requests.
[ ] Re-enumerate authoritative filesystem state.
[ ] Compute minimal correction via hashes/state comparison.
[ ] Fence events arriving during reconciliation.
[ ] Commit correction atomically where possible.
[ ] Expose degraded/reconciling status.
[ ] Test rescan path independently of real OS overflow.
```

---

# notify-debouncer-full Advanced — 12) Error model and diagnostics

## 12.0 Error channels

```text
construction errors
  new_debouncer / new_debouncer_opt
      → Result<Debouncer, notify::Error>

registration/config errors
  debouncer.watch(...)
  debouncer.unwatch(...)
  debouncer.configure(...)
      → Result<...>

asynchronous backend errors
  native watcher callback
      → Err(notify::Error)
      → debounce error queue
      → handler Err(Vec<notify::Error>)

application processing errors
  parse/hash/index/database
      → your own error domain
```

Do not collapse these into one stringly typed “watcher failed” category.

---

## 12.1 Construction-time error handling

```rust
let mut debouncer = new_debouncer(
    timeout,
    tick,
    handler,
).map_err(|e| anyhow::anyhow!("creating filesystem debouncer: {e}"))?;
```

`new_debouncer_opt` also rejects a `tick_rate` greater than `timeout`, so invalid timing configuration can fail before the watcher is returned. [S3]

Configuration should be validated at startup and fail fast.

---

## 12.2 Watch registration error handling

```rust
use notify_debouncer_full::notify::RecursiveMode;
use std::path::Path;

let root = Path::new("./src");

debouncer
    .watch(root, RecursiveMode::Recursive)
    .map_err(|e| anyhow::anyhow!(
        "registering recursive watch for {}: {e}",
        root.display()
    ))?;
```

Preserve the root and recursive mode in the error context.

---

## 12.3 Async error branch

```rust
fn handle_watch_result(
    result: notify_debouncer_full::DebounceEventResult,
) {
    match result {
        Ok(events) => process_events(events),
        Err(errors) => {
            for error in errors {
                tracing::error!(?error, "filesystem watcher error");
            }
            schedule_health_check_or_reconcile();
        }
    }
}
```

Not every async error proves state loss, but production systems should classify whether the error threatens completeness and react conservatively.

---

## 12.4 Error context structure

Recommended internal error envelope:

```rust
#[derive(Debug)]
struct WatchFailure {
    workspace_id: String,
    root: std::path::PathBuf,
    phase: WatchPhase,
    watcher_kind: notify_debouncer_full::notify::WatcherKind,
    source: String,
}

#[derive(Debug)]
enum WatchPhase {
    Construct,
    Register,
    Runtime,
    Reconcile,
    Shutdown,
}
```

This makes diagnostics answer:

```text
which workspace?
which root?
which watcher backend?
which lifecycle phase?
was state reconciliation triggered?
```

---

## 12.5 Linux `ENOSPC` is often watch exhaustion

On Linux, recursive inotify watches can hit the per-user watch limit. `notify` documents the familiar `ENOSPC` symptom and points to `fs.inotify.max_user_watches` as the relevant system setting; a recursive tree consumes many watch descriptors. [S8]

Do not automatically interpret `ENOSPC` as disk-full in watcher startup logs.

Suggested diagnostic:

```text
watch registration failed with ENOSPC on Linux
  → inspect /proc/sys/fs/inotify/max_user_watches
  → inspect current process/user watcher usage
  → reduce watch scope or raise limit if operationally appropriate
  → consider PollWatcher when native watches are unsuitable
```

---

## 12.6 Error severity policy

| Error class | Typical action |
|---|---|
| invalid startup config | fail startup |
| root missing at registration | fail or mark workspace unavailable |
| permission denied | fail root / surface configuration issue |
| inotify watch exhaustion | actionable operator error; optionally fallback |
| transient backend runtime error | record + health check/reconcile |
| event loss / rescan signal | reconcile authoritative state |
| downstream parser error | isolate file, preserve watcher health |
| downstream DB transaction failure | retry/rollback; do not blame watcher |

---

## 12.7 Diagnostics snapshot

Expose at least:

```text
watcher_kind
watch_roots
recursive_mode
configured_timeout_ms
configured_tick_ms
poll_interval_ms if applicable
compare_contents if applicable
follow_symlinks if applicable
last_event_at
last_error_at
last_error_summary
last_successful_reconcile_at
reconcile_in_progress
```

This turns “file updates stopped” from an opaque symptom into a debuggable state.

---

## 12.8 Error anti-patterns

* `unwrap()` around `watch()` in a long-lived service.
* Logging only `error.to_string()` with no root/workspace context.
* Treating every async error as fatal process termination.
* Treating every async error as harmless.
* Misdiagnosing Linux watcher `ENOSPC` as storage exhaustion.
* Retrying registration in a tight loop without fixing resource/permission conditions.
* Conflating parser failures with watcher failures.
* Continuing indefinitely after known event loss without reconciliation.

---

## 12.9 Error checklist

```text
[ ] Startup/config errors fail fast.
[ ] Registration errors include root and recursive mode.
[ ] Handler has Err(Vec<notify::Error>) branch.
[ ] Runtime errors are classified for consistency impact.
[ ] Linux ENOSPC diagnostics mention inotify watch limits.
[ ] Reconciliation is triggered when completeness is uncertain.
[ ] Downstream errors are separate from watcher health.
[ ] Health endpoint exposes backend/config/reconcile state.
```

---

# notify-debouncer-full Advanced — 13) Custom construction with `new_debouncer_opt`

## 13.0 When to leave `new_debouncer`

Use `new_debouncer(...)` for the common case:

```text
RecommendedWatcher
+ RecommendedCache
+ default notify::Config
```

Use `new_debouncer_opt(...)` when you need to choose:

```text
specific watcher backend
specific file-ID cache
notify::Config
poll interval
content comparison
symlink behavior
platform-specific backend policy
```

---

## 13.1 Signature mental model

Conceptually, 0.7.0 exposes:

```rust
new_debouncer_opt(
    timeout,
    tick_rate,
    event_handler,
    file_id_cache,
    notify_config,
)
```

with generic parameters for the `Watcher`, handler, and `FileIdCache`. [S3]

This means backend choice is a compile-time generic type at construction, while runtime registration remains through the common `Watcher` interface exposed by `Debouncer`.

---

## 13.2 Custom poll watcher

```rust
use notify_debouncer_full::{
    new_debouncer_opt,
    notify::{
        Config,
        PollWatcher,
        RecursiveMode,
    },
    NoCache,
};
use std::time::Duration;

let config = Config::default()
    .with_poll_interval(Duration::from_secs(2))
    .with_compare_contents(true);

let mut debouncer = new_debouncer_opt::<_, PollWatcher, _>(
    Duration::from_millis(150),
    None,
    |result| {
        println!("{result:?}");
    },
    NoCache,
    config,
)?;

debouncer.watch("./src", RecursiveMode::Recursive)?;
```

Use a polling backend where native notifications are unreliable or unavailable. [S13] [S14]

---

## 13.3 `notify::Config` options that matter

### Poll interval

```rust
let config = Config::default()
    .with_poll_interval(Duration::from_secs(2));
```

The documented default poll interval is 30 seconds. This option matters to `PollWatcher`; native backends generally do not become polling backends merely because this field exists. [S13]

### Compare contents

```rust
let config = Config::default()
    .with_compare_contents(true);
```

When enabled for polling, the watcher hashes file contents to detect changes even when modification time is unreliable. This adds potentially substantial I/O and CPU overhead and cannot be changed at runtime. [S13] [S14]

### Follow symlinks

```rust
let config = Config::default()
    .with_follow_symlinks(false);
```

The option applies to selected backends including inotify, kqueue, and polling; following symlinks is enabled by default and is not dynamically reconfigurable. [S13]

---

## 13.4 Poll interval is not debounce timeout

Keep the two clocks separate:

```text
PollWatcher poll_interval:
  how often filesystem state is sampled

Debouncer timeout:
  how old a raw event must be before eligible for delivery

Debouncer tick_rate:
  how often the debouncer checks for eligible events
```

Approximate change-to-delivery latency for polling can therefore include:

```text
0..poll_interval
+ timeout
+ 0..tick_rate
+ handler/queue delay
```

A 50 ms debounce cannot make a 30-second polling backend react in 50 ms.

---

## 13.5 Manual polling caveat

`notify::Config::with_manual_polling()` configures a `PollWatcher` for explicit `PollWatcher::poll()` calls. [S13] [S14]

However, `Debouncer` exposes the generic `Watcher` API, and 0.7.0 deprecates exposing the inner watcher. Therefore, a manually polled `PollWatcher` is not a natural fit for the standard full-debouncer wrapper unless your architecture retains access through another custom layer.

Agent rule:

```text
When using PollWatcher inside notify-debouncer-full 0.7.0,
prefer automatic poll_interval configuration.
Do not select manual polling unless you have an explicit design for invoking poll().
```

---

## 13.6 Custom cache selection

```rust
use notify_debouncer_full::{FileIdMap, NoCache};
```

Possible choices:

```text
NoCache:
  no file-ID matching
  lower startup/tree-walk cost

FileIdMap:
  path → file ID cache
  improved rename matching when tracker IDs are unavailable
  recursive cache work / filesystem identity lookup

custom FileIdCache:
  application-defined identity/cache behavior
```

The platform `RecommendedCache` aliases to `NoCache` on Linux/Android/WASM and `FileIdMap` on other supported platforms. [S7]

---

## 13.7 Construction factory pattern

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
    Debouncer,
    RecommendedCache,
    notify::RecommendedWatcher,
};
use std::time::Duration;

fn build_debouncer(
    tx: std::sync::mpsc::Sender<DebounceEventResult>,
) -> notify_debouncer_full::notify::Result<
    Debouncer<RecommendedWatcher, RecommendedCache>
> {
    new_debouncer(
        Duration::from_millis(75),
        Some(Duration::from_millis(20)),
        tx,
    )
}
```

Centralize construction so timeout/backend/cache behavior does not drift across application modules.

---

## 13.8 Configuration profile examples

```text
Interactive IDE/code graph:
  backend = RecommendedWatcher
  timeout = 50–100 ms
  tick = 10–25 ms
  cache = RecommendedCache

Container/shared-volume workspace:
  backend = PollWatcher if native notifications fail validation
  poll_interval = 0.5–2 s depending scale
  debounce timeout = 100–250 ms
  compare_contents = only if metadata unreliable

Large monorepo native Linux:
  backend = Inotify through RecommendedWatcher
  timeout = 75–200 ms
  roots = tightly scoped
  cache = NoCache by recommended default
  explicit rescan recovery
```

These are starting profiles, not universal performance constants.

---

## 13.9 Custom-construction anti-patterns

* Setting `tick_rate > timeout`.
* Treating `poll_interval` and debounce timeout as interchangeable.
* Enabling content comparison across a huge repository without measuring I/O.
* Enabling symlink following without considering cycles/external tree expansion.
* Configuring manual polling then never calling `poll()`.
* Choosing a backend by OS name alone without validating deployment filesystem behavior.
* Copying `new_debouncer_opt` generic parameters from an outdated crate version.

---

## 13.10 Construction checklist

```text
[ ] Default to new_debouncer unless custom behavior is required.
[ ] Keep tick_rate <= timeout.
[ ] Separate poll interval from debounce timing.
[ ] Enable compare_contents only for a demonstrated metadata problem.
[ ] Decide symlink-follow policy deliberately.
[ ] Choose file-ID cache based on rename requirements and platform cost.
[ ] Centralize construction in one factory/service boundary.
[ ] Integration-test the actual filesystem/deployment environment.
```

---

# notify-debouncer-full Advanced — 14) Backend selection and platform model

## 14.0 Recommended watcher aliases

`notify` 8.2.0 selects a `RecommendedWatcher` according to target/platform feature configuration. Broadly: [S16]

```text
Linux / Android        → inotify backend
macOS default          → FSEvents backend
Windows                → ReadDirectoryChangesW backend
BSD / selected targets → kqueue backend
unsupported/fallback   → PollWatcher
```

`WatcherKind` is non-exhaustive and includes variants for Inotify, Fsevent, Kqueue, PollWatcher, ReadDirectoryChangesWatcher, and NullWatcher. [S15]

Inspect the selected backend kind. `Debouncer::kind()` is an **associated function** (the watcher backend is carried by the `Debouncer<T, C>` type), not an instance method:

```rust
use notify_debouncer_full::{Debouncer, RecommendedCache};
use notify_debouncer_full::notify::RecommendedWatcher;

let kind = Debouncer::<RecommendedWatcher, RecommendedCache>::kind();
tracing::info!(?kind, "filesystem watcher backend");
```

---

## 14.1 Backend selection is a deployment property

Do not treat:

```text
operating system == filesystem notification behavior
```

as universally true.

Examples documented by `notify` include:

* network filesystems can fail to emit expected events;
* WSL paths may not behave like native Linux files;
* Linux containers running through macOS host filesystem layers can fail native watcher expectations;
* pseudo-filesystems such as `/proc` and `/sys` can have unusual event/mtime behavior. [S8]

Therefore:

```text
choose backend
  → test on actual mount/filesystem/container topology
  → validate create/modify/remove/rename
  → validate burst load
  → validate root deletion/recreation
```

---

## 14.2 Backend capabilities matrix

| Concern | Native backends | PollWatcher |
|---|---|---|
| latency | usually low | bounded by poll interval |
| kernel/resource limits | backend-specific | fewer native watch descriptor constraints |
| network/shared FS reliability | variable | often more robust if metadata visible |
| event richness | potentially richer | reconstructed from snapshots |
| rename fidelity | backend-specific | reconstructed/different semantics |
| CPU/I/O | event-driven | periodic directory/stat work |
| unreliable mtimes | can still get native events | use compare_contents at extra cost |
| portability | RecommendedWatcher abstracts | explicit universal fallback |

---

## 14.3 Runtime backend policy

A production service can encode:

```rust
#[derive(Debug, Clone, Copy)]
enum WatchBackendPolicy {
    NativeRecommended,
    Poll,
}
```

Then select different concrete service implementations at a higher abstraction boundary rather than scattering backend conditions through event-processing code.

Because `Debouncer<W, C>` is generic over watcher type, one ergonomic pattern is an enum around concrete debouncers or a higher-level trait that hides the generic type.

---

## 14.4 Fallback strategy

Do not silently switch from native to polling after arbitrary errors without observing the change. A backend change modifies latency and resource behavior.

Better:

```text
native watcher startup fails in recognized unsupported environment
  → log explicit fallback reason
  → construct PollWatcher profile
  → mark health metadata backend=poll
  → run initial reconciliation
```

For correctness-critical systems, fallback should be intentional and observable.

---

## 14.5 Feature flags and macOS backend choice

`notify`'s crate features include the normal macOS FSEvents support and an optional kqueue path. `notify-debouncer-full` forwards relevant platform feature controls. [S8] [S2]

Do not change macOS backend features casually: backend semantics, system dependencies, and deployment behavior can change.

---

## 14.6 Backend acceptance test

```text
fixture root
  a.rs

Test sequence:
  1. create b.rs
  2. modify a.rs in place
  3. atomic-save replace a.rs
  4. rename b.rs → c.rs
  5. move c.rs into nested/
  6. delete nested/
  7. create 1000 files rapidly
  8. delete/recreate watched root or parent according to policy

Assert application outcome:
  final indexed file inventory == filesystem inventory
  final file hashes == filesystem hashes
```

Assert **final state**, not exact raw event counts, across platforms.

---

## 14.7 Backend anti-patterns

* Assuming `RecommendedWatcher` means “same semantics everywhere.”
* Unit-testing only synthetic `Event` values and never testing a real filesystem.
* Depending on an exact raw event count across OSes.
* Silently switching backend without telemetry.
* Choosing polling merely to avoid learning native watcher limits.
* Choosing native watching on a shared/network mount without validation.
* Matching all `WatcherKind` variants without a future-compatible fallback.

---

## 14.8 Backend checklist

```text
[ ] Log the associated `Debouncer::<Watcher, Cache>::kind()` at startup.
[ ] Validate backend on actual deployment filesystem.
[ ] Test create/modify/atomic-save/rename/delete/burst.
[ ] Have explicit PollWatcher profile when needed.
[ ] Record fallback/backend in health metadata.
[ ] Assert final state, not exact event sequence, in cross-platform tests.
[ ] Keep WatcherKind matching future-compatible.
```

---

# notify-debouncer-full Advanced — 15) Linux / inotify deployment deep dive

## 15.0 Linux mental model

```text
watch(root, Recursive)
  → notify inotify backend
  → watches directory tree
  → kernel inotify queue
  → notify::Event
  → full debouncer
  → application
```

On Linux, the platform-recommended cache is `NoCache`; inotify provides useful rename correlation information, so the full file-ID map is not the default. [S7]

---

## 15.1 Recursive-watch resource pressure

Linux inotify has per-user resource limits. `notify` explicitly documents that recursive watching can exhaust `fs.inotify.max_user_watches`, producing `ENOSPC`. [S8]

Check current value:

```bash
cat /proc/sys/fs/inotify/max_user_watches
```

Temporary operational adjustment example:

```bash
sudo sysctl fs.inotify.max_user_watches=524288
```

System configuration policy belongs to deployment/operations, not library code.

---

## 15.2 Watch-root minimization

Bad for a code indexer:

```text
watch /home/user recursively
```

Better:

```text
watch /workspace/project recursively
exclude target, .git, generated dirs downstream
```

Even downstream filtering does not necessarily eliminate native watch descriptor cost: if the root is recursively watched, the backend still has to observe the tree.

For very large repositories, consider narrower stable roots if feasible.

---

## 15.3 Queue overflow and event loss

Large bursts can overflow native event queues. `notify` warns that very large directories can lose events and native watching should not be treated as a perfect transactional stream. [S8]

Therefore Linux production correctness requires:

```text
need_rescan handling
+ periodic or triggered reconciliation
+ final-state validation
```

Debouncing reduces duplicate application work; it does not increase kernel queue capacity.

---

## 15.4 Directory removal behavior

The full debouncer includes special handling for directory removals under inotify: queued child-path events and cache state are cleaned so a directory deletion does not emit redundant child removals as independent logical operations. [S1] [S3]

This is convenient, but application semantics still need to remove all indexed descendants when a directory path disappears.

Example:

```text
Remove(src/generated)
  → application inventory lookup:
      delete/tombstone every indexed file with path prefix src/generated/
```

Do not wait for one remove event per child.

---

## 15.5 Parent deletion

`notify` documents a general watcher caveat: if the watched path itself is deleted, you may need to watch its parent to observe/recover the lifecycle you care about. [S8]

For an IDE workspace where the root can be renamed/replaced:

```text
watch workspace root recursively
+ optionally watch parent non-recursively
+ supervise root existence
+ re-register/reconcile if root is recreated
```

---

## 15.6 WSL / mounted filesystems

A Linux process does not guarantee inotify-compatible semantics for every mounted location. `notify` specifically calls out WSL Windows paths and network filesystems as cases where native events may not be emitted reliably; polling is the fallback. [S8]

Deployment test:

```text
if path is on:
  /mnt/c/... under WSL
  NFS/CIFS/FUSE-like mount
  host-mounted container volume
then:
  validate native event delivery before assuming it
  otherwise choose PollWatcher
```

---

## 15.7 Linux service checklist

```text
[ ] Log watcher kind = Inotify when expected.
[ ] Inspect max_user_watches for large recursive trees.
[ ] Keep watch roots narrow.
[ ] Test burst creation/deletion.
[ ] Handle need_rescan.
[ ] Remove indexed descendants on directory remove.
[ ] Decide whether parent watch is needed for root replacement.
[ ] Validate WSL/network/container mounts separately.
[ ] Maintain PollWatcher fallback profile.
```

---

# notify-debouncer-full Advanced — 16) macOS / FSEvents deployment deep dive

## 16.0 macOS mental model

With normal default features, `RecommendedWatcher` uses the macOS FSEvents backend. [S16]

```text
FSEvents stream
  → notify events
  → full debouncer
  → RecommendedCache = FileIdMap
  → normalized path/rename behavior
```

The file-ID cache is more important here because the underlying event model does not necessarily expose the same rename tracker/cookie semantics as Linux inotify. [S7]

---

## 16.1 Startup/cache cost

`FileIdMap::add_path` walks watched directories and reads file IDs. It uses `WalkDir` with symlink following enabled in the cache implementation. [S12]

Consequences for large macOS workspaces:

```text
first recursive watch can involve full tree walk
+ filesystem metadata/file-ID reads
+ symlink traversal
```

Measure:

```text
watch_registration_duration
cache_initial_paths
workspace_file_count
```

If startup latency matters, scope roots carefully.

---

## 16.2 File ownership/security caveat

`notify` documents a macOS FSEvents security/permission caveat for files not owned by the observing user and suggests `PollWatcher` as a possible alternative when necessary. [S8]

Do not assume a watcher process with broad directory visibility receives every event regardless of ownership/security context.

---

## 16.3 FSEvents versus kqueue feature choice

The crate ecosystem supports alternate macOS backend feature selection. Do not switch to kqueue solely because its API seems lower-level; test the workload and deployment needs.

Questions:

```text
Do we need broad recursive tree observation?
How large is the tree?
What are ownership/permission constraints?
What native resource limits apply?
Does the deployment bundle permit the backend dependencies?
```

Keep the choice pinned in Cargo features and integration tests.

---

## 16.4 Atomic-save behavior

macOS editors frequently use atomic replacement strategies. The full debouncer's rename merging and queue path rewriting are specifically valuable here:

```text
temp write
  → rename temp to target
  → queued target-related operations normalized
  → application sees a more meaningful final target change/rename sequence
```

Still re-read the target path after delivery; do not treat the event payload as file content.

---

## 16.5 macOS checklist

```text
[ ] Expect RecommendedCache = FileIdMap.
[ ] Measure initial recursive cache/tree-walk time.
[ ] Scope roots to avoid unnecessary file-ID scans.
[ ] Review symlink behavior for workspace roots.
[ ] Test atomic saves in target editors.
[ ] Test files with relevant ownership/permission patterns.
[ ] Pin FSEvents vs kqueue feature choice.
[ ] Keep PollWatcher fallback for incompatible environments.
```

---

# notify-debouncer-full Advanced — 17) Windows / ReadDirectoryChangesW deployment deep dive

## 17.0 Windows mental model

`RecommendedWatcher` uses the Windows ReadDirectoryChangesW backend on Windows. [S16]

```text
ReadDirectoryChangesW
  → notify events
  → full debouncer
  → RecommendedCache = FileIdMap
  → rename matching via tracker where available or file ID fallback
```

As on macOS, the recommended full-debouncer cache is `FileIdMap`. [S7]

---

## 17.1 Path representation

Windows paths introduce cases that should be normalized only according to a clearly defined application policy:

```text
drive-letter casing
separator representation
UNC paths
long-path prefixes
case-insensitive filesystem semantics
junctions/symlinks
```

Do not lowercase arbitrary `PathBuf` strings as a general cross-platform canonicalization strategy. Preserve OS-native paths and derive an application key through a tested normalization layer.

---

## 17.2 File identity and rename correlation

The full debouncer's `FileIdMap` associates paths with `file-id` crate identities. During rename-to handling, correlation can use either an event tracker match or a cached file-ID match. [S3] [S12]

This helps turn:

```text
rename-from(old)
...
rename-to(new)
```

into:

```text
RenameMode::Both [old, new]
```

when sufficient identity evidence exists.

---

## 17.3 Network shares

Network/share semantics can differ materially from local NTFS behavior. As with other platforms, validate SMB/network locations rather than assuming `RecommendedWatcher` semantics are identical.

If native events are missing or incomplete, polling is the general fallback documented by `notify`. [S8]

---

## 17.4 Antivirus/indexer/contention effects

Windows development environments often have extra filesystem actors—antivirus, search indexing, backup/sync clients—that can increase metadata activity and transient access errors.

Application design should tolerate:

```text
file exists at event time but is briefly unreadable
file is replaced between event and read
multiple metadata/access events around a save
rename/replace races
```

Use retry-on-transient-read policy at the downstream file-read layer, not event-loop sleeps.

---

## 17.5 Windows checklist

```text
[ ] Expect RecommendedCache = FileIdMap.
[ ] Keep PathBuf native; normalize application keys deliberately.
[ ] Test drive/UNC/workspace path forms used in deployment.
[ ] Test atomic replace and rename behavior.
[ ] Retry transient file reads outside handler.
[ ] Validate network share behavior independently.
[ ] Reconcile final filesystem state after known loss/error.
```

---

# notify-debouncer-full Advanced — 18) PollWatcher and non-native environments

## 18.0 Polling mental model

```text
every poll_interval
  → enumerate/stat watched state
  → compare current snapshot with previous snapshot
  → optionally hash contents
  → synthesize notify::Event values
  → full debouncer
  → handler
```

`PollWatcher` is part of `notify` and implements the common `Watcher` trait. It uses metadata comparison by default; content hashing can be enabled for filesystems where mtimes/metadata do not reliably reveal changes. [S14]

---

## 18.1 When polling is the right choice

Use polling when native notification semantics fail the deployment requirement, including commonly documented categories such as: [S8]

```text
network/NFS-like filesystem
WSL Windows-mounted path
container host-volume topology with broken native propagation
pseudo-filesystem with unusual notifications
platform/backend unavailable
```

Polling is not “inferior by definition.” It trades latency/I/O for a different failure model.

---

## 18.2 Latency model

For automatic polling:

```text
change happens at arbitrary time
  ↓ wait until next poll: [0, poll_interval]
poll detects difference
  ↓ debouncer queues event
wait timeout
  ↓ wait to next tick: [0, tick_rate]
delivery
```

Approximate worst-case before downstream queueing:

```text
poll_interval + timeout + tick_rate
```

Average depends on change timing and workload.

---

## 18.3 Default poll interval

`notify::Config` documents a default poll interval of 30 seconds. [S13]

For interactive code tools, explicitly configure a much shorter value if polling is selected:

```rust
let config = notify_debouncer_full::notify::Config::default()
    .with_poll_interval(Duration::from_secs(1));
```

Do not assume a custom debounce timeout changes this default.

---

## 18.4 `compare_contents`

Enable:

```rust
let config = Config::default()
    .with_poll_interval(Duration::from_secs(1))
    .with_compare_contents(true);
```

Use when the environment can modify file content without trustworthy mtime/metadata updates. [S13]

Cost model:

```text
without compare_contents:
  directory traversal + metadata reads

with compare_contents:
  directory traversal + metadata reads
  + content reads/hashing for files
```

On a million-file repository or remote mount, content comparison can become dominant I/O.

---

## 18.5 Polling and debouncing are complementary

Polling can itself discover several related differences in one scan:

```text
poll detects:
  temp file created
  target replaced
  old file removed
```

The full debouncer can still normalize/coalesce resulting events before delivery. Therefore it remains useful even though the source is snapshot-based rather than kernel-notification-based.

---

## 18.6 Poll interval tuning by workload

```text
Interactive local/shared-volume code tool:
  250 ms–2 s range worth benchmarking

CI/background index refresh:
  2–10 s may be sufficient

Low-change archival tree:
  tens of seconds/minutes may be sufficient
```

Do not hardcode these as library truths. Benchmark:

```text
files per root
metadata latency
remote filesystem RTT
hash cost
CPU budget
acceptable freshness
```

---

## 18.7 Adaptive polling at application level

`notify` exposes runtime configuration support only for specific options, and not every setting can be changed dynamically. [S13]

If adaptive polling is a product requirement, design a supervisor that can rebuild the watcher with a new `Config` rather than assuming all fields can mutate in place.

Example policy:

```text
active editor session → 500 ms polling
idle workspace        → 5 s polling
background workspace  → 30 s polling
```

Reconstruction requires careful root-state replay and an initial reconciliation.

---

## 18.8 Polling anti-patterns

* Choosing PollWatcher but leaving the 30-second default in an interactive product.
* Setting a 20 ms debounce timeout and expecting 20 ms freshness under 5-second polling.
* Enabling `compare_contents` globally without measuring filesystem read volume.
* Manual polling configuration with no accessible `poll()` call path.
* Rebuilding polling watcher frequently without preserving/re-registering roots correctly.
* Treating poll-synthesized rename semantics as a guaranteed durable identity operation.
* Running aggressive polling over a remote share without backoff/health controls.

---

## 18.9 PollWatcher checklist

```text
[ ] Use polling only for an explicit environment/correctness reason.
[ ] Configure poll_interval explicitly.
[ ] Model latency as poll + debounce + tick.
[ ] Enable compare_contents only if metadata misses real changes.
[ ] Measure traversal/hash I/O.
[ ] Prefer automatic polling inside Debouncer 0.7.0.
[ ] Reconcile on startup/reconstruction.
[ ] Expose poll backend and interval in health metadata.
```

---
# notify-debouncer-full Advanced — 19) Filtering, ignore rules, and scope reduction

## 19.0 Mental model

`notify-debouncer-full` is an event normalizer/debouncer, not a repository ignore engine.

```text
registered watch roots
  → backend observes filesystem activity
  → full debouncer normalizes activity
  → application filter
      include? exclude?
      relevant extension?
      ignored directory?
      generated artifact?
  → dirty-work scheduler
```

Filtering can occur at several layers, and they solve different problems:

```text
watch-root selection     → reduces backend observation scope
recursive/nonrecursive   → reduces backend traversal scope
application path filter  → reduces downstream work
semantic hash filter     → reduces reparsing when bytes unchanged
parser dependency filter → reduces graph recomputation
```

---

## 19.1 Root-level filtering versus event-level filtering

Suppose a workspace contains:

```text
repo/
  src/
  crates/
  target/
  .git/
  node_modules/
  generated/
```

Option A:

```text
watch repo recursively
filter target/.git/node_modules/generated in application
```

Advantages:

* dynamic source subdirectories are automatically covered;
* simple registration;
* one root lifecycle.

Disadvantages:

* native backend may still pay watch-resource cost for ignored subtrees;
* ignored activity still enters the watcher/debouncer path;
* high-churn build outputs can create queue pressure before filtering.

Option B:

```text
watch src recursively
watch crates recursively
watch selected config files/dirs
```

Advantages:

* lower native resource and event volume;
* high-churn ignored trees never enter pipeline.

Disadvantages:

* more dynamic-root management;
* newly created source areas outside registered roots can be missed;
* monorepo configuration changes may alter desired scope.

---

## 19.2 Path-filter function

```rust
use std::path::Path;

fn should_index(path: &Path) -> bool {
    if path.components().any(|part| {
        matches!(
            part.as_os_str().to_str(),
            Some("target" | ".git" | "node_modules" | ".cache")
        )
    }) {
        return false;
    }

    matches!(
        path.extension().and_then(|x| x.to_str()),
        Some("rs" | "toml" | "md")
    )
}
```

This is illustrative, not a fully robust ignore implementation. Real repository tooling should preserve non-UTF-8 `OsStr` behavior and use a tested ignore matcher rather than converting every component with `to_str()`.

---

## 19.3 Gitignore-compatible filtering

For source repositories, a common production architecture is:

```text
watch root broadly enough for correctness
  → normalize absolute/relative path
  → gitignore-style matcher
  → explicit product overrides
  → extension/type filter
```

Keep watcher semantics separate from ignore semantics. This lets ignore rules change without reconstructing every watcher immediately, while root changes remain an explicit resource decision.

If ignore files themselves are watched:

```text
.gitignore / .ignore / product config changed
  → rebuild matcher
  → reconcile inclusion set
```

A newly ignored file may need removal from the index even though the file itself did not change.

---

## 19.4 Directory-event pruning

When a removed directory is delivered, do not call `should_index()` only on the directory extension and discard it. A directory removal can invalidate many indexed descendants.

Use event-aware filtering:

```text
Remove(directory path)
  → if directory was within indexed scope:
      remove/reconcile indexed descendants

Create/Modify(file path)
  → apply file inclusion rules
```

The debouncer's special directory-remove normalization makes this particularly important. [S3]

---

## 19.5 Rename across filter boundaries

Example:

```text
src/foo.rs → target/foo.rs
```

The normalized rename crosses from included to excluded scope.

Application result:

```text
old path included, new path excluded
  → remove old indexed record
  → do not add target/foo.rs
```

Reverse:

```text
target/foo.rs → src/foo.rs
  → add/reindex new path
```

Both included:

```text
src/foo.rs → src/bar.rs
  → rename identity if useful
  → re-read/reparse target according to policy
```

Both excluded: ignore.

Implement this as a truth table rather than assuming all renames are “move index record.”

---

## 19.6 Extension changes on rename

```text
notes.txt → src/lib.rs
src/old.rs → archive/old.txt
```

Filtering must evaluate **both** rename endpoints. A rename can change whether the object belongs in the index.

```rust
fn classify_rename(from: &Path, to: &Path) -> RenameScope {
    match (should_index(from), should_index(to)) {
        (true, true) => RenameScope::IndexedToIndexed,
        (true, false) => RenameScope::IndexedToIgnored,
        (false, true) => RenameScope::IgnoredToIndexed,
        (false, false) => RenameScope::IgnoredToIgnored,
    }
}
```

---

## 19.7 Content-level no-op filtering

An event does not guarantee semantic or even byte-level change.

```text
filesystem event
  → stat/read
  → content hash
      same hash → no parser/index work
      changed   → parse/index
```

This is especially valuable when tools rewrite timestamps, perform no-op atomic replacements, or generate repetitive metadata events.

For code intelligence, content hashing often provides a more reliable second debounce layer than trying to perfect OS event matching.

---

## 19.8 Filter ordering

Recommended order:

```text
1. rescan signal?             → reconciliation path
2. event has relevant path?   → basic validation
3. directory removal/rename?  → structural handling
4. path inclusion filter      → cheap
5. file type/extension        → cheap
6. current existence/stat     → cheap I/O
7. content digest             → moderate I/O/CPU
8. parse/semantic analysis    → expensive
9. graph delta                → expensive/shared state
```

---

## 19.9 Filtering anti-patterns

* Assuming application filtering reduces inotify watch descriptors.
* Watching a massive build-output tree and filtering only after debouncing.
* Ignoring directory removals because directories have no source extension.
* Evaluating only the destination of a rename.
* Failing to re-evaluate index membership when ignore configuration changes.
* UTF-8-lossy path conversion as the canonical filter key.
* Re-parsing on every event without comparing current file digest.
* Baking repository ignore semantics into the watcher wrapper itself.

---

## 19.10 Filtering checklist

```text
[ ] Separate root scope from downstream filter scope.
[ ] Exclude high-churn trees at root level when practical.
[ ] Apply tested ignore semantics downstream.
[ ] Watch/reconcile ignore-rule changes if dynamic.
[ ] Handle directory removals structurally.
[ ] Evaluate both endpoints of rename.
[ ] Hash current content before expensive reparse when useful.
[ ] Preserve native paths without assuming UTF-8.
```

---

# notify-debouncer-full Advanced — 20) Tokio and async integration

## 20.0 Core rule

`notify-debouncer-full` is callback/thread based. Tokio is optional application architecture, not a requirement of the crate.

```text
debouncer thread
  → synchronous callback
  → fast handoff
  → Tokio channel / actor / task
  → async processing
```

Do not attempt to `.await` directly inside the synchronous callback.

---

## 20.1 Tokio unbounded channel bridge

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
};
use std::time::Duration;
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();

let mut debouncer = new_debouncer(
    Duration::from_millis(75),
    None,
    move |result| {
        let _ = tx.send(result);
    },
)?;

tokio::spawn(async move {
    while let Some(result) = rx.recv().await {
        process_async(result).await;
    }
});
```

This is simple, but the channel is unbounded. Add queue-depth/work-lag observability or use a different handoff for high-volume trees.

---

## 20.2 Bounded Tokio channel with `try_send`

```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel::<DebounceEventResult>(256);

let handler = move |result| {
    match tx.try_send(result) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_result)) => {
            // Do not silently lose consistency.
            // Mark workspace reconciliation required.
        }
        Err(mpsc::error::TrySendError::Closed(_result)) => {
            // Supervisor owns shutdown semantics.
        }
    }
};
```

This protects the debouncer thread from blocking, but overflow becomes an **application event-loss condition**. Mark the workspace dirty for reconciliation rather than silently dropping the batch.

---

## 20.3 Dirty-set bridge: preferred for code indexing

Instead of queueing every batch:

```text
callback
  → insert changed paths into concurrent dirty set
  → set rescan_required flag if necessary
  → notify worker once

worker
  → drain dirty set
  → inspect current filesystem state
  → parse/index each unique current path once
```

This naturally coalesces repeated events beyond the library debounce window and gives bounded work proportional to unique dirty paths.

Conceptual types:

```rust
struct DirtySignal {
    paths: std::collections::HashSet<std::path::PathBuf>,
    full_rescan: bool,
}
```

For concurrency, use an application-appropriate mutex/concurrent set and a `Notify`, bounded channel, or actor mailbox.

---

## 20.4 `spawn_blocking` and parsing

Rust source parsing may be CPU-bound but not necessarily “blocking I/O.” Decide based on parser architecture:

```text
small fast parser work:
  normal Tokio task may be acceptable if bounded

CPU-heavy semantic extraction:
  dedicated Rayon/thread pool or spawn_blocking-style isolation

filesystem reads:
  std::fs on dedicated worker, tokio::fs, or controlled sync I/O
```

The watcher library itself does not dictate this. Keep the debounce callback independent of the parser execution pool.

---

## 20.5 Cancellation and stale work

A file can change again while an earlier parse is running.

Use per-path generations:

```text
foo.rs dirty generation 10
  → parse job 10 starts
foo.rs dirty generation 11
  → parse job 11 queued
parse 10 finishes
  → compare current generation
  → 10 != 11 → discard stale result
parse 11 finishes
  → commit if still current
```

This prevents out-of-order async completion from overwriting a newer graph state.

---

## 20.6 Async service ownership

```rust
struct WatchService {
    debouncer: Option<MyDebouncer>,
    worker: tokio::task::JoinHandle<()>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}
```

Shutdown order:

```text
1. stop accepting new external work
2. stop debouncer / event source
3. close or signal event handoff
4. drain/cancel worker according to product semantics
5. await worker termination
6. flush durable index if needed
```

Avoid dropping the Tokio receiver first while the debouncer continues silently sending to a disconnected bridge.

---

## 20.7 Async error propagation

Use a supervisor channel/state:

```text
watch callback errors
  → WatchHealthEvent::BackendError

queue full
  → WatchHealthEvent::DeliveryOverflow
  → reconciliation_required=true

worker parse failure
  → FileAnalysisError(path)

worker database failure
  → IndexCommitError
```

This preserves error origin and response policy.

---

## 20.8 Tokio integration anti-patterns

* Calling `tokio::runtime::Handle::block_on` inside the watcher callback.
* Doing `blocking_send` on a full bounded channel from the debouncer thread.
* Using an unbounded channel with no backlog metric.
* Dropping a full-channel event with no reconciliation signal.
* Allowing stale parse tasks to commit after newer changes.
* Coupling debouncer lifetime to a short-lived request task.
* Creating one Tokio runtime per event.

---

## 20.9 Tokio checklist

```text
[ ] Synchronous callback only hands off.
[ ] Async channel capacity policy is explicit.
[ ] Queue-full condition triggers reconciliation or equivalent safety path.
[ ] Unique dirty paths are coalesced where possible.
[ ] Per-path generation prevents stale commits.
[ ] Parser work has a bounded execution pool.
[ ] Shutdown stops source before dropping receiver.
[ ] Error origin is preserved across async boundary.
```

---

# notify-debouncer-full Advanced — 21) Backpressure, burst control, and work coalescing

## 21.0 Three independent queues

A production watcher pipeline can have at least three queueing stages:

```text
OS/backend queue
  → debouncer per-path event queues
  → application work queue / dirty set
  → parser/index execution queue
```

Debouncing controls only the middle normalization stage. It does not automatically bound kernel queues or your application worker backlog.

---

## 21.1 Burst example

```text
git checkout changes 20,000 files
  ↓
backend emits many events
  ↓
debouncer normalizes duplicates/renames
  ↓
still thousands of meaningful dirty paths
  ↓
application must schedule bounded analysis
```

The correct response is not to increase the debounce timeout until the burst disappears. The final filesystem really did change across thousands of files.

---

## 21.2 Dirty set versus FIFO

FIFO event queue:

```text
foo.rs
bar.rs
foo.rs
foo.rs
baz.rs
bar.rs
```

Dirty set:

```text
{foo.rs, bar.rs, baz.rs}
```

For a current-state index, the dirty set is usually superior because intermediate versions do not need to be analyzed.

Exception: if the product genuinely consumes every historical change (audit/version capture), a filesystem watcher is already the wrong primitive for guaranteed history; use a durable source/journal/VCS integration.

---

## 21.3 Batch draining

Worker strategy:

```text
wake
  → drain up to N dirty paths
  → group by module/package if useful
  → read/hash
  → parse in bounded parallel pool
  → compute graph delta
  → commit batch
  → repeat while dirty set nonempty
```

This amortizes database/graph transaction overhead and gives scheduler control.

---

## 21.4 Burst-to-rescan threshold

At extreme volume, incremental work can become less efficient than reconciliation.

Example policy:

```text
if dirty_unique_paths > 25_000
or queue_age > 10 s
or backend signals rescan
then:
  switch workspace to reconcile mode
  coalesce further events
  inventory current tree
  calculate minimal delta
```

Thresholds are workload-specific; measure before hardcoding.

---

## 21.5 Backlog age beats queue length alone

Metrics:

```text
dirty_paths_current
oldest_dirty_path_age_seconds
parser_jobs_running
parser_jobs_queued
index_commit_latency_seconds
watch_to_index_latency_seconds
```

A queue of 5,000 cheap files can be healthier than 50 files stalled behind a pathological analysis. Track age and end-to-end freshness.

---

## 21.6 Fairness across workspaces

If one service watches multiple repositories:

Bad:

```text
single FIFO queue
repo A checkout 100k files
→ repo B interactive save waits minutes
```

Better:

```text
per-workspace dirty sets
+ fair scheduler / weighted quotas
+ interactive priority
+ concurrency caps
```

Watcher delivery and parser scheduling should be independent layers.

---

## 21.7 Backpressure anti-patterns

* Assuming debounce window solves large checkouts/build bursts.
* One unbounded FIFO for every raw/debounced event.
* Re-parsing the same file multiple times while backlog exists.
* Starving small active repositories behind one giant repository.
* No transition from incremental mode to reconciliation under overload.
* Dropping batches on queue overflow without marking state uncertain.
* Measuring only handler throughput, not watch-to-index freshness.

---

## 21.8 Backpressure checklist

```text
[ ] Distinguish backend, debouncer, and application queues.
[ ] Use unique dirty-path coalescing for current-state indexes.
[ ] Bound parser concurrency.
[ ] Batch graph/index commits.
[ ] Measure oldest backlog age.
[ ] Add overload → reconciliation policy.
[ ] Schedule fairly across workspaces.
[ ] Treat dropped/overflowed application batches as consistency events.
```

---

# notify-debouncer-full Advanced — 22) Editor save patterns and logical-change interpretation

## 22.0 Why editor behavior matters

`notify` explicitly notes that editors may save differently: some modify a file in place, while others replace the original with a newly written file. [S8]

Therefore:

```text
user action: Ctrl+S
≠ stable filesystem event sequence
```

The debouncer improves normalization, but application code should still interpret events as state invalidations.

---

## 22.1 In-place save

Potential sequence:

```text
Modify metadata
Modify data
Modify data
Access close/write
```

After debouncing, the application may see one or a few modify semantics depending on exact kinds/timing.

Desired code-index behavior:

```text
mark file dirty once
read final current bytes
hash
parse if changed
```

---

## 22.2 Atomic replace save

Potential sequence:

```text
Create .foo.rs.tmp
Modify .foo.rs.tmp
Rename foo.rs → backup or remove old
Rename .foo.rs.tmp → foo.rs
Remove backup
```

The full debouncer's rename matching, queue path updating, duplicate-create suppression, and modify-after-create suppression are designed to make these bursts more coherent. [S1] [S3]

Still, application logic should focus on:

```text
What exists at foo.rs now?
What are its bytes/hash now?
```

---

## 22.3 Formatter-on-save

```text
editor writes version A
formatter rewrites version B 30 ms later
```

With a 75 ms timeout, the first event can still become eligible independently if enough time passes; this is not a strict “wait for complete silence forever” debounce. [S3]

If the product wants to avoid analyzing version A, add application-level per-path settle/generation logic or let dirty-path coalescing supersede stale work.

---

## 22.4 Language server/build tool writes

A save may trigger:

```text
source write
build script output
formatter
code generator
lockfile update
metadata cache
```

Filtering high-churn generated locations before parsing is essential. If generated source itself is part of the model, treat it as a separate workload class or lower-priority queue.

---

## 22.5 Swap and backup files

Examples:

```text
.foo.rs.swp
foo.rs~
.#foo.rs
foo.rs.tmp
```

Do not index based solely on “text-like extension appears somewhere in filename.” Use explicit include/ignore policy.

---

## 22.6 Save-pattern acceptance tests

Test with target tools, not synthetic writes only:

```text
VS Code save
Cursor save
rustfmt on save
JetBrains save/safe-write
Vim/Neovim save
Emacs save
cargo fmt
code generator used by project
```

The exact set depends on your users. Assert final indexed hash and semantic graph, not exact event count.

---

## 22.7 Editor anti-patterns

* Hardcoding one editor's raw event sequence.
* Treating rename as always a user-initiated rename rather than atomic save.
* Indexing `.tmp`, swap, backup, and formatter intermediates indiscriminately.
* Assuming debounce guarantees only the final formatter result is seen.
* Running analysis directly on event payload without reopening target path.
* Cross-platform tests asserting identical event sequences.

---

## 22.8 Editor checklist

```text
[ ] Test actual editors used by product/team.
[ ] Treat save as invalidation, not event verb replay.
[ ] Re-read final target path.
[ ] Filter temp/swap/backup patterns.
[ ] Handle formatter-on-save with stale-generation protection.
[ ] Assert final hash/index outcome across editors/platforms.
```

---

# notify-debouncer-full Advanced — 23) Path identity, normalization, symlinks, and filesystem races

## 23.0 Path is not file identity

```text
PathBuf
  = current name/location in a namespace

file ID
  = filesystem identity hint used by cache/correlation

content hash
  = current byte identity

semantic entity ID
  = application/compiler/index identity
```

Do not collapse these concepts.

---

## 23.1 Preserve `PathBuf`

Use `Path`/`PathBuf` end to end inside watcher infrastructure.

Bad:

```rust
let path = event.paths[0].to_string_lossy().to_string();
```

as canonical identity.

Problems:

* non-UTF-8 names can be lossy on Unix;
* Windows path semantics are more complex than lowercase strings;
* separators/prefixes can vary;
* string normalization can accidentally merge distinct paths.

Convert to display strings only for logs/UI.

---

## 23.2 Absolute versus relative keys

Choose one application invariant:

```text
watch roots stored as canonical absolute paths
index keys stored as workspace-relative normalized PathBuf-like components
```

Typical mapping:

```text
absolute event path
  → determine owning workspace/root
  → strip root prefix
  → validate path remains within workspace
  → store workspace-relative path key
```

This keeps index records portable if workspace root moves.

---

## 23.3 `canonicalize()` caveat

`std::fs::canonicalize` resolves symlinks and requires path existence. That makes it inappropriate as the only identity operation for remove events:

```text
Remove(path)
  → path no longer exists
  → canonicalize(path) fails
```

Keep lexical/root-relative identity available independently of current existence.

---

## 23.4 Rename and identity

For an index:

```text
rename old → new
```

can preserve some application identity:

```text
file record ID
VCS-like identity heuristic
symbol relationships after reparse
```

but a filesystem rename alone does not prove content equality by the time you process it. Re-stat/read/hash `new` when correctness matters.

---

## 23.5 Symlinks

There are two separate symlink controls to reason about:

```text
notify watcher backend follow_symlinks config
file-ID cache traversal behavior
```

`notify::Config` follows symlinks by default for relevant backends. [S13]

The full debouncer `FileIdMap` cache traversal uses `WalkDir::follow_links(true)`. [S12]

Implications:

* watch scope can escape the apparent repository tree through links;
* large external trees can increase work;
* cycles need underlying walker handling;
* security boundaries should not rely on textual root prefix alone if symlinks are allowed.

---

## 23.6 TOCTOU races

Between event delivery and processing:

```text
file can be renamed again
file can be deleted
file can be recreated with different content
permissions can change
```

Therefore handle expected filesystem races as ordinary outcomes:

```rust
match std::fs::read(path) {
    Ok(bytes) => analyze(bytes),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        // Treat as removed/currently absent.
    }
    Err(e) => {
        // classify retryable/permanent
    }
}
```

Do not retry `NotFound` indefinitely just because the event was `Modify`.

---

## 23.7 Root escape validation

For multi-tenant/workspace services:

```text
event path
  → lexical root association
  → if following symlinks / reading resolved target:
      enforce resolved target policy
  → reject/ignore paths outside authorized workspace as required
```

Watcher notifications are not an authorization mechanism.

---

## 23.8 Path anti-patterns

* `to_string_lossy()` as canonical database key.
* Lowercasing all paths on every platform.
* Requiring `canonicalize()` for remove handling.
* Assuming rename preserves unchanged content.
* Ignoring symlink expansion in security/resource analysis.
* Trusting event path as authorized input without workspace validation.
* Retrying `NotFound` forever after stale modify events.

---

## 23.9 Path checklist

```text
[ ] Keep native PathBuf through watcher pipeline.
[ ] Define workspace-relative index-key policy.
[ ] Keep remove identity independent of canonicalize().
[ ] Re-read/hash rename target as needed.
[ ] Decide symlink policy explicitly.
[ ] Validate resolved scope for security-sensitive services.
[ ] Treat filesystem races as normal states.
```

---

# notify-debouncer-full Advanced — 24) Code-intelligence / property-graph architecture

## 24.0 Recommended end-to-end model

```text
notify-debouncer-full
  → normalized filesystem invalidations
  → event classifier
  → dirty-path set + rescan flag
  → file-state resolver
      exists?
      size/mtime?
      content hash?
  → incremental syntax parser
  → semantic extractor
  → dependency/blast-radius resolver
  → graph delta builder
  → atomic graph commit
  → validation / consistency metrics
```

The watcher is the **front-door invalidation sensor**, not the graph update engine.

---

## 24.1 State model

```rust
struct FileRecord {
    file_id: u64,              // application identity, not notify file ID
    relative_path: PathBuf,
    content_hash: ContentHash,
    language: Language,
    parse_generation: u64,
    graph_generation: u64,
}
```

Maintain mappings:

```text
workspace + path → application file record
file record      → syntax tree cache
file record      → symbols/edges/properties
symbol           → source ranges/file record
```

A filesystem rename updates the path mapping; a parse determines whether semantic entities also persist.

---

## 24.2 Change pipeline

### Create/modify

```text
path dirty
  → inclusion check
  → read current bytes
  → hash
  → hash equals indexed hash?
       yes → clear dirty
       no  → parse/extract
  → calculate semantic delta
  → commit
```

### Remove

```text
path absent
  → remove file record or tombstone
  → remove file-owned graph nodes/edges
  → invalidate dependents
  → commit
```

### Rename

```text
from → to
  → classify include/exclude boundary
  → locate prior file record at from
  → read/hash to if present
  → preserve application file record when policy/evidence supports
  → update path
  → reparse if bytes/context/module path can change semantics
  → update graph
```

In Rust, a file rename can change module resolution even if bytes are identical, so “same content hash” does not always imply “same semantic graph.”

---

## 24.3 Semantic invalidation versus content invalidation

Example:

```text
src/foo.rs → src/bar.rs
```

Bytes unchanged, but Rust module path/name relationships may change.

Therefore distinguish:

```text
content invalidation:
  file bytes changed

location invalidation:
  path/module/package context changed

dependency invalidation:
  imported/resolved target changed

configuration invalidation:
  Cargo.toml, cfg, feature, build metadata changed
```

The watcher reports paths; your semantic engine decides invalidation radius.

---

## 24.4 Two-stage invalidation

Recommended:

```text
Stage 1 — cheap file-state delta
  path/existence/hash/language

Stage 2 — semantic delta
  parser tree diff
  definitions/references/imports
  build/module resolution
  call graph / type relationships
```

Do not trigger whole-graph recomputation on every source modify if dependency analysis can bound the affected region.

---

## 24.5 Atomic graph commit

For each processing generation:

```text
read current state
  → compute candidate delta
  → validate delta invariants
  → atomic transaction:
       file record update
       symbol/property changes
       edge changes
       dependency version
  → publish graph generation
```

If the file changes during analysis, generation fencing should reject the stale delta and requeue the path.

---

## 24.6 Handling configuration files

Files such as:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
build.rs
.cargo/config.toml
workspace manifests
feature/config files
```

can invalidate semantic resolution much more broadly than a single `.rs` file.

Policy example:

```text
.rs changed
  → file + affected dependents

Cargo.toml changed
  → crate/workspace dependency model reconcile
  → re-resolve module/dependency graph

rust-toolchain / cfg inputs changed
  → compiler-environment invalidation
```

Do not filter only source extensions.

---

## 24.7 `need_rescan()` in a graph system

```text
need_rescan
  → filesystem inventory uncertain
  → do not attempt graph repair from event history
  → inventory + hash reconcile
  → derive file-level delta
  → feed same semantic incremental engine
```

This reuse is important: reconciliation should produce the same `FileDelta` abstraction as incremental events, rather than maintaining a separate “full rebuild” code path where possible.

---

## 24.8 Canonical `FileDelta`

```rust
#[derive(Debug)]
enum FileDelta {
    Added {
        path: PathBuf,
        hash: ContentHash,
    },
    Changed {
        path: PathBuf,
        old_hash: ContentHash,
        new_hash: ContentHash,
    },
    Removed {
        path: PathBuf,
    },
    Renamed {
        from: PathBuf,
        to: PathBuf,
        content_changed: bool,
    },
}
```

The watcher layer can *suggest* a rename, but the state resolver should produce the authoritative `FileDelta` consumed by the graph layer.

---

## 24.9 High-confidence architecture rule

```text
notify event
  is not
FileDelta

notify event
  causes
filesystem state resolution
  which yields
FileDelta
```

This isolates platform/editor variance from graph semantics.

---

## 24.10 Code-intelligence metrics

```text
watch_event_batches_total
watch_debounced_events_total
dirty_paths_current
file_state_resolve_seconds
hash_seconds
parse_seconds
semantic_extract_seconds
graph_delta_nodes
graph_delta_edges
graph_commit_seconds
stale_generation_discard_total
reconciliation_total
watch_to_graph_freshness_seconds
```

---

## 24.11 Code-intelligence anti-patterns

* Mutating graph directly from `EventKind`.
* Assuming same file bytes at a new Rust path preserve all semantics.
* Re-parsing before hashing current bytes.
* Whole-repository graph rebuild for every save.
* Ignoring Cargo/workspace/config changes.
* Committing stale parse results after a newer save.
* Treating rescan recovery as a separate untested architecture.
* Using notify's cached file ID as the application's permanent graph entity ID.

---

## 24.12 Code-intelligence checklist

```text
[ ] Watcher only emits invalidation signals into application layer.
[ ] Resolve authoritative current filesystem state.
[ ] Convert state to FileDelta abstraction.
[ ] Hash before expensive parse when useful.
[ ] Separate content/location/config/dependency invalidation.
[ ] Fence asynchronous generations.
[ ] Commit graph deltas atomically.
[ ] Reuse FileDelta path for reconciliation.
[ ] Watch manifests/config in addition to source files.
[ ] Measure watch-to-graph freshness.
```

---

# notify-debouncer-full Advanced — 25) Hot reload, build triggers, and process restart patterns

## 25.0 Hot-reload model

```text
filesystem invalidation
  → debounce
  → relevance filter
  → build/reload generation
  → cancel/supersede older generation
  → restart/reload only when current generation succeeds
```

This is a different workload from code indexing: the downstream action may be expensive and side-effecting, so cancellation/supersession matters more than preserving every intermediate path.

---

## 25.1 Simple command trigger

```text
any relevant change batch
  → set build_requested=true
  → if build running:
      let it finish or cancel according to tool
  → start one newest build
```

Do not spawn a new `cargo check` per event.

---

## 25.2 Restart-after-success

For a development server:

```text
source/config dirty
  → build generation N
  → build succeeds?
      no  → keep old server alive, report diagnostics
      yes → gracefully stop old server
           start new artifact
```

This minimizes downtime and avoids restarting on transient half-written states.

---

## 25.3 Full debouncer versus watchexec

If the primary requirement is:

```text
watch paths
filter
restart/cancel child process
```

higher-level tooling such as the watchexec ecosystem may provide more process-lifecycle functionality out of the box.

Use `notify-debouncer-full` when filesystem events are an input to your own long-lived application logic or custom index/state machine.

---

## 25.4 Hot-reload anti-patterns

* Spawn one process per changed path.
* Restart on Access events.
* Kill healthy server before replacement build succeeds when avoidable.
* Let old build generation restart after a newer generation has completed.
* Treat build output directory changes as source input and create feedback loops.

---

## 25.5 Hot-reload checklist

```text
[ ] Filter source/config inputs from build outputs.
[ ] Coalesce changes into build generations.
[ ] Supersede stale builds/restarts.
[ ] Restart only from latest successful generation.
[ ] Prevent output→watch feedback loops.
[ ] Consider higher-level process watcher if process lifecycle is primary need.
```

---

# notify-debouncer-full Advanced — 26) Large repositories and performance engineering

## 26.0 Cost stack

```text
startup:
  watcher construction
  recursive root registration
  optional FileIdMap tree walk

steady state:
  backend notification cost
  debounce queue maintenance
  handler handoff
  filtering
  stat/read/hash
  parsing/semantic work

burst:
  backend queue pressure
  thousands of per-path queues
  downstream dirty-set growth
  graph transaction pressure
```

Measure each layer separately.

---

## 26.1 Startup benchmark

Record:

```text
workspace file count
workspace directory count
watch registration duration
initial FileIdMap population duration if applicable
RSS before/after watcher registration
backend kind
```

On Linux `RecommendedCache` is `NoCache`; on macOS/Windows the recommended cache uses file-ID mapping and recursive tree walking, so startup characteristics differ. [S7] [S12]

---

## 26.2 High-churn directory exclusion

Largest wins often come from not observing irrelevant churn:

```text
target/
node_modules/
.git/objects/
coverage/
logs/
generated caches/
```

If those reside beneath one recursive root, event filtering helps downstream CPU but may not fully eliminate backend resource cost.

Architect watch roots around repository topology where scale justifies it.

---

## 26.3 Timeout tuning benchmark

Benchmark at least:

```text
single-character save
save + formatter
10-file refactor
1000-file git checkout
large branch switch
cargo-generated churn
```

For each timeout/tick pair capture:

```text
raw-ish backend events if instrumentable
DebouncedEvent count
unique dirty paths
watch→dirty latency
watch→index latency
CPU
memory
reconciliation frequency
```

Choose timeout based on end-to-end work reduction, not event count alone.

---

## 26.4 Hash strategy

Potential hash tiers:

```text
metadata only
  fastest, weaker change detection

fast noncryptographic content hash
  good local invalidation identity

cryptographic hash
  stronger durable identity, more CPU

parser-native incremental tree identity
  can avoid full semantic recomputation after read
```

Watcher choice does not mandate hash algorithm. Pick based on index correctness and collision tolerance.

---

## 26.5 Parsing concurrency

More parser threads are not always faster:

```text
filesystem reads contend
memory allocation rises
compiler/LSP subprocesses contend
shared graph locks contend
cache locality degrades
```

Benchmark bounded pools: 1, 2, 4, 8, etc. independently of watcher timing.

---

## 26.6 Graph commit batching

Instead of:

```text
1000 dirty files
→ 1000 independent graph transactions
```

consider:

```text
parse in parallel
→ collect validated deltas in bounded batch
→ one/few atomic graph commits
```

Balance commit latency against interactive freshness.

---

## 26.7 Large-repo anti-patterns

* Recursive watch of entire home directory.
* FileIdMap across massive irrelevant trees without measuring startup.
* Parser concurrency equal to CPU count by reflex.
* Event count used as sole performance metric.
* No branch-switch/git-checkout benchmark.
* No overload-to-reconcile path.
* One graph transaction per raw/debounced event.
* Enabling PollWatcher content hashing across huge remote trees blindly.

---

## 26.8 Performance checklist

```text
[ ] Benchmark startup separately from steady state.
[ ] Record backend and cache type.
[ ] Exclude high-churn irrelevant roots where possible.
[ ] Tune timeout/tick on realistic edit and VCS workloads.
[ ] Measure unique dirty paths, not only delivered event count.
[ ] Bound parser concurrency experimentally.
[ ] Batch graph commits where beneficial.
[ ] Test overload/reconciliation behavior.
[ ] Measure end-to-end freshness.
```

---

# notify-debouncer-full Advanced — 27) Multiple workspaces, watcher ownership, and isolation

## 27.0 Architecture choices

### One debouncer, many roots

```text
Debouncer A
  ├─ watch workspace 1
  ├─ watch workspace 2
  └─ watch workspace 3
```

Advantages:

* fewer watcher/debouncer threads;
* centralized delivery;
* straightforward small-app architecture.

Disadvantages:

* backend error/rescan scope can be harder to map;
* one hot workspace increases shared queue pressure;
* all roots share timeout/backend/config/cache type.

### One debouncer per workspace

```text
Workspace 1 → Debouncer 1
Workspace 2 → Debouncer 2
Workspace 3 → Debouncer 3
```

Advantages:

* isolation;
* per-workspace backend/timing policy;
* simpler health ownership;
* easier restart/reconciliation of one workspace.

Disadvantages:

* more threads/native watcher objects/resources;
* potentially duplicated watches for overlapping roots.

---

## 27.1 Choose isolation domain intentionally

Good boundaries:

```text
one desktop IDE workspace
one tenant repository
one remotely mounted project
one backend-policy domain
```

Do not create one watcher per file.

---

## 27.2 Root overlap

Avoid:

```text
watch repo/ recursively
watch repo/src/ recursively
```

unless duplicate observation is intentional. Overlapping roots can create duplicate events and extra native resources.

Maintain a root registry that rejects or normalizes redundant registrations.

---

## 27.3 Workspace routing

If one debouncer owns many roots:

```text
event path
  → longest matching registered root
  → workspace ID
  → workspace dirty set
```

Use longest-prefix root ownership so nested workspace roots are handled deliberately.

---

## 27.4 Per-workspace health

Even under one debouncer, expose:

```text
workspace watch roots
last event time
last reconcile time
dirty path count
index freshness
root exists/readable
```

Global watcher health alone is insufficient.

---

## 27.5 Dynamic workspace lifecycle

```text
open workspace
  → construct or register roots
  → initial inventory/reconciliation
  → start accepting incremental updates

close workspace
  → unwatch roots
  → stop downstream jobs
  → flush/drop index according to product
```

Do not mark a workspace “ready” merely because `watch()` succeeded; initial state must be inventoried independently.

---

## 27.6 Multi-workspace anti-patterns

* One debouncer per file.
* Overlapping recursive roots without intent.
* One global dirty FIFO that allows one repo to starve all others.
* No workspace attribution in errors/events.
* Considering `watch()` success equivalent to initial index correctness.
* Closing workspace by dropping index while watcher remains registered.

---

## 27.7 Multi-workspace checklist

```text
[ ] Choose debouncer isolation domain deliberately.
[ ] Reject accidental overlapping roots.
[ ] Route path to workspace deterministically.
[ ] Maintain per-workspace dirty set and health.
[ ] Initial reconcile occurs before READY.
[ ] Unwatch before destroying workspace processing state.
[ ] Schedule work fairly across workspaces.
```

---

# notify-debouncer-full Advanced — 28) Metrics, tracing, and observability

## 28.0 Observability layers

```text
watch configuration
  backend kind / roots / timeout / tick

watch delivery
  event batches / events / errors / rescans

application queue
  dirty paths / backlog age / overflows

processing
  stat/hash/parse/semantic/commit durations

correctness
  reconciliation drift found / stale generations / index freshness
```

A watcher should be observable as a freshness pipeline, not just a callback counter.

---

## 28.1 Startup log

Example structured log:

```text
watcher_started
  workspace=repo-123
  backend=Inotify
  roots=[/workspace/repo]
  recursive=true
  debounce_timeout_ms=75
  tick_rate_ms=18
  cache=NoCache
```

Do not log only “watcher started.”

---

## 28.2 Core counters

```text
watch_batches_ok_total
watch_batches_error_total
watch_events_delivered_total
watch_rescan_signals_total
watch_registration_errors_total
watch_unwatch_errors_total
watch_queue_overflow_total
watch_reconcile_total
watch_reconcile_failure_total
watch_stale_result_discard_total
```

---

## 28.3 Event-kind counters

```text
watch_event_create_total
watch_event_modify_total
watch_event_remove_total
watch_event_rename_total
watch_event_access_total
watch_event_other_total
```

Use these to diagnose editor/backend changes, not as correctness contracts.

---

## 28.4 Latency histograms

Most valuable:

```text
watch_handler_duration_seconds
watch_to_dirty_seconds
watch_to_parse_start_seconds
watch_to_index_commit_seconds
file_hash_duration_seconds
file_parse_duration_seconds
graph_commit_duration_seconds
reconcile_duration_seconds
```

For interactive tooling, `watch_to_index_commit_seconds` is the user-visible freshness metric.

---

## 28.5 Backlog gauges

```text
watch_dirty_paths
watch_oldest_dirty_age_seconds
watch_parser_jobs_running
watch_parser_jobs_queued
watch_reconcile_in_progress
watch_index_generation_lag
```

---

## 28.6 Trace correlation

Assign an application generation/batch ID when work enters the dirty scheduler:

```text
watch batch 8124
  → dirty generation 9921
  → parse jobs [foo.rs#44, bar.rs#12]
  → graph commit 7710
```

Do not expect `notify` event tracker IDs to serve as global trace IDs; tracker attributes are backend-specific event metadata.

---

## 28.7 Sampling paths safely

Paths can contain sensitive project/user names. In telemetry sent outside the local machine:

```text
prefer:
  workspace ID
  extension/category
  hashed relative path if needed

avoid by default:
  full absolute user paths
  source contents
  cloud/share credentials embedded in paths
```

---

## 28.8 Health status

Example:

```rust
#[derive(Debug, Clone, Copy)]
enum WatchHealth {
    Starting,
    Healthy,
    Reconciling,
    Degraded,
    Stopped,
}
```

Transitions should be driven by actual state:

```text
Starting → initial reconcile → Healthy
Healthy → rescan signal → Reconciling
Reconciling → success → Healthy
Reconciling → failure → Degraded
* → shutdown → Stopped
```

---

## 28.9 Observability anti-patterns

* Only counting callbacks.
* No backend kind in diagnostics.
* No rescan counter.
* No dirty-backlog age.
* Treating event count reduction as user-visible latency improvement by itself.
* Logging full source paths/content to centralized telemetry by default.
* Using backend tracker/cookie as distributed trace identity.
* Health endpoint says “healthy” while reconciliation repeatedly fails.

---

## 28.10 Observability checklist

```text
[ ] Log backend, roots, timeout, tick, cache at startup.
[ ] Count success/error batches and delivered events.
[ ] Count rescan signals/reconciliations.
[ ] Measure handler latency.
[ ] Measure watch-to-index freshness.
[ ] Measure dirty backlog size and age.
[ ] Correlate application generations through parse/commit.
[ ] Protect path/source privacy in telemetry.
[ ] Expose explicit Healthy/Reconciling/Degraded state.
```

---
# notify-debouncer-full Advanced — 29) Testing and correctness

## 29.0 Testing philosophy

Filesystem watcher tests fail when they confuse two contracts:

```text
Contract A — library normalization behavior
  exact-ish event relationships matter
  e.g. matched rename becomes RenameMode::Both

Contract B — application correctness
  final state matters
  e.g. index converges to current filesystem contents
```

Use narrow event assertions for Contract A and state-convergence assertions for Contract B.

Do not build a cross-platform product test suite around one Linux/editor raw event trace.

---

## 29.1 Test layers

```text
Layer 1: pure unit tests
  synthetic Event / DebouncedEvent classifiers
  path filters
  rename-scope truth tables
  dirty-set scheduler
  FileDelta generation

Layer 2: filesystem integration tests
  real temporary directory
  real RecommendedWatcher / PollWatcher
  create/modify/remove/rename
  bounded wait with timeout

Layer 3: application convergence tests
  mutate fixture tree
  wait for processing quiescence
  compare index/graph inventory + hashes to filesystem

Layer 4: platform/deployment tests
  Linux native
  macOS native
  Windows native
  WSL/network/shared mount if supported
  container volume topology

Layer 5: stress/fault tests
  large burst
  queue overflow policy
  forced reconciliation
  root delete/recreate
  downstream failures
```

---

## 29.2 Pure event classifier test

```rust
use notify_debouncer_full::notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind,
};
use std::path::PathBuf;

fn rename_event(from: &str, to: &str) -> Event {
    Event::new(EventKind::Modify(
        ModifyKind::Name(RenameMode::Both),
    ))
    .add_path(PathBuf::from(from))
    .add_path(PathBuf::from(to))
}

#[test]
fn extracts_rename_scope() {
    let event = rename_event("src/a.rs", "src/b.rs");
    assert_eq!(event.paths.len(), 2);
    assert_eq!(event.paths[0], PathBuf::from("src/a.rs"));
    assert_eq!(event.paths[1], PathBuf::from("src/b.rs"));
}
```

Use synthetic events for deterministic application classification tests. Do not use them as proof that a real backend emits that shape under every OS/editor.

---

## 29.3 Temporary-directory integration harness

A simple harness needs:

```text
temporary root
watcher startup
channel receiver
filesystem mutation
bounded event collection
assertion
explicit shutdown
```

Illustrative pattern:

```rust
use notify_debouncer_full::{
    new_debouncer,
    notify::RecursiveMode,
    DebounceEventResult,
};
use std::{
    fs,
    sync::mpsc,
    time::{Duration, Instant},
};

fn collect_until<F>(
    rx: &mpsc::Receiver<DebounceEventResult>,
    deadline: Duration,
    mut done: F,
) -> Vec<notify_debouncer_full::DebouncedEvent>
where
    F: FnMut(&[notify_debouncer_full::DebouncedEvent]) -> bool,
{
    let started = Instant::now();
    let mut all = Vec::new();

    while started.elapsed() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(mut events)) => {
                all.append(&mut events);
                if done(&all) {
                    break;
                }
            }
            Ok(Err(errors)) => panic!("watch errors: {errors:?}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    all
}
```

Use a bounded deadline so a failed watcher cannot hang CI indefinitely.

---

## 29.4 Avoid fixed sleeps as assertions

Bad:

```rust
std::thread::sleep(Duration::from_millis(200));
assert_eq!(events.len(), 1);
```

Problems:

* CI scheduling varies;
* backend latency differs;
* debounce timeout/tick adds jitter;
* polling backend latency is different;
* event count can differ while final semantics remain correct.

Better:

```text
wait until condition true OR deadline exceeded
then assert semantic result
```

Use sleeps only as controlled mutation spacing when a test specifically needs two events separated by a known debounce relationship.

---

## 29.5 Create test

Application-level assertion:

```text
1. start watcher
2. create src/new.rs with known bytes
3. wait until application index contains src/new.rs
4. assert indexed hash == current file hash
```

Library-level optional assertion:

```text
at least one delivered event has Create semantics for new.rs
OR application classifier receives equivalent current-state invalidation
```

Do not require exactly one raw/debounced event unless testing the crate's duplicate-create behavior specifically.

---

## 29.6 Modify test

```text
1. initial file/index hash H1
2. write new bytes H2
3. wait for index generation to advance
4. assert current indexed hash == H2
5. assert graph reflects H2 semantics
```

Also test a no-op rewrite with identical bytes:

```text
rewrite same bytes
  → watcher may emit
  → file-state resolver hashes same
  → parser/graph generation should not change if policy skips no-op
```

---

## 29.7 Rename normalization test

For an actual same-filesystem rename:

```rust
fs::rename(root.join("a.rs"), root.join("b.rs"))?;
```

Wait for a normalized event:

```text
EventKind::Modify(Name(Both))
paths[0] == a.rs
paths[1] == b.rs
```

But make the assertion conditional on the backend semantics the crate documents and the test platform. The full debouncer is specifically intended to match rename halves when tracker/file-ID evidence permits. [S1] [S3]

Application-level assertion should always be:

```text
a.rs absent from index
b.rs present
b.rs hash matches filesystem
```

---

## 29.8 Chained rename test

Test:

```text
a.rs → b.rs → c.rs
```

within the debounce period.

The debouncer contains logic to merge multiple rename operations and update queued paths, preserving the original source path while moving the final destination forward. [S1] [S3]

Expected semantic shape worth testing under the pinned crate:

```text
normalized rename source = a.rs
normalized final target = c.rs
```

Also assert final filesystem/index state independently.

---

## 29.9 Rename-over-existing-target test

Test:

```text
a.rs exists
b.rs exists
rename/replace a.rs over b.rs according to platform-supported operation
```

The debouncer's rename merging logic can synthesize a removal for an overridden target with event info indicating override behavior. [S3]

This is a subtle regression target because index semantics must remove/replace the previous identity at `b.rs` correctly.

Application assertion:

```text
only final b.rs identity/content remains
no stale graph nodes owned by previous b.rs
```

---

## 29.10 Directory deletion test

Fixture:

```text
src/generated/a.rs
src/generated/nested/b.rs
```

Delete `src/generated/` recursively.

Expected application outcome:

```text
both file records removed
all graph nodes/edges owned by subtree removed
```

Do not require one remove event per child; the full debouncer intentionally collapses directory removal behavior on inotify. [S1] [S3]

---

## 29.11 Atomic-save test

Simulate safe-write:

```text
write .foo.rs.tmp
fsync/close if desired
rename tmp over foo.rs
```

Assert:

```text
foo.rs final indexed hash == replacement bytes
no .tmp record remains
no duplicate graph updates that violate generation invariants
```

This test is more representative than calling `fs::write(foo.rs, ...)` alone.

---

## 29.12 Formatter-on-save test

Sequence:

```text
write unformatted bytes A
wait 10–30 ms
write formatted bytes B
```

With a normal interactive debounce configuration, the application may begin or even complete analysis of A depending on timing. The correctness contract is:

```text
final committed generation == B
stale A result cannot overwrite B
```

This tests your generation fence rather than pretending timeout selection guarantees silence.

---

## 29.13 Filter-boundary rename tests

Truth table:

```text
included → included : move/reparse
included → ignored  : remove from index
ignored  → included : add to index
ignored  → ignored  : no index change
```

Create one test for each. This catches a common bug where only the destination path is filtered.

---

## 29.14 Initial-state test

A watcher does not emit historical create events for every file already present merely because you start watching.

Test service startup:

```text
fixture files exist before watcher starts
  → initial inventory/reconcile indexes them
  → watcher handles subsequent changes
```

Do not rely on watcher events to bootstrap initial state.

---

## 29.15 Rescan-path test without real kernel overflow

Real overflow is difficult and nondeterministic to induce in CI. Test the application recovery path directly:

```text
1. mutate filesystem behind index representation
2. inject/set reconciliation-required signal
3. run reconciliation
4. assert index == filesystem
```

Separately, unit-test that a synthetic event with the Rescan flag causes the signal.

This tests your correctness architecture even if the backend-specific overflow trigger is not deterministic.

---

## 29.16 Application queue overflow test

If bounded handoff uses `try_send`:

```text
1. force worker to block
2. fill channel
3. deliver another batch
4. assert overflow counter increments
5. assert reconciliation_required=true
6. release worker
7. run reconciliation
8. assert final state converges
```

Never test only “we dropped an event successfully.”

---

## 29.17 Root deletion/recreation test

If your product supports it:

```text
watch workspace
remove workspace root
recreate workspace root
write new source file
```

Assert supervisor behavior:

```text
root missing detected
watch registration/lifecycle updated
recreated root reconciled
new changes observed
```

Whether a parent watch is needed depends on your lifecycle architecture and backend behavior. [S8]

---

## 29.18 PollWatcher test profile

Use a shorter explicit poll interval in tests:

```rust
Config::default()
    .with_poll_interval(Duration::from_millis(100))
```

Then set test deadlines comfortably above:

```text
poll interval + debounce timeout + tick jitter + CI scheduling
```

Do not use the 30-second default in unit/integration tests. [S13]

---

## 29.19 Cross-platform CI matrix

Minimum if the product claims all three desktop OSes:

```text
Linux:
  RecommendedWatcher / inotify
  create/modify/rename/remove

macOS:
  RecommendedWatcher / FSEvents
  atomic save + rename

Windows:
  RecommendedWatcher / ReadDirectoryChangesW
  path/rename/replace
```

Optional deployment-specific jobs:

```text
WSL mounted Windows path
Docker Desktop/macOS bind mount
NFS/CIFS share
PollWatcher profile
```

---

## 29.20 Timing assertions

Avoid:

```text
must deliver between 75 and 80 ms
```

Prefer:

```text
not before a broad lower bound if testing debounce eligibility
must arrive before generous upper deadline
```

OS scheduling, thread wakeups, filesystem write completion, and CI load all introduce jitter.

---

## 29.21 Deterministic event sorting test

The full debouncer sorts eligible events while preserving same-path order and interleaving different path queues by timestamp. [S3]

If your application relies on the crate's normalized ordering, create pinned-version tests for representative event batches. Do not extrapolate this into a durable global filesystem transaction order.

---

## 29.22 Test helper: eventual assertion

Conceptual helper:

```rust
fn eventually(
    timeout: Duration,
    mut predicate: impl FnMut() -> bool,
) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if predicate() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("condition did not become true within {timeout:?}");
}
```

Use to test **state** becoming correct rather than sleeping and guessing.

---

## 29.23 Correctness oracle

For an indexed repository, write one reusable oracle:

```text
enumerate all included filesystem files
  → map relative path → content hash

read all indexed file records
  → map relative path → indexed content hash

assert maps equal
```

Then optionally validate graph invariants:

```text
no graph node owned by missing file
all source ranges within current file bounds
all file records reference current graph generation
no duplicate canonical path keys
```

Use this oracle after every stress scenario.

---

## 29.24 Testing anti-patterns

* Exact event count assertions in cross-platform application tests.
* Fixed sleep followed by immediate assertion.
* No test deadline, causing hung CI.
* Only testing `fs::write`, never atomic replacement.
* No chained rename test.
* No directory removal test.
* No filter-boundary rename test.
* No initial reconciliation test.
* Trying to induce real kernel overflow as the only rescan test.
* Dropping events in a bounded queue test without verifying recovery.
* Testing watcher layer but never comparing final index to filesystem.
* Disabling flaky platform tests instead of changing them to semantic assertions.

---

## 29.25 Testing checklist

```text
[ ] Unit-test event classifier and path filters.
[ ] Real tempdir test for create/modify/remove/rename.
[ ] Test chained rename.
[ ] Test atomic save.
[ ] Test formatter-style rapid double write.
[ ] Test directory removal.
[ ] Test all rename filter-boundary cases.
[ ] Test initial inventory before incremental events.
[ ] Test synthetic rescan → reconciliation.
[ ] Test application queue overflow → reconciliation.
[ ] Test root lifecycle if supported.
[ ] Run native integration tests on Linux/macOS/Windows.
[ ] Run PollWatcher profile where relevant.
[ ] Use eventual conditions and generous deadlines.
[ ] Assert final inventory/hash equality.
[ ] Validate graph/index invariants after stress scenarios.
```

---

# notify-debouncer-full Advanced — 30) Shutdown and resource lifecycle

## 30.0 Lifecycle mental model

```text
construct Debouncer
  → background debouncer loop thread starts
  → register roots
  → receive/process events
  → stop event source
  → close/drain handoff
  → stop workers
  → destroy application state
```

The `Debouncer` is an ownership guard around the watcher, shared state, and background loop thread. [S5]

---

## 30.1 `stop(self)`

`Debouncer::stop(self)` consumes the debouncer, signals its stop flag, and joins the debouncer thread. The docs/source note that it may block for up to the configured tick rate because the loop can be sleeping. [S5] [S3]

```rust
if let Some(debouncer) = service.debouncer.take() {
    debouncer.stop();
}
```

This is the preferred deterministic shutdown primitive when you can tolerate that bounded wait.

---

## 30.2 `stop_nonblocking(self)`

`stop_nonblocking(self)` signals the loop to stop without joining it. [S5]

Use when:

```text
caller must not synchronously wait
process is already tearing down
thread join is managed indirectly
```

Tradeoff:

```text
returning from stop_nonblocking
  != proof background loop has exited
```

Do not immediately destroy dependent state that the handler may still touch unless ownership prevents that race.

---

## 30.3 `Drop`

Dropping `Debouncer` signals the stop flag, but source inspection shows the `Drop` implementation does not perform the same explicit join as `stop()`. [S3]

Therefore:

```text
Drop = safety/lifecycle signal
stop() = deterministic joined shutdown
```

Use explicit `stop()` in services/tests where deterministic termination matters.

---

## 30.4 Shutdown ordering with channels

Bad:

```text
1. drop receiver
2. workers stop
3. debouncer continues delivering briefly
4. sender adapter silently ignores disconnect
```

Better:

```text
1. mark service stopping / reject new workspace operations
2. stop debouncer (no new delivery after joined stop)
3. close handoff sender/channel ownership
4. drain or cancel queued file work
5. await workers
6. flush index/database
7. release workspace state
```

If your handler captures sender clones, ensure all clones are dropped or closed according to the channel API.

---

## 30.5 Drain versus cancel policy

### Drain

```text
stop watcher
process all already-dirty paths
commit final state
exit
```

Use for:

* daemon graceful shutdown;
* durable index that should be current at exit;
* controlled desktop app close.

### Cancel

```text
stop watcher
cancel outstanding analysis
rollback/discard partial deltas
exit
```

Use for:

* ephemeral cache;
* fast process shutdown;
* index can reconcile on next startup.

Make the policy explicit.

---

## 30.6 Shutdown with reconciliation in progress

Options:

```text
finish reconcile:
  most consistent persisted state
  slower shutdown

cancel reconcile:
  mark persisted state generation incomplete
  force reconcile next startup
```

Never persist “healthy/current” metadata if a required reconciliation was canceled midway.

---

## 30.7 Test teardown

Integration tests should explicitly stop the debouncer before deleting the temp directory where possible:

```text
perform assertions
→ debouncer.stop()
→ drop receiver/workers
→ tempdir cleanup
```

This reduces late events and cross-test interference.

---

## 30.8 Tick-rate implication

If deterministic shutdown latency matters, remember `stop()` may wait up to approximately `tick_rate` for the sleeping loop. [S5]

A very large tick rate therefore affects both delivery jitter and joined shutdown responsiveness.

Do not configure `tick_rate = 10s` casually in a service expected to stop quickly.

---

## 30.9 Owner struct pattern

```rust
pub struct WatchService<D> {
    debouncer: Option<D>,
    state: WatchHealth,
}

impl<D> WatchService<D>
where
    D: StoppableDebouncer,
{
    pub fn shutdown(&mut self) {
        self.state = WatchHealth::Stopped;
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop_owned();
        }
    }
}
```

A small app-specific trait can make concrete generic debouncer types easier to test/mock.

---

## 30.10 Shutdown anti-patterns

* Relying exclusively on process exit to kill watcher thread in tests.
* Dropping receiver before stopping source and assuming sender error shuts watcher down.
* Using `stop_nonblocking` then destroying captured handler state immediately.
* Cancelling graph transaction midway without rollback/generation marker.
* Persisting index as healthy after canceled reconciliation.
* Huge tick rate causing surprising shutdown wait.
* Keeping hidden sender clones so async worker never sees channel closure.

---

## 30.11 Shutdown checklist

```text
[ ] Service owns Debouncer explicitly.
[ ] Prefer stop() for deterministic shutdown.
[ ] Understand stop_nonblocking does not join.
[ ] Stop event source before destroying consumers.
[ ] Choose drain vs cancel policy.
[ ] Roll back/discard incomplete graph deltas.
[ ] Mark interrupted reconciliation for next startup.
[ ] Await workers if graceful consistency matters.
[ ] Integration tests stop watcher before tempdir cleanup.
[ ] Tick rate meets shutdown-latency needs.
```

---

# notify-debouncer-full Advanced — 31) Security and resource governance

## 31.0 Threat / governance model

A watcher reads path metadata and can trigger downstream file reads. If users can choose roots, this is an access-control and resource-consumption surface.

```text
untrusted root request
  → path authorization
  → symlink policy
  → root size/resource budget
  → watcher registration
  → event paths
  → file read authorization
  → parsing/resource budget
```

Do not treat “the OS watcher delivered this path” as authorization to read/process it.

---

## 31.1 Root authorization

For multi-tenant or sandboxed services:

```text
requested root
  → normalize according to application policy
  → confirm it belongs to allowed workspace/storage prefix
  → reject parent/root/system paths
  → evaluate symlink escape policy
```

Examples to reject unless explicitly intended:

```text
/
/home
C:\
user profile root
shared secrets/config tree
system package trees
```

---

## 31.2 Symlink escape

If a workspace contains:

```text
repo/external -> /sensitive/path
```

then backend/cache symlink-follow behavior can extend observation or file-ID scanning outside the apparent repository root. `notify::Config` follows symlinks by default for applicable backends, and `FileIdMap` tree walking follows links. [S13] [S12]

Security choices:

```text
disallow symlink traversal
allow but resolve+authorize every read target
allow only links staying within workspace
separate trusted desktop-local policy from multi-tenant service policy
```

Do not rely only on lexical prefix checks if resolved targets matter.

---

## 31.3 Watch exhaustion as resource attack

Linux recursive watcher registration consumes inotify resources and can hit per-user limits. [S8]

If untrusted users can open arbitrary workspaces:

```text
cap roots per user/tenant
cap total watched directories/files
reject pathological huge roots
isolate OS users/processes where appropriate
monitor registration failures
```

Raising system limits is not a substitute for tenancy/resource policy.

---

## 31.4 Poll amplification

PollWatcher over a huge directory causes repeated tree metadata work; `compare_contents=true` can additionally read/hash content. [S13] [S14]

Potential abuse:

```text
user points poll watcher at multi-million-file tree
poll every 100 ms
compare contents enabled
→ sustained filesystem/CPU load
```

Govern:

```text
minimum poll interval
maximum root size
content-compare allowlist
per-tenant CPU/I/O budgets
```

---

## 31.5 File-ID cache traversal cost

On platforms using `FileIdMap`, adding a recursive root walks it and follows symlinks while retrieving file IDs. [S12]

Treat initial cache population as a resource-sensitive operation. Apply root authorization and scale limits **before** registering the watch.

---

## 31.6 Path disclosure

Watcher logs often include absolute paths:

```text
/home/alice/company-secret/repo/src/foo.rs
```

Central logging can inadvertently leak:

* usernames;
* project/customer names;
* repository topology;
* mounted share names.

Prefer structured workspace IDs and relative paths where possible. Redact external telemetry.

---

## 31.7 Source-content safety

A watcher often leads to reading source files. Keep file-read limits:

```text
max individual file size
allowed file types
binary detection
parse timeout/resource budget
archive/generated-file policy
```

A malicious repository can create enormous source-like files or rapidly rewrite files to cause repeated analysis.

---

## 31.8 Event storm abuse

A process inside the workspace can continuously touch/rename files.

Mitigations:

```text
dirty-path coalescing
per-workspace analysis quota
burst → reconcile mode
parser concurrency cap
max index freshness work budget
workspace suspension/degraded mode
```

Debounce timeout alone is not a denial-of-service control.

---

## 31.9 TOCTOU and path replacement

Security-sensitive readers should understand:

```text
validate path
→ path changes/symlink swapped
→ open reads different object
```

If strong sandbox guarantees are needed, use OS-level sandboxing/open-handle techniques appropriate to the platform rather than relying on watcher path validation alone.

`notify-debouncer-full` is not a filesystem sandbox.

---

## 31.10 Privilege separation

Prefer watcher/indexer running with the minimum filesystem permissions required for its workspace.

Avoid:

```text
root/admin watcher process
+ arbitrary user-supplied roots
```

when a less-privileged service/process can achieve the task.

---

## 31.11 Security configuration profile

```text
Multi-tenant remote code indexer:
  roots pre-provisioned by workspace manager
  no arbitrary absolute roots from client
  symlink policy restrictive
  max workspace files/bytes
  max source file size
  bounded parser CPU/concurrency
  per-tenant dirty backlog quota
  absolute paths not exported to telemetry
  reconciliation isolated by workspace

Local desktop IDE:
  broader user-selected roots acceptable
  still avoid system-root accidents
  symlink behavior documented
  watch-limit diagnostics user-visible
```

---

## 31.12 Security anti-patterns

* Accepting arbitrary watch roots from remote clients.
* Running watcher as root/admin unnecessarily.
* Assuming event path is authorized because backend emitted it.
* Following symlinks without a scope policy.
* Polling huge user trees at aggressive intervals with content hashing.
* Raising inotify limits globally without tenant quotas.
* Logging absolute private paths to centralized telemetry.
* Parsing arbitrarily large files on every event.
* Treating debounce as a DoS defense.
* Using watcher path checks as a filesystem sandbox.

---

## 31.13 Security checklist

```text
[ ] Authorize roots before registration.
[ ] Restrict/validate symlink traversal.
[ ] Run with least filesystem privilege.
[ ] Cap watched roots/tree scale per tenant.
[ ] Cap poll frequency and compare-contents use.
[ ] Cap source file size/parser resources.
[ ] Coalesce event storms and enforce work quotas.
[ ] Protect absolute path privacy in telemetry.
[ ] Re-authorize/read safely after filesystem races.
[ ] Use OS sandboxing when strong isolation is required.
```

---

# notify-debouncer-full Advanced — 32) Stable application wrapper and API design

## 32.0 Why wrap the crate

Directly exposing `notify::Event` across your entire application couples domain logic to:

```text
backend-specific event variants
crate version changes
path vector conventions
rescan attributes
rename representation
watcher generic types
```

Instead, isolate it:

```text
notify-debouncer-full
  → WatchAdapter
  → application invalidation API
  → rest of system
```

---

## 32.1 Recommended public abstraction

For a current-state code index, expose **invalidation**, not OS event verbs:

```rust
#[derive(Debug, Clone)]
pub enum WatchInvalidation {
    PathsDirty(Vec<PathBuf>),
    RenameHint { from: PathBuf, to: PathBuf },
    ReconcileRequired,
    RootUnavailable { root: PathBuf },
}
```

Downstream state resolver converts this into authoritative `FileDelta` values.

This prevents every parser/index module from learning `EventKind`/`RenameMode` details.

---

## 32.2 Wrapper responsibilities

```text
WatchAdapter owns:
  Debouncer concrete generic type
  watch root registry
  backend configuration
  event classification
  basic path/root routing
  rescan signaling
  handler → application queue bridge
  metrics
  lifecycle/shutdown

WatchAdapter does NOT own:
  source parsing
  semantic graph logic
  business ignore semantics unless explicitly centralized
  durable index transaction policy
```

---

## 32.3 Stable config type

```rust
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub debounce_timeout: Duration,
    pub tick_rate: Option<Duration>,
    pub backend: WatchBackend,
    pub recursive: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum WatchBackend {
    Recommended,
    Poll {
        interval: Duration,
        compare_contents: bool,
    },
}
```

Do not expose `notify::Config` as your public application config unless you intentionally want callers coupled to `notify`.

---

## 32.4 Stable service interface

```rust
pub trait WorkspaceWatcher {
    fn watch_root(&mut self, root: &Path) -> Result<(), WatchError>;
    fn unwatch_root(&mut self, root: &Path) -> Result<(), WatchError>;
    fn backend_kind(&self) -> WatchBackendKind;
    fn shutdown(self: Box<Self>);
}
```

Concrete implementations:

```text
RecommendedWorkspaceWatcher
PollWorkspaceWatcher
FakeWorkspaceWatcher for tests
```

This hides `Debouncer<W,C>` generic details from most code.

---

## 32.5 Error abstraction

```rust
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("invalid watch configuration: {0}")]
    Config(String),

    #[error("failed to register watch root {root}: {source}")]
    Register {
        root: PathBuf,
        #[source]
        source: notify_debouncer_full::notify::Error,
    },

    #[error("watch backend failed: {0}")]
    Backend(String),
}
```

Keep original source error internally where possible, while exposing stable application categories.

---

## 32.6 Event classifier module

```rust
fn classify_event(event: &DebouncedEvent) -> ClassifiedWatchEvent {
    if event.need_rescan() {
        return ClassifiedWatchEvent::Reconcile;
    }

    if let Some((from, to)) = rename_paths(&event.event) {
        return ClassifiedWatchEvent::Rename {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
    }

    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            ClassifiedWatchEvent::Dirty(event.paths.clone())
        }
        EventKind::Remove(_) => {
            ClassifiedWatchEvent::Removed(event.paths.clone())
        }
        _ => ClassifiedWatchEvent::Ignore,
    }
}
```

Keep this module small and exhaustively tested.

---

## 32.7 Do not over-normalize in wrapper

The wrapper should not invent certainty it does not have.

Bad:

```text
Modify event → DomainFileChanged(old_hash=?, new_hash=?)
```

The watcher layer does not know hashes.

Better:

```text
Modify event → path dirty
```

State resolver then reads/hashes to determine actual domain change.

---

## 32.8 Version isolation

If a future crate version changes:

```text
handler APIs
watch methods
feature flags
cache types
event serialization
```

only the wrapper and its integration tests should need broad changes.

This is especially valuable for LLM-generated code: give agents the stable internal watcher API by default, and reserve direct crate usage for the adapter module.

---

## 32.9 Wrapper test fake

```rust
#[derive(Default)]
struct FakeWatcher {
    roots: Vec<PathBuf>,
    emitted: Vec<WatchInvalidation>,
}

impl FakeWatcher {
    fn emit(&mut self, event: WatchInvalidation) {
        self.emitted.push(event);
    }
}
```

Most parser/index tests should use domain invalidations or `FileDelta`s rather than a real OS watcher.

---

## 32.10 Wrapper anti-patterns

* Passing `DebouncedEvent` into every application subsystem.
* Exposing `Debouncer<RecommendedWatcher, RecommendedCache>` in broad public APIs.
* Public config type is a thin alias of `notify::Config` without need.
* Wrapper claims file content changed without reading it.
* Wrapper performs parser/database work.
* No fake/mock watcher path for deterministic domain tests.
* Upgrade requires touching dozens of modules because `EventKind` leaked everywhere.

---

## 32.11 Wrapper checklist

```text
[ ] One adapter module owns direct crate API.
[ ] Downstream receives invalidation semantics.
[ ] Authoritative FileDelta created after filesystem state resolution.
[ ] Public config hides unnecessary notify details.
[ ] Public error categories are stable.
[ ] Concrete Debouncer generics stay private.
[ ] Event classifier is small and unit-tested.
[ ] Fake watcher exists for domain tests.
[ ] Upgrade surface is localized.
```

---

# notify-debouncer-full Advanced — 33) Troubleshooting cookbook

## 33.0 Diagnostic order

When “watching does not work,” debug from the source outward:

```text
1. Is the intended root correct and present?
2. Did watch() succeed?
3. Which WatcherKind is active?
4. Does the backend work on this filesystem/mount?
5. Are raw-ish changes reaching debouncer handler?
6. Is filtering dropping them?
7. Is application queue stalled/full?
8. Is parser/index work failing/stale?
9. Is final state reconciliation healthy?
```

Do not start by randomly changing debounce timeout.

---

## 33.1 Symptom: no events at all

Checklist:

```text
[ ] watch() returned Ok?
[ ] correct root path?
[ ] process has permission?
[ ] service still owns Debouncer?
[ ] handler receiver still alive?
[ ] backend kind expected?
[ ] filesystem is local/native?
[ ] WSL/network/container mount known to propagate native events?
```

`notify` documents network filesystems, WSL Windows paths, and some container/macOS volume arrangements as environments where native notifications can be absent; test PollWatcher. [S8]

Minimal diagnostic for the default concrete debouncer type:

```rust
use notify_debouncer_full::{Debouncer, RecommendedCache};
use notify_debouncer_full::notify::RecommendedWatcher;

println!(
    "watcher kind: {:?}",
    Debouncer::<RecommendedWatcher, RecommendedCache>::kind(),
);
```

Then create a brand-new file inside the exact root and observe the handler before applying application filters.

---

## 33.2 Symptom: events arrive only after a long delay

Possible causes:

```text
large debounce timeout
large tick_rate
PollWatcher with long poll_interval
blocked handler
downstream queue backlog
parser/index backlog
```

Latency decomposition:

```text
native backend:
  OS latency + timeout + tick jitter + handler + downstream

poll backend:
  poll wait + timeout + tick jitter + handler + downstream
```

Log all configured clocks and queue age.

---

## 33.3 Symptom: too many events / repeated parsing

Check:

```text
Are you measuring raw notify events or DebouncedEvent output?
Are exact EventKind variants preventing coalescing at application layer?
Is application queueing every event rather than unique dirty paths?
Are build/generated trees included?
Is formatter-on-save causing a real second content version?
Is content hash unchanged?
```

Fixes:

```text
use full debouncer correctly
filter high-churn paths
use dirty set
hash before parse
add stale-generation suppression
benchmark timeout, don't blindly increase it
```

---

## 33.4 Symptom: modify event missing after create

This may be expected. The full debouncer intentionally suppresses selected modify events after a create because the create already implies the path's current state must be read. [S1] [S3]

Application should treat Create as sufficient invalidation:

```text
Create(path) → read current bytes → index
```

Do not wait for a later Modify.

---

## 33.5 Symptom: duplicate create missing

Also expected by design: duplicate creates are suppressed in the full debouncer. [S1]

Do not write application correctness that requires a create count.

---

## 33.6 Symptom: rename not delivered as `Both`

Possible causes:

```text
backend emitted incomplete/unmatchable halves
tracker/cookie absent or mismatched
file-ID cache could not match identity
move crosses filesystem boundary
one side outside watched scope
path disappeared/replaced before identity lookup
```

Recovery:

```text
handle unmatched create/remove/rename-like invalidations conservatively
resolve current filesystem state
reconcile if identity uncertainty matters
```

The full debouncer improves rename correlation; it cannot guarantee semantic rename identity for every filesystem operation.

---

## 33.7 Symptom: rename source correct, target strange

Inspect:

```text
chained rename within debounce window?
atomic editor replacement?
rename over existing destination?
filter/root path transformation bug?
path normalization/string-loss bug?
```

The debouncer deliberately updates queued paths through renames and can merge a rename chain to the final target. [S3]

For debugging, log both normalized `event.paths` and current filesystem inventory.

---

## 33.8 Symptom: indexed children remain after directory delete

Likely application bug. The full debouncer can emit one directory remove rather than every child remove on inotify. [S1] [S3]

Fix:

```text
Remove(directory)
  → delete/reconcile all indexed descendants by path prefix/ownership
```

Do not depend on child events.

---

## 33.9 Symptom: `ENOSPC` when registering Linux watch

Likely inotify watch limit, not disk storage. `notify` documents `fs.inotify.max_user_watches`. [S8]

Diagnose:

```bash
cat /proc/sys/fs/inotify/max_user_watches
```

Then:

```text
reduce root scope
exclude unnecessary recursive trees structurally
raise operational limit if appropriate
or use polling where native watch counts are unsuitable
```

---

## 33.10 Symptom: works locally, fails on NFS/shared mount

This is a classic backend/filesystem mismatch. `notify` documents that some network filesystems do not emit events through native mechanisms. [S8]

Test explicit PollWatcher with a reasonable interval.

If polling works:

```text
make backend choice part of deployment configuration
surface polling latency/cost to operators
```

---

## 33.11 Symptom: WSL `/mnt/c` changes not observed

`notify` specifically documents WSL Windows paths as a case where events may not be emitted through the Linux native mechanism. [S8]

Use PollWatcher for that mount or place workspace on the Linux filesystem if native low-latency events are required.

---

## 33.12 Symptom: container on macOS host misses changes

Host-volume virtualization can break assumptions of the container's native Linux watcher. `notify` documents Docker Linux on macOS M1 as an example where native watching can fail and polling is a workaround. [S8]

Test the actual runtime/mount version; do not infer from container OS alone.

---

## 33.13 Symptom: macOS events missing for certain files

Review ownership/security behavior. `notify` documents an FSEvents caveat for unowned files and suggests polling as a possible workaround. [S8]

Also verify:

```text
process permissions
sandbox entitlements if applicable
root scope
backend feature selection
```

---

## 33.14 Symptom: PollWatcher uses huge CPU/I/O

Check:

```text
poll interval too short?
root too broad?
compare_contents=true?
large remote tree?
symlink expansion?
```

Fix in that order before micro-optimizing handler code.

---

## 33.15 Symptom: content changes not detected by PollWatcher

If metadata/mtime does not change reliably, enable `compare_contents`. [S13] [S14]

Then measure cost. If hashing the entire tree is unacceptable, consider an environment-specific alternative rather than assuming polling can be both metadata-independent and free.

---

## 33.16 Symptom: handler appears to stop receiving after consumer shutdown

If using built-in sender adapter, receiver disconnect is ignored by the handler implementation. [S3]

You may have:

```text
debouncer still alive
receiver dropped
all sends silently fail
```

Fix lifecycle ordering or use a custom handler that signals supervisor state on disconnect.

---

## 33.17 Symptom: shutdown blocks

`stop()` joins the loop thread and may wait up to the tick interval. [S5]

Check configured `tick_rate`.

If blocking is unacceptable:

```text
reduce tick rate
or use stop_nonblocking with safe state ownership
```

Do not mistake the bounded sleep for a deadlock without inspecting thread state.

---

## 33.18 Symptom: shutdown returns but callback still touches state

Likely using `stop_nonblocking` or relying on Drop without deterministic join.

Use `stop()` before destroying handler-dependent state.

---

## 33.19 Symptom: index occasionally reverts to old source

This is usually not a watcher ordering bug. It is often asynchronous stale-work commit:

```text
change A → parse A slow
change B → parse B fast → commit B
parse A completes later → commits A
```

Fix per-path generation fencing.

---

## 33.20 Symptom: branch checkout leaves graph inconsistent

Check:

```text
backend overflow/rescan signal ignored?
application queue overflowed?
dirty FIFO processed intermediate versions too slowly?
directory remove not cascading?
manifest/config invalidation incomplete?
```

Add overload-to-reconcile policy and final-state oracle after branch-switch stress tests.

---

## 33.21 Symptom: ignored files still consume resources

Application filtering happens **after** the backend observes events. If a recursive root includes `target/`, inotify/resource/event cost can remain even though parser work is filtered.

Reduce watch roots or redesign repository root registration if this is material.

---

## 33.22 Symptom: file ID cache startup is slow

On platforms where `RecommendedCache` is `FileIdMap`, recursive watch registration can walk the tree and retrieve file IDs; the implementation follows symlinks. [S7] [S12]

Reduce root scope, inspect linked trees, or choose cache/backend strategy deliberately through `new_debouncer_opt` if rename correlation tradeoffs justify it.

---

## 33.23 Symptom: unexpected paths outside repository

Investigate symlinks first. Both watcher follow-symlink config and FileIdMap traversal can matter. [S13] [S12]

Do not “fix” by string-prefix filtering only if the service then reads resolved outside targets.

---

## 33.24 Symptom: `tick_rate > timeout` construction error

This is deliberate validation in `new_debouncer_opt`. [S3]

Choose:

```text
0 < tick_rate <= timeout
```

A common default is `timeout / 4` when tick is omitted. [S4] [S3]

---

## 33.25 Symptom: setting timeout seems not to wait for complete quiet

Expected. In 0.7.0, each queued event carries its own timestamp and becomes eligible after aging past timeout; the loop does not implement one global trailing-edge quiet timer that resets indefinitely for every subsequent event. [S3]

If you need “analyze only after path has been quiet for X ms,” add a higher-level per-path quiet timer/generation scheduler.

---

## 33.26 Troubleshooting capture bundle

When reporting a bug, collect:

```text
OS + version
filesystem/mount type
container/WSL/VM context
notify-debouncer-full version
notify version
Cargo feature flags
WatcherKind
timeout
tick_rate
notify Config fields used
watch roots + RecursiveMode (sanitized)
cache type
minimal mutation sequence
normalized DebouncedEvent debug output
whether need_rescan appeared
whether PollWatcher reproduces
```

This distinguishes backend issues from debouncer and application issues quickly.

---

## 33.27 Troubleshooting anti-patterns

* Change timeout first for every watcher problem.
* Diagnose by OS but ignore filesystem/mount topology.
* No log of `WatcherKind`.
* Assume `ENOSPC` means disk full.
* Assume one missing rename means full debouncer is broken.
* Ignore `need_rescan` then debug stale state as random graph corruption.
* Blame watcher for stale async parse commits.
* Use exact event counts as the only reproduction criterion.
* Report bug without crate versions/features/backend.

---

## 33.28 Troubleshooting checklist

```text
[ ] Confirm root, registration, permissions, lifetime.
[ ] Log watcher kind and full timing config.
[ ] Identify filesystem/mount/container topology.
[ ] Observe normalized events before application filtering.
[ ] Check need_rescan and backend errors.
[ ] Inspect downstream queue/backlog.
[ ] Inspect stale-generation protection.
[ ] Test PollWatcher if native delivery is suspect.
[ ] Reconcile filesystem vs index to classify correctness drift.
[ ] Capture versions/features for reproducible issue report.
```

---
# notify-debouncer-full Advanced — 34) Upgrades, prereleases, and compatibility

## 34.0 Version policy

This document is pinned to:

```text
notify-debouncer-full 0.7.0
notify              8.2.x-compatible dependency surface
MSRV policy/current 0.7 docs: 1.85 anchor
```

The repository has a `0.8.0-rc.2` prerelease, but prerelease code is a separate migration target rather than a transparent patch upgrade. [S2] [S20]

Agent rule:

```text
Never generate watcher code from “latest docs” without first resolving:
  stable vs prerelease
  notify-debouncer-full version
  notify version
  enabled features
  Rust toolchain/MSRV
```

---

## 34.1 Why pin this crate

Filesystem watcher behavior is unusually sensitive to small changes because it crosses:

```text
public Rust API
backend implementation
OS behavior
rename heuristics
queue/coalescing logic
path representation
cache traversal
feature selection
```

Pinning lets you regression-test the semantic event normalization your application expects.

Recommended production manifest:

```toml
[dependencies]
notify-debouncer-full = "=0.7.0"
```

Then update intentionally through a PR with platform integration tests.

---

## 34.2 Stable 0.7.0 anchor

Published 0.7.0 is the current stable release at this document's anchor date. Its crate metadata depends on `notify ^8.2.0`; docs.rs lists the stable crate and also shows the 0.8 RC versions separately. [S2]

Key 0.7.0 behaviors covered in this reference include:

```text
matched rename halves → single RenameMode::Both
rename-chain merging
queued-path rewriting through rename
optional file-ID matching
single directory remove behavior on inotify
no duplicate create
selected modify-after-create suppression
direct Debouncer watch/unwatch/configure methods
stop / stop_nonblocking lifecycle
per-event timeout aging + periodic tick flush
```

These behaviors should be captured by application regression tests before upgrading. [S1] [S3]

---

## 34.3 Current 0.8 prerelease differences worth noticing

The official `notify-rs/notify` release notes for `notify-debouncer-full 0.8.0-rc.2` describe several breaking/material changes: [S20]

```text
upgrade underlying notify to 9.0.0 RC line
emit remove events even after a file was created then removed
optimize root tracking for large numbers of watched paths
```

The prior `0.8.0-rc.1` notes also call out:

```text
higher MSRV (1.88)
notify 9 RC dependency
must_use annotations
faster event flushing/file-ID cache lookups
stable path ordering work for equal-timestamp events
```

The key lesson is architectural: event-suppression behavior and underlying path/backend semantics can change across the major/prerelease boundary. [S20]

---

## 34.4 Create→remove behavior migration

Under the 0.7 mental model, a path created and removed inside the debounce window can be collapsed/suppressed in some queue paths. The 0.8 RC release notes explicitly change behavior to emit removes even after create, motivated by macOS repeated-create behavior. [S20]

If your application currently assumes:

```text
Create then Remove inside window
  → maybe no durable application effect
```

write a regression test before migrating. For a current-state index, authoritative state resolution should already make either sequence safe:

```text
Remove(path)
  → check current existence
  → absent → ensure not indexed
```

This is another reason to avoid using exact event counts as the domain contract.

---

## 34.5 `notify` 8 → 9 coupling

The 0.8 RC line upgrades to `notify` 9 release candidates. [S20]

That can affect:

```text
Watcher trait/API behavior
path representation
backend fixes
feature flags
MSRV
event serialization compatibility
watched path semantics
```

Do not upgrade the debouncer in isolation if your code also depends directly on `notify` 8 types/features elsewhere.

---

## 34.6 Path representation regression risk

The notify 9 RC release line includes changes around preserving watched path representation consistently across backends. [S20]

If your application currently depends on an incidental 8.x absolute/relative path transformation, migration can expose hidden assumptions.

Before upgrade, test:

```text
watch("relative/root")
  → what form are Event.paths?

watch("/absolute/root")
  → what form are Event.paths?

application workspace-relative mapping
  → remains correct?
```

Best practice remains: centralize path mapping in your watcher adapter rather than scattering assumptions.

---

## 34.7 Deprecated API cleanup before upgrade

0.7.0 already deprecates using `.watcher()` and `.cache()` for ordinary root management; direct `Debouncer` methods are preferred. [S5]

Clean up now:

```rust
// preferred 0.7 pattern
debouncer.watch(root, RecursiveMode::Recursive)?;
debouncer.unwatch(root)?;

// `kind` is associated with the concrete Debouncer type, not the instance.
let kind = Debouncer::<RecommendedWatcher, RecommendedCache>::kind();
```

This reduces migration complexity.

---

## 34.8 Feature compatibility

Record explicit features in the manifest rather than relying on transitive accidents.

Relevant ecosystem features include:

```text
crossbeam-channel
flume
serde / serialization compatibility
macOS backend feature choices
```

The exact feature graph is version-specific. Check the pinned crate page on every upgrade. [S2] [S8]

---

## 34.9 Serialization is not a timeless wire contract

If you serialize `notify::Event` values for IPC/storage:

```text
crate enum variants
attribute representation
path semantics
compatibility features
```

can become part of your persistence protocol.

Prefer serializing an application-owned stable invalidation/FileDelta schema instead.

If raw event persistence is necessary:

```text
include schema version
include crate version
include platform/backend metadata
keep compatibility tests
```

---

## 34.10 MSRV policy

The 0.7.0 crate page documents its current MSRV policy and an anchor of Rust 1.85; the 0.8 RC release notes raise the RC line to Rust 1.88. [S2] [S20]

Upgrade checklist must include CI on the project's actual minimum toolchain rather than only stable-latest Rust.

---

## 34.11 Cargo audit before upgrade

```bash
cargo tree -i notify-debouncer-full
cargo tree -i notify
cargo tree -i notify-types
cargo tree -i file-id
```

Look for:

```text
multiple notify major versions
direct dependencies that pin old types
feature unification changes
forked/renamed watcher crates
```

Type/API mismatches can occur if application modules import direct `notify` types from a different major version than the debouncer re-export.

---

## 34.12 Upgrade test matrix

```text
Compilation:
  [ ] stable wrapper compiles
  [ ] feature matrix compiles
  [ ] minimum supported Rust compiles

Behavior:
  [ ] create
  [ ] modify
  [ ] create→modify suppression expectation
  [ ] create→remove
  [ ] same-filesystem rename
  [ ] chained rename
  [ ] rename over target
  [ ] directory remove
  [ ] atomic save
  [ ] need_rescan handling

Platforms:
  [ ] Linux native
  [ ] macOS native
  [ ] Windows native
  [ ] PollWatcher

Application:
  [ ] initial reconciliation
  [ ] dirty-set coalescing
  [ ] stale generation fencing
  [ ] final filesystem/index oracle
  [ ] graceful shutdown
```

---

## 34.13 Upgrade workflow

```text
1. Read official crate release notes/changelog.
2. Pin candidate version in a branch.
3. Resolve notify major-version coupling.
4. Compile wrapper/feature matrix.
5. Run pure event-classifier tests.
6. Run native platform integration tests.
7. Run editor/atomic-save tests.
8. Run branch-switch/stress tests.
9. Compare event/latency metrics, but judge final-state correctness first.
10. Update internal wrapper only; avoid broad domain churn.
11. Roll out with backend/rescan/error telemetry watched closely.
```

---

## 34.14 Upgrade anti-patterns

* `notify-debouncer-full = "*"` in production.
* Treating 0.8 RC as if it were stable 0.7 documentation.
* Upgrading debouncer without checking underlying `notify` major.
* Depending on deprecated inner watcher/cache access.
* Persisting raw events with no schema/version tag.
* No cross-platform regression suite.
* Only comparing compile success, not semantic event behavior.
* Failing to test relative/absolute path behavior through a notify major upgrade.
* Assuming MSRV cannot rise outside a major release.

---

## 34.15 Upgrade checklist

```text
[ ] Pin current and target versions explicitly.
[ ] Read official release notes/changelog.
[ ] Record underlying notify version change.
[ ] Record MSRV change.
[ ] Remove deprecated APIs first.
[ ] Inspect Cargo feature graph.
[ ] Avoid mixed notify type universes.
[ ] Test create→remove and rename semantics.
[ ] Test path representation.
[ ] Test native backends + PollWatcher.
[ ] Run filesystem/index convergence oracle.
[ ] Version any persisted application event schema.
```

---

# notify-debouncer-full Advanced — 35) Comparison with adjacent crates and abstractions

## 35.0 Selection map

```text
Need raw cross-platform FS events?
  → notify

Need low-noise path-level “something changed” events?
  → notify-debouncer-mini

Need richer normalized rename/in-order-ish event semantics?
  → notify-debouncer-full

Need watch + filter + command/process restart orchestration?
  → watchexec ecosystem

Need Linux-only kernel-specific control?
  → inotify crate / direct inotify APIs

Native notifications unreliable?
  → notify::PollWatcher
```

The right abstraction is determined by downstream semantics, not by choosing the crate with the most features.

---

## 35.1 Bare `notify`

### What it gives

```text
cross-platform Watcher trait
RecommendedWatcher backend selection
raw-ish Event / Error callbacks
RecursiveMode
Config
PollWatcher
backend-specific watcher types
```

### What you provide

```text
debouncing
rename-chain normalization
per-path queueing
duplicate suppression
application work coalescing
```

### Use bare `notify` when

```text
you need minimum latency
raw event distinctions matter
you have your own state machine/debouncer
simple event count is low
embedded component wants no extra debouncer thread/logic
```

### Prefer full debouncer when

```text
editor save noise creates repeated expensive work
rename semantics matter
queued paths must follow rename chains
cross-platform normalization is worth added latency/state
```

---

## 35.2 `notify-debouncer-mini`

The mini debouncer intentionally provides a simpler abstraction around changed paths/events. It is attractive when the application does not need rich rename reconstruction and mainly needs “this path became dirty.”

Conceptual distinction:

```text
mini:
  path changed → debounced path-level signal

full:
  preserve more event semantics
  correlate renames
  update queued paths
  maintain optional file IDs
  sort/coalesce richer event stream
```

For a current-state code index that ignores rename identity and always hashes paths, mini can be sufficient and cheaper conceptually.

For a graph that wants to preserve file record identity/path moves and avoid redundant intermediate rename work, full is better suited.

---

## 35.3 `watchexec`

Watchexec sits at a higher orchestration level:

```text
watch filesystem
filter events
manage commands/processes
restart/cancel child processes
```

Use when the product requirement is essentially:

```text
“run/restart X when Y changes”
```

rather than:

```text
“feed normalized filesystem invalidations into my own stateful application model.”
```

For custom code-property graphs, `notify-debouncer-full` is usually the more direct library layer; for a CLI replacement for `cargo watch`, watchexec-like orchestration may reduce custom code.

---

## 35.4 Direct `inotify` crate

Linux-only direct inotify access gives:

```text
watch descriptors
kernel masks
rename cookies
queue-specific behavior
async FD integration options
```

Use when:

```text
Linux-only is acceptable
kernel semantics are part of product design
you need precise watch descriptor lifecycle
custom overflow/reconciliation policy requires raw inotify details
you want to avoid cross-platform abstraction
```

Costs:

```text
no Windows/macOS portability
more rename/directory/watch bookkeeping
more backend-specific testing
```

For a cross-platform code intelligence tool, `notify`/full debouncer usually saves substantial engineering.

---

## 35.5 `PollWatcher`

`PollWatcher` is not a competitor to `notify-debouncer-full`; it is a `notify` backend that can be used **inside** the full debouncer. [S14]

```text
PollWatcher = event source
notify-debouncer-full = normalization/debounce layer
```

Use both when native events are unreliable but you still want full normalization and batching.

---

## 35.6 File-system journal APIs

For requirements like:

```text
recover every change across process downtime
consume durable historical changes
resume from sequence number
```

ordinary watcher callbacks are not sufficient. Platform journals (where available), VCS history, or a durable domain event source may be more appropriate.

`notify-debouncer-full` is a live observer, not a persistent change journal.

---

## 35.7 Comparison table

| Option | Cross-platform | Debounce | Rename normalization | Process orchestration | Durable history | Best for |
|---|---:|---:|---:|---:|---:|---|
| `notify` | yes | no | backend/raw | no | no | custom low-level watcher logic |
| `notify-debouncer-mini` | yes | yes | limited/simple | no | no | dirty-path invalidation |
| `notify-debouncer-full` | yes | yes | strong relative to raw events | no | no | stateful apps/indexers |
| `watchexec` | yes | yes/higher-level | application-oriented | yes | no | rebuild/restart CLIs |
| direct inotify | Linux | no unless built | cookie-level raw | no | no | Linux-specific control |
| PollWatcher | yes | source-level polling | source semantics | no | no | difficult filesystems |
| durable journal/VCS | varies | n/a | history-dependent | no | yes-ish | historical/replay requirements |

---

## 35.8 Decision tree for code intelligence

```text
Do you only need to know which paths are dirty?
  yes → evaluate mini debouncer
  no  → continue

Do rename relationships improve incremental identity/performance?
  yes → full debouncer
  no  → mini or full depending event detail needs

Do you need every historical edit/version?
  yes → watcher alone is insufficient; add durable source

Are native events reliable on deployment filesystem?
  yes → RecommendedWatcher
  no  → PollWatcher inside chosen debouncer
```

---

## 35.9 Migration from raw `notify` to full debouncer

Before:

```rust
let mut watcher = notify::recommended_watcher(move |res| {
    // raw event handling
})?;
```

After:

```rust
let mut debouncer = notify_debouncer_full::new_debouncer(
    Duration::from_millis(75),
    None,
    move |res| {
        // batches of normalized DebouncedEvent
    },
)?;
```

Migration changes to test:

```text
intentional delivery delay
batching
modify-after-create suppression
duplicate-create suppression
rename shape/order
error batching
shutdown thread lifecycle
```

---

## 35.10 Migration from Python `watchfiles`

Conceptually:

```text
watchfiles
  → Python-facing watcher/debouncer built on notify ecosystem

Rust-native application
  → notify / notify-debouncer-full directly
```

Do not search for a one-for-one Python API clone. In Rust, choose the lower-level semantics your application actually needs and build a small stable adapter.

---

## 35.11 Comparison anti-patterns

* Choosing full debouncer when only a single low-rate config file is watched and raw notify is sufficient.
* Choosing mini then rebuilding rename correlation manually without reason.
* Choosing watchexec as a library dependency when you only need event normalization and no process orchestration.
* Choosing direct inotify in a product that must support Windows/macOS.
* Treating PollWatcher and full debouncer as mutually exclusive.
* Expecting any live watcher to provide durable replay after downtime.

---

## 35.12 Selection checklist

```text
[ ] Define whether domain needs path dirtiness or event semantics.
[ ] Define whether rename identity matters.
[ ] Define latency tolerance.
[ ] Define platform requirements.
[ ] Define process-restart/orchestration requirement.
[ ] Define durable-history requirement separately.
[ ] Validate native filesystem behavior.
[ ] Choose polling as backend when needed, not as a different domain API.
```

---

# notify-debouncer-full Advanced — 36) Production deployment patterns

## 36.0 Pattern map

```text
CLI / one-shot dev tool
long-lived daemon
IDE / code intelligence service
hot-reload dev server
multi-workspace desktop app
multi-tenant remote indexing service
container/shared-volume watcher
background sync/metadata service
```

Each should use the same correctness principles but different lifecycle/resource profiles.

---

## 36.1 CLI / local development tool

Example:

```text
watch one project
print/act on changes
Ctrl+C exits
```

Recommended profile:

```text
RecommendedWatcher
one recursive root
75–200 ms debounce starting range
simple mpsc/closure handoff
explicit Ctrl+C shutdown if application framework supports it
minimal metrics/logging
```

Still handle async errors and `need_rescan()` if maintaining state.

---

## 36.2 Long-lived daemon

Requirements:

```text
explicit service ownership
health endpoint
backend/error/rescan metrics
joined shutdown
bounded downstream work
reconciliation after event loss
persistent state generation
```

Architecture:

```text
Supervisor
  ├─ WatchService
  ├─ DirtyScheduler
  ├─ Reconciler
  ├─ WorkerPool
  └─ StateStore
```

Do not let a watcher callback own business processing.

---

## 36.3 IDE / code intelligence service

Recommended:

```text
RecommendedWatcher native where valid
full debouncer for rename/path normalization
dirty-path set
content hash
incremental parser
semantic dependency invalidation
generation fencing
atomic graph delta
fast interactive freshness metrics
```

Watch:

```text
source
manifests/config
build scripts affecting semantic environment
```

Exclude:

```text
build output
VCS object store
large generated caches
```

unless product semantics require them.

---

## 36.4 Hot-reload server

Architecture:

```text
watch
  → relevant change generation
  → one build/reload job
  → cancel/supersede stale generation
  → successful new artifact
  → graceful process replacement
```

Prefer higher-level process watcher tooling if this is the entire product requirement.

---

## 36.5 Desktop app with multiple open workspaces

Options:

```text
one Debouncer per workspace
  better isolation, more resources

one Debouncer many roots
  fewer watcher threads, shared error/burst domain
```

Recommendation depends on workspace count and scale. Use per-workspace dirty queues/health regardless.

On macOS/Windows, account for file-ID cache startup when opening large workspaces.

---

## 36.6 Multi-tenant remote indexing service

Use stronger isolation:

```text
workspace manager authorizes root
watch service runs least privilege
per-workspace or per-shard watcher ownership
per-tenant dirty/work quotas
bounded parser pool
absolute paths redacted from external telemetry
rescan/reconcile state durable
initial reconcile before serving READY graph
```

Do not let remote clients directly submit arbitrary local filesystem roots.

---

## 36.7 Container with source bind mount

Startup sequence:

```text
inspect configured backend policy
start watcher
perform initial reconciliation
run mutation self-test if environment is uncertain
if native events fail expected validation:
  switch to configured PollWatcher fallback
  reconcile again
```

Container OS is not enough to identify filesystem notification behavior; host/mount topology matters. [S8]

---

## 36.8 Network-mounted workspace

Recommended posture:

```text
assume nothing about native event propagation
validate
if unreliable → PollWatcher
configure explicit poll interval
compare_contents only if metadata is unreliable
measure I/O against share scale
```

Expose expected freshness SLA because polling changes latency.

---

## 36.9 Background sync service

If freshness can be seconds rather than milliseconds:

```text
PollWatcher may be acceptable even on local FS
longer debounce timeout
larger worker batches
lower polling frequency
periodic reconciliation
```

Optimize for resource efficiency over edit-loop latency.

---

## 36.10 Deployment config example

```toml
[watch]
backend = "recommended"
debounce_ms = 75
tick_ms = 20
recursive = true

[watch.poll]
interval_ms = 1000
compare_contents = false

[index]
max_parallel_parses = 8
max_file_bytes = 8388608
reconcile_dirty_threshold = 25000
```

Use application config, not public direct serialization of `notify::Config`, so your product remains stable across dependency upgrades.

---

## 36.11 Startup readiness sequence

```text
STARTING
  → validate root/config
  → construct watcher
  → register root
  → identify backend
  → initial filesystem inventory
  → build/verify initial index
  → merge any dirtiness observed during startup
  → READY/HEALTHY
```

Watcher registration alone does not make a stateful service ready.

---

## 36.12 Failure mode matrix

| Failure | CLI | Daemon/IDE | Multi-tenant service |
|---|---|---|---|
| root registration fails | print + exit | workspace unavailable | isolate tenant/workspace |
| backend runtime error | print | health degrade + reconcile | isolate + reconcile/alert |
| need_rescan | optional if stateless | required reconcile | required reconcile |
| queue overflow | warn | reconcile | reconcile + quota metric |
| parser file error | print | isolate file diagnostic | isolate file diagnostic |
| index transaction fails | fail command | retry/degrade | rollback/retry/isolate |
| native mount unsupported | suggest poll | fallback policy | configured backend class |

---

## 36.13 Rolling restart / persisted index

On startup with an existing persisted index:

```text
persisted index generation is not proof filesystem stayed unchanged while process was down
```

Perform reconciliation before declaring current:

```text
load persisted inventory
→ enumerate current filesystem
→ compare hashes/metadata
→ calculate delta
→ update index
→ start/merge watcher dirtiness
```

For very large repositories, optimize the reconciliation rather than skipping it.

---

## 36.14 Deployment anti-patterns

* Declaring READY immediately after `watch()`.
* Persistent index assumed current after process downtime without reconciliation.
* One unbounded queue shared by all tenants.
* Native watcher selected inside container solely from guest OS.
* PollWatcher left at default interval unintentionally for interactive product.
* Arbitrary user roots in privileged multi-tenant service.
* No health distinction between Healthy and Reconciling.
* No explicit graceful shutdown.

---

## 36.15 Deployment checklist

```text
[ ] Define deployment filesystem topology.
[ ] Define backend policy + polling fallback.
[ ] Validate/register roots.
[ ] Perform initial reconciliation.
[ ] Merge startup-time incremental dirtiness.
[ ] Expose watcher kind/config in health data.
[ ] Bound application queues and parser concurrency.
[ ] Implement need_rescan recovery.
[ ] Persist/restore index with startup reconciliation.
[ ] Isolate workspaces/tenants appropriately.
[ ] Define graceful shutdown drain/cancel policy.
```

---
# notify-debouncer-full Advanced — 37) Best-practice rules

## 37.0 Core engineering rules

### Rule 1 — Treat events as invalidations, not transactions

```text
Event says:
  “re-check this area of filesystem state.”

Event does not guarantee:
  “replay this verb and your model will be correct.”
```

Always resolve current state before domain mutation when correctness matters.

---

### Rule 2 — Use the full debouncer for semantic normalization, not just delay

The value is not merely “sleep 75 ms.” It is:

```text
per-path queues
rename correlation
rename-chain merging
queued-path rewriting
duplicate create suppression
modify-after-create suppression
directory-remove normalization
ordered batch emission
```

If none of these matter, a simpler watcher/debouncer may be sufficient. [S1] [S3]

---

### Rule 3 — Pin versions

```toml
notify-debouncer-full = "=0.7.0"
```

Then update intentionally with platform tests.

---

### Rule 4 — Keep the callback fast

Callback work:

```text
classify
metric
mark dirty / enqueue
return
```

Not:

```text
read entire repository
parse
run compiler
write graph DB
call remote API
```

---

### Rule 5 — Separate three clocks

```text
poll interval
≠ debounce timeout
≠ debounce tick rate
```

Know which backend/configuration each controls.

---

### Rule 6 — Do not interpret debounce as “wait for global silence”

0.7.0 ages individual events against `timeout` and flushes eligible queues on a periodic tick. [S3]

If you need strict per-path trailing-edge quiet, implement it in the application scheduler.

---

### Rule 7 — Use `need_rescan()` as a correctness boundary

```text
need_rescan == true
  → incremental history may be incomplete
  → authoritative reconcile
```

Never log and ignore this in a maintained in-memory index. [S9] [S19]

---

### Rule 8 — Reconcile initial state independently

Starting a watcher does not bootstrap all existing files.

```text
register watcher
+ initial inventory/hash reconcile
+ merge startup events
→ READY
```

---

### Rule 9 — Hash before expensive work when appropriate

```text
event
→ current bytes/hash
→ same hash? skip parse
→ changed? parse
```

This is a domain-level second debounce that is robust to backend/editor noise.

---

### Rule 10 — Fence stale async work

Every path/workspace analysis generation must prove it is still current before commit.

---

### Rule 11 — Directory removal is structural

A single remove can represent an entire subtree. Remove/reconcile indexed descendants; do not wait for child events.

---

### Rule 12 — Evaluate both sides of a rename

```text
included → ignored
ignored → included
included → included
ignored → ignored
```

Filtering only the destination is wrong.

---

### Rule 13 — A rename can be semantically more than a path change

For Rust/code models, location/module resolution may change even with identical bytes.

---

### Rule 14 — Preserve `PathBuf`

Do not make lossy UTF-8 strings your canonical watcher/index identity.

---

### Rule 15 — Symlink policy is both correctness and security

Know backend follow-symlink configuration and file-ID cache traversal behavior. [S13] [S12]

---

### Rule 16 — Filter high-churn irrelevant trees as early as practical

Application filtering saves downstream work; root selection can also save native watcher resources.

---

### Rule 17 — Bound downstream work, not necessarily callback ingestion

Use dirty sets, generation coalescing, parser pools, and overload-to-reconcile policy.

---

### Rule 18 — Polling is a legitimate backend

Use it when native events fail the actual deployment filesystem. Configure interval explicitly and measure I/O. [S8] [S14]

---

### Rule 19 — Never assume guest/container OS tells you event semantics

Host mount/filesystem topology can dominate behavior.

---

### Rule 20 — Log the active backend

```rust
let kind = Debouncer::<RecommendedWatcher, RecommendedCache>::kind();
tracing::info!(?kind, "watcher started");
```

You cannot debug platform watcher issues if you do not know what is actually running.

---

### Rule 21 — Handle async error batches

`DebounceEventResult` can be `Err(Vec<notify::Error>)`. Error delivery is part of the normal API contract. [S1]

---

### Rule 22 — Receiver disconnect is your lifecycle problem

Built-in sender handler adapters can ignore send errors. Do not assume consumer disappearance shuts the watcher down. [S3]

---

### Rule 23 — Prefer deterministic `stop()` for service shutdown

Use `stop_nonblocking` only when the remaining handler/thread lifetime is safe. [S5]

---

### Rule 24 — Test final state across platforms

Exact event traces are backend/editor-specific. Your application invariant should be filesystem/index convergence.

---

### Rule 25 — Test atomic replacement, not only in-place write

Real editors frequently use safe-write/rename strategies. [S8]

---

### Rule 26 — Test branch-switch scale

A code intelligence watcher must survive thousands of meaningful changes, not only one `foo.rs` save.

---

### Rule 27 — Make overload recoverable

```text
queue full
huge dirty set
known event loss
→ reconciliation mode
```

Not silent data loss.

---

### Rule 28 — Keep `notify` details behind an adapter

Downstream should consume stable application invalidations/FileDelta objects.

---

### Rule 29 — Do not persist raw event semantics as your domain protocol unless necessary

If persistence is required, version it explicitly.

---

### Rule 30 — Upgrade behavior, not just code

Across watcher/debouncer releases, test create/remove suppression, renames, paths, platform backends, timing, and MSRV—not just compilation. [S20]

---

## 37.1 Compact architecture doctrine

```text
Observe broadly enough for correctness.
Normalize filesystem noise.
Filter cheaply.
Coalesce work by current path/state.
Read authoritative bytes.
Calculate domain delta.
Commit only current generation.
Reconcile whenever completeness is uncertain.
Measure freshness, not callback volume.
```

---

# notify-debouncer-full Advanced — 38) Cross-cutting anti-pattern inventory

## 38.0 Timing and debounce mistakes

* Describing full debounce as a strict trailing-edge “wait until quiet” timer.
* Setting timeout extremely high to mask downstream scheduling problems.
* Setting tick rate larger than timeout.
* Confusing poll interval with debounce timeout.
* Assuming default polling is interactive; the documented default poll interval is 30 seconds. [S13]
* Choosing a huge tick rate without considering shutdown wait/jitter.
* Assuming all events in one user save collapse to exactly one event.

---

## 38.1 Event semantic mistakes

* Treating `EventKind` as a durable filesystem transaction verb.
* Matching only one exact modify subtype for content invalidation.
* Waiting for Modify after Create even though full debouncer can suppress it.
* Counting duplicate creates as meaningful activity.
* Indexing `paths[0]` without checking path count.
* Reversing `RenameMode::Both` source/destination order.
* Assuming every rename has a matchable Both event.
* Assuming rename proves bytes unchanged.
* Using Access events as source-change signals by default.
* Persisting `DebouncedEvent::time` as wall-clock time.

---

## 38.2 Consistency mistakes

* Ignoring `need_rescan()`.
* Assuming debouncer cache rescan updates application state.
* No initial filesystem inventory.
* No reconciliation after process downtime with persisted index.
* Dropping a bounded-channel batch without setting reconciliation required.
* Continuing from a known event-loss state as if healthy.
* No final-state consistency oracle in stress tests.
* Reconciliation that drops events arriving during the scan.

---

## 38.3 Async/work scheduling mistakes

* Parsing in the debouncer callback.
* Network/database calls in the callback.
* Blocking bounded-channel send on the debouncer thread.
* Unbounded work queue with no backlog metric.
* FIFO queue containing hundreds of duplicate versions of the same path.
* No per-path generation fencing.
* Old parse commits after newer parse.
* One hot workspace starves every other workspace.
* One parser task per event with unbounded concurrency.

---

## 38.4 Filtering mistakes

* Watching `target/`/`.git/`/`node_modules/` at huge scale and expecting downstream ignore rules to save native watcher resources.
* Ignoring directory removes due to file-extension filters.
* Evaluating only rename destination.
* Failing to update index membership when ignore rules change.
* Indexing swap/temp/backup files.
* Filtering only `.rs` and missing manifests/build/config inputs needed for semantic correctness.

---

## 38.5 Path/identity mistakes

* Using lossy UTF-8 path strings as canonical keys.
* Lowercasing all paths across platforms.
* Calling `canonicalize()` as required remove-path normalization.
* Treating notify file ID as durable application entity ID.
* Ignoring path/module semantic invalidation on rename.
* Trusting event path authorization.
* Ignoring symlink expansion.
* Infinite retry on stale `Modify` whose path is now gone.

---

## 38.6 Backend/platform mistakes

* Assuming `RecommendedWatcher` has identical semantics everywhere.
* Selecting backend based only on OS, not filesystem/mount.
* Native watching on NFS/WSL/shared volume without validation.
* Polling with default interval accidentally.
* Enabling `compare_contents` over huge tree without measuring.
* Treating PollWatcher as incompatible with full debouncer.
* No Linux inotify watch-limit diagnostics.
* Exact cross-platform event sequence assertions.

---

## 38.7 Cache/rename mistakes

* Manually managing deprecated `.cache()` roots in 0.7 code.
* Assuming `RecommendedCache` is the same type on all platforms.
* Ignoring FileIdMap recursive startup cost.
* Ignoring its symlink-following tree walk.
* Expecting file IDs to match every cross-filesystem move.
* Building a second rename correlation layer without understanding the full debouncer's existing logic.

---

## 38.8 Lifecycle mistakes

* Debouncer scoped to a function then dropped immediately.
* Receiver dropped before watcher source stops.
* Assuming sender disconnect stops debouncer.
* `stop_nonblocking` followed by immediate destruction of handler state.
* No explicit shutdown in integration tests.
* Persisting incomplete index as healthy on cancellation.
* Overlapping watch roots accidentally registered.

---

## 38.9 Testing mistakes

* `sleep(100ms); assert_eq!(events.len(), 1)`.
* No bounded deadlines.
* Testing only create/modify.
* No atomic-save test.
* No chained-rename test.
* No directory-delete test.
* No filter-boundary rename test.
* No application queue overflow test.
* No cross-platform CI while claiming portability.
* No deployment-specific shared-filesystem test.
* Real kernel overflow as only rescan test.

---

## 38.10 Security/resource mistakes

* Arbitrary remote user watch roots.
* Privileged watcher process without need.
* Symlink traversal with no authorization policy.
* Poll every 100 ms across enormous remote root.
* Parse unlimited file size.
* Raise OS watcher limits without tenant/workspace quotas.
* Absolute path leakage in centralized telemetry.
* Treat debounce as denial-of-service protection.

---

## 38.11 Upgrade/API mistakes

* Wildcard dependency version.
* Copying prerelease examples into stable 0.7 code.
* Direct `notify` major version different from debouncer's type universe.
* Using deprecated `.watcher()`/`.cache()` patterns.
* No MSRV test.
* Persisting raw crate events with no protocol version.
* Upgrading because code compiles without behavioral regression tests.

---

## 38.12 Code-intelligence mistakes

* Watch event directly mutates code property graph.
* Full parse before content hash/no-op check.
* Same bytes at new path assumed semantically identical.
* Every save triggers whole repository graph rebuild.
* Cargo/config changes treated as ordinary one-file edits.
* Graph transaction per event.
* No graph-generation atomicity.
* No watch-to-graph freshness metric.

---

# notify-debouncer-full Advanced — 39) Production implementation and review checklist

## 39.0 Dependency/version

```text
[ ] notify-debouncer-full version pinned explicitly.
[ ] Underlying notify version recorded.
[ ] Rust MSRV tested in CI.
[ ] Cargo feature flags explicit.
[ ] No accidental second incompatible notify major version.
[ ] Prerelease APIs not mixed with stable documentation.
```

---

## 39.1 Watcher construction

```text
[ ] new_debouncer used unless custom backend/config/cache needed.
[ ] timeout chosen from workload benchmark.
[ ] tick_rate omitted intentionally or explicitly configured.
[ ] tick_rate <= timeout.
[ ] watcher construction errors contextualized.
[ ] backend kind logged after construction.
```

---

## 39.2 Polling configuration

```text
[ ] PollWatcher used only intentionally.
[ ] poll_interval set explicitly.
[ ] PollWatcher latency budget includes poll + debounce + tick.
[ ] compare_contents justified by metadata behavior.
[ ] compare_contents I/O benchmarked.
[ ] manual polling not selected without an accessible poll() architecture.
[ ] polling backend/interval visible in health endpoint.
```

---

## 39.3 Roots and scope

```text
[ ] Watch roots are smallest stable correct scope.
[ ] RecursiveMode chosen deliberately.
[ ] Accidental overlapping roots rejected.
[ ] High-churn irrelevant trees excluded structurally where practical.
[ ] Parent watch exists if root deletion/recreation must be observed.
[ ] Root lifecycle is owned by a long-lived service object.
[ ] Dynamic roots have desired-state registry.
```

---

## 39.4 Path/security

```text
[ ] Paths remain PathBuf/Path internally.
[ ] Workspace-relative key policy defined.
[ ] Remove events do not require canonicalize().
[ ] Symlink-follow behavior explicitly understood.
[ ] Resolved paths authorized when security boundary requires it.
[ ] Untrusted users cannot watch arbitrary host roots.
[ ] Process runs with least required filesystem privilege.
[ ] Absolute path telemetry is sanitized/redacted as required.
```

---

## 39.5 Handler/delivery

```text
[ ] Handler covers Ok(events) and Err(errors).
[ ] Handler performs only fast classification/handoff.
[ ] No parsing/database/network work in callback.
[ ] Channel/backpressure policy explicit.
[ ] Receiver disconnect policy explicit.
[ ] Application overflow causes reconciliation or equivalent recovery.
[ ] Handler duration instrumented.
```

---

## 39.6 Event interpretation

```text
[ ] Create is treated as sufficient invalidation.
[ ] Modify broadly handled where content can change.
[ ] Remove handles files and whole directory subtrees.
[ ] RenameMode::Both path order handled correctly.
[ ] Unmatched/unknown rename semantics degrade safely.
[ ] Event attrs treated as optional.
[ ] Any/Other/future enum variants have fallback behavior.
[ ] DebouncedEvent::time not treated as wall clock.
```

---

## 39.7 Filtering

```text
[ ] Ignore matcher separate from watcher semantics.
[ ] Temp/swap/backup/build outputs filtered.
[ ] Directory removals bypass inappropriate extension-only filtering.
[ ] Both rename endpoints evaluated.
[ ] Ignore-rule changes trigger membership reconciliation if dynamic.
[ ] Semantic config/manifests remain watched even if not source extension.
```

---

## 39.8 Current-state resolution

```text
[ ] Event paths are re-stat/read against current filesystem state.
[ ] NotFound is a normal race outcome.
[ ] Content hash/no-op detection implemented where valuable.
[ ] File-size limit enforced before expensive read/parse.
[ ] Authoritative FileDelta abstraction produced after state resolution.
[ ] Watcher event is not directly treated as FileDelta.
```

---

## 39.9 Async scheduling

```text
[ ] Unique dirty-path coalescing implemented for current-state index.
[ ] Parser concurrency bounded.
[ ] Per-path/workspace generation counter exists.
[ ] Stale analysis cannot commit.
[ ] Work is fair across workspaces/tenants.
[ ] Large burst can switch to reconciliation mode.
[ ] Queue length and oldest backlog age measured.
```

---

## 39.10 Reconciliation

```text
[ ] Initial reconciliation runs before READY.
[ ] Persisted index reconciled after downtime.
[ ] need_rescan() triggers authoritative reconciliation.
[ ] Application queue overflow triggers reconciliation.
[ ] Reconciliation requests coalesced.
[ ] Events arriving during reconcile are fenced/retained.
[ ] Reconcile calculates minimal file delta when possible.
[ ] Reconcile commits atomically or with explicit incomplete generation.
[ ] Failed reconcile puts service/workspace in Degraded state.
```

---

## 39.11 Code-intelligence graph updates

```text
[ ] File content/location/config/dependency invalidation distinguished.
[ ] Rename can trigger semantic re-resolution even with same hash.
[ ] Manifest/build-script changes have appropriate blast radius.
[ ] Parser result tagged with source generation/hash.
[ ] Graph delta validated before commit.
[ ] Graph transaction atomic per chosen generation/batch.
[ ] Removed directory cascades to owned graph entities.
[ ] No stale nodes owned by absent files.
```

---

## 39.12 Platform: Linux

```text
[ ] Inotify backend confirmed when expected.
[ ] max_user_watches known for large trees.
[ ] ENOSPC diagnostic is actionable.
[ ] Burst/queue-loss reconciliation tested.
[ ] WSL/network mounts not assumed native-reliable.
```

---

## 39.13 Platform: macOS

```text
[ ] FSEvents/kqueue feature choice pinned.
[ ] RecommendedCache/FileIdMap startup measured.
[ ] Atomic-save editor behavior tested.
[ ] File ownership/permission environment tested.
[ ] Symlink/cache traversal scope reviewed.
```

---

## 39.14 Platform: Windows

```text
[ ] ReadDirectoryChangesW backend confirmed when expected.
[ ] FileIdMap startup/rename behavior tested.
[ ] Drive/UNC/path forms tested.
[ ] Atomic replace/rename tested.
[ ] Transient read contention handled.
[ ] Network shares validated independently.
```

---

## 39.15 Tests

```text
[ ] Pure event classifier tests.
[ ] Path/filter truth-table tests.
[ ] Real create integration test.
[ ] Real modify integration test.
[ ] Same-filesystem rename test.
[ ] Chained rename test.
[ ] Rename-over-existing-target test if relevant.
[ ] Directory delete test.
[ ] Atomic-save test.
[ ] Formatter rapid-double-write test.
[ ] Filter-boundary rename four-way test.
[ ] Initial-state bootstrap test.
[ ] Synthetic rescan recovery test.
[ ] Application queue overflow recovery test.
[ ] Branch-switch/large burst stress test.
[ ] Native CI on every supported OS.
[ ] PollWatcher test on difficult deployment topology.
[ ] Final filesystem/index hash oracle.
```

---

## 39.16 Observability

```text
[ ] watcher kind exposed.
[ ] roots/config exposed safely.
[ ] debounce timeout/tick exposed.
[ ] poll interval/content compare exposed when applicable.
[ ] success/error batch counters.
[ ] event-kind counters.
[ ] rescan/reconcile counters.
[ ] dirty backlog size + oldest age.
[ ] handler latency.
[ ] parse/hash/graph commit latency.
[ ] watch-to-index freshness.
[ ] stale generation discard counter.
[ ] Healthy/Reconciling/Degraded state.
```

---

## 39.17 Security/resource governance

```text
[ ] Root authorization precedes registration.
[ ] Watch-tree scale quotas exist where untrusted.
[ ] Poll frequency minimum enforced where untrusted.
[ ] compare_contents restricted where expensive.
[ ] Parser/file-size/CPU limits exist.
[ ] Work quotas prevent event-storm starvation.
[ ] Symlink escape policy tested.
[ ] Path/source telemetry privacy reviewed.
```

---

## 39.18 Shutdown

```text
[ ] Debouncer has explicit owner.
[ ] Source stopped before consumers destroyed.
[ ] stop() used for deterministic join where appropriate.
[ ] stop_nonblocking only with safe remaining ownership.
[ ] Drain vs cancel policy documented.
[ ] Incomplete reconcile/index generation handled safely.
[ ] Worker tasks/threads joined or intentionally abandoned at process exit.
[ ] Integration tests shut down watcher before fixture cleanup.
```

---

## 39.19 Upgrade readiness

```text
[ ] Direct crate usage isolated behind adapter.
[ ] Deprecated APIs eliminated.
[ ] Stable application invalidation/FileDelta schema exists.
[ ] Raw notify Event not persisted as unversioned protocol.
[ ] Behavioral regression suite covers normalization.
[ ] Platform backend CI is automated.
[ ] MSRV CI is automated.
[ ] Cargo dependency graph inspection is part of upgrade workflow.
```

---
# notify-debouncer-full Advanced — 40) API quick reference and canonical recipes

## 40.0 Core dependency

```toml
[dependencies]
notify-debouncer-full = "=0.7.0"
```

Optional application dependencies commonly useful around it:

```toml
[dependencies]
# async bridge / service runtime, only if your app uses Tokio
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "signal"] }

# application errors/logging, optional
thiserror = "2"
tracing = "0.1"
```

Use `notify-debouncer-full::notify::*` re-exports for notify types unless you intentionally need a direct `notify` dependency.

---

## 40.1 Core imports

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
    DebouncedEvent,
    Debouncer,
    RecommendedCache,
    notify::{
        Event,
        EventKind,
        RecursiveMode,
        RecommendedWatcher,
        WatcherKind,
        event::{ModifyKind, RenameMode},
    },
};
```

For custom construction:

```rust
use notify_debouncer_full::{
    new_debouncer_opt,
    FileIdMap,
    NoCache,
    notify::{Config, PollWatcher},
};
```

---

## 40.2 Core type signatures: mental quick reference

```text
DebounceEventResult
  = Result<Vec<DebouncedEvent>, Vec<notify::Error>>

DebouncedEvent
  .event: notify::Event
  .time: Instant
  derefs to Event

Debouncer<W, C>
  W: notify::Watcher
  C: FileIdCache

RecommendedWatcher
  = platform-selected native watcher (or fallback)

RecommendedCache
  = NoCache on Linux/Android/WASM
  = FileIdMap on other supported platforms
```

See the pinned crate rustdoc/source for exact generic bounds when building custom abstractions. [S1] [S5] [S7]

---

## 40.3 `new_debouncer`

Canonical shape:

```rust
let debouncer = new_debouncer(
    timeout,
    optional_tick_rate,
    handler,
)?;
```

Interpretation:

```text
timeout:
  event-age threshold before eligible emission

tick_rate:
  periodic scan interval
  None → timeout / 4

handler:
  DebounceEventHandler
```

`tick_rate` must not exceed `timeout`. [S3] [S4]

---

## 40.4 `new_debouncer_opt`

Conceptual shape:

```rust
let debouncer = new_debouncer_opt::<_, WatcherType, CacheType>(
    timeout,
    tick_rate,
    handler,
    cache,
    notify_config,
)?;
```

Use when choosing custom:

```text
Watcher type
FileIdCache
notify::Config
polling settings
symlink settings
```

[S3]

---

## 40.5 `Debouncer` methods

Application-important 0.7 methods:

```rust
debouncer.watch(path, RecursiveMode::Recursive)?;
debouncer.unwatch(path)?;
debouncer.configure(config)?;

// Associated function on the concrete Debouncer type:
let kind = Debouncer::<RecommendedWatcher, RecommendedCache>::kind();

debouncer.stop();
// or
debouncer.stop_nonblocking();
```

Avoid deprecated ordinary usage of:

```rust
debouncer.watcher()
debouncer.cache()
```

Root/cache bookkeeping is handled through the debouncer's direct methods in current stable 0.7. [S5]

---

## 40.6 Recipe A — smallest real recursive watcher

```rust
use notify_debouncer_full::{
    new_debouncer,
    DebounceEventResult,
    Debouncer,
    RecommendedCache,
    notify::{RecursiveMode, RecommendedWatcher},
};
use std::{path::Path, time::Duration};

fn main() -> notify_debouncer_full::notify::Result<()> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    for event in events {
                        println!("{:?} {:?}", event.kind, event.paths);
                    }
                }
                Err(errors) => {
                    for error in errors {
                        eprintln!("watch error: {error:?}");
                    }
                }
            }
        },
    )?;

    debouncer.watch(
        Path::new("./src"),
        RecursiveMode::Recursive,
    )?;

    println!(
        "backend={:?}",
        Debouncer::<RecommendedWatcher, RecommendedCache>::kind(),
    );

    // Keep owner alive for demo.
    std::thread::park();

    #[allow(unreachable_code)]
    Ok(())
}
```

Production modification:

```text
replace thread::park with application lifecycle/shutdown
hand off events rather than heavy processing in callback
initially reconcile existing tree
```

---

## 40.7 Recipe B — standard channel handoff

```rust
use notify_debouncer_full::{
    new_debouncer,
    notify::RecursiveMode,
    DebounceEventResult,
};
use std::{
    io,
    path::Path,
    sync::mpsc,
    thread,
    time::Duration,
};

fn main() -> notify_debouncer_full::notify::Result<()> {
    let (tx, rx) = mpsc::channel::<DebounceEventResult>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(75),
        None,
        tx,
    )?;

    debouncer.watch(
        Path::new("."),
        RecursiveMode::Recursive,
    )?;

    let worker = thread::spawn(move || {
        for result in rx {
            match result {
                Ok(events) => {
                    for event in events {
                        println!("{:?}", event);
                    }
                }
                Err(errors) => {
                    eprintln!("watch errors: {errors:?}");
                }
            }
        }
    });

    println!("Watching. Press Enter to stop.");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);

    // Joined watcher shutdown drops the callback-owned sender.
    debouncer.stop();
    let _ = worker.join();

    Ok(())
}
```

Remember: built-in sender handling ignores send failures on a disconnected receiver; service lifetime must not depend on that error propagating. [S3]

---

## 40.8 Recipe C — detect normalized rename

```rust
use notify_debouncer_full::notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind,
};
use std::path::Path;

fn normalized_rename(event: &Event) -> Option<(&Path, &Path)> {
    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
            if event.paths.len() >= 2 =>
        {
            Some((&event.paths[0], &event.paths[1]))
        }
        _ => None,
    }
}
```

Contract:

```text
paths[0] = from
paths[1] = to
```

for `RenameMode::Both`. [S10]

---

## 40.9 Recipe D — classify into application invalidations

```rust
use notify_debouncer_full::{
    DebouncedEvent,
    notify::{
        EventKind,
        event::{ModifyKind, RenameMode},
    },
};
use std::path::PathBuf;

#[derive(Debug)]
enum Invalidation {
    Dirty(Vec<PathBuf>),
    Removed(Vec<PathBuf>),
    Rename { from: PathBuf, to: PathBuf },
    Reconcile,
    Ignore,
}

fn classify(event: &DebouncedEvent) -> Invalidation {
    if event.need_rescan() {
        return Invalidation::Reconcile;
    }

    if let EventKind::Modify(ModifyKind::Name(RenameMode::Both)) = event.kind {
        if event.paths.len() >= 2 {
            return Invalidation::Rename {
                from: event.paths[0].clone(),
                to: event.paths[1].clone(),
            };
        }
    }

    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            Invalidation::Dirty(event.paths.clone())
        }
        EventKind::Remove(_) => {
            Invalidation::Removed(event.paths.clone())
        }
        _ => Invalidation::Ignore,
    }
}
```

This adapter is intentionally conservative.

---

## 40.10 Recipe E — rename inclusion truth table

```rust
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenameScope {
    IncludedToIncluded,
    IncludedToIgnored,
    IgnoredToIncluded,
    IgnoredToIgnored,
}

fn rename_scope(
    from: &Path,
    to: &Path,
    included: impl Fn(&Path) -> bool,
) -> RenameScope {
    match (included(from), included(to)) {
        (true, true) => RenameScope::IncludedToIncluded,
        (true, false) => RenameScope::IncludedToIgnored,
        (false, true) => RenameScope::IgnoredToIncluded,
        (false, false) => RenameScope::IgnoredToIgnored,
    }
}
```

Application actions:

```text
IncludedToIncluded → move/re-resolve/reparse as needed
IncludedToIgnored  → remove old record
IgnoredToIncluded  → add/read/index new record
IgnoredToIgnored   → ignore
```

---

## 40.11 Recipe F — current-state resolver

```rust
use std::{fs, io, path::Path};

#[derive(Debug)]
enum CurrentFileState {
    Present(Vec<u8>),
    Absent,
}

fn read_current_state(path: &Path) -> io::Result<CurrentFileState> {
    match fs::read(path) {
        Ok(bytes) => Ok(CurrentFileState::Present(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(CurrentFileState::Absent)
        }
        Err(error) => Err(error),
    }
}
```

Real systems should enforce maximum file size before allocating/reading large content.

---

## 40.12 Recipe G — dirty-path set

Conceptual synchronous version:

```rust
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct DirtySet {
    inner: Arc<Mutex<HashSet<PathBuf>>>,
}

impl DirtySet {
    fn mark<I>(&self, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut dirty = self.inner.lock().unwrap();
        dirty.extend(paths);
    }

    fn drain(&self) -> Vec<PathBuf> {
        let mut dirty = self.inner.lock().unwrap();
        dirty.drain().collect()
    }
}
```

Production code should handle poisoned locks/error policy and use concurrency primitives appropriate to workload. The architectural point is unique current paths, not this exact mutex implementation.

---

## 40.13 Recipe H — Tokio bridge with overflow recovery

```rust
use notify_debouncer_full::DebounceEventResult;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;

fn build_handler(
    tx: mpsc::Sender<DebounceEventResult>,
    reconcile_required: Arc<AtomicBool>,
) -> impl FnMut(DebounceEventResult) + Send + 'static {
    move |result| {
        match tx.try_send(result) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                reconcile_required.store(true, Ordering::Release);
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Supervisor/shutdown owns this condition.
            }
        }
    }
}
```

Invariant:

```text
queue overflow cannot silently reduce correctness
```

---

## 40.14 Recipe I — generation fence

```rust
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct Generation {
    current: AtomicU64,
}

impl Generation {
    fn mark_dirty(&self) -> u64 {
        self.current.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn current(&self) -> u64 {
        self.current.load(Ordering::Acquire)
    }

    fn still_current(&self, generation: u64) -> bool {
        self.current() == generation
    }
}
```

Worker:

```text
g = mark_dirty()
parse snapshot
if still_current(g): commit
else: discard result; newer work exists
```

For multiple paths, maintain generations in a map or file-record state rather than one global counter.

---

## 40.15 Recipe J — rescan flag handling

```rust
fn handle_batch(
    events: Vec<DebouncedEvent>,
    state: &WatchState,
) {
    if events.iter().any(|event| event.need_rescan()) {
        state.request_reconciliation();
    }

    for event in events {
        if event.need_rescan() {
            continue;
        }
        state.handle_incremental(classify(&event));
    }
}
```

If a rescan signal makes incremental completeness uncertain, application policy may instead skip all incremental processing in that batch and rely on reconcile. Choose one documented strategy.

---

## 40.16 Recipe K — custom `PollWatcher`

```rust
use notify_debouncer_full::{
    new_debouncer_opt,
    Debouncer,
    notify::{Config, PollWatcher, RecursiveMode},
    NoCache,
};
use std::time::Duration;

fn main() -> notify_debouncer_full::notify::Result<()> {
    let config = Config::default()
        .with_poll_interval(Duration::from_secs(1))
        .with_compare_contents(false);

    let mut debouncer = new_debouncer_opt::<_, PollWatcher, _>(
        Duration::from_millis(150),
        None,
        |result| println!("{result:?}"),
        NoCache,
        config,
    )?;

    debouncer.watch(".", RecursiveMode::Recursive)?;
    println!("backend={:?}", Debouncer::<PollWatcher, NoCache>::kind());

    std::thread::park();
    #[allow(unreachable_code)]
    Ok(())
}
```

For unreliable mtimes:

```rust
let config = Config::default()
    .with_poll_interval(Duration::from_secs(1))
    .with_compare_contents(true);
```

Measure I/O before deploying over large trees. [S13] [S14]

---

## 40.17 Recipe L — explicit symlink policy for `notify::Config`

```rust
let config = notify_debouncer_full::notify::Config::default()
    .with_follow_symlinks(false);
```

This controls relevant watcher backends. Remember that the full debouncer's `FileIdMap` cache traversal has its own implementation behavior and follows links during its WalkDir scan. [S13] [S12]

Security-sensitive systems should test the complete path, not assume one flag defines all traversal behavior.

---

## 40.18 Recipe M — service owner with deterministic shutdown

```rust
use notify_debouncer_full::{
    Debouncer,
    RecommendedCache,
    notify::RecommendedWatcher,
};

type DefaultDebouncer =
    Debouncer<RecommendedWatcher, RecommendedCache>;

struct WatchService {
    debouncer: Option<DefaultDebouncer>,
}

impl WatchService {
    fn shutdown(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }
    }
}

impl Drop for WatchService {
    fn drop(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }
    }
}
```

Be cautious doing substantial blocking work in `Drop` for complex async applications; an explicit async/service shutdown method may be more appropriate. The point is deterministic ownership and joined watcher shutdown.

---

## 40.19 Recipe N — initial reconciliation + watcher startup ordering

Robust conceptual startup:

```text
A. construct/register watcher first
B. record startup generation
C. inventory filesystem
D. build/repair index
E. process/coalesce dirtiness received since B
F. verify no rescan-required condition unresolved
G. mark workspace READY
```

Why watcher first?

```text
If inventory starts before watcher registration,
a change occurring between inventory and watcher startup can be missed entirely.
```

Why reconcile after registering?

```text
Existing files generate no guaranteed historical create stream.
```

The small startup overlap plus dirty-set replay closes the gap.

---

## 40.20 Recipe O — final-state reconciliation

Conceptual algorithm:

```rust
struct FileSnapshot {
    path: PathBuf,
    hash: ContentHash,
}

fn reconcile(
    filesystem: Vec<FileSnapshot>,
    indexed: Vec<FileSnapshot>,
) -> Vec<FileDelta> {
    // map by canonical workspace-relative path
    // filesystem only  → Added
    // index only       → Removed
    // both, hash diff  → Changed
    // both, hash same  → no-op
    // optional identity heuristics → Renamed
    todo!()
}
```

Keep the same `FileDelta` consumer used by ordinary incremental updates.

---

## 40.21 Recipe P — directory removal cascade

```rust
fn remove_subtree_from_index(
    removed_dir: &Path,
    indexed_paths: impl Iterator<Item = PathBuf>,
) -> Vec<PathBuf> {
    indexed_paths
        .filter(|path| path.starts_with(removed_dir))
        .collect()
}
```

Use workspace-relative, normalized path keys consistently. For massive indexes, use a database/path-prefix index rather than scanning all records.

---

## 40.22 Recipe Q — backend health snapshot

```rust
#[derive(Debug)]
struct WatchHealthSnapshot {
    backend: String,
    debounce_timeout_ms: u64,
    tick_rate_ms: u64,
    roots: usize,
    dirty_paths: usize,
    reconcile_required: bool,
    reconciling: bool,
    last_error: Option<String>,
}
```

Keep absolute root paths out of remote telemetry unless required/authorized.

---

## 40.23 Recipe R — platform-independent semantic test

```text
fixture:
  src/a.rs = A1

start service + wait READY
assert index hash(a.rs) == hash(A1)

write A2
wait until index hash(a.rs) == hash(A2)

rename a.rs → b.rs
wait until:
  a.rs absent
  b.rs hash == hash(A2)

delete b.rs
wait until b.rs absent

run consistency oracle:
  filesystem included-path map == index file map
```

This test remains meaningful on Linux/macOS/Windows even when intermediate event sequences differ.

---

## 40.24 Recipe S — timeout/tick benchmark matrix

Run representative values rather than guessing:

```text
timeout ms: 25, 50, 75, 100, 150, 250
tick ms:    timeout/4 default, plus selected lower/higher valid values
```

Workloads:

```text
single save
save + formatter
10-file refactor
1000-file checkout
10k-file branch switch
```

Measure:

```text
delivered events
unique dirty paths
parses executed
stale parses discarded
watch→index p50/p95/p99
CPU
memory
rescan/overflow incidence
```

Choose the profile minimizing **total useful work and freshness latency**, not just event volume.

---

## 40.25 Recipe T — complete recommended architecture for a code-property graph

```text
┌──────────────────────── filesystem ────────────────────────┐
│ create / write / atomic replace / rename / remove          │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
                    RecommendedWatcher
                  (or explicit PollWatcher)
                              │
                              ▼
                 notify-debouncer-full 0.7
                timeout + tick + rename state
                              │
                 DebounceEventResult batches
                              │
                              ▼
                      WatchAdapter
                ┌─────────────┴─────────────┐
                │                           │
           need_rescan                 incremental
                │                           │
                ▼                           ▼
       reconciliation flag            path classifier
                │                           │
                └─────────────┬─────────────┘
                              ▼
                        dirty-path set
                              │
                              ▼
                    bounded work scheduler
                              │
                              ▼
                   current-state resolver
                  stat / existence / read / hash
                              │
                              ▼
                          FileDelta
                              │
                  ┌───────────┴───────────┐
                  ▼                       ▼
            syntax parser          config resolver
                  │                       │
                  └───────────┬───────────┘
                              ▼
                     semantic extractor
                              │
                              ▼
                  dependency/blast radius
                              │
                              ▼
                      graph delta builder
                              │
                        validate generation
                              │
                              ▼
                     atomic graph commit
                              │
                              ▼
                  freshness / health metrics
```

Operational side channel:

```text
need_rescan / queue overflow / restart after downtime
  → enumerate authoritative current tree
  → compare with indexed inventory
  → produce FileDelta set
  → feed same downstream pipeline
```

This is the preferred architecture because filesystem-specific uncertainty ends at `WatchAdapter`/state resolution, while the semantic engine consumes stable application-owned deltas.

---

## 40.26 Final agent invariants

```text
Invariant 1:
  Debouncing reduces/normalizes events; it does not make the stream durable.

Invariant 2:
  timeout is per-event age eligibility, not a guaranteed global quiet period.

Invariant 3:
  EventKind is evidence about filesystem activity, not authoritative domain state.

Invariant 4:
  need_rescan means incremental completeness is uncertain; reconcile.

Invariant 5:
  Create is sufficient invalidation; do not require a subsequent Modify.

Invariant 6:
  RenameMode::Both paths are [from, to].

Invariant 7:
  directory remove can stand for an entire subtree.

Invariant 8:
  PollWatcher interval and debounce timeout are independent clocks.

Invariant 9:
  RecommendedCache is platform-dependent.

Invariant 10:
  callback work must be bounded and fast.

Invariant 11:
  queue overflow cannot silently preserve a “healthy” consistency state.

Invariant 12:
  current bytes/hash/path context determine FileDelta and semantic update.

Invariant 13:
  stale async analysis may never commit over newer generations.

Invariant 14:
  initial and post-downtime state require reconciliation independent of events.

Invariant 15:
  cross-platform correctness tests assert convergence, not identical event traces.

Invariant 16:
  wrapper/adapter owns notify-specific API; domain engine owns semantic truth.
```

---

# Source anchors

This reference intentionally uses **version-pinned released Rust API/source documentation** for call signatures and implementation-sensitive behavior, plus the official `notify-rs/notify` project/release pages for release-line notes. Re-check these anchors when changing the pinned crate version.

[S1]: https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/ "notify_debouncer_full 0.7.0 — crate documentation"

[S2]: https://docs.rs/crate/notify-debouncer-full/0.7.0 "notify-debouncer-full 0.7.0 — crate metadata, versions, features, dependencies"

[S3]: https://docs.rs/notify-debouncer-full/0.7.0/src/notify_debouncer_full/lib.rs.html "notify-debouncer-full 0.7.0 — lib.rs source"

[S4]: https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/fn.new_debouncer.html "new_debouncer — notify-debouncer-full 0.7.0"

[S5]: https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/struct.Debouncer.html "Debouncer — notify-debouncer-full 0.7.0"

[S6]: https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/struct.DebouncedEvent.html "DebouncedEvent — notify-debouncer-full 0.7.0"

[S7]: https://docs.rs/notify-debouncer-full/0.7.0/src/notify_debouncer_full/cache.rs.html "FileIdCache / RecommendedCache source — notify-debouncer-full 0.7.0"

[S8]: https://docs.rs/notify/8.2.0/notify/ "notify 8.2.0 — crate documentation and known problems"

[S9]: https://docs.rs/notify/8.2.0/notify/event/struct.Event.html "notify::Event — notify 8.2.0"

[S10]: https://docs.rs/notify/8.2.0/notify/event/enum.RenameMode.html "RenameMode — notify 8.2.0"

[S11]: https://docs.rs/notify/8.2.0/notify/event/enum.EventKind.html "EventKind — notify 8.2.0"

[S12]: https://docs.rs/notify-debouncer-full/0.7.0/src/notify_debouncer_full/file_id_map.rs.html "FileIdMap source — notify-debouncer-full 0.7.0"

[S13]: https://docs.rs/notify/8.2.0/notify/struct.Config.html "notify::Config — notify 8.2.0"

[S14]: https://docs.rs/notify/8.2.0/notify/poll/struct.PollWatcher.html "PollWatcher — notify 8.2.0"

[S15]: https://docs.rs/notify/8.2.0/notify/enum.WatcherKind.html "WatcherKind — notify 8.2.0"

[S16]: https://docs.rs/notify/8.2.0/src/notify/lib.rs.html "notify 8.2.0 — lib.rs source / RecommendedWatcher aliases"

[S17]: https://docs.rs/notify/8.2.0/notify/trait.Watcher.html "Watcher trait — notify 8.2.0"

[S18]: https://docs.rs/notify/8.2.0/notify/event/enum.ModifyKind.html "ModifyKind — notify 8.2.0"

[S19]: https://docs.rs/notify/8.2.0/notify/event/enum.Flag.html "Event Flag / Rescan — notify 8.2.0"

[S20]: https://github.com/notify-rs/notify/releases "notify-rs official release notes — notify and notify-debouncer-full"

[S21]: https://docs.rs/file-id/0.2.3/file_id/ "file-id 0.2.3 — filesystem file identity helper"

---

# Closing implementation stance

The safest way to use `notify-debouncer-full` is to give it a **narrow, explicit responsibility**:

```text
turn noisy, backend-shaped filesystem notifications
into a lower-noise, rename-aware stream of invalidations
```

Then let an application-owned state resolver decide what actually changed.

For robust stateful systems—especially code indexes, dependency graphs, and continuously updated code property graphs—the correctness loop is:

```text
observe
→ debounce/normalize
→ mark dirty
→ read authoritative state
→ derive delta
→ validate generation
→ commit atomically
→ reconcile whenever completeness is uncertain
```

That architecture preserves the performance benefits of native incremental watching without making filesystem notifications carry guarantees they do not provide.
