---
artifact: design-dossier
design_id: codefabric-build-cache-and-feature-isolation
version: v1
date: 2026-08-20
status: accepted
baseline_commit: a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91+working-tree-0e3b3b28b976ed7426730dcdea997513060db4eeca16bba2ffe764907dc3eacd
primary_scope:
  - Cargo.toml
  - .cargo/config.toml
  - fuzz/
  - rustc-extractor/
  - src/
  - tests/integration.rs
  - justfile
  - scripts/
  - .github/workflows/ci.yml
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric build-cache and feature-isolation design

## 1. Executive decision

CodeFabric will remain one root Cargo package and one library crate. The root package will
expose additive capability features for canonical JSON, contract tooling, local RPC,
repository state, the data fabric, and compatibility probes. `local-workstation` will remain
the default aggregate and preserve the complete production dependency graph required by the
accepted Wave 0 design, including DataFusion and local Delta support but excluding the Delta
S3 implementation.

Narrow tools will select only the capability they execute. In particular, the contract CLI
will select `contracts-tooling`, the JCS fuzzer will select `canonical-json`, and the Protobuf
generator will select `proto-tooling`, all with default features disabled. Source modules and
the single integration-test target will use the same gates.

The stable root package and stable Pyrefly sidecar will share the repository-root `target/`
directory. Dated-nightly extractor artifacts, nightly assurance artifacts, and
sanitizer/fuzz artifacts will remain in separate target subdirectories. `sccache` will retain
its host-global local cache; the repository will not set `SCCACHE_DIR`. CI will disable Cargo
incremental compilation so the CI sccache backend receives cacheable compilation units.

This correction is accepted for implementation during WP06 of plan v4.

## 2. Problem, outcomes, and non-goals

The full root graph resolves 514 packages. The JCS fuzz package currently resolves 517 because
its path dependency selects the root package without a feature that excludes the data-fabric
stack. A first fuzz build took 22 minutes 59 seconds while an identical second build took
0.79 seconds. This proves that warm reuse works, but it also shows that the wrong graph is being
built for narrow tasks. The stable Pyrefly sidecar separately resolves 378 packages in its own
target directory despite sharing 184 exact package identities with the root graph.

The observable outcomes are:

- contract verification, contract generation, and JCS fuzzing do not resolve DataFusion,
  Delta Lake, Arrow, Parquet, object-store, gix, rusqlite, or tonic unless their selected
  capability requires them;
- the default build continues to compile the accepted complete local-workstation graph;
- `s3-storage` remains an explicit opt-in and activates the approved Delta S3 path;
- root and sidecar stable builds report one target directory, while incompatible toolchain or
  sanitizer domains report distinct target directories;
- CI contract work uses narrow features and CI sets `CARGO_INCREMENTAL=0`;
- the repository keeps one top-level Rust integration-test target and one root package.

Non-goals are changing production semantics, reducing the complete default product graph,
introducing a Cargo workspace or another production crate, changing dependency pins, adopting
a repository-local or remote developer sccache, deleting existing build artifacts, or making
license-policy changes.

## 3. Constraints and measurable quality attributes

- Repo-spec §0.3 and §61.2 require one package unless a second package has independent build,
  release, platform, reuse, dependency-isolation, or measured compilation justification.
  Feature isolation satisfies the present need without changing package architecture.
- FAB §2.1 and §2.2 require exact DataFusion 54.1.0, Arrow/Parquet 58.4.0,
  object_store 0.13.2, and the pinned delta-rs revision in one public type universe.
- The accepted local-storage correction D-11 requires the default local graph to retain Delta
  with `rustls` and `datafusion` while omitting `deltalake/s3` and the AWS SDK path.
- Cargo feature composition must be additive under resolver 3. No feature may disable another
  feature or change a public type's identity.
- A narrow graph is conforming only when `cargo tree` proves heavyweight package families are
  absent and the corresponding target compiles/tests.
- Cache reuse is acceleration only. Correctness gates remain valid with an empty cache.

## 4. Current-state evidence and architecture

The root manifest has four features and 37 direct dependencies; most production dependencies
are unconditional. `src/lib.rs` unconditionally publishes `compatibility`, `contracts`, and
`rpc`, and unconditionally compiles the Delta and gix implementation boundaries. The one
integration-test target unconditionally includes compatibility, contract, and RPC test modules.

`fuzz/Cargo.toml` selects `codefabric` with default features disabled but does not select a
narrow capability, so it still inherits every unconditional root dependency. The Protobuf and
contract recipes build bins without disabling the default aggregate. CI repeats those full-root
builds in a dedicated contracts job.

The root, sidecar, fuzz, and extractor currently use four target roots. `sccache` is active and
uses the host-global cache at `~/Library/Caches/Mozilla.sccache`; its observed overall hit rate
was 41.80 percent and its Rust hit rate was 30.41 percent. The cache is working. Stable versus
nightly compilers, AArch64 versus x86_64 targets, development versus release profiles, and
sanitizer flags intentionally form different compiler-cache keys.

## 5. Target architecture

```text
                         local-workstation (default)
                                  |
                +-----------------+------------------+
                |                                    |
       contracts-tooling                    compatibility-probes
                |                            /       |       \
        canonical-json              data-fabric    rpc    repository-state

proto-tooling  -> generator-only build, no default product features
s3-storage     -> data-fabric + deltalake/s3, explicit opt-in

stable root -----------+
                       +--> repository target/ + host-global sccache
stable Pyrefly sidecar-+

dated-nightly extractor --> target/extractor/
nightly assurance       --> target/nightly-assurance/
fuzz/sanitizer by host  --> target/fuzz/<rustc-host>/
```

`compatibility-probes` is a temporary executable proof surface that aggregates the production
capabilities used by the Wave 0 compatibility module. It is not a new domain layer. Later
packets may remove individual probes when production modules provide stronger compile and
behavioral evidence.

## 6. Target invariants, contracts, ownership, and flows

- **I-01 — One-package boundary.** Root production code remains one Cargo package and one
  library crate; sidecar, extractor, and fuzz remain independent build domains rather than
  workspace members.
- **I-02 — Default-product fidelity.** `default = ["local-workstation"]`, and the default
  aggregate compiles every currently accepted local production capability and compatibility
  probe.
- **I-03 — Narrow capability closure.** Each narrow feature activates all and only the direct
  dependencies and source modules needed for that capability. `contracts-tooling` includes
  `canonical-json`; no inverse dependency exists.
- **I-04 — Local/cloud boundary.** `data-fabric` activates the exact Delta `rustls` and
  `datafusion` profile. Only `s3-storage` activates `deltalake/s3`.
- **I-05 — Test topology.** `tests/integration.rs` remains the only top-level integration-test
  crate and conditionally includes its existing subsystem modules.
- **I-06 — Cache correctness.** Stable root and sidecar may share artifact storage because
  Cargo fingerprints compiler, target, profile, features, and dependency inputs. Nightly,
  rustc-private, and sanitizer domains are separated operationally to avoid high-churn
  contention and ambiguous launch paths.
- **I-07 — Global cache ownership.** The developer environment owns sccache storage and
  eviction. The repository owns only the wrapper declaration, telemetry recipes, and CI
  cacheability settings.
- **I-08 — Native fuzzing.** Fuzz recipes derive the target from the selected nightly rustc
  host and pass it explicitly. Cross-architecture fuzzing requires an explicit caller override,
  not a hidden default.
- **I-09 — CI feature fidelity.** Contract and Protobuf CI commands use the same narrow feature
  selections as local recipes.

The feature table in `Cargo.toml` is the authority. Module `cfg` attributes, bin
`required-features`, test-module gates, just recipes, CI commands, and graph governance are
consumers and must agree with it.

## 7. Library/platform capability decisions

- **LD-01 — Adopt Cargo optional dependencies and additive features.** Cargo natively omits an
  optional dependency when no selected feature activates `dep:<name>`. This displaces custom
  wrapper manifests and avoids a package split.
- **LD-02 — Adopt Cargo target-directory sharing for compatible stable domains.** Cargo's
  fingerprinted target layout provides safe artifact coexistence. Root `.cargo/config.toml`
  owns the shared stable target. Domain-specific environment/config owns incompatible target
  roots.
- **LD-03 — Retain sccache as a host-global compiler cache.** Its compiler-key model correctly
  separates toolchains, targets, flags, and source inputs. A repo-local cache would duplicate
  storage and reduce cross-checkout reuse without making incompatible compilations compatible.
- **LD-04 — Retain exact DataFusion/Arrow/Delta pins.** Feature isolation changes selection,
  not versions. DataFusion/Arrow reference guidance requires one aligned public type universe;
  delta-rs guidance requires explicit `datafusion` and explicit cloud features.
- **LD-05 — Use Cargo's resolved graph as the oracle.** `cargo metadata` and `cargo tree` replace
  assumptions based on manifest text or target-directory size.

## 8. Alternatives and decision rationale

### Alternative A — Feature-isolated one-package root (selected)

This preserves the accepted package architecture, gives narrow tools dependency-closed graphs,
and uses Cargo's built-in optional-dependency and fingerprint machinery. Its main cost is an
explicit feature/module/test governance table.

### Alternative B — Keep the monolithic graph and rely only on sccache

This has the smallest manifest diff but makes every new target architecture, compiler, profile,
or CI job pay for unrelated heavy crates at least once. It treats a working compiler cache as a
substitute for dependency design and fails the narrow-graph outcome.

### Alternative C — Split contract/JCS/data-fabric code into new workspace crates

This offers strong package-level isolation but conflicts with the present one-package contract,
adds workspace and release/build topology, and is not required to obtain the measured benefit.
It may be reconsidered only if later measurements show feature isolation cannot contain rebuilds
or a subsystem gains independent reuse, platform, artifact, or release requirements.

## 9. Clean-sheet challenge

Without the current implementation, a single product package with additive capability features
would still be preferred: all capabilities ship as one daemon/data-plane artifact, while
developer tools need smaller compile closures. Separate build domains remain justified only for
the compiler-private extractor, independently sourced Pyrefly sidecar, and cargo-fuzz harness.
The selected design is therefore not preserving a legacy package shape at the expense of a
cleaner target.

Relevant doctrine: the design **advances** Principle 1 (Information Hiding), Principle 4 (High
Cohesion and Low Coupling), Principle 5 (Dependency Direction), Principle 22 (Resource
Lifecycle), Principle 25 (Reproducibility and Hermeticity), Principle 30 (Testability), and
Principle 31 (Executable Governance). It **maintains** Principle 10 (single-sourced feature
authority) and Principle 29 (declared contracts). It mitigates the anti-principle of diffuse
operational authority by keeping cache ownership explicit.

## 10. Legacy disposition matrix

| ID | Current surface | Disposition | Target/exit condition |
|---|---|---|---|
| L-01 | Unconditional heavyweight root dependencies | reshape | Every capability-owned dependency is optional and selected by one named feature. |
| L-02 | Unconditional root modules | reshape | Module availability follows the owning feature. |
| L-03 | Unconditional integration modules | reshape | Existing modules remain under the one test target and are feature-gated. |
| L-04 | Full-root contract/Protobuf recipes | replace | Recipes disable defaults and select the narrow feature. |
| L-05 | Fuzz path dependency with no selected capability | replace | It selects only `canonical-json`. |
| L-06 | Separate stable sidecar target | replace | Sidecar metadata resolves the root target directory. Existing files need not be deleted. |
| L-07 | Separate extractor/fuzz targets | preserve/reshape | Keep isolation, but place them under explicit root target subdirectories and derive native fuzz host. |
| L-08 | Host-global sccache | preserve | No repository `SCCACHE_DIR`; telemetry remains available. |
| L-09 | CI incremental default | replace | Workflow-level `CARGO_INCREMENTAL=0`. |

## 11. Transition, cutover, rollback, and decommission

The cutover is atomic at the manifest/consumer level: add the feature table and optional flags,
gate source/test modules, update every narrow command, and strengthen graph governance in one
change. Regenerate lockfiles only as Cargo requires; dependency versions do not change.

Then set the shared stable target directory, add an extractor override, update direct-binary
launch paths, and isolate nightly/fuzz recipes. Existing sidecar, extractor, and fuzz target
directories are stale build products and are not deleted as part of the correction.

Rollback is a source/config revert. Build products are caches and need no data migration. A
partial rollback is forbidden if it leaves a recipe selecting a feature whose module or bin is
unavailable.

## 12. Failure, security, resource lifecycle, observability, and performance

Unknown or incomplete feature combinations fail at compile time. Required-feature bins are
skipped by Cargo unless their feature is selected. Graph governance fails with a named missing
or forbidden package family. A direct launcher fails with a precise instruction when its
domain-specific binary is absent.

No trust boundary changes. The global cache may contain compiler outputs from other checkouts,
but Cargo and sccache validate keys rather than treating cached bytes as authority. CI uses its
existing managed cache backend and disables incremental state, which is job-local and not a
correctness input.

Target directories are generated, ignored, and disposable. The developer owns host-global
sccache capacity and may inspect it with `just cache-stats`. Routine execution must not invoke
`cargo clean`; controlled clean builds use isolated temporary target directories.

Performance evidence is graph-based first: package-family absence is deterministic and explains
why compile work cannot occur. Timings are supporting telemetry because machine load and cache
state vary.

## 13. Test oracle and conformance strategy

- Compile the featureless root and each named feature in isolation.
- Compile and test the default aggregate.
- Run contract unit/integration checks with only `contracts-tooling`.
- Build and run the JCS fuzzer with only `canonical-json`, explicit native host, and isolated
  fuzz target directory.
- Use `cargo tree --no-default-features --features <feature>` to prove required packages present
  and forbidden package families absent.
- Prove the default graph still excludes `deltalake-aws`/AWS SDK and the S3 graph includes them.
- Prove root and sidecar metadata share a target directory, while extractor metadata and fuzz
  recipes point at separate target roots.
- Run the repository's root formatting, check, Clippy, test, doctest, dependency-hygiene, and
  stable-graph gates after the cutover.

## 14. Risks, assumptions, and design-level replan triggers

- **A-01.** Root and sidecar stable builds use compatible host toolchains. Validate from their
  metadata and CI toolchain setup. If false, give the sidecar an explicit separate target root.
- **A-02.** cargo-fuzz supports the selected nightly host on the current platform. Validate with
  a build and bounded run. If false, record and select an explicit supported cross target rather
  than silently reverting globally.
- **R-01.** A dependency used by two capabilities may be assigned too narrowly. Mitigation:
  each-feature checks and compiler errors enumerate missing closure.
- **R-02.** Cargo heuristics may flag feature-gated dependencies as unused. Mitigation: compiled
  feature tests remain authority; scanner exceptions require a named feature-gated rationale.
- **R-03.** Shared target locks can serialize simultaneous root and sidecar builds. Measure
  persistent contention before separating them; duplicate compilation is not preferred by
  default.

Reopen the design if one feature cannot compile independently without activating the full data
fabric, if stable target sharing causes measured persistent contention or artifact corruption,
if a capability requires a distinct released artifact, or if a supported native fuzz target is
unavailable.

## 15. Acceptance decision and open blockers

**Accepted for implementation.** The user approved all six operational changes and the design
correction. No design blocker remains. External Ubuntu clean-checkout proof remains an execution
milestone constraint and is not a blocker to implementing this local correction.

## 16. Evidence ledger

| ID | Claim | Status | Evidence | Coverage/limits | Used by |
|---|---|---|---|---|---|
| E-01 | Root graph resolves 514 packages and has 37 direct dependencies | observed | `cargo metadata --locked`; `Cargo.toml` | Current dirty-tree snapshot | D-01, I-03 |
| E-02 | Sidecar graph resolves 378 packages in a separate target and shares 184 exact package identities with root | observed | paired Cargo metadata comparison | Package identity, not compiled-unit identity | D-02, A-01 |
| E-03 | Fuzz graph resolves 517 packages and first/second builds took 22m59s/0.79s | observed | WP06 local build telemetry | One machine and cache state | D-01, R-03 |
| E-04 | sccache is active at a host-global path with 41.80% overall and 30.41% Rust hit rates | observed | `sccache --show-stats` and cache config | Snapshot, not a guaranteed future hit rate | D-03 |
| E-05 | Cargo optional features and target-dir are built-in selection/storage mechanisms | observed | Cargo reference plus local metadata probes | Exact behavior remains compiler-tested | LD-01, LD-02 |
| E-06 | Delta's DataFusion and cloud integrations are explicit features | observed | pinned delta-rs reference §0.6-§0.7 and local lockfile | Pinned revision only | I-04, LD-04 |
| E-07 | DataFusion/Arrow public types require one aligned version family | observed | DataFusion Rust §1; PyArrow Rust §1-§2; FAB §2.2 | Selected versions only | I-04, LD-04 |
| E-08 | Root config commits sccache and current domains use four target roots | observed | `.cargo/config.toml`; Cargo metadata | Before cutover | D-02, I-06 |
