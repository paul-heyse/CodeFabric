# gix in Rust — advanced technical reference / feature and deployment catalog for LLM coding agents



Modeled after an exhaustive advanced-reference format: an upfront capability map followed by self-contained deep-dive sections. This document is written for coding agents that must choose APIs, Cargo features, concurrency models, security posture, and deployment patterns without silently assuming Git porcelain behavior that the library does not yet provide.



## Version / source anchors



**Release anchor:** `gix = 0.86.0` (current released crate at the time of this reference, August 2026). The released package uses Rust edition 2024 and declares **MSRV Rust 1.85**. The reference intentionally distinguishes release-pinned `docs.rs` / released `Cargo.toml` facts from the continuously changing `main` branch and `crate-status.md`.



**Security floor:** do not deploy `gix <= 0.85.0` for Windows-capable incremental checkout. GHSA-pmm9-4h7q-24c8 was published 2026-08-02; `gix >= 0.86.0`, `gix-features >= 0.49.0`, `gix-worktree >= 0.55.0`, and `gix-worktree-state >= 0.33.0` contain the fix. The affected low-level path could follow an existing terminal symlink/reparse point during non-exclusive checkout on Windows and write outside the worktree.



### Source-of-truth hierarchy



| Priority | Source | Use |

|---:|---|---|

| 1 | released `gix 0.86.0` rustdoc / docs.rs | exact public methods, feature gates, type signatures |

| 2 | released `gix 0.86.0` `Cargo.toml.orig` | exact MSRV, feature graph, dependency versions |

| 3 | Gitoxide `crate-status.md` | workflow completeness, known missing plumbing/porcelain |

| 4 | Gitoxide security advisories | security floors and affected call paths |

| 5 | Gitoxide README / stability guidance | project-level intent, CLI stability, ecosystem posture |

| 6 | `main` branch source | forward-looking only; never treat as released API |



Primary anchors used throughout:



* [S1] `gix 0.86.0` crate docs: https://docs.rs/gix/0.86.0/gix/

* [S2] `gix 0.86.0` released Cargo manifest: https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* [S3] `gix 0.86.0` feature flags: https://docs.rs/crate/gix/0.86.0/features

* [S4] `Repository` rustdoc: https://docs.rs/gix/0.86.0/gix/struct.Repository.html

* [S5] all public items: https://docs.rs/gix/0.86.0/gix/all.html

* [S6] Gitoxide crate/workflow status: https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* [S7] Gitoxide README: https://github.com/GitoxideLabs/gitoxide/blob/main/README.md

* [S8] GHSA-pmm9-4h7q-24c8: https://github.com/GitoxideLabs/gitoxide/security/advisories/GHSA-pmm9-4h7q-24c8

* [S9] `PrepareFetch`: https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareFetch.html

* [S10] `PrepareCheckout`: https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareCheckout.html

* [S11] `Remote`: https://docs.rs/gix/0.86.0/gix/struct.Remote.html

* [S12] status module: https://docs.rs/gix/0.86.0/gix/status/index.html



---



## Feature inventory: what this reference covers



The `gix` facade is the application-oriented hub over the Gitoxide plumbing crates. This reference covers repository discovery/open/init; trust and configuration; object IDs and hash algorithms; objects and the object database; blobs, trees, commits, tags, actors and signatures; references, HEAD, reflogs and reference edits; revision parsing and walking; commit-graph acceleration; index reading/writing; pathspecs, attributes, ignores, directory walking and filter pipelines; status; blob/tree diffs and rename tracking; merge primitives; blame and mailmap; submodules; worktrees; checkout/worktree mutation; worktree streams and archives; remotes, refspecs and URL rewriting; credentials, external commands and prompts; transports and Git protocol; fetch, negotiation and shallow repositories; clone; pack/ODB internals; progress, tracing and interruption; performance and caches; error handling; thread-safety and async integration; testing; security; platform differences; deployment patterns; the `gix-*` crate topology; migration from `git2`; current workflow gaps; upgrade policy; and final LLM-agent decision rules.



A core rule is repeated throughout: **the existence of plumbing does not imply that the corresponding `git` porcelain workflow is complete.** For example, fetching is implemented at a useful high level, while full push workflow plumbing and high-level checkout/switch/restore/reset orchestration remain tracked as incomplete in the project status document.



---



# Proposed comprehensive documentation map



## 0) Scope, versioning, and the gix mental model

## 1) Installation, crate selection, Cargo features, and Rust project layout

## 2) First executable applications: discover, inspect HEAD, resolve revisions

## 3) Repository handle model, attached vs detached objects, and thread safety

## 4) Repository discovery, open, init, bare repositories, and trust

## 5) Configuration, environment, command context, and trust-sensitive values

## 6) Repository layout: git-dir, work-dir, common-dir, namespaces, and state

## 7) Object IDs, hash algorithms, SHA-1/SHA-256 feature policy

## 8) Object database: lookup, headers, writing, caching, and in-memory writes

## 9) Blob objects and binary-safe content handling

## 10) Tree objects, traversal, entry lookup, and tree editing

## 11) Commit objects, actors, messages, signatures, parents, and creation inputs

## 12) Tag objects and annotated/lightweight tag handling

## 13) References, HEAD, symbolic refs, reflogs, edits, and transactions

## 14) Revision specifications, rev-parse behavior, describe, and object peeling

## 15) Revision walking, commit graphs, merge bases, and graph caches

## 16) The Git index: load, create from tree, stages, write, and integrity

## 17) Pathspecs, glob/path normalization, case behavior, and search

## 18) Attributes, ignores, excludes, directory walking, and worktree stacks

## 19) Clean/smudge filter pipelines and external process boundaries

## 20) Repository status: index↔worktree, tree↔index, untracked, submodules

## 21) Blob and tree diff, line diff, rewrite/rename tracking, and diff caches

## 22) Merge primitives: blob, tree, commit merge, options, conflicts, limitations

## 23) Blame and mailmap

## 24) Submodules: discovery, configuration, activation, open, and status

## 25) Main and linked worktrees

## 26) Checkout/worktree mutation, low-level semantics, and Windows safety

## 27) Worktree byte streams and archives

## 28) Remotes, remote names, URLs, URL rewriting, and refspecs

## 29) Credentials, prompts, command spawning, and execution permissions

## 30) Transport and protocol: file, git, SSH, HTTP(S), sync/async feature choices

## 31) Fetch: ref mapping, negotiation, shallow behavior, ref updates, and packs

## 32) Clone builders, fetch-then-checkout, cleanup semantics, and persistence

## 33) Push: current API surface and the critical workflow-completeness gap

## 34) Creating commits and mutating objects/trees/refs/index safely

## 35) Packfiles, multi-pack indexes, ODB internals, alternates, compression

## 36) Locks, tempfiles, filesystem snapshots, and atomicity boundaries

## 37) Progress, tracing, interruption, signals, and cancellation

## 38) Error model, error chains, diagnostics, and application context

## 39) Concurrency, `parallel`, ThreadSafeRepository, blocking/async integration

## 40) Performance tuning: object cache, pack caches, commit graph, parallelism

## 41) Testing and correctness: fixtures, Git parity, adversarial repositories

## 42) Cross-platform behavior: Linux/macOS/Windows, symlinks, executable bits

## 43) Security and governance for untrusted repositories and remotes

## 44) Production deployment patterns in Rust

## 45) Complete `gix-*` plumbing-crate architecture catalog

## 46) Migration from `git2` / libgit2 and API-equivalence strategy

## 47) Capability matrix: implemented plumbing vs incomplete porcelain workflows

## 48) API stability, upgrades, MSRV, feature drift, and release migration

## 49) CLI `gix` / `ein` as debugging tools, not stable scripting contracts

## 50) Final best practices, anti-patterns, and LLM-agent execution playbook



---



# Suggested expansion / reading order



1. **0–4:** identity, dependency policy, first programs, repository handles, open/discover/init.

2. **5–8:** config/trust, layout, hashes, ODB.

3. **9–16:** Git data model: blob/tree/commit/tag/refs/revisions/index.

4. **17–22:** path matching, worktree metadata, status, diff, merge.

5. **23–27:** blame/mailmap/submodules/worktrees/checkout/archive.

6. **28–33:** remotes, credentials, transports, fetch, clone, and push limitations.

7. **34–40:** mutation, pack/ODB, locks, progress, errors, concurrency, performance.

8. **41–50:** testing, platforms, security, deployment, crate topology, migration, gaps, upgrades, CLI, final agent rules.



---



# gix Advanced — 1A) Release-pinned feature flag catalog



`gix 0.86.0` exposes **55 Cargo feature flags; 30 are enabled by the default bundle**. The released manifest is the authority for feature relationships. Default = `max-performance-safe + comfort + basic + extras + auto-chain-error + sha1`. Network-client features are intentionally not selected by default because async/blocking transport families and TLS implementations are mutually exclusive policy choices for the final application. [S2][S3]



| Feature | Category | Pulls in / relationship | Agent meaning |

|---|---|---|---|

| `default` | bundle | max-performance-safe, comfort, basic, extras, auto-chain-error, sha1 | batteries-included local/non-network functionality |

| `basic` | bundle | blob-diff, revision, index | fundamental repository analysis |

| `extras` | bundle | worktree-stream, worktree-archive, revparse-regex, mailmap, excludes, attributes, worktree-mutation, credentials, interrupt, status, dirwalk, blame | broad application surface |

| `need-more-recent-msrv` | bundle | merge, tree-editor | legacy-named bundle; release package itself already declares Rust 1.85 |

| `comfort` | bundle | human/byte progress units | better progress rendering |

| `sha1` | hash | SHA-1 throughout stack | default Git object format |

| `sha256` | hash | SHA-256 throughout stack | hash support; do not infer complete Git SHA-256 repository parity |

| `command` | component | gix-command | Git-like external command spawning |

| `status` | component | gix-status + dirwalk + index + blob-diff | git-status-like plumbing |

| `interrupt` | component | signal-hook + tempfile signal cleanup | interruptible long operations |

| `index` | component | gix-index | read/write .git/index |

| `dirwalk` | component | gix-dir + attributes + excludes | Git-aware worktree walks |

| `credentials` | component | gix-credentials + gix-prompt + gix-negotiate | credential helpers and fetch negotiation dependencies |

| `worktree-mutation` | component | attributes + gix-worktree-state | checkout/reset-like low-level worktree mutation |

| `excludes` | component | gix-ignore + gix-worktree + index | exclude/ignore stacks |

| `tree-editor` | component | tree editor API | convenient tree edits |

| `attributes` | component | pathspec/filter/attributes/submodule + command | gitattributes, pathspec, filters, submodules |

| `mailmap` | component | gix-mailmap + revision | identity canonicalization |

| `revision` | component | describe + merge_base + index | revspec/revision support |

| `revparse-regex` | component | regex + revision | full regex revspec syntax |

| `blob-diff` | component | gix-diff/blob + attributes | line/blob diff and rename support |

| `merge` | component | tree-editor + blob-diff + gix-merge + attributes | blob/tree/commit merge primitives |

| `blame` | component | gix-blame + blob-diff | blame plumbing |

| `worktree-stream` | component | gix-worktree-stream + attributes | tree-to-worktree byte stream |

| `worktree-archive` | component | gix-archive + stream + attributes | archive generation; container formats require explicit gix-archive features |

| `async-network-client` | network | gix-protocol async client + transport | async protocol client base |

| `async-network-client-async-std` | network | async-network-client + async-std transport | async-std integration |

| `blocking-network-client` | network | blocking protocol + file/git/ssh + credentials | blocking fetch/clone base |

| `blocking-http-transport-curl` | network | blocking-network-client + curl HTTP | HTTP/S transport base via curl |

| `blocking-http-transport-curl-rustls` | network | curl + rustls | HTTPS without OpenSSL |

| `blocking-http-transport-curl-openssl` | network | curl + OpenSSL | HTTPS via OpenSSL |

| `blocking-http-transport-reqwest` | network | blocking-network-client + reqwest | HTTP transport via reqwest; reqwest defaults off |

| `blocking-http-transport-reqwest-rust-tls` | network | reqwest + rustls | HTTPS via reqwest/rustls |

| `blocking-http-transport-reqwest-rust-tls-trust-dns` | network | reqwest/rustls + trust-dns | alternate DNS stack |

| `blocking-http-transport-reqwest-native-tls` | network | reqwest + native-tls | platform/native TLS |

| `max-control` | performance | parallel + both pack LRU caches | performance knobs without extra compatibility-affecting choices |

| `max-performance-safe` | performance | max-control | deprecated name in 0.86 manifest; now equivalent path to max-performance |

| `max-performance` | performance | max-performance-safe | all performance features |

| `parallel` | performance | thread-safe structures + multithreaded algorithms | also minimum feature for Repository Send when defaults are disabled |

| `pack-cache-lru-static` | performance | fixed-size allocation-free pack LRU | low-overhead pack cache |

| `pack-cache-lru-dynamic` | performance | memory-capped hash-map LRU | dynamic pack cache |

| `hp-tempfile-registry` | performance | high-performance concurrent tempfile registry | massively parallel tempfile workloads |

| `auto-chain-error` | error | gix-error auto-chain | anyhow-friendly chained errors |

| `tree-error` | error | opposite error organization | overrides auto-chain behavior |

| `tracing` | observability | coarse tracing | production spans/events |

| `tracing-detail` | observability | detailed + coarse tracing | deep diagnostics; higher volume |

| `serde` | interop | Serialize/Deserialize across supported gix types | structured persistence/telemetry |

| `progress-tree` | observability | prodash progress tree root | render nested progress |

| `cache-efficiency-debug` | diagnostic | cache-use diagnostics | tune object/pack caches |

| `document-features` | docs | feature documentation helper | documentation build |



**Feature invariant:** at least one hash algorithm must be enabled. If `default-features = false`, explicitly select `sha1` and/or `sha256`. [S2]



---



---



# gix Advanced — 0) Scope, versioning, and the gix mental model



## 0.0 Scope and mental model



Treat `gix` as the application-facing facade over Gitoxide plumbing crates. `Repository` is the hub. It aims to behave like Git where implemented, but it is not a drop-in porcelain implementation of every Git command.



**Feature gate / dependency posture:** `core / always available`.




### 0.1 Identity: what `gix` is and is not

`gix` is the application-facing crate of Gitoxide. The crate documentation describes `Repository` as the hub into Git functionality and explicitly positions the facade as more convenient than wiring the plumbing crates directly without intending to sacrifice meaningful performance for most applications. The Gitoxide project itself is a Git implementation written in Rust, but the project does **not** claim that every Git command has a complete high-level workflow today.

For an LLM agent, “can gix do X?” must be decomposed into two questions:

1. Does the required **plumbing** exist — e.g. read refs, decode objects, diff trees, merge blobs, write an index, fetch a pack?
2. Does the full **workflow orchestration** exist — e.g. `git switch`, `git merge --continue`, `git rebase`, `git push` with all state/hooks/config semantics?

The answer can be yes to the first and no to the second. That distinction is more important with `gix` than with a mature command wrapper because the plumbing surface is intentionally rich and exposed.

### 0.2 Core object graph

```text
Repository
  ├─ configuration + trust + filesystem capabilities
  ├─ RefStore
  │    ├─ HEAD
  │    ├─ branches / tags / remotes
  │    └─ reflogs / transactions
  ├─ OdbHandle
  │    ├─ loose objects
  │    ├─ packs / indexes / MIDX
  │    ├─ alternates / replacements
  │    └─ object caches
  ├─ worktree/index
  │    ├─ .git/index
  │    ├─ status / dirwalk
  │    ├─ attributes / ignore / filters
  │    └─ checkout plumbing
  └─ network
       ├─ Remote / refspecs / URL rewriting
       ├─ credentials / transport
       ├─ protocol / negotiation
       └─ fetch / clone
```

### 0.3 Minimum vocabulary

| Term | Meaning | Agent use |
|---|---|---|
| `Repository` | thread-local high-level handle | default entry point |
| `ThreadSafeRepository` | thread-safe container around repository resources | shared service state / cross-thread re-open |
| `ObjectId` | owned hash identifier | persistent object identity |
| `Id<'repo>` | object ID attached to a repository | convenient lookup/traversal |
| `Object<'repo>` | decoded object + repo association | kind conversion / peeling |
| `ObjectDetached` | standalone decoded object | cross-thread / cross-lifetime payload |
| `Commit`, `Tree`, `Blob`, `Tag` | typed attached object wrappers | semantic object access |
| `Reference<'repo>` | snapshot of a ref + repo | peel/read/mutate ref |
| `Remote<'repo>` | remote configuration + repo | connect/fetch configuration |
| `Worktree<'repo>` | worktree view | per-worktree index/excludes |

### 0.4 LLM invariant

Never choose an API by matching a Git command name alone. First identify the state domains the command touches: refs, objects, index, worktree, config, remote transport, hooks, and transient operation-state files. If more than one domain must change, explicitly verify that `gix` provides the orchestration; otherwise compose plumbing deliberately or use a fallback.

## 0.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository` | main high-level repository handle |

| `ThreadSafeRepository` | thread-safe repository resource container |

| `ObjectId` | owned object hash identity |

| `Id` | repository-attached object identity |

| `Object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote` | configured/unnamed remote abstraction |

| `Worktree` | worktree-specific view |



## 0.2 Canonical pattern



```text
gix facade
  Repository hub
    -> refs / HEAD / reflogs
    -> object database / blobs / trees / commits / tags
    -> revision parsing / walking / commit graph
    -> index / worktree / status / diff
    -> config / trust / credentials / remotes
    -> fetch / clone
    -> lower-level gix-* plumbing as needed
```



## 0.3 Value cases



* **repository inspection and code-intelligence tooling.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **high-performance history/object/ref analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **custom Git-aware services.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **controlled mutation workflows built from plumbing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 0.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 0.5 Failure modes and edge cases



* **assuming command-level parity from type names.**

* **using `main` docs with a released crate.**

* **deploying pre-0.86 checkout code on Windows.**

* **treating Git object/ref data as UTF-8.**



## 0.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 0.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 0.8 Anti-pattern inventory



* **Avoid:** assuming command-level parity from type names.

* **Avoid:** using `main` docs with a released crate.

* **Avoid:** deploying pre-0.86 checkout code on Windows.

* **Avoid:** treating Git object/ref data as UTF-8.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 0.9 Agent checklist



```text
[ ] Pin `gix = "=0.86.0"` for reproducible agent work
[ ] Use docs.rs for exact signatures
[ ] Consult crate-status before implementing a porcelain workflow
[ ] Preserve Git byte-string semantics
[ ] Set explicit security/trust policy for untrusted repositories
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 0.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 1) Installation, crate selection, Cargo features, and Rust project layout



## 1.0 Scope and mental model



The compile-time feature graph is architecture, not decoration. `gix` defaults are broad, but network policy is deliberately opt-in. Library crates should start lean; applications can select performance and transport features deliberately.



**Feature gate / dependency posture:** `Cargo feature graph`.




### 1.1 Release manifest facts

`gix 0.86.0` declares edition 2024 and `rust-version = "1.85"`. The default feature bundle is broad and intentionally optimized for applications. The released manifest tells **library developers** to start with `default-features = false` and add only components they need, and advises libraries not to force performance options on dependents.

### 1.2 Dependency profiles

**General application, local repositories:**

```toml
[dependencies]
gix = "=0.86.0"
```

**Networked application, HTTPS via reqwest + rustls:**

```toml
[dependencies]
gix = { version = "=0.86.0", features = [
  "blocking-http-transport-reqwest-rust-tls",
] }
```

Because this HTTP feature stacks on `blocking-network-client`, it also activates the blocking Git protocol client and the attributes/credentials needed by network workflows.

**Lean library:**

```toml
[dependencies]
gix = { version = "0.86", default-features = false, features = [
  "sha1",
  "revision",
  "index",
] }
```

For a reusable library, prefer a compatible version requirement and avoid forcing transport/TLS/performance features unless your public API actually requires them. For an LLM-agent reference application or reproducible production binary, exact pinning is safer.

### 1.3 Workspace feature unification warning

Cargo unifies features for a package across the resolved graph. A “lean” internal crate and a “networked” sibling may therefore produce a `gix` build with the union. Do not reason about one `Cargo.toml` in isolation; inspect the resolved feature graph.

```bash
cargo tree -e features -i gix
cargo tree -d
cargo metadata --format-version 1 > metadata.json
```

### 1.4 Application vs library policy

| Consumer | Recommended posture |
|---|---|
| binary / service | exact pin or tightly managed lockfile; select transport + performance intentionally |
| reusable library | `default-features = false`; expose your own feature forwarding; avoid selecting TLS/runtime policy |
| internal workspace | centralize version in `[workspace.dependencies]`; inspect feature union |
| plugin ABI boundary | hide gix types behind your own types if independently versioned plugins are expected |

## 1.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `default-features` | part of this capability surface; verify exact signature in pinned rustdoc |

| `basic` | part of this capability surface; verify exact signature in pinned rustdoc |

| `extras` | part of this capability surface; verify exact signature in pinned rustdoc |

| `parallel` | part of this capability surface; verify exact signature in pinned rustdoc |

| `blocking-network-client` | part of this capability surface; verify exact signature in pinned rustdoc |

| `async-network-client` | part of this capability surface; verify exact signature in pinned rustdoc |

| `serde` | part of this capability surface; verify exact signature in pinned rustdoc |

| `tracing` | part of this capability surface; verify exact signature in pinned rustdoc |



## 1.2 Canonical pattern



```toml
[dependencies]
gix = "=0.86.0"

# Networked clone/fetch using blocking protocol + HTTPS via reqwest/rustls:
# gix = { version = "=0.86.0", features = ["blocking-http-transport-reqwest-rust-tls"] }
```



## 1.3 Value cases



* **read-only local analyzer.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **networked fetch/clone service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **library embedding gix without forcing downstream choices.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **high-performance repository scanner.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 1.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 1.5 Failure modes and edge cases



* **enabling both async and blocking network families.**

* **forgetting hash feature when defaults are off.**

* **forcing TLS/backend choices from a library.**

* **feature-unifying an entire workspace unintentionally.**



## 1.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 1.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 1.8 Anti-pattern inventory



* **Avoid:** enabling both async and blocking network families.

* **Avoid:** forgetting hash feature when defaults are off.

* **Avoid:** forcing TLS/backend choices from a library.

* **Avoid:** feature-unifying an entire workspace unintentionally.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 1.9 Agent checklist



```text
[ ] Choose application vs library dependency posture
[ ] Enable one hash algorithm
[ ] Select exactly one network-client family
[ ] Pin TLS transport deliberately
[ ] Run `cargo tree -e features` and `cargo tree -d` in CI
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 1.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 2) First executable applications: discover, inspect HEAD, resolve revisions



## 2.0 Scope and mental model



A first useful `gix` program should prove discovery, HEAD access, object attachment, and revision resolution without mutating the repository.



**Feature gate / dependency posture:** `revision (default via basic)`.




### 2.1 Minimal read-only inspection

The crate-level example demonstrates `head_commit()`, `head_name()` and tree lookup. A coding agent should use this shape before reaching for lower-level object/ref stores.

```rust
fn inspect(path: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = gix::discover(path)?;
    let head = repo.head_commit()?;

    println!("head={}", head.id());
    println!("tree={}", head.tree_id()?);

    if let Some(name) = repo.head_name()? {
        println!("branch={}", name.shorten());
    }
    Ok(())
}
```

### 2.2 Unborn HEAD is normal

A newly initialized repository has no HEAD commit. APIs such as `head_commit()` and `head_tree_id()` can fail for an unborn repository; `head_tree_id_or_empty()` exists for workflows where the empty tree is the intended semantic baseline.

### 2.3 Revision lookup

```rust
let commit = repo.rev_parse_single("HEAD")?.object()?.into_commit();
let parent = repo.rev_parse_single("HEAD^")?.object()?.into_commit();
let tree = repo.rev_parse_single("HEAD^{tree}")?.object()?.into_tree();
```

Use `rev_parse_single` only when the contract is exactly one resulting object. Ranges and multi-endpoint specs belong to `rev_parse` / revision-walk APIs.

## 2.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::discover` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::head_commit` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::head_name` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::head_tree_id` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::rev_parse_single` | part of this capability surface; verify exact signature in pinned rustdoc |



## 2.2 Canonical pattern



```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = gix::discover(".")?;
    let head = repo.head_commit()?;

    println!("HEAD: {}", head.id());
    println!("tree: {}", head.tree_id()?);

    let parent = repo.rev_parse_single("HEAD^")?;
    println!("parent: {parent}");
    Ok(())
}
```



## 2.3 Value cases



* **repository metadata probe.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code-indexer startup.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **CI repository validation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **history tooling.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 2.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 2.5 Failure modes and edge cases



* **unborn HEAD.**

* **non-commit HEAD targets.**

* **running discovery from an unexpected working directory.**

* **assuming message/path bytes are UTF-8.**



## 2.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 2.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 2.8 Anti-pattern inventory



* **Avoid:** unborn HEAD.

* **Avoid:** non-commit HEAD targets.

* **Avoid:** running discovery from an unexpected working directory.

* **Avoid:** assuming message/path bytes are UTF-8.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 2.9 Agent checklist



```text
[ ] Handle unborn repositories
[ ] Print IDs rather than trusting names
[ ] Use explicit start path in services
[ ] Separate read-only startup from mutation
[ ] Test bare and non-bare repositories
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 2.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 3) Repository handle model, attached vs detached objects, and thread safety



## 3.0 Scope and mental model



`Repository` is a thread-local handle. With normal features it is `Send` but not `Sync`; convert it to `ThreadSafeRepository` when a shared thread-safe container is required. Attached object wrappers borrow the repository and are commonly !Send/!Sync; detach when ownership must cross boundaries.



**Feature gate / dependency posture:** `parallel for Send when defaults off`.




### 3.1 Why `Repository` is not `Sync`

The high-level handle carries thread-local caches/configuration to make the common single-thread access path fast. Rustdoc explicitly calls it a thread-local handle. With the normal feature set it is `Send` but not `Sync`; under `default-features = false`, it is not even `Send` unless `parallel` is enabled.

This means the default service design should not be `Arc<Repository>` shared by every request. Better choices are:

```text
A. one Repository per worker/thread
B. ThreadSafeRepository in shared state -> obtain thread-local handle in worker
C. exact-path reopen when operation boundaries are coarse
D. detach immutable object data before sending through channels
```

### 3.2 Attached objects

`Commit<'repo>`, `Blob<'repo>`, `Tree<'repo>`, `Tag<'repo>`, `Reference<'repo>` and `Id<'repo>` use repository attachment to expose convenient follow-up methods. This is ergonomic but intentionally ties lifetimes and often auto-trait behavior to the repo handle.

Use `detach()` / `detached()` when the result is a payload rather than an interactive repository object. For large code-intelligence pipelines, a good boundary is often `(ObjectId, owned metadata/content)` rather than an attached object crossing queues.

### 3.3 Clone semantics of Repository

Rustdoc notes that cloning a `Repository` intentionally produces a handle with reset/empty configurable caches/settings so the fastest default is maintained. Do not tune one handle and assume every clone inherited the tuning. Centralize handle construction/tuning when repeatability matters.

## 3.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository` | main high-level repository handle |

| `Repository::into_sync` | part of this capability surface; verify exact signature in pinned rustdoc |

| `ThreadSafeRepository` | thread-safe repository resource container |

| `Commit::detach` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Blob::detach` | part of this capability surface; verify exact signature in pinned rustdoc |

| `ObjectDetached` | part of this capability surface; verify exact signature in pinned rustdoc |



## 3.2 Canonical pattern



```rust
let repo = gix::open("/repo")?;

// Thread-local handle -> thread-safe container when required.
let sync_repo = repo.into_sync();

// Re-obtain a thread-local Repository from the thread-safe container
// using the conversion/API supported by the pinned release before use.
// For payload handoff, prefer detached object data.
```



## 3.3 Value cases



* **parallel code indexing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **worker-pool repository analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **task handoff of object data.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **shared service state.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 3.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 3.5 Failure modes and edge cases



* **putting `Repository` behind Arc and assuming Sync.**

* **moving attached Commit/Tree/Blob across tasks.**

* **expecting Repository clones to preserve tuning state.**

* **disabling defaults without `parallel` then requiring Send.**



## 3.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 3.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 3.8 Anti-pattern inventory



* **Avoid:** putting `Repository` behind Arc and assuming Sync.

* **Avoid:** moving attached Commit/Tree/Blob across tasks.

* **Avoid:** expecting Repository clones to preserve tuning state.

* **Avoid:** disabling defaults without `parallel` then requiring Send.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 3.9 Agent checklist



```text
[ ] Choose per-thread Repository handles or ThreadSafeRepository
[ ] Detach object data before cross-thread queues
[ ] Configure caches on each intended handle
[ ] Test trait bounds under minimal feature set
[ ] Avoid long-lived attached objects when repository handle lifetime complicates APIs
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 3.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 4) Repository discovery, open, init, bare repositories, and trust



## 4.0 Scope and mental model



Opening an exact repository path, discovering upward, initializing, and cloning have distinct semantics. Discovery applies trust determination based on ownership; clone is a staged builder with cleanup-on-drop behavior until persisted/successful.



**Feature gate / dependency posture:** `core; worktree-mutation for clone checkout`.



## 4.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::open` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::open_opts` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::discover` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::discover_opts` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::init` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::init_bare` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::prepare_clone` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::prepare_clone_bare` | part of this capability surface; verify exact signature in pinned rustdoc |



## 4.2 Canonical pattern



```rust
let exact = gix::open("/srv/repos/project.git")?;
let discovered = gix::discover("/workspace/project/src")?;
let normal = gix::init("/tmp/new-project")?;
let bare = gix::init_bare("/tmp/new-project.git")?;
```



## 4.3 Value cases



* **developer CLI discovery.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **server exact-path open.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **ephemeral repository init.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **managed clone cache.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 4.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 4.5 Failure modes and edge cases



* **discovering a parent repository unintentionally.**

* **ignoring trust state.**

* **assuming init implies initial commit.**

* **losing failed clone forensic state because builder cleans up.**



## 4.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 4.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 4.8 Anti-pattern inventory



* **Avoid:** discovering a parent repository unintentionally.

* **Avoid:** ignoring trust state.

* **Avoid:** assuming init implies initial commit.

* **Avoid:** losing failed clone forensic state because builder cleans up.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 4.9 Agent checklist



```text
[ ] Use exact open in services
[ ] Use discover only where parent search is intended
[ ] Inspect `repo.trust()`
[ ] Decide bare vs worktree explicitly
[ ] Call clone `persist()` only when retaining incomplete state is deliberate
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 4.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareFetch.html

* https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareCheckout.html



---



# gix Advanced — 5) Configuration, environment, command context, and trust-sensitive values



## 5.0 Scope and mental model



Git configuration is layered and trust-aware. `gix` tracks trust at configuration-section/source level and may suppress sensitive executable-path values from untrusted configuration. Configuration snapshots are not a substitute for a persistence strategy.



**Feature gate / dependency posture:** `core + command/attributes for some executables`.



## 5.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::config_snapshot` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::config_snapshot_mut` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::reload` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::command_context` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::open_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `config::Source` | part of this capability surface; verify exact signature in pinned rustdoc |



## 5.2 Canonical selection pattern



```text
Need: Configuration, environment, command context, and trust-sensitive values
  -> start with Repository facade
  -> use feature gate: core + command/attributes for some executables
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 5.3 Value cases



* **read Git-compatible settings.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **derive author/committer or transport policy.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **controlled external commands.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **multi-tenant repository service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 5.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 5.5 Failure modes and edge cases



* **executing commands sourced from untrusted config.**

* **assuming in-memory mutation writes disk.**

* **stale config after external changes.**

* **letting environment variables silently reshape server behavior.**



## 5.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 5.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 5.8 Anti-pattern inventory



* **Avoid:** executing commands sourced from untrusted config.

* **Avoid:** assuming in-memory mutation writes disk.

* **Avoid:** stale config after external changes.

* **Avoid:** letting environment variables silently reshape server behavior.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 5.9 Agent checklist



```text
[ ] Define allowed config sources
[ ] Reload when external config changes matter
[ ] Keep security-sensitive values trust-gated
[ ] Audit environment overrides
[ ] Do not persist config changes unless API explicitly writes them
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 5.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 6) Repository layout: git-dir, work-dir, common-dir, namespaces, and state



## 6.0 Scope and mental model



Git repositories may be bare, have linked worktrees, use a common directory, or be in an in-progress operation state. Never hard-code `.git` path arithmetic when repository APIs expose the resolved locations.



**Feature gate / dependency posture:** `core`.



## 6.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::git_dir` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::work_dir` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::common_dir` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::path` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::kind` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::is_bare` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::state` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::namespace` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::set_namespace` | part of this capability surface; verify exact signature in pinned rustdoc |



## 6.2 Canonical selection pattern



```text
Need: Repository layout: git-dir, work-dir, common-dir, namespaces, and state
  -> start with Repository facade
  -> use feature gate: core
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 6.3 Value cases



* **correct repository file addressing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **linked-worktree tooling.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **server bare repositories.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **operation-state detection.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 6.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 6.5 Failure modes and edge cases



* **assuming `.git` is a directory.**

* **using work_dir in bare repo.**

* **writing common data into per-worktree git dir.**

* **ignoring merge/rebase/bisect state.**



## 6.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 6.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 6.8 Anti-pattern inventory



* **Avoid:** assuming `.git` is a directory.

* **Avoid:** using work_dir in bare repo.

* **Avoid:** writing common data into per-worktree git dir.

* **Avoid:** ignoring merge/rebase/bisect state.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 6.9 Agent checklist



```text
[ ] Use API-resolved git/common/work dirs
[ ] Branch on bare/non-bare
[ ] Check repository state before mutation
[ ] Test linked worktrees
[ ] Use namespace APIs instead of prefixing ref strings manually
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 6.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 7) Object IDs, hash algorithms, SHA-1/SHA-256 feature policy



## 7.0 Scope and mental model



Object IDs are algorithm-tagged Git hashes, not arbitrary strings. `gix` compiles hash algorithms by feature; at least one is mandatory. SHA-256 feature availability is not evidence that the entire Git SHA-256/reftable ecosystem is feature-complete.



**Feature gate / dependency posture:** `sha1 default; sha256 opt-in`.



## 7.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `ObjectId` | owned object hash identity |

| `oid` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::object_hash` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::hash` | part of this capability surface; verify exact signature in pinned rustdoc |

| `ObjectId::from_hex` | part of this capability surface; verify exact signature in pinned rustdoc |



## 7.2 Canonical pattern



```toml
# Normal Git compatibility
gix = { version = "=0.86.0", default-features = false, features = ["sha1", "revision", "index", "parallel"] }

# Compile SHA-256 primitives too only when needed
# features = ["sha1", "sha256", ...]
```



## 7.3 Value cases



* **object identity.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **content-addressed caches.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **revision storage.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **cross-process API IDs.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 7.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 7.5 Failure modes and edge cases



* **assuming 40 hex chars forever.**

* **hard-coding SHA-1 byte length.**

* **serializing IDs without algorithm context.**

* **enabling sha256 and assuming full Git 3.0 interoperability.**



## 7.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 7.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 7.8 Anti-pattern inventory



* **Avoid:** assuming 40 hex chars forever.

* **Avoid:** hard-coding SHA-1 byte length.

* **Avoid:** serializing IDs without algorithm context.

* **Avoid:** enabling sha256 and assuming full Git 3.0 interoperability.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 7.9 Agent checklist



```text
[ ] Use ObjectId/oid types in core logic
[ ] Render hex only at boundaries
[ ] Store algorithm with IDs when persistence format needs it
[ ] Keep sha1 enabled for normal Git repos
[ ] Consult project status for SHA-256 repository completeness
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 7.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 8) Object database: lookup, headers, writing, caching, and in-memory writes



## 8.0 Scope and mental model



The ODB is a layered object lookup/write system over loose objects, packs, alternates and caches. High-level find methods attach objects to the repository; low-level ODB crates are available when allocation/control matters.



**Feature gate / dependency posture:** `core; cache features for tuning`.



## 8.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::find_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::try_find_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_header` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::has_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_blob_stream` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::object_cache_size` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::with_object_memory` | part of this capability surface; verify exact signature in pinned rustdoc |



## 8.2 Canonical pattern



```rust
let id = repo.rev_parse_single("HEAD^{tree}")?;
let object = repo.find_object(id)?;
let header = repo.find_header(id)?;
println!("kind={:?} size={}", header.kind, header.size);
```



## 8.3 Value cases



* **code-history indexer.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **object graph traversal.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **content extraction.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **temporary object construction.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 8.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 8.5 Failure modes and edge cases



* **unbounded repeated decompression without cache.**

* **assuming object existence before dereference.**

* **using `with_object_memory` and expecting durable writes.**

* **ignoring replacements/alternates semantics.**



## 8.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 8.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 8.8 Anti-pattern inventory



* **Avoid:** unbounded repeated decompression without cache.

* **Avoid:** assuming object existence before dereference.

* **Avoid:** using `with_object_memory` and expecting durable writes.

* **Avoid:** ignoring replacements/alternates semantics.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 8.9 Agent checklist



```text
[ ] Size object cache for repeated traversals
[ ] Use try_find for absence-tolerant probes
[ ] Use headers when full decode is unnecessary
[ ] Treat object writes and ref publication as separate phases
[ ] Benchmark high-level vs gix-odb only if needed
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 8.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 9) Blob objects and binary-safe content handling



## 9.0 Scope and mental model



Git blobs are opaque bytes. `gix::Blob` owns decoded blob bytes and a repository attachment; never impose text or UTF-8 semantics unless the caller explicitly knows the path/content contract.



**Feature gate / dependency posture:** `core; blob-diff for line diffs`.



## 9.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_blob_stream` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Blob::detach` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Blob::take_data` | part of this capability surface; verify exact signature in pinned rustdoc |



## 9.2 Canonical selection pattern



```text
Need: Blob objects and binary-safe content handling
  -> start with Repository facade
  -> use feature gate: core; blob-diff for line diffs
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 9.3 Value cases



* **source-code retrieval.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **binary assets.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **hashing new file content.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **diff input.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 9.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 9.5 Failure modes and edge cases



* **String::from_utf8 unwrap.**

* **normalizing newlines while reading raw blobs.**

* **confusing worktree-filtered content with raw blob bytes.**

* **holding huge blobs unnecessarily.**



## 9.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 9.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 9.8 Anti-pattern inventory



* **Avoid:** String::from_utf8 unwrap.

* **Avoid:** normalizing newlines while reading raw blobs.

* **Avoid:** confusing worktree-filtered content with raw blob bytes.

* **Avoid:** holding huge blobs unnecessarily.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 9.9 Agent checklist



```text
[ ] Keep blob APIs byte-oriented
[ ] Use stream write for large content
[ ] Apply filters only when worktree semantics are desired
[ ] Detach/take_data when moving data out of repo lifetime
[ ] Cap blob size in untrusted scans
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 9.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 10) Tree objects, traversal, entry lookup, and tree editing



## 10.0 Scope and mental model



Trees are sorted directory snapshots mapping byte-string names and modes to object IDs. Traversal is object-graph traversal, not filesystem walking. Tree editing constructs new immutable tree objects rather than mutating existing objects in place.



**Feature gate / dependency posture:** `tree-editor for editing; core traversal`.



## 10.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Tree::iter` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Tree::lookup_entry` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::empty_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `object::tree::Editor` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::traverse` | part of this capability surface; verify exact signature in pinned rustdoc |



## 10.2 Canonical selection pattern



```text
Need: Tree objects, traversal, entry lookup, and tree editing
  -> start with Repository facade
  -> use feature gate: tree-editor for editing; core traversal
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 10.3 Value cases



* **repository snapshot indexing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **file-at-revision lookup.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **tree diff inputs.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **constructing commits without touching worktree.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 10.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 10.5 Failure modes and edge cases



* **treating tree names as UTF-8.**

* **confusing tree modes with OS metadata.**

* **assuming edit changes HEAD.**

* **recursive traversal without depth/resource controls.**



## 10.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 10.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 10.8 Anti-pattern inventory



* **Avoid:** treating tree names as UTF-8.

* **Avoid:** confusing tree modes with OS metadata.

* **Avoid:** assuming edit changes HEAD.

* **Avoid:** recursive traversal without depth/resource controls.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 10.9 Agent checklist



```text
[ ] Separate tree traversal from dirwalk
[ ] Preserve entry mode and byte name
[ ] Write edited trees before updating refs
[ ] Test submodule/gitlink entries
[ ] Set resource limits for hostile/deep trees
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 10.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 11) Commit objects, actors, messages, signatures, parents, and creation inputs



## 11.0 Scope and mental model



Commit objects are immutable byte-encoded objects containing tree ID, parent IDs, actor signatures, message and optional extra headers/signature data. `Commit` is repository-attached; creation still requires explicit publication via references.



**Feature gate / dependency posture:** `core + revision for traversal helpers`.



## 11.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Commit` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::decode` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::author` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::committer` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::message` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::parent_ids` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::signature` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::ancestors` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::commit` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::new_commit` | part of this capability surface; verify exact signature in pinned rustdoc |



## 11.2 Canonical selection pattern



```text
Need: Commit objects, actors, messages, signatures, parents, and creation inputs
  -> start with Repository facade
  -> use feature gate: core + revision for traversal helpers
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 11.3 Value cases



* **history analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **authorship metadata.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **commit graph extraction.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **programmatic commit creation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 11.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 11.5 Failure modes and edge cases



* **assuming one parent.**

* **assuming commit message UTF-8.**

* **using author instead of committer semantics.**

* **creating an object and assuming branch moved.**



## 11.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 11.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 11.8 Anti-pattern inventory



* **Avoid:** assuming one parent.

* **Avoid:** assuming commit message UTF-8.

* **Avoid:** using author instead of committer semantics.

* **Avoid:** creating an object and assuming branch moved.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 11.9 Agent checklist



```text
[ ] Iterate all parents
[ ] Preserve raw message bytes when fidelity matters
[ ] Distinguish author and committer
[ ] Verify tree exists before strict workflows
[ ] Update ref with previous-value constraint after object creation
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 11.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 12) Tag objects and annotated/lightweight tag handling



## 12.0 Scope and mental model



Annotated tags are objects; lightweight tags are references. APIs that peel references may traverse symbolic refs and annotated tag chains until a non-tag object is reached.



**Feature gate / dependency posture:** `core`.



## 12.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Tag` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_tag` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::tag` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::tag_reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::peel_to_tag` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::peel_to_id` | part of this capability surface; verify exact signature in pinned rustdoc |



## 12.2 Canonical selection pattern



```text
Need: Tag objects and annotated/lightweight tag handling
  -> start with Repository facade
  -> use feature gate: core
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 12.3 Value cases



* **release metadata.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **tag listing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **resolve tag to commit/tree/blob.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **create annotated or lightweight tags.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 12.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 12.5 Failure modes and edge cases



* **assuming every refs/tags/* points to Tag object.**

* **ignoring signed tag payloads.**

* **peeling when caller wanted tag identity.**

* **stale Reference snapshots.**



## 12.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 12.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 12.8 Anti-pattern inventory



* **Avoid:** assuming every refs/tags/* points to Tag object.

* **Avoid:** ignoring signed tag payloads.

* **Avoid:** peeling when caller wanted tag identity.

* **Avoid:** stale Reference snapshots.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 12.9 Agent checklist



```text
[ ] Distinguish ref from annotated Tag object
[ ] Choose peel depth deliberately
[ ] Preserve tagger/message/signature data
[ ] Use ref constraints on creation
[ ] Refresh references when concurrent writers matter
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 12.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 13) References, HEAD, symbolic refs, reflogs, edits, and transactions



## 13.0 Scope and mental model



References are mutable names over immutable objects. `Reference` values are snapshots and can become stale. Safe mutation requires expected-previous-value constraints, lock/transaction behavior, and explicit reflog messages.



**Feature gate / dependency posture:** `core`.



## 13.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::references` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::try_find_reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_references` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::follow` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::peel_to_id` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::set_target_id` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::log_iter` | part of this capability surface; verify exact signature in pinned rustdoc |



## 13.2 Canonical pattern



```rust
use gix::refs::transaction::PreviousValue;

let target = repo.rev_parse_single("HEAD")?;
let new_ref = repo.reference(
    "refs/heads/agent-snapshot",
    target,
    PreviousValue::MustNotExist,
    "agent: create snapshot ref",
)?;
println!("{}", new_ref.name());
```



## 13.3 Value cases



* **branch/tag lookup.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **HEAD resolution.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **atomic branch moves.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **reflog-aware mutation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 13.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 13.5 Failure modes and edge cases



* **lost updates from blind ref writes.**

* **using stale Reference snapshot after concurrent change.**

* **confusing symbolic target with peeled object.**

* **inventing ref names without validation.**



## 13.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 13.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 13.8 Anti-pattern inventory



* **Avoid:** lost updates from blind ref writes.

* **Avoid:** using stale Reference snapshot after concurrent change.

* **Avoid:** confusing symbolic target with peeled object.

* **Avoid:** inventing ref names without validation.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 13.9 Agent checklist



```text
[ ] Use full validated ref names
[ ] Use expected previous target when mutation correctness matters
[ ] Write reflog messages intentionally
[ ] Re-read after concurrent operations
[ ] Use batch edit/transaction APIs for related refs
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 13.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 14) Revision specifications, rev-parse behavior, describe, and object peeling



## 14.0 Scope and mental model



Revspec parsing is a user-facing language with ambiguity, peeling, ancestry operators and optional regex syntax. `rev_parse_single` is the best convenience API when exactly one object ID must result; `rev_parse` preserves multi-endpoint/range semantics.



**Feature gate / dependency posture:** `revision; revparse-regex for regex syntax`.



## 14.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::rev_parse` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::rev_parse_single` | part of this capability surface; verify exact signature in pinned rustdoc |

| `revision::Spec` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Id::object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Object::peel_to_kind` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::describe` | part of this capability surface; verify exact signature in pinned rustdoc |



## 14.2 Canonical pattern



```rust
let one = repo.rev_parse_single("HEAD")?;
let parent = repo.rev_parse_single("HEAD^")?;
let tree = repo.rev_parse_single("HEAD^{tree}")?;

let commit = parent.object()?.into_commit();
println!("{}", commit.id);
```



## 14.3 Value cases



* **CLI-like revision input.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code review range resolution.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **history navigation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **tag/branch/hash resolution.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 14.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 14.5 Failure modes and edge cases



* **treating arbitrary user input as only a ref name.**

* **ambiguous abbreviated IDs.**

* **Git deviations documented by rustdoc.**

* **regex feature compile-time cost.**



## 14.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 14.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 14.8 Anti-pattern inventory



* **Avoid:** treating arbitrary user input as only a ref name.

* **Avoid:** ambiguous abbreviated IDs.

* **Avoid:** Git deviations documented by rustdoc.

* **Avoid:** regex feature compile-time cost.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 14.9 Agent checklist



```text
[ ] Use rev_parse_single only for single-object contract
[ ] Validate expected object kind after resolution
[ ] Enable revparse-regex only if needed
[ ] Test Git-parity revspecs important to product
[ ] Cap expensive searches in untrusted input paths
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 14.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 15) Revision walking, commit graphs, merge bases, and graph caches



## 15.0 Scope and mental model



History traversal can use object parsing alone or accelerate with commit-graph data. Repeated merge-base or ancestry queries benefit from cache/graph reuse and object caching.



**Feature gate / dependency posture:** `revision`.



## 15.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Commit::ancestors` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::rev_walk` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::revision_graph` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::commit_graph` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::commit_graph_if_enabled` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::merge_base` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::merge_bases_many` | part of this capability surface; verify exact signature in pinned rustdoc |



## 15.2 Canonical selection pattern



```text
Need: Revision walking, commit graphs, merge bases, and graph caches
  -> start with Repository facade
  -> use feature gate: revision
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 15.3 Value cases



* **call-history/code lineage.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **reachability.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **merge-base.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **topological/time-sorted history.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 15.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 15.5 Failure modes and edge cases



* **re-decoding commits on every query.**

* **assuming commit graph always exists or is complete.**

* **incorrect ordering assumptions.**

* **unbounded traversal from attacker-controlled tips.**



## 15.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 15.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 15.8 Anti-pattern inventory



* **Avoid:** re-decoding commits on every query.

* **Avoid:** assuming commit graph always exists or is complete.

* **Avoid:** incorrect ordering assumptions.

* **Avoid:** unbounded traversal from attacker-controlled tips.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 15.9 Agent checklist



```text
[ ] Reuse revision graph for repeated queries
[ ] Enable object cache
[ ] Choose sorting explicitly
[ ] Treat missing commit graph as fallback case
[ ] Set traversal limits/timeouts in services
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 15.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 16) The Git index: load, create from tree, stages, write, and integrity



## 16.0 Scope and mental model



The Git index is a persisted staging data structure with entries, stages and extensions. It is not simply a map from path to object ID. Load, mutation and write paths must preserve or deliberately invalidate extensions and stat data.



**Feature gate / dependency posture:** `index`.



## 16.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::index` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::open_index` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::index_from_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::index_or_load_from_head` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::index_or_load_from_head_or_empty` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::index::File` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::index::State` | part of this capability surface; verify exact signature in pinned rustdoc |



## 16.2 Canonical selection pattern



```text
Need: The Git index: load, create from tree, stages, write, and integrity
  -> start with Repository facade
  -> use feature gate: index
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 16.3 Value cases



* **staged-change analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **status.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **construct checkout input.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **programmatic staging workflows.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 16.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 16.5 Failure modes and edge cases



* **assuming stage 0 only during conflicts.**

* **writing stale extensions.**

* **path ordering/normalization mistakes.**

* **concurrent Git writers.**



## 16.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] crash/failure before publication
[ ] lock contention
[ ] mixed Git CLI -> gix -> Git CLI round trip
```



## 16.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 16.8 Anti-pattern inventory



* **Avoid:** assuming stage 0 only during conflicts.

* **Avoid:** writing stale extensions.

* **Avoid:** path ordering/normalization mistakes.

* **Avoid:** concurrent Git writers.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 16.9 Agent checklist



```text
[ ] Handle conflict stages
[ ] Validate entries before write
[ ] Use locking/transactional write APIs
[ ] Test mixed Git↔gix index round trips
[ ] Re-read after external index changes
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 16.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 17) Pathspecs, glob/path normalization, case behavior, and search



## 17.0 Scope and mental model



Git pathspecs are richer than glob strings: magic signatures, prefixes, case behavior, exclusions and attribute predicates can participate. Use gix pathspec APIs instead of translating to regex ad hoc.



**Feature gate / dependency posture:** `attributes (brings pathspec)`.



## 17.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::pathspec` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Pathspec` | part of this capability surface; verify exact signature in pinned rustdoc |

| `PathspecDetached` | part of this capability surface; verify exact signature in pinned rustdoc |

| `pathspec::SearchMode` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::glob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::normalize_path` | part of this capability surface; verify exact signature in pinned rustdoc |



## 17.2 Canonical selection pattern



```text
Need: Pathspecs, glob/path normalization, case behavior, and search
  -> start with Repository facade
  -> use feature gate: attributes (brings pathspec)
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 17.3 Value cases



* **select files at revision/worktree.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **filter status/diff scope.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code-index inclusion rules.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **Git-compatible CLI paths.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 17.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 17.5 Failure modes and edge cases



* **regex approximation of Git pathspec.**

* **OS-native separator leakage.**

* **case-sensitivity assumptions.**

* **UTF-8-only paths.**



## 17.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 17.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 17.8 Anti-pattern inventory



* **Avoid:** regex approximation of Git pathspec.

* **Avoid:** OS-native separator leakage.

* **Avoid:** case-sensitivity assumptions.

* **Avoid:** UTF-8-only paths.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 17.9 Agent checklist



```text
[ ] Normalize through repository APIs
[ ] Preserve byte strings
[ ] Test top/prefix/exclude pathspec forms
[ ] Use correct search mode
[ ] Apply attribute predicates with correct source
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 17.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 18) Attributes, ignores, excludes, directory walking, and worktree stacks



## 18.0 Scope and mental model



Attributes and excludes are hierarchical, path-sensitive stacks sourced from worktree, index and configuration. Directory walking integrates Git ignore and attribute semantics and is distinct from tree traversal.



**Feature gate / dependency posture:** `attributes + excludes + dirwalk`.



## 18.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::attributes` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::attributes_only` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::excludes` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::dirwalk_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `worktree::Stack` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::ignore` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::attributes` | part of this capability surface; verify exact signature in pinned rustdoc |



## 18.2 Canonical selection pattern



```text
Need: Attributes, ignores, excludes, directory walking, and worktree stacks
  -> start with Repository facade
  -> use feature gate: attributes + excludes + dirwalk
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 18.3 Value cases



* **honor .gitignore.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **query .gitattributes.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **scan untracked files.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **prepare filtered checkout/archive.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 18.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 18.5 Failure modes and edge cases



* **reading untrusted external attribute/filter config.**

* **wrong source precedence.**

* **treating ignore as security boundary.**

* **walking .git internals accidentally.**



## 18.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 18.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 18.8 Anti-pattern inventory



* **Avoid:** reading untrusted external attribute/filter config.

* **Avoid:** wrong source precedence.

* **Avoid:** treating ignore as security boundary.

* **Avoid:** walking .git internals accidentally.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 18.9 Agent checklist



```text
[ ] Choose attribute/ignore source explicitly
[ ] Use dirwalk rather than generic walk for Git semantics
[ ] Do not use ignore rules as authorization
[ ] Bound filesystem traversal
[ ] Test nested ignore/attribute precedence
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 18.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 19) Clean/smudge filter pipelines and external process boundaries



## 19.0 Scope and mental model



Clean/smudge and process filters bridge Git object content and worktree content. Some filters invoke external processes, making trust and command policy first-class security boundaries.



**Feature gate / dependency posture:** `attributes`.



## 19.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::filter_pipeline` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::filter` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::command_context` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::command` | part of this capability surface; verify exact signature in pinned rustdoc |



## 19.2 Canonical selection pattern



```text
Need: Clean/smudge filter pipelines and external process boundaries
  -> start with Repository facade
  -> use feature gate: attributes
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 19.3 Value cases



* **Git LFS-like filter integration.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **checkout materialization.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **worktree-to-object conversion.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **archive/worktree streams.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 19.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 19.5 Failure modes and edge cases



* **executing untrusted filter commands.**

* **deadlocking long-lived process filters.**

* **confusing raw blob with smudged worktree bytes.**

* **non-deterministic filters.**



## 19.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 19.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 19.8 Anti-pattern inventory



* **Avoid:** executing untrusted filter commands.

* **Avoid:** deadlocking long-lived process filters.

* **Avoid:** confusing raw blob with smudged worktree bytes.

* **Avoid:** non-deterministic filters.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 19.9 Agent checklist



```text
[ ] Only enable external processes from trusted config
[ ] Set process timeouts/cancellation
[ ] Log filter identity without secrets
[ ] Test clean↔smudge round trips
[ ] Keep raw-object code paths separate from filtered content
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 19.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 20) Repository status: index↔worktree, tree↔index, untracked, submodules



## 20.0 Scope and mental model



High-level status is a configurable platform that combines HEAD/tree↔index and index↔worktree comparisons, untracked discovery and optional submodule work. Fast dirty checks can stop early; full status should be configured for deterministic sorting only when needed.



**Feature gate / dependency posture:** `status`.



## 20.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::is_dirty` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::is_pristine` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::index_worktree_status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::tree_index_status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `status::Platform` | part of this capability surface; verify exact signature in pinned rustdoc |

| `status::Item` | part of this capability surface; verify exact signature in pinned rustdoc |

| `status::UntrackedFiles` | part of this capability surface; verify exact signature in pinned rustdoc |



## 20.2 Canonical pattern



```rust
let platform = repo.status(gix::progress::Discard)?;
// Configure untracked/submodule/sorting/interrupt options on `platform`,
// then convert it into the iterator form provided by the pinned release.
// For a boolean-only guard, prefer repo.is_dirty() / repo.is_pristine().
```



## 20.3 Value cases



* **IDE/code-agent dirty checks.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **pre-commit validation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **change inventory.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **incremental code-index invalidation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 20.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 20.5 Failure modes and edge cases



* **full status for simple dirty boolean.**

* **sorting huge results unnecessarily.**

* **ignoring submodule cost.**

* **assuming filesystem stat always proves content equality.**



## 20.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 20.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 20.8 Anti-pattern inventory



* **Avoid:** full status for simple dirty boolean.

* **Avoid:** sorting huge results unnecessarily.

* **Avoid:** ignoring submodule cost.

* **Avoid:** assuming filesystem stat always proves content equality.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 20.9 Agent checklist



```text
[ ] Use is_dirty/is_pristine for boolean need
[ ] Configure untracked policy
[ ] Set interrupt flag for long walks
[ ] Sort only for deterministic output
[ ] Test symlink/executable/case-only changes
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 20.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/status/index.html

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 21) Blob and tree diff, line diff, rewrite/rename tracking, and diff caches



## 21.0 Scope and mental model



Diff separates structural tree changes from content diff and rewrite/rename tracking. Rename/copy detection can require blob similarity work; use a reusable resource cache for repeated diffs.



**Feature gate / dependency posture:** `blob-diff + attributes`.



## 21.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::diff_tree_to_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::diff_resource_cache` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::diff_algorithm` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Tree::changes` | part of this capability surface; verify exact signature in pinned rustdoc |

| `object::tree::diff::Change` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::diff::Rewrites` | part of this capability surface; verify exact signature in pinned rustdoc |



## 21.2 Canonical selection pattern



```text
Need: Blob and tree diff, line diff, rewrite/rename tracking, and diff caches
  -> start with Repository facade
  -> use feature gate: blob-diff + attributes
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 21.3 Value cases



* **code review.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **changed-file dependency invalidation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **rename-aware history.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **patch/stat generation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 21.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 21.5 Failure modes and edge cases



* **assuming tree diff gives textual patch automatically.**

* **unbounded rename candidate matrix.**

* **ignoring attributes/binary drivers.**

* **using path-only identity through renames.**



## 21.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 21.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 21.8 Anti-pattern inventory



* **Avoid:** assuming tree diff gives textual patch automatically.

* **Avoid:** unbounded rename candidate matrix.

* **Avoid:** ignoring attributes/binary drivers.

* **Avoid:** using path-only identity through renames.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 21.9 Agent checklist



```text
[ ] Choose structural vs content diff need
[ ] Reuse diff cache
[ ] Set rewrite limits/similarity policy
[ ] Treat binary files explicitly
[ ] Test mode-only and submodule changes
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 21.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 22) Merge primitives: blob, tree, commit merge, options, conflicts, limitations



## 22.0 Scope and mental model



Merge support provides increasingly high-level blob/tree/commit merge primitives, conflict records, attribute-aware drivers and merge-base support. It is not the same thing as a complete `git merge` porcelain workflow with state files, hooks, index/worktree orchestration and continue/abort semantics.



**Feature gate / dependency posture:** `merge opt-in`.



## 22.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::merge_trees` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::merge_commits` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::blob_merge_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::tree_merge_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::merge_resource_cache` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::merge::blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::merge::tree` | part of this capability surface; verify exact signature in pinned rustdoc |



## 22.2 Canonical selection pattern



```text
Need: Merge primitives: blob, tree, commit merge, options, conflicts, limitations
  -> start with Repository facade
  -> use feature gate: merge opt-in
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 22.3 Value cases



* **server-side virtual merge.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **conflict analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **three-way file merge.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **preflight mergeability.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 22.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 22.5 Failure modes and edge cases



* **equating merge_commits with checkout/update HEAD workflow.**

* **discarding unresolved conflict metadata.**

* **external merge driver execution.**

* **incorrect rename/attribute assumptions.**



## 22.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 22.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 22.8 Anti-pattern inventory



* **Avoid:** equating merge_commits with checkout/update HEAD workflow.

* **Avoid:** discarding unresolved conflict metadata.

* **Avoid:** external merge driver execution.

* **Avoid:** incorrect rename/attribute assumptions.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 22.9 Agent checklist



```text
[ ] Enable `merge` explicitly on 0.86
[ ] Use merge resource cache for repeated operations
[ ] Inspect conflict outcomes before publishing
[ ] Implement state/index/ref/worktree orchestration separately
[ ] Consult crate-status before emulating git merge
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 22.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 23) Blame and mailmap



## 23.0 Scope and mental model



Blame is plumbing over diff/history traversal and remains a relatively early-area crate; mailmap canonicalizes identities without changing stored commit objects.



**Feature gate / dependency posture:** `blame and mailmap default via extras`.



## 23.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::blame_file` | part of this capability surface; verify exact signature in pinned rustdoc |

| `repository::blame_file::Options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::open_mailmap` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::author` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Commit::committer` | part of this capability surface; verify exact signature in pinned rustdoc |



## 23.2 Canonical selection pattern



```text
Need: Blame and mailmap
  -> start with Repository facade
  -> use feature gate: blame and mailmap default via extras
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 23.3 Value cases



* **line ownership.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code archaeology.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **author normalization.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **developer analytics.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 23.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 23.5 Failure modes and edge cases



* **treating blame as authorship truth.**

* **high cost on large histories.**

* **forgetting moves/renames limitations.**

* **using mailmap-canonical identity as object identity.**



## 23.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 23.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 23.8 Anti-pattern inventory



* **Avoid:** treating blame as authorship truth.

* **Avoid:** high cost on large histories.

* **Avoid:** forgetting moves/renames limitations.

* **Avoid:** using mailmap-canonical identity as object identity.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 23.9 Agent checklist



```text
[ ] Bound blame scope
[ ] Cache history/diff resources
[ ] Keep raw and mapped identity fields
[ ] Test binary/large files
[ ] Version-pin expectations because blame is less mature
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 23.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 24) Submodules: discovery, configuration, activation, open, and status



## 24.0 Scope and mental model



Submodules combine a gitlink entry in a tree/index, `.gitmodules`, local config, activation rules and another repository. Treat each submodule as a separate trust/config/network/resource boundary.



**Feature gate / dependency posture:** `attributes enables submodule APIs; status for status`.



## 24.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::submodules` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::modules` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::open_modules_file` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Submodule` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Submodule::open` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Submodule::status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Submodule::is_active` | part of this capability surface; verify exact signature in pinned rustdoc |



## 24.2 Canonical selection pattern



```text
Need: Submodules: discovery, configuration, activation, open, and status
  -> start with Repository facade
  -> use feature gate: attributes enables submodule APIs; status for status
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 24.3 Value cases



* **monorepo dependency discovery.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **recursive code indexing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **submodule status.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **controlled submodule opening.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 24.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 24.5 Failure modes and edge cases



* **recursive cycles/resource explosion.**

* **untrusted submodule URLs.**

* **assuming submodule checkout exists.**

* **confusing desired gitlink ID with current HEAD.**



## 24.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 24.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 24.8 Anti-pattern inventory



* **Avoid:** recursive cycles/resource explosion.

* **Avoid:** untrusted submodule URLs.

* **Avoid:** assuming submodule checkout exists.

* **Avoid:** confusing desired gitlink ID with current HEAD.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 24.9 Agent checklist



```text
[ ] Set recursion depth/count limits
[ ] Validate/rewrite URLs under policy
[ ] Compare index/head/worktree states separately
[ ] Open nested repositories with trust policy
[ ] Do not auto-fetch submodules unless explicitly authorized
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 24.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 25) Main and linked worktrees



## 25.0 Scope and mental model



Linked worktrees share repository data through a common directory while keeping per-worktree HEAD/index/state. Correct tools must distinguish main/common repository resources from worktree-local resources.



**Feature gate / dependency posture:** `core + index/excludes/attributes as needed`.



## 25.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::worktree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::worktrees` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::main_repo` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Worktree` | worktree-specific view |

| `Worktree::open_index` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Worktree::excludes` | part of this capability surface; verify exact signature in pinned rustdoc |

| `worktree::Proxy` | part of this capability surface; verify exact signature in pinned rustdoc |



## 25.2 Canonical selection pattern



```text
Need: Main and linked worktrees
  -> start with Repository facade
  -> use feature gate: core + index/excludes/attributes as needed
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 25.3 Value cases



* **multi-branch developer tooling.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code intelligence across linked worktrees.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **worktree-specific status.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **shared object-store analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 25.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 25.5 Failure modes and edge cases



* **editing wrong index.**

* **assuming one HEAD.**

* **using git_dir where common_dir is needed.**

* **deleting shared resources from one worktree.**



## 25.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 25.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 25.8 Anti-pattern inventory



* **Avoid:** editing wrong index.

* **Avoid:** assuming one HEAD.

* **Avoid:** using git_dir where common_dir is needed.

* **Avoid:** deleting shared resources from one worktree.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 25.9 Agent checklist



```text
[ ] Resolve worktree-specific paths via APIs
[ ] Treat index/HEAD/state as per-worktree
[ ] Share ODB analysis carefully
[ ] Test main + linked worktrees
[ ] Do not invent `.git/worktrees` paths manually
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 25.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 26) Checkout/worktree mutation, low-level semantics, and Windows safety



## 26.0 Scope and mental model



Checkout is low-level worktree/index materialization, not a complete branch-switch command. The 0.86.0 security floor is mandatory: earlier versions had a Windows incremental-checkout terminal-symlink arbitrary-write issue when replacing an existing symlink with a file.



**Feature gate / dependency posture:** `worktree-mutation`.




### 26.1 The August 2026 checkout security boundary

GHSA-pmm9-4h7q-24c8 is directly relevant to any agent building checkout/reset-like behavior. In affected releases, `gix_worktree_state::checkout()` could follow an existing terminal symlink/reparse point on Windows during incremental materialization. The critical old configuration was `destination_is_initially_empty = false`, which was also the default returned by `Repository::checkout_options()` in affected versions. A regular-file write replacing an existing symlink could therefore target the symlink destination outside the worktree.

Fresh clone checkout was not the demonstrated vulnerable shipped path because Gitoxide's clone code explicitly used the empty-destination mode. The advisory is nevertheless a library-level security issue because downstream applications implementing incremental checkout were directed to the same public low-level primitive.

### 26.2 Required deployment stance

```text
Never deploy affected versions for write-capable checkout logic.
Minimum patched family:
  gix                 >= 0.86.0
  gix-features        >= 0.49.0
  gix-worktree        >= 0.55.0
  gix-worktree-state  >= 0.33.0
```

An agent must **not** “mitigate” by simply setting `destination_is_initially_empty = true` when the target directory is not actually empty. That flag is a semantic statement used to select safe behavior; lying about the destination can create other correctness failures. Upgrade first.

### 26.3 Checkout is not switch

A full branch switch requires, at minimum, resolving the target commit/tree, checking index/worktree conflicts, materializing files with attributes/filters, updating index, updating HEAD/symbolic refs where appropriate, preserving untracked data, handling sparse/check-out rules where supported, and matching Git's failure semantics. Project status still tracks full checkout/switch/restore/reset orchestration as incomplete. Low-level checkout should therefore be treated as a building block, not a porcelain replacement.

## 26.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::checkout_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix_worktree_state::checkout` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::index_from_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::clone::PrepareCheckout` | part of this capability surface; verify exact signature in pinned rustdoc |



## 26.2 Canonical pattern



```text
SECURITY FLOOR
  gix >= 0.86.0
  gix-features >= 0.49.0
  gix-worktree >= 0.55.0
  gix-worktree-state >= 0.33.0

Affected old pattern on Windows:
  incremental checkout
  destination_is_initially_empty = false
  existing terminal symlink/reparse point
  replacement with regular file
  -> possible write through symlink outside worktree
```



## 26.3 Value cases



* **fresh clone checkout.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **controlled materialization.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **custom reset/checkout orchestration.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **sandboxed worktree creation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 26.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 26.5 Failure modes and edge cases



* **gix <=0.85 on Windows incremental checkout.**

* **assuming destination is empty when it is not.**

* **TOCTOU/symlink/path collisions.**

* **forgetting filters/attributes/executable/symlink modes.**



## 26.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] crash/failure before publication
[ ] lock contention
[ ] mixed Git CLI -> gix -> Git CLI round trip
```



## 26.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 26.8 Anti-pattern inventory



* **Avoid:** gix <=0.85 on Windows incremental checkout.

* **Avoid:** assuming destination is empty when it is not.

* **Avoid:** TOCTOU/symlink/path collisions.

* **Avoid:** forgetting filters/attributes/executable/symlink modes.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 26.9 Agent checklist



```text
[ ] Require gix >=0.86
[ ] Use fresh empty destination when possible
[ ] Keep destination_is_initially_empty truthful
[ ] Sandbox target path
[ ] Do not expose arbitrary checkout destination to untrusted input
[ ] Test Windows symlink/reparse-point collisions
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 26.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/security/advisories/GHSA-pmm9-4h7q-24c8

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 27) Worktree byte streams and archives



## 27.0 Scope and mental model



A worktree stream converts a tree into a byte stream of materialized entries with worktree conversions; archive support consumes that stream. The `gix` archive feature deliberately disables gix-archive default container support, so applications must add `gix-archive` directly and select formats such as tar/zip if needed.



**Feature gate / dependency posture:** `worktree-stream / worktree-archive`.



## 27.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::worktree_stream` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::worktree_archive` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix_worktree_stream` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix_archive` | part of this capability surface; verify exact signature in pinned rustdoc |



## 27.2 Canonical selection pattern



```text
Need: Worktree byte streams and archives
  -> start with Repository facade
  -> use feature gate: worktree-stream / worktree-archive
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 27.3 Value cases



* **server archive endpoint.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **snapshot export.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **remote code bundle.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **tree materialization without filesystem checkout.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 27.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 27.5 Failure modes and edge cases



* **assuming raw tree blob bytes equal worktree stream bytes.**

* **forgetting archive container feature.**

* **unbounded output size.**

* **external filters during stream generation.**



## 27.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 27.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 27.8 Anti-pattern inventory



* **Avoid:** assuming raw tree blob bytes equal worktree stream bytes.

* **Avoid:** forgetting archive container feature.

* **Avoid:** unbounded output size.

* **Avoid:** external filters during stream generation.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 27.9 Agent checklist



```text
[ ] Select archive format crate features explicitly
[ ] Set byte/file limits
[ ] Use cancellation
[ ] Audit filters/attributes
[ ] Stream output rather than buffering whole archive
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 27.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 28) Remotes, remote names, URLs, URL rewriting, and refspecs



## 28.0 Scope and mental model



A `Remote` is configuration plus resolved fetch/push URLs/refspecs. URL rewrite rules (`insteadOf` / `pushInsteadOf`) can change the effective contacted endpoint; authorization and credential matching should reason about the effective URL, not merely the original string.



**Feature gate / dependency posture:** `network client for connect/fetch; core remote config`.



## 28.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::remote_at` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_remote` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::try_find_remote` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::remote_names` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote` | configured/unnamed remote abstraction |

| `Remote::url` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::push_url` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::refspecs` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::save_to` | part of this capability surface; verify exact signature in pinned rustdoc |



## 28.2 Canonical selection pattern



```text
Need: Remotes, remote names, URLs, URL rewriting, and refspecs
  -> start with Repository facade
  -> use feature gate: network client for connect/fetch; core remote config
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 28.3 Value cases



* **remote discovery.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **fetch configuration.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **mirror services.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **URL/refspec auditing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 28.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 28.5 Failure modes and edge cases



* **SSRF through unvalidated URLs.**

* **credential selection against pre-rewrite host.**

* **persisting transient remote config.**

* **assuming remote name always exists.**



## 28.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 28.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 28.8 Anti-pattern inventory



* **Avoid:** SSRF through unvalidated URLs.

* **Avoid:** credential selection against pre-rewrite host.

* **Avoid:** persisting transient remote config.

* **Avoid:** assuming remote name always exists.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 28.9 Agent checklist



```text
[ ] Validate effective URL/scheme/host
[ ] Separate fetch and push URLs
[ ] Parse refspecs with gix types
[ ] Use unnamed remote for transient access
[ ] Persist only under explicit policy
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 28.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/struct.Remote.html

* https://docs.rs/gix/0.86.0/gix/remote/fetch/index.html

* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md



---



# gix Advanced — 29) Credentials, prompts, command spawning, and execution permissions



## 29.0 Scope and mental model



Git authentication may invoke credential helpers, SSH programs, askpass/prompt machinery and other commands. The trust model intentionally suppresses unsafe executable paths from untrusted configuration; preserve that boundary in services.



**Feature gate / dependency posture:** `credentials + command`.



## 29.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::credentials` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::command_context` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::command` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::prompt` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::sec` | part of this capability surface; verify exact signature in pinned rustdoc |



## 29.2 Canonical selection pattern



```text
Need: Credentials, prompts, command spawning, and execution permissions
  -> start with Repository facade
  -> use feature gate: credentials + command
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 29.3 Value cases



* **HTTPS credentials.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **SSH command setup.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **custom credential helper.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **controlled Git-compatible external tools.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 29.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 29.5 Failure modes and edge cases



* **credential leakage in logs.**

* **interactive prompt in headless service.**

* **running arbitrary helper from repo config.**

* **cross-host credential forwarding after redirect/rewrite.**



## 29.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 29.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 29.8 Anti-pattern inventory



* **Avoid:** credential leakage in logs.

* **Avoid:** interactive prompt in headless service.

* **Avoid:** running arbitrary helper from repo config.

* **Avoid:** cross-host credential forwarding after redirect/rewrite.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 29.9 Agent checklist



```text
[ ] Disable prompts in headless deployments
[ ] Use secret manager/credential callback policy
[ ] Redact URL userinfo and helper outputs
[ ] Allowlist executable sources/schemes
[ ] Match credentials to effective endpoint
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 29.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 30) Transport and protocol: file, git, SSH, HTTP(S), sync/async feature choices



## 30.0 Scope and mental model



Transport selection is compile-time feature policy plus runtime URL scheme. Blocking networking supports file/git/ssh and can be extended with curl or reqwest HTTP(S). Async client support exists, but significant pack/index/write computation remains blocking in higher-level workflows.



**Feature gate / dependency posture:** `choose blocking or async network family`.



## 30.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Remote::connect` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::Connection` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::transport` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::protocol` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix_packetline` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::Direction` | part of this capability surface; verify exact signature in pinned rustdoc |



## 30.2 Canonical pattern



```toml
# Pick ONE high-level client family.

# Blocking HTTPS, pure-Rust TLS:
gix = { version = "=0.86.0", features = ["blocking-http-transport-reqwest-rust-tls"] }

# Blocking file://, git://, ssh:// only:
# gix = { version = "=0.86.0", features = ["blocking-network-client"] }

# Async-std protocol integration:
# gix = { version = "=0.86.0", features = ["async-network-client-async-std"] }
```



## 30.3 Value cases



* **fetch service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **clone cache.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **protocol proxy/server experiments.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **custom transport.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 30.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 30.5 Failure modes and edge cases



* **blocking executor threads in async runtime.**

* **enabling incompatible client families.**

* **TLS backend drift.**

* **shell/SSH command injection via URL/config.**



## 30.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 30.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 30.8 Anti-pattern inventory



* **Avoid:** blocking executor threads in async runtime.

* **Avoid:** enabling incompatible client families.

* **Avoid:** TLS backend drift.

* **Avoid:** shell/SSH command injection via URL/config.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 30.9 Agent checklist



```text
[ ] Pick one network family
[ ] Offload blocking high-level fetch/clone from latency-sensitive async executors
[ ] Pin TLS backend
[ ] Set connect/read timeouts around service calls
[ ] Validate URL schemes
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 30.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 31) Fetch: ref mapping, negotiation, shallow behavior, ref updates, and packs



## 31.0 Scope and mental model



Fetch is a staged protocol/pack/ref-update workflow: connect, map refs/refspecs, negotiate wants/haves, receive/index pack, and update refs/FETCH_HEAD-like state as configured. New packs may be kept protected from concurrent garbage collection during the operation.



**Feature gate / dependency posture:** `network client + credentials + negotiation`.



## 31.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Remote::connect` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::Connection::prepare_fetch` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::fetch::Prepare` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Prepare::receive` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::fetch::RefMap` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::fetch::Shallow` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::fetch::Tags` | part of this capability surface; verify exact signature in pinned rustdoc |

| `remote::fetch::negotiate::Algorithm` | part of this capability surface; verify exact signature in pinned rustdoc |



## 31.2 Canonical selection pattern



```text
Need: Fetch: ref mapping, negotiation, shallow behavior, ref updates, and packs
  -> start with Repository facade
  -> use feature gate: network client + credentials + negotiation
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 31.3 Value cases



* **remote update.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **mirror cache.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **shallow fetch.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code-index refresh.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 31.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 31.5 Failure modes and edge cases



* **assuming fetch updates worktree.**

* **unbounded pack download.**

* **refspec widening beyond allowed namespace.**

* **running fetch on async executor without blocking isolation.**



## 31.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 31.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 31.8 Anti-pattern inventory



* **Avoid:** assuming fetch updates worktree.

* **Avoid:** unbounded pack download.

* **Avoid:** refspec widening beyond allowed namespace.

* **Avoid:** running fetch on async executor without blocking isolation.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 31.9 Agent checklist



```text
[ ] Scope refspecs
[ ] Use progress + interrupt flag
[ ] Set shallow/tags policy explicitly
[ ] Validate ref update outcome
[ ] Run worktree/index update as a separate deliberate phase
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 31.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/struct.Remote.html

* https://docs.rs/gix/0.86.0/gix/remote/fetch/index.html

* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md



---



# gix Advanced — 32) Clone builders, fetch-then-checkout, cleanup semantics, and persistence



## 32.0 Scope and mental model



Clone is explicitly staged. `PrepareFetch` owns a newly created repository that is deleted on drop if the clone fails and has not been persisted. Fetch-only returns a repository; fetch-then-checkout returns `PrepareCheckout`, whose own drop behavior protects against leaving an incomplete checkout unless persisted.



**Feature gate / dependency posture:** `blocking-network-client or async-std client; worktree-mutation for checkout`.




### 32.1 Clone state machine

```text
prepare_clone(url, path)
  -> PrepareFetch
      configure remote / ref / shallow / fetch options
      -> fetch_only()
           -> Repository
      OR
      -> fetch_then_checkout()
           -> PrepareCheckout
                -> main_worktree()
                     -> Repository
```

`PrepareFetch` owns the newly created target until successful persistence. Its docs state that unsuccessful state is removed when the builder drops. `PrepareCheckout` similarly deletes the fetched repository if dropped without a successful checkout. This gives clone unusually strong cleanup semantics but is surprising if an agent expects a failed clone directory to remain for inspection.

### 32.2 Async caveat

The docs explicitly warn that the technically async clone/fetch path still performs substantial non-async writes and computation. In Tokio or another latency-sensitive runtime, treat high-level Git operations as blocking/CPU-heavy jobs and isolate them on a dedicated blocking pool or worker process.

### 32.3 Clone correctness checklist

A production clone service should constrain target directory, effective remote URL, redirect/rewrite behavior, credentials, shallow depth/ref selection, total bytes/object count, checkout workers, wall-clock duration, and cleanup/persistence behavior. It should publish a clone to consumers only after the desired terminal phase is complete and the repository can be reopened successfully.

## 32.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::prepare_clone` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::prepare_clone_bare` | part of this capability surface; verify exact signature in pinned rustdoc |

| `clone::PrepareFetch` | part of this capability surface; verify exact signature in pinned rustdoc |

| `PrepareFetch::fetch_only` | part of this capability surface; verify exact signature in pinned rustdoc |

| `PrepareFetch::fetch_then_checkout` | part of this capability surface; verify exact signature in pinned rustdoc |

| `clone::PrepareCheckout` | part of this capability surface; verify exact signature in pinned rustdoc |

| `PrepareCheckout::main_worktree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `PrepareFetch::persist` | part of this capability surface; verify exact signature in pinned rustdoc |



## 32.2 Canonical pattern



```rust
use std::{path::Path, sync::atomic::AtomicBool};

fn clone_repo(url: &str, path: &Path) -> Result<gix::Repository, Box<dyn std::error::Error>> {
    let mut prep = gix::prepare_clone(url, path)?;
    let interrupt = AtomicBool::new(false);

    let (mut checkout, _fetch_outcome) =
        prep.fetch_then_checkout(gix::progress::Discard, &interrupt)?;
    let (repo, _checkout_outcome) =
        checkout.main_worktree(gix::progress::Discard, &interrupt)?;
    Ok(repo)
}
```



## 32.3 Value cases



* **managed clone.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **bare mirror seed.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **ephemeral workspace.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **clone-and-index service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 32.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 32.5 Failure modes and edge cases



* **losing failed repo needed for debugging.**

* **assuming async clone is non-blocking.**

* **wrong remote/ref checkout selection.**

* **oversubscribing checkout workers.**



## 32.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 32.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 32.8 Anti-pattern inventory



* **Avoid:** losing failed repo needed for debugging.

* **Avoid:** assuming async clone is non-blocking.

* **Avoid:** wrong remote/ref checkout selection.

* **Avoid:** oversubscribing checkout workers.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 32.9 Agent checklist



```text
[ ] Use AtomicBool interruption
[ ] Use progress implementation or Discard
[ ] Configure shallow/ref name before fetch
[ ] Call persist only intentionally
[ ] Bound worker count in high-concurrency services
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 32.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareFetch.html

* https://docs.rs/gix/0.86.0/gix/clone/struct.PrepareCheckout.html



---



# gix Advanced — 33) Push: current API surface and the critical workflow-completeness gap



## 33.0 Scope and mental model



Do not infer push support from the existence of `gix::push` or remote push URLs. In the current high-level crate, `gix::push` primarily exposes `push.default` configuration semantics, while project `crate-status.md` still tracks push plumbing/workflow as incomplete.



**Feature gate / dependency posture:** `no complete push workflow in gix 0.86 facade`.




### 33.1 Why this deserves its own section

Agents frequently infer symmetry: if `Remote::connect(Direction::Fetch)` and fetch builders exist, there must be a matching `push()` convenience API. In `gix 0.86.0`, that inference is unsafe. The public all-items page shows `gix::push::Default` (configuration for `push.default`), while current project status still marks push as unfinished plumbing/workflow work.

### 33.2 Safe architecture today

If a product must push now, choose one of these explicitly:

1. invoke a pinned Git executable with a hardened command/environment/credential boundary;
2. use `git2`/libgit2 for that workflow while using `gix` for read/fetch/indexing paths;
3. implement lower-level protocol/pack/ref-update plumbing only if the product truly needs it and can own Git protocol correctness and security.

Do not have an LLM synthesize a hypothetical `gix` push API from method-name intuition.

## 33.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::push::Default` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::push_url` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::refspecs` | part of this capability surface; verify exact signature in pinned rustdoc |



## 33.2 Canonical pattern



```rust
// Do NOT invent a high-level API such as:
// remote.push(...);   // not a supported gix 0.86 high-level workflow

// `gix::push::Default` models push.default configuration semantics,
// not a completed push execution pipeline.
```



## 33.3 Value cases



* **deciding whether gix can replace git CLI for writes.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **architecture boundary for remote publication.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 33.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 33.5 Failure modes and edge cases



* **building production push assuming symmetry with fetch.**

* **fabricating nonexistent high-level methods.**

* **implementing pack/ref update protocol incompletely.**

* **credential/security surprises.**



## 33.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] authentication failure / redirect / URL rewrite
[ ] interrupt during network or pack phase
[ ] oversized/hostile remote response
```



## 33.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 33.8 Anti-pattern inventory



* **Avoid:** building production push assuming symmetry with fetch.

* **Avoid:** fabricating nonexistent high-level methods.

* **Avoid:** implementing pack/ref update protocol incompletely.

* **Avoid:** credential/security surprises.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 33.9 Agent checklist



```text
[ ] For production push, use Git CLI/libgit2 or verified lower-level custom implementation until project status changes
[ ] Check current crate-status on every upgrade
[ ] Do not generate `remote.push()` unless exact API exists
[ ] Separate local mutation from remote publication
[ ] Integration-test against real Git servers if implementing plumbing
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 33.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 34) Creating commits and mutating objects/trees/refs/index safely



## 34.0 Scope and mental model



Mutation is a multi-object transaction you compose: write blobs, build/write trees, create commit object, then update one or more references under constraints. Worktree and index updates are additional state transitions, not side effects of object creation.



**Feature gate / dependency posture:** `index/tree-editor/core; merge as needed`.



## 34.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::write_blob` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::write_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_tree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::commit` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::new_commit` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_reference` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::edit_references` | part of this capability surface; verify exact signature in pinned rustdoc |



## 34.2 Canonical selection pattern



```text
Need: Creating commits and mutating objects/trees/refs/index safely
  -> start with Repository facade
  -> use feature gate: index/tree-editor/core; merge as needed
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 34.3 Value cases



* **bot commits.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **generated-code commits.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **server-side commit synthesis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **metadata-only branch update.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 34.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 34.5 Failure modes and edge cases



* **publishing ref before all objects durable.**

* **lost-update branch race.**

* **tree entry mode mistakes.**

* **inconsistent index/worktree after ref move.**



## 34.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] crash/failure before publication
[ ] lock contention
[ ] mixed Git CLI -> gix -> Git CLI round trip
```



## 34.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 34.8 Anti-pattern inventory



* **Avoid:** publishing ref before all objects durable.

* **Avoid:** lost-update branch race.

* **Avoid:** tree entry mode mistakes.

* **Avoid:** inconsistent index/worktree after ref move.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 34.9 Agent checklist



```text
[ ] Build objects bottom-up
[ ] Verify referenced object IDs
[ ] Use previous-value constraints
[ ] Write meaningful reflogs
[ ] Update index/worktree only as separately designed
[ ] Test crash points between phases
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 34.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 35) Packfiles, multi-pack indexes, ODB internals, alternates, compression



## 35.0 Scope and mental model



Gitoxide exposes pack index/data decoding, delta resolution, object database stores, alternates and commit-graph plumbing. The high-level Repository should remain first choice; drop to subcrates when performance/control justifies the complexity.



**Feature gate / dependency posture:** `core; pack cache performance features`.



## 35.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::pack` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::odb` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::objects` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::pack_compression` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::object_cache_size` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::commitgraph` | part of this capability surface; verify exact signature in pinned rustdoc |



## 35.2 Canonical selection pattern



```text
Need: Packfiles, multi-pack indexes, ODB internals, alternates, compression
  -> start with Repository facade
  -> use feature gate: core; pack cache performance features
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 35.3 Value cases



* **huge repository indexing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **pack inspection.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **server/mirror internals.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **custom object storage.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 35.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 35.5 Failure modes and edge cases



* **reimplementing ODB policy unnecessarily.**

* **cache memory blowup.**

* **GC races.**

* **assuming partial clone/promisor completeness.**



## 35.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 35.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 35.8 Anti-pattern inventory



* **Avoid:** reimplementing ODB policy unnecessarily.

* **Avoid:** cache memory blowup.

* **Avoid:** GC races.

* **Avoid:** assuming partial clone/promisor completeness.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 35.9 Agent checklist



```text
[ ] Benchmark facade before subcrate rewrite
[ ] Protect packs during concurrent operations
[ ] Tune object/pack caches
[ ] Test alternates
[ ] Consult crate-status for promisor/partial-clone/bundle gaps
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 35.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 36) Locks, tempfiles, filesystem snapshots, and atomicity boundaries



## 36.0 Scope and mental model



Git mutation relies on lockfiles/tempfiles/rename-style publication and filesystem snapshots. The plumbing crates encode many consistency rules, but application-level multi-file transactional semantics remain the application’s responsibility.



**Feature gate / dependency posture:** `core + interrupt for signal cleanup`.



## 36.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::lock` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::tempfile` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::fs` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::features::interrupt` | part of this capability surface; verify exact signature in pinned rustdoc |



## 36.2 Canonical selection pattern



```text
Need: Locks, tempfiles, filesystem snapshots, and atomicity boundaries
  -> start with Repository facade
  -> use feature gate: core + interrupt for signal cleanup
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 36.3 Value cases



* **ref/index/config writes.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **temporary pack files.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **atomic file replacement.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **crash-safe staging.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 36.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 36.5 Failure modes and edge cases



* **manual writes bypassing locks.**

* **tempfiles on another filesystem breaking rename assumptions.**

* **signal cleanup races.**

* **network filesystem semantics.**



## 36.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
[ ] crash/failure before publication
[ ] lock contention
[ ] mixed Git CLI -> gix -> Git CLI round trip
```



## 36.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 36.8 Anti-pattern inventory



* **Avoid:** manual writes bypassing locks.

* **Avoid:** tempfiles on another filesystem breaking rename assumptions.

* **Avoid:** signal cleanup races.

* **Avoid:** network filesystem semantics.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 36.9 Agent checklist



```text
[ ] Use gix lock/tempfile APIs
[ ] Keep temp and destination on compatible filesystem
[ ] Handle lock contention
[ ] Fsync/durability only to the level your application requires and verifies
[ ] Test crash/retry behavior
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 36.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 37) Progress, tracing, interruption, signals, and cancellation



## 37.0 Scope and mental model



Long-running clone/fetch/checkout/status/diff operations expose progress and interruption hooks. Treat interruption as cooperative cancellation and pair it with outer deadlines/resource caps in services.



**Feature gate / dependency posture:** `interrupt, tracing, tracing-detail, progress-tree`.



## 37.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::progress::Discard` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::Progress` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::NestedProgress` | part of this capability surface; verify exact signature in pinned rustdoc |

| `AtomicBool` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::interrupt` | part of this capability surface; verify exact signature in pinned rustdoc |

| `tracing` | part of this capability surface; verify exact signature in pinned rustdoc |



## 37.2 Canonical selection pattern



```text
Need: Progress, tracing, interruption, signals, and cancellation
  -> start with Repository facade
  -> use feature gate: interrupt, tracing, tracing-detail, progress-tree
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 37.3 Value cases



* **CLI progress.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **server timeout.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **batch telemetry.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **cancellable repository scan.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 37.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 37.5 Failure modes and edge cases



* **never checking interrupt flag in custom loops.**

* **detailed tracing leaking repo paths/URLs.**

* **expensive progress rendering in hot path.**

* **assuming interruption rolls back every application-level side effect.**



## 37.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 37.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 37.8 Anti-pattern inventory



* **Avoid:** never checking interrupt flag in custom loops.

* **Avoid:** detailed tracing leaking repo paths/URLs.

* **Avoid:** expensive progress rendering in hot path.

* **Avoid:** assuming interruption rolls back every application-level side effect.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 37.9 Agent checklist



```text
[ ] Thread one cancellation flag through operation
[ ] Use Discard when progress not needed
[ ] Redact tracing metadata
[ ] Define cleanup/persist behavior on cancellation
[ ] Test cancellation at multiple phases
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 37.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 38) Error model, error chains, diagnostics, and application context



## 38.0 Scope and mental model



The facade aggregates many typed plumbing errors. Default `auto-chain-error` makes chains work well with anyhow-style reporting; `tree-error` changes representation. Preserve source errors and add operation/repository context at application boundaries.



**Feature gate / dependency posture:** `auto-chain-error default; tree-error alternative`.



## 38.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::Error` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix_error` | part of this capability surface; verify exact signature in pinned rustdoc |

| `auto-chain-error` | part of this capability surface; verify exact signature in pinned rustdoc |

| `tree-error` | part of this capability surface; verify exact signature in pinned rustdoc |



## 38.2 Canonical selection pattern



```text
Need: Error model, error chains, diagnostics, and application context
  -> start with Repository facade
  -> use feature gate: auto-chain-error default; tree-error alternative
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 38.3 Value cases



* **CLI diagnostics.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **service error classification.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **retry policy.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **observability.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 38.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 38.5 Failure modes and edge cases



* **matching full display strings.**

* **retrying integrity errors.**

* **logging credentials from URL/config errors.**

* **panic/unwrap on corrupt repo.**



## 38.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 38.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 38.8 Anti-pattern inventory



* **Avoid:** matching full display strings.

* **Avoid:** retrying integrity errors.

* **Avoid:** logging credentials from URL/config errors.

* **Avoid:** panic/unwrap on corrupt repo.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 38.9 Agent checklist



```text
[ ] Classify by typed/root cause where possible
[ ] Add repo/operation ID context
[ ] Sanitize public errors
[ ] Keep full chain internally
[ ] Test corrupt/truncated repo error paths
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 38.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 39) Concurrency, `parallel`, ThreadSafeRepository, blocking/async integration



## 39.0 Scope and mental model



Concurrency has three layers: thread-safety of repository handles, internal algorithm parallelism, and transport runtime model. `parallel` affects trait bounds and internal multi-threading; async transport does not make CPU/decompression/filesystem portions magically non-blocking.



**Feature gate / dependency posture:** `parallel; network choices`.



## 39.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::into_sync` | part of this capability surface; verify exact signature in pinned rustdoc |

| `ThreadSafeRepository` | thread-safe repository resource container |

| `Repository::clone` | part of this capability surface; verify exact signature in pinned rustdoc |

| `parallel` | part of this capability surface; verify exact signature in pinned rustdoc |

| `async-network-client` | part of this capability surface; verify exact signature in pinned rustdoc |

| `blocking-network-client` | part of this capability surface; verify exact signature in pinned rustdoc |



## 39.2 Canonical pattern



```text
HTTP request concurrency N
  x repository jobs per request
  x gix internal parallel workers
  x pack/diff/checkout work
= potential oversubscription

Bound the outer concurrency first; then benchmark internal `parallel` behavior.
```



## 39.3 Value cases



* **parallel codebase indexing.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **multi-repo service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **async HTTP service invoking Git operations.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **batch farm.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 39.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 39.5 Failure modes and edge cases



* **oversubscription: requests × internal threads.**

* **blocking async runtime.**

* **sharing mutable caches incorrectly.**

* **assuming cloned Repository keeps custom caches.**



## 39.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 39.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 39.8 Anti-pattern inventory



* **Avoid:** oversubscription: requests × internal threads.

* **Avoid:** blocking async runtime.

* **Avoid:** sharing mutable caches incorrectly.

* **Avoid:** assuming cloned Repository keeps custom caches.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 39.9 Agent checklist



```text
[ ] Cap request concurrency
[ ] Cap/measure internal worker counts
[ ] Use spawn_blocking/dedicated pool around blocking workflows
[ ] Convert to sync container only when needed
[ ] Benchmark one repository vs many
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 39.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 40) Performance tuning: object cache, pack caches, commit graph, parallelism



## 40.0 Scope and mental model



Most gix performance wins come from work avoided: object caching, commit-graph reuse, pack delta/cache behavior, path pruning, and bounded parallelism. Default `max-performance-safe` already enables parallel and pack caches in 0.86.



**Feature gate / dependency posture:** `max-control/max-performance/cache debug`.



## 40.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository::object_cache_size` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::object_cache_size_if_unset` | part of this capability surface; verify exact signature in pinned rustdoc |

| `pack-cache-lru-static` | part of this capability surface; verify exact signature in pinned rustdoc |

| `pack-cache-lru-dynamic` | part of this capability surface; verify exact signature in pinned rustdoc |

| `parallel` | part of this capability surface; verify exact signature in pinned rustdoc |

| `commit_graph` | part of this capability surface; verify exact signature in pinned rustdoc |

| `cache-efficiency-debug` | part of this capability surface; verify exact signature in pinned rustdoc |



## 40.2 Canonical selection pattern



```text
Need: Performance tuning: object cache, pack caches, commit graph, parallelism
  -> start with Repository facade
  -> use feature gate: max-control/max-performance/cache debug
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 40.3 Value cases



* **large monorepo scan.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **history graph service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **repeated blame/diff.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **mirror/fetch.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 40.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 40.5 Failure modes and edge cases



* **cache larger than memory budget.**

* **parallel oversubscription.**

* **benchmarking debug builds.**

* **optimizing subcrates before profiling.**



## 40.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 40.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 40.8 Anti-pattern inventory



* **Avoid:** cache larger than memory budget.

* **Avoid:** parallel oversubscription.

* **Avoid:** benchmarking debug builds.

* **Avoid:** optimizing subcrates before profiling.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 40.9 Agent checklist



```text
[ ] Benchmark release
[ ] Use cache-efficiency-debug during tuning
[ ] Reuse graph/diff resources
[ ] Size caches by workload
[ ] Measure cold and warm paths separately
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 40.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 41) Testing and correctness: fixtures, Git parity, adversarial repositories



## 41.0 Scope and mental model



Git repositories are adversarial data structures: corrupt objects, odd byte paths, alternate object stores, linked worktrees, shallow boundaries, conflict stages, symlinks and concurrent writers all need tests. Use the Git executable as a behavioral oracle where compatibility matters.



**Feature gate / dependency posture:** `all relevant features under test`.



## 41.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::open` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::discover` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository methods` | part of this capability surface; verify exact signature in pinned rustdoc |

| `git CLI oracle for parity tests` | part of this capability surface; verify exact signature in pinned rustdoc |



## 41.2 Canonical selection pattern



```text
Need: Testing and correctness: fixtures, Git parity, adversarial repositories
  -> start with Repository facade
  -> use feature gate: all relevant features under test
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 41.3 Value cases



* **regression suite.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **agent-generated patch validation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **cross-platform CI.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **upgrade gate.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 41.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 41.5 Failure modes and edge cases



* **happy-path fixtures only.**

* **ASCII paths only.**

* **single-platform checkout tests.**

* **snapshotting unstable Debug strings.**



## 41.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 41.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 41.8 Anti-pattern inventory



* **Avoid:** happy-path fixtures only.

* **Avoid:** ASCII paths only.

* **Avoid:** single-platform checkout tests.

* **Avoid:** snapshotting unstable Debug strings.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 41.9 Agent checklist



```text
[ ] Create fixtures with Git where practical
[ ] Test bare/unborn/shallow/linked-worktree
[ ] Test non-UTF8 paths on Unix
[ ] Test corrupt/truncated objects
[ ] Run Windows checkout security regression
[ ] Run mixed Git↔gix mutation tests
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 41.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 42) Cross-platform behavior: Linux/macOS/Windows, symlinks, executable bits



## 42.0 Scope and mental model



Git behavior depends on filesystem capabilities: symlinks, executable bits, case sensitivity, precomposition, file identity/stat and Windows reparse-point behavior. Query/use gix filesystem options rather than assuming Unix semantics.



**Feature gate / dependency posture:** `core + worktree-mutation`.



## 42.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix::fs` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::filesystem_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::stat_options` | part of this capability surface; verify exact signature in pinned rustdoc |

| `checkout options` | part of this capability surface; verify exact signature in pinned rustdoc |



## 42.2 Canonical selection pattern



```text
Need: Cross-platform behavior: Linux/macOS/Windows, symlinks, executable bits
  -> start with Repository facade
  -> use feature gate: core + worktree-mutation
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 42.3 Value cases



* **cross-platform code tooling.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **checkout.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **status.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **index stat acceleration.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 42.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 42.5 Failure modes and edge cases



* **executable-bit expectations on Windows.**

* **case collision.**

* **Unicode normalization mismatch.**

* **symlink privilege/Developer Mode differences.**

* **NTFS reparse points.**



## 42.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 42.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 42.8 Anti-pattern inventory



* **Avoid:** executable-bit expectations on Windows.

* **Avoid:** case collision.

* **Avoid:** Unicode normalization mismatch.

* **Avoid:** symlink privilege/Developer Mode differences.

* **Avoid:** NTFS reparse points.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 42.9 Agent checklist



```text
[ ] CI Linux/macOS/Windows
[ ] Use repository-detected capabilities
[ ] Treat path bytes/normalization carefully
[ ] Test case-only renames
[ ] Stay on patched checkout versions
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 42.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/security/advisories/GHSA-pmm9-4h7q-24c8

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 43) Security and governance for untrusted repositories and remotes



## 43.0 Scope and mental model



Opening a repository is not equivalent to trusting its configuration, paths or commands. Security-sensitive systems should combine gix trust filtering with their own path/network/executable/resource policy.



**Feature gate / dependency posture:** `trust model + selected external/network features`.



## 43.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `repo.trust` | part of this capability surface; verify exact signature in pinned rustdoc |

| `open::Options::bail_if_untrusted` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix::sec` | part of this capability surface; verify exact signature in pinned rustdoc |

| `command_context` | part of this capability surface; verify exact signature in pinned rustdoc |

| `credentials` | part of this capability surface; verify exact signature in pinned rustdoc |

| `checkout options` | part of this capability surface; verify exact signature in pinned rustdoc |



## 43.2 Canonical selection pattern



```text
Need: Security and governance for untrusted repositories and remotes
  -> start with Repository facade
  -> use feature gate: trust model + selected external/network features
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 43.3 Value cases



* **scan arbitrary user repositories.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **SaaS code intelligence.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **CI artifact analysis.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **remote clone service.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 43.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 43.5 Failure modes and edge cases



* **command/filter/credential helper execution.**

* **SSRF via remote URL.**

* **path traversal/checkouts.**

* **zip/archive bombs and huge blobs/packs.**

* **resource exhaustion.**

* **terminal/log injection from arbitrary bytes.**



## 43.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 43.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 43.8 Anti-pattern inventory



* **Avoid:** command/filter/credential helper execution.

* **Avoid:** SSRF via remote URL.

* **Avoid:** path traversal/checkouts.

* **Avoid:** zip/archive bombs and huge blobs/packs.

* **Avoid:** resource exhaustion.

* **Avoid:** terminal/log injection from arbitrary bytes.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 43.9 Agent checklist



```text
[ ] Open untrusted repos with explicit trust policy
[ ] Disable/allowlist external commands
[ ] Allowlist network schemes/hosts
[ ] Sandbox checkout/output paths
[ ] Set size/time/file-count limits
[ ] Escape terminal/log output
[ ] Pin security floor
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 43.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/security/advisories/GHSA-pmm9-4h7q-24c8

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 44) Production deployment patterns in Rust



## 44.0 Scope and mental model



Production architecture should choose a repository-handle lifetime, trust domain, network policy, mutation rights, cache budget and concurrency budget explicitly. A code-indexing service should usually be read-mostly and treat fetch/checkout as separate controlled jobs.



**Feature gate / dependency posture:** `profile-dependent`.




### 44.1 Read-mostly code-intelligence service

For repository analysis, the strongest pattern is to use immutable object IDs as computation boundaries and isolate mutation:

```text
network/mutation plane
  clone/fetch -> validate -> bump repository generation

read plane
  open exact repo generation
  resolve IDs
  traverse objects/commits/trees
  status/diff only when worktree semantics are required
  emit detached immutable records
```

This prevents a long-running analysis from accidentally depending on a moving branch name. Resolve `HEAD`/branch/ref to an `ObjectId` at job start and use that ID throughout the job unless “follow branch changes live” is explicitly desired.

### 44.2 Repository pool

A pool should key by canonical repository identity plus generation/config/trust domain—not merely filesystem path. Repositories with different allowed config sources, credential scopes, namespaces or object-cache budgets should not silently share mutable policy.

### 44.3 Mutation bot

For a bot that creates commits:

```text
1. resolve expected branch tip
2. write blobs
3. write/edit tree
4. write commit
5. CAS-like reference update with expected old target
6. optional index/worktree synchronization
7. push through separately supported mechanism
```

If step 5 loses a race, do not force the ref. Re-resolve and rebase/merge at the application layer according to policy.

## 44.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Repository` | main high-level repository handle |

| `ThreadSafeRepository` | thread-safe repository resource container |

| `status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `rev_parse_single` | part of this capability surface; verify exact signature in pinned rustdoc |

| `prepare_clone` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote::connect` | part of this capability surface; verify exact signature in pinned rustdoc |

| `tracing` | part of this capability surface; verify exact signature in pinned rustdoc |



## 44.2 Canonical pattern



```text
Recommended code-intelligence service split

Repository manager
  -> exact-path open/discover policy
  -> trust/config policy
  -> per-repo version/generation

Fetch/clone worker (network + mutation)
  -> bounded concurrency
  -> URL/credential allowlist
  -> interrupt + timeout
  -> atomic publication/reopen

Read/index workers
  -> object/revision/diff/status
  -> object cache + commit graph
  -> detached immutable results

API layer
  -> no arbitrary external command or checkout path
  -> size/time/result caps
```



## 44.3 Value cases



* **local CLI.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **code-intelligence daemon.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **multi-tenant Git analysis API.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **clone/fetch worker.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **mutation bot.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 44.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 44.5 Failure modes and edge cases



* **one global mutable repo state across tenants.**

* **arbitrary clone URLs.**

* **unbounded status/diff.**

* **mixing mutation and serving without locks/versioning.**



## 44.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 44.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 44.8 Anti-pattern inventory



* **Avoid:** one global mutable repo state across tenants.

* **Avoid:** arbitrary clone URLs.

* **Avoid:** unbounded status/diff.

* **Avoid:** mixing mutation and serving without locks/versioning.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 44.9 Agent checklist



```text
[ ] Define per-repo ownership
[ ] Separate read and mutation workers
[ ] Pin versions/features
[ ] Instrument cache/concurrency
[ ] Use immutable object IDs as durable job inputs
[ ] Reopen/reload after external Git mutations
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 44.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# gix Advanced — 45) Complete `gix-*` plumbing-crate architecture catalog



## 45.0 Scope and mental model



The facade re-exports many plumbing crates. Use them directly only when the lower abstraction is the actual product boundary or when measured performance/control requires it; otherwise the facade avoids wiring dozens of stores/options/caches manually.



**Feature gate / dependency posture:** `subcrates`.



## 45.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix-object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-odb` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-ref` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-config` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-index` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-diff` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-revision` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-worktree` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-protocol` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-transport` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix-pack` | part of this capability surface; verify exact signature in pinned rustdoc |



## 45.2 Canonical selection pattern



```text
Need: Complete `gix-*` plumbing-crate architecture catalog
  -> start with Repository facade
  -> use feature gate: subcrates
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 45.3 Value cases



* **custom Git server/storage.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **specialized pack analyzer.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **no-Repository library layer.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **compile-time minimization.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 45.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 45.5 Failure modes and edge cases



* **version skew across gix-* crates.**

* **duplicating facade policy.**

* **lower-level security defaults misunderstood.**

* **upgrade churn.**



## 45.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 45.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 45.8 Anti-pattern inventory



* **Avoid:** version skew across gix-* crates.

* **Avoid:** duplicating facade policy.

* **Avoid:** lower-level security defaults misunderstood.

* **Avoid:** upgrade churn.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 45.9 Agent checklist



```text
[ ] Prefer facade first
[ ] Pin compatible versions together
[ ] Use release Cargo manifest as dependency family map
[ ] Keep adapter module around direct plumbing use
[ ] Test against facade semantics
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 45.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 46) Migration from `git2` / libgit2 and API-equivalence strategy



## 46.0 Scope and mental model



Migration from `git2` is conceptual, not mechanical. gix often distinguishes attached/detached objects, platform builders, trust-aware configuration and feature-gated subsystems more explicitly. The official docs intentionally tag many known `git2` equivalents for search.



**Feature gate / dependency posture:** `varies`.



## 46.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `docs.rs `?search=git2`` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::find_object` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Repository::references` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Reference::peel_to_*` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Remote` | configured/unnamed remote abstraction |

| `status` | part of this capability surface; verify exact signature in pinned rustdoc |

| `rev_parse_single` | part of this capability surface; verify exact signature in pinned rustdoc |



## 46.2 Canonical selection pattern



```text
Need: Migration from `git2` / libgit2 and API-equivalence strategy
  -> start with Repository facade
  -> use feature gate: varies
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 46.3 Value cases



* **replace libgit2 C dependency.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **Rust-native code intelligence.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **incremental migration.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 46.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 46.5 Failure modes and edge cases



* **transliterating method names without semantics.**

* **assuming push/checkout porcelain parity.**

* **ignoring byte strings.**

* **expecting Repository to be Sync.**



## 46.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 46.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 46.8 Anti-pattern inventory



* **Avoid:** transliterating method names without semantics.

* **Avoid:** assuming push/checkout porcelain parity.

* **Avoid:** ignoring byte strings.

* **Avoid:** expecting Repository to be Sync.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 46.9 Agent checklist



```text
[ ] Search gix docs for `git2` equivalences
[ ] Migrate read-only operations first
[ ] Add parity tests per operation
[ ] Keep Git CLI fallback for unsupported porcelain
[ ] Revisit fallback on gix upgrades
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 46.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 47) Capability matrix: implemented plumbing vs incomplete porcelain workflows



## 47.0 Scope and mental model



This matrix is a deployment gate. Implemented examples include discovery, object/ref/index/config access, status, blob/tree diff, revision parsing/walking/merge-base, fetch/clone plumbing, worktree streams/archives and low-level checkout. Tracked incomplete workflows include complete checkout/switch/restore/reset orchestration, merge porcelain state orchestration, rebase/cherry-pick/revert/bisect/stash/am/apply flows, hooks, full push plumbing, and broader Git-3.0/partial-clone/bundle work.



**Feature gate / dependency posture:** `project status`.




### 47.1 Capability matrix

| Workflow / capability | gix/Gitoxide status to assume for architecture | Safe agent stance |
|---|---|---|
| open/discover/init | strong high-level support | use gix directly |
| read/write objects | strong plumbing | use Repository; drop to gix-odb/object for specialized work |
| read/write refs | strong plumbing | use constraints/transactions |
| read/write index | strong plumbing | test extensions/conflict stages/concurrency |
| rev-parse / history walk / merge-base | strong | use gix directly |
| status | high-level platform exists | use gix directly; configure cost knobs |
| blob/tree diff + rename tracking | available | use gix; reuse caches |
| blame | available but comparatively early | pin/test performance and edge cases |
| mailmap | mature plumbing | use directly |
| attributes / ignore / pathspec | broad support | use Git-native semantics, not regex approximations |
| worktree stream/archive | available | choose archive container features directly |
| low-level checkout | available | require >=0.86; not full switch/reset porcelain |
| merge blobs/trees/commits | available behind merge | not full `git merge` workflow state machine |
| fetch | available high-level | use network feature + interruption/resource controls |
| clone | available staged builder | understand cleanup and blocking behavior |
| push | incomplete workflow/plumbing per current project status | fallback or explicit low-level ownership |
| rebase | incomplete workflow orchestration | fallback |
| cherry-pick/revert | incomplete orchestration | fallback |
| bisect | incomplete orchestration | implement app logic or fallback |
| stash | incomplete orchestration | fallback |
| `git am` / `git apply` complete workflow | incomplete | fallback |
| hooks | project status still tracks broader hook support | do not assume Git-hook parity |
| partial clone/promisor/bundles | incomplete cross-cutting area | verify exact scenario; avoid assumption |
| Git SHA-256/reftable full future format | incomplete cross-cutting area | `sha256` feature alone is insufficient proof |

### 47.2 Agent decision algorithm

```text
request says “do Git operation X”
  -> list state domains X mutates
  -> check gix rustdoc for exact APIs
  -> check Cargo feature gates
  -> check crate-status for workflow completeness
  -> if all domains are orchestrated: use gix workflow
  -> if only plumbing exists:
       either implement explicit state machine + parity tests
       or use Git/libgit2 fallback
```

## 47.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `crate-status.md` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix facade` | part of this capability surface; verify exact signature in pinned rustdoc |



## 47.2 Canonical selection pattern



```text
Need: Capability matrix: implemented plumbing vs incomplete porcelain workflows
  -> start with Repository facade
  -> use feature gate: project status
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 47.3 Value cases



* **architecture decisions.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **fallback policy.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **agent capability checks.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 47.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 47.5 Failure modes and edge cases



* **hallucinating missing method.**

* **mistaking crate name placeholder for implementation.**

* **using CLI checklist marks as API guarantee.**



## 47.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 47.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 47.8 Anti-pattern inventory



* **Avoid:** hallucinating missing method.

* **Avoid:** mistaking crate name placeholder for implementation.

* **Avoid:** using CLI checklist marks as API guarantee.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 47.9 Agent checklist



```text
[ ] Read current crate-status before starting workflow
[ ] Classify request as plumbing vs porcelain
[ ] Build missing workflow only with explicit state-machine design
[ ] Use Git CLI fallback for unsupported high-risk mutation
[ ] Update capability tests during upgrade
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 47.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 48) API stability, upgrades, MSRV, feature drift, and release migration



## 48.0 Scope and mental model



The project follows semver, but a pre-1.0 family with many plumbing crates can still have meaningful API movement across minor releases. Feature relationships and current-main workflow status can drift independently of the released crate.



**Feature gate / dependency posture:** `all`.



## 48.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `Cargo.lock` | part of this capability surface; verify exact signature in pinned rustdoc |

| `Cargo.toml exact pin` | part of this capability surface; verify exact signature in pinned rustdoc |

| `docs.rs version selector` | part of this capability surface; verify exact signature in pinned rustdoc |

| `crate changelogs` | part of this capability surface; verify exact signature in pinned rustdoc |

| `crate-status` | part of this capability surface; verify exact signature in pinned rustdoc |



## 48.2 Canonical selection pattern



```text
Need: API stability, upgrades, MSRV, feature drift, and release migration
  -> start with Repository facade
  -> use feature gate: all
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 48.3 Value cases



* **dependency updates.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **agent reference refresh.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **workspace maintenance.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 48.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 48.5 Failure modes and edge cases



* **using latest docs against pinned older crate.**

* **letting compatible range update silently break snapshots.**

* **mixed gix-* family versions.**

* **missing security bump.**



## 48.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 48.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 48.8 Anti-pattern inventory



* **Avoid:** using latest docs against pinned older crate.

* **Avoid:** letting compatible range update silently break snapshots.

* **Avoid:** mixed gix-* family versions.

* **Avoid:** missing security bump.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 48.9 Agent checklist



```text
[ ] Exact-pin in generated reference environments
[ ] Commit Cargo.lock for applications
[ ] Run cargo update intentionally
[ ] Diff feature manifest
[ ] Run parity/integration/security tests
[ ] Refresh crate-status gap table
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 48.10 Primary sources



* https://docs.rs/crate/gix/0.86.0/source/Cargo.toml.orig

* https://docs.rs/crate/gix/0.86.0/features



---



# gix Advanced — 49) CLI `gix` / `ein` as debugging tools, not stable scripting contracts



## 49.0 Scope and mental model



The project describes `gix` and `ein` binaries as development/human tools that may remain unstable and explicitly says not to rely on them in scripts. Use the library API for stable application integration; use CLI commands for debugging and real-repo validation.



**Feature gate / dependency posture:** `gitoxide binary package, not gix library`.



## 49.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `gix CLI` | part of this capability surface; verify exact signature in pinned rustdoc |

| `ein CLI` | part of this capability surface; verify exact signature in pinned rustdoc |

| `gix --help` | part of this capability surface; verify exact signature in pinned rustdoc |



## 49.2 Canonical selection pattern



```text
Need: CLI `gix` / `ein` as debugging tools, not stable scripting contracts
  -> start with Repository facade
  -> use feature gate: gitoxide binary package, not gix library
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 49.3 Value cases



* **manual inspection.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **benchmarking/validation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **learning plumbing behavior.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 49.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 49.5 Failure modes and edge cases



* **production scripts parsing unstable CLI output.**

* **assuming CLI command coverage equals library completeness.**

* **shipping accidental gitoxide binary dependency.**



## 49.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 49.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 49.8 Anti-pattern inventory



* **Avoid:** production scripts parsing unstable CLI output.

* **Avoid:** assuming CLI command coverage equals library completeness.

* **Avoid:** shipping accidental gitoxide binary dependency.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 49.9 Agent checklist



```text
[ ] Use library in production integration
[ ] Use CLI only as debug oracle unless intentionally version-pinned
[ ] Do not parse human output
[ ] Record binary version in benchmarks
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 49.10 Primary sources



* https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md

* https://github.com/GitoxideLabs/gitoxide/blob/main/README.md



---



# gix Advanced — 50) Final best practices, anti-patterns, and LLM-agent execution playbook



## 50.0 Scope and mental model



The final agent playbook prioritizes correctness boundaries: version pin, feature gate, trust model, byte semantics, repository lifetime, plumbing-vs-porcelain classification, explicit mutation transactions, resource limits, and cross-platform tests.



**Feature gate / dependency posture:** `all`.




### 50.1 Non-negotiable invariants

1. **Pin and verify.** Code against `gix 0.86.0` APIs in this reference, not memory of older/newer Gitoxide.
2. **No invented porcelain.** Search rustdoc/all-items and crate-status before writing a method for a Git command.
3. **Bytes first.** Object content, commit messages, ref/path components and worktree names may not be UTF-8.
4. **Trust is explicit.** Untrusted repository config must not silently gain command/filter/credential execution privileges.
5. **Object write != ref update != index update != checkout != push.** These are separate state transitions.
6. **Use CAS-like ref constraints.** Do not blind-force branches after reading an old tip.
7. **Treat Repository as thread-local.** Use `into_sync`/fresh handles/detached payloads deliberately.
8. **Network features are an application policy.** Pick one client/TLS family; libraries should not casually force it.
9. **Async transport does not make Git CPU/filesystem work non-blocking.** Isolate it.
10. **Checkout requires the 0.86 security floor.** Test Windows symlink/reparse-point cases.
11. **Bound hostile repositories.** Bytes, objects, depth, paths, files, time, memory and external processes all need limits.
12. **Use Git as oracle.** For semantics that matter, test gix results and on-disk state against canonical Git.

## 50.1 Key public surface



| API / type | Agent interpretation |

|---|---|

| `all prior sections` | part of this capability surface; verify exact signature in pinned rustdoc |



## 50.2 Canonical selection pattern



```text
Need: Final best practices, anti-patterns, and LLM-agent execution playbook
  -> start with Repository facade
  -> use feature gate: all
  -> verify exact method in gix 0.86 rustdoc
  -> drop to gix-* plumbing only for missing control/performance
  -> consult crate-status if this is a multi-state Git workflow
```



## 50.3 Value cases



* **LLM code generation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **review/validation.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **architecture selection.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.

* **production hardening.** Use this surface when its semantics match the requested state domain; preserve immutable object/ref identities at boundaries rather than passing mutable repository assumptions implicitly.



## 50.4 Planning and ownership rules



```text
Identify the Git state domain before selecting the method.
Verify Cargo feature availability in the resolved build, not just source manifest.
Treat repository-attached values as short-lived convenience handles.
Preserve byte-oriented Git data unless a text contract is explicit.
Separate lookup/planning from mutation/publication.
For long operations, define interruption and resource limits.
```



## 50.5 Failure modes and edge cases



* **invented API names.**

* **unbounded work.**

* **implicit external execution.**

* **unsafe checkout/network path.**

* **incorrect Git semantics.**



## 50.6 Testing matrix



```text
[ ] normal repository / happy path
[ ] bare repository where meaningful
[ ] unborn or empty repository where meaningful
[ ] missing/corrupt/stale object or ref input
[ ] non-UTF8 or unusual byte path/name where platform permits
[ ] concurrent Git process or stale snapshot behavior
[ ] minimal Cargo feature build for this section
[ ] cross-platform behavior if filesystem/worktree is touched
```



## 50.7 Deployment advisory



| Deployment | Recommended stance |

|---|---|

| local developer tool | convenience facade is appropriate; rich diagnostics allowed |

| code-indexing daemon | read-mostly, exact-path open, caches, bounded traversal, detached results |

| multi-tenant service | isolate trust/config/network/mutation policy per tenant/repository |

| mutation bot | explicit object→tree→commit→ref transaction; lock and race handling |

| untrusted repository scanner | disable/allowlist external execution; no arbitrary checkout/network paths; strict resource caps |



## 50.8 Anti-pattern inventory



* **Avoid:** invented API names.

* **Avoid:** unbounded work.

* **Avoid:** implicit external execution.

* **Avoid:** unsafe checkout/network path.

* **Avoid:** incorrect Git semantics.

* **Avoid:** guessing an API from a Git command name instead of checking rustdoc.

* **Avoid:** using `unwrap()` on repository data that can be malformed or missing.

* **Avoid:** converting all paths/messages/content to UTF-8 at ingestion.

* **Avoid:** changing more Git state domains than the operation contract documents.

* **Avoid:** assuming debug/Display output is a stable serialization format.



## 50.9 Agent checklist



```text
[ ] Confirm exact API in 0.86 rustdoc
[ ] Confirm feature is enabled
[ ] Confirm workflow is implemented
[ ] Confirm trust/network/path policy
[ ] Confirm byte/path semantics
[ ] Confirm concurrency model
[ ] Confirm tests cover corrupt and cross-platform cases
[ ] Confirm the exact gix 0.86 API and feature gate before emitting code
[ ] Confirm byte/path semantics
[ ] Confirm thread/async ownership model
[ ] Confirm failure and cancellation behavior
[ ] Add Git-parity integration test when behavior is user-visible
```



## 50.10 Primary sources



* https://docs.rs/gix/0.86.0/gix/

* https://docs.rs/gix/0.86.0/gix/struct.Repository.html



---



# Appendix A — High-level `gix` facade inventory



This is a discovery index, not a claim that every re-export is enabled under every feature set. Feature-gated modules/types disappear from builds where their component is disabled. Use the release all-items page as the final authority.



## A.1 Primary facade types



* `Repository`

* `ThreadSafeRepository`

* `ObjectId` / `oid`

* `Id`

* `Object` / `ObjectDetached`

* `Blob`

* `Tree`

* `Commit`

* `Tag`

* `Reference`

* `Head`

* `Remote`

* `Worktree`

* `Pathspec` / `PathspecDetached` (attributes feature)

* `Submodule` (attributes feature)



## A.2 Public high-level modules



* `clone`

* `commit`

* `config`

* `create`

* `diff`

* `dirwalk`

* `filter`

* `head`

* `id`

* `init`

* `mailmap`

* `merge`

* `object`

* `open`

* `pathspec`

* `push`

* `reference`

* `remote`

* `repository`

* `revision`

* `shallow`

* `state`

* `status`

* `submodule`

* `tag`

* `worktree`



## A.3 Major plumbing re-exports



* `actor`

* `attrs` / attributes

* `blame`

* `command`

* `commitgraph`

* `credentials`

* `date`

* `dir`

* `error`

* `features`

* `fs`

* `glob`

* `hash`

* `hashtable`

* `ignore`

* `index`

* `lock`

* `negotiate`

* `objs` / object plumbing

* `odb`

* `prompt`

* `protocol`

* `refs`

* `refspec`

* `revwalk`

* `sec`

* `tempfile`

* `trace`

* `traverse`

* `url` / `Url`

* `utils`

* `validate`

* `zlib`

* `bstr`



---



# Appendix B — `gix-*` plumbing-crate catalog and selection guide



| Crate | Domain | Use it directly when… |

|---|---|---|

| `gix-actor` | actors/signatures | parse/write author/committer identities |

| `gix-archive` | archive containers | consume worktree stream into archive formats; select container features directly |

| `gix-attributes` | gitattributes | parse/match attributes |

| `gix-blame` | blame | line attribution; comparatively early maturity |

| `gix-chunk` | chunk files | Git chunk-file structures |

| `gix-command` | external commands | Git-compatible command preparation/spawn |

| `gix-commitgraph` | commit graph | accelerated commit metadata/reachability |

| `gix-config` | configuration | parse/layer/query/write Git config |

| `gix-config-value` | config values | typed config value parsing |

| `gix-credentials` | credential helpers | Git credential helper protocol |

| `gix-date` | dates | parse/format Git times |

| `gix-diff` | diff | tree/blob diff and rewrite tracking |

| `gix-dir` | directory walk | Git-aware worktree walking |

| `gix-discover` | repo discovery | locate repo/git-dir/worktree |

| `gix-error` | error infrastructure | error trees/chains |

| `gix-features` | shared features | parallelism, progress, fs helpers, tracing |

| `gix-filter` | filters | clean/smudge/process-filter primitives |

| `gix-fs` | filesystem | filesystem capabilities/snapshots/helpers |

| `gix-glob` | glob | Git glob matching |

| `gix-hash` | hash IDs | SHA-1/SHA-256 identifiers and hashing |

| `gix-hashtable` | hash tables | specialized maps/sets |

| `gix-ignore` | gitignore | ignore/exclude parsing/search |

| `gix-index` | index | decode/edit/write .git/index |

| `gix-lock` | locks | lockfile consistency; project labels as production-grade tier 1 |

| `gix-mailmap` | mailmap | identity canonicalization |

| `gix-merge` | merge | blob/tree/commit merge primitives |

| `gix-negotiate` | fetch negotiation | want/have algorithms |

| `gix-object` | object model | zero-copy refs + owned encode/decode |

| `gix-odb` | object database | loose/pack lookup stores and caches |

| `gix-pack` | packfiles | pack/index/delta/MIDX operations |

| `gix-packetline` | pkt-line | Git protocol packet framing |

| `gix-path` | paths | Git path normalization/conversion |

| `gix-pathspec` | pathspec | Git pathspec parsing/search |

| `gix-prompt` | prompt | credential/interactive prompt integration |

| `gix-protocol` | wire protocol | fetch/ls-refs protocol client logic |

| `gix-ref` | references | loose/packed refs, reflogs, transactions |

| `gix-refspec` | refspecs | parse/match/transform refspecs |

| `gix-revision` | revisions | revspec parsing, describe, merge-base helpers |

| `gix-revwalk` | revision walk | graph traversal infrastructure |

| `gix-sec` | security/trust | trust and external-process permission policy |

| `gix-shallow` | shallow repos | shallow boundary read/write |

| `gix-status` | status | index/worktree and tree/index status plumbing |

| `gix-submodule` | submodules | config/state/open primitives |

| `gix-tempfile` | tempfiles | registered tempfiles/signal cleanup; project labels as production-grade tier 2 |

| `gix-trace` | trace facade | lightweight tracing abstraction |

| `gix-transport` | transport | file/git/ssh/http transport client plumbing |

| `gix-traverse` | traversal | tree/object traversal |

| `gix-url` | URLs | Git URL parse/normalize |

| `gix-utils` | utilities | shared helpers |

| `gix-validate` | validation | Git names/path validation |

| `gix-worktree` | worktree | worktree stack/index/ignore/attribute support |

| `gix-worktree-state` | checkout/status state | worktree materialization/state transitions |

| `gix-worktree-stream` | worktree streams | tree -> worktree byte-stream conversion |

| `gix-zlib` | compression | zlib integration |



### B.1 Release-family versions used by `gix 0.86.0`



The released manifest pins compatible family ranges including: `gix-ref ^0.66.0`, `gix-discover ^0.54.0`, `gix-tempfile ^24.0.0`, `gix-lock ^24.0.0`, `gix-sec ^0.14.2`, `gix-refspec ^0.44.0`, `gix-config ^0.59.0`, `gix-odb ^0.83.0`, `gix-hash ^0.26.0`, `gix-shallow ^0.13.0`, `gix-object ^0.63.0`, `gix-pack ^0.73.0`, `gix-revision ^0.48.0`, `gix-revwalk ^0.34.0`, `gix-negotiate ^0.34.0`, `gix-url ^0.37.0`, `gix-traverse ^0.60.0`, `gix-diff ^0.66.0`, and `gix-merge ^0.19.0`. If adding direct plumbing dependencies, derive versions from the **released** `gix 0.86.0` manifest rather than current `main`.



---



# Appendix C — `Repository` method-family map for agent discovery



## C.1 identity/layout



```text
git_dir
work_dir
common_dir
path
kind
is_bare
trust
state
namespace
set_namespace
object_hash
```



## C.2 HEAD/refs



```text
head
head_id
head_commit
head_tree
head_tree_id
head_tree_id_or_empty
head_name
find_reference
try_find_reference
references
reference
edit_reference
edit_references
```



## C.3 objects



```text
find_object
try_find_object
find_header
try_find_header
find_blob
find_tree
find_commit
find_tag
has_object
write_object
write_blob
write_blob_stream
empty_blob
empty_tree
```



## C.4 revisions



```text
rev_parse
rev_parse_single
rev_walk
revision_graph
merge_base
merge_bases_many
commit_graph
commit_graph_if_enabled
```



## C.5 index/worktree



```text
index
open_index
index_from_tree
index_or_load_from_head
index_or_load_from_head_or_empty
worktree
worktrees
checkout_options
filesystem_options
stat_options
```



## C.6 status/diff



```text
status
is_dirty
is_pristine
index_worktree_status
tree_index_status
diff_tree_to_tree
diff_resource_cache
diff_algorithm
```



## C.7 paths/attributes



```text
pathspec
normalize_path
attributes
attributes_only
excludes
dirwalk_options
filter_pipeline
```



## C.8 merge/blame/mailmap



```text
merge_commits
merge_trees
merge_resource_cache
blob_merge_options
tree_merge_options
blame_file
open_mailmap
```



## C.9 remotes



```text
remote_names
remote_default_name
remote_at
remote_at_without_url_rewrite
find_remote
try_find_remote
branch_remote_name
branch_remote_ref_name
ssh_connect_options
transport_options
```



## C.10 config/runtime



```text
config_snapshot
config_snapshot_mut
reload
command_context
open_options
object_cache_size
object_cache_size_if_unset
pack_compression
set_freelist
```



## C.11 submodules



```text
submodules
modules
open_modules_file
```



## C.12 worktree stream/archive



```text
worktree_stream
worktree_archive
```



## C.13 commit/mutation



```text
commit
new_commit
edit_tree
tag
tag_reference
```



Agent rule: this appendix is a discovery index only. Verify exact signatures, lifetimes, feature gates and errors in `gix 0.86.0` rustdoc before emitting code.



---



# Appendix D — Cargo and deployment recipes



## D.1 Read-only local code-intelligence engine



```toml
[dependencies]
gix = { version = "=0.86.0", default-features = false, features = [
  "sha1",
  "revision",
  "index",
  "blob-diff",
  "attributes",
  "status",
  "parallel",
  "auto-chain-error",
] }
```



Use exact-path `gix::open()` in a daemon. Resolve refs to object IDs at job start. Use object cache and commit graph for repeated history queries. Avoid worktree status when the task is purely historical/object-based.



## D.2 Clone/fetch worker with HTTPS



```toml
[dependencies]
gix = { version = "=0.86.0", features = [
  "blocking-http-transport-reqwest-rust-tls",
  "tracing",
] }
```



Run clone/fetch in a dedicated worker or blocking pool. Apply URL/credential allowlists and byte/time limits. Reopen the repository after successful publication to consumers.



## D.3 Lean reusable library



```toml
[features]
default = []
git-revisions = ["gix/revision", "gix/sha1"]

[dependencies]
gix = { version = "0.86", default-features = false }
```



Do not force HTTP/TLS/runtime/performance features downstream. Expose feature forwarding and keep gix types private if long-term API stability across gix minor versions is more important than zero-cost type exposure.



## D.4 High-performance batch analyzer



```toml
[dependencies]
gix = { version = "=0.86.0", features = [
  "max-performance",
  "cache-efficiency-debug",
  "tracing",
] }

[profile.release]
lto = "thin"
codegen-units = 1
```



Benchmark cache sizes and parallelism with representative repositories. Remove `cache-efficiency-debug` after tuning if its diagnostics are not required.



---



# Appendix E — Security threat model



| Boundary | Threat | Required control |

|---|---|---|

| repository config | external commands, filters, credential helpers, path settings | trust-aware config + application allowlist |

| remote URL | SSRF, credential exfiltration, unexpected scheme/host | validate effective rewritten URL; allowlist schemes/hosts |

| checkout target | path traversal, symlink/reparse-point escape | patched >=0.86, sandbox path, no arbitrary destination |

| object database | decompression/resource bombs, corrupt objects | size/time/object-count caps; error handling |

| worktree traversal | millions of files, symlink/device weirdness | bounded walk, no follow outside policy, timeout |

| commit/ref names/messages | terminal/log injection, non-UTF8 | byte-safe handling + escaped rendering |

| archive output | huge files/count, traversal-sensitive consumer | stream with caps; sanitize at consumer boundary |

| external Git fallback | argument/env/config injection | no shell string; fixed executable; sanitized env; `--`/explicit args where applicable |

| credentials | secrets in logs/redirects | redaction; endpoint binding; noninteractive policy |

| concurrency | lock races / stale refs / partial publication | previous-value constraints, gix locks, explicit state machine |



---



# Appendix F — Final LLM coding-agent prompt fragment



```text

When implementing Git functionality with gix:

1. Pin behavior to gix 0.86.0 unless the project manifest says otherwise.
2. Check the exact API in version-matched rustdoc before writing code.
3. Check the resolved Cargo features; never assume network/merge/status/etc. are compiled in.
4. Classify the request into state domains: objects, refs, index, worktree, config, network, operation-state files.
5. Consult Gitoxide crate-status for multi-domain porcelain workflows. Do not invent missing high-level methods.
6. Preserve Git byte strings and object IDs; do not assume UTF-8 or SHA-1-width strings at internal boundaries.
7. Treat Repository as thread-local; use ThreadSafeRepository, per-worker handles, or detached payloads deliberately.
8. For mutation, compose bottom-up immutable object writes and constraint-checked ref publication; index/worktree/push are separate steps.
9. For untrusted repositories/remotes, preserve gix trust filtering and add application-level command/network/path/resource restrictions.
10. For checkout/worktree mutation, require gix >=0.86.0 and test Windows symlink/reparse-point collisions.
11. For async services, isolate blocking Git computation/filesystem work on a blocking pool/worker.
12. Use Git CLI as a parity oracle and fallback for workflows not yet complete in gix.
13. Test corrupt, bare, unborn, shallow, linked-worktree, non-UTF8, conflict-stage, concurrent-writer and cross-platform cases.
14. Never rely on gix/ein CLI output as a stable scripting interface; the project explicitly treats those binaries as unstable development/human tools.
```



# End of reference
