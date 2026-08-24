---
artifact: design-dossier
design_id: codefabric-build-cache-and-feature-isolation
version: v2
date: 2026-08-24
status: accepted
baseline_commit: c0d902ca78c9c72d56fd25c162e5f2a7acf62746
primary_scope:
  - Cargo.toml
  - .cargo/config.toml
  - rustc-extractor/
  - src/
  - tests/integration.rs
  - justfile
  - scripts/
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric build-cache and feature-isolation design v2

## 1. Successor decision

This dossier replaces v1's dependency-version assumption while preserving its package, feature,
test-target, and cache architecture. CodeFabric remains one stable root package and library crate;
the dated-nightly extractor remains an independent Cargo root and target. No workspace or second
integration-test crate is introduced.

The default `local-workstation` aggregate now resolves DataFusion 55.0.0, Arrow/Parquet 59.2.0,
`object_store` 0.13.2, and exact delta-rs `43a0cf10…`. The extractor independently resolves
Arrow 59.2.0 for its IPC producer boundary. `s3-storage` remains explicit opt-in.

## 2. Feature decisions

- All stable-root data-fabric dependencies remain optional and owned by `data-fabric`.
- `model-compiler` adds the already-declared optional `toml` dependency so the model-owned
  AC-G-07 toolchain identity can strictly derive root and extractor manifest/lock pins.
- Narrow canonical JSON, contract, model, and Proto graphs continue to omit DataFusion, Delta,
  Arrow, Parquet, and unrelated runtime families.
- Features remain additive under resolver 3 and never select a second public type universe.
- The root and stable sidecar share `target/`; the dated-nightly extractor stays under
  `target/extractor/`; revision compatibility builds use the ignored revision target.

## 3. Proof and rollback

`stable-graph-check`, `features-each`, featureless/narrow checks, extractor compilation, the
single-target structural oracle, and cross-revision IPC tests prove isolation. Cache reuse is not
correctness evidence; the same checks must pass from exact locks.

A mixed Arrow/DataFusion family, target-directory merger, new Cargo root, feature subtraction,
default Delta AWS activation, or narrow-graph data-fabric leak is a replan condition. Before target
writes, rollback is the WP01 proving commit. After target writes, runtime rollback uses the
preserved old namespace and WP05 compatibility evidence.

## 4. Legacy disposition

The v1 dossier remains historical. Its DataFusion 54.1.0 and Arrow/Parquet 58.4.0 wording is
superseded; its one-package, one-test-target, additive-feature, and cache-domain decisions remain
in force through this successor.
