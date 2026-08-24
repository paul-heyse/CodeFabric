---
artifact: design-dossier
design_id: codefabric-model-driven-artifact-and-assurance-control-plane
version: v2
date: 2026-08-24
status: accepted
baseline_commit: c0d902ca78c9c72d56fd25c162e5f2a7acf62746
primary_scope:
  - src/bin/codefabric_model/
  - contracts/toolchain/toolchain-identity.json
  - contracts/generated/model/
  - Cargo.toml
  - Cargo.lock
  - rustc-extractor/Cargo.toml
  - rustc-extractor/Cargo.lock
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric model-driven artifact and assurance control plane design v2

## 1. Successor decision

This dossier preserves the v1 sole-writer, typed-projection, transactional synchronization, and
reproducibility design. It supersedes LD-06's Arrow 58.4.0 version basis with Arrow 59.2.0 and
closes the missing AC-G-07 data-fabric identity projection.

## 2. Toolchain identity projection

`codefabric.toolchain.identity` remains a released 1.x manifest and receives an additive
`data_fabric` member derived by the native aggregate driver. The member records:

- `rust_version`, `datafusion_version`, `arrow_version`, `parquet_version`,
  `object_store_version`, `toml_version`;
- exact `delta_rs_git_rev` plus the resolved `deltalake_declared_version`;
- root Cargo manifest and lock digests; and
- extractor package version, exact toolchain channel, manifest digest, lock digest, toolchain-file
  digest, and canonical extractor identity digest.

The renderer parses TOML strictly from repository-model source bytes. No generated identity field
is handwritten. Changing either Cargo root or the extractor toolchain changes the toolchain
artifact digest, toolchain bundle digest, and build/deployment bundle chain without changing fact,
ontology, query, or schema IDs.

## 3. Library decisions

- Arrow 59.2.0 constructs and validates schema projections; DataFusion 55.0.0 may validate table
  consumption but does not own the repository model.
- Pydantic, Protobuf descriptor IR, canonical JSON, and registry/CBEF family decisions from v1
  remain unchanged.
- The existing optional `toml` crate is enabled in `model-compiler` for native manifest/lock
  projection. This does not pull the data-fabric family into the narrow compiler graph.

## 4. Proof, compatibility, and legacy disposition

Unit tests assert exact target fields and prove extractor-lock mutation changes the extractor
identity digest. `model-plan` must report the complete derived cascade; confirmed `model-sync` is
the sole writer; a follow-up plan must contain zero actions. Model reproducibility and release
checks run at the proving commit and HEAD.

The v1 dossier remains historical. Its Arrow 58.4.0 LD-06 basis and incomplete generated
toolchain projection are superseded. Its other accepted architecture and all stable artifact IDs
remain in force.
