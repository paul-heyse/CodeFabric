---
artifact: design-dossier
design_id: codefabric-local-storage-dependency-isolation
version: v2
date: 2026-08-24
status: accepted
baseline_commit: c0d902ca78c9c72d56fd25c162e5f2a7acf62746
primary_scope:
  - docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - scripts/stable_graph_check.sh
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric local-storage dependency isolation design v2

## 1. Successor decision

This dossier supersedes v1 only where v1 names the predecessor DataFusion/Arrow/delta graph.
Its application-owned storage-authority decision remains unchanged.

The accepted stack is delta-rs revision
`43a0cf10a313e5077c48637ad786a05359136bbb`, DataFusion 55.0.0, Arrow/Parquet 59.2.0,
`object_store` 0.13.2, `buoyant_kernel` 0.25.1, and `buoyant_kernel_engine` 0.25.0. The kernel
pair selects `arrow-59` transitively.

`local-workstation-v1` authorizes only local filesystem Delta namespaces. It does not enable
`deltalake/s3`; its resolved graph contains neither `deltalake-aws` nor AWS SDK packages. The
kernel still selects latent `object_store` `aws`, `azure`, `gcp`, and `http` features. Compiled
capability is reported honestly and does not become provider authority.

## 2. Invariants

- Default resolution contains no `deltalake-aws`, `aws-config`, or `aws-sdk-*` package.
- `s3-storage` is the only CodeFabric feature enabling `deltalake/s3`; that graph contains the
  Delta AWS implementation and AWS SDK.
- The released kernel pair is selected only through the exact delta-rs pin and lock; CodeFabric
  does not declare it directly.
- Both kernel crates activate `arrow-59` and never `arrow-58`.
- Default metadata reports the latent `object_store` cloud features rather than claiming they are
  absent.
- Local configuration rejects cloud schemes, credentials, endpoints, and storage-option maps
  before provider construction.
- Every RustSec exception is exact-version bound, machine-checked, and owned by WP06 before M05.

## 3. Evidence and failure policy

`scripts/stable_graph_check.sh` proves exact source, package, kernel, Arrow-feature, default/S3,
and latent-feature facts from Cargo metadata and trees. `scripts/advisory_policy_check.sh` proves
the exception registry equals `deny.toml`, the lock, and current RustSec output. Negative provider
tests prove operational authority.

A different delta source, split kernel line, `arrow-58`, default AWS implementation package, or
unowned security exception fails closed. Rollback serves the preserved old namespace; no old
binary writes a target-stack namespace.

## 4. Legacy disposition

The v1 dossier remains historical evidence for the predecessor graph. Its `arrow-58`, prior
delta revision, and old review milestone are not current authority. All other provider-isolation,
least-privilege, and honest-reporting decisions are inherited unchanged.
