# Rust Parallelism & Concurrency Stack — advanced technical reference

**Crates covered:** `tokio`, `rayon`, `crossbeam`, `dashmap`, `tokio-rayon`  
**Reference date:** 2026-08-19  
**Primary audience:** Rust developers and LLM coding agents implementing highly concurrent / parallel systems  
**Structure:** modeled on the supplied DataFusion advanced reference: version-pinned source anchors, exhaustive feature map, self-contained deep dives, canonical code, deployment rules, testing matrices, anti-patterns, and agent checklists.

---

## Version / source anchors

This reference is anchored to the latest released crate versions verified on docs.rs on 2026-08-19:

| Crate | Version anchor | Primary role in this stack |
|---|---:|---|
| `tokio` | **1.53.1** | asynchronous runtime, task scheduler, non-blocking I/O, async synchronization |
| `rayon` | **1.12.0** | CPU-bound data parallelism and fork/join work stealing |
| `crossbeam` | **0.8.4** | lower-level concurrent data structures, channels, work-stealing primitives, atomics, epoch reclamation |
| `dashmap` | **6.2.1** | sharded concurrent hash map / set |
| `tokio-rayon` | **2.1.0** | thin async bridge from Tokio code to Rayon pools |

Source-of-truth stance:

* Use **docs.rs for the exact released API** corresponding to the pinned version.
* Use the crate's own repository / source manifest for feature flags, MSRV, changelog, and release notes.
* Treat `latest` URLs as navigation conveniences, not reproducibility guarantees. Production code should pin versions through `Cargo.toml` + `Cargo.lock` and test upgrades.
* `tokio-rayon` deserves an extra caveat: its current release is still 2.1.0 from 2021. Its API is tiny and remains compatible with Tokio 1.x / Rayon 1.x through semver dependency ranges, but treat it as an optional convenience layer rather than an architectural requirement. ([tokio-rayon docs][TR0])
* `crossbeam` is a facade crate whose 0.8.4 manifest specifies compatible ranges for several independently released subcrates. If reproducible transitive versions matter, commit `Cargo.lock`; if you import subcrates directly, pin them explicitly. Current direct-subcrate anchors are `crossbeam-channel 0.5.16`, `crossbeam-deque 0.8.7`, `crossbeam-epoch 0.9.20`, `crossbeam-queue 0.3.13`, and `crossbeam-utils 0.8.22`. ([Crossbeam facade][C0])

---

## Feature inventory: what this reference covers

The stack breaks naturally into five complementary layers:

```text
application / service / CLI
        |
        +-- Tokio ------------------------------------+
        |   async tasks, network/files/process I/O,   |
        |   timers, cancellation, channels, admission |
        |                                              |
        +-- tokio-rayon -------------------------------+--> async/CPU hand-off
        |                                              |
        +-- Rayon -------------------------------------+
        |   fixed-size CPU pool, parallel iterators,   |
        |   fork/join, work stealing, reductions       |
        |                                              |
        +-- Crossbeam ---------------------------------+
        |   MPMC channels/queues, deques, atomics,     |
        |   epoch GC, scheduler-building primitives    |
        |                                              |
        +-- DashMap -----------------------------------+
            sharded shared map/set when shared keyed
            mutable state is genuinely required
```

The most important boundary is **async concurrency vs CPU parallelism**. Tokio's multithreaded scheduler can run tasks on multiple cores, but Tokio's design center is asynchronous, non-blocking work. Rayon is the normal default when the objective is to keep cores busy with CPU-bound work. Tokio itself recommends a dedicated CPU pool such as Rayon when many CPU-bound computations need bounded parallelism. ([Tokio blocking guidance][T_BLOCK])

This reference covers:

* async runtime construction and scheduler topology;
* Tokio tasks, join handles, cooperative scheduling, `select!`, cancellation, timeouts, channels, synchronization, I/O, blocking boundaries, and runtime tuning;
* Rayon parallel iterators, reductions, nested parallelism, `join`, `scope`, spawning, custom pools, scheduling order, grain size, panic behavior, and CPU tuning;
* Crossbeam channels, queues, deques, epoch reclamation, atomics, cache padding, backoff, parking, wait groups, and when to use direct subcrates;
* DashMap construction, sharding, reference guards, entries, mutation, iteration, Rayon integration, read-only views, raw API, and deadlock hazards;
* `tokio-rayon` global/custom-pool bridge APIs and their limitations;
* whole-system thread budgets, oversubscription, backpressure, bounded CPU admission, cancellation, shutdown, shared-state design, pipeline patterns, testing, observability, profiling, deployment, and version migration;
* explicit LLM-agent invariants so generated code selects the correct executor and does not accidentally serialize or oversubscribe the program.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and stack mental model
## 1) Installation, dependency selection, features, and project layout
## 2) First executable programs
## 3) Concurrency, parallelism, asynchrony, and thread-pool vocabulary
## 4) Work-classification and executor-selection decision procedure
## 5) Whole-process thread topology and oversubscription budget
## 6) Tokio identity and runtime architecture
## 7) Tokio runtime construction, feature flags, and builders
## 8) Tokio tasks, `spawn`, `JoinHandle`, and task lifecycle
## 9) Tokio cooperative scheduling, yielding, fairness, and task granularity
## 10) Tokio channels: `mpsc`, `oneshot`, `broadcast`, `watch`
## 11) Tokio state synchronization: `Mutex`, `RwLock`, `Semaphore`, `Notify`, `Barrier`
## 12) Tokio `select!`, cancellation safety, timeouts, and abort semantics
## 13) Tokio blocking boundary: `spawn_blocking` and `block_in_place`
## 14) Tokio I/O, filesystem, process, signals, and timers
## 15) Tokio testing, runtime metrics, and deployment tuning
## 16) Rayon identity and work-stealing mental model
## 17) Rayon parallel iterators
## 18) Rayon indexed iterators, slices, strings, and parallel sorting
## 19) Rayon folds, reductions, fallible parallelism, and early termination
## 20) Rayon `join`, `scope`, `spawn`, FIFO variants, and structured fork/join
## 21) Rayon custom `ThreadPool` and `ThreadPoolBuilder`
## 22) Rayon nested parallelism, granularity, scheduling, and pool choice
## 23) Rayon panics, blocking, cooperative yielding, and cancellation patterns
## 24) Rayon performance: cache locality, allocation, false sharing, NUMA, and SIMD interaction
## 25) Crossbeam identity, facade/subcrate topology, and feature selection
## 26) Crossbeam channels and synchronous pipeline backpressure
## 27) Crossbeam work-stealing deques
## 28) Crossbeam concurrent queues
## 29) Crossbeam epoch-based reclamation
## 30) Crossbeam atomics, synchronization, scoped threads, and utilities
## 31) Building custom worker schedulers and pipelines with Crossbeam
## 32) DashMap identity, sharding model, and construction
## 33) DashMap core API, entries, and mutation
## 34) DashMap reference guards, locking behavior, and deadlock prevention
## 35) DashMap iteration, Rayon integration, read-only views, `DashSet`, and raw API
## 36) DashMap design alternatives and shared-state decision rules
## 37) tokio-rayon identity and API
## 38) tokio-rayon global pool, custom pools, LIFO/FIFO scheduling, and panic semantics
## 39) Canonical Tokio ↔ Rayon integration architectures
## 40) CPU admission, backpressure, queue bounds, and overload control
## 41) Shared state vs message passing vs reduce/merge
## 42) Pipeline, fan-out/fan-in, actor, and staged-work patterns
## 43) Errors, panics, cancellation, and graceful shutdown across the stack
## 44) Testing concurrent and parallel Rust
## 45) Benchmarking, profiling, tracing, and performance regression methodology
## 46) Production deployment patterns
## 47) Robustness, resource governance, and denial-of-service boundaries
## 48) API stability, upgrades, and migration procedure
## 49) Selection recipes and decision tables
## 50) Best practices and anti-pattern compendium
## 51) Final LLM-agent invariants and implementation checklist

---

# Rust Parallelism Stack Advanced — 0) Scope, versioning, and stack mental model

## 0.0 Documentation stance

This is not a generic “Rust threading” tutorial. It is an implementation reference for deciding **where work should run, how much of it may run at once, how results move between layers, and which synchronization model should be used**.

The core invariant is:

```text
waiting / I/O-bound work  -> Tokio
CPU-bound parallel work   -> Rayon
sync worker pipelines     -> Crossbeam
shared keyed state        -> DashMap (only when actually needed)
Tokio -> Rayon hand-off   -> direct bridge or tokio-rayon
```

There are valid exceptions, but exceptions should be intentional and documented.

## 0.1 Identity: what each crate is / is not

| Crate | What it is | What it is not |
|---|---|---|
| Tokio | event-driven async runtime with multithread work-stealing task scheduler, I/O reactor, timers and async primitives | a general CPU-batch parallelism framework |
| Rayon | data-parallel / fork-join CPU executor over a fixed work-stealing pool | async I/O runtime or long-lived blocking-worker framework |
| Crossbeam | collection of lower-level concurrency components | single integrated runtime |
| DashMap | sharded concurrent hash map / set | automatically contention-free shared state or substitute for data ownership design |
| tokio-rayon | thin future-returning bridge onto Rayon | independent scheduler, admission controller, cancellation framework, or replacement for Tokio/Rayon |

Tokio's own crate docs describe a multithreaded work-stealing scheduler plus event-driven I/O, while its blocking guidance recommends a dedicated pool such as Rayon for CPU-bound work. ([Tokio crate][T0]) ([Tokio blocking guidance][T_BLOCK]) Rayon describes itself as a data-parallelism library and uses work stealing to exploit idle CPUs. ([Rayon crate][R0])

## 0.2 Value case for the combined stack

The stack gives you different scheduling semantics at different layers instead of asking one executor to do everything:

| Need | Preferred mechanism |
|---|---|
| 50k sockets mostly waiting | Tokio tasks |
| parse/hash/analyze 100k files | Rayon parallel iterator / custom pool |
| bridge async request to parse job | `tokio-rayon`, `oneshot`, or custom submission wrapper |
| bounded synchronous stage queue | `crossbeam::channel::bounded` |
| custom work-stealing scheduler | `crossbeam::deque` |
| lock-free data structure internals | `crossbeam::epoch` + atomics |
| concurrent symbol/index cache | DashMap, provided guard lifetimes are tightly controlled |
| request concurrency cap | Tokio `Semaphore` |
| CPU parallelism cap | Rayon pool size, optionally plus upstream semaphore |
| per-worker scratch buffers | Rayon `map_init` / `for_each_init`, thread-local state, or worker ownership |

## 0.3 Canonical whole-stack execution model

```text
external event / request / filesystem notification
        |
        v
Tokio runtime
  parse request, async I/O, deadlines, admission
        |
        | bounded admission permit
        v
CPU submission boundary
        |
        +---- tokio_rayon::spawn(...) ------------------+
        |                                                |
        +---- custom Rayon ThreadPool::spawn_async(...) -+
        |                                                |
        +---- oneshot + ThreadPool::spawn ---------------+
                                                         v
                                                   Rayon pool
                                              split / steal / reduce
                                                         |
                                     +-------------------+------------------+
                                     |                                      |
                              immutable/local data                  shared state if needed
                                     |                                      |
                               reduce / merge                         DashMap / atomics
                                     |                                      |
                                     +-------------------+------------------+
                                                         |
                                                         v
                                                     result
                                                         |
                                                         v
                                                   Tokio task
                                          async write / response / DB
```

## 0.4 Minimum vocabulary

| Term | Meaning in this reference |
|---|---|
| concurrency | multiple units of work are in progress, not necessarily simultaneously executing |
| parallelism | multiple units of work are executing at the same time on different hardware threads/cores |
| async task | cooperatively polled future scheduled by an async runtime |
| OS thread | kernel-scheduled execution thread; can run one CPU instruction stream at a time |
| worker pool | a bounded set of threads that accept many logical jobs |
| work stealing | idle workers take work from other workers' queues to improve utilization |
| backpressure | deliberately slowing producers when downstream capacity is saturated |
| admission control | cap applied before expensive work is accepted/executed |
| task granularity | amount of useful work per scheduled job |
| oversubscription | more runnable CPU threads than useful hardware execution capacity |
| guard | object whose lifetime holds a lock, borrow, epoch pin, or other access right |
| structured concurrency | child work is owned/joined by a scope/lifetime rather than silently detached |

## 0.5 Core LLM-agent invariants

**Invariant 1 — Async is not synonymous with parallel.** `tokio::select!` runs branch futures concurrently on the current task; parallel execution requires spawned tasks or another executor. ([Tokio select][T_SELECT])

**Invariant 2 — CPU-heavy loops do not belong directly in Tokio futures.** A future that computes for a long time without yielding can delay unrelated async work. ([Tokio blocking guidance][T_BLOCK])

**Invariant 3 — `spawn_blocking` is not an unlimited CPU executor.** Tokio gives the blocking pool a large upper thread limit because it must support blocking I/O. Many CPU jobs should be explicitly bounded or sent to Rayon. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

**Invariant 4 — Rayon assumes CPU-style work.** `join` documentation warns that blocking I/O or cross-task blocking waits can produce poor performance or deadlock. ([Rayon join][R_JOIN])

**Invariant 5 — shared mutable state is a cost center.** Prefer ownership, per-task/per-worker state, then reduce/merge. Reach for DashMap only when concurrent keyed mutation is genuinely part of the problem.

**Invariant 6 — a DashMap reference is a lock guard.** Holding `Ref`/`RefMut` while calling other map methods can deadlock; the crate documents locking warnings on many methods. ([DashMap API][D_MAP])

**Invariant 7 — thread budgets are process-wide.** Tokio workers, Rayon workers, Tokio blocking threads, database/native library pools, and manually spawned threads all compete for the same cores.

## 0.6 Initial anti-pattern inventory

* Wrapping a CPU-heavy loop in `async fn` and assuming it became non-blocking.
* Creating one Tokio task per CPU element when a Rayon parallel iterator would be cheaper and more appropriate.
* Calling blocking I/O from a Rayon worker.
* Calling `spawn_blocking` thousands of times for CPU work without a cap.
* Creating multiple default-sized Rayon pools that each assume they own all logical CPUs.
* Holding a DashMap `RefMut` across `.await`.
* Holding a DashMap guard while invoking another map method.
* Using an unbounded Crossbeam channel for an ingestion path whose producer can outrun its consumer indefinitely.
* Building a custom Crossbeam scheduler when Rayon already matches the workload.
* Treating tokio-rayon as admission control; it only provides the bridge.

---

# Rust Parallelism Stack Advanced — 1) Installation, dependency selection, features, and project layout

## 1.0 Canonical application manifest

A broad service manifest:

```toml
[dependencies]
tokio = { version = "=1.53.1", features = [
  "rt-multi-thread",
  "macros",
  "sync",
  "time",
  "net",
  "io-util",
  "fs",
  "process",
  "signal",
] }
rayon = "=1.12.0"
crossbeam = "=0.8.4"
dashmap = { version = "=6.2.1", features = ["rayon"] }
tokio-rayon = "=2.1.0"
```

For a smaller binary, enable only the Tokio features you actually use. Tokio has no default features; `full` activates most stable runtime/application functionality, while targeted feature selection reduces compile surface. ([Tokio manifest][T_FEATURES])

## 1.1 `full` vs explicit Tokio features

```toml
# Convenience / application prototype
tokio = { version = "=1.53.1", features = ["full"] }
```

```toml
# Production-minimized example
tokio = { version = "=1.53.1", features = [
  "rt-multi-thread", "macros", "sync", "time", "net", "io-util"
] }
```

Important stable features include:

| Feature | Enables |
|---|---|
| `rt` | core runtime/task execution |
| `rt-multi-thread` | multithread scheduler; implies `rt` |
| `macros` | `#[tokio::main]`, `#[tokio::test]`, `select!`, `join!` macro support where applicable |
| `sync` | async channels and synchronization primitives |
| `time` | sleeps, intervals, timeouts, runtime timer |
| `net` | TCP/UDP/Unix and associated OS I/O integration |
| `io-util` | async I/O extension traits and `bytes` dependency |
| `fs` | async filesystem facade |
| `process` | async process APIs |
| `signal` | async signal APIs |
| `test-util` | paused-time/testing helpers |
| `parking_lot` | optional internal synchronization implementation |

Tokio's `full` feature currently expands to the major application-facing features listed in its manifest. ([Tokio manifest][T_FEATURES])

## 1.2 Rayon feature posture

Rayon 1.12.0 has a deliberately small feature surface. The only crate feature currently listed is `web_spin_lock`, intended for `wasm32-unknown-unknown` browser constraints. Normal native CPU-parallel applications need no Rayon features. ([Rayon features][R_FEATURES])

```toml
rayon = "=1.12.0"
```

## 1.3 Crossbeam facade vs direct subcrates

Facade:

```toml
crossbeam = "=0.8.4"
```

Direct subcrates when you want narrower compile/API ownership:

```toml
crossbeam-channel = "=0.5.16"
crossbeam-deque = "=0.8.7"
crossbeam-epoch = "=0.9.20"
crossbeam-queue = "=0.3.13"
crossbeam-utils = "=0.8.22"
```

The facade re-exports the major tools. The direct-subcrate route is useful when:

* a library wants minimal dependencies;
* `no_std`/`alloc` boundaries matter;
* you want independent subcrate version control;
* the public API exposes a subcrate's types;
* you are implementing low-level infrastructure and want the dependency intent to be explicit.

The `crossbeam` facade defaults to `std`; its `alloc` and individual subcrate features expose narrower subsets. ([Crossbeam features][C_FEATURES])

## 1.4 DashMap feature posture

DashMap 6.2.1 exposes these optional features: `arbitrary`, `inline`, `raw-api`, `rayon`, `serde`, and `typesize`; none are enabled by default. ([DashMap features][D_FEATURES])

Typical application:

```toml
dashmap = "=6.2.1"
```

Enable Rayon-backed parallel iteration only if used:

```toml
dashmap = { version = "=6.2.1", features = ["rayon"] }
```

Avoid `raw-api` unless implementing a specialized low-level path: it exposes shards/raw-table internals and expands the amount of invariants your code must preserve.

## 1.5 tokio-rayon dependency posture

```toml
tokio-rayon = "=2.1.0"
```

It has a tiny API and no meaningful application feature matrix. Its manifest depends on Tokio with only `sync` and on Rayon via a compatible 1.x range. Your application still needs its own Tokio runtime features and normally a direct Rayon dependency when configuring pools. ([tokio-rayon manifest][TR_MANIFEST])

## 1.6 Workspace dependency policy

```toml
[workspace]
resolver = "2"
members = ["crates/runtime", "crates/compute", "crates/app"]

[workspace.dependencies]
tokio = { version = "=1.53.1", features = ["rt-multi-thread", "macros", "sync", "time"] }
rayon = "=1.12.0"
crossbeam = "=0.8.4"
dashmap = "=6.2.1"
tokio-rayon = "=2.1.0"
```

Agent rule: define executor versions once at workspace root. Threading/runtime dependency drift is especially undesirable because behavior is system-wide, not local to one leaf module.

## 1.7 Recommended project layout

```text
my-app/
  Cargo.toml
  Cargo.lock
  crates/
    runtime/
      src/
        lib.rs
        tokio_runtime.rs      # Runtime Builder / shutdown
        admission.rs          # semaphores / request limits
        cancellation.rs
    compute/
      src/
        lib.rs
        pool.rs               # Rayon ThreadPool construction
        jobs.rs               # CPU job functions
        bridge.rs             # Tokio <-> Rayon handoff
        local_state.rs        # per-worker scratch/reduce patterns
    concurrency/
      src/
        channels.rs           # Crossbeam pipeline primitives
        shared_maps.rs        # DashMap wrappers, guard-safe APIs
        queues.rs
    app/
      src/main.rs             # transport/CLI only
  tests/
    integration/
    concurrency/
    performance/
```

Keep **CPU kernels synchronous** where possible. An `async fn` should generally orchestrate asynchronous waits, not serve as the home of a CPU kernel.

## 1.8 Build and inspection commands

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo tree -d
cargo tree -i tokio
cargo tree -i rayon
cargo tree -i crossbeam-utils
cargo tree -i dashmap
cargo build --release
```

For a machine-specific performance binary:

```bash
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

Do not ship a `target-cpu=native` binary to heterogeneous CPUs unless deployment compatibility is controlled.

## 1.9 Installation anti-patterns

* `tokio = { features = ["full"] }` in a low-level library without justification.
* Depending on `crossbeam` plus every direct subcrate redundantly without a reason.
* Enabling DashMap `raw-api` because it “sounds faster.”
* Configuring global Rayon in multiple crates with competing assumptions.
* Building a library that implicitly initializes global runtime/thread-pool policy.
* Benchmarking debug builds.
* Leaving `Cargo.lock` uncommitted for deployable binaries.

## 1.10 Installation checklist

```text
[ ] Pin or deliberately semver-range Tokio, Rayon, Crossbeam, DashMap, tokio-rayon.
[ ] Commit Cargo.lock for binaries/services.
[ ] Choose explicit Tokio features.
[ ] Enable DashMap rayon only if parallel map iteration is used.
[ ] Keep raw-api off unless low-level shard access is required.
[ ] Decide Crossbeam facade vs direct subcrate imports.
[ ] Centralize pool/runtime construction in dedicated modules.
[ ] Keep CPU kernels synchronous.
[ ] Run cargo tree -d and inspect duplicate executor/concurrency dependencies.
[ ] Performance-test with --release.
```

---

# Rust Parallelism Stack Advanced — 2) First executable programs

## 2.0 Objective

Prove the five essential paths:

```text
Tokio async concurrency
Rayon CPU parallelism
Crossbeam synchronous backpressure
DashMap concurrent keyed state
tokio-rayon async -> CPU bridge
```

## 2.1 App 1 — Tokio concurrent tasks

```rust
use std::time::Duration;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let a = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        10
    });

    let b = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        20
    });

    let (a, b) = tokio::join!(a, b);
    assert_eq!(a.unwrap() + b.unwrap(), 30);
}
```

`tokio::spawn` schedules an async task immediately and returns a `JoinHandle`. Dropping the handle detaches rather than cancels the task. ([Tokio JoinHandle][T_JOIN])

## 2.2 App 2 — Rayon parallel map/reduce

```rust
use rayon::prelude::*;

fn main() {
    let sum: u64 = (0_u64..1_000_000)
        .into_par_iter()
        .map(|x| x * x)
        .sum();

    println!("{sum}");
}
```

The common Rayon transformation is `iter()` -> `par_iter()`, `iter_mut()` -> `par_iter_mut()`, or `into_iter()` -> `into_par_iter()`. ([Rayon iterator docs][R_ITER])

## 2.3 App 3 — Crossbeam bounded MPMC channel

```rust
use crossbeam::channel::bounded;
use std::thread;

fn main() {
    let (tx, rx) = bounded::<u64>(128);

    let producer = thread::spawn(move || {
        for i in 0..10_000 {
            tx.send(i).unwrap(); // blocks when queue reaches 128
        }
    });

    let consumer = thread::spawn(move || {
        let mut sum = 0_u64;
        while let Ok(v) = rx.recv() {
            sum += v;
        }
        sum
    });

    producer.join().unwrap();
    let sum = consumer.join().unwrap();
    println!("{sum}");
}
```

A bounded Crossbeam channel limits queued messages; sending blocks when capacity is full. Capacity zero creates a rendezvous channel where send/receive synchronize directly. ([Crossbeam channel][C_CHANNEL])

## 2.4 App 4 — DashMap concurrent accumulation

```rust
use dashmap::DashMap;
use rayon::prelude::*;
use std::sync::Arc;

fn main() {
    let counts = Arc::new(DashMap::<u32, usize>::new());

    (0_u32..100_000).into_par_iter().for_each({
        let counts = Arc::clone(&counts);
        move |x| {
            let key = x % 100;
            *counts.entry(key).or_insert(0) += 1;
        }
    });

    assert_eq!(counts.len(), 100);
}
```

This is safe but is **not automatically optimal**. For hot keys, local per-worker counts followed by a merge can greatly reduce contention.

## 2.5 App 5 — Tokio to Rayon with tokio-rayon

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let answer = tokio_rayon::spawn(|| {
        (0_u64..2_000_000).map(|x| x.wrapping_mul(x)).sum::<u64>()
    })
    .await;

    println!("{answer}");
}
```

`tokio_rayon::spawn` submits to the **global Rayon pool** with LIFO priority and returns `AsyncRayonHandle<R>`, which is awaitable. ([tokio-rayon spawn][TR_SPAWN])

## 2.6 App 6 — custom Rayon pool awaited from Tokio

```rust
use rayon::ThreadPoolBuilder;
use tokio_rayon::AsyncThreadPool;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let pool = ThreadPoolBuilder::new()
        .num_threads(8)
        .thread_name(|i| format!("cpu-{i}"))
        .build()
        .unwrap();

    let result = pool
        .spawn_async(|| {
            (0_u64..1_000_000).map(|x| x ^ (x >> 3)).sum::<u64>()
        })
        .await;

    println!("{result}");
}
```

Use a custom pool when the application needs an explicit CPU budget, dedicated thread names, stack sizes, lifecycle, or workload isolation. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER]) ([tokio-rayon AsyncThreadPool][TR_POOL])

## 2.7 First-app decision table

| If the code does... | Start with... |
|---|---|
| awaits sockets/timers/files | Tokio |
| transforms a large in-memory collection | Rayon |
| needs synchronous MPMC work queue | Crossbeam channel |
| implements a scheduler | Crossbeam deque |
| needs shared concurrent map | DashMap |
| async handler needs CPU result | tokio-rayon or custom Rayon bridge |

## 2.8 First-app anti-patterns

* Replacing App 2 with 1,000,000 `tokio::spawn` calls.
* Replacing App 1's sleeps with expensive CPU loops.
* Using an unbounded channel where a bounded channel expresses the real capacity.
* Assuming DashMap accumulation beats thread-local aggregation.
* Creating a new Rayon pool for every request.
* Holding a DashMap guard across the asynchronous return path.

---

# Rust Parallelism Stack Advanced — 3) Concurrency, parallelism, asynchrony, and thread-pool vocabulary

## 3.0 Three independent axes

Do not collapse these concepts:

```text
Concurrency:  A is waiting, B advances, then A advances.
Parallelism:  A and B execute simultaneously on separate CPU execution contexts.
Asynchrony:   code represents suspension explicitly as a Future / awaitable operation.
```

A single-thread Tokio runtime can be highly concurrent but not CPU-parallel. A Rayon computation is highly parallel but ordinarily synchronous to its caller. A Crossbeam channel may coordinate parallel OS threads without any async Rust.

## 3.1 Async task vs OS thread

| Property | Tokio task | OS thread / Rayon worker |
|---|---|---|
| scheduling | cooperative future polling by runtime | preemptive kernel thread scheduling |
| typical count | thousands+ possible | usually O(core count) for CPU pools |
| stack | state-machine storage, no dedicated native stack per task | native stack per thread |
| blocking effect | can stall runtime worker | stalls that OS thread/pool worker |
| migration | Tokio task may be polled by different worker threads when `Send` | thread itself is execution context |
| cancellation | async task can be aborted at yield/await boundaries | arbitrary native computation generally cannot be forcibly cancelled safely |

## 3.2 `Send` and `Sync` operational meaning

Agent shorthand:

```text
T: Send   -> ownership of T may cross a thread boundary safely.
T: Sync   -> &T may be shared across threads safely.
```

`tokio::spawn` on a multithread runtime normally needs a `Send + 'static` future because the task may move between runtime workers. Rayon closures and items similarly require `Send`/`Sync` constraints appropriate to cross-thread execution.

## 3.3 Cooperative vs preemptive scheduling

Tokio cannot preempt a task in the middle of ordinary Rust computation. The task returns control when its future returns `Poll::Pending`, reaches an `.await` that is not ready, or explicitly yields. Therefore a long CPU loop inside an async task can monopolize a runtime worker.

Rayon workers are OS threads. A CPU job may be preempted by the OS, but from Rayon's perspective that worker is occupied until the closure returns or participates in Rayon's own scheduling mechanisms.

## 3.4 Work stealing in Tokio vs Rayon

Both use work-stealing concepts, but they solve different problems:

| Tokio | Rayon |
|---|---|
| schedules asynchronous tasks | schedules CPU jobs / iterator partitions |
| I/O reactor wakes tasks | recursive/data-parallel splitting creates work |
| responsiveness/fairness to ready futures matters | utilization/locality/grain size matters |
| worker count is runtime configuration | worker count is CPU pool configuration |
| blocking pool is separate from core workers | one pool is expected to do compute, not I/O waits |

## 3.5 Structured vs detached work

Prefer forms where lifetime is explicit:

* `rayon::join` — both closures complete before return.
* `rayon::scope` — all scoped jobs complete before scope exits.
* Tokio: retain and await `JoinHandle`s, use `JoinSet` for dynamic groups, or maintain explicit ownership/shutdown protocol.
* Crossbeam: own worker threads and join them during shutdown.

Detached work is appropriate only when its lifetime is truly process/service scoped and errors are observed elsewhere.

## 3.6 Grain size

Every scheduling system has overhead. If a unit of CPU work takes tens of nanoseconds, scheduling it individually is usually absurd; batch it. Rayon parallel iterators automatically split work, but even they benefit from chunking or minimum-length controls when per-item work is tiny.

Think in terms of:

```text
parallel benefit ~= useful CPU time
                    - splitting/scheduling overhead
                    - synchronization/merge overhead
                    - cache/memory bandwidth interference
```

## 3.7 Vocabulary agent checklist

```text
[ ] State whether the workload is I/O-bound, CPU-bound, or mixed.
[ ] State whether concurrency or simultaneous CPU execution is required.
[ ] Identify every OS-thread pool in the process.
[ ] Identify whether work is structured/joined or detached.
[ ] Estimate job grain size.
[ ] Identify shared mutable state and guard lifetimes.
[ ] Identify backpressure boundary.
```

---

# Rust Parallelism Stack Advanced — 4) Work-classification and executor-selection decision procedure

## 4.0 Decision tree

```text
Does the work primarily wait on an external resource?
  yes -> Tokio async API if available
         |
         +-- only blocking API exists? -> spawn_blocking or dedicated blocking thread/pool

Does it primarily burn CPU on in-memory data?
  yes -> Rayon
         |
         +-- one/few occasional short blocking calls? evaluate carefully
         +-- large blocking I/O waits? move those outside Rayon

Does it need a synchronous producer/consumer pipeline?
  yes -> Crossbeam bounded channel / queues

Does it need a custom work-stealing scheduler?
  yes -> Crossbeam deque (only if Rayon does not satisfy semantics)

Does it require concurrent key-based shared state?
  yes -> first consider local state + merge
         then DashMap if shared online access is required

Does async code need the CPU result?
  yes -> tokio-rayon / custom Rayon oneshot bridge
```

## 4.1 Workload matrix

| Work | Tokio | Rayon | Crossbeam | DashMap |
|---|---:|---:|---:|---:|
| network requests | **yes** | no | sometimes sync handoff | cache only |
| filesystem metadata fan-out | yes | maybe CPU post-process | maybe | maybe |
| parsing ASTs | orchestration | **yes** | pipeline option | symbol cache option |
| hashing/compression | orchestration | **yes** | queue option | rarely |
| database client async calls | **yes** | no | no | cache option |
| numeric kernels | no | **yes** | no | no |
| actor mailbox | Tokio mpsc | no | sync actor only | no |
| custom scheduler | no need usually | maybe sufficient | **deque** | no |
| hot concurrent lookup table | maybe wrapper | maybe population | no | **yes** |
| one-shot request/reply | Tokio oneshot | no | Crossbeam channel in sync code | no |

## 4.2 `spawn_blocking` vs Rayon

Use `spawn_blocking` when:

* an async path must call a synchronous blocking API;
* the operation is not easily expressed as data-parallel work;
* blocking duration is finite;
* CPU concurrency is explicitly bounded if many such jobs may run.

Prefer Rayon when:

* the operation is predominantly CPU compute;
* many jobs can exist simultaneously;
* you want a fixed CPU pool and work stealing;
* nested parallelism or parallel iterators are valuable.

Tokio explicitly notes that its blocking-thread cap is very large by default and recommends semaphore limiting or specialized CPU executors for many CPU-bound computations. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 4.3 Crossbeam vs Tokio channels

| Need | Prefer |
|---|---|
| async sender/receiver that should await capacity | Tokio `mpsc` |
| sync OS threads that should block on capacity | Crossbeam channel |
| MPMC receiver cloning | Crossbeam channel |
| async MPSC application architecture | Tokio `mpsc` |
| zero-capacity rendezvous | Crossbeam `bounded(0)` |
| async broadcast to all subscribers | Tokio `broadcast` |
| latest-value config/state | Tokio `watch` |

## 4.4 DashMap vs `Mutex<HashMap>`

Prefer a single `Mutex<HashMap>` when:

* contention is low;
* critical sections are tiny;
* simplicity matters more than parallel access;
* operations need multi-key atomicity under one lock.

Prefer DashMap when:

* keys distribute across shards;
* multiple independent keys are frequently accessed concurrently;
* online mutation is required;
* guard lifetimes can be kept short and synchronous.

Prefer local maps + merge when:

* writes are generated by batch parallel computation;
* results do not need to be visible immediately across workers.

## 4.5 Decision anti-patterns

* Executor selection based on API aesthetics rather than workload.
* “Tokio has threads, therefore use Tokio for all parallelism.”
* “DashMap is concurrent, therefore it is faster than Mutex.”
* “Lock-free” as an automatic reason to use Crossbeam epoch.
* Moving a blocking wait from Tokio into Rayon and calling the problem solved.
* Selecting unbounded queues because capacity is difficult to estimate.

---

# Rust Parallelism Stack Advanced — 5) Whole-process thread topology and oversubscription budget

## 5.0 Thread-budget mental model

The machine sees threads, not crate names:

```text
logical CPUs = N

runnable CPU demand may include:
  Tokio core workers
+ Rayon workers
+ Tokio spawn_blocking threads doing CPU work
+ manual std::thread workers
+ database / compression / BLAS / ML library threads
+ GC/runtime threads from embedded foreign runtimes
+ kernel / system workload
```

A useful heuristic is to keep **steady-state CPU-bound runnable workers near the amount of CPU capacity you intentionally allocate to the process**, while allowing I/O-oriented threads to exist because they are frequently sleeping. This is a heuristic, not a hard formula.

## 5.1 Default pool interactions

* Tokio multithread runtime defaults worker count to available cores. ([Tokio Builder][T_BUILDER])
* Rayon currently defaults to `RAYON_NUM_THREADS` if set, otherwise logical CPU count; docs warn the default strategy may change and recommend setting `num_threads` if you require a fixed count. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER])
* Tokio's blocking pool can grow far beyond core count unless limited. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

Therefore simply combining default Tokio + default Rayon does **not** mean twice the cores are continuously consumed: Tokio workers are often waiting/polling lightweight futures. But if Tokio workers themselves run CPU-heavy work while Rayon is saturated, oversubscription becomes real.

## 5.2 Example topology: 32 logical CPUs

Latency-sensitive service with substantial CPU analysis:

```text
Tokio async workers:          8-16     # enough I/O scheduling capacity
Rayon CPU pool:              24-30     # main CPU budget
Tokio blocking CPU jobs:      0        # route CPU work to Rayon
Tokio blocking I/O:       bounded      # synchronous libraries only
other native CPU pools:       0-4      # account explicitly
```

There is no universal best split. Benchmark the actual mix. If the service is almost entirely async I/O with occasional CPU bursts, Tokio may reasonably use default workers while Rayon still owns a bounded CPU pool. If the process is a batch compute executable, Tokio can often use only a small runtime or even current-thread runtime while Rayon owns nearly all cores.

## 5.3 Two-level admission control

For a public service:

```text
request admission semaphore
        |
        v
small async orchestration
        |
        v
CPU-job semaphore / bounded submission queue
        |
        v
fixed Rayon pool
```

The Rayon pool size bounds **executing CPU workers**. An upstream semaphore/queue bounds **waiting jobs and memory retained by queued inputs**. You often need both.

## 5.4 Avoid nested native pool multiplication

Danger pattern:

```text
Rayon: 32 workers
  each calls library that starts 32 BLAS threads
=> potential 1024 runnable threads
```

Mitigations:

* configure the nested library to one thread when outer Rayon owns parallelism;
* or disable outer parallelism for that kernel and allow the optimized library to own the cores;
* separate workload classes into distinct pools if they have incompatible thread policies.

## 5.5 CPU affinity and NUMA

Neither Tokio nor Rayon automatically guarantees the affinity topology your application might need for NUMA-sensitive workloads. Only introduce explicit pinning / NUMA-aware allocation after profiling demonstrates cross-socket memory traffic or scheduling migration is material. Overly rigid affinity can reduce load balancing.

## 5.6 Thread names

Name custom threads/pools:

```rust
let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(16)
    .thread_name(|i| format!("analysis-cpu-{i}"))
    .build()?;
```

Tokio's `runtime::Builder` likewise exposes thread naming/customization methods. Names make flamegraphs, crash dumps, and system profilers much more interpretable.

## 5.7 Oversubscription checklist

```text
[ ] Count logical CPUs visible inside container/cgroup, not host assumption.
[ ] List every thread pool.
[ ] Separate usually-runnable CPU workers from mostly-blocked I/O threads.
[ ] Configure Rayon pool size explicitly when resource policy matters.
[ ] Bound CPU job queue/admission separately from worker count.
[ ] Limit spawn_blocking CPU use.
[ ] Inspect native libraries for their own threading.
[ ] Benchmark p50/p95/p99 latency and throughput under saturation.
[ ] Observe context switches, run queue, CPU utilization, and memory bandwidth.
[ ] Re-test inside deployment container/cgroup, not only developer workstation.
```

---

# Rust Parallelism Stack Advanced — 6) Tokio identity and runtime architecture

## 6.0 Tokio mental model

Tokio is an **event-driven, non-blocking I/O runtime**. Its application-facing architecture can be modeled as:

```text
Future / async task
      |
      v
Tokio scheduler
  +-- local worker queues
  +-- global/injection queue
  +-- work stealing between workers
      |
      +---------------------------+
      |                           |
      v                           v
poll ready futures          reactor / drivers
                            I/O readiness
                            timers
                            signals
      ^                           |
      +---------------------------+
            wake task
```

The multithreaded scheduler uses a work-stealing design; the runtime also drives OS event facilities and asynchronous sockets. ([Tokio crate][T0])

## 6.1 Core thread classes

Do not treat “Tokio threads” as one pool:

```text
core runtime workers
  - poll async tasks
  - should not block
  - should not run long non-yielding CPU kernels

blocking threads
  - used by spawn_blocking and some Tokio facilities built on blocking syscalls
  - may grow up to max_blocking_threads
  - not a fixed CPU pool by default
```

This distinction is central to mixed async/CPU systems.

## 6.2 Current-thread vs multithread runtime

| Runtime | Use |
|---|---|
| `Builder::new_current_thread()` | small CLI orchestration, deterministic/simple embedding, runtime per dedicated OS thread, no need for async task parallelism |
| `Builder::new_multi_thread()` | normal network service, many tasks, async task execution across runtime workers |

`worker_threads` affects only the multithread runtime. ([Tokio Builder][T_BUILDER])

## 6.3 Tasks are scheduled units, not threads

```rust
let handle = tokio::spawn(async move {
    do_async_work().await
});
```

A task starts being eligible for execution immediately. Awaiting the handle is for result/lifetime observation; it is not what starts the task. ([Tokio JoinHandle][T_JOIN])

## 6.4 Reactor-friendly workload shape

Good async task:

```rust
async fn fetch_transform_write() -> anyhow::Result<()> {
    let bytes = fetch().await?;       // waits without owning a core
    let small = cheap_transform(bytes); // short CPU step
    write(small).await?;              // waits without owning a core
    Ok(())
}
```

Bad async task:

```rust
async fn compute_forever(input: &[u8]) -> u64 {
    // No .await for a very long period: monopolizes this runtime worker.
    expensive_cpu_kernel(input)
}
```

Route the second form to Rayon or a deliberately bounded blocking executor.

## 6.5 Runtime responsibility split

| Tokio supplies | Application still decides |
|---|---|
| task scheduler | request/job admission |
| async I/O drivers | CPU executor policy |
| async channels | queue capacities and overload policy |
| async synchronization | ownership model and lock granularity |
| timers/timeouts | timeout values and semantic recovery |
| task abort handle | cooperative cleanup and business-level cancellation |
| blocking pool | whether work belongs there at all |

## 6.6 Agent rules

```text
Tokio task != OS thread.
Tokio multithread runtime != CPU batch engine.
Runtime worker count != max blocking thread count.
Awaiting JoinHandle != starting task.
No long CPU loop in async task without deliberate offload/yield policy.
```

---

# Rust Parallelism Stack Advanced — 7) Tokio runtime construction, feature flags, and builders

## 7.0 Macro default

For normal binaries:

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run().await
}
```

`#[tokio::main]` is ergonomic, but runtime policy is less visible. Long-lived services benefit from an explicit `runtime::Builder`.

## 7.1 Explicit multithread runtime

```rust
use tokio::runtime::Builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Builder::new_multi_thread()
        .worker_threads(8)
        .thread_name_fn(|| {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            format!("async-{id}")
        })
        .enable_all()
        .build()?;

    rt.block_on(async { run().await })
}
```

Builder methods are version-sensitive; verify exact signatures against the pinned Tokio docs before copying advanced runtime-tuning code. ([Tokio Builder][T_BUILDER])

## 7.1a Current-thread runtime

A current-thread runtime executes spawned async tasks on the thread driving `block_on` rather than maintaining a multithread worker scheduler:

```rust
let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

rt.block_on(async {
    run().await
});
```

Use for deterministic/single-threaded embeddings, lightweight CLIs, tests, or environments where task migration is undesirable. It provides async concurrency but **not CPU parallelism**; CPU-heavy work should still be offloaded.

## 7.1b `LocalSet` / `spawn_local` for `!Send` futures

`tokio::spawn` requires spawned futures to be `Send` because a multithread runtime may move them between worker threads. `LocalSet` schedules `!Send` futures on one thread, and `spawn_local` is the local-task equivalent. ([Tokio LocalSet][T_LOCAL])

```rust
use std::rc::Rc;
use tokio::task::{self, LocalSet};

let local = LocalSet::new();
local.run_until(async {
    let value = Rc::new(String::from("local"));
    task::spawn_local(async move {
        println!("{value}");
    }).await.unwrap();
}).await;
```

Important boundaries:

```text
LocalSet task -> guaranteed same local thread
`tokio::spawn` from inside LocalSet -> normal runtime task, not automatically local
a LocalSet itself is !Send / !Sync
```

Use local tasks for genuinely thread-affine / `!Send` resources; do not use them as a workaround for accidental `Send` design problems in ordinary service state.

## 7.2 `worker_threads`

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(8)
    .enable_all()
    .build()?;
```

Current default: available core count. Tokio advises keeping worker count relatively small; these are core async scheduler threads, not a general “more threads = faster” knob. ([Tokio Builder][T_BUILDER])

## 7.3 `max_blocking_threads`

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(8)
    .max_blocking_threads(64)
    .enable_all()
    .build()?;
```

This caps **additional blocking threads**, not core workers. Blocking threads are spawned on demand and may exit after being idle. The default limit is intentionally large because blocking APIs can include filesystem/DNS/std I/O. ([Tokio Builder][T_BUILDER])

Agent rule: lowering the limit is not a substitute for CPU admission control. A saturated blocking queue can still retain huge inputs and create unacceptable latency.

## 7.4 Scheduler polling knobs

Advanced knobs include `global_queue_interval` and `event_interval`:

* smaller global-queue interval: new/injected work starts sooner, more synchronization overhead;
* larger global-queue interval: favor local work, potentially greater latency for global arrivals;
* smaller event interval: poll I/O/timers more frequently, potentially more overhead;
* larger event interval: favor task polling, potentially worse external-event latency.

Tokio documents an event interval default of 61 scheduler ticks; global-queue behavior differs by scheduler. Treat these as benchmark-only knobs. ([Tokio Builder][T_BUILDER])

## 7.5 Enabling drivers

```rust
Builder::new_multi_thread()
    .enable_io()
    .enable_time()
    .build()?;
```

or:

```rust
Builder::new_multi_thread()
    .enable_all()
    .build()?;
```

A custom runtime used purely as a task executor may deliberately omit some drivers, but most application runtimes should enable required facilities explicitly.

## 7.6 Multiple runtimes

Multiple Tokio runtimes can be appropriate when:

* embedding a runtime into a subsystem with lifecycle isolation;
* separating latency-sensitive I/O from a very unusual task class;
* interacting with a foreign threading environment.

But do not create runtimes casually:

* every runtime owns scheduler infrastructure;
* moving I/O resources between runtimes has rules and can be error-prone;
* many runtimes complicate shutdown, metrics, and thread budgets;
* CPU work is usually better isolated with Rayon than with a second Tokio runtime.

Tokio's own guidance says a separate runtime can be used for CPU work but cautions that I/O-bound tasks on such a CPU-dedicated runtime would behave poorly; Rayon is the simpler CPU executor choice. ([Tokio crate task guidance][T_TASK])

## 7.7 Runtime lifecycle

Production ownership:

```text
main
  create config
  create Tokio runtime
  create Rayon pool(s)
  create application state
  run service root future
  initiate shutdown
  stop accepting work
  drain/cancel async tasks
  stop CPU submissions
  wait for CPU jobs according to policy
  drop pools/runtime
```

## 7.8 Builder anti-patterns

* Setting `worker_threads(128)` on a 16-core machine to “increase parallelism.”
* Using `max_blocking_threads` as the primary CPU-parallelism setting.
* Tweaking event/global queue intervals before profiling.
* Creating a runtime per request.
* Running nested `Runtime::block_on` from arbitrary async code.
* Enabling every unstable Tokio knob in production without a pinned migration plan.

## 7.9 Runtime builder checklist

```text
[ ] Choose current-thread vs multi-thread deliberately.
[ ] Set worker_threads only if default does not match service policy.
[ ] Name threads.
[ ] Enable required I/O/time facilities.
[ ] Treat max_blocking_threads separately from CPU pool sizing.
[ ] Create runtime once at process/subsystem boundary.
[ ] Define explicit shutdown ownership.
[ ] Benchmark before changing scheduler polling knobs.
```

---

# Rust Parallelism Stack Advanced — 8) Tokio tasks, `spawn`, `JoinHandle`, and task lifecycle

## 8.0 Task lifecycle

```text
construct future
    |
tokio::spawn(future)
    |
    +--> task scheduled immediately
    |
JoinHandle<T>
    |
    +--> await -> Result<T, JoinError>
    +--> abort -> request async cancellation
    +--> drop  -> detach; task continues
```

Tokio documents `JoinHandle` as owned permission to await task termination; dropping the handle detaches the task rather than stopping it. ([Tokio JoinHandle][T_JOIN])

## 8.1 Spawn with result

```rust
let handle = tokio::spawn(async move {
    40 + 2
});

let value = handle.await?;
assert_eq!(value, 42);
```

When using `?`, remember there are usually two result layers:

```rust
let handle = tokio::spawn(async move {
    compute_async().await // returns Result<u64, MyError>
});

let value: u64 = handle.await??;
```

First `?`: task join/panic/cancel result. Second `?`: application's task result.

## 8.2 `JoinSet` for dynamic task groups

For a runtime-determined number of homogeneous tasks, `tokio::task::JoinSet` is usually cleaner than a `Vec<JoinHandle<_>>` because it supports joining completed tasks as they finish.

Conceptual pattern:

```rust
let mut set = tokio::task::JoinSet::new();

for item in items {
    set.spawn(async move { process(item).await });
}

while let Some(result) = set.join_next().await {
    result??;
}
```

Agent rule: do not spawn an unbounded task set from untrusted input. Pair with a semaphore, bounded stream combinator, or worker architecture.

## 8.3 Detachment semantics

Bad accidental detach:

```rust
for req in requests {
    tokio::spawn(handle(req)); // handle discarded
}
```

Now:

* errors are unobserved unless logged inside task;
* shutdown does not know what remains;
* the caller has no result/cancellation handle;
* task count may grow without bound.

If detachment is intentional, name the architecture: background daemon, supervisor-owned task, actor, etc.

## 8.4 Abort behavior

```rust
let handle = tokio::spawn(async {
    long_async_work().await
});

handle.abort();
let err = handle.await.unwrap_err();
assert!(err.is_cancelled());
```

`JoinHandle::abort` requests cancellation of async tasks. `spawn_blocking` work is different: once running, it cannot be forcibly aborted by `JoinHandle::abort`. ([Tokio JoinHandle][T_JOIN])

## 8.5 Panic behavior

A panic in a spawned Tokio task is captured in the join result rather than automatically panicking the awaiting task:

```rust
let handle = tokio::spawn(async { panic!("boom") });
let err = handle.await.unwrap_err();
assert!(err.is_panic());
```

Choose an application policy:

* expected recoverable errors -> `Result`;
* invariant violation -> panic, but supervisor must observe it;
* critical service child panic -> trigger controlled shutdown/restart if appropriate.

## 8.6 Task ownership pattern

```rust
struct BackgroundTasks {
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl BackgroundTasks {
    async fn shutdown(self) {
        for h in &self.handles {
            h.abort();
        }
        for h in self.handles {
            let _ = h.await;
        }
    }
}
```

This is deliberately simple; complex services normally need cooperative cancellation so tasks can flush state rather than be aborted immediately.

## 8.7 Task anti-patterns

* Fire-and-forget without an owner.
* Ignoring `JoinError`.
* Spawning one task for work that finishes faster than scheduling overhead.
* Spawning CPU kernels on Tokio.
* Assuming abort interrupts blocking/native code.
* Capturing large request bodies in queued tasks with no admission cap.

---

# Rust Parallelism Stack Advanced — 9) Tokio cooperative scheduling, yielding, fairness, and task granularity

## 9.0 Cooperative-scheduling invariant

A Tokio worker can only run other tasks when the current task returns control to the runtime. Long synchronous computation between await points delays other ready tasks.

```text
poll Task A
  executes Rust code...
  executes Rust code...
  executes Rust code...
  reaches pending await -> runtime regains control
poll Task B
```

## 9.1 Explicit yielding

For a short loop that must stay async-owned:

```rust
for i in 0..n {
    do_small_step(i);
    if i % 1024 == 0 {
        tokio::task::yield_now().await;
    }
}
```

This can improve scheduler responsiveness but **does not turn CPU work into efficient parallelism**. If the loop is meaningfully expensive, move it to Rayon.

## 9.2 Task granularity

Bad:

```rust
for x in 0..1_000_000 {
    tokio::spawn(async move { cheap_hash(x) });
}
```

Better for CPU work:

```rust
use rayon::prelude::*;

let results: Vec<_> = (0..1_000_000)
    .into_par_iter()
    .map(cheap_hash)
    .collect();
```

Better for bounded async I/O: use a concurrency-limited stream or worker task set, not one unbounded task per input.

## 9.3 `select!` concurrency is same-task concurrency

Tokio explicitly notes that futures inside one `select!` run concurrently on the current task, **not in parallel**. If branches need parallel execution, spawn them and select over handles. ([Tokio select][T_SELECT])

```rust
let a = tokio::spawn(async { work_a().await });
let b = tokio::spawn(async { work_b().await });

tokio::select! {
    result = a => { /* ... */ }
    result = b => { /* ... */ }
}
```

## 9.4 Fairness

`select!` chooses a branch order pseudo-randomly by default to provide fairness when several branches are ready. `biased;` polls in source order and transfers fairness responsibility to your code. ([Tokio select][T_SELECT])

Use `biased;` only when deterministic priority is needed and starvation analysis is explicit.

## 9.5 Queue fairness vs throughput

Many runtime/synchronization knobs expose a trade-off:

```text
fairness / responsiveness
  more global/event checks
  more synchronization
  lower starvation risk

throughput / locality
  stay on local work longer
  fewer shared-queue accesses
  possibly slower pickup of new work
```

Do not optimize fairness knobs in isolation. Measure service-level outcomes.

## 9.6 Scheduler anti-patterns

* Adding `yield_now()` to a large CPU kernel instead of offloading it.
* `biased; select!` with a permanently-ready high-volume branch before shutdown.
* Spawning microscopic tasks.
* Assuming the runtime will preempt a poorly behaved future.
* Increasing worker count to mask blocking code.

---

# Rust Parallelism Stack Advanced — 10) Tokio channels: `mpsc`, `oneshot`, `broadcast`, `watch`

## 10.0 Channel selection map

Tokio's sync module centers message passing and provides four distinct channel semantics. ([Tokio sync][T_SYNC])

| Channel | Producers | Consumers | Buffer semantics | Canonical use |
|---|---:|---:|---|---|
| `mpsc` | many | one | bounded or unbounded queue | worker/actor inbox |
| `oneshot` | one | one | exactly one value | request/reply completion |
| `broadcast` | many | many | bounded history; every receiver sees sent values while able to keep up | events to subscribers |
| `watch` | many | many | retains only latest value | configuration/state version |

## 10.1 Bounded `mpsc`

```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel::<Job>(256);

// Producer: awaits capacity.
tx.send(job).await?;

// Consumer.
while let Some(job) = rx.recv().await {
    handle(job).await;
}
```

Bounded channels make overload visible and are usually the correct default for production pipelines.

## 10.2 Unbounded `mpsc`

An unbounded channel shifts the capacity limit to process memory. Use it only when:

* producer rate is inherently bounded below consumer rate;
* messages are tiny;
* external admission already provides a hard bound;
* queue growth is observable and failure policy is defined.

Otherwise prefer bounded.

## 10.3 `oneshot`

```rust
use tokio::sync::oneshot;

let (tx, rx) = oneshot::channel();

std::thread::spawn(move || {
    let _ = tx.send(expensive_sync_result());
});

let value = rx.await?;
```

Tokio's oneshot `send` is synchronous, so it can be used from non-async code or another runtime. That makes it a useful building block for a custom Rayon bridge. ([Tokio oneshot][T_ONESHOT])

## 10.4 `broadcast`

```rust
use tokio::sync::broadcast;

let (tx, _) = broadcast::channel::<Event>(1024);
let mut a = tx.subscribe();
let mut b = tx.subscribe();

tx.send(event)?;
```

A single stored value is cloned for receivers as they receive it. Capacity matters: slow subscribers may lag beyond retained history. ([Tokio broadcast][T_BROADCAST])

## 10.5 `watch`

Use `watch` for latest-state semantics:

```text
configuration version
health / readiness state
leader identity
current model snapshot
shutdown phase
```

Do not use `watch` when every intermediate event must be processed.

## 10.6 Channel closure as protocol

Dropping all senders naturally closes many channel forms. Build shutdown protocols around this intentionally:

```text
stop accepting producers
  -> drop/close senders
  -> consumer drains remaining queue
  -> receiver observes None/error
  -> consumer exits
```

## 10.7 Channel anti-patterns

* Unbounded channel by default.
* `broadcast` for work distribution: every receiver gets messages; it is not a competing-worker queue.
* `watch` for an audit/event log: it discards intermediate history.
* multiple competing consumers on Tokio `mpsc` assumption: it is MPSC, not MPMC.
* sending huge owned buffers into a backlog without a memory budget.

---

# Rust Parallelism Stack Advanced — 11) Tokio state synchronization: `Mutex`, `RwLock`, `Semaphore`, `Notify`, `Barrier`

## 11.0 Primitive selection

Tokio's synchronization types wait asynchronously rather than blocking the runtime worker. The module includes `Mutex`, `RwLock`, `Semaphore`, `Notify`, and `Barrier`. ([Tokio sync][T_SYNC])

| Need | Primitive |
|---|---|
| exclusive async access | `tokio::sync::Mutex` |
| many readers / one writer with async waiting | `tokio::sync::RwLock` |
| bound concurrency/resource units | `Semaphore` |
| wake one/all waiters without payload | `Notify` |
| wait for N tasks to rendezvous | `Barrier` |

## 11.1 Async mutex vs `std::sync::Mutex`

Do **not** automatically use Tokio Mutex just because code is async.

Use `std::sync::Mutex` / `parking_lot::Mutex` when:

* lock hold is extremely short;
* no `.await` occurs while holding it;
* contention is modest;
* blocking the runtime worker for the lock acquisition is acceptably tiny.

Use Tokio Mutex when the lock may need to remain held across `.await` or async lock contention is expected.

Best design remains: avoid holding any lock across slow external awaits if ownership/message passing can express the state transition.

## 11.2 Semaphore as admission control

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

let cpu_slots = Arc::new(Semaphore::new(16));

let permit = cpu_slots.clone().acquire_owned().await?;
let result = tokio_rayon::spawn(move || expensive_cpu(job)).await;
drop(permit);
```

This bounds in-flight CPU jobs **at the async boundary**. Rayon pool size still bounds executing workers.

Better lifecycle with scope:

```rust
let _permit = cpu_slots.clone().acquire_owned().await?;
let result = tokio_rayon::spawn(move || expensive_cpu(job)).await;
```

Permit drops automatically.

## 11.3 Weighted resources

`Semaphore::acquire_many` can model units such as estimated memory or file descriptors when jobs have non-uniform cost. Keep the unit model coarse and observable; false precision adds complexity without control value.

## 11.4 Notify

`Notify` is useful for edge/condition notification when there is no payload. It is not a general queue; if every event matters, use a channel.

## 11.5 Barrier

```rust
let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(workers));
```

Barriers are useful for coordinated test phases or algorithms with real synchronization phases. They are often a smell in service code because one stalled participant stalls all.

## 11.6 Synchronization anti-patterns

* Tokio Mutex around CPU-heavy critical section.
* Lock held while awaiting network I/O when state could be copied out first.
* `RwLock` chosen because reads “sound common” without measuring writer starvation/critical-section costs.
* Semaphore created after expensive input allocation rather than before admission.
* `Notify` used as a queue and losing semantic events.
* Barrier among tasks with failure/cancellation paths that can skip arrival.

---

# Rust Parallelism Stack Advanced — 12) Tokio `select!`, cancellation safety, timeouts, and abort semantics

## 12.0 `select!` mental model

```text
construct branch futures
      |
      v
poll enabled branches on current task
      |
first matching branch completes
      |
remaining branch futures are dropped
```

That final drop is why **cancellation safety** matters. ([Tokio select][T_SELECT])

## 12.1 Basic timeout/shutdown selection

```rust
tokio::select! {
    result = do_work() => {
        handle(result?);
    }
    _ = shutdown.changed() => {
        return Ok(());
    }
}
```

## 12.2 Cancellation-safe receive operations

Tokio documents common receive operations such as bounded/unbounded `mpsc::Receiver::recv`, `broadcast::Receiver::recv`, and `watch::Receiver::changed` as cancellation safe in `select!`. It also documents methods such as `read_exact`, `read_to_end`, `write_all`, and fair-queue lock acquisition as not cancellation safe in the same sense. ([Tokio select][T_SELECT])

Agent rule: before placing an operation inside a looped `select!`, check its cancellation-safety section.

## 12.3 Timeout wrapper

```rust
use tokio::time::{timeout, Duration};

let value = timeout(Duration::from_secs(2), async_operation()).await??;
```

A timeout drops/cancels the future being awaited. That stops ordinary async work at suspension points, but it does not magically terminate CPU/native/blocking work already executing elsewhere.

## 12.4 Abort vs cooperative cancellation

Abort is abrupt from the task's perspective. Cooperative cancellation is normally safer:

```text
signal cancellation
  -> task stops accepting new work
  -> flush/finish critical state
  -> close channels
  -> return
supervisor awaits completion
  -> abort only after grace policy expires
```

## 12.5 `spawn_blocking` cancellation limitation

Tokio explicitly documents that a `spawn_blocking` task generally cannot be aborted once it has begun executing. ([Tokio JoinHandle][T_JOIN])

Therefore CPU work requiring cancellation should accept a cooperative token/atomic flag and poll at reasonable grain boundaries.

## 12.6 Cancellation-safe state-machine design

Design async functions so dropping them at `.await` leaves durable invariants valid:

```text
BAD:
  remove item from authoritative queue
  await remote call
  commit result
  # cancellation can lose ownership between remove and commit

BETTER:
  mark lease/in-progress durably
  await remote call
  commit result or lease expires/retries
```

Cancellation is an application-state problem, not only a runtime API problem.

## 12.7 `select!` anti-patterns

* Looping `select!` around a non-cancellation-safe future without understanding lost progress.
* Assuming timeout kills blocking CPU work.
* Using `biased;` without starvation analysis.
* Doing CPU-heavy branch work inline before next `.await`.
* Treating abort as graceful shutdown.

---

# Rust Parallelism Stack Advanced — 13) Tokio blocking boundary: `spawn_blocking` and `block_in_place`

## 13.0 `spawn_blocking`

Signature shape:

```rust
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static;
```

It runs a closure on a thread where blocking is acceptable. Tokio grows the blocking pool up to its configured upper limit, then queues additional blocking tasks. The default upper limit is large because the pool supports blocking I/O; Tokio specifically calls out Rayon as a good fit for CPU-bound work. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 13.1 Good `spawn_blocking` uses

```text
sync library with no async API
legacy database call
filesystem/library operation that genuinely blocks
one-off CPU operation with strict external concurrency cap
FFI call that blocks the calling thread
```

## 13.2 Poor `spawn_blocking` use

```rust
for item in huge_input {
    tokio::task::spawn_blocking(move || cpu_heavy(item));
}
```

Problems:

* large potential blocking-thread count;
* large queued job count;
* no work-stealing data-parallel abstraction;
* difficult CPU budget;
* cancellation once started is limited.

Prefer a fixed Rayon pool and bounded admission.

## 13.3 `block_in_place`

`block_in_place` tells the multithread runtime that the current worker is about to block, allowing other tasks to move to another worker. It cannot be used inside a current-thread runtime and the enclosed code cannot be forcibly cancelled. ([Tokio block_in_place][T_BLOCK_IN_PLACE])

```rust
tokio::task::block_in_place(|| {
    synchronous_operation()
});
```

Use it sparingly. `spawn_blocking` generally gives cleaner task isolation.

## 13.4 `block_in_place` + `join!` trap

Tokio warns that other futures running concurrently **inside the same task**, for example through `join!`, are suspended while `block_in_place` is active. Independent spawned tasks are different. ([Tokio block_in_place][T_BLOCK_IN_PLACE])

## 13.5 Re-entering async from blocking code

`block_in_place` can re-enter async context through `Handle::current().block_on(...)`, but this is an advanced bridge and can make control flow hard to reason about. Prefer clean async/sync boundaries when possible.

## 13.6 CPU cancellation token

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

fn cpu_job(cancel: &AtomicBool, data: &[u8]) -> Result<u64, Cancelled> {
    let mut acc = 0u64;
    for chunk in data.chunks(64 * 1024) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        acc ^= process_chunk(chunk);
    }
    Ok(acc)
}
```

Poll at meaningful chunk boundaries, not every instruction.

## 13.7 Blocking-boundary checklist

```text
[ ] Does an async API exist? Use it first.
[ ] If sync-only, is operation blocking I/O or CPU?
[ ] Blocking I/O -> spawn_blocking/dedicated blocking pool.
[ ] CPU -> Rayon by default.
[ ] Bound queued inputs before offload.
[ ] Define cancellation semantics before starting native/blocking work.
[ ] Never assume JoinHandle::abort stops already-running blocking work.
```

---

# Rust Parallelism Stack Advanced — 14) Tokio I/O, filesystem, process, signals, and timers

## 14.0 Scope

Tokio's value is largest around **waiting**. The runtime integrates async tasks with network I/O, timers, process handles, signals, and asynchronous façades around filesystem operations.

## 14.1 Network I/O

Feature: `net`.

Typical pattern:

```rust
use tokio::net::TcpListener;

let listener = TcpListener::bind("127.0.0.1:9000").await?;
loop {
    let (socket, peer) = listener.accept().await?;
    tokio::spawn(async move {
        if let Err(e) = handle_connection(socket).await {
            eprintln!("connection {peer} failed: {e:?}");
        }
    });
}
```

The accept loop still needs admission policy if each connection can trigger expensive CPU or memory work.

## 14.2 `AsyncRead` / `AsyncWrite` and extension traits

Tokio's `io` module centers on `AsyncRead` and `AsyncWrite`; the `io-util` feature adds `AsyncReadExt`, `AsyncWriteExt`, `AsyncBufReadExt`, buffering helpers, and copy utilities. ([Tokio io][T_IO])

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};

let n = stream.read(&mut buf).await?;
stream.write_all(&buf[..n]).await?;
stream.flush().await?;
```

Common tools:

```text
AsyncRead / AsyncWrite     -> polling contracts
AsyncReadExt / AsyncWriteExt -> ergonomic async methods
BufReader / BufWriter      -> buffering
AsyncBufReadExt            -> lines/read_until/fill_buf-style helpers
io::copy                   -> streamed reader->writer copy
io::copy_bidirectional     -> proxy/tunnel-style full-duplex copy
io::split / owned socket splits -> concurrent read/write halves, with different ownership costs
```

`tokio::io::copy` streams data rather than materializing the entire input and currently allocates an internal 8 KiB copy buffer; `copy_buf` can use an existing `AsyncBufRead` buffer. ([Tokio io copy][T_IO_COPY])

### Buffer ownership rule

Prefer bounded streaming:

```text
read chunk -> validate/process -> write chunk
```

rather than:

```text
read_to_end(untrusted stream) -> giant Vec -> process
```

unless the protocol already enforces a small total size.

## 14.3 Filesystem

Feature: `fs`.

Filesystem APIs may ultimately require blocking OS operations on platforms/filesystems where no true async interface exists. This is one reason Tokio's blocking infrastructure exists. Do not interpret `tokio::fs` as proof that storage latency is nonblocking at the kernel/filesystem layer in every environment.

## 14.4 Process

Feature: `process`.

Use Tokio process APIs when child-process stdin/stdout/stderr and exit status participate in an async service. Bound child process concurrency; spawning a process is a heavyweight resource operation relative to an async task.

## 14.5 Signals

Feature: `signal`.

Canonical shutdown root:

```rust
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}
```

Production Unix services often combine SIGTERM + Ctrl+C and broadcast a cooperative shutdown signal through application state.

## 14.6 Timers

Feature: `time`.

```rust
use tokio::time::{sleep, interval, timeout, Duration};

sleep(Duration::from_millis(50)).await;
let mut ticker = interval(Duration::from_secs(1));
ticker.tick().await;
let result = timeout(Duration::from_secs(3), operation()).await;
```

Timer ticks should drive control work, not perform unbounded CPU batches inline.

## 14.7 I/O-to-CPU pipeline

```text
Tokio read bytes
  -> validate/bound size
  -> acquire CPU permit
  -> Rayon parse/decode/analyze
  -> release CPU permit
  -> Tokio write/store/respond
```

Keep large buffers owned/moved across the boundary. Avoid needless cloning merely to satisfy `'static`; design ownership of request data around the job.

## 14.8 I/O anti-patterns

* Async accept loop with no connection/job admission.
* `read_to_end` on untrusted stream with no maximum size.
* Huge CPU parse immediately after `.await` on runtime worker.
* Process spawn per tiny item.
* Timer callback launching unbounded work every tick when previous run is still active.

---

# Rust Parallelism Stack Advanced — 15) Tokio testing, runtime metrics, and deployment tuning

## 15.0 Async test

```rust
#[tokio::test]
async fn request_completes() {
    let value = async_operation().await.unwrap();
    assert_eq!(value, 42);
}
```

Specify runtime flavor when behavior depends on multithreading:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_test() {
    // ...
}
```

## 15.1 Paused time

With Tokio testing utilities, use paused/advanced time for deterministic timer logic rather than sleeping wall-clock seconds in tests. Keep such tests separate from real scheduler/performance tests.

## 15.2 What to observe

At minimum:

```text
in-flight tasks
admitted requests
queue depth
CPU-job queue depth
CPU-job wait time
async request latency
I/O latency
runtime worker utilization if available
blocking task count / saturation symptoms
cancellation count
panic / JoinError count
```

Tokio exposes runtime metrics and optional unstable tracing/latency instrumentation surfaces; exact APIs and feature gates are version-sensitive. Do not make unstable Tokio metrics part of a long-lived public application API without isolation.

## 15.3 Thread naming and tracing

Name runtime and Rayon workers. Attach logical job IDs to tracing spans instead of using thread ID as request identity: async tasks migrate and Rayon work stealing moves jobs between workers.

```text
trace fields:
  request_id
  job_id
  queue_wait_us
  cpu_exec_us
  input_bytes
  output_bytes
  cancelled
  executor=tokio|rayon|blocking
```

## 15.4 Load-test shape

Test at four levels:

1. underloaded steady state;
2. CPU saturation;
3. I/O saturation / slow downstream;
4. combined overload with cancellation and shutdown.

A system that benchmarks well only when queues are empty has not been production-tested.

## 15.5 Tokio tuning order

```text
1. remove blocking/CPU work from core workers
2. bound admission / queues
3. choose sensible worker count
4. inspect I/O and downstream bottlenecks
5. inspect task granularity / lock contention
6. only then tune scheduler intervals / unstable knobs
```

## 15.6 Tokio deployment anti-patterns

* Using runtime worker count to compensate for blocking code.
* No metric for queue wait time.
* Testing only throughput, not tail latency.
* Wall-clock sleeps in unit tests when virtual time is possible.
* Depending on unstable Tokio APIs directly throughout business logic.
* Shutting runtime down while CPU jobs still own resources with no lifecycle protocol.

---

# Rust Parallelism Stack Advanced — 16) Rayon identity and work-stealing mental model

## 16.0 Identity

Rayon is a **data-parallel and fork/join CPU parallelism library**. Its default abstraction is not “spawn N threads”; it is “describe decomposable work, let a fixed worker pool split and steal it.” ([Rayon crate][R0])

```text
parallel computation
        |
        v
split into jobs
        |
        +--> worker 0 local deque ----+
        +--> worker 1 local deque ----+-- idle workers steal available work
        +--> worker 2 local deque ----+
        +--> worker N local deque ----+
        |
        v
combine / reduce results
```

## 16.1 Why work stealing matters

Static partitioning:

```text
worker A -> 25 files, 3 seconds each
worker B -> 25 files, 50 ms each
worker C -> 25 files, 50 ms each
worker D -> 25 files, 50 ms each
```

Three workers go idle while A owns a long tail. Work stealing decomposes the job so idle workers can acquire pending work, reducing imbalance when item cost is irregular.

Rayon's `join` docs explain that one closure is executed locally while the other is advertised for stealing; the current worker keeps participating in the pool if the advertised work is stolen. ([Rayon join][R_JOIN])

## 16.2 Fixed-pool model

Rayon currently selects thread count from:

```text
ThreadPoolBuilder::num_threads(nonzero)
else RAYON_NUM_THREADS
else logical CPU count
```

The docs explicitly warn that default selection may evolve; set `num_threads` when a fixed resource contract matters. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER])

## 16.3 Global vs custom pool

Global pool:

```rust
use rayon::prelude::*;

let out: Vec<_> = input.par_iter().map(expensive).collect();
```

Custom pool:

```rust
let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(12)
    .build()?;

let out = pool.install(|| {
    input.par_iter().map(expensive).collect::<Vec<_>>()
});
```

All Rayon operations called inside `install` execute in that pool's context. ([Rayon ThreadPool][R_POOL])

## 16.4 Data parallel vs task parallel

Data parallel:

```rust
input.par_iter().map(parse).collect::<Vec<_>>()
```

Task parallel:

```rust
let (left, right) = rayon::join(
    || solve(left_input),
    || solve(right_input),
);
```

Dynamic task graph:

```rust
rayon::scope(|s| {
    for item in items {
        s.spawn(move |_| process(item));
    }
});
```

Prefer parallel iterators for collection-like work. Use `join`/`scope` when the algorithm's structure is naturally recursive or task-based.

## 16.5 Rayon and data-race freedom

Rust's ownership/type system plus Rayon trait bounds make ordinary safe Rayon APIs data-race safe. That does **not** imply race-free business semantics: atomics can have logical races, shared locks can deadlock, and order-dependent algorithms can produce nondeterministic results.

## 16.6 Rayon agent rules

```text
Rayon = CPU executor.
Prefer par_iter over manual thread partitioning.
Prefer custom pool when CPU budget is a product/deployment concern.
Do not block on network I/O inside Rayon jobs.
Nested Rayon parallelism normally reuses the current pool; do not create nested pools casually.
Use local accumulation + reduce rather than shared mutation when possible.
```

---

# Rust Parallelism Stack Advanced — 17) Rayon parallel iterators

## 17.0 Core conversion pattern

```rust
use rayon::prelude::*;

// borrowed
let out: Vec<_> = values.par_iter().map(|v| transform(v)).collect();

// mutable borrowed
values.par_iter_mut().for_each(|v| normalize(v));

// owned
let out: Vec<_> = values.into_par_iter().map(transform_owned).collect();
```

The `ParallelIterator` trait supplies the general combinators; `IndexedParallelIterator` adds operations requiring known indexed length/order properties. ([Rayon ParallelIterator][R_PAR_ITER])

## 17.1 API category map

| Category | Common methods |
|---|---|
| transform | `map`, `filter`, `filter_map`, `flat_map`, `flatten`, `update` |
| side effects | `for_each`, `for_each_with`, `for_each_init` |
| aggregation | `sum`, `product`, `min`, `max`, `reduce`, `fold` |
| search | `find_any`, `find_first`, `find_last`, `any`, `all` |
| fallible | `try_for_each`, `try_fold`, `try_reduce` |
| collection | `collect`, `partition`, `unzip` |
| ordering-sensitive | `find_first`, `find_last` where iterator supports meaningful order |
| flow hints | `panic_fuse`, `while_some`, `take_any`, `skip_any` |

## 17.2 `map` + `collect`

```rust
let parsed: Vec<Result<Node, ParseError>> = files
    .par_iter()
    .map(|path| parse_file(path))
    .collect();
```

If a single error should terminate the operation, use a fallible combinator or collect into `Result<Vec<_>, _>` when supported by the iterator/result traits rather than collecting all errors accidentally.

## 17.3 `for_each_init`: per-job-local expensive state

```rust
use rayon::prelude::*;

chunks.par_iter().for_each_init(
    || ScratchBuffer::new(),
    |scratch, chunk| {
        scratch.reset();
        process_with_scratch(chunk, scratch);
    },
);
```

Use `for_each_init` / `map_init` for non-`Sync` or expensive-to-create local state that should be created lazily per Rayon job context. The initializer may run multiple times; do not assume one instance exactly per OS worker unless the API explicitly promises it.

## 17.4 `for_each_with`: cloneable local state

```rust
let (tx, rx) = crossbeam::channel::unbounded();

items.par_iter().for_each_with(tx, |sender, item| {
    sender.send(process(item)).unwrap();
});
```

The initial state is cloned as needed. For channels, cloneable senders fit naturally; for heavyweight scratch objects, prefer `for_each_init`.

## 17.5 `filter` changes iterator properties

An indexed source such as a slice starts with `IndexedParallelIterator`, but operations whose output cardinality is unknown, such as `filter`, can remove indexed guarantees. This changes which methods are available. ([Rayon ParallelIterator][R_PAR_ITER])

Agent rule: do not force an algorithm to depend on exact work partition boundaries; they are scheduler details.

## 17.6 `find_any` vs `find_first`

```rust
let any = items.par_iter().find_any(|x| expensive_predicate(x));
let first = items.par_iter().find_first(|x| expensive_predicate(x));
```

Use `find_any` when any match is semantically valid: it gives the scheduler more freedom. Use `find_first` only when source order determines correctness.

## 17.7 Parallel mutation through disjoint borrows

```rust
values.par_iter_mut().for_each(|v| {
    *v = transform(*v);
});
```

This is an ideal Rayon shape: each job receives an exclusive reference to a disjoint element/slice, so no shared lock is needed.

## 17.8 Avoid shared push mutex

Poor pattern:

```rust
let out = std::sync::Mutex::new(Vec::new());
input.par_iter().for_each(|x| {
    out.lock().unwrap().push(transform(x));
});
```

Better:

```rust
let out: Vec<_> = input.par_iter().map(transform).collect();
```

Let Rayon own the collection/reduction topology instead of serializing on a central mutex.

## 17.9 Iterator anti-patterns

* `par_iter().for_each` followed by a global Mutex for every item.
* Using parallel iteration on a dozen trivial integers and expecting speedup.
* Depending on execution order of `for_each`.
* Performing async/blocking network calls inside iterator jobs.
* Cloning huge per-item state because a closure requires ownership instead of using `map_init`/shared immutable data.

---

# Rust Parallelism Stack Advanced — 18) Rayon indexed iterators, slices, strings, and parallel sorting

## 18.0 Indexed iterator model

`IndexedParallelIterator` means the iterator has a known length and indexed ordering that can be split deterministically as a sequence, even though execution order remains parallel. This enables methods such as exact chunking and length-aware splitting.

## 18.1 Parallel slice operations

```rust
use rayon::prelude::*;

let sums: Vec<u64> = data
    .par_chunks(4096)
    .map(|chunk| chunk.iter().copied().sum())
    .collect();
```

Useful slice families include:

```text
par_iter / par_iter_mut
par_chunks / par_chunks_mut
par_chunks_exact
par_windows
par_split / par_split_mut
par_sort / par_sort_by / par_sort_by_key
par_sort_unstable / variants
```

## 18.2 Chunking as grain control

If per-item work is tiny, process chunks:

```rust
bytes.par_chunks(64 * 1024).for_each(|chunk| {
    process_chunk(chunk);
});
```

Chunk size trades:

```text
larger chunks -> less scheduling overhead, better sequential locality, worse load balance
smaller chunks -> better load balance, more scheduling overhead
```

Benchmark with representative data distributions.

## 18.3 Parallel sort

Rayon documents parallel slice sorting APIs and even recommends `par_sort` rather than hand-written quicksort built on `join` for real sorting. ([Rayon join][R_JOIN])

```rust
use rayon::prelude::*;

values.par_sort_unstable();
```

Choose stable vs unstable based on semantics; unstable sort can avoid preserving equal-key input order and may be cheaper.

## 18.4 Strings

Rayon supports parallel string operations such as `par_split` and `par_lines`. ([Rayon iterator docs][R_ITER])

```rust
let count = text
    .par_lines()
    .filter(|line| line.contains("ERROR"))
    .count();
```

Parallelizing one small string is rarely useful; this shines for large text blobs with substantial per-line processing.

## 18.5 `with_min_len` / `with_max_len`

Indexed iterator adapters can influence splitting grain. Treat them as **hints/controls for partition size**, not guarantees about one closure invocation per chunk in all downstream adapter shapes.

Example:

```rust
let out: Vec<_> = data
    .par_iter()
    .with_min_len(1024)
    .map(expensive)
    .collect();
```

Use these after profiling scheduler overhead or cache behavior.

## 18.6 Order and determinism

Parallel collection into an ordered collection from an indexed iterator often preserves logical iterator order even though processing occurs out of order. But side effects (`println!`, external writes, shared map mutation) do not become ordered automatically.

Separate:

```text
logical output order
execution order
side-effect order
```

## 18.7 Slice/string anti-patterns

* Parallel sort for tiny vectors.
* Very small `par_chunks` causing scheduler overhead.
* Gigantic chunks causing long-tail imbalance.
* Assuming log output order reflects data order.
* Splitting UTF-8 by raw byte offsets without a byte-oriented algorithm that understands boundaries.

---

# Rust Parallelism Stack Advanced — 19) Rayon folds, reductions, fallible parallelism, and early termination

## 19.0 Why reduce instead of shared accumulation

Bad shared counter structure:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

let total = AtomicU64::new(0);
items.par_iter().for_each(|x| {
    total.fetch_add(expensive_value(x), Ordering::Relaxed);
});
```

The atomic may become a contention point.

Better:

```rust
let total: u64 = items
    .par_iter()
    .map(expensive_value)
    .sum();
```

Rayon can accumulate locally and combine partial results.

## 19.1 `reduce`

```rust
let total = items
    .par_iter()
    .map(|x| score(x))
    .reduce(|| 0u64, |a, b| a + b);
```

The identity function must produce a valid neutral value for independent partitions.

## 19.2 `fold` then `reduce`

Use `fold` to build local accumulators:

```rust
use std::collections::HashMap;

let counts = items
    .par_iter()
    .fold(HashMap::new, |mut local, item| {
        *local.entry(key(item)).or_insert(0usize) += 1;
        local
    })
    .reduce(HashMap::new, |mut a, b| {
        for (k, v) in b {
            *a.entry(k).or_insert(0) += v;
        }
        a
    });
```

For batch aggregation this is often superior to a DashMap because hot-key updates remain local until merge.

## 19.3 Fallible `try_for_each`

Rayon documents that `try_for_each` attempts to stop remaining work when an `Err`/`None` is encountered; if several failures race, which one is returned is unspecified. ([Rayon ParallelIterator][R_PAR_ITER])

```rust
items.par_iter().try_for_each(|item| -> Result<(), Error> {
    validate(item)?;
    process(item)?;
    Ok(())
})?;
```

Agent rule: never rely on “the first input error” unless your algorithm explicitly preserves/collects index information and selects deterministically after the parallel phase.

## 19.4 Fallible local state

```rust
items.par_iter().try_for_each_init(
    || ParserScratch::new(),
    |scratch, item| -> Result<(), ParseError> {
        parse_with_scratch(item, scratch)?;
        Ok(())
    },
)?;
```

## 19.5 `any` / `all`

```rust
let has_match = items.par_iter().any(expensive_predicate);
let all_valid = items.par_iter().all(validate_bool);
```

These may short-circuit logically, but already-running parallel jobs may continue briefly. Do not attach irreversible side effects to work you assume will never run after the result becomes knowable.

## 19.6 Floating-point reduction

Parallel reduction can change grouping/associativity compared with a sequential loop, producing slightly different floating-point rounding. If reproducibility is strict:

* use a numerically stable/reproducible reduction strategy;
* impose a deterministic merge tree if necessary;
* test tolerance vs bitwise exactness deliberately.

## 19.7 Reduction anti-patterns

* One global Mutex/atomic update per item.
* Non-associative reduction assumed to equal sequential left fold.
* Depending on error choice when multiple parallel errors are possible.
* Performing irreversible external writes inside a fallible parallel operation and assuming early error rolls them back.

---

# Rust Parallelism Stack Advanced — 20) Rayon `join`, `scope`, `spawn`, FIFO variants, and structured fork/join

## 20.0 `join`

```rust
let (a, b) = rayon::join(
    || compute_left(),
    || compute_right(),
);
```

`join` has low overhead relative to spawning OS threads and uses the pool's work-stealing scheduler. Both closures execute before `join` returns. ([Rayon join][R_JOIN])

## 20.1 Recursive divide-and-conquer

```rust
fn sum_tree(slice: &[u64]) -> u64 {
    const GRAIN: usize = 16_384;
    if slice.len() <= GRAIN {
        return slice.iter().copied().sum();
    }

    let mid = slice.len() / 2;
    let (left, right) = slice.split_at(mid);
    let (a, b) = rayon::join(|| sum_tree(left), || sum_tree(right));
    a + b
}
```

Always include a sequential grain cutoff. Recursive parallel decomposition down to one element wastes overhead.

## 20.2 Blocking warning

Rayon's `join` documentation assumes CPU-bound closures and warns that blocking I/O can perform poorly; cross-closure blocking waits can deadlock. ([Rayon join][R_JOIN])

Do not write:

```rust
rayon::join(
    || receiver.recv().unwrap(),
    || sender.send(compute()).unwrap(),
);
```

Even if a particular pool size makes it work today, the synchronization topology is fragile.

## 20.3 `scope`

```rust
rayon::scope(|s| {
    for chunk in chunks {
        s.spawn(move |_| process(chunk));
    }
});
```

Scoped jobs complete before the scope returns and can borrow data whose lifetime satisfies the scope constraints. ([Rayon Scope][R_SCOPE])

## 20.4 `spawn`

```rust
rayon::spawn(move || {
    background_cpu_job(owned_data);
});
```

Top-level `spawn` requires `'static` captures and is detached from the caller's stack frame. Its API is side-effect oriented: no result handle is returned. ([Rayon spawn][R_SPAWN])

Prefer `join`, `scope`, a channel/oneshot result, or tokio-rayon if result ownership matters.

## 20.5 FIFO vs default LIFO-style scheduling

Rayon exposes FIFO variants such as `spawn_fifo` / `scope_fifo`. LIFO/local depth-first scheduling often improves cache locality and recursive efficiency; FIFO can improve behavior when fairness/order of queued sibling work matters.

Do not treat FIFO as a strict global execution order under work stealing.

## 20.6 `in_place_scope`

Rayon 1.12 exposes `in_place_scope` as another structured scope form. Use specialized scope APIs only when their scheduling/stack behavior is required; ordinary `scope` should remain the default. ([Rayon in-place scope][R_IN_PLACE_SCOPE])

## 20.7 Panic propagation

For `join`, both closures execute; if one panics, the panic is propagated after execution semantics complete, and if both panic the documented propagation choice applies. ([Rayon join][R_JOIN])

With detached `spawn`, configure/understand the pool panic handler because there may be no natural caller to receive the panic.

## 20.8 Task API decision table

| Shape | Use |
|---|---|
| collection transformation | parallel iterator |
| exactly two recursive branches | `join` |
| dynamic structured child set | `scope` |
| detached side-effect job | `spawn` only with explicit supervisor/error path |
| async caller needs value | tokio-rayon / oneshot bridge |
| fairness-sensitive sibling queueing | evaluate FIFO APIs |

---

# Rust Parallelism Stack Advanced — 21) Rayon custom `ThreadPool` and `ThreadPoolBuilder`

## 21.0 Why custom pools matter

A global pool is excellent for libraries and single-class batch programs. A custom pool is preferable when the application has a **resource contract**:

```text
interactive CPU jobs: 8 threads
batch CPU jobs:       24 threads
low-priority indexing: 4 threads
```

## 21.1 Construction

```rust
let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(12)
    .thread_name(|i| format!("compute-{i}"))
    .stack_size(4 * 1024 * 1024)
    .build()?;
```

`ThreadPoolBuilder` currently supports configuration including `num_threads`, `thread_name`, `stack_size`, start/exit/panic handlers, spawn-handler customization, `breadth_first`, and current-thread usage. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER])

## 21.2 `install`

```rust
let result = pool.install(|| {
    data.par_iter().map(expensive).sum::<u64>()
});
```

Any Rayon operations invoked inside inherit the pool context. ([Rayon ThreadPool][R_POOL])

## 21.3 Global pool configuration

```rust
rayon::ThreadPoolBuilder::new()
    .num_threads(16)
    .thread_name(|i| format!("global-rayon-{i}"))
    .build_global()?;
```

Configure global pool once, near process startup. Library crates should generally avoid calling `build_global()` because they would seize application-wide policy.

## 21.4 Start/exit handlers

Use handlers for thread-local observability or platform integration:

```rust
let pool = rayon::ThreadPoolBuilder::new()
    .start_handler(|idx| tracing::debug!(idx, "rayon worker started"))
    .exit_handler(|idx| tracing::debug!(idx, "rayon worker stopped"))
    .build()?;
```

Keep handlers robust and nonblocking.

## 21.5 Panic handler

Rayon documents that some detached-spawn panics have no normal propagation target; a configured panic handler receives them. If no handler exists in such cases, default behavior may abort rather than let a panic disappear. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER])

Production rule: do not use detached Rayon jobs unless panic/error observation is designed.

## 21.6 `spawn_handler`

`spawn_handler` lets advanced applications control how Rayon OS worker threads are created. Use for affinity, special OS thread attributes, or embedding. This is low-level: a broken handler can prevent the pool from operating.

## 21.7 `use_current_thread`

The builder can use the current thread as a worker. Rayon notes this thread is not managed like ordinary pool workers and does not continuously run the normal stealing loop unless code yields/enters Rayon operations. ([Rayon ThreadPoolBuilder][R_POOL_BUILDER])

Treat this as an embedding feature, not an optimization default.

## 21.8 Multiple pools

Multiple pools are justified for:

* priority/resource-class isolation;
* different stack-size requirements;
* workloads calling libraries with incompatible thread policies;
* tenant/workload isolation where CPU allocation must be explicit.

Costs:

* aggregate thread count;
* cross-pool coordination;
* lost sharing of idle CPU capacity;
* lifecycle/metrics complexity.

## 21.9 Pool builder anti-patterns

* Pool per request.
* Global pool initialization inside reusable library constructor.
* Three 32-thread pools on a 32-thread host without resource policy.
* Huge stack sizes multiplied by many workers.
* Blocking telemetry in start/exit/panic handlers.
* Custom OS spawn handler without tests for panic/startup failure.

---

# Rust Parallelism Stack Advanced — 22) Rayon nested parallelism, granularity, scheduling, and pool choice

## 22.0 Nested parallelism is a feature

```rust
files.par_iter().for_each(|file| {
    sections(file).par_iter().for_each(|section| {
        analyze(section);
    });
});
```

When both layers execute in the same Rayon pool, this does **not** create a fresh OS thread pool per file. Rayon can schedule nested work across the existing worker pool.

That said, nested decomposition can create excessive scheduling overhead if both layers are too fine-grained.

## 22.1 Choose the highest-value parallel axis

Example dimensions:

```text
10,000 files x 10 functions/file x expensive function analysis
```

Possible choices:

* parallelize files only;
* parallelize functions within each file only;
* nested parallelism;
* flatten `(file,function)` jobs.

Prefer the dimension that:

* has enough independent units;
* preserves data locality;
* minimizes repeated setup;
* balances uneven work.

## 22.2 Sequential cutoffs

Recursive algorithms:

```rust
if input.len() < PARALLEL_THRESHOLD {
    return sequential(input);
}
```

The threshold should be benchmarked. It may depend on data shape and CPU.

## 22.3 LIFO locality

Depth-first/local LIFO behavior tends to keep recently produced subproblems near the producing worker's cache while thieves take older work. This is a common work-stealing advantage. FIFO APIs change this bias when fairness/breadth-first behavior is more important.

## 22.4 Memory-bandwidth saturation

CPU utilization at 100% is not proof that more parallelism helps. Many workloads become memory-bandwidth bound:

```text
threads 1 -> 10 GB/s
threads 4 -> 32 GB/s
threads 8 -> 45 GB/s
threads 16 -> 47 GB/s
threads 32 -> 46 GB/s + more latency
```

Benchmark throughput vs thread count. A smaller Rayon pool may be optimal for memory-heavy kernels.

## 22.5 Heterogeneous workloads

Do not mix a handful of 10-second jobs and thousands of 50-microsecond jobs in one queue without thinking about latency objectives. Work stealing balances throughput, not necessarily application-level priority.

Options:

* separate pools by priority/resource class;
* admission queues before pool;
* chunk short jobs;
* preemptible/cooperative job design if latency matters.

## 22.6 Cross-pool install

Calling `install` on a different Rayon pool from a Rayon worker has documented behavior that tries to keep the originating pool worker productive while waiting. Still, frequent cross-pool synchronous calls are a complexity smell; model the resource flow explicitly. ([Rayon ThreadPool][R_POOL])

## 22.7 Nested-parallelism checklist

```text
[ ] Is outer dimension large enough alone?
[ ] Is inner work expensive enough to justify splitting?
[ ] Does nested work reuse same pool?
[ ] Are per-item setup costs duplicated?
[ ] Does locality favor outer or inner partitioning?
[ ] Is memory bandwidth saturated before all workers are useful?
[ ] Are latency-sensitive and batch jobs sharing one pool?
```

---

# Rust Parallelism Stack Advanced — 23) Rayon panics, blocking, cooperative yielding, and cancellation patterns

## 23.0 Panic is not ordinary error handling

Use `Result` for expected failures:

```rust
let results: Result<Vec<_>, Error> = items
    .par_iter()
    .map(process)
    .collect();
```

Reserve panic for violated invariants. Parallel panic propagation is API-specific; detached work especially needs a pool panic handler.

## 23.1 Blocking I/O prohibition

Rayon's `join` documentation explicitly warns that its closures are assumed CPU-bound. Blocking I/O degrades performance and blocking one closure on another can deadlock. ([Rayon join][R_JOIN])

Architecture rule:

```text
Tokio -> await remote data
Rayon -> compute on local data
Tokio -> await remote write
```

not:

```text
Rayon worker -> blocking HTTP request -> waits seconds
```

## 23.2 Cooperative yield

Rayon 1.12 exposes `yield_now` and `yield_local`. `yield_now` may execute one pending unit of pool work; `yield_local` only considers local work. ([Rayon yield_now][R_YIELD]) ([Rayon yield_local][R_YIELD_LOCAL])

These are advanced tools for polling/embedding loops, not a substitute for structuring work correctly.

## 23.3 Cancellation is cooperative

Rayon has no general “kill this closure now” mechanism. Design long CPU jobs with a shared cancellation flag/token:

```rust
fn search(cancel: &std::sync::atomic::AtomicBool, chunks: &[Chunk]) -> Option<Hit> {
    use std::sync::atomic::Ordering;

    for chunk in chunks {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if let Some(hit) = search_chunk(chunk) {
            return Some(hit);
        }
    }
    None
}
```

At a Rayon layer, use iterator short-circuiting plus a cancellation flag if jobs are themselves long-running.

## 23.4 Panic containment around FFI/plugin code

If untrusted plugin-like Rust code may panic, consider `std::panic::catch_unwind` at a deliberate subsystem boundary where unwinding is supported. Do not indiscriminately catch panics and continue with potentially violated invariants.

## 23.5 Shutdown semantics

Dropping a user-created Rayon `ThreadPool` signals its workers to terminate after remaining spawned work is handled according to pool semantics. ([Rayon ThreadPool][R_POOL])

Before pool drop:

```text
stop new submissions
signal cancellation to long jobs
await/receive results for owned jobs
apply timeout/escalation policy
then drop pool
```

## 23.6 Anti-patterns

* Network waits on Rayon.
* Channel dependency cycle between Rayon jobs.
* Detached `rayon::spawn` with no panic/error observation.
* Believing Tokio abort can kill a Rayon closure submitted through a bridge.
* Checking cancellation flag on every tiny arithmetic operation and destroying performance.
* Never checking cancellation in a multi-minute job.

---

# Rust Parallelism Stack Advanced — 24) Rayon performance: cache locality, allocation, false sharing, NUMA, and SIMD interaction

## 24.0 Performance hierarchy

Optimize in this order:

```text
algorithm
  -> remove unnecessary work / asymptotic cost
memory representation
  -> contiguous data, fewer allocations
parallel axis / grain size
  -> enough work per job, balanced
shared-state elimination
  -> local + reduce
thread count
  -> match CPU/memory behavior
microarchitecture
  -> SIMD, cache alignment, NUMA, affinity
```

## 24.1 Cache locality

Prefer contiguous slices and chunk-local computation:

```rust
matrix.par_chunks_mut(row_width * rows_per_chunk)
    .for_each(process_block);
```

Pointer-heavy shared graphs can be parallel but may spend more time waiting on memory than computing.

## 24.2 False sharing

Two independent atomics on the same cache line can cause cache-line ping-pong:

```text
core 0 updates counter A  --- same cache line --- counter B updates core 1
```

Crossbeam's `CachePadded` exists specifically to pad/align values to reduce this form of contention. ([Crossbeam utilities][C_UTILS])

Use it when profiling shows hot per-worker counters/queues share cache lines; padding every object wastes memory.

## 24.3 Allocation

Bad per-item allocation:

```rust
items.par_iter().map(|x| {
    let mut tmp = Vec::with_capacity(1_000_000);
    expensive(x, &mut tmp)
}).collect::<Vec<_>>();
```

Better per-job scratch:

```rust
items.par_iter().map_init(
    || Vec::with_capacity(1_000_000),
    |tmp, x| {
        tmp.clear();
        expensive(x, tmp)
    },
).collect::<Vec<_>>();
```

## 24.4 Atomics and contended locks

Atomics eliminate lock blocking, not cache-coherence cost. A single hot `fetch_add` can scale poorly. Local counters + reduction often win.

## 24.5 SIMD interaction

Parallelism and SIMD compose:

```text
Rayon distributes chunks across cores
inside each chunk compiler/library vectorizes operations
```

But tiny chunks can inhibit vectorization/locality. Benchmark release binaries with target CPU settings appropriate to deployment.

## 24.6 NUMA

On multi-socket systems:

* memory allocation location can matter as much as thread location;
* first-touch policy may place pages near the thread that initializes them;
* work stealing can move computation across NUMA nodes;
* a single global shared map can generate cross-socket traffic.

Only add NUMA-aware pool partitioning/affinity when deployment hardware and profiling justify complexity.

## 24.7 Thread-count sweep

Benchmark:

```text
1, 2, 4, 8, 16, 24, 32 ... threads
```

for each important kernel and whole-service saturation test. The optimal count can be below logical CPU count due to memory bandwidth, SMT behavior, nested library threading, or latency goals.

## 24.8 Performance checklist

```text
[ ] Use --release.
[ ] Eliminate global per-item mutex/atomic updates.
[ ] Reuse scratch allocation.
[ ] Sweep chunk/grain size.
[ ] Sweep Rayon thread count.
[ ] Measure memory bandwidth/cache misses if scaling flattens.
[ ] Check false sharing on hot per-worker structures.
[ ] Inspect nested native library threading.
[ ] Keep CPU affinity/NUMA tuning as measured late-stage optimization.
[ ] Re-run after compiler/crate upgrades.
```

---
# Crossbeam Advanced — 25) Identity, facade, subcrate topology, and versioning

## 25.0 Mental model

```text
Crossbeam facade (`crossbeam`)
  ├─ channel  -> blocking MPMC message passing
  ├─ deque    -> work-stealing queues
  ├─ epoch    -> epoch-based memory reclamation
  ├─ queue    -> concurrent MPMC queues
  └─ utils    -> CachePadded, Backoff, scoped-thread utilities, atomics helpers

std / Tokio / Rayon provide higher-level scheduling
Crossbeam provides lower-level concurrent building blocks
```

Crossbeam is not a runtime and does not create an application-wide scheduler. It is a family of concurrency primitives. The `crossbeam` facade re-exports the component crates when their features are enabled. ([Crossbeam facade][C0])

## 25.1 Version topology

This reference pins the facade to:

```toml
crossbeam = "=0.8.4"
```

The facade version is older than several compatible component releases. As of this reference date the directly published component anchors are:

| Component | Current release | Primary role |
|---|---:|---|
| `crossbeam-channel` | 0.5.16 | MPMC channels |
| `crossbeam-deque` | 0.8.7 | work stealing |
| `crossbeam-epoch` | 0.9.20 | lock-free reclamation |
| `crossbeam-queue` | 0.3.13 | ArrayQueue / SegQueue |
| `crossbeam-utils` | 0.8.22 | utility primitives |

([Crossbeam channel release][C_CH_LATEST]) ([Crossbeam deque release][C_DQ_LATEST]) ([Crossbeam epoch release][C_EP_LATEST]) ([Crossbeam queue release][C_Q_LATEST]) ([Crossbeam utils release][C_U_LATEST])

### Reproducibility implication

A facade dependency can resolve compatible newer component releases according to Cargo semver ranges. Therefore:

```text
Cargo.toml pin != entire resolved dependency graph pin
Cargo.lock     = resolved graph reproducibility for binaries/apps
```

For a shipped binary/service, commit `Cargo.lock`. If a library requires behavior from a particular component release, depend on that component explicitly and document why.

## 25.2 Facade vs direct subcrate dependencies

### Facade-first

```toml
[dependencies]
crossbeam = "=0.8.4"
```

```rust
use crossbeam::{channel, deque, epoch, queue};
```

Use when several components are needed and facade convenience is valuable.

### Direct dependency

```toml
[dependencies]
crossbeam-channel = "=0.5.16"
```

```rust
use crossbeam_channel::{bounded, Receiver, Sender};
```

Use when:

* only one component is required;
* feature/dependency minimization matters;
* you need a newer component API than the facade documentation anchor;
* you want the component version visible as a first-class compatibility contract.

## 25.3 Feature flags

The facade exposes features for its component crates plus `std`/`alloc`/nightly-oriented paths. The default includes the normal standard-library environment. ([Crossbeam features][C_FEATURES])

Agent rule:

```text
Do not cargo-feature Crossbeam by guessing.
Inspect `cargo tree -e features` when reducing the graph.
```

## 25.4 When Crossbeam is the right abstraction

| Need | Crossbeam primitive | Prefer something else when |
|---|---|---|
| blocking multi-producer/multi-consumer queue | channel | async wait is required -> Tokio channel |
| custom work-stealing scheduler | deque | ordinary CPU data parallelism -> Rayon |
| bounded lock-free MPMC queue | `ArrayQueue` | blocking/backpressure semantics -> channel |
| unbounded lock-free-ish MPMC queue API | `SegQueue` | capacity must be bounded |
| safe reclamation for lock-free pointers | epoch | a lock is fast enough / simpler |
| reduce false sharing | `CachePadded` | object is cold / memory overhead matters |
| retry-loop backoff | `Backoff` | OS blocking primitive already fits |

## 25.5 What Crossbeam does not provide

Crossbeam does not by itself provide:

* async futures scheduling;
* an I/O reactor;
* CPU data-parallel iterators;
* a managed fixed CPU pool comparable to Rayon;
* a concurrent associative map comparable to DashMap;
* application-level cancellation or structured concurrency.

## 25.6 Anti-pattern inventory

* Treating Crossbeam as a Tokio or Rayon replacement at the runtime level.
* Building a custom work-stealing scheduler before benchmarking Rayon.
* Using unbounded channels/queues without a memory-growth argument.
* Using epoch reclamation where `Mutex`/`RwLock` is already fast enough.
* Depending on facade semver alone while assuming exact component versions.
* Using Crossbeam scoped threads in new code by default; `crossbeam::scope` is soft-deprecated since Rust 1.63 in favor of `std::thread::scope`. ([Crossbeam scope][C_SCOPE])

## 25.7 Agent checklist

```text
[ ] Decide facade vs direct subcrate deliberately.
[ ] Commit Cargo.lock for deployable applications.
[ ] Use channel for blocking MPMC messaging.
[ ] Use deque only for custom schedulers/work stealing.
[ ] Use queue for non-blocking concurrent queue data structures.
[ ] Use epoch only when implementing lock-free pointer structures.
[ ] Prefer std::thread::scope over Crossbeam scope in new code.
[ ] Benchmark complexity against simpler std/Tokio/Rayon alternatives.
```

---

# Crossbeam Advanced — 26) Channels: bounded/unbounded MPMC, selection, and backpressure

## 26.0 Identity

Crossbeam channels are synchronous/thread-blocking **multi-producer, multi-consumer** channels. Both `Sender` and `Receiver` can be cloned; cloned receivers compete for the same message stream rather than receiving broadcast copies. ([Crossbeam channel][C_CHANNEL])

```text
producer A ─┐
producer B ─┼─> channel queue ─┬─> consumer 1
producer C ─┘                 └─> consumer 2

one message -> one receiver
```

This differs materially from Tokio `broadcast`, where each receiver observes each retained message.

## 26.1 Constructors

### Bounded

```rust
use crossbeam_channel::bounded;

let (tx, rx) = bounded::<Job>(1024);
```

A bounded channel blocks a sender once capacity is exhausted until a receiver makes space. ([Crossbeam channel][C_CHANNEL])

### Unbounded

```rust
use crossbeam_channel::unbounded;

let (tx, rx) = unbounded::<Job>();
```

An unbounded channel avoids capacity blocking but shifts overload handling into memory growth.

### Zero-capacity rendezvous

```rust
use crossbeam_channel::bounded;

let (tx, rx) = bounded::<Job>(0);
```

A zero-capacity channel stores no messages: send and receive rendezvous directly. ([Crossbeam channel][C_CHANNEL])

Use for strict handoff/synchronization, not general buffering.

## 26.2 Sender / receiver sharing

```rust
let tx2 = tx.clone();
let rx2 = rx.clone();
```

Receiver cloning is load distribution:

```text
rx  and rx2 consume from the same stream
NOT: each receives every message
```

If broadcast/fan-out copy semantics are needed, use Tokio `broadcast`, explicit fan-out sends, or multiple channels.

## 26.3 Blocking, nonblocking, and timed operations

Typical API categories:

```text
send / recv                 -> block current OS thread
try_send / try_recv         -> return immediately
send_timeout / recv_timeout -> bounded wait
recv_deadline               -> absolute deadline where available
```

In Tokio async tasks, do not call blocking `recv()` on a runtime worker. Bridge blocking channel use through a dedicated thread or blocking boundary.

## 26.4 Disconnect semantics

When all senders are dropped, the channel is disconnected. Buffered messages remain receivable; once drained, receives return an error immediately. When all receivers are dropped, future sends fail. ([Crossbeam channel][C_CHANNEL])

Canonical worker termination:

```rust
while let Ok(job) = rx.recv() {
    process(job);
}
// exits after all senders are dropped and queue drains
```

This makes sender ownership part of shutdown semantics.

## 26.5 Iteration

```rust
for job in rx.iter() {
    process(job);
}
```

The iterator waits for messages and terminates when disconnected and drained.

Use `try_iter()` for currently available messages without waiting.

## 26.6 `select!`

Crossbeam supports channel selection so a thread can wait on multiple channel operations.

```rust
use crossbeam_channel::{select, tick};
use std::time::Duration;

let ticker = tick(Duration::from_secs(1));

loop {
    select! {
        recv(work_rx) -> msg => match msg {
            Ok(job) => process(job),
            Err(_) => break,
        },
        recv(ticker) -> _ => {
            report_progress();
        }
    }
}
```

The channel crate also provides timing channels such as `after`, `tick`, and `never`. ([Crossbeam channel][C_CHANNEL])

## 26.7 Bounded capacity as a control system

Capacity is not merely a performance parameter.

```text
arrival rate > service rate
  bounded queue   -> producer backpressure / bounded memory
  unbounded queue -> queue delay and memory rise until load falls or process fails
```

For service pipelines, default toward **bounded** channels unless there is an explicit proof that the producer rate and total workload are bounded.

### Capacity heuristic

Start with enough queue depth to absorb expected burstiness while keeping queueing latency observable:

```text
capacity ≈ worker_count × small burst factor
```

Then benchmark. A queue of one million jobs is often hiding overload rather than solving it.

## 26.8 Crossbeam channel vs Tokio channel

| Context | Prefer |
|---|---|
| ordinary OS worker threads | Crossbeam channel |
| async tasks that must wait without blocking a runtime thread | Tokio `mpsc` / `oneshot` / `broadcast` / `watch` |
| sync producer -> async consumer | Tokio channels can often bridge cleanly; `oneshot::Sender::send` is synchronous |
| CPU worker pool implemented with threads | Crossbeam bounded channel is natural |
| select over sync blocking operations | Crossbeam `select!` |
| select over futures/I/O | `tokio::select!` |

## 26.9 Worker-pool pattern

```rust
use crossbeam_channel::bounded;
use std::thread;

fn run_jobs(jobs: Vec<Job>, workers: usize) {
    let (tx, rx) = bounded::<Job>(workers * 4);

    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let rx = rx.clone();
            thread::spawn(move || {
                while let Ok(job) = rx.recv() {
                    process(job);
                }
            })
        })
        .collect();

    for job in jobs {
        tx.send(job).expect("workers still alive");
    }
    drop(tx);

    for h in handles {
        h.join().expect("worker panic");
    }
}
```

This is suitable when you truly need a custom fixed worker pool. For ordinary CPU maps/reductions, Rayon is less code and usually a better default.

## 26.10 Anti-pattern inventory

* Unbounded job channel for externally supplied work.
* Calling blocking Crossbeam `recv()` directly in an async Tokio task.
* Cloning `Receiver` and expecting broadcast semantics.
* Forgetting to drop the last `Sender`, causing workers to wait forever.
* Treating channel capacity only as throughput tuning rather than backpressure policy.
* Spinning on `try_recv()` instead of blocking/selecting/backing off.
* Using channel queue length as an exact correctness condition under concurrency.

## 26.11 Agent checklist

```text
[ ] Decide bounded vs unbounded intentionally.
[ ] Prefer bounded for externally driven pipelines.
[ ] Treat bounded(0) as rendezvous, not a tiny queue.
[ ] Remember receiver clones compete for messages.
[ ] Model shutdown by sender/receiver ownership.
[ ] Do not block Tokio worker threads on Crossbeam recv/send.
[ ] Use select! for blocking multiplexing; tokio::select! for futures.
[ ] Instrument queue depth, send wait, service time, and drops/errors.
```

---

# Crossbeam Advanced — 27) Work-stealing deques

## 27.0 Mental model

Crossbeam deque exposes the primitives from which a work-stealing scheduler can be built:

```text
                        global Injector
                           /   |   \
                          /    |    \
                         v     v     v
worker 0 local deque   worker 1    worker 2
 owner push/pop          ...          ...
     ^                                |
     |________ other worker steals ___|
```

Core types are `Injector<T>`, `Worker<T>`, and `Stealer<T>`. ([Crossbeam deque][C_DEQUE])

## 27.1 `Worker`

A `Worker<T>` is a local queue owned by one worker thread. Workers can be created with FIFO or LIFO local behavior:

```rust
use crossbeam_deque::Worker;

let fifo = Worker::new_fifo();
let lifo = Worker::new_lifo();
```

The owner accesses its `Worker`; other threads receive a `Stealer<T>`.

## 27.2 `Stealer`

```rust
let stealer = worker.stealer();
```

A steal attempt returns a `Steal<T>` result, which distinguishes success, empty, and retry due to concurrent interference.

Canonical retry helper:

```rust
use crossbeam_deque::Steal;

loop {
    match stealer.steal() {
        Steal::Success(job) => break Some(job),
        Steal::Empty => break None,
        Steal::Retry => continue,
    }
}
```

Do not convert `Retry` into `Empty`: it means retrying can succeed.

## 27.3 `Injector`

`Injector<T>` is a shared work queue suitable for injecting jobs from outside individual workers. Workers typically check local work first, then the injector, then other stealers.

Conceptual order:

```text
1. pop local
2. steal batch from global injector
3. steal from peer worker
4. back off / park
```

Batch stealing reduces contention compared with taking one global item at a time. ([Crossbeam deque][C_DEQUE])

## 27.4 Why local LIFO often helps CPU work

A worker that executes newly created child work quickly often benefits from:

* hot cache state;
* bounded active working set;
* depth-first locality.

Stealers can take older work to preserve global parallelism. This local-LIFO / remote-steal pattern is a common work-stealing design.

FIFO may be useful when fairness/older-job progression matters more than depth-first locality.

## 27.5 Skeleton custom scheduler

```rust
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use std::sync::Arc;

fn find_job<T>(
    local: &Worker<T>,
    global: &Injector<T>,
    stealers: &[Stealer<T>],
) -> Option<T> {
    local.pop()
        .or_else(|| {
            loop {
                match global.steal_batch_and_pop(local) {
                    Steal::Success(job) => return Some(job),
                    Steal::Empty => return None,
                    Steal::Retry => continue,
                }
            }
        })
        .or_else(|| {
            for stealer in stealers {
                loop {
                    match stealer.steal() {
                        Steal::Success(job) => return Some(job),
                        Steal::Empty => break,
                        Steal::Retry => continue,
                    }
                }
            }
            None
        })
}
```

A production scheduler additionally needs:

```text
sleep/wakeup protocol
shutdown state
panic isolation
job accounting
fairness
queue metrics
cancellation
worker lifecycle
memory ordering correctness
```

That is exactly why Rayon should remain the default for CPU task scheduling unless custom semantics are genuinely required.

## 27.6 When deque is justified

Use Crossbeam deque when you need scheduling semantics that Rayon does not directly expose, such as:

* heterogeneous worker classes;
* priority tiers implemented as multiple injectors;
* domain-local queues;
* custom task ownership;
* nonstandard wake/park logic;
* special affinity or resource constraints;
* embedded scheduler research.

## 27.7 Anti-pattern inventory

* Reimplementing Rayon just to “have control.”
* One shared injector with no local queues, creating a global contention point.
* Busy-spinning forever when no work exists.
* Treating `Steal::Retry` as no work.
* Sharing a `Worker` as though it were a general MPMC queue.
* Ignoring shutdown/wakeup races.
* Creating custom work stealing without saturation/fairness tests.

## 27.8 Agent checklist

```text
[ ] Use Injector for shared external work.
[ ] Give each worker its own Worker deque.
[ ] Share Stealer handles, not Worker ownership.
[ ] Handle Success / Empty / Retry distinctly.
[ ] Prefer batch stealing from the injector.
[ ] Add backoff then parking for idle workers.
[ ] Define shutdown and wakeup state before implementation.
[ ] Compare against Rayon under representative workloads.
```

---

# Crossbeam Advanced — 28) Concurrent queues: `ArrayQueue` and `SegQueue`

## 28.0 Queue selection

| Type | Capacity | Core characteristic | Typical use |
|---|---|---|---|
| `ArrayQueue<T>` | fixed | bounded MPMC array queue | bounded nonblocking handoff/buffer |
| `SegQueue<T>` | dynamic | unbounded MPMC segmented queue | internal work/event accumulation with bounded upstream assumptions |

Crossbeam documents both as concurrent queues that can be shared among threads. ([Crossbeam queue][C_QUEUE])

## 28.1 `ArrayQueue`

```rust
use crossbeam_queue::ArrayQueue;

let q = ArrayQueue::new(1024);

q.push(job).map_err(|job| QueueFull(job))?;
let next = q.pop(); // Option<Job>
```

Value case:

```text
fixed memory bound
nonblocking push/pop API
caller controls what “full” means
```

Unlike a bounded channel, `ArrayQueue::push` does not provide an automatic blocking wait for space. Backpressure policy belongs to your code.

Possible policies on full:

```text
retry with backoff
drop newest
drop oldest through a different design
spill elsewhere
return overload upstream
block using an external condvar/parking mechanism
```

## 28.2 `SegQueue`

```rust
use crossbeam_queue::SegQueue;

let q = SegQueue::new();
q.push(job);
let next = q.pop();
```

Use only when unbounded accumulation is acceptable or upstream cardinality is independently bounded.

## 28.3 Queue vs channel

```text
Queue = data structure
Channel = communication abstraction + connection/blocking semantics
```

Choose channel if you need:

* sender/receiver ownership;
* disconnect semantics;
* blocking waits;
* timeouts/select;
* queue capacity as built-in producer backpressure.

Choose queue if you are implementing the surrounding scheduling/wakeup protocol yourself.

## 28.4 Queue + notification pattern

A concurrent queue alone does not efficiently sleep a consumer waiting for future work. A custom design may pair it with a `Condvar`, parked-thread notification, eventfd-like mechanism, or async `Notify` depending on environment.

Correctness risk:

```text
check queue empty
  -> producer pushes + notifies
  -> consumer begins waiting after notification
  -> lost wakeup
```

The state transition and notification protocol must prevent this race. Prefer channels unless custom queue behavior is necessary.

## 28.5 Agent checklist

```text
[ ] Use ArrayQueue when memory must be bounded.
[ ] Define explicit full-queue behavior.
[ ] Use SegQueue only with an external boundedness argument.
[ ] Do not spin on empty queue indefinitely.
[ ] Pair queues with a race-safe wakeup protocol when consumers sleep.
[ ] Prefer channels when disconnect/blocking/select semantics are desired.
```

---

# Crossbeam Advanced — 29) Epoch-based memory reclamation

## 29.0 Why reclamation is the hard part of lock-free structures

A lock-free pointer can be atomically removed from a structure while another thread still holds a previously loaded pointer to that allocation.

Bad immediate free:

```text
thread A loads pointer P
thread B removes P
thread B frees P
thread A dereferences P -> use-after-free
```

Epoch-based reclamation separates **logical removal** from **physical destruction** until it is safe to know no pinned participant can still access the retired object. Crossbeam's epoch module exposes this machinery. ([Crossbeam epoch][C_EPOCH])

## 29.1 Core vocabulary

```text
Atomic<T>  -> atomic pointer slot managed with epoch-aware operations
Owned<T>   -> uniquely owned heap allocation not yet shared
Shared<'g,T> -> pointer reference tied to a Guard lifetime
Guard      -> pins participant / protects relevant shared pointers
pin()      -> enter protected epoch participation
retire/defer_destroy -> schedule reclamation after safe grace period
```

## 29.2 Conceptual lifecycle

```text
Owned<Node>
   |
   | store / compare_exchange
   v
Atomic<Node> ---- load under Guard ----> Shared<'g, Node>
   |
   | remove atomically
   v
retired Shared
   |
   | defer destruction through Guard
   v
reclaimed only after no relevant pinned readers can still observe it
```

## 29.3 Pinning

```rust
use crossbeam_epoch as epoch;

let guard = &epoch::pin();
let ptr = atomic.load(std::sync::atomic::Ordering::Acquire, guard);
```

The `Shared` pointer's usable lifetime is coupled to the guard.

Agent invariant:

```text
A Shared pointer is not an owned allocation.
Its safe dereference depends on the Guard/reclamation protocol.
```

## 29.4 Compare-and-swap loop shape

Lock-free updates often look conceptually like:

```rust
loop {
    let guard = &epoch::pin();
    let current = head.load(Ordering::Acquire, guard);

    let new = Owned::new(Node::new(value, current));

    match head.compare_exchange(
        current,
        new,
        Ordering::AcqRel,
        Ordering::Acquire,
        guard,
    ) {
        Ok(_) => break,
        Err(err) => {
            // recover ownership of the uninstalled allocation and retry
            // using the error value returned by the exact API
        }
    }
}
```

Exact `CompareExchangeError` fields and ownership recovery should be taken from the pinned component documentation. Do not invent unsafe pointer conversions from memory.

## 29.5 Retiring removed nodes

After atomically unlinking a node, destruction is typically deferred through the guard rather than performed immediately.

Conceptual rule:

```text
unlink != free
unlink -> retire -> grace period -> destroy
```

## 29.6 Memory ordering

Epoch reclamation solves reclamation safety; it does **not** remove the need for correct atomic memory ordering.

You still must prove:

```text
publication ordering
read visibility
CAS success/failure ordering
linked-node initialization visibility
```

Do not replace `SeqCst` with relaxed/acquire/release orderings based on performance intuition alone. Establish the happens-before proof and test with concurrency-model tools where possible.

## 29.7 Pin duration

A participant pinned for a very long time can delay reclamation of retired objects, allowing memory usage to rise even though logical removals occur.

Therefore:

```text
pin around a bounded critical traversal/update
avoid holding Guard through long I/O/sleep/unrelated work
```

## 29.8 When epoch is appropriate

Use when:

* you are implementing a lock-free pointer-based data structure;
* profile data shows lock contention is material;
* the team can maintain unsafe/concurrency invariants;
* correctness can be tested with stress/model checking/fuzzing.

Do not use merely because “lock-free is faster.” Often it is not, especially under low contention or complex memory-reclamation pressure.

## 29.9 Anti-pattern inventory

* Immediate free after atomic unlink.
* Dereferencing `Shared` outside its guard validity assumptions.
* Holding guards across long waits/I/O.
* Treating epoch GC as a substitute for atomic ordering correctness.
* Adding `unsafe` conversions to bypass lifetime errors instead of understanding the protocol.
* Building a custom lock-free map instead of using a proven structure without measured need.
* Benchmarking throughput while ignoring retained retired-memory high-water marks.

## 29.10 Agent checklist

```text
[ ] Prove the data structure needs lock-free reclamation.
[ ] Keep Guard scope bounded.
[ ] Tie Shared access to the correct Guard.
[ ] Atomically unlink before retirement.
[ ] Defer physical destruction until safe.
[ ] Prove memory orderings separately from reclamation safety.
[ ] Test ABA/tagging assumptions if pointers can be reused.
[ ] Track retired-memory behavior under stalled/pinned threads.
[ ] Use Loom/Miri/sanitizers/stress tests where applicable.
[ ] Never fabricate unsafe code from approximate API memory.
```

---

# Crossbeam Advanced — 30) Utilities, atomics support, backoff, cache padding, and scoped threads

## 30.0 `Backoff`

`Backoff` implements exponential backoff for spin/retry loops. It can issue processor pause/yield behavior, yield an OS time slice, and indicate when switching to a blocking mechanism is advisable. ([Crossbeam Backoff][C_BACKOFF])

```rust
use crossbeam_utils::Backoff;

let backoff = Backoff::new();
loop {
    if try_operation() {
        break;
    }
    backoff.snooze();
}
```

Use:

```text
spin()   -> retry because another thread made progress; shorter CPU-level backoff
snooze() -> waiting for another thread; can progress toward OS yield
is_completed() -> consider parking/blocking instead
```

Do not spin forever under potentially long waits.

## 30.1 `CachePadded<T>`

False sharing occurs when independent hot variables share a hardware cache line:

```text
core A updates counter A --┐
                           ├ same cache line -> coherence ping-pong
core B updates counter B --┘
```

`CachePadded<T>` pads/alignment-isolates a hot value to reduce false sharing.

```rust
use crossbeam_utils::CachePadded;
use std::sync::atomic::AtomicU64;

struct WorkerCounters {
    processed: CachePadded<AtomicU64>,
}
```

Use only for measured hot per-thread/per-shard state; padding cold structures increases memory footprint/cache pressure.

## 30.2 Scoped threads

Crossbeam historically provided scoped threads that can borrow stack data, but its `scope` function is now soft-deprecated in favor of `std::thread::scope` on modern Rust. ([Crossbeam scope][C_SCOPE])

Prefer:

```rust
std::thread::scope(|s| {
    s.spawn(|| use_borrowed_slice(&data));
});
```

instead of introducing Crossbeam only for scoped threads.

## 30.3 Atomic helpers

Crossbeam utilities include low-level concurrency helpers useful in specialized implementations. Treat these as systems-programming tools, not general application primitives.

Decision rule:

```text
std atomic works -> use std
proven Crossbeam utility materially helps -> use it
custom unsafe atomic abstraction -> last resort
```

## 30.4 `AtomicCell<T>`

`AtomicCell<T>` is a thread-safe mutable cell that can atomically load/store/exchange compatible values and provides additional atomic operations for supported types. Crossbeam documents that operations use atomic instructions when possible and may fall back to global locks otherwise; `AtomicCell::<T>::is_lock_free()` exposes whether a type is lock-free on the target. Loads use Acquire and stores Release in the documented API. ([Crossbeam AtomicCell][C_ATOMIC])

```rust
use crossbeam::atomic::AtomicCell;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum State { Idle, Running }

let state = AtomicCell::new(State::Idle);
state.store(State::Running);
let now = state.load();
assert_eq!(now, State::Running);
```

Use for small copyable state where `AtomicBool`/`AtomicUsize` does not express the type conveniently. Do not assume an arbitrary `T` is lock-free.

## 30.5 `Parker` / `Unparker`

`Parker` is a thread-parking primitive with an associated token. `park()` consumes an available token or blocks; `unpark()` makes a token available, so an unpark that happens before park is not lost. Timed/deadline parking variants are also provided. ([Crossbeam Parker][C_PARKER])

```rust
use crossbeam::sync::Parker;

let parker = Parker::new();
let unparker = parker.unparker().clone();

std::thread::spawn(move || {
    produce_work();
    unparker.unpark();
});

parker.park();
```

This is useful when implementing custom worker sleep/wakeup protocols. Prefer a channel/condvar/runtime primitive when you do not need custom parking semantics.

## 30.6 `ShardedLock<T>`

`ShardedLock` is a reader-writer lock optimized for scalable reads by spreading readers across internal cache-line-separated shards; writes must acquire all shards and are correspondingly slower. ([Crossbeam ShardedLock][C_SHARDED])

```rust
use crossbeam::sync::ShardedLock;

let config = ShardedLock::new(Config::default());
{
    let r = config.read().unwrap();
    consume(&r);
}
{
    let mut w = config.write().unwrap();
    w.reload();
}
```

Use for very read-heavy synchronous state after benchmarking against `std::sync::RwLock`; it is not an async lock and should not be held across `.await`.

## 30.7 `WaitGroup`

`WaitGroup` supports one-shot synchronization where cloning registers participants and `wait(self)` drops the caller's reference then waits for all other references to be dropped. Unlike a reusable barrier, participant count is established through clones and synchronization is one-shot. ([Crossbeam WaitGroup][C_WAITGROUP])

```rust
use crossbeam::sync::WaitGroup;

let wg = WaitGroup::new();
for _ in 0..4 {
    let worker_wg = wg.clone();
    std::thread::spawn(move || {
        do_work();
        drop(worker_wg);
    });
}
wg.wait();
```

For owned threads that should be inspected for panics/results, joining thread handles is often a stronger lifecycle primitive than a wait group.

## 30.8 Utility checklist

```text
[ ] Use Backoff only inside bounded retry/wait strategy.
[ ] Transition from spin to park/block for longer waits.
[ ] Use CachePadded after observing false sharing/cache-line contention.
[ ] Check AtomicCell::is_lock_free before assuming lock-free behavior.
[ ] Use Parker only with a proven wake/sleep protocol.
[ ] Benchmark ShardedLock against ordinary RwLock for read-heavy state.
[ ] Use WaitGroup for one-shot coordination; join handles when results/panics matter.
[ ] Prefer std::thread::scope on modern Rust.
[ ] Keep low-level atomic helpers isolated behind safe APIs.
```

---

# Crossbeam Advanced — 31) Custom worker schedulers and staged pipelines

## 31.0 Why this section exists

Crossbeam becomes most valuable when the application has scheduling semantics not captured cleanly by Tokio or Rayon. The danger is accidentally constructing a fragile runtime.

Use this section as a **minimum architecture checklist**, not an encouragement to build one.

## 31.1 Bounded staged pipeline

```text
producer
   |
   v
bounded ingress channel
   |
   +--> worker pool A: parse/normalize
            |
            v
       bounded intermediate channel
            |
            +--> worker pool B: resolve/analyze
                         |
                         v
                   bounded output channel
                         |
                         v
                      writer
```

Properties:

```text
bounded memory at each stage
backpressure propagates upstream
stage-specific worker counts
queue depth exposes bottleneck location
```

## 31.2 Worker count by bottleneck

Do not give every stage `num_cpus` threads.

```text
CPU stage       -> bounded near useful CPU parallelism
memory-bandwidth stage -> may saturate below core count
blocking I/O stage -> potentially more concurrency, preferably async if appropriate
serialized sink -> 1 or small writer count
```

## 31.3 Crossbeam channel pipeline example

```rust
use crossbeam_channel::bounded;
use std::thread;

let (parse_tx, parse_rx) = bounded::<RawItem>(128);
let (analyze_tx, analyze_rx) = bounded::<ParsedItem>(128);

let parsers: Vec<_> = (0..parse_workers).map(|_| {
    let rx = parse_rx.clone();
    let tx = analyze_tx.clone();
    thread::spawn(move || {
        while let Ok(raw) = rx.recv() {
            let parsed = parse(raw);
            if tx.send(parsed).is_err() {
                break;
            }
        }
    })
}).collect();

drop(analyze_tx); // keep only worker clones
```

In real code, coordinate sender drop order carefully so downstream workers see disconnection only after every upstream worker is done.

## 31.4 Global + local work queues

For recursive/dynamically generated work:

```text
external submissions -> Injector
worker-generated children -> local Worker deque
idle workers -> steal batch/global/peers
```

This is closer to Rayon internals conceptually, but you must supply lifecycle and wakeup mechanics yourself.

## 31.5 Priority queues

Crossbeam deque has no universal application-level priority scheduler abstraction. A simple custom design can use multiple bounded channels/injectors:

```text
high priority queue
normal queue
background queue
```

Worker policy must prevent starvation, e.g. weighted service rather than always draining high priority first.

## 31.6 Shutdown state machine

Define explicitly:

```text
RUNNING
  -> DRAINING: reject new external submissions, finish accepted work
  -> STOPPING: tell workers to terminate after queue policy
  -> STOPPED: all workers joined, resources released
```

Avoid a single `AtomicBool` with ambiguous semantics for “stop accepting”, “cancel current”, and “exit now.”

## 31.7 Observability contract

Every custom worker system should expose at least:

```text
queue depth / utilization
jobs accepted/completed/failed/dropped
queue wait latency
service latency
worker active/idle count
steal attempts/success if applicable
backpressure duration
panic count
shutdown duration
```

## 31.8 Agent anti-patterns

```text
custom scheduler without explicit shutdown state
unbounded ingress
busy-spin idle workers
one global mutex-protected VecDeque for high contention
no panic handling
no accepted-vs-completed accounting
priority without starvation policy
nested custom pools with no whole-process thread budget
```

---

# DashMap Advanced — 32) Identity, sharding, construction, and fit

## 32.0 Mental model

`DashMap<K, V>` provides a concurrent map with a `HashMap`-like API while methods generally take `&self`, enabling the map to be placed in an `Arc` and shared across threads. Internally, data is partitioned into shards protected by locks rather than one global lock. ([DashMap][D_MAP])

```text
hash(key)
   |
   v
select shard
   |
   +--> shard 0 lock + table
   +--> shard 1 lock + table
   +--> shard 2 lock + table
   ...
```

Sharding reduces contention when keys distribute across shards, but operations on the same/hot shard still serialize.

## 32.1 Construction

```rust
use dashmap::DashMap;

let map: DashMap<String, Value> = DashMap::new();
let map = DashMap::<String, Value>::with_capacity(100_000);
```

Custom shard amount:

```rust
let map = DashMap::<u64, Value>::with_shard_amount(32);
```

The shard count must be greater than zero and a power of two; invalid counts panic. ([DashMap][D_MAP])

Combined construction:

```rust
let map = DashMap::<u64, Value>::with_capacity_and_shard_amount(
    1_000_000,
    64,
);
```

Do not tune shard count from folklore. More shards can reduce lock contention but increase metadata, iteration work, and complexity of multi-shard operations.

## 32.2 Basic sharing

```rust
use dashmap::DashMap;
use std::sync::Arc;

let index = Arc::new(DashMap::<String, Metadata>::new());

let index2 = Arc::clone(&index);
std::thread::spawn(move || {
    index2.insert("file.rs".into(), metadata());
});
```

## 32.3 When DashMap fits

Good fit:

* many independent key-level reads/writes;
* values can be updated under short guard lifetimes;
* a fully coherent multi-key transaction is not required;
* hot keys are not overwhelmingly concentrated in one shard;
* simplicity is more valuable than a bespoke sharded map.

Poor fit:

* most operations scan/lock the whole map;
* long computations happen while entry guards are held;
* multi-key atomic invariants dominate;
* a single-writer actor can own the state cheaply;
* read-mostly snapshot replacement is simpler;
* per-worker local maps + merge would avoid shared mutation entirely.

## 32.4 Feature flags

DashMap's optional feature surface includes `rayon`, `serde`, `raw-api`, `inline`, `arbitrary`, and type-size-oriented support; no broad feature is required just to use the normal map. ([DashMap features][D_FEATURES])

Production rule:

```text
Do not enable raw-api unless low-level shard access is required and reviewed.
Enable rayon only when parallel map iteration is actually used.
```

## 32.5 Shard visibility

With `raw-api`, `shards()` exposes the internal padded `RwLock<RawTable<...>>` array. The docs explicitly warn that this is low-level functionality. ([DashMap][D_MAP])

```rust
// requires raw-api
let shard_count = map.shards().len();
```

Do not bind application correctness to undocumented shard/hash layout.

## 32.6 Agent checklist

```text
[ ] Confirm workload is independent key-level concurrent access.
[ ] Keep guard lifetime short.
[ ] Size capacity if large growth is predictable.
[ ] Tune shard amount only through contention benchmarks.
[ ] Keep raw-api disabled unless genuinely necessary.
[ ] Compare against local-map + merge, actor ownership, or std locks.
```

---

# DashMap Advanced — 33) Core API, Entry API, updates, and ownership patterns

## 33.0 Core operations

```rust
let old = map.insert(key, value);
let value = map.get(&key);
let value_mut = map.get_mut(&key);
let removed = map.remove(&key);
let exists = map.contains_key(&key);
```

`get()` returns a guard-like `Ref`; `get_mut()` returns `RefMut`. These guards keep the relevant shard lock state alive. ([DashMap][D_MAP])

## 33.1 Read pattern: copy/clone and drop guard early

Prefer:

```rust
let metadata = map.get(&key).map(|r| r.value().clone());

// no DashMap guard held here
if let Some(metadata) = metadata {
    expensive_async_or_cpu_work(metadata);
}
```

rather than:

```rust
if let Some(entry) = map.get(&key) {
    expensive_work(entry.value()); // guard remains held for entire call
}
```

If only a small copyable field is required:

```rust
let state = map.get(&key).map(|r| r.state);
```

## 33.2 Mutation pattern

```rust
if let Some(mut entry) = map.get_mut(&key) {
    entry.counter += 1;
} // guard released here
```

Do not call back into the same map while a mutable guard is live unless the exact locking behavior proves it safe.

## 33.3 Entry API

```rust
map.entry(key)
    .and_modify(|v| v.count += 1)
    .or_insert_with(|| Value::new());
```

The Entry API mirrors common `HashMap` patterns and includes occupied/vacant handling and helpers such as `or_insert`, `or_insert_with`, `or_default`, `and_modify`, and fallible insertion variants. ([DashMap Entry][D_ENTRY])

Use Entry to avoid a separate lookup + insert race:

Bad logical pattern:

```rust
if !map.contains_key(&key) {
    map.insert(key, make_value());
}
```

Two threads can both observe absence.

Better:

```rust
map.entry(key).or_insert_with(make_value);
```

## 33.4 `try_get` / `try_get_mut`

DashMap exposes try-style access that can distinguish an unavailable locked shard from absence, via its `TryResult` family.

Use when the application genuinely prefers skip/retry behavior over blocking on a contended shard.

Do not spin aggressively on `try_get_mut`; use backoff/yield or redesign hot-key contention.

## 33.5 Remove conditionally

Use conditional remove APIs when a read-check-remove sequence must be performed under the map's synchronization semantics rather than as three independently interleavable operations.

## 33.6 Value structure matters

A DashMap of huge mutable values can make each guard-held operation expensive. Often prefer:

```rust
DashMap<Key, Arc<Value>>
```

when values are immutable/read-mostly and replaced atomically at key granularity, or:

```rust
DashMap<Key, SmallState>
```

where expensive payload lives elsewhere.

But `Arc<Mutex<Value>>` inside `DashMap` introduces two lock layers; use only when necessary and define lock ordering.

## 33.7 Agent rules

```text
get/get_mut/entry return synchronization-bearing guards.
Copy/clone what you need, then drop the guard.
Use Entry for atomic-at-key initialize/update patterns.
Never assume contains_key + insert is atomic as a compound action.
Keep value mutation short and non-blocking.
```

---

# DashMap Advanced — 34) Guard lifetimes, locking behavior, and deadlock avoidance

## 34.0 The most important DashMap rule

**Never treat a DashMap reference guard like an ordinary detached `&V`/`&mut V`.** It participates in shard locking.

The DashMap documentation explicitly warns that many operations may deadlock if called while holding references into the map. For example, `get_mut`, `iter_mut`, `entry`, `retain`, `clear`, and several whole-map operations may deadlock while any map reference is held; even read-like operations such as `get`, `len`, and `contains_key` may deadlock while a mutable reference is held. ([DashMap locking behavior][D_LOCKS])

## 34.1 Bad nested access

```rust
let mut a = map.get_mut(&key_a).unwrap();

// dangerous: another map operation while mutable guard `a` is held
let b = map.get(&key_b);

update(&mut a, b.as_deref());
```

Even if `key_a != key_b`, they can hash to the same shard; application logic should not assume distinct keys imply distinct locks.

## 34.2 Safe staged access

```rust
let b_snapshot = map.get(&key_b).map(|r| r.value().clone());

if let Some(mut a) = map.get_mut(&key_a) {
    update(&mut a, b_snapshot.as_ref());
}
```

Or explicitly scope:

```rust
let snapshot = {
    let r = map.get(&key).unwrap();
    r.value().clone()
}; // guard definitely dropped

use_snapshot(snapshot);
```

## 34.3 Never hold guards across `.await`

Bad:

```rust
let entry = map.get(&key).unwrap();
let remote = fetch_remote(entry.id).await;
```

The map guard is held while the task can be suspended for an arbitrary duration.

Better:

```rust
let id = {
    let entry = map.get(&key).unwrap();
    entry.id.clone()
};

let remote = fetch_remote(id).await;
```

This principle applies even if the compiler allows a particular guard to cross the await boundary.

## 34.4 Never run long CPU work while guard held

```rust
let payload = map.get(&key).map(|r| r.payload.clone());
if let Some(payload) = payload {
    rayon_heavy_analysis(payload);
}
```

Guard-held CPU work increases shard wait time and creates convoy effects.

## 34.5 Whole-map operations

Operations like iteration, retain, clear, shrinking, and global size/capacity queries can touch many shards. Treat them as coordination-heavy operations, not cheap metadata reads in hot loops.

## 34.6 Lock ordering for multiple maps

If code must hold references/locks from multiple structures simultaneously, define a global order:

```text
Map A before Map B
smaller shard/key class before larger, if explicitly controlled
never reverse order elsewhere
```

Better still: snapshot from one structure, release, then acquire the other.

## 34.7 `alter` panic semantics

DashMap documents that a panic inside the closure passed to `alter`/`alter_all` aborts the process. Do not place fallible or user-provided logic there unless its panic behavior is deliberately controlled. ([DashMap locking behavior][D_LOCKS])

## 34.8 Deadlock code-review checklist

```text
[ ] Any Ref / RefMut / Entry guard held while calling map.* again?
[ ] Any guard held across await?
[ ] Any guard held while blocking on channel/mutex/thread join?
[ ] Any guard held while invoking arbitrary callback/user code?
[ ] Any nested DashMap / mutex lock acquisition order?
[ ] Any whole-map operation called from inside iter/get_mut/entry closure?
[ ] Any expensive clone avoided at cost of a dangerously long guard?
```

---

# DashMap Advanced — 35) Iteration, Rayon integration, read-only views, DashSet, and raw access

## 35.0 Iteration

```rust
for entry in map.iter() {
    consume(entry.key(), entry.value());
}
```

`iter()` yields reference guards. Do not recursively mutate/query in ways that can reacquire conflicting shard locks while an iteration guard is live.

Mutable iteration:

```rust
map.iter_mut().for_each(|mut entry| {
    entry.value_mut().count += 1;
});
```

The documentation explicitly warns `iter_mut()` may deadlock if called while holding any reference into the map. ([DashMap locking behavior][D_LOCKS])

## 35.1 Snapshot before expensive processing

For expensive global analysis:

```rust
let snapshot: Vec<(Key, Value)> = map
    .iter()
    .map(|r| (r.key().clone(), r.value().clone()))
    .collect();

let result = snapshot
    .par_iter()
    .map(expensive_analysis)
    .collect::<Vec<_>>();
```

This trades cloning/memory for releasing all shard locks before CPU work. Often that is the right throughput/latency tradeoff in a long-lived service.

## 35.2 Rayon integration

With DashMap's `rayon` feature, parallel iterator support is available. ([DashMap Rayon][D_RAYON])

```toml
[dependencies]
dashmap = { version = "=6.2.1", features = ["rayon"] }
```

Parallel iteration can improve bulk independent work, but it does not make long lock-held callbacks harmless. Keep per-entry work short or clone/snapshot first.

## 35.3 `ReadOnlyView`

A consumed DashMap can be converted into a read-only view:

```rust
let view = map.into_read_only();
```

The read-only view can expose raw references because no concurrent writes occur through the consumed map. ([DashMap read-only view][D_READONLY])

Use when an index has a build/freeze/query lifecycle:

```text
parallel build/mutation phase -> DashMap
freeze boundary              -> into_read_only()
read-mostly serving phase    -> ReadOnlyView
```

This is often better than retaining mutation capability forever.

## 35.4 `DashSet`

DashMap also provides `DashSet` for concurrent set semantics. Use it when only key membership matters rather than storing unit values in `DashMap<K, ()>`.

## 35.5 Raw shard API

With `raw-api`, methods such as `shards`, `shards_mut`, `into_shards`, and shard-determination helpers expose implementation-level control. ([DashMap][D_MAP])

Use cases:

* specialized diagnostics;
* controlled bulk operations;
* integrations that need shard-level access.

Risks:

```text
coupling to internals
manual locking discipline
more deadlock surface
reduced upgrade flexibility
```

## 35.6 Agent checklist

```text
[ ] Treat iter entries as guards.
[ ] Snapshot before expensive global CPU analysis when feasible.
[ ] Enable rayon feature only for real parallel iteration use.
[ ] Consider into_read_only after build/freeze boundary.
[ ] Use DashSet for membership-only state.
[ ] Keep raw-api off by default.
```

---

# DashMap Advanced — 36) Alternatives and shared-state decision framework

## 36.0 Do not default to a concurrent map

The fastest shared mutation is often **no shared mutation**.

Decision order:

```text
Can each worker own local state and reduce/merge later?
  yes -> local HashMap + merge
  no
Can one actor/task own the map and process messages?
  yes -> single owner + channel
  no
Is state read-mostly with occasional wholesale replacement?
  yes -> Arc snapshot / ArcSwap-style architecture
  no
Are accesses naturally partitionable into explicit shards?
  yes -> Vec<Mutex/RwLock<HashMap>>>
  no
Need ergonomic independent-key concurrent mutation?
  -> DashMap
```

## 36.1 Local maps + merge

For batch analysis:

```rust
use rayon::prelude::*;
use std::collections::HashMap;

let counts = items
    .par_iter()
    .fold(HashMap::new, |mut local, item| {
        *local.entry(item.key()).or_default() += 1usize;
        local
    })
    .reduce(HashMap::new, |mut a, b| {
        for (k, v) in b {
            *a.entry(k).or_default() += v;
        }
        a
    });
```

This usually scales better than every item contending on a global concurrent map.

## 36.2 Actor-owned map

```text
many tasks -> bounded channel -> one owner of HashMap
                                -> replies via oneshot if needed
```

Advantages:

* no map locks;
* clear serialization/transaction boundary;
* easier multi-key invariants;
* straightforward audit/event ordering.

Cost: owner can bottleneck; shard actors by key if needed.

## 36.3 Explicit sharding

```rust
struct ShardedMap<K, V> {
    shards: Vec<std::sync::Mutex<std::collections::HashMap<K, V>>>,
}
```

Explicit sharding gives control over shard count, lock implementation, and multi-key ordering, but requires more code than DashMap.

## 36.4 `RwLock<HashMap>`

A single `RwLock<HashMap>` may outperform more complex structures when:

* writes are rare;
* operations are short;
* map size is moderate;
* workload contention is low;
* whole-map snapshots are common.

Always benchmark the simple baseline.

## 36.5 Decision matrix

| Workload | Recommended first design |
|---|---|
| batch frequency count | Rayon local maps + reduce |
| async service cache, independent keys | DashMap or read-mostly snapshot strategy |
| multi-key transactional state | actor or explicit lock with transaction scope |
| static index after startup | build mutable, then read-only/snapshot |
| hot single key | dedicated atomic/lock/actor for that key, not generic DashMap assumption |
| tiny low-contention map | `Mutex<HashMap>` / `RwLock<HashMap>` |
| externally owned state changes | message passing where possible |

## 36.6 Agent invariant

```text
Do not recommend DashMap merely because multiple threads exist.
Recommend it only after establishing a real concurrent shared-map workload.
```

---
# tokio-rayon Advanced — 37) Identity, role, and API surface

## 37.0 Identity

`tokio-rayon` is a deliberately small integration crate that makes Rayon CPU work awaitable from Tokio. Its own documentation motivates the crate by contrasting Tokio's potentially large blocking-thread pool with the fixed-size CPU-oriented Rayon pool. ([tokio-rayon][TR0])

```text
Tokio async task
    |
    | submit CPU closure
    v
Rayon pool
    |
    | result through async handle
    v
await continuation on Tokio
```

It is **not** a replacement for either runtime:

```text
Tokio       -> async I/O/runtime/tasks
Rayon       -> CPU work-stealing pool/data parallelism
tokio-rayon -> bridge returning an awaitable Rayon result
```

## 37.1 Version and dependency posture

This reference pins:

```toml
tokio-rayon = "=2.1.0"
```

The crate was released in 2021 and its manifest declares broad compatible dependencies on Tokio `1.4.0` and Rayon `1.5.0`, so Cargo can resolve modern 1.x Tokio/Rayon versions that satisfy those ranges. ([tokio-rayon manifest][TR_MANIFEST])

Implication:

```text
tokio-rayon version being old != Tokio/Rayon runtime being old
but bridge API evolution has been minimal, so test it explicitly with the pinned modern stack
```

## 37.2 Top-level API

The crate exposes:

```text
spawn
spawn_fifo
AsyncRayonHandle<T>
AsyncThreadPool extension trait
re-export of rayon
```

([tokio-rayon][TR0])

## 37.3 `spawn`

```rust
let result = tokio_rayon::spawn(move || {
    expensive_cpu_work(input)
}).await;
```

The function submits to the **global Rayon pool** with LIFO priority and returns `AsyncRayonHandle<R>`, a `Future<Output = R>`. The closure and result must be `Send + 'static`. Panics in the closure are propagated when the returned future is polled/awaited. ([tokio-rayon spawn][TR_SPAWN])

## 37.4 `spawn_fifo`

```rust
let result = tokio_rayon::spawn_fifo(move || {
    expensive_cpu_work(input)
}).await;
```

This submits with FIFO priority rather than the default LIFO scheduling preference. ([tokio-rayon spawn_fifo][TR_SPAWN_FIFO])

Use FIFO when older queued independent jobs should receive stronger progression/fairness; use normal LIFO when locality/depth-first execution is favorable. Benchmark rather than assuming.

## 37.5 `AsyncRayonHandle<T>`

```text
AsyncRayonHandle<T>: Future<Output = T>
```

It represents the eventual result of a Rayon closure. If the closure panics, polling the handle propagates that panic. ([tokio-rayon handle][TR_HANDLE])

Important lifecycle rule:

```text
Dropping the future is NOT a documented cooperative cancellation mechanism for the CPU closure.
Design explicit cancellation inside long-running CPU work when cancellation matters.
```

The underlying bridge uses a Tokio oneshot receiver to transport a thread result from Rayon to the async future. ([tokio-rayon handle source][TR_HANDLE_SRC])

## 37.6 `AsyncThreadPool`

For a custom Rayon pool:

```rust
use rayon::ThreadPoolBuilder;
use tokio_rayon::AsyncThreadPool;

let pool = ThreadPoolBuilder::new()
    .num_threads(8)
    .thread_name(|i| format!("cpu-{i}"))
    .build()?;

let output = pool
    .spawn_async(move || expensive_cpu_work(input))
    .await;
```

The sealed `AsyncThreadPool` trait is implemented for Rayon `ThreadPool` and provides:

```text
spawn_async      -> LIFO
spawn_fifo_async -> FIFO
```

([tokio-rayon AsyncThreadPool][TR_POOL])

## 37.7 Why custom pool is usually preferable in services

Global Rayon pool:

```rust
let result = tokio_rayon::spawn(work).await;
```

is convenient but shares CPU capacity with every other global Rayon user in the process.

Custom pool:

```text
service CPU class A -> pool A, 8 threads
service CPU class B -> pool B, 4 threads
background work     -> pool C, 2 threads
```

provides:

* explicit thread budget;
* workload isolation;
* naming/observability;
* easier saturation testing;
* reduced interference with libraries that use global Rayon.

## 37.8 Anti-pattern inventory

* Adding tokio-rayon when direct Rayon use already occurs wholly inside synchronous code.
* Assuming `await` makes CPU work cancelable.
* Using the global pool for unrelated latency-critical and background workloads without an admission policy.
* Submitting one Rayon job per tiny item from Tokio instead of one coarse job that uses Rayon internally.
* Nesting tokio-rayon submissions from code already running in Rayon workers without understanding nesting/scheduling.
* Using FIFO purely because it sounds “fairer” without latency/throughput measurement.

## 37.9 Agent checklist

```text
[ ] Use tokio-rayon only at async<->CPU boundary.
[ ] Prefer custom Rayon ThreadPool for service isolation.
[ ] Submit coarse CPU jobs; parallelize internally with Rayon.
[ ] Treat AsyncRayonHandle as result transport, not cancellation token.
[ ] Choose LIFO/FIFO based on workload semantics and tests.
[ ] Add admission control before CPU submission.
```

---

# tokio-rayon Advanced — 38) Scheduling priority, panics, custom pools, and boundary semantics

## 38.0 LIFO vs FIFO intuition

### LIFO default

```text
newly spawned local work tends to run sooner
-> depth-first behavior
-> useful cache locality / bounded working set
```

### FIFO

```text
older local work tends to progress sooner
-> useful for streams of unrelated peer jobs
-> can reduce starvation perception for queued tasks
```

The exact observable behavior is a scheduler policy, not a strict globally ordered queue contract.

## 38.1 Panic propagation

`tokio_rayon::spawn` and `spawn_fifo` document that a panic in the submitted function is propagated through the returned future rather than invoking Rayon's pool panic handler for that task. ([tokio-rayon spawn][TR_SPAWN])

Therefore a service boundary should decide whether a panic:

```text
propagates and crashes task/request
is caught with catch_unwind around untrusted plugin code
is translated into a controlled error
aborts process by policy
```

Do not casually catch panics from trusted internal code and continue in a potentially corrupted logical state.

## 38.2 Fallible result pattern

Return `Result` from the closure:

```rust
let result: Result<Output, ComputeError> = pool
    .spawn_async(move || compute(input))
    .await;

let output = result?;
```

This keeps normal expected failures separate from panics.

## 38.3 Admission-controlled custom pool

```rust
use std::sync::Arc;
use rayon::ThreadPool;
use tokio::sync::Semaphore;
use tokio_rayon::AsyncThreadPool;

struct CpuExecutor {
    pool: Arc<ThreadPool>,
    slots: Arc<Semaphore>,
}

impl CpuExecutor {
    async fn run<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let permit = self.slots.clone().acquire_owned().await.unwrap();
        let pool = Arc::clone(&self.pool);
        let result = pool.spawn_async(f).await;
        drop(permit);
        result
    }
}
```

The semaphore controls **accepted outstanding CPU jobs**, while the Rayon pool controls **simultaneous worker threads**.

These are different levers:

```text
semaphore permits -> queue/admission bound
Rayon threads      -> execution parallelism
```

## 38.4 Where result transfer happens

The bridge makes a synchronous Rayon closure awaitable. Do not put async operations inside that closure and try to `block_on` them unless you have a deliberate runtime-crossing design.

Preferred split:

```rust
let input = fetch_async().await?;
let output = cpu.run(move || analyze(input)).await;
persist_async(output).await?;
```

Not:

```text
Rayon closure -> block_on network I/O -> Rayon thread wasted waiting
```

## 38.5 Result size

Large results are moved from the CPU closure into the async continuation. Prefer owned compact results or `Arc` where appropriate; do not manufacture huge transient copies at the boundary.

For streaming CPU production, a single future returning one huge vector may be a poor architecture. Consider chunked CPU jobs feeding a bounded async channel if progressive output/backpressure is required.

## 38.6 Shutdown

Rayon jobs are not naturally async-cancelled merely because a Tokio request disappears. Long-running kernels should accept cooperative cancellation state where needed:

```rust
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn compute(items: &[Item], cancelled: &AtomicBool) -> Result<Output, Cancelled> {
    for chunk in items.chunks(1024) {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        process(chunk);
    }
    Ok(finish())
}
```

Check at a granularity that balances responsiveness against atomic/check overhead.

---

# Integration Advanced — 39) Canonical Tokio ↔ Rayon architectures

## 39.0 Recommended default architecture

```text
                 Tokio runtime
        ┌────────────┼─────────────┐
        v            v             v
      network      files         timers
        |            |             |
        └────────────┼─────────────┘
                     v
              request/orchestration
                     |
              CPU admission gate
                     |
                     v
              dedicated Rayon pool
        ┌────────────┼────────────┐
        v            v            v
      parse        analyze       hash
        |            |            |
        └────────────┼────────────┘
                     v
                owned result
                     |
                     v
                 Tokio task
                     |
          async write / response
```

## 39.1 Pattern A — one coarse CPU closure

```rust
async fn handle(req: Request, cpu: Arc<CpuExecutor>) -> Result<Response> {
    let bytes = load_input(req).await?;

    let model = cpu.run(move || {
        parse_and_analyze(bytes)
    }).await?;

    save_or_respond(model).await
}
```

Use when one CPU phase can internally exploit Rayon or is itself one serial CPU kernel.

## 39.2 Pattern B — one Rayon job using parallel iterators internally

```rust
let output = cpu.run(move || {
    inputs
        .par_iter()
        .map(analyze)
        .collect::<Vec<_>>()
}).await;
```

This is usually better than submitting N tokio-rayon jobs for N items because it:

* amortizes bridge/admission overhead;
* lets Rayon work-steal directly;
* avoids creating a second application-level queue per item;
* centralizes cancellation/error handling.

## 39.3 Pattern C — many independent requests, bounded CPU admission

```text
request 1 ─┐
request 2 ─┼-> Semaphore(N outstanding CPU jobs) -> Rayon pool(M threads)
request 3 ─┤
...       ─┘
```

This protects async responsiveness when many clients simultaneously request CPU-heavy work.

## 39.4 Pattern D — CPU chunks streaming back to Tokio

```text
Tokio input
   -> CPU chunk job
      -> bounded mpsc output
         -> Tokio serializer/network writer
```

Use when output is naturally incremental and buffering the entire result is undesirable.

Be careful not to call async `Sender::send().await` directly from a Rayon closure. Depending on channel, use an appropriate blocking/synchronous bridge or return chunks to an orchestration thread/task. An alternative is to have the CPU closure produce bounded-size chunk results one job at a time.

## 39.5 Pattern E — separate CPU pools by class

```text
interactive parse pool: 8 threads, low queue bound
batch analysis pool:    24 threads, larger queue bound
background indexing:     4 threads, low priority/FIFO policy
```

Use when latency isolation matters. Total threads must still fit the process/machine budget.

## 39.6 Pattern F — shared global pool

Acceptable for:

* CLI tools;
* batch utilities;
* simple services with one CPU workload class;
* libraries already organized around global Rayon.

Avoid in complex services where unrelated subsystems can saturate one another.

## 39.7 Pattern G — Tokio-only `spawn_blocking`

Use for:

* short/sparse blocking calls;
* unavoidable blocking I/O;
* low-volume CPU work where another dependency/pool is not justified.

For sustained CPU work, Tokio itself recommends limiting CPU-bound use of `spawn_blocking` or using a specialized executor such as Rayon because its blocking pool permits many threads by default. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 39.8 Integration decision table

| Shape | Recommended boundary |
|---|---|
| async network + heavy CPU | Tokio + custom Rayon + tokio-rayon or explicit oneshot bridge |
| pure CPU batch | Rayon only |
| blocking library call from async | Tokio `spawn_blocking` |
| CPU work plus blocking file syscall | separate concerns; don't block Rayon if avoidable |
| many tiny CPU operations | batch/coarsen before Rayon |
| recursive CPU fork/join | Rayon `join`/scope/parallel iterators |
| custom sync pipeline | Crossbeam channels/deques if justified |
| async shared cache | DashMap with short guards or actor/snapshot design |

---

# Integration Advanced — 40) CPU admission control, backpressure, and overload

## 40.0 Parallelism does not imply unbounded admission

A fixed 16-thread Rayon pool can still receive 100,000 queued jobs. Threads are bounded; **memory and queueing latency may not be**.

```text
external demand
   |
   v
ADMISSION LIMIT  <--- must exist somewhere
   |
   v
CPU queue
   |
   v
fixed workers
```

## 40.1 Semaphore admission

```rust
let permit = cpu_slots.clone().acquire_owned().await?;
let result = pool.spawn_async(move || compute(input)).await;
drop(permit);
```

Interpret permits as the maximum number of in-flight accepted CPU requests, not necessarily the number of Rayon threads.

Example:

```text
Rayon threads = 16
CPU permits   = 32
```

allows about one running and one queued request per worker on average. Actual best values are workload-dependent.

## 40.2 Queue-time budget

Measure separately:

```text
admission wait
executor queue wait
CPU service time
end-to-end time
```

If queue wait dominates, adding threads may not help: the workload may be CPU-saturated, memory-bandwidth-limited, or simply overloaded.

## 40.3 Overload responses

A service should choose a policy:

```text
wait with request deadline
reject immediately when permit unavailable
shed low-priority work
coalesce duplicate work
serve stale cache
reduce fidelity / sampling
queue to durable external system
```

Do not let overload policy emerge accidentally from heap growth.

## 40.4 Duplicate-work coalescing

For expensive deterministic computations keyed by input:

```text
first request key K -> launch compute
later request key K -> await same in-flight result
```

This can reduce CPU demand dramatically under hot-key bursts. Implement carefully so failure/cancellation removes in-flight entries.

## 40.5 Bounded channel between stages

Crossbeam/Tokio bounded channels are useful when stages have different throughput. Queue capacity should be deliberately small enough that backpressure arrives before memory and latency explode.

## 40.6 User-provided parallelism parameters

Never accept arbitrary external values directly into:

```text
Rayon num_threads
Tokio max_blocking_threads
DashMap shard count
queue capacity
chunk fanout
```

Clamp against deployment policy.

## 40.7 Backpressure checklist

```text
[ ] Bound accepted CPU work.
[ ] Bound pipeline queues.
[ ] Measure queue/admission wait separately from service time.
[ ] Define overload status/error policy.
[ ] Enforce request deadlines before and during compute where possible.
[ ] Deduplicate/coalesce identical expensive work where valuable.
[ ] Clamp user/config-supplied thread/capacity values.
[ ] Load-test sustained overload, not just peak throughput.
```

---

# Integration Advanced — 41) Shared state vs message passing vs local reduction

## 41.0 Decision hierarchy

Prefer designs in this order when semantics allow:

```text
1. immutable shared data (`Arc<T>`)
2. worker-local mutable state + reduce/merge
3. ownership transfer through channels
4. single-owner actor state
5. sharded synchronized state (DashMap / shard locks)
6. global lock
7. custom lock-free structure
```

This is not a universal performance ordering, but it is a useful complexity/risk default.

## 41.1 Immutable sharing

```rust
let config = Arc::new(config);
```

Best when read-mostly data can be constructed once or replaced wholesale.

## 41.2 Local reduction

```rust
items.par_iter()
    .fold(LocalState::new, |mut s, x| {
        s.update(x);
        s
    })
    .reduce(LocalState::new, |a, b| a.merge(b));
```

Ideal for associative aggregation, indexes assembled in batches, counters, histograms, and many graph preprocessing tasks.

## 41.3 Message passing

```text
producer owns value
   -> send(value)
consumer now owns value
```

Reduces shared aliasing and makes lifecycle/backpressure explicit.

## 41.4 Actor ownership

One Tokio task or OS thread owns a normal `HashMap` and accepts commands:

```rust
enum Command {
    Get { key: Key, reply: oneshot::Sender<Option<Value>> },
    Put { key: Key, value: Value },
}
```

Advantages:

* multi-key invariants serialized naturally;
* no guard deadlock;
* audit/order clear;
* bounded mailbox gives backpressure.

## 41.5 DashMap

Use when high-volume independent keyed accesses really benefit from concurrent shard-level reads/writes and guard discipline is manageable.

## 41.6 Atomics

Use atomics for truly atomic scalar state/counters/flags, not as a reflexive substitute for structural synchronization.

```rust
AtomicBool
AtomicU64
AtomicUsize
```

A contended global atomic increment can become a cache-coherence bottleneck; prefer per-worker counters + reduction for high-rate metrics.

## 41.7 Crossbeam epoch

Use only when implementing lock-free pointer structures and the reclamation complexity is justified.

## 41.8 Agent rule

```text
Before introducing a synchronization primitive, ask whether ownership/dataflow can remove the shared mutation.
```

---

# Integration Advanced — 42) Pipeline, fan-out/fan-in, actor, and staged-compute patterns

## 42.0 Async fan-out for I/O

```rust
let (a, b, c) = tokio::try_join!(
    fetch_a(),
    fetch_b(),
    fetch_c(),
)?;
```

Use for independently awaitable operations. This is concurrency; whether tasks execute simultaneously depends on runtime/thread scheduling and what the futures do.

## 42.1 CPU fan-out

Prefer Rayon:

```rust
let results = inputs
    .par_iter()
    .map(compute)
    .collect::<Vec<_>>();
```

rather than one Tokio task per CPU item.

## 42.2 Mixed staged pipeline

```text
Tokio fetch stage
     |
     v
bounded input batches
     |
     v
Rayon CPU stage
     |
     v
bounded output batches
     |
     v
Tokio persist/serve stage
```

Batching is important: boundaries should carry meaningful chunks, not individual scalar operations.

## 42.3 Scatter / gather

```text
one request
   -> split into independent CPU partitions
      -> Rayon work stealing
   -> gather result
```

If gathering requires a global lock on every item, redesign with local outputs and a final merge.

## 42.4 Actor + Rayon

An actor can own mutable state but offload pure CPU snapshots:

```text
actor owns state
   -> snapshot immutable input
   -> submit CPU compute to Rayon
   -> receive result
   -> validate version / apply result
```

Because state may change while compute runs, attach a generation/version and reject or reconcile stale results.

## 42.5 Incremental indexing pattern

```text
watcher events (Tokio)
    -> debounce/coalesce
    -> determine changed units
    -> bounded CPU admission
    -> parse/analyze changed units (Rayon)
    -> local results
    -> atomic/serialized graph update
    -> notify subscribers (Tokio)
```

Key invariant: expensive analysis does not hold the global index write lock.

## 42.6 Parallel graph update pattern

Bad:

```text
Rayon workers -> mutate one global graph under mutex per edge
```

Better:

```text
Rayon workers -> produce local edge/node deltas
                 |
                 v
          sort/group by destination shard
                 |
                 v
           apply batched updates
```

This reduces lock operations and improves locality.

## 42.7 Fan-in ordering

Parallel outputs are generally not completion-ordered deterministically. If stable ordering matters:

```text
attach sequence/index -> parallel compute -> sort/collect by sequence
```

Do not assume scheduler order.

---

# Integration Advanced — 43) Errors, panics, cancellation, and graceful shutdown

## 43.0 Failure taxonomy

```text
expected domain error     -> Result<T,E>
panic / invariant breach  -> panic policy
request cancellation      -> async cancellation / cooperative CPU token
executor shutdown         -> stop acceptance + drain/cancel policy
channel disconnect        -> ownership/lifecycle signal
resource overload         -> explicit overload error/backpressure
```

## 43.1 Tokio task errors

A Tokio `JoinHandle<T>` yields a `Result<T, JoinError>` when awaited. Distinguish task cancellation from panic, and distinguish those from an inner application `Result`. ([Tokio JoinHandle][T_JOIN])

Canonical shape:

```rust
let handle = tokio::spawn(async move {
    do_async_work().await
});

let application_result = handle.await?; // JoinError handled here
let value = application_result?;        // domain error handled here
```

## 43.2 Rayon panics

Fork/join/parallel iterator panic propagation should be treated as an invariant failure unless the workload explicitly runs untrusted/plugin code. Custom pools can install panic handling for detached spawned work; joined work generally propagates through the invoking API.

## 43.3 tokio-rayon panics

Submitted closure panics are propagated through `AsyncRayonHandle` when polled/awaited. ([tokio-rayon spawn][TR_SPAWN])

## 43.4 Cancellation model by executor

| Work | Cancellation model |
|---|---|
| ordinary async future | dropping future may cancel future progress, subject to operation cancellation safety |
| Tokio spawned task | `abort()` / runtime shutdown; async cooperative polling boundary |
| `spawn_blocking` after started | Tokio documents it generally cannot be aborted; runtime shutdown can wait indefinitely unless timeout policy is used |
| Rayon CPU closure | no general preemptive cancellation; add cooperative token/checks |
| tokio-rayon closure | awaiting future does not imply CPU preemption; add cooperative token |

([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 43.5 Cooperative CPU cancellation

```rust
#[derive(Clone)]
struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
```

Check between meaningful chunks, not every scalar operation.

For Rayon parallel iterators, fallible traversal can short-circuit:

```rust
items.par_iter().try_for_each(|item| {
    if token.is_cancelled() {
        return Err(Cancelled);
    }
    process(item)
})?;
```

Short-circuiting is cooperative; already-running work may continue briefly.

## 43.6 Graceful service shutdown sequence

Recommended conceptual order:

```text
1. mark service draining
2. stop accepting new external requests/jobs
3. signal async background tasks
4. close/drop producer handles where channel disconnect is shutdown signal
5. stop admitting new CPU jobs
6. cancel cooperative long CPU jobs if policy requires
7. await/drain accepted async work to deadline
8. join dedicated worker threads/pools where owned
9. flush persistence/metrics
10. exit
```

## 43.7 Draining vs cancelling

Define per workload:

```text
interactive request after client gone -> usually cancel cooperative compute
committed write/index update          -> often drain to maintain consistency
background refresh                    -> cancel/restart acceptable
financial/transactional mutation      -> completion/rollback protocol required
```

## 43.8 Channel shutdown

Crossbeam channel:

```text
drop all senders -> consumers drain remaining messages then receive disconnect
```

Tokio `mpsc` has its own sender/receiver close/drop semantics. Do not mix implicit channel-drop shutdown with a separate boolean without defining precedence.

## 43.9 Shutdown checklist

```text
[ ] Separate stop-accepting from cancel-running.
[ ] Define which jobs drain and which cancel.
[ ] Give long CPU loops cooperative cancellation points.
[ ] Close channels intentionally.
[ ] Await/join all owned worker lifetimes or document detachment.
[ ] Bound graceful shutdown by service policy.
[ ] Test shutdown under full queues, panics, stalled I/O, and CPU saturation.
```

---
# Integration Advanced — 44) Testing concurrent and parallel Rust

## 44.0 Testing mental model

Concurrency defects are frequently **schedule-dependent**, so a normal green unit test is weaker evidence than in sequential code.

Use multiple layers:

```text
pure deterministic unit tests
    -> synchronization contract tests
        -> repeated stress tests
            -> model/schedule exploration where practical
                -> saturation/load tests
                    -> production observability
```

## 44.1 Separate pure kernel tests from executor tests

Bad testability architecture:

```text
business logic exists only inside tokio::spawn / rayon closure
```

Better:

```rust
fn analyze(input: &Input) -> Result<Output, Error> {
    // deterministic CPU kernel
}

async fn orchestrate(input: Input, cpu: &CpuExecutor) -> Result<Output, Error> {
    cpu.run(move || analyze(&input)).await
}
```

Test `analyze` heavily without concurrency. Test the executor boundary separately.

## 44.2 Tokio async tests

```rust
#[tokio::test]
async fn request_pipeline_returns_result() {
    let result = async_pipeline().await.unwrap();
    assert_eq!(result, expected());
}
```

Use explicit test timeouts for operations that could deadlock/hang:

```rust
let result = tokio::time::timeout(
    std::time::Duration::from_secs(2),
    async_pipeline(),
).await.expect("test timed out");
```

Do not rely on the CI global timeout to diagnose a local concurrency hang.

## 44.3 Test cancellation safety

For every `tokio::select!` loop or timeout boundary, test what happens when a branch loses:

```text
message receive partially progressed?
buffer read state lost?
request remains queued?
permit leaked?
lock guard retained?
```

Tokio documents cancellation-safety behavior for many operations in the `select!` reference; verify the exact operation rather than assuming every future can be freely dropped. ([Tokio select][T_SELECT])

## 44.4 Deterministic Tokio time

When testing timers, prefer paused/test-controlled time where the Tokio feature/API allows it instead of real sleeps. Real wall-clock sleeps create slow, flaky tests.

Test:

```text
timeout before completion
completion before timeout
simultaneous-ready behavior where relevant
shutdown while sleeping
```

## 44.5 Rayon correctness tests

Test parallel kernels against a sequential oracle:

```rust
let sequential = items.iter().map(f).collect::<Vec<_>>();
let parallel = items.par_iter().map(f).collect::<Vec<_>>();
assert_eq!(parallel, sequential);
```

For reductions where order affects floating-point roundoff, compare with an explicit tolerance and document nondeterministic association semantics.

## 44.6 Vary Rayon pool size

A correct parallel algorithm should generally work under different worker counts:

```rust
for threads in [1, 2, 3, 4, 8] {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();

    pool.install(|| {
        assert_correct();
    });
}
```

One-thread tests are valuable because they reveal algorithms that accidentally require simultaneous worker progress, while multi-thread tests reveal races/contention.

## 44.7 Crossbeam channel tests

Test:

```text
bounded full -> sender blocks/try_send reports full
zero-capacity rendezvous
sender drop -> receiver drains then disconnects
receiver drop -> send errors
multiple receivers consume exactly once
select branch behavior
shutdown with full queue
```

## 44.8 DashMap deadlock-oriented tests

Unit tests cannot prove deadlock freedom, but add regression tests around known risk patterns and use explicit time bounds.

Code-review/static rule is stronger:

```text
No DashMap Ref/RefMut/Entry crosses await.
No guard calls arbitrary callback.
No nested map operation while mutable guard is live.
```

## 44.9 Stress tests

Repeated high-contention test shape:

```rust
for _ in 0..1_000 {
    run_parallel_scenario();
}
```

Vary:

* worker counts;
* item counts including 0/1/small/large;
* queue capacities including 0/1;
* artificial yields;
* random task ordering;
* cancellation timing;
* panic/error injection.

Run stress suites separately from fast unit tests if they are expensive.

## 44.10 Model-checking / interpreter tooling

For custom synchronization/atomics/lock-free structures, consider schedule-exploration tools such as Loom and interpreter/UB tools such as Miri, plus platform sanitizers where available. These are especially valuable for Crossbeam-epoch-adjacent custom unsafe code.

Keep the model small: model check the synchronization protocol, not the whole application.

## 44.11 Property tests

Useful properties:

```text
parallel output multiset == sequential output multiset
no accepted job processed more than once
all accepted jobs eventually complete under non-cancel test
count(in) == count(success)+count(error)+count(cancelled)
queue never exceeds explicit capacity
shutdown leaves no outstanding jobs
```

## 44.12 Failure injection

Inject failures at:

```text
before enqueue
after enqueue before start
mid-compute checkpoint
CPU result transfer
async persistence
worker panic
channel disconnect
shutdown initiation
```

Verify resources/permits/guards are released on each path.

## 44.13 Testing matrix

```text
Tokio:
  [ ] spawned task success
  [ ] task panic / JoinError
  [ ] abort/cancellation
  [ ] select cancellation-safe operations
  [ ] timeout
  [ ] semaphore permit release on error/panic path
  [ ] shutdown

Rayon:
  [ ] sequential parity
  [ ] 1-thread pool
  [ ] multi-thread pool
  [ ] empty/singleton/large inputs
  [ ] panic behavior
  [ ] fallible short circuit
  [ ] nested parallelism

Crossbeam:
  [ ] queue/channel capacity boundaries
  [ ] disconnection
  [ ] multi-producer/multi-consumer exactly-once
  [ ] steal Retry handling
  [ ] shutdown/wakeup protocol

DashMap:
  [ ] concurrent independent-key updates
  [ ] entry atomic-at-key logic
  [ ] guard lifetimes do not span await/callback
  [ ] iteration while ordinary traffic continues
  [ ] read-only transition

Integration:
  [ ] CPU saturation does not starve async health endpoint
  [ ] overload rejects/backs up as designed
  [ ] client cancellation stops cooperative CPU work
  [ ] shutdown drains/cancels according to policy
```

---

# Integration Advanced — 45) Benchmarking, profiling, tracing, and regression baselines

## 45.0 Measure the whole system, not only the kernel

A CPU kernel that scales 15x can still make the service worse if it creates:

* queueing delay;
* memory-bandwidth saturation;
* async-runtime starvation;
* allocator pressure;
* lock contention;
* cache thrash;
* oversized result transfer.

Benchmark at three levels:

```text
micro: individual kernel / data structure
meso: executor + queue + kernel
macro: whole service under realistic concurrent traffic
```

## 45.1 Release builds only

```bash
cargo bench
cargo build --release
```

For host-specific benchmarking when deployment permits:

```bash
RUSTFLAGS='-C target-cpu=native' cargo bench
```

Do not compare debug runtime performance to production expectations.

## 45.2 Parallel speedup baseline

For each major CPU kernel, capture:

```text
T1 = one-thread time
TN = N-thread time
speedup = T1 / TN
parallel efficiency = speedup / N
```

Expect diminishing returns. Determine why scaling flattens before increasing thread count.

## 45.3 Thread-count sweep

Measure:

```text
1, 2, 4, 8, 16, ...
physical cores
logical cores
possibly below/above those bounds if justified
```

Record throughput, p50/p95/p99 latency, CPU utilization, memory, context switches, and cache/memory indicators where possible.

## 45.4 Grain-size sweep

Measure batch sizes/chunk sizes because scheduling overhead can dominate tiny units.

```text
work per Rayon task too small -> overhead dominates
work too coarse             -> load imbalance / poor stealing
```

## 45.5 Tokio saturation benchmark

Run a lightweight latency-sensitive async endpoint while simultaneously saturating CPU work.

Success criterion:

```text
CPU load reaches intended limit
health/network/timer p99 remains within service SLO
```

If async latency collapses, inspect:

* CPU admission limit;
* Tokio worker count;
* accidental CPU work on Tokio workers;
* `spawn_blocking` thread explosion;
* OS CPU saturation;
* allocator/global lock contention.

## 45.6 Queue metrics

For each boundary expose:

```text
accepted jobs
rejected jobs
queue depth or outstanding permits
admission wait histogram
queue wait histogram
service time histogram
end-to-end histogram
```

Throughput alone hides queueing collapse.

## 45.7 Rayon pool instrumentation

Name worker threads and, where useful, use pool builder start/exit handlers or surrounding instrumentation to observe worker lifecycle. Keep profiling hooks inexpensive.

## 45.8 Tokio runtime metrics

Tokio exposes runtime metrics APIs and builder hooks in current releases. Treat metrics availability/API shape as version-pinned. Useful signals include worker/queue/task behavior, but OS-level CPU and application queue metrics remain necessary. ([Tokio runtime builder][T_BUILDER])

## 45.9 Tracing spans across boundary

A CPU closure does not automatically preserve every async task-local context. Explicitly capture request/job IDs and instrument the closure:

```text
request_id
cpu_job_id
work_class
input_size
queue_wait_ms
compute_ms
result_size
```

Do not log per-item details in a million-item parallel loop.

## 45.10 Lock/contention profiling

Symptoms:

```text
CPU high but throughput flat
workers frequently sleeping/waiting
hot shard/key
high cache-coherence traffic
```

Test alternatives:

* local reduction;
* more/less sharding;
* actor ownership;
* batch updates;
* shorter guard lifetime;
* remove hot global atomics.

## 45.11 Memory profiling

Watch:

```text
unbounded queues
large per-worker scratch buffers
parallel temporary vectors
DashMap capacity/shard overhead
retired epoch memory under stalled guards
results buffered before async output
```

Peak memory under parallel execution can be approximately per-worker memory multiplied by active workers.

## 45.12 Regression benchmark contract

Store versioned baselines:

```text
crate versions
rustc version
CPU model / host class
thread counts
input dataset/hash
build flags
throughput
latency percentiles
peak RSS
CPU utilization
queue wait
```

Do not declare a crate/runtime upgrade neutral solely because unit tests pass.

---

# Integration Advanced — 46) Production deployment patterns

## 46.0 CLI / local analysis tool

Recommended shape:

```text
Tokio runtime if async I/O needed
Rayon global pool acceptable for one workload class
Crossbeam only if pipeline needs sync channels
DashMap only for real shared incremental state
```

Defaults:

```text
CPU threads near available parallelism
no unbounded producer from external network
release build
human-readable errors
simple shutdown acceptable
```

## 46.1 Long-lived HTTP/gRPC service

```text
Tokio runtime
  -> request admission
  -> async I/O
  -> CPU semaphore
  -> dedicated Rayon pool
  -> response serialization/I/O
```

Require:

```text
bounded CPU admission
request deadlines
cooperative cancellation for long kernels
separate latency/queue/service metrics
thread naming
controlled shutdown
```

Use tokio-rayon or an explicit oneshot bridge to await CPU results.

## 46.2 Incremental code-indexing daemon

```text
filesystem watcher / RPC: Tokio
change coalescing: Tokio
parse/analyze changed files: Rayon
shared active index: snapshot/actor/DashMap depending access pattern
worker-stage queues: bounded Tokio/Crossbeam according to sync/async boundary
query serving: Tokio
```

Strong rules:

* never parse entire repository under a global map lock;
* produce deltas in local memory;
* apply updates in batches;
* attach generation IDs for consistent snapshots;
* bound change backlog;
* coalesce repeated changes to same file.

## 46.3 Batch ETL / scientific analysis

Often simpler:

```text
Rayon first
Tokio only for async remote I/O/orchestration
local reductions instead of DashMap
```

Do not import an async architecture simply because Tokio is popular.

## 46.4 Desktop/background application

Separate foreground responsiveness from background CPU work with a small/dedicated Rayon pool and queue/admission cap. Background indexing should not consume every logical core if UI latency matters.

## 46.5 Multi-tenant service

Potential structure:

```text
global process CPU budget
    -> per-tenant admission limit
    -> work-class pool or common pool
```

Do not create a full `num_cpus` Rayon pool per tenant. Isolation should be achieved primarily through admissions/quotas/work classes, with separate pools only where justified.

## 46.6 Plugin/extension execution

If plugins can panic, loop indefinitely, or allocate unbounded memory, in-process thread pools do not provide security isolation. Use process/container boundaries for hostile/untrusted execution.

Tokio/Rayon/Crossbeam are concurrency tools, not sandboxing mechanisms.

## 46.7 FFI/native-library integration

Before parallelizing calls into native libraries, determine whether they internally spawn threads (BLAS/OpenMP/compression/etc.). Otherwise:

```text
Rayon 32 threads × native library 32 threads = catastrophic oversubscription potential
```

Configure one layer to serial/limited threading where possible.

## 46.8 Deployment topology checklist

```text
[ ] Count Tokio worker threads.
[ ] Count Tokio blocking max/actual behavior.
[ ] Count every Rayon/custom pool.
[ ] Identify third-party internal pools.
[ ] Identify native-library threading.
[ ] Bound external admission.
[ ] Bound queue capacities.
[ ] Reserve capacity for OS/network/latency-critical tasks.
[ ] Validate under cgroup/container CPU quotas, not only host CPU count.
[ ] Test shutdown while saturated.
```

---

# Integration Advanced — 47) Robustness, resource governance, and denial-of-service resistance

## 47.0 Thread pools are not resource limits by themselves

A fixed thread count limits simultaneous execution but not necessarily:

```text
queued jobs
input bytes
result bytes
per-job memory
wall-clock time
recursive work generation
external calls
```

Production robustness requires independent limits.

## 47.1 CPU quota

Controls:

```text
Rayon pool size
CPU admission semaphore
per-tenant outstanding job count
cooperative deadline/cancel checks
work-size validation
```

## 47.2 Memory quota

Controls:

```text
bounded channels
input size limits
chunked result streaming
per-job scratch allocation bounds
cache/index size policy
no unbounded SegQueue/channel for external demand
```

## 47.3 Time quota

Tokio timeout can stop awaiting an async operation, but it does not magically preempt already-running `spawn_blocking` or Rayon CPU closures. Long CPU work needs cooperative checks or stronger process isolation. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 47.4 Fan-out quota

Validate recursive or user-shaped workloads before spawning parallel work:

```text
archive entries
AST nodes
files
URLs
query partitions
graph frontier size
```

Parallel execution can turn a logical amplification bug into a rapid resource spike.

## 47.5 DashMap adversarial hot keys

Sharding does not protect against a workload that concentrates most writes on one key/shard. A hot counter or singleton state should use a purpose-built primitive rather than assuming map sharding solves contention.

## 47.6 Queue abuse

Expose queue saturation as a first-class overload response rather than silently extending latency.

```text
permit not acquired by deadline -> reject/cancel
bounded channel full -> await up to deadline or return overload
```

## 47.7 Panic containment

Panics in worker code can terminate tasks/threads or propagate through joins/handles depending on executor. Decide policy before executing user-supplied callbacks.

Untrusted code should generally not run in the same process merely because a panic can be caught.

## 47.8 Deadlock/liveness governance

Code review should flag:

```text
blocking operation on Tokio worker
Rayon job waiting indefinitely on another queued Rayon job
DashMap guard across await/channel recv/lock
nested locks with inconsistent order
custom queue wait without lost-wakeup proof
shutdown that waits on producer still waiting on full consumer queue
```

## 47.9 Resource-hardening checklist

```text
[ ] CPU admission limit.
[ ] Maximum input/work cardinality.
[ ] Queue/channel capacities.
[ ] Result size policy.
[ ] Per-request deadline.
[ ] Cooperative CPU cancellation for long kernels.
[ ] Cache/index memory policy.
[ ] No user-controlled thread count/shard count without clamping.
[ ] Hot-key stress test.
[ ] Saturated graceful-shutdown test.
[ ] Process isolation for hostile/untrusted execution.
```

---

# Integration Advanced — 48) API stability, upgrades, and migration discipline

## 48.0 Version anchors

This document's released crate anchors are:

```text
Tokio       1.53.1
Rayon       1.12.0
Crossbeam   0.8.4 facade
DashMap     6.2.1
tokio-rayon 2.1.0
```

The Crossbeam direct subcrate versions are separately recorded in §25 because the facade can resolve compatible component versions.

## 48.1 Pinning policy

Application/reproducibility posture:

```toml
[dependencies]
tokio = { version = "=1.53.1", features = ["rt-multi-thread", "macros", "sync", "time"] }
rayon = "=1.12.0"
crossbeam = "=0.8.4"
dashmap = "=6.2.1"
tokio-rayon = "=2.1.0"
```

Whether to use exact manifest pins or semver ranges is an organizational policy, but **commit the lockfile for deployable binaries** and test dependency updates intentionally.

## 48.2 Upgrade surface by crate

### Tokio

Audit:

```text
runtime scheduler/config behavior
feature flags
spawn_blocking/runtime shutdown semantics
task cancellation/select safety
sync primitive fairness/behavior
runtime metrics API
MSRV
```

### Rayon

Audit:

```text
parallel iterator behavior/API
pool builder methods/defaults
worker count defaults/env handling
panic behavior
nested scheduling
MSRV
```

### Crossbeam

Audit both facade and resolved components:

```bash
cargo tree -i crossbeam
cargo tree -i crossbeam-channel
cargo tree -i crossbeam-deque
cargo tree -i crossbeam-epoch
cargo tree -i crossbeam-queue
cargo tree -i crossbeam-utils
```

### DashMap

Audit:

```text
guard/locking warnings
feature defaults
shard construction constraints
raw-api coupling
Rayon integration
serde behavior if enabled
```

### tokio-rayon

Because the bridge has an old release but broad dependency ranges, explicitly run compatibility tests with the modern resolved Tokio and Rayon versions.

## 48.3 Upgrade test gate

```text
1. cargo update in isolated branch
2. inspect Cargo.lock diff
3. cargo tree -d
4. cargo check --all-targets --all-features as appropriate
5. clippy
6. unit/integration tests
7. concurrency stress tests
8. sequential-vs-parallel correctness tests
9. saturation/latency benchmark
10. shutdown/cancellation tests
11. compare thread counts/runtime metrics
12. record performance/memory delta
```

## 48.4 Do not snapshot scheduler accidents

Tests should assert semantic results, not incidental worker order, thread ID, exact steal sequence, or completion order unless the API explicitly guarantees it.

## 48.5 Migration report template

```text
Rust parallel stack upgrade
  Tokio:       x -> y
  Rayon:       x -> y
  Crossbeam:   x -> y
  components:  ...
  DashMap:     x -> y
  tokio-rayon: x -> y
  rustc:       x -> y

API changes:
  ...

Resolved dependency changes:
  ...

Correctness tests:
  ...

Performance:
  throughput ...
  p99 ...
  peak RSS ...
  queue wait ...

Concurrency observations:
  worker count ...
  hot locks/shards ...

Action required:
  ...
```

---

# Integration Advanced — 49) Recipes and executor-selection decision tables

## 49.0 “Parallelize a `Vec<T>` computation”

```rust
use rayon::prelude::*;

let out = items
    .par_iter()
    .map(expensive)
    .collect::<Vec<_>>();
```

Default: **Rayon**.

## 49.1 “Run async requests concurrently”

Use Tokio tasks/futures:

```rust
let (a, b) = tokio::try_join!(fetch_a(), fetch_b())?;
```

Default: **Tokio**.

## 49.2 “Async handler must perform heavy CPU analysis”

```rust
let permit = cpu_slots.acquire().await?;
let result = cpu_pool.spawn_async(move || analyze(input)).await;
drop(permit);
```

Default: **Tokio + Rayon + tokio-rayon**, preferably custom pool.

## 49.3 “Call a blocking filesystem/native function from async”

```rust
let out = tokio::task::spawn_blocking(move || blocking_call()).await??;
```

Default: **Tokio `spawn_blocking`**.

If it becomes sustained CPU-heavy work, migrate to Rayon.

## 49.4 “Many threads need a job queue”

```rust
let (tx, rx) = crossbeam_channel::bounded(capacity);
```

Default: **Crossbeam channel** for synchronous OS threads; Tokio `mpsc` for async tasks.

## 49.5 “Build a custom work-stealing scheduler”

Use `crossbeam_deque::{Injector, Worker, Stealer}` only after proving Rayon cannot express required semantics.

## 49.6 “Concurrent cache/index by key”

Use DashMap when accesses are independent and short:

```rust
let map = DashMap::<Key, Arc<Value>>::new();
```

But evaluate actor ownership, snapshots, or local-merge design first.

## 49.7 “Parallel frequency counting”

Prefer local reduction:

```rust
items.par_iter()
    .fold(HashMap::new, update_local)
    .reduce(HashMap::new, merge_maps)
```

not a global DashMap increment per item.

## 49.8 “One producer must hand off directly to one consumer”

Crossbeam zero-capacity rendezvous:

```rust
let (tx, rx) = crossbeam_channel::bounded(0);
```

for synchronous threads.

## 49.9 “Broadcast latest configuration to async tasks”

Tokio `watch` is generally the natural fit: receivers care about current/latest value rather than every historical message.

## 49.10 “Broadcast every event to async subscribers”

Tokio `broadcast`, with explicit lag handling.

## 49.11 “One async request, one response”

Tokio `oneshot`.

## 49.12 “Limit 50 simultaneous async operations”

Tokio `Semaphore`.

## 49.13 “Limit 16 CPU workers and at most 32 accepted CPU jobs”

```text
Rayon pool threads = 16
Tokio Semaphore permits = 32
```

## 49.14 “Need lock-free queue with fixed capacity and custom wake protocol”

Crossbeam `ArrayQueue`.

If you also need blocking/disconnect/select semantics, use a channel instead.

## 49.15 “Need lock-free pointer structure”

Crossbeam epoch only if a proven existing data structure does not already solve the problem.

## 49.16 “Read-mostly index built once”

Build with ordinary mutable map/DashMap as convenient, freeze to immutable/read-only representation, and serve without ongoing mutation locks.

## 49.17 Compact decision table

| Requirement | First crate/primitive |
|---|---|
| async I/O | Tokio |
| lots of awaitable tasks | Tokio |
| CPU map/filter/reduce | Rayon |
| recursive CPU fork/join | Rayon |
| async -> CPU bridge | tokio-rayon + Rayon |
| sync MPMC channel | Crossbeam channel |
| custom work stealing | Crossbeam deque |
| bounded nonblocking MPMC queue | Crossbeam ArrayQueue |
| lock-free reclamation | Crossbeam epoch |
| concurrent keyed map | DashMap |
| limit async concurrency | Tokio Semaphore |
| read-mostly immutable state | `Arc`/snapshot, often no special concurrent collection |

---

# Integration Advanced — 50) Best-practice and anti-pattern compendium

## 50.0 Executor ownership

Best practice:

```text
Tokio owns async waiting/orchestration.
Rayon owns sustained CPU execution.
Crossbeam supplies sync low-level coordination where justified.
DashMap supplies short-lived independent keyed shared access.
tokio-rayon bridges Tokio to Rayon.
```

Anti-pattern:

```text
“everything is a Tokio task” including long CPU loops
```

## 50.1 Parallelism budget

Best practice:

* inventory every pool/thread source;
* benchmark useful CPU worker count;
* reserve latency headroom;
* account for cgroup/container CPU limits;
* suppress nested native threading where appropriate.

Anti-pattern:

```text
Tokio workers = cores
Rayon pool = cores
custom pool = cores
BLAS = cores
all busy simultaneously
```

## 50.2 Work granularity

Best practice:

```text
coarse enough to amortize scheduling/bridge overhead
fine enough for work stealing/load balance
```

Anti-pattern: spawning one task for a few arithmetic instructions.

## 50.3 Blocking

Best practice:

* `spawn_blocking` for blocking calls from async;
* Rayon for sustained CPU;
* no blocking channel receive on Tokio worker;
* no long I/O in Rayon workers.

Anti-pattern: `std::thread::sleep` or blocking socket/file operation in normal async task.

## 50.4 State

Best practice:

```text
immutable -> local -> message-owned -> actor -> sharded shared -> lock-free
```

Anti-pattern: global `Arc<Mutex<HashMap>>` in every parallel hot loop without profiling.

## 50.5 DashMap

Best practice:

* short guards;
* clone/snapshot before await/CPU work;
* Entry API for initialize/update;
* freeze/read-only phase when possible.

Anti-patterns:

* guard across `.await`;
* nested map calls under `RefMut`;
* callbacks under guard;
* `iter_mut` with other references live;
* assuming distinct keys imply distinct shards.

## 50.6 Crossbeam

Best practice:

* bounded channels for pipelines;
* explicit disconnect/shutdown ownership;
* deque only for custom schedulers;
* epoch only for lock-free pointer structures.

Anti-patterns:

* unbounded external work queue;
* busy-spin on `try_recv`/queue pop;
* custom lost-wakeup-prone queue protocol;
* interpreting `Steal::Retry` as empty;
* using old Crossbeam scoped threads instead of `std::thread::scope` by default.

## 50.7 Tokio

Best practice:

* short nonblocking polls;
* `Semaphore` for bounded concurrency;
* understand `select!` cancellation safety;
* own/join/abort task lifecycles intentionally;
* explicit runtime builder for complex services.

Anti-patterns:

* CPU-heavy future that never yields;
* unlimited `spawn_blocking` CPU work;
* dropping `JoinHandle` and forgetting task continues detached;
* relying on `select!` branch order without `biased;` and an explicit fairness reason;
* holding sync locks across `.await`.

## 50.8 Rayon

Best practice:

* parallel iterators for data parallelism;
* `join`/scope for irregular fork/join;
* local folds/reductions;
* custom pool for isolation;
* release benchmarking.

Anti-patterns:

* blocking I/O in workers;
* Rayon tasks waiting on each other through blocking primitives;
* global shared mutation per item;
* manual static partitioning without load-balance reason;
* pool per request;
* assuming more threads always improves throughput.

## 50.9 tokio-rayon

Best practice:

* bridge coarse jobs;
* custom pool for service workload classes;
* separate CPU admission from worker count;
* explicit cooperative cancellation for long tasks.

Anti-patterns:

* job per scalar item;
* assuming handle drop preempts compute;
* async I/O inside Rayon closure through ad hoc `block_on`;
* unbounded request submission to global Rayon.

## 50.10 Final anti-pattern inventory

```text
Architecture:
  all work forced through one executor
  multiple full-core pools with no budget
  per-request thread pools
  no overload/admission policy

Tokio:
  CPU loops on runtime workers
  blocking recv/lock in async task
  uncontrolled spawn_blocking CPU fanout
  detached tasks accidentally outlive request

Rayon:
  blocking I/O
  global lock/atomic per item
  tiny tasks
  no sequential baseline
  nested library oversubscription

Crossbeam:
  unbounded queue from untrusted producer
  custom scheduler without shutdown/wakeup proof
  epoch reclamation without need
  spin forever

DashMap:
  Ref/RefMut across await
  nested map calls while guard held
  long callback under guard
  whole-map operations in hot path
  concurrent map used where local reduce works

Integration:
  no CPU semaphore/admission
  timeout only cancels waiter, not CPU closure
  no cooperative cancellation
  no queue wait metrics
  no saturated shutdown test

Testing:
  only happy-path unit tests
  exact scheduler ordering assertions
  performance test in debug build
  no 1-thread Rayon test
  no stress/error/cancel injection

Deployment:
  host CPU count used blindly inside container quota
  no native-library thread inventory
  no memory bound for queues/results/scratch
```

---

# Integration Advanced — 51) Final LLM-agent invariants and implementation checklist

## 51.0 Invariant 1 — concurrency is not parallelism

```text
async/concurrent != CPU-parallel by definition
```

Tokio is primarily the async concurrency runtime; Rayon is the default CPU-parallel executor.

## 51.1 Invariant 2 — classify work before choosing an executor

```text
waiting/I/O -> Tokio
CPU-bound   -> Rayon
blocking syscall/library -> spawn_blocking or dedicated blocking thread
custom sync coordination -> Crossbeam when justified
shared keyed map -> DashMap only when shared mutation is truly required
```

## 51.2 Invariant 3 — one process has one finite CPU budget

Every runtime/pool/library draws from the same cores. Count them together.

## 51.3 Invariant 4 — worker count and admitted-work count are different

```text
Rayon threads -> active CPU parallelism
Semaphore / bounded queue -> accepted outstanding work
```

Control both.

## 51.4 Invariant 5 — blocking a worker is a scheduling decision

Blocking a Tokio worker delays unrelated async tasks. Blocking a Rayon worker removes CPU capacity and can deadlock if it waits on same-pool progress. Move blocking to the correct boundary.

## 51.5 Invariant 6 — prefer removing synchronization

Before selecting a concurrent structure:

```text
immutable?
local reduce?
message ownership?
actor?
```

Only then choose shared locks/maps/atomics.

## 51.6 Invariant 7 — DashMap guards are locks in ergonomic clothing

Keep them short. Do not cross `await`. Do not nest map operations under mutable guards.

## 51.7 Invariant 8 — queue capacity is architecture

A bounded queue is a memory/latency/backpressure contract. An unbounded queue requires proof that workload accumulation is bounded elsewhere.

## 51.8 Invariant 9 — cancellation is executor-specific

Async future cancellation does not imply OS-thread/CPU preemption. Long Rayon/spawn_blocking work requires cooperative cancellation or process-level isolation.

## 51.9 Invariant 10 — `spawn_blocking` is not the sustained CPU pool

Tokio documents a large blocking-thread limit because blocking I/O can require many threads; sustained CPU work should be concurrency-limited or moved to a specialized executor such as Rayon. ([Tokio spawn_blocking][T_SPAWN_BLOCKING])

## 51.10 Invariant 11 — Rayon nested parallelism is preferable to nested pools

Use one pool with nested Rayon operations rather than constructing sub-pools per function/request unless isolation semantics explicitly demand separate pools.

## 51.11 Invariant 12 — completion order is not semantic order

Parallel execution order is generally unstable. Attach indices/keys and sort/gather explicitly when stable order matters.

## 51.12 Invariant 13 — lock-free does not mean contention-free

Atomics and lock-free structures still incur cache coherence, retries, memory reclamation, and complexity. Benchmark against locks/local ownership.

## 51.13 Invariant 14 — production optimization is a saturation problem

Microbenchmarks must be complemented by whole-service tests with concurrent demand, queue wait, p99 latency, memory, and shutdown behavior.

## 51.14 Invariant 15 — version-pin call signatures and behavior

Use docs.rs for the released crate version, inspect Cargo.lock, and rerun correctness/performance tests on upgrades.

## 51.15 Final implementation checklist

```text
DEPENDENCIES / BUILD
[ ] Pin/record Tokio, Rayon, Crossbeam, DashMap, tokio-rayon versions.
[ ] Commit Cargo.lock for deployable binaries.
[ ] Record direct Crossbeam component versions if relevant.
[ ] Enable only required features.
[ ] Benchmark release builds.

WORK CLASSIFICATION
[ ] Mark each major operation I/O-bound, CPU-bound, blocking, or shared-state.
[ ] Keep async waiting on Tokio.
[ ] Keep sustained CPU computation on Rayon.
[ ] Put blocking syscalls/library calls behind spawn_blocking/dedicated boundary.

THREAD TOPOLOGY
[ ] Count Tokio workers.
[ ] Count Rayon/custom pool workers.
[ ] Count third-party/native library threads.
[ ] Respect container/cgroup quotas.
[ ] Prevent nested oversubscription.

TOKIO
[ ] No long CPU loop in normal async task.
[ ] Task ownership/JoinHandle lifecycle explicit.
[ ] select! operations checked for cancellation safety.
[ ] Semaphores bound externally driven concurrency.
[ ] Runtime builder used when service needs explicit worker/blocking configuration.

RAYON
[ ] Use par_iter/join/scope rather than manual thread spawning for CPU work.
[ ] Use local fold/reduce instead of hot global mutation.
[ ] Test 1-thread and multi-thread correctness.
[ ] Custom pool used for workload isolation when needed.
[ ] Blocking I/O absent from worker hot paths.

CROSSBEAM
[ ] Bounded channels for bounded-memory pipelines.
[ ] Receiver clone semantics understood as competing consumers.
[ ] Sender-drop shutdown semantics intentional.
[ ] Deque Retry handled correctly.
[ ] Custom scheduler has park/wake/shutdown metrics/proof.
[ ] Epoch used only for real lock-free reclamation needs.

DASHMAP
[ ] Guard scope visibly short.
[ ] No Ref/RefMut/Entry across await.
[ ] No nested map operation under mutable guard.
[ ] Entry API used for atomic-at-key initialization/update.
[ ] Snapshot/local-reduce/actor alternatives considered.
[ ] Raw API disabled unless reviewed.

TOKIO-RAYON
[ ] Bridge used only at async<->CPU boundary.
[ ] CPU work submitted at coarse granularity.
[ ] Custom pool considered for services.
[ ] Separate admission semaphore/queue exists.
[ ] Long work has cooperative cancellation if request can disappear.

BACKPRESSURE / ROBUSTNESS
[ ] CPU admission bounded.
[ ] Every pipeline queue has an explicit capacity policy.
[ ] Input/work cardinality bounded.
[ ] Result memory/size bounded or streamed.
[ ] Overload behavior defined.
[ ] Hot-key/adversarial workload tested.

ERROR / SHUTDOWN
[ ] Domain errors use Result, panics reserved for invariants/policy.
[ ] Tokio JoinError handled separately from inner Result.
[ ] CPU cancellation semantics documented.
[ ] Stop-accepting and cancel-running states separated.
[ ] All owned workers/tasks joined/drained/cancelled intentionally.
[ ] Saturated shutdown tested.

TESTING
[ ] Sequential oracle for parallel kernels.
[ ] Empty/single/small/large inputs.
[ ] Vary worker counts.
[ ] Stress/repetition suite.
[ ] Cancellation/error/panic injection.
[ ] Concurrency model/unsafe tooling for custom atomics/lock-free code.

OBSERVABILITY
[ ] Request/job IDs cross executor boundaries.
[ ] Admission wait measured.
[ ] Queue wait measured.
[ ] CPU service time measured.
[ ] End-to-end p50/p95/p99 measured.
[ ] Worker/thread counts observable.
[ ] Queue depth/outstanding permits observable.
[ ] Memory high-water mark monitored.

PERFORMANCE
[ ] Sequential baseline captured.
[ ] Thread-count sweep captured.
[ ] Grain-size sweep captured.
[ ] Cache/lock/atomic contention investigated if scaling flattens.
[ ] Native nested threading controlled.
[ ] Whole-service latency tested under CPU saturation.
[ ] Performance baseline rerun after crate/rustc upgrades.
```

---

# Source reference index

The document is intentionally anchored to released Rust API documentation. Where `latest` URLs resolve to the pinned release today, the explicit version is still recorded in the prose/manifest examples so an agent does not infer that future `latest` APIs are interchangeable.

[T0]: https://docs.rs/crate/tokio/1.53.1 "Tokio 1.53.1 — Docs.rs"
[T_FEATURES]: https://docs.rs/crate/tokio/1.53.1/source/Cargo.toml "Tokio 1.53.1 Cargo.toml / features"
[T_TASK]: https://docs.rs/tokio/1.53.1/tokio/task/index.html "tokio::task"
[T_BLOCK]: https://docs.rs/tokio/1.53.1/tokio/index.html "Tokio crate documentation"
[T_SPAWN_BLOCKING]: https://docs.rs/tokio/1.53.1/tokio/task/fn.spawn_blocking.html "tokio::task::spawn_blocking"
[T_BLOCK_IN_PLACE]: https://docs.rs/tokio/1.53.1/tokio/task/fn.block_in_place.html "tokio::task::block_in_place"
[T_JOIN]: https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinHandle.html "tokio::task::JoinHandle"
[T_SELECT]: https://docs.rs/tokio/1.53.1/tokio/macro.select.html "tokio::select!"
[T_BUILDER]: https://docs.rs/tokio/1.53.1/tokio/runtime/struct.Builder.html "tokio::runtime::Builder"
[T_SYNC]: https://docs.rs/tokio/1.53.1/tokio/sync/index.html "tokio::sync"
[T_ONESHOT]: https://docs.rs/tokio/1.53.1/tokio/sync/oneshot/index.html "tokio::sync::oneshot"
[T_BROADCAST]: https://docs.rs/tokio/1.53.1/tokio/sync/broadcast/index.html "tokio::sync::broadcast"
[T_LOCAL]: https://docs.rs/tokio/1.53.1/tokio/task/struct.LocalSet.html "tokio::task::LocalSet"
[T_IO]: https://docs.rs/tokio/1.53.1/tokio/io/index.html "tokio::io"
[T_IO_COPY]: https://docs.rs/tokio/1.53.1/tokio/io/fn.copy.html "tokio::io::copy"


[R0]: https://docs.rs/crate/rayon/1.12.0 "Rayon 1.12.0 — Docs.rs"
[R_FEATURES]: https://docs.rs/crate/rayon/1.12.0/features "Rayon 1.12.0 features"
[R_JOIN]: https://docs.rs/rayon/1.12.0/rayon/fn.join.html "rayon::join"
[R_ITER]: https://docs.rs/rayon/1.12.0/rayon/iter/index.html "rayon::iter"
[R_PAR_ITER]: https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html "ParallelIterator"
[R_SCOPE]: https://docs.rs/rayon/1.12.0/rayon/fn.scope.html "rayon::scope"
[R_SPAWN]: https://docs.rs/rayon/1.12.0/rayon/fn.spawn.html "rayon::spawn"
[R_POOL_BUILDER]: https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPoolBuilder.html "rayon::ThreadPoolBuilder"
[R_POOL]: https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPool.html "rayon::ThreadPool"
[R_IN_PLACE_SCOPE]: https://docs.rs/rayon/1.12.0/rayon/fn.in_place_scope.html "rayon::in_place_scope"
[R_YIELD]: https://docs.rs/rayon/1.12.0/rayon/fn.yield_now.html "rayon::yield_now"
[R_YIELD_LOCAL]: https://docs.rs/rayon/1.12.0/rayon/fn.yield_local.html "rayon::yield_local"

[C0]: https://docs.rs/crate/crossbeam/0.8.4 "Crossbeam 0.8.4 — Docs.rs"
[C_FEATURES]: https://docs.rs/crate/crossbeam/0.8.4/features "Crossbeam 0.8.4 features"
[C_CHANNEL]: https://docs.rs/crossbeam/0.8.4/crossbeam/channel/index.html "crossbeam::channel"
[C_DEQUE]: https://docs.rs/crossbeam/0.8.4/crossbeam/deque/index.html "crossbeam::deque"
[C_QUEUE]: https://docs.rs/crossbeam/0.8.4/crossbeam/queue/index.html "crossbeam::queue"
[C_EPOCH]: https://docs.rs/crossbeam/0.8.4/crossbeam/epoch/index.html "crossbeam::epoch"
[C_UTILS]: https://docs.rs/crossbeam/0.8.4/crossbeam/utils/index.html "crossbeam::utils"
[C_SCOPE]: https://docs.rs/crossbeam/0.8.4/crossbeam/fn.scope.html "crossbeam::scope"
[C_BACKOFF]: https://docs.rs/crossbeam/0.8.4/crossbeam/utils/struct.Backoff.html "crossbeam::utils::Backoff"
[C_CH_LATEST]: https://docs.rs/crate/crossbeam-channel/0.5.16 "crossbeam-channel 0.5.16"
[C_DQ_LATEST]: https://docs.rs/crate/crossbeam-deque/0.8.7 "crossbeam-deque 0.8.7"
[C_EP_LATEST]: https://docs.rs/crate/crossbeam-epoch/0.9.20 "crossbeam-epoch 0.9.20"
[C_Q_LATEST]: https://docs.rs/crate/crossbeam-queue/0.3.13 "crossbeam-queue 0.3.13"
[C_U_LATEST]: https://docs.rs/crate/crossbeam-utils/0.8.22 "crossbeam-utils 0.8.22"

[C_ATOMIC]: https://docs.rs/crossbeam/0.8.4/crossbeam/atomic/struct.AtomicCell.html "crossbeam::atomic::AtomicCell"
[C_PARKER]: https://docs.rs/crossbeam/0.8.4/crossbeam/sync/struct.Parker.html "crossbeam::sync::Parker"
[C_SHARDED]: https://docs.rs/crossbeam/0.8.4/crossbeam/sync/struct.ShardedLock.html "crossbeam::sync::ShardedLock"
[C_WAITGROUP]: https://docs.rs/crossbeam/0.8.4/crossbeam/sync/struct.WaitGroup.html "crossbeam::sync::WaitGroup"

[D0]: https://docs.rs/crate/dashmap/6.2.1 "DashMap 6.2.1 — Docs.rs"
[D_FEATURES]: https://docs.rs/crate/dashmap/6.2.1/features "DashMap 6.2.1 features"
[D_MAP]: https://docs.rs/dashmap/6.2.1/dashmap/struct.DashMap.html "DashMap"
[D_ENTRY]: https://docs.rs/dashmap/6.2.1/dashmap/mapref/entry/enum.Entry.html "DashMap Entry"
[D_READONLY]: https://docs.rs/dashmap/6.2.1/dashmap/struct.ReadOnlyView.html "DashMap ReadOnlyView"
[D_RAYON]: https://docs.rs/dashmap/6.2.1/dashmap/rayon/index.html "DashMap Rayon integration"
[D_LOCKS]: https://docs.rs/dashmap/6.2.1/dashmap/struct.DashMap.html "DashMap locking behavior warnings"

[TR0]: https://docs.rs/crate/tokio-rayon/2.1.0 "tokio-rayon 2.1.0 — Docs.rs"
[TR_MANIFEST]: https://docs.rs/crate/tokio-rayon/2.1.0/source/Cargo.toml.orig "tokio-rayon 2.1.0 manifest"
[TR_SPAWN]: https://docs.rs/tokio-rayon/2.1.0/tokio_rayon/fn.spawn.html "tokio_rayon::spawn"
[TR_SPAWN_FIFO]: https://docs.rs/tokio-rayon/2.1.0/tokio_rayon/fn.spawn_fifo.html "tokio_rayon::spawn_fifo"
[TR_POOL]: https://docs.rs/tokio-rayon/2.1.0/tokio_rayon/trait.AsyncThreadPool.html "tokio_rayon::AsyncThreadPool"
[TR_HANDLE]: https://docs.rs/tokio-rayon/2.1.0/tokio_rayon/struct.AsyncRayonHandle.html "tokio_rayon::AsyncRayonHandle"
[TR_HANDLE_SRC]: https://docs.rs/tokio-rayon/2.1.0/src/tokio_rayon/async_handle.rs.html "AsyncRayonHandle source"
