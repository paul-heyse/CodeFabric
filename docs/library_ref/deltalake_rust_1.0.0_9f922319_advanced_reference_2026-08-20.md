# 0. Version, feature, and compatibility baseline — Rust `deltalake` / `delta-rs`

Target: **LLM programming agents generating Rust code against Delta Lake tables inside a DataFusion + Arrow codebase.**
Primary objective: **prevent syntax drift, dependency skew, feature-gate omissions, incompatible Arrow/DataFusion types, and cloud-storage misconfiguration before writing any downstream deep dives.**

---

## 0.1 Canonical baseline: `1.0.0` @ git rev `9f922319…` (pre-release pin)

Use **`deltalake`/`deltalake-core` `1.0.0`, pinned to git rev `9f9223197469897ef05ae4369eb4fd1390174e65`**, as the documentation target. This SHA is the tip of protected `main` verified on **2026-08-20** (commit timestamp 2026-08-19T19:29:14Z). It remains a **pre-release pin**: the delta-rs workspace declares crate version `1.0.0`, but there is still no tagged `rust-v1.0.0` release and the repository `CHANGELOG.md` still begins with `rust-v0.32.3` (2026-05-19). For post-0.32.3 / pre-1.0 behavior, the exact pinned source and the commit range from the prior pin are the authoritative records. Expect the eventual tagged 1.0.0 release to add changes beyond this SHA.

Pin-refresh procedure when moving the baseline forward: (1) choose the new delta-rs rev; (2) update `rev = "…"` in the `deltalake` / `deltalake-core` git pins; (3) re-resolve the released `buoyant_kernel` and `buoyant_kernel_engine` 0.25.x dependencies selected by that delta-rs revision rather than carrying a separate kernel git SHA; (4) run `cargo update -p deltalake -p deltalake-core`; (5) re-run the duplicate-dependency gates in §0.8; (6) re-verify every doc-referenced API surface against the new checkout before restamping this document.

The pinned-rev workspace baseline is:

```toml
# deltalake 1.0.0 @ rev 9f922319 workspace-level compatibility anchor

rust-version = "1.94.1"
edition = "2024"

arrow = "58"        # resolves to 58.3.0 alongside DataFusion 54
parquet = "58"
datafusion = "54.0.0"
datafusion-datasource = "54.0.0"
datafusion-physical-expr-adapter = "54.0.0"
datafusion-ffi = "54.0.0"
datafusion-proto = "54.0.0"
object_store = "0.13.2"
tokio = "1"

# Delta kernel: released 0.25.x crates; default engine is now a separate package.
delta_kernel = { package = "buoyant_kernel", version = "0.25.0,<0.25.100", features = ["arrow-58", "internal-api"] }
delta_kernel_default_engine = { package = "buoyant_kernel_engine", version = "0.25.0,<0.25.100", features = ["arrow-58", "rustls"], default-features = false }
```

This is not optional metadata: the rev’s root `Cargo.toml` sets `rust-version = "1.94.1"`, `edition = "2024"`, Arrow-family crates at `58`, `parquet = "58"`, `object_store = "0.13.2"`, and DataFusion-family crates at `54.0.0`. The Delta kernel is now the released `buoyant_kernel` 0.25.x line with `arrow-58` + `internal-api`, while its default engine is split into released `buoyant_kernel_engine` 0.25.x; the repository’s `rust-toolchain.toml` pins channel `1.94.1` with `rustfmt` and `clippy`. The Rust floor was raised from 1.91.1 after upstream AWS crates increased their MSRV. ([GitHub][2])

**Agent rule:** do not generate examples using older DataFusion APIs, older Arrow constructors, or older `object_store` registration patterns without explicitly marking them as legacy.

---

## 0.2 Canonical `Cargo.toml` profiles

### 0.2.1 Local filesystem + Arrow-only table inspection

Use when loading tables, inspecting schema/history/files, and writing Arrow batches without DataFusion SQL/planning integration.

```toml
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
tracing = "0.1"
```

Default `deltalake` enables `rustls` only; docs.rs lists one default feature, `rustls`. ([Docs.rs][3])

### 0.2.2 DataFusion + Arrow integration baseline

Use for a codebase where Delta tables are registered as DataFusion table providers, queried with SQL/DataFrame APIs, written from plans, or used by merge/update/delete expressions.

```toml
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion"] }

# Align with the delta-rs 1.0.0 pinned-rev workspace.
datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
tracing = "0.1"
```

The `datafusion` feature in `deltalake-core` gates optional dependencies on `datafusion`, `datafusion-datasource`, `datafusion-physical-expr-adapter`, and `datafusion-proto`; therefore DataFusion examples must either enable `deltalake/datafusion` or fail compilation at the import/API surface. ([GitHub][4])

### 0.2.3 S3 + DataFusion production baseline

```toml
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

`deltalake`’s `s3` feature enables `deltalake-aws/rustls` and `rustls`; `s3-native-tls` instead enables `deltalake-aws/native-tls` plus `native-tls`. Pick exactly one TLS posture unless you have a deliberate corporate CA/native TLS reason. ([GitHub][5])

### 0.2.4 Multi-cloud exploratory/dev baseline

```toml
[dependencies]
deltalake = {
  git = "https://github.com/delta-io/delta-rs.git",
  rev = "9f9223197469897ef05ae4369eb4fd1390174e65",
  features = [
    "datafusion",
    "s3",
    "azure",
    "gcs",
    "hdfs",
    "lakefs",
    "json"
  ]
}

datafusion = "=54.0.0"
arrow = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
```

Use this for integration-test discovery, not minimal production services. The published wrapper crate exposes optional feature gates for `azure`, `gcs`, `hdfs`, `lakefs`, `glue`, `unity-experimental`, `json`, `nanosecond-timestamps`, `native-tls`, `s3`, and `s3-native-tls`. ([Docs.rs][3])

---

## 0.3 `deltalake` vs `deltalake-core`

### Practical rule

Use **`deltalake`** in application crates. Treat **`deltalake-core`** as the lower-level implementation crate unless writing internal extensions, embedding, custom crate work, or contributing to `delta-rs`.

The public docs repeatedly describe the `deltalake` crate as a **meta-package shim for `deltalake-core`**, while the visible public surface re-exports modules such as `arrow`, `datafusion`, `delta_datafusion`, `kernel`, `logstore`, `operations`, `protocol`, `table`, and `writer`. ([Docs.rs][6])

### Dependency shape

At the pinned rev (`9f922319…`), the wrapper crate is named `deltalake`, version `1.0.0`, and depends on `deltalake-core` plus optional backend/catalog crates including `deltalake-aws`, `deltalake-azure`, `deltalake-gcp`, `deltalake-hdfs`, `deltalake-lakefs`, `deltalake-opendal`, `deltalake-catalog-glue`, and `deltalake-catalog-unity`. ([GitHub][5])

Application import posture:

```rust
// preferred application imports
use deltalake::open_table;
use deltalake::DeltaTable;
use deltalake::DeltaTableError;
use deltalake::operations;
use deltalake::kernel;
```

Avoid this unless you are intentionally binding to internals:

```rust
// avoid in app-level docs unless explicitly required
use deltalake_core::*;
```

### Agent rule

When documenting syntax, write against `deltalake` public exports first. Only mention `deltalake-core` when documenting implementation-layer extension points, feature-gate internals, log-store internals, or crate-contribution workflows.

---

## 0.4 Version pinning policy

### Recommended for reproducible documentation

```toml
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "s3"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
```

Use exact pins for documentation examples, CI golden tests, and generated code templates. Use caret ranges only in application repos with a tested upgrade workflow.

### Recommended for production workspace

```toml
[workspace.dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
tokio = "1"
url = "2"
tracing = "0.1"
```

Then in member crates:

```toml
[dependencies]
deltalake.workspace = true
datafusion.workspace = true
arrow.workspace = true
arrow-array.workspace = true
arrow-schema.workspace = true
parquet.workspace = true
object_store.workspace = true
tokio.workspace = true
```

### Why exact pins matter

Arrow/DataFusion/Rust lakehouse crates tend to expose type identities across crate boundaries: `SchemaRef`, `RecordBatch`, `TableProvider`, `ExecutionPlan`, `ObjectStore`, `ObjectStoreUrl`, `SessionContext`, `SessionState`, `Expr`, and Parquet writer properties. A minor version skew can produce “same conceptual type, different crate version” compile failures or trait-bound failures.

Agent diagnostic pattern:

```text
Symptom:
  expected arrow_array::record_batch::RecordBatch, found arrow_array::record_batch::RecordBatch

Likely cause:
  duplicate Arrow versions in Cargo.lock.

Action:
  cargo tree -d
  cargo tree -i arrow-array
  cargo tree -i datafusion
  align Arrow/DataFusion/Parquet/ObjectStore versions to delta-rs baseline.
```

---

## 0.5 Rust edition and MSRV contract

The repository root at the pinned rev declares `edition = "2024"` and `rust-version = "1.94.1"`; `rust-toolchain.toml` pins `channel = "1.94.1"` and includes `rustfmt` and `clippy`. This is a post-baseline MSRV increase: commit `e8072a63…` raised the workspace/toolchain from 1.91.1 to 1.94.1 because the AWS dependency line raised its MSRV. ([GitHub][2])

### Required repository files

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy"]
```

```toml
# Cargo.toml
[package]
edition = "2024"
rust-version = "1.94.1"
```

### CI baseline

```bash
rustup show
cargo +1.94.1 fmt --all -- --check
cargo +1.94.1 clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.94.1 test --workspace --all-features
```

### Agent rule

Do not emit Rust 2021 assumptions for this documentation target. Use Rust 2024-compatible syntax and tooling assumptions unless explicitly writing a migration note.

---

## 0.6 Feature flag matrix

### Wrapper crate feature map

`deltalake` 1.0.0 at the pinned rev exposes 34 feature flags, with `rustls` as the only default feature; the growth over 0.32.x (22 flags) is almost entirely the new `opendal` backend family (`opendal`, `opendal-fs`, `opendal-memory`, `opendal-s3`, `opendal-gcs`, `opendal-azblob`, `opendal-azdls`, `opendal-oss`, `opendal-obs`, `opendal-cos`, `opendal-tos`, `opendal-b2`, `opendal-swift`, `opendal-webhdfs`, `opendal-webdav`, `opendal-ftp`, `opendal-sftp`, `opendal-hf` — see §2.23). ([Docs.rs][3])

| Feature                 | Enables                                         | Use case                                            | Production guidance                 |
| ----------------------- | ----------------------------------------------- | --------------------------------------------------- | ----------------------------------- |
| `rustls`                | `deltalake-core/rustls`                         | default TLS stack                                   | preferred default                   |
| `native-tls`            | `deltalake-core/native-tls`                     | OS-native TLS roots/corporate PKI                   | use when required                   |
| `datafusion`            | `deltalake-core/datafusion`                     | DataFusion query/DML/planning integration           | required for your codebase          |
| `datafusion-ext`        | `datafusion`                                    | compatibility alias/extension posture               | avoid unless docs/API require it    |
| `s3`                    | `deltalake-aws/rustls`, `rustls`                | AWS S3, MinIO, R2-style object storage              | preferred S3 feature                |
| `s3-native-tls`         | `deltalake-aws/native-tls`, `native-tls`        | S3 with native TLS                                  | use instead of `s3` when mandated   |
| `azure`                 | `deltalake-azure`                               | Azure Blob / ADLS Gen2 / OneLake                    | enable only in Azure builds         |
| `gcs`                   | `deltalake-gcp`                                 | Google Cloud Storage                                | enable only in GCP builds           |
| `hdfs`                  | `deltalake-hdfs`                                | HDFS                                                | environment-specific                |
| `lakefs`                | `deltalake-lakefs`                              | lakeFS integration                                  | environment-specific                |
| `glue`                  | `deltalake-catalog-glue`                        | AWS Glue catalog                                    | catalog-specific                    |
| `unity-experimental`    | `deltalake-catalog-unity`                       | Unity Catalog experimental path                     | isolate behind experimental feature |
| `json`                  | `deltalake-core/json` → `parquet/json`          | JSON/Parquet JSON support                           | enable only if needed               |
| `python`                | `deltalake-core/python` → Arrow PyArrow support | Python binding/interop build path                   | usually not for pure Rust apps      |
| `nanosecond-timestamps` | `deltalake-core/nanosecond-timestamps`          | experimental nanosecond timestamp primitive support | test cross-engine compatibility     |
| `deltalake-aws` etc.    | direct optional dependency features             | internal feature names                              | do not prefer in app docs           |

The wrapper’s Cargo file documents that many wrapper features are reflected into the core crate “until that functionality is broken apart,” which is an API-stability warning: do not overfit documentation to internal feature plumbing. ([GitHub][5])

### Core crate feature map

`deltalake-core` 1.0.0 at the pinned rev retains the same public core feature names as the earlier 0.32.x line: `datafusion`, `datafusion-ext`, `json`, `python`, `native-tls`, `rustls`, `cloud`, `delta-cache`, `nanosecond-timestamps`, and `integration_test`. The plumbing changed with kernel 0.25: `native-tls` / `rustls` now forward to the separate `delta_kernel_default_engine` package rather than feature names embedded in `delta_kernel`; `datafusion` still pulls the optional DataFusion crates, `json` maps to `parquet/json`, `python` maps to `arrow/pyarrow`, and `delta-cache` enables `foyer`, `tempfile`, and `url/serde`. ([GitHub][4])

---

## 0.7 Feature selection recipes

### 0.7.1 Minimal local Delta table tooling

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65" }
```

Use for:

```text
open table
inspect metadata
inspect schema
list active files
history
time travel
basic Arrow writes if supported by path
```

Do not use for:

```text
DataFusion SQL
DataFusion TableProvider
merge/update/delete SQL expression evaluation docs
DataFusion plan writes
cloud object-store access
```

### 0.7.2 DataFusion-native service

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
```

Use for:

```text
SessionContext registration
TableProvider integration
SQL/DataFrame reads
DML expression evaluation
DeltaScan / scan planning
DataFusion plan write paths
predicate/projection pushdown verification
```

Delta Lake’s DataFusion integration docs state that Delta Lake works with the DataFusion Rust and Python APIs, and that Delta uses DataFusion internally for SQL-related features such as update/merge expressions and constraints/invariants. ([Delta IO][7])

### 0.7.3 S3 + DataFusion

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "s3"] }
```

Use for:

```text
AWS S3-backed Delta tables
MinIO-backed integration tests
Cloudflare R2-style S3-compatible backends
DataFusion query over object storage
transaction-log/object-store integration
```

### 0.7.4 Native TLS S3

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["datafusion", "s3-native-tls"] }
```

Use for:

```text
enterprise TLS root stores
corporate MITM/proxy trust chains
native OS certificate store requirements
```

### 0.7.5 Azure

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "azure"] }
```

Use for:

```text
Azure Blob
ADLS Gen2
Microsoft OneLake
```

The project feature/support table lists Azure Blob, ADLS Gen2, and Microsoft OneLake as supported storage integrations for Rust. ([GitHub][8])

### 0.7.6 GCS

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "gcs"] }
```

Use for:

```text
Google Cloud Storage-backed Delta tables
GCP object-store registration
GCS integration tests
```

The project support table lists Google Cloud Storage as a Rust-supported storage integration. ([GitHub][8])

### 0.7.7 Glue catalog

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "s3", "glue"] }
```

Use for:

```text
AWS Glue catalog resolution
catalog-to-table-location lookup
S3 table storage + Glue metadata
```

### 0.7.8 Unity Catalog experimental

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion", "unity-experimental"] }
```

Use for:

```text
experimental Unity Catalog integration tests
non-critical exploratory catalog workflows
```

Agent rule: label all Unity examples as experimental.

---

## 0.8 Cargo.lock and dependency-skew controls

### Required checks

```bash
cargo tree -d

cargo tree -i deltalake
cargo tree -i deltalake-core
cargo tree -i datafusion
cargo tree -i arrow
cargo tree -i arrow-array
cargo tree -i arrow-schema
cargo tree -i parquet
cargo tree -i object_store
```

### Failure classes

| Failure                            | Likely cause                                              | Fix                                    |
| ---------------------------------- | --------------------------------------------------------- | -------------------------------------- |
| duplicate `arrow-array`            | transitive crate on older Arrow                           | align workspace deps; upgrade offender |
| duplicate `datafusion`             | direct app version differs from `deltalake` feature graph | pin to `54.0.0` for this baseline      |
| `TableProvider` trait mismatch     | DataFusion version skew                                   | one DataFusion version only            |
| `RecordBatch` type mismatch        | Arrow version skew                                        | one Arrow version only                 |
| object-store URL/registry mismatch | object_store version/API skew                             | align `object_store = 0.13.2`          |
| TLS duplicate/feature conflict     | `rustls` + `native-tls` unintentionally mixed             | choose one TLS posture                 |
| missing cloud scheme               | feature not enabled                                       | enable `s3`, `azure`, `gcs`, etc.      |
| missing DataFusion APIs            | `datafusion` feature omitted                              | enable `deltalake/datafusion`          |

### CI gate

```bash
#!/usr/bin/env bash
set -euo pipefail

cargo tree -d | tee /tmp/cargo-tree-duplicates.txt

if grep -E 'arrow|datafusion|object_store|parquet' /tmp/cargo-tree-duplicates.txt; then
  echo "Duplicate lakehouse dependency detected. Align deltalake/DataFusion/Arrow/ObjectStore."
  exit 1
fi
```

---

## 0.9 API stability risk zones

### 0.9.1 DataFusion integration surface

High risk because DataFusion APIs evolve quickly, and `delta-rs` has recently changed its table-provider implementation. The `rust-v0.32.0` release notes (historical) called out improvements to the new “next” DataFusion `TableProvider`; the 1.0.0 line completes that pivot — the pinned rev **removes the legacy `DeltaTableProvider` type entirely** (PR #4435), routes reads through the kernel-backed provider (`DeltaScanExec`), and moves the workspace to DataFusion 54 / Arrow 58.3 / `object_store` 0.13. The legacy physical `DeltaScan` serialization codec (`DeltaPhysicalCodec`) is `#[deprecated]` at the rev — use `DeltaLogicalCodec` for table-provider serialization. For the DataFusion 53→54 API changes that affect snippets in this document (notably the `as_any` removal from `TableProvider`/`ExecutionPlan`/UDF traits in favor of an `Any` supertrait + trait upcasting, and `ExecutionPlan::partition_statistics`), see the companion catalog `docs/library_ref/datafusion_54vs53.md`. ([GitHub][1])

Documentation posture:

```text
Do:
  - pin DataFusion 54.0.0
  - compile every example
  - include feature=["datafusion"]
  - test table registration and query collection
  - include EXPLAIN/physical-plan smoke tests
  - consult docs/library_ref/datafusion_54vs53.md for DF 53->54 breaking changes

Do not:
  - copy older DeltaTableProvider examples blindly (the type no longer exists at the pinned rev)
  - assume DataFusion 49/50/52/53 examples compile
  - assume extension/provider internals remain stable
  - use the deprecated DeltaPhysicalCodec for new plan-serialization code
```

### 0.9.2 `DeltaOps` / operations API surface

The docs.rs public item list marks `DeltaOps` as deprecated while still describing it as the high-level interface for commands against `DeltaTable`; therefore operation examples should be compiled against the exact tag and should prefer current operation-specific builders if deprecation warnings require it. ([Docs.rs][6])

Documentation posture:

```text
For each operation section:
  - compile with RUSTFLAGS="-D warnings" only after resolving deprecation posture
  - record exact builder type path
  - avoid undocumented re-export assumptions
  - include version note when using deprecated compatibility API
```

### 0.9.3 Table protocol feature compatibility

The project feature table tracks protocol support, including append-only tables, column invariants, checkpoint stats properties, CHECK constraints, Change Data Feed, generated columns, table features, and partial/unlisted statuses for column mapping / identity columns depending on reader/writer version rows. ([Docs.rs][9])

Documentation posture:

```text
Do:
  - inspect protocol before writes
  - reject unsupported writer features
  - include cross-engine compatibility notes
  - test Spark/Databricks-created tables separately

Do not:
  - assume every Delta protocol feature is writable
  - assume column mapping / identity columns are safe write targets
  - ignore reader/writer protocol version failures
```

### 0.9.4 Nanosecond timestamps

`nanosecond-timestamps` is explicitly feature-gated and remains experimental. At the pinned rev the wrapper feature forwards `deltalake-core/nanosecond-timestamps`, which forwards `delta_kernel/nanosecond-timestamps`; when enabled, the write path gates on `TableFeature::TimestampNanos`. The current 0.25-kernel line additionally carries both nanosecond logical variants: timezone-aware `TimestampNanos` and timezone-less **`TimestampNanosNtz`**, which maps to Arrow `Timestamp(Nanosecond, None)` and uses the same `timestampNanos` Delta table feature. ([GitHub][4])

Documentation posture:

```text
Default:
  avoid enabling nanosecond-timestamps

Enable only when:
  - upstream data genuinely requires ns precision
  - downstream engines support the same representation
  - round-trip tests cover Arrow -> Delta -> DataFusion -> Parquet -> external engine
```

### 0.9.5 Variant type

The delta-rs CHANGELOG entry for `rust-v0.32.3` (the newest entry retained at the pinned rev) lists “add support for variant type” (PR #4325), and the Python 1.6.0 release notes mention minimum PyArrow 21.0.0 for preliminary variant support. At the pinned rev this is verifiable in source: `TableFeature::VariantType`/`VariantTypePreview` are in the supported reader and writer feature sets, the write path gates on `check_can_write_variant`, and `deltalake-core` enables parquet’s `variant_experimental` feature. See §4.23. ([GitHub][1])

Documentation posture:

```text
Treat variant as new/high-risk:
  - include explicit compatibility tests
  - avoid default use in schema examples
  - isolate behind a "new type support" subsection
  - test with Arrow 58 and external readers
```

---

## 0.10 DataFusion + Arrow alignment requirements

### Required alignment matrix

| Layer               |                        Target version | Reason                                         |
| ------------------- | -------------------------------------: | ---------------------------------------------- |
| `deltalake`         | `1.0.0` @ git rev `9f922319…` (pre-release pin) | public app crate                       |
| `deltalake-core`    |                     `1.0.0` (same rev) | implementation crate behind wrapper            |
| `delta_kernel`      | `buoyant_kernel` `0.25.x` (`>=0.25.0,<0.25.100`; `arrow-58` + `internal-api`) | Delta kernel implementation |
| kernel default engine | `buoyant_kernel_engine` `0.25.x` (`>=0.25.0,<0.25.100`; `arrow-58`) | filesystem/JSON/Parquet/default-engine implementation |
| Rust edition        |                                 `2024` | workspace edition                              |
| Rust toolchain/MSRV |                               `1.94.1` | workspace/toolchain pin                        |
| Arrow               |            `58` (resolves to `58.3.0`) | Delta kernel + DataFusion shared memory format |
| Parquet             |            `58` (resolves to `58.3.0`) | Delta data-file writer/reader compatibility    |
| DataFusion          |                               `54.0.0` | query/planning/expression engine               |
| `object_store`      |                               `0.13.2` | storage abstraction used by Delta/DataFusion   |
| Tokio               |                                    `1` | async runtime                                  |

The DataFusion integration docs emphasize that DataFusion uses Apache Arrow as its in-memory format, and delta-rs exposes DataFusion integration modules for Delta table querying; for this baseline, the pinned rev’s workspace explicitly aligns Arrow 58 and DataFusion 54.0.0 — and DataFusion 54 itself declares `arrow = "58.3.0"`, so exact Arrow pins in application workspaces must be `=58.3.0`, not `=58.0.0`. ([Docs.rs][6])

**sqlparser duality note:** the resolved dependency graph legitimately contains **two** `sqlparser` versions — `deltalake-core` declares `sqlparser = "0.61.0"` while DataFusion 54 declares `sqlparser = "0.62.0"`. Do not “fix” this in `cargo tree -d` output; it is the expected shape of this baseline. For DataFusion 53→54 API-level changes, see `docs/library_ref/datafusion_54vs53.md`.

### LLM code-generation rule

Never generate:

```toml
datafusion = "*"
arrow = "*"
deltalake = "1.0"   # floating crates.io range cannot express the pre-release pin
```

Prefer:

```toml
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", features = ["datafusion"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
```

For non-documentation applications, caret ranges may be acceptable after CI proves no duplicate Arrow/DataFusion versions.

---

## 0.11 Recommended documentation test harness

Every syntax block in later deep dives should compile in a test crate with the same dependency baseline.

### Workspace layout

```text
delta-rs-doc-tests/
  Cargo.toml
  rust-toolchain.toml
  crates/
    baseline_compile/
      Cargo.toml
      src/lib.rs
    local_table_smoke/
      Cargo.toml
      src/main.rs
    datafusion_smoke/
      Cargo.toml
      src/main.rs
    s3_minio_smoke/
      Cargo.toml
      src/main.rs
```

### `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy"]
```

### Root `Cargo.toml`

```toml
[workspace]
resolver = "3"
members = [
  "crates/baseline_compile",
  "crates/local_table_smoke",
  "crates/datafusion_smoke",
  "crates/s3_minio_smoke",
]

[workspace.package]
edition = "2024"
rust-version = "1.94.1"

[workspace.dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
tokio = "1"
url = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
```

### Baseline compile smoke

```rust
use deltalake::{DeltaTable, DeltaTableError};
use datafusion::prelude::SessionContext;
use arrow_schema::{DataType, Field, Schema};

#[allow(dead_code)]
fn assert_type_surface() {
    let _ctx = SessionContext::new();

    let _schema = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("value", DataType::Utf8, true),
    ]);

    fn _accept_delta_result(_: Result<DeltaTable, DeltaTableError>) {}
}
```

### CI

```bash
cargo +1.94.1 fmt --all -- --check
cargo +1.94.1 clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.94.1 test --workspace --all-features
cargo +1.94.1 tree -d
```

---

## 0.12 Documentation “version banner” template

Every subsequent deep dive should start with:

```text
Version target:
  deltalake: 1.0.0 (git rev 9f922319, pre-release)
  deltalake-core: 1.0.0 (same git rev)
  Rust edition: 2024
  Rust toolchain/MSRV: 1.94.1
  Arrow: 58
  Parquet: 58
  DataFusion: 54.0.0
  object_store: 0.13.2
  Tokio: 1
  Required deltalake features: [...]
  Optional deltalake features: [...]
  Examples compiled: yes/no
```

For your codebase, default banner:

```text
Version target:
  deltalake: 1.0.0 (git rev 9f922319, pre-release)
  Rust edition: 2024
  Rust toolchain/MSRV: 1.94.1
  Arrow: 58
  Parquet: 58
  DataFusion: 54.0.0
  object_store: 0.13.2
  Tokio: 1
  Required deltalake features: ["datafusion", "s3"]
  TLS: rustls
  Examples compiled: required before promotion to canonical docs
```

---

## 0.13 Agent-operable checklist before writing any deep-dive code

```text
1. Read target version banner.
2. Confirm deltalake feature flags.
3. Confirm DataFusion feature is enabled for any SQL/TableProvider/Expr examples.
4. Confirm cloud feature is enabled for any non-local URI.
5. Confirm Arrow/DataFusion/ObjectStore versions match the baseline.
6. Run cargo tree -d.
7. Compile minimal imports.
8. Compile one open_table example.
9. Compile one DataFusion SessionContext registration example.
10. Compile one Arrow RecordBatch construction example.
11. Only then write syntax-heavy examples.
```

---

## 0.14 Baseline value case

This section prevents the most expensive failure mode in Rust lakehouse work: generating plausible code against incompatible versions. For your DataFusion/Arrow-heavy simulation backbone, the baseline gives:

```text
- one Arrow type universe
- one DataFusion trait universe
- one object_store abstraction version
- one Delta protocol/API target
- predictable feature-gated API availability
- reproducible examples for LLM agents
- CI-detectable dependency drift
- safer cloud deployment matrix
- clearer upgrade boundaries
```

The immediate documentation standard should be:

```text
No unpinned syntax.
No uncompiled examples.
No DataFusion examples without feature=["datafusion"].
No cloud examples without explicit feature gate.
No direct deltalake-core examples unless documenting internals.
No Arrow/DataFusion version assumptions outside the version banner.
```

---

## 0.15 The 1.0.0 identity: semver milestone, git-pin status, and kernel identity

### What the 1.0.0 bump means for this document

**The public API surface documented here is stable across 0.32.x → 1.0.0.** A direct source diff between `deltalake-core` 0.32.4 (crates.io) and the pinned 1.0.0 rev shows every doc-referenced API surface **unchanged or additive**:

```text
Unchanged (verified by source diff):
  open_table / open_table_with_storage_options / open_table_with_version / open_table_with_ds
  DeltaTableBuilder (byte-identical), DeltaTable::try_from_url
  DeltaOps (still present, still deprecated-style)
  all operation builders: write / merge / delete / update / create /
    optimize / vacuum / restore / load / load_cdf (same module paths)
  Snapshot / EagerSnapshot
  DeltaScanConfig / DeltaScanConfigBuilder / TableProviderBuilder
  DeltaSessionContext / create_session / create_session_state_with_spill_config
  update_datafusion_session (idempotence contract unchanged)
  CdfLoadBuilder / scan_cdf
  SaveMode / SchemaMode, the writer module, TableFeatures
  WriteBuilder still awaits to Result<DeltaTable> — no metrics tuple

Additive at the pinned rev:
  BlindDeltaTable re-export (see §3.27)
  DeltaTable::drop_column_not_null / DropColumnNotNullBuilder (see §11.7)
  opendal storage-backend family (see §2.23)
  next-scan file selection: FileSelection / MissingSelectedFilePolicy /
    DeltaScanNext / DeltaScanExec re-exports (see §6.35)
  column-mapping writer-feature support in the protocol checker (see §4.25, §11.13)

Removed / deprecated at the pinned rev:
  DeltaTableProvider (legacy provider type) — removed (PR #4435)
  DeltaPhysicalCodec — #[deprecated]; use DeltaLogicalCodec (PR #4458)
```

### Git-pin status

`deltalake`/`deltalake-core` `1.0.0` is **not published to crates.io or docs.rs** at the time of this baseline; the only way to consume it is a git pin. Mirror this workspace block (the same shape this repository uses):

```toml
[workspace.dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = [
    "rustls",
    "datafusion",
    "s3",
    "gcs",
    "azure",
] }
deltalake-core = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false }

# Only add direct kernel dependencies if application code intentionally consumes
# kernel APIs. delta-rs itself already selects these released 0.25.x crates.
delta_kernel = { package = "buoyant_kernel", version = "0.25.0,<0.25.100", default-features = false, features = [
    "arrow-58",
    "internal-api",
] }
delta_kernel_default_engine = { package = "buoyant_kernel_engine", version = "0.25.0,<0.25.100", default-features = false, features = [
    "arrow-58",
    "rustls",
] }
```

There are no formal 1.0.0 release notes; the `CHANGELOG.md` at the rev is the change record (§0.1), and the eventual tagged release may add changes beyond this rev — re-verify on pin refresh.

### Kernel identity: released `buoyant_kernel` / `buoyant_kernel_engine` 0.25.x

The Delta kernel underneath `deltalake-core` is the `delta_kernel` crate **packaged as `buoyant_kernel` 0.25.x**, now consumed from a released crate constraint (`>=0.25.0,<0.25.100`) rather than a git SHA. Kernel 0.25 also split the default engine into the separate **`buoyant_kernel_engine` 0.25.x** package. `deltalake-core` depends on both: the kernel carries `arrow-58` + `internal-api`, while the default engine supplies the filesystem, JSON, Parquet, and Tokio execution implementations and the selected TLS feature. Practical consequences:

```text
- Cargo.lock shows `buoyant_kernel` and `buoyant_kernel_engine`; this is expected.
- A pure deltalake application normally should NOT add either crate directly;
  let the exact delta-rs git pin and Cargo.lock select the compatible 0.25.x pair.
- The kernel owns snapshot construction, log replay, checkpoint writing (§8.25),
  and per-operation table-feature validation (§11.27).
- The `arrow-58` kernel/default-engine features tie the stack to Arrow 58.x;
  do not select a different kernel Arrow feature.
- `internal-api` remains an implementation-facing surface without a cross-rev
  stability promise; isolate any direct use behind application-owned adapters.
```

### Cross-version behavior notes carried by this baseline

```text
- Checkpoints written via the kernel mark required `add` fields non-nullable;
  very old delta-rs readers (<= 0.25.5) may reject them (§8.25).
- Table-feature validation is strict: any declared-but-unsupported reader/writer
  feature fails the whole operation, even if unused by the touched data (§11.27).
- DeltaScan stats loading is selective/lazy at the pinned rev (§12.33).
```



## 0.16 Delta from the prior documentation pin (`35cfed45…`) to `9f922319…`

The current target is **20 commits ahead** of the previous reference baseline. The dependency family deliberately stays on DataFusion 54 / Arrow 58 / Parquet 58 / `object_store` 0.13.2, so this is a stabilization refresh rather than an ecosystem-major migration. The material Rust-side changes are:

| Area | Latest-pin change | Practical significance |
|---|---|---|
| Snapshot replay | explicit lazy/eager materialization modes; capability-aware active-add replay | lower activation memory and safer on-demand file/stat replay |
| Snapshot cache identity | cache keyed/validated against table root, version, checkpoint, protocol, metadata | stale or cross-snapshot materialized file state is rejected |
| Snapshot serde | pre-identity cache compatibility + identityless-cache rejection | safer restart/serialization boundaries |
| Delta kernel | `buoyant_kernel` upgraded to released 0.25.x and default engine split to `buoyant_kernel_engine` | removes the second unpublished kernel git pin and clarifies engine ownership |
| DataFusion scan | nested physical nullability adapter | fixes Spark-written nested-schema `OPTIMIZE`/scan failures |
| Partition mapping | nested fields no longer compared to top-level partition columns | fixes latest compaction/schema-mapping edge case |
| Protocol features | `V2Checkpoint` accepted as reader/writer feature | broader protocol compatibility |
| CDF | prefers `inCommitTimestamp` when present | more correct timestamp-range behavior |
| Nanosecond types | adds timezone-less `TimestampNanosNtz` under the feature gate | complete ns timestamp/NTZ pairing |
| Delta action paths | URI-encode spaces while preserving path/Hive delimiters | correct log-path serialization and Spark round trips |
| Toolchain | workspace/MSRV 1.91.1 -> **1.94.1** | repository and CI toolchain update required |
| Python packaging | explicit source distribution release workflow | no effect on pure-Rust API/design |

### Update recommendation embedded in this reference

Use the exact SHA, commit `Cargo.lock`, and compile the complete workspace on Rust 1.94.1. Do not float on `main`. Continue to exact-pin DataFusion/Arrow/Parquet/ObjectStore at the application workspace boundary to prevent type-universe skew.

### Known compatibility boundary that did **not** move

This revision still uses Arrow/Parquet 58 because DataFusion 54 is the compatibility anchor. Do not independently move Parquet to 59 in an application workspace merely to pick up newer Parquet internals; that would break the intentionally unified DataFusion/Arrow/Delta dependency universe. Track such upgrades as a future coordinated stack migration.

Primary upstream evidence:

- latest protected `main`: https://github.com/delta-io/delta-rs/commit/9f9223197469897ef05ae4369eb4fd1390174e65
- prior-to-current comparison: https://github.com/delta-io/delta-rs/compare/35cfed4545f41c2f483706d29670f7cc2fe7e217...9f9223197469897ef05ae4369eb4fd1390174e65
- root dependency baseline: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/Cargo.toml
- snapshot implementation: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/snapshot/mod.rs
- protocol feature checker: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/transaction/protocol.rs

[1]: https://github.com/delta-io/delta-rs/releases "Releases · delta-io/delta-rs · GitHub"
[2]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/Cargo.toml "delta-rs/Cargo.toml at rev 9f922319 · delta-io/delta-rs · GitHub"
[3]: https://docs.rs/crate/deltalake/latest/features "deltalake - Docs.rs (crates.io latest; the 1.0.0 pre-release rev is not yet published to docs.rs)"
[4]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/Cargo.toml "delta-rs/crates/core/Cargo.toml at rev 9f922319 · delta-io/delta-rs · GitHub"
[5]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/deltalake/Cargo.toml "delta-rs/crates/deltalake/Cargo.toml at rev 9f922319 · delta-io/delta-rs · GitHub"
[6]: https://docs.rs/deltalake/latest/deltalake/ "deltalake - Rust"
[7]: https://delta-io.github.io/delta-rs/integrations/delta-lake-datafusion/ "DataFusion - Delta Lake Documentation"
[8]: https://github.com/delta-io/delta-rs/tree/9f9223197469897ef05ae4369eb4fd1390174e65 "GitHub - delta-io/delta-rs at rev 9f922319 · GitHub"
[9]: https://docs.rs/crate/deltalake/latest "deltalake - Docs.rs (crates.io latest; the 1.0.0 pre-release rev is not yet published to docs.rs)"


# 2. Deployment and project setup — Rust `deltalake` in production services

Version target for this section:

```text id="57mg81"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Tokio: 1
Default TLS: rustls
Primary service profile: DataFusion + Arrow + S3-compatible object storage
```

The pinned 1.0.0 rev (`9f922319…`) pins Rust `1.94.1`, edition `2024`, Arrow `58` (resolved `58.3.0`), Parquet `58`, DataFusion `54.0.0`, `object_store 0.13.2`, and Tokio `1`; production documentation and generated code should align to that dependency universe to avoid Arrow/DataFusion type-identity skew. ([GitHub][1])

---

## 2.1 Deployment mental model

`deltalake` deployment has four separable layers:

```text id="9n0aob"
application service
  -> deltalake public API
    -> Delta LogStore: transaction-log consistency, commit atomicity, metadata correctness
    -> object_store ObjectStore: object IO, multipart IO, cloud/local storage API
      -> filesystem / S3 / Azure ADLS / GCS / HDFS / lakeFS / catalog-resolved locations
```

The `LogStore` is table-scoped and responsible for Delta metadata/commit correctness; it requires atomic visibility, mutual exclusion for log-file creation, and consistent listing. `ObjectStore` handles direct storage-system interaction during data IO and log-store internals. ([Docs.rs][2])

The `object_store` crate is the storage substrate underneath much of this stack: it exposes a uniform async `ObjectStore` trait for local files and object storage, supports URL + key/value configuration, and is explicitly designed so the same binary can run across cloud and local test environments by changing runtime configuration. ([Docs.rs][3])

---

## 2.2 Cargo installation profiles

### 2.2.1 Local-only / metadata / basic Arrow-write service

```toml id="66mev3"
[package]
edition = "2024"
rust-version = "1.94.1"

[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
```

Use for:

```text id="dltmvr"
local filesystem tables
metadata inspection
history inspection
schema inspection
table-state loading
basic non-cloud development fixtures
```

### 2.2.2 DataFusion + Arrow production baseline

```toml id="a3mxn5"
[package]
edition = "2024"
rust-version = "1.94.1"

[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Use this for local/object-store-independent service code that registers Delta tables into `SessionContext`, exposes SQL/DataFrame execution, writes from Arrow batches, or eventually writes from DataFusion plans. The `datafusion` feature gates Delta/DataFusion integration in the published crate. ([Docs.rs][4])

### 2.2.3 S3 + DataFusion production baseline

```toml id="ospw9j"
[package]
edition = "2024"
rust-version = "1.94.1"

[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

The `s3` feature enables `deltalake-aws/rustls` plus `rustls`; `s3-native-tls` enables `deltalake-aws/native-tls` plus `native-tls`. Do not enable both TLS stacks by accident; the upstream docs.rs metadata avoids `all_features` because TLS features are mutually exclusive. ([GitHub][5])

### 2.2.4 Cloud matrix profiles

```toml id="8tw4xl"
# AWS S3
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

# AWS S3 with native TLS
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["native-tls", "datafusion", "s3-native-tls"] }

# Azure ADLS / Blob / OneLake-style endpoints
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "azure"] }

# Google Cloud Storage
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "gcs"] }

# AWS Glue catalog + S3 data
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3", "glue"] }

# Experimental Unity Catalog
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "unity-experimental"] }
```

Published feature flags include `azure`, `datafusion`, `datafusion-ext`, `gcs`, `glue`, `hdfs`, `json`, `lakefs`, `nanosecond-timestamps`, `native-tls`, `python`, `s3`, `s3-native-tls`, and `unity-experimental`; default is `rustls`. ([Docs.rs][4])

---

## 2.3 Tokio runtime contract

All core table-loading and object-store operations are async. Production services should use a multithread Tokio runtime unless they are tiny CLI tools.

```rust id="21sad0"
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // initialize logging, config, DataFusion, Delta tables
    Ok(())
}
```

Recommended runtime posture:

```text id="p79ie5"
CLI smoke tests:        #[tokio::main(flavor = "current_thread")] acceptable
API service:            multi_thread required
batch ingestion worker: multi_thread required
DataFusion service:     multi_thread strongly preferred
object-store-heavy IO:  multi_thread + explicit object-store concurrency tuning
```

Avoid creating nested Tokio runtimes inside request handlers, UDFs, or DataFusion execution callbacks. Pass handles, sessions, object stores, and configuration downward.

---

## 2.4 URL construction and table loading

Current Rust helpers take `url::Url`, not a raw `&str`. `open_table` creates and loads a `DeltaTable` from a URL, infers the backend from the URL scheme, and fails fast for a missing local path. `open_table_with_storage_options` is the storage-options variant and accepts `HashMap<String, String>`. ([Docs.rs][6])

### 2.4.1 Existing local table

```rust id="p0ykyl"
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn open_local_table(path: &str) -> anyhow::Result<DeltaTable> {
    let table_url = Url::from_directory_path(path)
        .map_err(|_| anyhow::anyhow!("invalid local Delta table path: {path}"))?;

    let table = open_table(table_url).await?;
    Ok(table)
}
```

Deployment notes:

```text id="dq3ib2"
- Use local filesystem only for development, embedded mode, tests, or single-node workloads.
- Do not use ephemeral container filesystems for persistent Delta tables.
- Mount durable volumes explicitly.
- Treat local paths as existing table roots unless using create-table operations.
- Check for _delta_log before treating a directory as a Delta table.
```

### 2.4.2 Existing S3 table with explicit storage options

```rust id="qaw7ld"
use deltalake::{open_table_with_storage_options, DeltaTable};
use std::collections::HashMap;
use url::Url;

pub async fn open_s3_table() -> anyhow::Result<DeltaTable> {
    let mut options = HashMap::new();

    options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
    options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
    options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());

    let table_url = Url::parse("s3://my-bucket/path/to/table")?;
    let table = open_table_with_storage_options(table_url, options).await?;

    Ok(table)
}
```

Production rule: load credentials from environment/metadata/workload identity whenever possible; use `storage_options` for non-secret knobs, local test endpoints, role overrides, table-specific locking configuration, or explicit credential injection in controlled integration tests.

---

## 2.5 Storage-options map design

Use one internal configuration struct, then emit `HashMap<String, String>` only at the Delta boundary.

```rust id="f16byh"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeltaStorageOptions {
    pub cloud: CloudBackend,
    pub region: Option<String>,
    pub endpoint_url: Option<String>,
    pub allow_http: bool,
    pub timeout: Option<String>,
    pub connect_timeout: Option<String>,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum CloudBackend {
    Local,
    S3 {
        locking_provider: Option<String>,
        dynamo_table_name: Option<String>,
        virtual_hosted_style: Option<bool>,
    },
    Azure,
    Gcs,
}

impl DeltaStorageOptions {
    pub fn to_storage_options(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();

        if let Some(region) = &self.region {
            out.insert("AWS_REGION".to_owned(), region.clone());
        }

        if let Some(endpoint) = &self.endpoint_url {
            out.insert("AWS_ENDPOINT_URL".to_owned(), endpoint.clone());
        }

        if self.allow_http {
            out.insert("allow_http".to_owned(), "true".to_owned());
        }

        if let Some(timeout) = &self.timeout {
            out.insert("timeout".to_owned(), timeout.clone());
        }

        if let Some(connect_timeout) = &self.connect_timeout {
            out.insert("connect_timeout".to_owned(), connect_timeout.clone());
        }

        match &self.cloud {
            CloudBackend::S3 {
                locking_provider,
                dynamo_table_name,
                virtual_hosted_style,
            } => {
                if let Some(locking_provider) = locking_provider {
                    out.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), locking_provider.clone());
                }

                if let Some(table) = dynamo_table_name {
                    out.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), table.clone());
                }

                if let Some(vhost) = virtual_hosted_style {
                    out.insert(
                        "AWS_VIRTUAL_HOSTED_STYLE_REQUEST".to_owned(),
                        vhost.to_string(),
                    );
                }
            }
            CloudBackend::Local | CloudBackend::Azure | CloudBackend::Gcs => {}
        }

        out.extend(self.extra.clone());
        out
    }
}
```

Storage options should be treated as configuration, not business logic. Never scatter hard-coded key strings across loaders, writers, query planners, and test fixtures.

---

## 2.6 S3 deployment

### 2.6.1 S3 feature gate

```toml id="njic4z"
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }
```

Delta-rs has native S3 backend support, but credentials must be configured correctly; the docs explicitly warn Python users not to assume boto3 credential behavior. Supported credential paths include storage options, environment variables, EC2 metadata, AWS profiles, and web identity. ([Delta IO][7])

### 2.6.2 S3 URL schemes

Supported S3 URL schemes include:

```text id="ls066f"
s3://bucket-name/path/to/table
s3a://bucket-name/path/to/table
https://s3.<region>.amazonaws.com/bucket-name/path/to/table
https://bucket-name.s3.<region>.amazonaws.com/path/to/table
```

Delta-rs documents `s3://`, `s3a://`, HTTPS path-style, and HTTPS virtual-hosted-style forms. ([Delta IO][7])

### 2.6.3 S3 credential/storage options

```rust id="0ueui4"
pub fn s3_prod_options() -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    // Non-secret deployment knobs.
    options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());

    // Required for safe concurrent writes on AWS S3.
    options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
    options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());

    options
}
```

Common S3 configuration keys include `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` / `AWS_DEFAULT_REGION`, `AWS_ENDPOINT_URL`, `AWS_SESSION_TOKEN`, virtual-hosted-style control, web identity token file, role ARN, and role session name; Delta-specific keys include `AWS_S3_LOCKING_PROVIDER`, `DELTA_DYNAMO_TABLE_NAME`, and `AWS_S3_ALLOW_UNSAFE_RENAME`. ([Delta IO][7])

### 2.6.4 S3 safe concurrent writes

For AWS S3 writes, configure DynamoDB locking. Delta-rs docs state that S3 needs a locking provider because AWS S3 does not guarantee mutual exclusion, and DynamoDB is the currently available locking provider. ([Delta IO][8])

```bash id="64uw74"
aws dynamodb create-table \
  --table-name delta_log \
  --attribute-definitions AttributeName=tablePath,AttributeType=S AttributeName=fileName,AttributeType=S \
  --key-schema AttributeName=tablePath,KeyType=HASH AttributeName=fileName,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST

aws dynamodb update-time-to-live \
  --table-name delta_log \
  --time-to-live-specification Enabled=true,AttributeName=expireTime
```

```rust id="lm7h37"
let mut options = std::collections::HashMap::new();
options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());
```

The DynamoDB lock key includes `tablePath`; all writers to the same table must agree exactly on the table root URL, including scheme normalization such as `s3://` vs `s3a://`. ([Delta IO][8])

### 2.6.5 Unsafe S3 rename

```rust id="22mml6"
// Single-writer test fixtures only. Do not use in production multi-writer tables.
options.insert("AWS_S3_ALLOW_UNSAFE_RENAME".to_owned(), "true".to_owned());
```

`AWS_S3_ALLOW_UNSAFE_RENAME=true` allows commits without concurrent-writer protection and is only safe when exactly one writer writes to a given table. ([Docs.rs][9])

### 2.6.6 S3 IAM minimum posture

Delta-rs docs list S3 `GetObject`, `PutObject`, and `DeleteObject` as required; they also note delete permission is needed even for append because temporary log files are deleted after use. DynamoDB locking requires `GetItem`, `Query`, `PutItem`, `UpdateItem`, and `DeleteItem`. ([Delta IO][8])

Policy design:

```text id="tedslm"
S3:
  - scope to bucket + table prefix
  - allow read/write/delete under table root
  - allow list against table prefix where required by deployment
  - deny broad bucket-wide access unless service owns bucket

DynamoDB:
  - scope to lock table
  - allow lock-table item CRUD/query actions
  - enable TTL
  - isolate dev/stage/prod lock tables
```

---

## 2.7 MinIO, R2, and LocalStack testing

### 2.7.1 MinIO / R2-style conditional put

For some S3-compatible clients such as Cloudflare R2 or MinIO, docs state that concurrent writes can be enabled by setting `conditional_put` to `etag`. ([Delta IO][8])

```rust id="mg9qoa"
pub fn minio_storage_options() -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
    options.insert("AWS_ACCESS_KEY_ID".to_owned(), "minioadmin".to_owned());
    options.insert("AWS_SECRET_ACCESS_KEY".to_owned(), "minioadmin".to_owned());
    options.insert("AWS_ENDPOINT_URL".to_owned(), "http://localhost:9000".to_owned());
    options.insert("AWS_VIRTUAL_HOSTED_STYLE_REQUEST".to_owned(), "false".to_owned());

    // Required for HTTP local endpoints.
    options.insert("allow_http".to_owned(), "true".to_owned());

    // S3-compatible concurrent-write path for clients that support conditional put.
    options.insert("conditional_put".to_owned(), "etag".to_owned());

    options
}
```

### 2.7.2 LocalStack with DynamoDB locking

```rust id="wo3sl3"
pub fn localstack_s3_dynamo_options() -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
    options.insert("AWS_ACCESS_KEY_ID".to_owned(), "test".to_owned());
    options.insert("AWS_SECRET_ACCESS_KEY".to_owned(), "test".to_owned());

    options.insert("AWS_ENDPOINT_URL".to_owned(), "http://localhost:4566".to_owned());
    options.insert("AWS_ENDPOINT_URL_DYNAMODB".to_owned(), "http://localhost:4566".to_owned());

    options.insert("allow_http".to_owned(), "true".to_owned());
    options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
    options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());

    options
}
```

The locking docs expose DynamoDB override keys such as `AWS_ENDPOINT_URL_DYNAMODB`, `AWS_REGION_DYNAMODB`, `AWS_ACCESS_KEY_ID_DYNAMODB`, and `AWS_SECRET_ACCESS_KEY_DYNAMODB` for custom endpoints or separate DynamoDB credentials. ([Delta IO][8])

---

## 2.8 Azure ADLS deployment

### 2.8.1 Azure feature gate

```toml id="3k4855"
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "azure"] }
```

Delta-rs documents native support for Microsoft Azure Data Lake Storage as an object-storage backend; explicit storage options are forwarded to the object-store library. ([Delta IO][10])

### 2.8.2 Azure URL schemes

```text id="xkpsfz"
abfss://container@account.dfs.core.windows.net/path/to/table
abfs://container@account.dfs.core.windows.net/path/to/table
az://container/path/to/table
adl://container/path/to/table
```

Azure ADLS docs list `abfss`, `abfs`, `az`, and `adl` schemes. ([Delta IO][10])

### 2.8.3 Azure options

```rust id="oabq5o"
pub fn azure_access_key_options(
    account_name: &str,
    access_key: &str,
) -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert("account_name".to_owned(), account_name.to_owned());
    options.insert("access_key".to_owned(), access_key.to_owned());

    options
}
```

```rust id="d6uac4"
pub fn azure_cli_options(tenant_id: &str) -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert("azure_tenant_id".to_owned(), tenant_id.to_owned());
    options.insert("azure_use_azure_cli".to_owned(), "true".to_owned());

    options
}
```

Azure configuration keys include account name, access key, service-principal client ID/secret, tenant ID, SAS key, bearer token, emulator mode, endpoint, Azure CLI usage, federated token file, container name, managed-identity endpoint/object/resource IDs, anonymous skip-signature, Fabric endpoint, and blob-tagging disablement. ([Delta IO][10])

Deployment guidance:

```text id="e9z8hz"
preferred production auth:
  workload identity / managed identity / federated token

acceptable controlled auth:
  service principal secret stored in secret manager

avoid:
  account access keys in app config
  SAS tokens in checked-in test files
  broad account-level storage permissions
```

---

## 2.9 GCS deployment

### 2.9.1 GCS feature gate

```toml id="bs6qk1"
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "gcs"] }
```

With the `gcs` feature enabled through the `deltalake` meta-crate, GCS support is automatically registered; manual `deltalake_gcp::register_handlers` is described as the legacy/deprecated path for application users of the meta-crate. ([Delta IO][11])

### 2.9.2 GCS URL scheme

```text id="0jipjo"
gs://bucket-name/path/to/table
```

GCS docs list `gs://bucket-name/path/to/table` as the supported URL scheme. ([Delta IO][11])

### 2.9.3 GCS options

```rust id="cr02ui"
pub fn gcs_adc_options(credentials_file: &str) -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert(
        "GOOGLE_APPLICATION_CREDENTIALS".to_owned(),
        credentials_file.to_owned(),
    );

    options
}
```

```rust id="w8uxfv"
pub fn gcs_service_account_options(service_account_json_path: &str) -> std::collections::HashMap<String, String> {
    let mut options = std::collections::HashMap::new();

    options.insert(
        "GOOGLE_SERVICE_ACCOUNT".to_owned(),
        service_account_json_path.to_owned(),
    );

    options
}
```

GCS configuration supports service-account file path, serialized service-account key, application-credentials file, bucket name, and custom endpoint; authentication precedence includes service-account key/file, ADC via `GOOGLE_APPLICATION_CREDENTIALS`, gcloud application-default credentials, and workload identity. ([Delta IO][11])

Required GCS permissions listed by the docs include `storage.objects.create`, `storage.objects.delete` for overwrites, `storage.objects.get`, and `storage.objects.list` depending on CLI/use case. ([Delta IO][11])

---

## 2.10 TLS selection

### 2.10.1 Default: `rustls`

```toml id="zqf4lb"
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }
```

Use `rustls` for:

```text id="cxy98b"
containerized Linux services
minimal runtime system dependencies
portable deployments
standard public CA trust
S3/GCS/Azure services without enterprise TLS interception
```

### 2.10.2 Native TLS

```toml id="q53wxh"
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["native-tls", "datafusion", "s3-native-tls"] }
```

Use `native-tls` for:

```text id="0z99rn"
enterprise OS certificate stores
corporate PKI requirements
TLS interception/proxy environments
platform-native certificate policy
```

Deployment implications:

```text id="5k7dwr"
rustls container:
  runtime usually needs ca-certificates

native-tls container:
  build image often needs pkg-config + OpenSSL development headers
  runtime image may need OpenSSL runtime libraries + ca-certificates
```

The crate feature mapping distinguishes `s3` + `rustls` from `s3-native-tls` + `native-tls`, and upstream docs.rs metadata avoids all-features builds because TLS features are mutually exclusive. ([GitHub][5])

---

## 2.11 DataFusion runtime configuration

### 2.11.1 Register Delta table provider

`DeltaTable::table_provider()` returns a `TableProviderBuilder`; the builder can be awaited into an `Arc<dyn TableProvider>`, while `.build()` returns a `DeltaScan`. The builder supports options such as log store, snapshot/eager snapshot, DataFusion session, table version, and file-column inclusion. ([Docs.rs][12])

```rust id="6br2ao"
use datafusion::prelude::SessionContext;
use deltalake::open_table_with_storage_options;
use std::collections::HashMap;
use url::Url;

pub async fn register_delta_table(
    ctx: &SessionContext,
    name: &str,
    url: &str,
    storage_options: HashMap<String, String>,
) -> anyhow::Result<()> {
    let table_url = Url::parse(url)?;
    let table = open_table_with_storage_options(table_url, storage_options).await?;

    // Idempotently prepare DataFusion RuntimeEnv object-store mapping.
    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    // Await TableProviderBuilder into Arc<dyn TableProvider>.
    let provider = table.table_provider().await?;

    ctx.register_table(name, provider)?;

    Ok(())
}
```

`update_datafusion_session` prepares the DataFusion session by registering the table root object store in the runtime environment when missing; it is idempotent and does not overwrite an existing mapping, so stale or incorrect mappings require direct `RuntimeEnv::register_object_store` replacement. ([Docs.rs][13])

### 2.11.2 Query smoke

```rust id="5hsq8m"
pub async fn query_registered_delta_table(ctx: &SessionContext) -> anyhow::Result<()> {
    let batches = ctx
        .sql("SELECT COUNT(*) AS n FROM simulation_runs")
        .await?
        .collect()
        .await?;

    for batch in batches {
        tracing::info!(rows = batch.num_rows(), "delta query batch");
    }

    Ok(())
}
```

### 2.11.3 Runtime anti-patterns

```text id="yekdcq"
avoid:
  - registering stale object stores and assuming update_datafusion_session overwrites them
  - opening tables per row/request
  - creating one SessionContext per trivial query in a service
  - mixing DataFusion versions through transitive dependencies
  - using raw Parquet registration for Delta table paths
  - bypassing Delta metadata and reading all Parquet files manually
```

---

## 2.12 Docker deployment

### 2.12.1 Rustls-based image

```dockerfile id="3iy1br"
ARG RUST_VERSION=1.94.1

FROM rust:${RUST_VERSION}-bookworm AS builder
WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates pkg-config \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --uid 10001 appuser
USER appuser

COPY --from=builder /app/target/release/engineering-workbench /usr/local/bin/engineering-workbench

ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/engineering-workbench"]
```

### 2.12.2 Native-TLS image delta

```dockerfile id="qz84ek"
# builder stage additions for native-tls/OpenSSL builds
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# runtime stage additions when dynamically linked against OpenSSL
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 \
  && rm -rf /var/lib/apt/lists/*
```

Container configuration rules:

```text id="2q0q9q"
- inject credentials through workload identity, metadata service, mounted secret, or env vars
- never bake cloud credentials into images
- include ca-certificates
- use --locked builds
- use a non-root runtime user
- mount local Delta table paths only for dev/single-node use
- keep object-store endpoint/region/lock config environment-specific
```

---

## 2.13 Environment-variable configuration

### 2.13.1 Generic loader

```rust id="7lem8i"
use std::collections::HashMap;

pub fn collect_storage_options(keys: &[&str]) -> HashMap<String, String> {
    keys.iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| ((*key).to_owned(), value))
        })
        .collect()
}
```

### 2.13.2 S3 env keys

```rust id="gg6kzs"
pub fn s3_env_storage_options() -> HashMap<String, String> {
    collect_storage_options(&[
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_ENDPOINT_URL",
        "AWS_VIRTUAL_HOSTED_STYLE_REQUEST",
        "AWS_S3_LOCKING_PROVIDER",
        "DELTA_DYNAMO_TABLE_NAME",
        "AWS_ENDPOINT_URL_DYNAMODB",
        "AWS_REGION_DYNAMODB",
        "AWS_ACCESS_KEY_ID_DYNAMODB",
        "AWS_SECRET_ACCESS_KEY_DYNAMODB",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
        "AWS_ROLE_ARN",
        "AWS_ROLE_SESSION_NAME",
    ])
}
```

### 2.13.3 Azure env keys

```rust id="19znmn"
pub fn azure_env_storage_options() -> HashMap<String, String> {
    collect_storage_options(&[
        "AZURE_STORAGE_ACCOUNT_NAME",
        "AZURE_STORAGE_ACCOUNT_KEY",
        "AZURE_STORAGE_CLIENT_ID",
        "AZURE_STORAGE_CLIENT_SECRET",
        "AZURE_STORAGE_TENANT_ID",
        "AZURE_STORAGE_SAS_KEY",
        "AZURE_STORAGE_TOKEN",
        "AZURE_STORAGE_USE_EMULATOR",
        "AZURE_STORAGE_ENDPOINT",
        "AZURE_STORAGE_USE_AZURE_CLI",
        "AZURE_FEDERATED_TOKEN_FILE",
        "AZURE_STORAGE_CONTAINER_NAME",
        "IDENTITY_ENDPOINT",
        "AZURE_MSI_ENDPOINT",
        "AZURE_STORAGE_OBJECT_ID",
        "AZURE_STORAGE_MSI_RESOURCE_ID",
    ])
}
```

### 2.13.4 GCS env keys

```rust id="3je2k0"
pub fn gcs_env_storage_options() -> HashMap<String, String> {
    collect_storage_options(&[
        "GOOGLE_SERVICE_ACCOUNT",
        "GOOGLE_SERVICE_ACCOUNT_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_BUCKET",
        "GOOGLE_ENDPOINT",
    ])
}
```

### 2.13.5 Common client options

Advanced storage options include object-store concurrency limiting, retry configuration, retry timeout, backoff parameters, mounted-storage unsafe rename, and cross-backend HTTP client options such as `allow_http`, `allow_invalid_certificates`, `connect_timeout`, `timeout`, proxy settings, idle pool options, HTTP/1 or HTTP/2 constraints, custom user agent, default content type, and certificate path. ([Delta IO][14])

```rust id="cd5fzv"
pub fn common_client_options() -> HashMap<String, String> {
    let mut options = HashMap::new();

    options.insert("timeout".to_owned(), "120s".to_owned());
    options.insert("connect_timeout".to_owned(), "30s".to_owned());
    options.insert("pool_max_idle_per_host".to_owned(), "10".to_owned());
    options.insert("user_agent".to_owned(), "engineering-workbench/1.0".to_owned());

    options
}
```

Do not use `allow_invalid_certificates=true` outside test environments; the docs explicitly warn that disabling TLS certificate validation is dangerous. ([Delta IO][14])

---

## 2.14 CI fixtures

### 2.14.1 Local Delta fixture

```rust id="2cy5wh"
use tempfile::TempDir;
use url::Url;

pub struct LocalDeltaFixture {
    pub root: TempDir,
    pub table_url: Url,
}

impl LocalDeltaFixture {
    pub fn new() -> anyhow::Result<Self> {
        let root = tempfile::tempdir()?;
        let table_url = Url::from_directory_path(root.path())
            .map_err(|_| anyhow::anyhow!("invalid tempdir URL"))?;

        Ok(Self { root, table_url })
    }
}
```

Use for:

```text id="hxy1we"
unit tests
schema tests
operation-builder compile tests
local create/write/read smoke tests
no-cloud CI lanes
```

### 2.14.2 MinIO service fixture

```yaml id="uwx6gw"
services:
  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    ports:
      - "9000:9000"
      - "9001:9001"
```

```bash id="zymabb"
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
export AWS_REGION=us-east-1
export AWS_ENDPOINT_URL=http://localhost:9000
export AWS_VIRTUAL_HOSTED_STYLE_REQUEST=false
```

Test categories:

```text id="yahnyx"
smoke:
  open table
  create table
  append batch
  read via DataFusion
  delete temp files
  update_datafusion_session idempotence

concurrency:
  parallel append
  conflict behavior
  conditional_put=etag behavior where supported

negative:
  missing credentials
  bad endpoint
  missing bucket
  lock table absent
  unsafe rename disabled on AWS-style S3 without lock
```

### 2.14.3 LocalStack fixture

```yaml id="vgf5cj"
services:
  localstack:
    image: localstack/localstack:latest
    environment:
      SERVICES: s3,dynamodb,sts
      AWS_DEFAULT_REGION: us-east-1
    ports:
      - "4566:4566"
```

```bash id="ogmm2o"
aws --endpoint-url=http://localhost:4566 s3 mb s3://delta-test

aws --endpoint-url=http://localhost:4566 dynamodb create-table \
  --table-name delta_log \
  --attribute-definitions AttributeName=tablePath,AttributeType=S AttributeName=fileName,AttributeType=S \
  --key-schema AttributeName=tablePath,KeyType=HASH AttributeName=fileName,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST
```

---

## 2.15 Production dependency pinning and drift detection

### 2.15.1 Workspace dependency pin

```toml id="n0sspd"
[workspace]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.94.1"

[workspace.dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }
datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"
tokio = "1"
url = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 2.15.2 Drift gate

```bash id="i7ph9j"
set -euo pipefail

cargo +1.94.1 check --workspace --all-targets --locked
cargo +1.94.1 test --workspace --all-features --locked
cargo +1.94.1 clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.94.1 tree -d | tee /tmp/cargo-tree-duplicates.txt

if grep -E 'arrow|arrow-array|arrow-schema|parquet|datafusion|object_store|deltalake' /tmp/cargo-tree-duplicates.txt; then
  echo "Duplicate lakehouse dependency detected"
  exit 1
fi
```

### 2.15.3 Toolchain pin

```toml id="xqlcfd"
# rust-toolchain.toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy"]
```

The upstream `rust-toolchain.toml` pins channel `1.94.1` with `rustfmt` and `clippy`. ([GitHub][15])

---

## 2.16 Deployment configuration object

Centralize Delta table deployment configuration.

```rust id="m72ted"
#[derive(Debug, Clone)]
pub struct DeltaTableDeployment {
    pub logical_name: String,
    pub table_uri: String,
    pub storage_options: std::collections::HashMap<String, String>,
    pub register_in_datafusion: bool,
    pub datafusion_table_name: Option<String>,
}

impl DeltaTableDeployment {
    pub async fn open(&self) -> anyhow::Result<deltalake::DeltaTable> {
        let url = url::Url::parse(&self.table_uri)?;
        let table = deltalake::open_table_with_storage_options(
            url,
            self.storage_options.clone(),
        )
        .await?;
        Ok(table)
    }

    pub async fn register(
        &self,
        ctx: &datafusion::prelude::SessionContext,
    ) -> anyhow::Result<()> {
        let table = self.open().await?;
        let state = ctx.state();

        table.update_datafusion_session(&state)?;

        let provider = table.table_provider().await?;
        let name = self
            .datafusion_table_name
            .as_deref()
            .unwrap_or(&self.logical_name);

        ctx.register_table(name, provider)?;
        Ok(())
    }
}
```

Recommended configuration source hierarchy:

```text id="8xeu61"
1. code defaults: timeouts, user-agent, non-secret local defaults
2. environment: region, endpoint, identity hints, service mode
3. secret manager: static access keys only when workload identity unavailable
4. deployment manifest: table URI, logical table name, catalog/table mapping
5. runtime override: tests only
```

---

## 2.17 IAM and secret-handling rules

```text id="3vdn5j"
S3:
  - use IAM role, web identity, or instance/task metadata before static keys
  - require DynamoDB locking for AWS S3 multi-writer tables
  - scope S3 access to table prefix
  - include delete permission for Delta temp/log cleanup
  - isolate lock tables per environment or per trust boundary

Azure:
  - prefer managed identity, workload identity, federated token, or service principal
  - prefer container/path-scoped permissions
  - avoid account-wide access keys where possible

GCS:
  - prefer workload identity or ADC in managed runtimes
  - use service-account JSON only for controlled local/dev fixtures
  - scope permissions to bucket/prefix where supported by org policy

All:
  - never serialize storage_options with secrets into logs
  - redact keys containing KEY, SECRET, TOKEN, SAS, CREDENTIAL, PASSWORD
  - version table URI separately from secrets
  - test missing/expired credential failure modes
```

---

## 2.18 Logging and observability

```rust id="q2tbyd"
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,deltalake=info,object_store=warn,datafusion=warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .init();
}
```

Recommended emitted fields:

```text id="f6s60x"
delta_table_logical_name
delta_table_uri_hash
delta_backend_kind
delta_table_version
delta_operation
delta_operation_metrics
datafusion_session_id
storage_endpoint_kind
lock_provider
error_class
```

Do not log:

```text id="9b5q2j"
AWS_SECRET_ACCESS_KEY
AWS_SESSION_TOKEN
AZURE_STORAGE_ACCOUNT_KEY
AZURE_STORAGE_CLIENT_SECRET
AZURE_STORAGE_SAS_KEY
AZURE_STORAGE_TOKEN
GOOGLE_SERVICE_ACCOUNT_KEY
serialized service-account JSON
bearer tokens
```

---

## 2.19 Production best practices

```text id="br8it4"
dependency:
  - exact-pin deltalake/DataFusion/Arrow/Parquet/object_store in service workspaces
  - run cargo tree -d in CI
  - avoid wildcard features

runtime:
  - one Tokio runtime per process
  - reuse SessionContext or controlled session pools
  - refresh table state intentionally
  - do not open tables repeatedly inside tight loops

storage:
  - use object-store features only for backends actually deployed
  - use rustls unless native TLS is required
  - use explicit endpoint/region in CI
  - use allow_http only for local object stores
  - never use allow_invalid_certificates in production

S3:
  - configure DynamoDB locking for AWS S3 multi-writer writes
  - never use AWS_S3_ALLOW_UNSAFE_RENAME in production multi-writer tables
  - normalize table URI scheme across all writers
  - create lock table before deployment
  - enable DynamoDB TTL for lock cleanup

DataFusion:
  - enable deltalake/datafusion
  - register Delta providers, not raw Parquet folders
  - call update_datafusion_session when binding Delta tables into custom sessions
  - treat existing object-store mappings as authoritative unless deliberately overridden

testing:
  - local filesystem tests for fast unit coverage
  - MinIO/LocalStack tests for object-store behavior
  - negative tests for credentials, locks, endpoints, stale sessions
  - concurrency tests for writer conflicts
```

---

## 2.20 Anti-patterns

```text id="kn4g18"
- deltalake = "*"
- datafusion = "*"
- enabling all cloud features in production binaries
- mixing rustls and native-tls accidentally
- passing raw &str to open_table* helpers
- relying on boto3-style implicit AWS behavior
- storing credentials in storage_options logs
- using S3 unsafe rename for multi-writer production
- using different URI spellings for the same S3 Delta table across writers
- reading Delta table Parquet files directly through DataFusion instead of registering Delta
- assuming update_datafusion_session overwrites stale object-store registrations
- using local container filesystem as persistent table storage
- using allow_http or allow_invalid_certificates outside test endpoints
```

---

## 2.21 LLM-agent deployment checklist

```text id="f6a8b1"
Before generating Rust code:
  1. Confirm deltalake version.
  2. Confirm Rust toolchain.
  3. Confirm DataFusion/Arrow/object_store alignment.
  4. Select cloud feature flags.
  5. Select TLS stack.
  6. Confirm URL scheme.
  7. Convert table path to url::Url.
  8. Build HashMap<String, String> storage_options.
  9. Omit secrets from logs.
  10. Configure S3 locking for AWS S3 writes.
  11. Register object store into DataFusion session.
  12. Register Delta TableProvider, not raw Parquet path.
  13. Run cargo check --locked.
  14. Run cargo tree -d.
  15. Run local + object-store integration smoke tests.
```

---

## 2.22 Value case

Proper deployment setup turns `deltalake` into a production-grade persistence layer for your Rust DataFusion/Arrow simulation backbone:

```text id="7qdkkb"
- Arrow-native data movement
- DataFusion-native query registration
- Delta transaction log correctness
- object-store portability
- cloud/local test parity
- explicit schema/table/version boundary
- S3 concurrency protection
- CI-detectable dependency drift
- table-level operational isolation
- safer LLM-generated code templates
```

The critical deployment invariant:

```text id="43vtuy"
One Delta table URI + one storage backend config + one dependency universe + one DataFusion runtime mapping.
```

That invariant prevents most production failures: duplicate Arrow types, stale object stores, unsafe S3 writes, broken cloud credentials, raw-Parquet bypasses, and version-dependent API drift.

---

## 2.23 OpenDAL storage backends (new at the 1.0.0 pinned rev)

The 1.0.0 line adds a generic [OpenDAL](https://opendal.apache.org/)-based storage backend crate, `deltalake-opendal`, re-exported by the wrapper as `deltalake::opendal` and auto-registered when the `opendal` feature (or any `opendal-<service>` feature) is enabled:

```rust
// crates/deltalake/src/lib.rs at the pinned rev
#[cfg(feature = "opendal")]
pub use deltalake_opendal as opendal;

// auto-registration on process start (ctor), same pattern as s3/azure/gcs:
#[cfg(feature = "opendal")]
mod __deltalake_auto_register_opendal {
    #[ctor::ctor]
    fn register() {
        crate::opendal::register_handlers(None);
    }
}
```

Feature gating is per service: the umbrella `opendal` feature enables the crate, and each `opendal-<service>` feature (e.g. `opendal-fs`, `opendal-s3`, `opendal-gcs`, `opendal-azblob`, `opendal-azdls`, `opendal-oss`, `opendal-obs`, `opendal-cos`, `opendal-tos`, `opendal-b2`, `opendal-swift`, `opendal-webhdfs`, `opendal-webdav`, `opendal-ftp`, `opendal-sftp`, `opendal-hf`, `opendal-memory`) auto-registers its scheme. Cloud-facing service features also pull `deltalake-opendal/rustls`.

Scheme-registration semantics (from the crate source at the pinned rev):

```text
- Every enabled service registers under the unambiguous scheme
  opendal+<service>://   (e.g. opendal+s3://bucket/table)
- When the service's natural scheme does NOT collide with a native delta
  backend, it is also registered bare: hf://, fs://, oss://, ... work directly.
- Services whose natural scheme is owned by a native backend (s3, memory, ...)
  are reachable ONLY via the opendal+ form — enabling them never clobbers the
  native s3/azure/gcs backends.
- register_opendal_handlers(scheme, adapter) registers a custom OpendalAdapter
  for a scheme, overwriting any existing factory for that scheme.
```

Public surface: `deltalake::opendal::{register_handlers, register_opendal_handlers, GenericAdapter, OpendalAdapter, OperatorSpec, OpendalObjectStoreFactory, OpendalLogStoreFactory, ConditionalPutShim, OPENDAL_PREFIX}`.

Production guidance:

```text
Use native backends (s3/azure/gcs features) for AWS/Azure/GCP production paths;
use opendal-* for long-tail stores (HuggingFace hf://, OSS, OBS, COS, TOS, B2,
Swift, WebHDFS, WebDAV, SFTP) or for test backends. Do not enable opendal-s3
expecting it to replace the native S3 backend — it registers opendal+s3:// only.
```

[1]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/Cargo.toml "delta-rs/Cargo.toml at rev 9f922319 · delta-io/delta-rs · GitHub"
[2]: https://docs.rs/deltalake/latest/deltalake/logstore/index.html "deltalake::logstore - Rust"
[3]: https://docs.rs/object_store/0.13.2/object_store/ "object_store - Rust"
[4]: https://docs.rs/crate/deltalake/latest/features "deltalake - Docs.rs (crates.io latest; the 1.0.0 pre-release rev is not yet published to docs.rs)"
[5]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/deltalake/Cargo.toml "delta-rs/crates/deltalake/Cargo.toml at rev 9f922319 · delta-io/delta-rs · GitHub"
[6]: https://docs.rs/deltalake/latest/deltalake/fn.open_table.html "open_table in deltalake - Rust"
[7]: https://delta-io.github.io/delta-rs/integrations/object-storage/s3/ "AWS S3 Storage Backend - Delta Lake Documentation"
[8]: https://delta-io.github.io/delta-rs/usage/writing/writing-to-s3-with-locking-provider/ "Writing to S3 with a locking provider - Delta Lake Documentation"
[9]: https://docs.rs/deltalake-aws/latest/deltalake_aws/constants/constant.AWS_S3_ALLOW_UNSAFE_RENAME.html "AWS_S3_ALLOW_UNSAFE_RENAME in deltalake_aws::constants - Rust"
[10]: https://delta-io.github.io/delta-rs/integrations/object-storage/adls/ "Azure ADLS Storage Backend - Delta Lake Documentation"
[11]: https://delta-io.github.io/delta-rs/integrations/object-storage/gcs/ "GCS Storage Backend - Delta Lake Documentation"
[12]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.TableProviderBuilder.html "TableProviderBuilder in deltalake::delta_datafusion - Rust"
[13]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[14]: https://delta-io.github.io/delta-rs/integrations/object-storage/special_configuration/ "Advanced object storage configuration - Delta Lake Documentation"
[15]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/rust-toolchain.toml "delta-rs/rust-toolchain.toml at rev 9f922319 · delta-io/delta-rs · GitHub"


# 3. Table loading, snapshots, state, and time travel — Rust `deltalake`

Version target:

```text id="g8t7vn"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Tokio: 1
Primary APIs:
  deltalake::open_table
  deltalake::open_table_with_storage_options
  deltalake::DeltaTableBuilder
  deltalake::DeltaTable
  deltalake::table::state::DeltaTableState
```

Important correction for this version: **`table.update().await?` is not the table-state refresh API in `deltalake` 1.0.0 @ the pinned rev.** `DeltaTable::update()` is the DML row-update operation builder; use `update_state()` or `update_incremental(...)` for refreshing loaded table state. The current docs list `update()` as “Update data from Delta table,” while `update_state()` refreshes to the most recent committed transaction-log state and `update_incremental(max_version)` forward-applies newer versions. ([Docs.rs][1])

---

## 3.1 Core mental model

`DeltaTable` is an in-memory logical handle to a Delta dataset. It is not automatically equivalent to “latest table forever.” It contains an optional loaded `DeltaTableState`; that state represents the table as of the most recently loaded Delta log entry. The docs define `DeltaTable` as a logical concept representing a dataset that evolves over time, where concrete information requires loading a snapshot, usually latest but optionally by version or point in time. ([Docs.rs][1])

Operational distinction:

```text id="iur1co"
DeltaTable:
  table URL
  load config
  log store
  object store
  optional loaded state

DeltaTableState:
  concrete snapshot
  version
  protocol
  metadata
  schema
  table properties
  active add actions
  tombstones
  log-derived accessors

DataFusion TableProvider:
  query-time view over a specific DeltaTable/TableProviderBuilder state path
  must be refreshed/rebuilt deliberately when snapshot freshness matters
```

State-load invariant:

```text id="7rayv1"
No loaded state => version() returns None and snapshot() returns NotInitialized.
Loaded latest => state describes latest visible committed version at load/refresh time.
Loaded version N => state describes version N until explicitly refreshed/reloaded.
Loaded datetime T => state describes latest version committed at or before T.
```

---

## 3.2 Minimal dependency baseline

```toml id="rww7g0"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
tracing = "0.1"
futures = "0.3"
```

`open_table` and `open_table_with_storage_options` take `url::Url`, not raw `&str`; `open_table` infers storage backend from the URL scheme and fails fast for a missing local path, while `open_table_with_storage_options` adds a `HashMap<String, String>` storage-options argument. ([Docs.rs][2])

---

## 3.3 Opening latest table state

### Local filesystem

```rust id="9zf3yp"
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn open_local_latest(path: &str) -> anyhow::Result<DeltaTable> {
    let table_url = Url::from_directory_path(path)
        .map_err(|_| anyhow::anyhow!("invalid local Delta table path: {path}"))?;

    let table = open_table(table_url).await?;

    Ok(table)
}
```

### S3/object-store with storage options

```rust id="qsz6vn"
use deltalake::{open_table_with_storage_options, DeltaTable};
use std::collections::HashMap;
use url::Url;

pub async fn open_s3_latest() -> anyhow::Result<DeltaTable> {
    let mut options = HashMap::new();
    options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
    options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
    options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());

    let table_url = Url::parse("s3://bucket/table")?;
    let table = open_table_with_storage_options(table_url, options).await?;

    Ok(table)
}
```

### Generic backend loader

```rust id="swk2ne"
use deltalake::{open_table, open_table_with_storage_options, DeltaTable};
use std::collections::HashMap;
use url::Url;

pub async fn open_delta_table(
    uri: &str,
    storage_options: HashMap<String, String>,
) -> anyhow::Result<DeltaTable> {
    let url = Url::parse(uri)?;

    let table = if storage_options.is_empty() {
        open_table(url).await?
    } else {
        open_table_with_storage_options(url, storage_options).await?
    };

    Ok(table)
}
```

Deployment rule:

```text id="w9mors"
Use open_table(...) only when:
  - local filesystem path is sufficient, or
  - backend credentials/config are entirely environmental.

Use open_table_with_storage_options(...) when:
  - explicit region/endpoint/lock/tenant/test config is required,
  - MinIO/LocalStack/R2/custom endpoints are used,
  - table-specific credential routing exists,
  - deterministic CI fixtures are required.
```

---

## 3.4 Builder-based loading

`DeltaTableBuilder` is the precise API for configured table loading: `from_url`, `with_storage_options`, `with_version`, `with_datestring`, `with_timestamp`, `without_files`, `with_skip_stats`, `with_log_buffer_size`, `with_allow_http`, `build`, and `load`. `build()` creates an uninitialized table; `load()` builds and loads state. ([Docs.rs][3])

### Latest state through builder

```rust id="2w0wcx"
use deltalake::{DeltaTable, DeltaTableBuilder};
use url::Url;

pub async fn builder_load_latest(uri: &str) -> anyhow::Result<DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .load()
        .await?;

    Ok(table)
}
```

### Builder with storage options

```rust id="ru4h88"
use deltalake::{DeltaTable, DeltaTableBuilder};
use std::collections::HashMap;
use url::Url;

pub async fn builder_load_with_options(
    uri: &str,
    storage_options: HashMap<String, String>,
) -> anyhow::Result<DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_storage_options(storage_options)
        .load()
        .await?;

    Ok(table)
}
```

### Builder without file materialization

```rust id="3v478p"
use deltalake::DeltaTableBuilder;
use url::Url;

pub async fn load_metadata_or_protocol_fast(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .without_files()
        .load()
        .await?;

    Ok(table)
}
```

`without_files()` sets `require_files=false` and is now best understood as a **metadata-first / lazy snapshot posture**, not an absolute prohibition on later file-backed reads. At this pin, active-file rows can be replayed on demand by the snapshot/provider path. Use it when avoiding eager file materialization is valuable; account for the fact that the first operation needing active files/statistics may pay that replay cost. ([Docs.rs][3])

---

## 3.5 Loaded latest vs uninitialized table

```rust id="m76n6m"
use deltalake::DeltaTableBuilder;
use url::Url;

pub async fn build_vs_load(uri: &str) -> anyhow::Result<()> {
    let builder = DeltaTableBuilder::from_url(Url::parse(uri)?)?;

    // Uninitialized: log not loaded, snapshot unavailable.
    let mut table = builder.build()?;
    assert!(table.version().is_none());

    // Explicitly load latest state.
    table.load().await?;
    assert!(table.version().is_some());

    Ok(())
}
```

`DeltaTableBuilder::build()` does not load the transaction log, so the table is not initialized; `load()` builds the table and loads its state. ([Docs.rs][3])

---

## 3.6 Version inspection

```rust id="t7el61"
use deltalake::DeltaTable;

pub async fn inspect_versions(table: &DeltaTable) -> anyhow::Result<()> {
    let loaded_version: Option<u64> = table.version();
    let latest_available: u64 = table.get_latest_version().await?;

    tracing::info!(
        ?loaded_version,
        latest_available,
        "delta table version state"
    );

    Ok(())
}
```

`version()` returns the currently loaded table version, or `None` if the table has not been loaded; `get_latest_version()` returns the latest available version in backing storage. ([Docs.rs][1])

Operational distinction:

```text id="xhl2r3"
table.version():
  local in-memory loaded state

table.get_latest_version().await?:
  remote/backing-store latest visible Delta log version

table.version() == table.get_latest_version().await?:
  loaded state is current at the instant of check

table.version() < table.get_latest_version().await?:
  loaded state is stale or intentionally pinned
```

---

## 3.7 Refreshing latest state

### Full latest-state refresh

```rust id="7cxazg"
pub async fn refresh_to_latest(table: &mut deltalake::DeltaTable) -> anyhow::Result<u64> {
    table.update_state().await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("Delta table not initialized after refresh"))?;

    Ok(version)
}
```

`update_state()` updates the table to the most recent committed transaction-log state by loading the last checkpoint and applying each version since. ([Docs.rs][1])

### Incremental forward-only refresh

```rust id="uadzq5"
pub async fn refresh_incremental_latest(
    table: &mut deltalake::DeltaTable,
) -> anyhow::Result<u64> {
    table.update_incremental(None).await?;

    table
        .version()
        .ok_or_else(|| anyhow::anyhow!("Delta table not initialized after incremental refresh"))
}
```

### Incremental bounded refresh

```rust id="xj986c"
pub async fn refresh_incremental_to_at_most(
    table: &mut deltalake::DeltaTable,
    max_version: u64,
) -> anyhow::Result<u64> {
    table.update_incremental(Some(max_version)).await?;

    table
        .version()
        .ok_or_else(|| anyhow::anyhow!("Delta table not initialized after bounded refresh"))
}
```

`update_incremental(max_version)` forward-applies newer versions and can be bounded by `max_version`; it is forward-only, so loading an older version requires `load_version`. ([Docs.rs][1])

Refresh-selection rule:

```text id="yjlnf8"
Use update_state():
  - table may be badly stale
  - checkpoint boundary may matter
  - service wants canonical latest
  - operational simplicity > minimal log replay

Use update_incremental(None):
  - table is already loaded
  - frequent refresh loop
  - forward-only reader
  - lower metadata IO desired

Use update_incremental(Some(v)):
  - reproducible bounded catch-up
  - CDC/replay control
  - deterministic batch windows
  - avoid accidentally crossing a version boundary
```

---

## 3.8 Loading a specific version

### Mutating an existing handle

```rust id="os2by5"
pub async fn load_exact_version(
    table: &mut deltalake::DeltaTable,
    version: u64,
) -> anyhow::Result<()> {
    table.load_version(version).await?;

    let loaded = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("table not initialized after load_version"))?;

    anyhow::ensure!(
        loaded == version,
        "loaded Delta version mismatch: expected {version}, got {loaded}"
    );

    Ok(())
}
```

`load_version(version)` loads the table state for the specified version. ([Docs.rs][1])

### Builder-based exact-version load

```rust id="o2cpd7"
use deltalake::{DeltaTable, DeltaTableBuilder};
use url::Url;

pub async fn open_exact_version(uri: &str, version: u64) -> anyhow::Result<DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_version(version)
        .load()
        .await?;

    Ok(table)
}
```

Use exact-version loading for:

```text id="xwir16"
reproducible simulation inputs
auditable report regeneration
model-validation replay
CI golden outputs
bug reproduction
before/after optimization comparisons
merge/delete/update diagnostics
cross-engine consistency tests
```

Avoid exact-version loading for:

```text id="q1o6jw"
hot service reads where latest data is required
long-lived providers unless deliberately immutable
writes that assume current schema/protocol
post-vacuum historical reads outside retention policy
```

---

## 3.9 Time travel by timestamp

### Load by `chrono::DateTime<Utc>`

```rust id="5e2o0q"
use chrono::{DateTime, Utc};

pub async fn load_as_of_datetime(
    table: &mut deltalake::DeltaTable,
    datetime: DateTime<Utc>,
) -> anyhow::Result<u64> {
    table.load_with_datetime(datetime).await?;

    table
        .version()
        .ok_or_else(|| anyhow::anyhow!("table not initialized after load_with_datetime"))
}
```

`load_with_datetime(datetime)` time-travels to the latest version created at or before the provided datetime and internally performs a binary search across Delta transaction logs. ([Docs.rs][1])

### Builder with timestamp

```rust id="2b1cdn"
use chrono::{TimeZone, Utc};
use deltalake::DeltaTableBuilder;
use url::Url;

pub async fn open_as_of_timestamp(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let timestamp = Utc
        .with_ymd_and_hms(2026, 6, 2, 12, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid timestamp"))?;

    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_timestamp(timestamp)
        .load()
        .await?;

    Ok(table)
}
```

### Builder with ISO/RFC-3339 string

```rust id="y0kegs"
use deltalake::DeltaTableBuilder;
use url::Url;

pub async fn open_as_of_datestring(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_datestring("2026-06-02T12:00:00Z")?
        .load()
        .await?;

    Ok(table)
}
```

`DeltaTableBuilder::with_datestring` accepts an ISO-8601/RFC-3339 timestamp string, and `with_timestamp` accepts a `DateTime<Utc>`. ([Docs.rs][3])

Time-travel guidance:

```text id="sqcoel"
Prefer version pinning when:
  - exact reproducibility is required
  - version number is persisted in job metadata
  - deterministic CI/golden testing matters

Prefer datetime pinning when:
  - user-facing "as of timestamp" UX is required
  - audit/reporting asks for temporal cutoff
  - upstream systems do not record Delta versions

Persist both when possible:
  requested_as_of_timestamp
  resolved_delta_version
  resolved_version_commit_timestamp
```

---

## 3.10 Snapshot inspection

```rust id="w2gqxq"
pub fn inspect_snapshot(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let state = table.snapshot()?;

    let version = state.version();
    let protocol = state.protocol();
    let metadata = state.metadata();
    let schema = state.schema();
    let table_config = state.table_config();

    tracing::info!(
        version,
        ?protocol,
        ?metadata,
        ?schema,
        ?table_config,
        "delta snapshot inspection"
    );

    Ok(())
}
```

`DeltaTable::snapshot()` returns the currently loaded `DeltaTableState` and errors with `NotInitialized` when state has not been loaded; `DeltaTableState` exposes `version`, `protocol`, `metadata`, `schema`, `table_config`, `version_timestamp`, `log_data`, `snapshot`, `add_actions_table`, and `add_actions_batches`. ([Docs.rs][1])

Inspection categories:

```text id="0vlaou"
protocol:
  reader/writer compatibility
  table-feature compatibility
  cross-engine safety

metadata:
  table ID
  table name/description/configuration
  partition columns
  schema string/source metadata

schema:
  Delta logical schema
  Arrow conversion target
  DataFusion provider schema basis

table_config:
  CDF
  append-only
  constraints/invariants
  data-skipping properties
  retention properties
```

---

## 3.11 Table history

```rust id="3mfjkj"
pub async fn log_recent_history(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let commits = table.history(Some(20)).await?;

    for commit in commits {
        tracing::info!(?commit, "delta commit history item");
    }

    Ok(())
}
```

```rust id="o6fgum"
pub async fn collect_full_history(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<Vec<deltalake::kernel::CommitInfo>> {
    let commits = table.history(None).await?;
    Ok(commits.collect())
}
```

`history(limit)` returns provenance information for table writes, including operation/user-style commit information; with `Some(limit)`, it returns the latest `limit` commits, and with `None`, it returns all available commits from the earliest retained commit. History retention is governed by the Delta table’s `logRetentionDuration`, documented as 30 days by default. ([Docs.rs][1])

Production uses:

```text id="k6qkar"
audit trail
operation-class frequency
writer/client identification
rollback investigation
schema-change detection
maintenance-job verification
commit-metadata observability
simulation-run lineage
```

---

## 3.12 Active file URI enumeration

### All active file URIs

```rust id="j78xtp"
pub fn active_file_uris(table: &deltalake::DeltaTable) -> anyhow::Result<Vec<String>> {
    let files: Vec<String> = table.get_file_uris()?.collect();
    Ok(files)
}
```

`get_file_uris()` returns an iterator of URI strings for all active files present in the current loaded table version. ([Docs.rs][1])

### Partition-filtered file URIs

```rust id="26vw4y"
use deltalake::PartitionFilter;

pub async fn active_file_uris_for_partition(
    table: &deltalake::DeltaTable,
    run_date: &str,
) -> anyhow::Result<Vec<String>> {
    let filters = vec![
        PartitionFilter::try_from(("run_date", "=", run_date))?,
    ];

    let uris = table.get_file_uris_by_partitions(&filters).await?;
    Ok(uris)
}
```

`PartitionFilter` has `key` and `PartitionValue` fields and implements `TryFrom<(&str, &str, &str)>` and `TryFrom<(&str, &str, &[&str])>` for tuple-style construction; `get_file_uris_by_partitions(filters)` returns URI strings for matching partitions. ([Docs.rs][4])

### Partition-filtered relative/log paths

```rust id="sjgzxt"
use deltalake::PartitionFilter;

pub async fn active_log_paths_for_partition(
    table: &deltalake::DeltaTable,
    scenario_id: &str,
) -> anyhow::Result<Vec<deltalake::Path>> {
    let filters = vec![
        PartitionFilter::try_from(("scenario_id", "=", scenario_id))?,
    ];

    let paths = table.get_files_by_partitions(&filters).await?;
    Ok(paths)
}
```

`get_files_by_partitions(filters)` returns the file list tracked in the current table state filtered by `PartitionFilter`s. ([Docs.rs][1])

### Active add-action stream

```rust id="9byf9k"
use deltalake::PartitionFilter;
use futures::StreamExt;

pub async fn stream_active_add_actions(
    table: &deltalake::DeltaTable,
    scenario_id: &str,
) -> anyhow::Result<()> {
    let filters = vec![
        PartitionFilter::try_from(("scenario_id", "=", scenario_id))?,
    ];

    let mut stream = table.get_active_add_actions_by_partitions(&filters);

    while let Some(item) = stream.next().await {
        let logical_file = item?;
        tracing::debug!(?logical_file, "active delta add action");
    }

    Ok(())
}
```

`get_active_add_actions_by_partitions` streams logical files matching partition filters, while `DeltaTableState::add_actions_table(flatten)` can expose add-action metadata as an Arrow `RecordBatch` where each row represents an active file and includes fields such as path, size, modification time, partition values, min/max/null-count/statistics columns when available. ([Docs.rs][1])

---

## 3.13 Add actions as Arrow batches

```rust id="b81axf"
pub fn add_actions_record_batch(
    table: &deltalake::DeltaTable,
    flatten: bool,
) -> anyhow::Result<arrow_array::RecordBatch> {
    let state = table.snapshot()?;
    let batch = state.add_actions_table(flatten)?;
    Ok(batch)
}
```

```rust id="zbwb0e"
pub fn add_actions_batches(
    table: &deltalake::DeltaTable,
    flatten: bool,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let state = table.snapshot()?;
    let batches = state.add_actions_batches(flatten)?;
    Ok(batches)
}
```

Use this in a DataFusion/Arrow-heavy codebase to introspect Delta file layout as tabular metadata:

```text id="73py64"
file count by partition
file size distribution
stats-column availability
data-skipping coverage
small-file detection
partition skew detection
maintenance-job prioritization
lineage/debugging of active files
```

---

## 3.14 Metadata/protocol validation gate

```rust id="ki8nhm"
pub fn validate_loaded_table_for_service(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let state = table.snapshot()?;

    let version = state.version();
    let protocol = state.protocol();
    let metadata = state.metadata();
    let schema = state.schema();

    tracing::info!(
        version,
        ?protocol,
        ?metadata,
        ?schema,
        "validated loaded delta table snapshot"
    );

    // Put service-specific checks here:
    // - required columns
    // - prohibited table features
    // - append-only expectation
    // - CDF expectation
    // - partition column expectation
    // - protocol compatibility expectation

    Ok(())
}
```

Do this once on service startup for required tables and again after any version-changing refresh if your application assumes a stable schema/protocol.

Validation checklist:

```text id="w8jr3d"
required columns exist
partition columns match service expectation
protocol writer/reader versions are supported
table properties match feature expectation
schema hash matches compiled simulation contract where applicable
CDF enabled when incremental readers depend on it
append-only enabled when mutability is forbidden
schema evolution detected before unsafe query compilation
```

---

## 3.15 Version pinning for reproducibility

### Persist resolved version

```rust id="aemhj9"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaSnapshotPin {
    pub table_uri: String,
    pub delta_version: u64,
    pub requested_as_of: Option<String>,
    pub resolved_at_utc: String,
}

pub fn make_snapshot_pin(
    table_uri: impl Into<String>,
    table: &deltalake::DeltaTable,
    requested_as_of: Option<String>,
) -> anyhow::Result<DeltaSnapshotPin> {
    let delta_version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("cannot pin uninitialized Delta table"))?;

    Ok(DeltaSnapshotPin {
        table_uri: table_uri.into(),
        delta_version,
        requested_as_of,
        resolved_at_utc: chrono::Utc::now().to_rfc3339(),
    })
}
```

### Reopen from pin

```rust id="k9thci"
use deltalake::{DeltaTable, DeltaTableBuilder};
use std::collections::HashMap;
use url::Url;

pub async fn open_from_pin(
    pin: &DeltaSnapshotPin,
    storage_options: HashMap<String, String>,
) -> anyhow::Result<DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(&pin.table_uri)?)?
        .with_storage_options(storage_options)
        .with_version(pin.delta_version)
        .load()
        .await?;

    Ok(table)
}
```

Pinned-snapshot metadata should be stored alongside:

```text id="u9ycko"
simulation run record
model build artifact
query result cache
report export
optimization baseline
debug/replay bundle
cross-engine validation output
```

Do not persist only a timestamp when exact version is available; timestamp resolution and commit-log availability can make later reproduction less direct than version pinning.

---

## 3.16 Snapshot caching patterns

### Pattern A — read-mostly latest cache

```rust id="9xmsm5"
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct DeltaTableCache {
    inner: Arc<RwLock<deltalake::DeltaTable>>,
}

impl DeltaTableCache {
    pub fn new(table: deltalake::DeltaTable) -> Self {
        Self {
            inner: Arc::new(RwLock::new(table)),
        }
    }

    pub async fn loaded_version(&self) -> Option<u64> {
        self.inner.read().await.version()
    }

    pub async fn refresh_latest(&self) -> anyhow::Result<u64> {
        let mut table = self.inner.write().await;
        table.update_incremental(None).await?;

        table
            .version()
            .ok_or_else(|| anyhow::anyhow!("table not initialized after refresh"))
    }

    pub async fn with_snapshot<R>(
        &self,
        f: impl FnOnce(&deltalake::table::state::DeltaTableState) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let table = self.inner.read().await;
        let state = table.snapshot()?;
        f(state)
    }
}
```

Use for:

```text id="d4erkn"
metadata-heavy API service
dashboard state
catalog/schema visibility
small number of hot Delta tables
freshness SLA measured in seconds/minutes
```

### Pattern B — immutable per-request version pin

```rust id="pa73j5"
pub async fn open_request_pinned_table(
    uri: &str,
    storage_options: std::collections::HashMap<String, String>,
    version: Option<u64>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let mut builder = deltalake::DeltaTableBuilder::from_url(url::Url::parse(uri)?)?
        .with_storage_options(storage_options);

    if let Some(version) = version {
        builder = builder.with_version(version);
    }

    Ok(builder.load().await?)
}
```

Use for:

```text id="u5mjcu"
deterministic query APIs
report generation
simulation replay
long-running query consistency
user-selected "as of version"
```

### Pattern C — latest-version watcher

```rust id="oesjvi"
pub async fn refresh_if_newer(table: &mut deltalake::DeltaTable) -> anyhow::Result<bool> {
    let loaded = table.version();
    let latest = table.get_latest_version().await?;

    if loaded == Some(latest) {
        return Ok(false);
    }

    table.update_incremental(Some(latest)).await?;
    Ok(true)
}
```

Use for:

```text id="oo3ah5"
low-cost freshness checks
hot table cache refresh
event-loop-based metadata refresh
avoiding unnecessary provider rebuilds
```

---

## 3.17 Open once vs open per query

### Open once, refresh intentionally

```text id="c9bkkw"
Best for:
  long-lived service
  bounded table set
  stable storage credentials
  frequent repeated queries
  schema/protocol validation at startup
  DataFusion provider/caching integration

Risks:
  stale state unless refreshed
  stale DataFusion provider if provider is bound to old state
  credential rotation complexity
```

### Open per query/request

```text id="yyvqri"
Best for:
  deterministic version-pinned requests
  infrequent accesses
  per-tenant credentials
  per-request storage options
  short batch jobs

Risks:
  repeated log replay/listing
  object-store overhead
  request latency
  harder provider reuse
```

### Hybrid

```text id="bcxg5o"
Best default:
  maintain table registry/cache
  refresh metadata periodically or before writes
  pin version for long-running queries
  rebuild DataFusion provider after table refresh when snapshot correctness matters
  expose explicit "latest" vs "as_of_version" semantics to callers
```

---

## 3.18 Avoiding stale state

Common stale-state failure modes:

```text id="zcecc4"
- service opens table at startup and never refreshes
- DataFusion provider registered once while writers continue committing
- table.version() used as latest without get_latest_version() check
- long-running process mutates table state backward/forward for unrelated requests
- cached schema survives schema evolution
- cached active file list survives compaction/optimize/delete/merge
- writer commits then reader uses pre-commit handle
```

Freshness guard:

```rust id="23m4dr"
pub async fn require_latest(table: &mut deltalake::DeltaTable) -> anyhow::Result<u64> {
    let latest = table.get_latest_version().await?;
    let loaded = table.version();

    if loaded != Some(latest) {
        table.update_incremental(Some(latest)).await?;
    }

    let now_loaded = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

    anyhow::ensure!(
        now_loaded == latest,
        "Delta table still stale after refresh: loaded={now_loaded}, latest={latest}"
    );

    Ok(now_loaded)
}
```

Read-policy taxonomy:

```text id="l5jjaa"
LATEST_STRICT:
  check latest version before query
  refresh when stale
  rebuild provider if provider caches snapshot

LATEST_EVENTUAL:
  periodic refresh
  tolerate bounded staleness

PINNED_VERSION:
  never refresh within request
  use load_version/with_version
  return resolved version in response

AS_OF_TIME:
  resolve datetime to version
  return resolved version
  avoid mutating same handle for unrelated requests
```

---

## 3.19 DataFusion integration implications

`DeltaTable::table_provider()` returns a `TableProviderBuilder`, and `update_datafusion_session` registers the table root object store into the DataFusion runtime environment if missing; that registration is idempotent and will not overwrite an existing object-store mapping. ([Docs.rs][1])

### Register pinned provider

```rust id="7074sm"
use datafusion::prelude::SessionContext;
use deltalake::DeltaTableBuilder;
use url::Url;

pub async fn register_pinned_delta_provider(
    ctx: &SessionContext,
    name: &str,
    uri: &str,
    version: u64,
) -> anyhow::Result<()> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_version(version)
        .load()
        .await?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let provider = table.table_provider().await?;
    ctx.register_table(name, provider)?;

    Ok(())
}
```

### Register latest provider after refresh

```rust id="u0tvmp"
use datafusion::prelude::SessionContext;

pub async fn refresh_and_register_provider(
    ctx: &SessionContext,
    table: &mut deltalake::DeltaTable,
    name: &str,
) -> anyhow::Result<u64> {
    table.update_incremental(None).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let provider = table.table_provider().await?;
    ctx.deregister_table(name)?;
    ctx.register_table(name, provider)?;

    Ok(version)
}
```

Provider-freshness rule:

```text id="rlkoz7"
If table state changes and query correctness requires the new snapshot:
  refresh DeltaTable
  rebuild/register provider
  include resolved Delta version in query diagnostics

If table is pinned:
  do not refresh during query
  provider name should encode or carry version metadata
```

---

## 3.20 Service registry pattern

```rust id="3o2h3e"
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct DeltaTableSpec {
    pub logical_name: String,
    pub uri: String,
    pub storage_options: HashMap<String, String>,
    pub freshness: FreshnessPolicy,
}

#[derive(Debug, Clone)]
pub enum FreshnessPolicy {
    LatestStrict,
    LatestEventual,
    Pinned(u64),
}

#[derive(Clone)]
pub struct DeltaTableRegistry {
    tables: Arc<RwLock<HashMap<String, Arc<RwLock<deltalake::DeltaTable>>>>>,
}

impl DeltaTableRegistry {
    pub fn new() -> Self {
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_table(&self, spec: DeltaTableSpec) -> anyhow::Result<u64> {
        let mut builder = deltalake::DeltaTableBuilder::from_url(url::Url::parse(&spec.uri)?)?
            .with_storage_options(spec.storage_options);

        if let FreshnessPolicy::Pinned(version) = spec.freshness {
            builder = builder.with_version(version);
        }

        let table = builder.load().await?;
        let version = table
            .version()
            .ok_or_else(|| anyhow::anyhow!("loaded Delta table has no version"))?;

        self.tables
            .write()
            .await
            .insert(spec.logical_name, Arc::new(RwLock::new(table)));

        Ok(version)
    }

    pub async fn refresh_latest_strict(&self, logical_name: &str) -> anyhow::Result<u64> {
        let tables = self.tables.read().await;
        let table_lock = tables
            .get(logical_name)
            .ok_or_else(|| anyhow::anyhow!("unknown Delta table: {logical_name}"))?
            .clone();

        drop(tables);

        let mut table = table_lock.write().await;
        table.update_incremental(None).await?;

        table
            .version()
            .ok_or_else(|| anyhow::anyhow!("Delta table not initialized after refresh"))
    }
}
```

Registry value case:

```text id="iqdakx"
centralizes storage options
centralizes version/freshness policy
avoids accidental per-request log replay
prevents mutation of random table handles
supports tenant/table isolation
supports DataFusion provider rebuild policy
exposes deterministic resolved-version observability
```

---

## 3.21 Error handling taxonomy

```text id="7vgipc"
NotInitialized:
  snapshot() called before load/build+load/update_state
  fix: use open_table/open_table_with_storage_options/builder.load/table.load

VersionNotFound / log replay failure:
  requested version unavailable
  fix: validate history retention, vacuum/log cleanup, requested version

Datetime resolution failure:
  timestamp predates first retained commit or logs unavailable
  fix: return no-such-as-of-time diagnostic

Object-store error:
  credentials, endpoint, list/get/read failures
  fix: verify feature flag, URL scheme, storage_options, IAM

Protocol incompatibility:
  table has unsupported reader/writer/table features
  fix: block query/write, upgrade dependency, use compatible table

Stale state:
  loaded version behind latest
  fix: update_incremental/update_state/rebuild provider

Schema drift:
  refreshed snapshot has unexpected schema
  fix: fail closed, migrate compiled schema contract
```

Diagnostic payload:

```rust id="x10yg8"
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeltaStateDiagnostic {
    pub table_uri: String,
    pub loaded_version: Option<u64>,
    pub latest_version: Option<u64>,
    pub freshness_policy: String,
    pub operation: String,
}
```

---

## 3.22 Testing matrix

```text id="wkwb7u"
unit:
  Url parsing
  storage option construction
  version pin struct serialization
  freshness policy selection
  NotInitialized behavior via builder.build()

local integration:
  create table
  open latest
  inspect snapshot
  append/write new version
  update_incremental sees new version
  load_version restores older version
  history returns commits
  get_file_uris returns active files

time travel:
  open as-of timestamp
  verify resolved version
  invalid timestamp behavior
  post-vacuum historical-read behavior, if retention test available

object-store integration:
  S3/MinIO open latest
  storage_options path
  missing credentials
  bad endpoint
  version pin with object storage
  partition-filtered file listing

DataFusion integration:
  register provider at version N
  write version N+1
  verify old provider behavior
  rebuild provider
  verify new provider behavior
```

Minimal compile smoke:

```rust id="bd4lkj"
#[tokio::test]
async fn delta_table_state_api_compiles() -> anyhow::Result<()> {
    let table = deltalake::DeltaTable::new_in_memory();

    assert!(table.version().is_none());

    Ok(())
}
```

`DeltaTable::new_in_memory()` creates an uninitialized in-memory table for testing and does not persist changes beyond the table object lifetime. ([Docs.rs][1])

---

## 3.23 Best practices

```text id="cflsh8"
API:
  - use Url explicitly
  - use open_table/open_table_with_storage_options for normal loads
  - use DeltaTableBuilder for version/timestamp/custom load configuration
  - use update_state/update_incremental for state refresh
  - never use DeltaTable::update() for refresh; it is DML

versioning:
  - persist resolved version for every reproducible job
  - return resolved version in user/API responses
  - pin long-running reports and simulations
  - avoid mixing latest reads and pinned reads through one mutable handle

freshness:
  - define table freshness policy explicitly
  - check loaded vs latest version for strict latest reads
  - rebuild DataFusion provider after refresh when provider snapshot correctness matters
  - validate schema/protocol after refresh

performance:
  - do not open per row or tight-loop operation
  - prefer incremental refresh for hot loaded tables
  - use metadata-only/without_files paths only for metadata-only use cases
  - avoid full file enumeration on very large tables unless required

governance:
  - validate protocol before writes
  - validate schema before query compilation
  - inspect table properties for CDF/append-only/retention assumptions
  - expose commit history in diagnostic endpoints

testing:
  - test NotInitialized path
  - test stale-state path
  - test exact-version load
  - test datetime-to-version resolution
  - test partition-filtered file listing
  - test DataFusion provider rebuild semantics
```

---

## 3.24 Anti-patterns

```text id="zk8ygp"
- table.update().await? as a refresh operation
- passing raw &str to open_table (it takes url::Url at the 1.0.0 pinned rev)
- assuming open once means latest forever
- using table.version() without knowing whether state was refreshed
- calling snapshot() on builder.build() output before load()
- mutating one shared DeltaTable handle backward to old versions for unrelated requests
- using datetime time travel but failing to persist resolved version
- using active file URI enumeration as a substitute for DataFusion provider/query planning
- registering a DataFusion provider once and assuming it tracks future Delta commits automatically
- bypassing Delta state and scanning table Parquet files directly
- ignoring schema/protocol drift after refresh
```

---

## 3.25 LLM-agent checklist

```text id="99j0tf"
Before table-state code generation:
  1. Convert path/URI to url::Url.
  2. Select open_table, open_table_with_storage_options, or DeltaTableBuilder.
  3. Decide freshness policy: latest strict, latest eventual, pinned version, as-of timestamp.
  4. If exact version: use with_version or load_version.
  5. If datetime: use with_timestamp, with_datestring, or load_with_datetime.
  6. If refresh: use update_state or update_incremental, not update().
  7. After load/refresh: read table.version().
  8. For metadata: use table.snapshot()?.metadata().
  9. For protocol: use table.snapshot()?.protocol().
  10. For schema: use table.snapshot()?.schema().
  11. For active files: use get_file_uris or partition-filtered APIs.
  12. For history: use history(Some(n)) or history(None).
  13. For reproducibility: persist resolved Delta version.
  14. For DataFusion: rebuild/register provider after snapshot refresh when needed.
  15. For service code: avoid per-query open unless pinned/request-isolated behavior is required.
```

---

## 3.26 Value case

Correct table loading/state management gives your Rust DataFusion/Arrow simulation platform:

```text id="09cp6k"
deterministic simulation replay
auditable model/report generation
freshness-aware query serving
explicit stale-state detection
Delta protocol/schema governance
active-file and add-action observability
partition-level file diagnostics
safe DataFusion provider lifecycle control
version-pinned cross-engine validation
high-fidelity debugging of write/read anomalies
```

Core invariant:

```text id="cdkwm6"
A DeltaTable handle is only as fresh as its loaded DeltaTableState.
```

Production corollary:

```text id="jtsqrx"
Always know whether a read is latest, eventually latest, pinned by version, or resolved from timestamp.
```

---

## 3.27 `BlindDeltaTable`: stats-free loading for blind appends (new at the 1.0.0 pinned rev)

The 1.0.0 line adds `BlindDeltaTable`, re-exported at the crate root (`pub use self::table::{BlindDeltaTable, DeltaTable}` in `deltalake-core`, visible as `deltalake::BlindDeltaTable`). It is a lightweight table handle for append-only write workloads: it loads only table metadata (protocol, schema, properties) and **skips file-statistics loading entirely**, which materially reduces load time on large tables.

```rust
use deltalake::BlindDeltaTable;
use deltalake::writer::{DeltaWriter, RecordBatchWriter};

// Loads protocol/schema/properties only — no file-stats parsing.
let mut table = BlindDeltaTable::try_new("s3://bucket/table").await?;

let mut writer = RecordBatchWriter::for_blind_appends(&table)?;
writer.write(batch).await?;

let adds = writer.flush().await?;
table.commit(adds).await?;   // kernel Transaction API commit path, returns new version
```

Surface at the pinned rev (all verified in `crates/core/src/table/blind.rs`):

```text
constructors: try_new(table_uri) / try_new_with_options(...) / try_new_with_log_store(log_store)
accessors:    log_store() / object_store() / table_url() / version() / metadata()
              protocol() / schema() -> kernel SchemaRef / arrow_schema()
              table_properties() / snapshot() / is_append_only()
commit:       commit(adds: Vec<Add>) -> u64  (kernel FileSystemCommitter,
              up to 15 internal commit retries)
```

Design constraints to respect:

```text
- Intentionally does NOT expose files() / log_data(): no file statistics are
  loaded, so merge / delete / update / read paths are impossible by construction.
- Use DeltaTable for anything that reads data or needs the DataFusion provider.
- Ideal for high-throughput ingest services appending to large tables where
  DeltaTable::load() cost is dominated by stats parsing.
- Commits bypass CommitBuilder and use the kernel Transaction API directly.
```



## 3.28 Lazy snapshot replay, cache identity, and same-version checkpoint refresh (latest pin)

The post-`9f922319` baseline makes the kernel-backed snapshot model more explicit and more defensive. The current source distinguishes two internal materialization modes:

```text
Snapshot (lazy / require_files=false):
  protocol + metadata are resident
  active-file rows are replayed on demand
  replay can request no stats, raw JSON stats, or parsed stats
  replayed batches can remain operation-local instead of becoming a resident cache

EagerSnapshot / require_files=true:
  maintains a materialized active-file cache
  cache policy records whether raw statistics were preserved
```

Every materialized file cache now carries a `SnapshotIdentity` covering the table root, Delta version, checkpoint version, protocol, and metadata. A cache is reused only when that identity and the requested stats capability match the live snapshot. Identity-less or mismatched cached state is rejected/ignored and the table state is replayed instead. The serialization compatibility path explicitly distinguishes pre-identity caches so stale state cannot silently become authoritative.

A further subtlety matters for reproducibility: a checkpoint may be written **after** a process has already loaded the same logical Delta version. The current snapshot update path can rebuild that same-version snapshot to adopt the newly available checkpoint. This is a storage/replay optimization only: **the logical Delta version has not changed**. Application snapshot identities should therefore pin the Delta table version, not a particular checkpoint file, unless the application is specifically testing checkpoint encoding.

Operational guidance:

```text
- Treat delta-rs materialized-file caches as ephemeral implementation state.
- Do not persist them as an application source of truth across releases.
- For deterministic reads, pin the table version; checkpoint selection may vary.
- `without_files()` is now a stronger basis for metadata-first / lazy workflows,
  but query-serving handles should still follow the public `skip_stats` guidance.
- Benchmark table-open memory and first-query latency separately: lazy loading can
  shift work from activation time to the first operation that needs file state.
```

Primary source: `crates/core/src/kernel/snapshot/mod.rs` at the pinned revision and commits `53d4475a…`, `0b83064c…`, `439583af…`, `95ad71d1…`, and `84fad0b1…`.

[1]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/fn.open_table.html "open_table in deltalake - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/struct.DeltaTableBuilder.html "DeltaTableBuilder in deltalake - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/struct.PartitionFilter.html "PartitionFilter in deltalake - Rust"



# 4. Schema, Arrow type mapping, and metadata governance — Rust `deltalake`

Version target:

```text id="6ydet6"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required features for this section:
  ["datafusion"] for DataFusion read/write/query paths
Typical production features:
  ["rustls", "datafusion", "s3"]
Primary schema APIs:
  StructType
  StructField
  DataType
  ArrayType
  MapType
  DecimalType
  DeltaTableState::schema()
  DeltaTable::add_columns()
  DeltaTable::update_field_metadata()
  DeltaTable::update_table_metadata()
  WriteBuilder::with_schema_mode()
  WriteBuilder::with_cast_safety()
```

Current API correction: in `deltalake` 1.0.0 @ the pinned rev, use `StructType::try_new(...)` for validated Delta schema construction, not `StructType::new(...)` as the canonical production form. `try_new` validates duplicate field names case-insensitively and rejects invalid metadata-column collisions; `StructField` provides `new`, `nullable`, `not_null`, metadata helpers, Arrow conversion traits, column-mapping helpers, and physical-name helpers. ([Docs.rs][1])

---

## 4.1 Delta schema mental model

```text id="z7gjrs"
Delta schema:
  Delta logical schema
  stored in Delta table metadata
  represented in Rust by StructType / StructField / DataType
  converted to/from Arrow schema at read/write/query boundaries
  interpreted by DataFusion through TableProvider schema
  persisted in transaction-log metadata actions

Arrow schema:
  in-memory execution schema
  RecordBatch boundary
  DataFusion physical/logical execution boundary
  Parquet writer input boundary

Parquet schema:
  persisted data-file schema
  may differ physically from logical Delta schema under column mapping
  carries physical encoding/statistics
```

`DeltaTableState::schema()` returns the loaded table’s current Delta logical schema, while `metadata()`, `protocol()`, and `table_config()` expose table metadata, protocol compatibility, and table-property state. ([Docs.rs][2])

Core invariant:

```text id="n3tbfv"
Delta schema is the table contract.
Arrow schema is the execution/batch contract.
Parquet schema is the physical file contract.
```

---

## 4.2 Cargo baseline

```toml id="yuubak"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
arrow-cast = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

Do not allow Arrow/DataFusion to float independently from the Delta baseline. Rust Arrow schemas, arrays, `RecordBatch`, DataFusion `TableProvider`, and DataFusion `Expr` are concrete crate-versioned types; duplicate Arrow versions can produce “expected `RecordBatch`, found `RecordBatch`” failures.

---

## 4.3 Canonical Delta schema construction

```rust id="lg2w71"
use deltalake::kernel::{DataType, StructField, StructType};

pub fn simulation_output_delta_schema() -> anyhow::Result<StructType> {
    let schema = StructType::try_new(vec![
        StructField::not_null("simulation_id", DataType::STRING),
        StructField::not_null("run_id", DataType::STRING),
        StructField::not_null("run_date", DataType::DATE),
        StructField::not_null("scenario_family", DataType::STRING),
        StructField::not_null("unit_id", DataType::STRING),
        StructField::not_null("metric_id", DataType::STRING),
        StructField::nullable("output_value", DataType::DOUBLE),
        StructField::nullable("output_unit", DataType::STRING),
        StructField::not_null("created_at", DataType::TIMESTAMP),
    ])?;

    Ok(schema)
}
```

Use `StructField::not_null` for required identifiers, required partition columns, required timestamps, and contract keys. Use `StructField::nullable` only where missingness is semantically valid.

Explicit constructor form:

```rust id="8gwhih"
use deltalake::kernel::{DataType, StructField, StructType};

pub fn small_schema() -> anyhow::Result<StructType> {
    Ok(StructType::try_new(vec![
        StructField::new("id", DataType::INTEGER, false),
        StructField::new("name", DataType::STRING, true),
    ])?)
}
```

---

## 4.4 Primitive type catalog

`DataType` in the Rust kernel model includes primitive, array, struct, map, and variant categories, with constants for common primitive logical types including string, integer families, floating-point, boolean, binary, date, timestamp, and timestamp without timezone. It also exposes decimal and variant constructors. ([Docs.rs][3])

```rust id="2k6r9x"
use deltalake::kernel::DataType;

pub fn primitive_type_examples() -> Vec<(&'static str, DataType)> {
    vec![
        ("string_col", DataType::STRING),
        ("long_col", DataType::LONG),
        ("integer_col", DataType::INTEGER),
        ("short_col", DataType::SHORT),
        ("byte_col", DataType::BYTE),
        ("float_col", DataType::FLOAT),
        ("double_col", DataType::DOUBLE),
        ("boolean_col", DataType::BOOLEAN),
        ("binary_col", DataType::BINARY),
        ("date_col", DataType::DATE),
        ("timestamp_col", DataType::TIMESTAMP),
        ("timestamp_ntz_col", DataType::TIMESTAMP_NTZ),
    ]
}
```

Practical mapping guidance:

```text id="1ec7v1"
Identifiers:
  STRING unless numeric semantics are required

Surrogate / monotonically increasing IDs:
  LONG

Counts:
  LONG

Small bounded integer flags:
  INTEGER unless storage/interop explicitly justifies smaller types

Engineering continuous values:
  DOUBLE by default
  DECIMAL for exact fixed-precision values

Money/accounting:
  DECIMAL, not DOUBLE

Raw bytes:
  BINARY

Event dates:
  DATE for calendar date
  TIMESTAMP for instant-like event time
  TIMESTAMP_NTZ only with explicit cross-engine policy
```

---

## 4.5 Create table from Delta schema

```rust id="6xmq72"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_simulation_output_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let schema = simulation_output_delta_schema()?;

    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Canonical simulation output facts")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(schema.fields().cloned())
        .with_partition_columns(["run_date", "scenario_family"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

The Rust creation examples show constructing a Delta `StructType` from `StructField`s and passing `schema.fields().cloned()` into `CreateBuilder::with_columns`; the same docs show Arrow `Schema`, `Field`, arrays, `RecordBatch`, and a first write into the created table. ([delta-io.github.io][4])

---

## 4.6 Arrow schema boundary

```rust id="y6od3y"
use std::sync::Arc;

use arrow_array::{Float64Array, RecordBatch, StringArray, TimestampMicrosecondArray};
use arrow_schema::{DataType as ArrowDataType, Field, Schema as ArrowSchema, TimeUnit};

pub fn simulation_output_arrow_batch() -> anyhow::Result<RecordBatch> {
    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("simulation_id", ArrowDataType::Utf8, false),
        Field::new("run_id", ArrowDataType::Utf8, false),
        Field::new("run_date", ArrowDataType::Date32, false),
        Field::new("scenario_family", ArrowDataType::Utf8, false),
        Field::new("unit_id", ArrowDataType::Utf8, false),
        Field::new("metric_id", ArrowDataType::Utf8, false),
        Field::new("output_value", ArrowDataType::Float64, true),
        Field::new("output_unit", ArrowDataType::Utf8, true),
        Field::new(
            "created_at",
            ArrowDataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["sim-001"])),
            Arc::new(StringArray::from(vec!["run-001"])),
            Arc::new(arrow_array::Date32Array::from(vec![20_606])),
            Arc::new(StringArray::from(vec!["base_case"])),
            Arc::new(StringArray::from(vec!["unit-a"])),
            Arc::new(StringArray::from(vec!["yield"])),
            Arc::new(Float64Array::from(vec![Some(0.92)])),
            Arc::new(StringArray::from(vec![Some("fraction")])),
            Arc::new(TimestampMicrosecondArray::from(vec![1_717_300_000_000_000_i64])),
        ],
    )?;

    Ok(batch)
}
```

The kernel module exposes Arrow conversion support for Delta/Arrow schemas and types, and Delta write examples use Arrow `RecordBatch` input as the Rust data boundary. ([Docs.rs][5])

Boundary rule:

```text id="k8dcvd"
Use Delta StructType for table contracts.
Use Arrow Schema/RecordBatch for in-memory batches.
Validate Arrow schema against Delta schema before write.
```

---

## 4.7 Arrow-to-Delta schema validation posture

```rust id="ezfpzt"
pub fn validate_record_batches_for_write(
    batches: &[arrow_array::RecordBatch],
) -> anyhow::Result<arrow_schema::SchemaRef> {
    anyhow::ensure!(!batches.is_empty(), "refusing to write zero RecordBatches");

    let schema = batches[0].schema();

    for (idx, batch) in batches.iter().enumerate() {
        anyhow::ensure!(
            batch.schema().as_ref() == schema.as_ref(),
            "batch {idx} schema differs from first batch schema"
        );
    }

    Ok(schema)
}
```

Recommended stronger validation before governed writes:

```text id="udv84x"
- exact field names
- exact field order, if table contract requires stable ordering
- exact Arrow data types
- expected nullability
- no missing required fields
- no unexpected fields unless schema_mode=Merge is explicitly approved
- decimal precision/scale match
- timestamp unit/timezone policy match
- partition columns present and non-null when required
```

---

## 4.8 Nullable fields

```rust id="ssfel2"
use deltalake::kernel::{DataType, StructField};

pub fn nullable_policy_fields() -> Vec<StructField> {
    vec![
        StructField::not_null("simulation_id", DataType::STRING),
        StructField::not_null("run_id", DataType::STRING),
        StructField::not_null("run_date", DataType::DATE),
        StructField::nullable("output_value", DataType::DOUBLE),
        StructField::nullable("output_unit", DataType::STRING),
        StructField::nullable("quality_code", DataType::STRING),
    ]
}
```

Nullability policy:

```text id="ae1f7i"
NOT NULL:
  identity keys
  partition columns
  required timestamps
  enum/domain discriminator columns
  foreign-key-like references

NULLABLE:
  optional measurements
  late-arriving enrichment
  sparse annotations
  output values that can be undefined
  quality/error fields
```

Anti-pattern:

```text id="u13fl0"
Making every column nullable to avoid write failures.
```

That pushes validation complexity downstream and weakens DataFusion/Arrow type contracts.

Relaxation path: if a `not_null` column later needs to admit nulls, the 1.0.0 pinned rev adds a dedicated operation — `DeltaTable::drop_column_not_null()` (`DropColumnNotNullBuilder`); see §11.7. Tightening in the other direction (nullable → NOT NULL) remains unsupported without a rewrite.

---

## 4.9 Field metadata

```rust id="3zvxi6"
use deltalake::kernel::{DataType, MetadataValue, StructField};
use std::collections::HashMap;

pub fn field_with_governance_metadata() -> StructField {
    let metadata: HashMap<String, MetadataValue> = HashMap::from([
        ("semantic_type".to_owned(), "simulation_identifier".into()),
        ("governance.required".to_owned(), "true".into()),
        ("owner".to_owned(), "simulation_engine".into()),
    ]);

    StructField::not_null("simulation_id", DataType::STRING)
        .with_metadata(metadata)
}
```

`StructField` metadata is a `HashMap<String, MetadataValue>`, and methods exist for setting and adding metadata. Metadata is useful for semantic annotations, units, ownership, governance hints, and UI hints; it is not a replacement for enforced constraints or authorization. ([Docs.rs][6])

Field metadata guidance:

```text id="9z9wp1"
Good:
  semantic_type
  unit_family
  physical_unit
  source_system
  quality_policy
  governance.classification
  display_name
  contract_version

Bad:
  secrets
  tokens
  raw credentials
  row-level policy
  high-cardinality business values
  volatile runtime state
```

---

## 4.10 Update field metadata

```rust id="g0f4cz"
use deltalake::kernel::MetadataValue;
use std::collections::HashMap;

pub async fn update_simulation_id_metadata(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let metadata: HashMap<String, MetadataValue> = HashMap::from([
        ("semantic_type".to_owned(), "simulation_identifier".into()),
        ("governance.required".to_owned(), "true".into()),
        ("ui.visible".to_owned(), "true".into()),
    ]);

    let table = table
        .update_field_metadata()
        .with_field_name("simulation_id")
        .with_metadata(metadata)
        .await?;

    Ok(table)
}
```

`UpdateFieldMetadataBuilder` updates metadata for a named field; if a metadata key does not already exist, the key/value is inserted. It also supports commit properties and custom execution hooks. ([Docs.rs][7])

Migration policy:

```text id="muydbf"
Field metadata updates are schema-governance migrations.
Record:
  migration_id
  table version before/after
  changed field
  changed metadata keys
  reason
  compatibility assessment
```

---

## 4.11 Update table metadata

```rust id="zcnq0t"
use deltalake::operations::update_table_metadata::TableMetadataUpdate;

pub async fn update_table_name_and_description(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let update = TableMetadataUpdate {
        name: Some("simulation_outputs".to_owned()),
        description: Some("Canonical governed simulation output facts".to_owned()),
    };

    let table = table
        .update_table_metadata()
        .with_update(update)
        .await?;

    Ok(table)
}
```

`UpdateTableMetadataBuilder` applies `TableMetadataUpdate`, whose current public fields are `name` and `description`; the builder supports commit properties and custom hooks. ([Docs.rs][8])

Table metadata rule:

```text id="xmddrg"
Table name and description are contract documentation.
Do not encode deployment endpoint, credentials, secrets, or volatile runtime state.
```

---

## 4.12 Add columns

```rust id="m5tq4g"
use deltalake::kernel::{DataType, StructField};

pub async fn add_quality_columns(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .add_columns()
        .with_fields([
            StructField::nullable("quality_code", DataType::STRING),
            StructField::nullable("quality_message", DataType::STRING),
        ])
        .await?;

    Ok(table)
}
```

`AddColumnBuilder` supports adding fields, commit properties, and custom execution hooks, and awaits to an updated `DeltaTable`. ([Docs.rs][9])

Add-column policy:

```text id="59rshg"
Safe default:
  add nullable columns
  preserve existing columns
  preserve existing partitioning
  update schema contract version
  notify DataFusion/PyArrow/Spark consumers

High-risk:
  adding non-null column without default/backfill
  changing existing type
  renaming existing column
  dropping column
  reordering expected fields if consumers depend on order
```

---

## 4.13 Schema enforcement during writes

```rust id="bad0gb"
use deltalake::protocol::SaveMode;

pub async fn strict_append(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

Default write posture should be strict: incoming Arrow schema must match the table contract unless schema evolution is deliberately enabled. `WriteBuilder` exposes `with_schema_mode`, `with_cast_safety`, `with_partition_columns`, and `with_replace_where`; the write docs state default behavior raises an error when schema differs, while schema modes can intentionally merge or overwrite schema. ([Docs.rs][10])

Strict write rule:

```text id="3y10gc"
No schema mode:
  governed append
  stable table contract
  preferred production default
```

---

## 4.14 Schema evolution during writes

`SchemaMode` has two current variants: `Merge` and `Overwrite`. `Merge` appends new schema fields to the existing schema; `Overwrite` replaces the schema. ([Docs.rs][11])

### 4.14.1 Additive merge

```rust id="f7v09w"
use deltalake::operations::write::SchemaMode;
use deltalake::protocol::SaveMode;

pub async fn append_with_schema_merge(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .with_schema_mode(SchemaMode::Merge)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

Use merge for:

```text id="rx7ql7"
- additive nullable columns
- optional telemetry enrichment
- backward-compatible field expansion
- sparse non-critical attributes
```

### 4.14.2 Full schema overwrite

```rust id="vv7aib"
use deltalake::operations::write::SchemaMode;
use deltalake::protocol::SaveMode;

pub async fn overwrite_with_new_schema(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Overwrite)
        .with_schema_mode(SchemaMode::Overwrite)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

Use overwrite only for controlled migrations, derived table rebuilds, or local/dev fixture resets. The write docs state `overwrite` can completely replace schema, while overwrite with a predicate must keep existing partition columns. ([delta-io.github.io][12])

---

## 4.15 Cast safety

```rust id="qv3c4i"
pub async fn append_fail_fast_casts(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

`with_cast_safety(true)` returns `NULL` on cast failure, while `with_cast_safety(false)` returns an error. ([Docs.rs][10])

Production default:

```text id="jv4aa0"
with_cast_safety(false)
```

Use null-on-failed-cast only for raw/bronze ingestion where bad source values are expected and downstream quarantine/null policy is explicit.

---

## 4.16 Partition columns and schema

```rust id="l4m4t8"
use deltalake::protocol::SaveMode;

pub async fn create_partitioned_by_write(
    target: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = target
        .write(batches)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_partition_columns(["run_date", "scenario_family"])
        .await?;

    Ok(table)
}
```

Partition columns must be part of the schema. For existing tables, `with_partition_columns` must match current partitioning unless performing a full-table overwrite with schema overwrite and no `replaceWhere`. ([Docs.rs][10])

Partition schema policy:

```text id="hsxvps"
Partition columns:
  should be NOT NULL unless null partitions are intentionally supported
  should be low/medium cardinality
  should be stable across table lifetime
  should be validated before write
```

---

## 4.17 Decimal types

### Delta schema

```rust id="7q6f8c"
use deltalake::kernel::{DataType, StructField};

pub fn decimal_field() -> anyhow::Result<StructField> {
    Ok(StructField::nullable(
        "amount_usd",
        DataType::decimal(18, 2)?,
    ))
}
```

`DecimalType::try_new` validates decimal precision/scale; `DataType::decimal(precision, scale)` is the ergonomic constructor. ([Docs.rs][13])

### Arrow batch

```rust id="3d7abm"
use std::sync::Arc;

use arrow_array::{Decimal128Array, RecordBatch};
use arrow_schema::{DataType as ArrowDataType, Field, Schema};

pub fn decimal_arrow_batch() -> anyhow::Result<RecordBatch> {
    let values = Decimal128Array::from(vec![Some(12345_i128), Some(67890_i128)])
        .with_precision_and_scale(18, 2)?;

    let schema = Arc::new(Schema::new(vec![
        Field::new("amount_usd", ArrowDataType::Decimal128(18, 2), true),
    ]));

    Ok(RecordBatch::try_new(schema, vec![Arc::new(values)])?)
}
```

Decimal policy:

```text id="qcc6ot"
- use DECIMAL for money/exact quantities
- specify precision and scale explicitly
- test Arrow -> Delta -> Parquet -> Spark/PyArrow/DataFusion round trip
- reject implicit Float64-to-Decimal schema drift unless migration-approved
```

---

## 4.18 Timestamp types

```rust id="k58tzv"
use deltalake::kernel::{DataType, StructField};

pub fn timestamp_fields() -> Vec<StructField> {
    vec![
        StructField::not_null("created_at", DataType::TIMESTAMP),
        StructField::nullable("local_wall_clock_ts", DataType::TIMESTAMP_NTZ),
    ]
}
```

Arrow example:

```rust id="7epl5o"
use std::sync::Arc;

use arrow_array::{RecordBatch, TimestampMicrosecondArray};
use arrow_schema::{DataType as ArrowDataType, Field, Schema, TimeUnit};

pub fn timestamp_arrow_batch() -> anyhow::Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new(
            "created_at",
            ArrowDataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
    ]));

    let values = TimestampMicrosecondArray::from(vec![
        1_717_300_000_000_000_i64,
        1_717_300_060_000_000_i64,
    ]);

    Ok(RecordBatch::try_new(schema, vec![Arc::new(values)])?)
}
```

Timestamp policy:

```text id="yv57z6"
- standardize timestamp unit
- standardize timezone semantics
- prefer UTC instants for event/order/audit time
- reserve TIMESTAMP_NTZ for explicit local-wall-clock semantics
- avoid mixed timestamp units across producers
```

---

## 4.19 Binary types

```rust id="3vp0gd"
use deltalake::kernel::{DataType, StructField};

pub fn binary_fields() -> Vec<StructField> {
    vec![
        StructField::nullable("payload_bytes", DataType::BINARY),
        StructField::nullable("checksum_sha256", DataType::BINARY),
    ]
}
```

Binary policy:

```text id="iuj37q"
Use BINARY for:
  raw payloads
  hashes/checksums
  encoded artifacts
  compact opaque bytes

Avoid BINARY for:
  queryable JSON
  user-readable text
  values needing DataFusion predicate pushdown
```

---

## 4.20 Structs

```rust id="hnlhq4"
use deltalake::kernel::{DataType, StructField};

pub fn nested_struct_field() -> anyhow::Result<StructField> {
    let nested_type = DataType::try_struct_type(vec![
        StructField::nullable("temperature_c", DataType::DOUBLE),
        StructField::nullable("pressure_kpa", DataType::DOUBLE),
    ])?;

    Ok(StructField::nullable("operating_conditions", nested_type))
}
```

Struct policy:

```text id="y8gqye"
Use nested structs for:
  cohesive sub-records
  grouped parameters
  nested domain objects
  sparse grouped measurements

Avoid nested structs when:
  columns are top-level filter/group keys
  cross-engine consumers flatten everything
  schema evolution of nested fields is poorly tested
```

---

## 4.21 Arrays/lists

```rust id="0wyk2v"
use deltalake::kernel::{ArrayType, DataType, StructField};

pub fn array_field() -> StructField {
    let tags_type = DataType::from(ArrayType::new(DataType::STRING, true));
    StructField::nullable("tags", tags_type)
}
```

`ArrayType::new(element_type, contains_null)` constructs list-like Delta array types with explicit element nullability. ([Docs.rs][14])

Array policy:

```text id="v57h9i"
Use arrays for:
  repeated tags
  bounded repeated measurements
  ordered simple lists

Avoid arrays for:
  high-cardinality child records requiring joins
  frequently filtered values
  unbounded event collections
```

---

## 4.22 Maps

```rust id="b8jb7q"
use deltalake::kernel::{DataType, MapType, StructField};

pub fn map_field() -> StructField {
    let attributes_type = DataType::from(MapType::new(
        DataType::STRING,
        DataType::STRING,
        true,
    ));

    StructField::nullable("attributes", attributes_type)
}
```

`MapType::new(key_type, value_type, value_contains_null)` constructs map types with explicit value-nullability. ([Docs.rs][15])

Map policy:

```text id="sylreq"
Use maps for:
  sparse optional attributes
  semi-structured metadata
  non-critical extensibility

Avoid maps for:
  governed core fields
  columns needing constraints
  columns needing partitioning/stats/pushdown
  stable analytics dimensions
```

---

## 4.23 Variant type

```rust id="4kp6aj"
use deltalake::kernel::{DataType, StructField};

pub fn variant_field() -> StructField {
    StructField::nullable("raw_variant_payload", DataType::unshredded_variant())
}
```

`DataType` includes a `Variant` category and constructors for variant types (`DataType::unshredded_variant()` is the preferred constructor at the pinned rev). Treat variant as an advanced/newer feature: isolate it behind compatibility tests, avoid it for core governed columns, and validate DataFusion/Spark/PyArrow reading before production use. ([Docs.rs][3])

Verified status at the 1.0.0 pinned rev (PR #4325): `TableFeature::VariantType` and `TableFeature::VariantTypePreview` are in **both** the supported reader and writer feature sets of the protocol checker; the write path enforces `check_can_write_variant` — writing a schema containing variant into a table whose protocol does not declare the feature fails with `TransactionError::TableFeaturesRequired(VariantType)`; and `deltalake-core` compiles parquet with the `variant_experimental` feature. The parquet-level representation is experimental — hence the compatibility-test posture above.

Variant policy:

```text id="z5zx92"
Use variant for:
  raw flexible payloads
  ingestion landing zones
  experimental semi-structured data

Avoid variant for:
  simulation core facts
  high-value filter columns
  cross-engine-critical schema contracts
```

---

## 4.24 Arrow extension metadata

Arrow field metadata can carry extension-type annotations, semantic hints, units, display names, or application metadata. Do not assume Arrow extension metadata automatically becomes a durable, enforced Delta governance layer.

Policy:

```text id="w6xzai"
Allowed:
  semantic hints
  unit annotations
  display metadata
  extension type hints for in-process Arrow consumers

Not allowed as sole authority:
  constraints
  authorization
  data classification enforcement
  unit conversion guarantees
  schema evolution approvals
```

Golden round-trip test:

```text id="8uxwqm"
Arrow Field metadata
  -> Delta StructField metadata
  -> table creation
  -> open table
  -> Delta schema inspection
  -> DataFusion provider schema inspection
  -> Arrow batch read
  -> metadata comparison
```

If metadata is required for correctness, enforce it in your service schema contract, not merely as Arrow metadata.

---

## 4.25 Column mapping implications

Column mapping allows Delta logical column names to differ from Parquet physical column names and supports metadata-only rename/drop in engines that implement it, but it affects compatibility and can break older readers/writers. Delta documentation warns that enabling column mapping upgrades table protocol versions and may break clients that do not support the feature; it also changes physical partition directory naming behavior in some cases. ([Delta Lake][16])

Rust-specific note:

```text id="jvmpm5"
StructField exposes:
  physical_name(column_mapping_mode)
  make_physical(column_mapping_mode)
  column_mapping_id()

These support logical-to-physical schema handling.
Do not enable column mapping casually.
```

`StructField` includes helpers for physical names and column mapping metadata, including `physical_name`, `make_physical`, and `column_mapping_id`. ([Docs.rs][6])

Verified status at the 1.0.0 pinned rev: column-mapping support advanced but is deliberately **partial**, and unsupported operations are now rejected eagerly (PR #4424) instead of corrupting physical schemas:

```text
Supported at the pinned rev:
  - reads through the DataFusion provider (physical->logical projection via an
    internal ColumnMappingState; ColumnMapping is in the supported READER set)
  - plain writes to column-mapped tables (ColumnMapping newly joined the
    supported WRITER feature set — it was commented out in 0.32.x)

Rejected at the pinned rev (typed error, not a warning):
  - schema evolution on column-mapped tables: any write with a schema_mode set
    fails with "Schema evolution on column-mapped tables is not yet supported"
  - operations that would break logical/physical mapping raise
    DeltaTableError::UnsupportedColumnMapping { mode: read|write, operation },
    enforced in create, add_column, set_tbl_properties, optimize, load_cdf,
    and the low-level writer at the pinned rev
```

```text id="vtz6h9"
Before enabling:
  verify Rust deltalake reader/writer behavior
  verify DataFusion provider behavior
  verify Spark/Databricks/PyArrow behavior
  verify CDF behavior
  verify partition-directory expectations
  verify DML behavior
  document rollback limitations
```

Default recommendation for your workbench:

```text id="6ppfs0"
Use columnMapping.mode = none unless:
  metadata-only rename/drop is a hard requirement
  all engines are compatibility-certified
  column names require unsupported Parquet physical characters
```

---

## 4.26 Type widening compatibility

Delta type widening allows certain column type changes to wider types without rewriting data files in supporting engines, but it is a table feature with compatibility implications. Delta documentation describes type widening as preview in Delta Lake 3.2 and fully supported in Delta Lake 4.0, and automatic type widening during schema evolution requires type widening to be enabled and the incoming type to be a supported wider type. ([Delta Lake][17])

Governance rule:

```text id="o5f418"
Treat type widening as a protocol/table-feature migration.
Do not assume Rust DataFusion + deltalake + Spark + PyArrow all behave identically without tests.
```

Type-widening candidates:

```text id="2hdbme"
integer -> long
float -> double
decimal precision increase
date/timestamp changes only if explicitly supported by target engines
```

Migration runbook:

```text id="3uh3x1"
1. Inspect protocol/features.
2. Validate all reader/writer engines support type widening.
3. Stage table copy.
4. Apply widening in staging.
5. Write old-type and new-type data.
6. Query through Rust DataFusion.
7. Query through Spark/PyArrow if relevant.
8. Validate schema and data equality.
9. Promote only after compatibility matrix passes.
```

---

## 4.27 Cross-engine compatibility matrix

| Concern          | Rust `deltalake`                   | DataFusion              | PyArrow                 | Spark/Databricks          | Policy                 |
| ---------------- | ---------------------------------- | ----------------------- | ----------------------- | ------------------------- | ---------------------- |
| Primitive fields | strong                             | strong                  | strong                  | strong                    | standardize mappings   |
| Decimal          | test precision/scale               | test                    | test                    | test                      | exact contract         |
| Timestamp        | unit/timezone sensitive            | unit/timezone sensitive | unit/timezone sensitive | timezone semantics differ | standardize UTC        |
| Nested structs   | supported but test evolution       | query support varies    | supported               | supported                 | golden tests           |
| Arrays/maps      | supported but query semantics vary | query support varies    | supported               | supported                 | avoid for core filters |
| Variant          | new/high-risk                      | validate                | validate                | validate                  | isolate                |
| Column mapping   | compatibility-sensitive            | validate                | validate                | validate                  | avoid unless required  |
| Type widening    | compatibility-sensitive            | validate                | validate                | validate                  | migration only         |
| Metadata         | not always enforced                | not always surfaced     | not always preserved    | not always surfaced       | test explicitly        |

Schema-enforcement and schema-evolution are core Delta behaviors: schema enforcement rejects incompatible writes by default, while schema evolution intentionally permits schema changes when explicitly enabled. ([Delta Lake][18])

---

## 4.28 Inspect loaded table schema

```rust id="6eg7vn"
pub fn inspect_loaded_schema(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let state = table.snapshot()?;

    tracing::info!(
        version = state.version(),
        schema = ?state.schema(),
        metadata = ?state.metadata(),
        protocol = ?state.protocol(),
        table_config = ?state.table_config(),
        "Delta schema inspection"
    );

    Ok(())
}
```

Schema inspection should run:

```text id="99zr4p"
- at service startup
- before every governed write path
- after schema evolution
- after metadata migration
- before registering a DataFusion provider
- before cross-engine export
```

---

## 4.29 Schema contract object

```rust id="jig2w7"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaSchemaContract {
    pub table_name: String,
    pub contract_version: String,
    pub required_columns: Vec<String>,
    pub nullable_columns: Vec<String>,
    pub partition_columns: Vec<String>,
    pub required_table_properties: std::collections::HashMap<String, String>,
    pub metadata_keys_required: Vec<String>,
}
```

Contract use cases:

```text id="c6eafk"
- service startup validation
- LLM-agent codegen guardrails
- CI golden fixtures
- schema migration approval
- DataFusion query-builder validation
- UI field catalog generation
```

---

## 4.30 Schema validation helper

```rust id="vwcyjd"
pub fn require_columns(
    table: &deltalake::DeltaTable,
    required_columns: &[&str],
) -> anyhow::Result<()> {
    let schema = table.snapshot()?.schema();

    for column in required_columns {
        anyhow::ensure!(
            schema.field_with_name(column).is_ok(),
            "Delta table missing required column: {column}"
        );
    }

    Ok(())
}
```

Recommended validation checks:

```text id="g9dxvt"
- required column exists
- column data type matches
- nullability matches
- partition columns match expected order
- metadata keys present
- table properties match
- protocol features are allowed
- column mapping mode is expected
```

---

## 4.31 Golden schema fixtures

Recommended fixture set:

```text id="009ir8"
schema.delta.json:
  serialized Delta StructType

schema.arrow.json:
  serialized Arrow Schema

table_metadata.json:
  expected name, description, partition columns, properties

protocol.json:
  expected min reader/writer versions and features

sample_valid.arrow.ipc:
  valid RecordBatch fixture

sample_invalid.arrow.ipc:
  invalid null/type/range fixture

roundtrip.parquet:
  expected Parquet physical compatibility fixture
```

Test matrix:

```text id="5etj3u"
Delta schema:
  construct StructType
  serialize/deserialize
  duplicate-case field rejection
  nested type construction
  decimal validation

Arrow mapping:
  Delta -> Arrow
  Arrow -> Delta
  RecordBatch -> write -> read -> RecordBatch
  metadata preservation

Writes:
  strict schema accepts exact batch
  strict schema rejects missing/extra/type-drift batch
  SchemaMode::Merge adds nullable columns
  SchemaMode::Overwrite replaces schema only in controlled path
  cast_safety=false rejects bad casts
  cast_safety=true nulls failed casts only where allowed

Cross-engine:
  Spark/PyArrow read table
  Rust DataFusion read table
  Decimal/timestamp equality
  nested array/map round trip
  column mapping disabled/enabled behavior if used
```

---

## 4.32 Production schema governance runbook

```text id="om978d"
For new table:
  1. Define Delta StructType.
  2. Define Arrow Schema fixture.
  3. Define table properties.
  4. Define partition columns.
  5. Define constraints.
  6. Create table.
  7. Open table.
  8. Validate schema/protocol/metadata.
  9. Register DataFusion provider.
  10. Write valid fixture.
  11. Verify invalid fixture rejected.

For additive evolution:
  1. Add nullable fields.
  2. Use add_columns or SchemaMode::Merge with approval.
  3. Validate downstream readers.
  4. Update contract version.
  5. Update UI/query catalogs.

For breaking evolution:
  1. Create new table/location.
  2. Backfill.
  3. Validate cross-engine.
  4. Switch catalog/pointer.
  5. Retire old table after retention.
```

---

## 4.33 Best practices

```text id="003syz"
Schema:
  - use StructType::try_new
  - centralize schema constructors
  - use stable lowercase snake_case column names
  - specify nullability explicitly
  - use Decimal for exact values
  - standardize timestamp policy
  - keep core analytics fields top-level

Arrow:
  - pin Arrow 58 (resolved 58.3.0) with the deltalake 1.0.0 pinned rev
  - validate batch schemas before write
  - avoid duplicate Arrow versions
  - test nested/decimal/timestamp round trips

Evolution:
  - default strict
  - use SchemaMode::Merge only for approved additive nullable fields
  - use SchemaMode::Overwrite only for controlled rebuild/migration
  - use add_columns for explicit schema migrations
  - update contract version with every schema change

Metadata:
  - use field metadata for semantic hints
  - update table metadata through migration-controlled operations
  - never store secrets in metadata
  - test metadata preservation if required for behavior

Compatibility:
  - avoid column mapping unless required
  - treat type widening as compatibility-sensitive
  - test Spark/PyArrow/DataFusion on every advanced feature
```

---

## 4.34 Anti-patterns

```text id="tb1w86"
- using StructType::new as canonical unchecked production construction
- making every field nullable
- schema inference as production contract
- using Float64 for money/exact quantities
- mixing timestamp units/timezone assumptions
- hiding governed fields inside map/variant payloads
- using SchemaMode::Merge as default ingestion convenience
- using SchemaMode::Overwrite for routine appends
- enabling column mapping without engine compatibility tests
- assuming Arrow extension metadata is enforced by Delta
- allowing duplicate Arrow/DataFusion crate versions
- changing partition columns during normal writes
```

---

## 4.35 LLM-agent checklist

```text id="gmnkkk"
Before generating schema code:
  1. Use the deltalake 1.0.0 pinned-rev baseline.
  2. Use StructType::try_new.
  3. Use StructField::not_null / nullable deliberately.
  4. Use DataType constants for primitive fields.
  5. Use DataType::decimal for exact fixed-precision values.
  6. Use ArrayType/MapType/StructType for nested fields only when justified.
  7. Avoid Variant for governed core columns.
  8. Define Arrow Schema separately for RecordBatch construction.
  9. Validate RecordBatch schemas before write.
  10. Default to strict schema enforcement.
  11. Use SchemaMode::Merge only for approved additive evolution.
  12. Use SchemaMode::Overwrite only for controlled full migration/rebuild.
  13. Use with_cast_safety(false) for governed writes.
  14. Use add_columns for explicit schema migrations.
  15. Use update_field_metadata/update_table_metadata for metadata migrations.
  16. Inspect schema/protocol/table_config after migration.
  17. Run Arrow/Delta/DataFusion/Spark/PyArrow golden round trips for advanced types.
```

---

## 4.36 Value case

Precise schema governance gives your Rust DataFusion/Arrow simulation platform:

```text id="tqcd54"
- deterministic Arrow batch write boundaries
- DataFusion query-planning stability
- cross-engine Delta compatibility
- explicit schema evolution control
- metadata-backed semantic discoverability
- safer LLM-generated code
- faster debugging of type/nullability errors
- reproducible simulation output contracts
- enforceable table design before data lands
```

Core invariant:

```text id="1h0kiz"
The Delta schema is the authoritative contract; Arrow schemas must conform to it, and schema evolution must be an explicit migration.
```

Production invariant:

```text id="p4uqna"
Every table should have a versioned schema contract: Delta StructType, Arrow Schema fixture, partition columns, field metadata policy, table properties, compatibility matrix, and golden round-trip tests.
```



## 4.37 Nested-schema physical/nullability interoperability hardening (latest pin)

The latest pin adds Delta-aware DataFusion schema adaptation for a real Spark interoperability edge case: Spark may write nested Parquet fields as physically nullable even when the Delta logical schema declares the nested field non-nullable. The Delta scan path now relaxes **nested** physical-read nullability for structs, list variants, maps, dictionaries, and fixed-size lists, delegates expression adaptation to DataFusion, and then restores/validates the strict logical Delta result schema above the Parquet scan. Top-level required-column behavior remains strict, and map entries/keys retain Arrow's non-nullability invariants.

A second fix at the current tip prevents a nested field whose **name happens to equal a top-level partition column** from being treated as a partition-field candidate while mapping physical types. This fixes an `OPTIMIZE`/compaction failure mode where a nested string could otherwise be dictionary-encoded as though it were a partition column.

For schema-governed systems, add two golden tests:

```text
1. Delta nested field non-nullable; physical Parquet nested field optional -> read/OPTIMIZE succeeds, logical schema stays strict.
2. Top-level partition column `date` plus nested `properties.date` -> read/OPTIMIZE preserves the nested field's ordinary data type and values.
```

These are compatibility fixes, not permission to weaken the declared Delta schema. Keep the Delta logical nullability contract authoritative.

[1]: https://docs.rs/delta_kernel/latest/delta_kernel/schema/struct.StructType.html "StructType in delta_kernel::schema - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/table/state/struct.DeltaTableState.html "DeltaTableState in deltalake::table::state - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/enum.DataType.html "DataType in deltalake - Rust"
[4]: https://delta-io.github.io/delta-rs/usage/create-delta-lake-table/ "Creating a table - Delta Lake Documentation"
[5]: https://docs.rs/deltalake/latest/deltalake/kernel/index.html "deltalake::kernel - Rust"
[6]: https://docs.rs/deltalake/latest/deltalake/struct.StructField.html "StructField in deltalake - Rust"
[7]: https://docs.rs/deltalake/latest/deltalake/operations/update_field_metadata/struct.UpdateFieldMetadataBuilder.html "UpdateFieldMetadataBuilder in deltalake::operations::update_field_metadata - Rust"
[8]: https://docs.rs/deltalake/latest/deltalake/operations/update_table_metadata/struct.UpdateTableMetadataBuilder.html "UpdateTableMetadataBuilder in deltalake::operations::update_table_metadata - Rust"
[9]: https://docs.rs/deltalake/latest/deltalake/operations/add_column/struct.AddColumnBuilder.html "AddColumnBuilder in deltalake::operations::add_column - Rust"
[10]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html "WriteBuilder in deltalake::operations::write - Rust"
[11]: https://docs.rs/deltalake/latest/deltalake/operations/write/enum.SchemaMode.html "SchemaMode in deltalake::operations::write - Rust"
[12]: https://delta-io.github.io/delta-rs/usage/writing/ "Writing Delta Tables - Delta Lake Documentation"
[13]: https://docs.rs/deltalake/latest/deltalake/struct.DecimalType.html "DecimalType in deltalake - Rust"
[14]: https://docs.rs/deltalake/latest/deltalake/struct.ArrayType.html "ArrayType in deltalake - Rust"
[15]: https://docs.rs/deltalake/latest/deltalake/struct.MapType.html "MapType in deltalake - Rust"
[16]: https://docs.delta.io/delta-column-mapping/?utm_source=chatgpt.com "Delta column mapping"
[17]: https://docs.delta.io/delta-type-widening/?utm_source=chatgpt.com "Delta type widening"
[18]: https://delta.io/blog/2022-11-16-delta-lake-schema-enforcement/?utm_source=chatgpt.com "Delta Lake Schema Enforcement"


Proceeding with **topic 5**. I am treating the newer write-path request as superseding the prior schema-section request.

# 5. Writing data from Arrow and DataFusion — Rust `deltalake`

Version target:

```text id="rz2vp8"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Tokio: 1
Required deltalake features for this section:
  ["datafusion"]
Typical cloud profile:
  ["rustls", "datafusion", "s3"]
```

Important current-version correction: the user-provided anchor using `DeltaOps(table).write(...)` is now legacy style. In `deltalake` 1.0.0 @ the pinned rev, `DeltaOps` is marked deprecated, and the docs direct users to methods on `DeltaTable`, for example `delta_table.create()` rather than `DeltaOps(...).create()`. `DeltaOps::write` is also deprecated in favor of `DeltaTable::write`. Also, current `WriteBuilder` awaits to `Result<DeltaTable, DeltaTableError>`, not `(DeltaTable, metrics)`. ([Docs.rs][1])

---

## 5.1 Write-path mental model

A Rust `deltalake` write is a **DataFusion/Arrow-to-Parquet-to-Delta-log transaction pipeline**:

```text id="2rzck3"
input:
  Arrow RecordBatch iterator
  or DataFusion LogicalPlan

execution:
  optional DataFusion planning/execution
  Arrow batch stream
  Parquet file writer
  Delta Add actions
  optional Remove actions for overwrite / replaceWhere
  optimistic transaction commit

output:
  new DeltaTable handle loaded/advanced to the committed table state
```

`WriteBuilder` is the main Rust builder for this path. It supports `with_save_mode`, `with_schema_mode`, `with_replace_where`, `with_partition_columns`, `with_input_plan`, `with_session_state`, `with_session_fallback_policy`, `with_target_file_size`, `with_write_batch_size`, `with_cast_safety`, `with_writer_properties`, `with_commit_properties`, `with_custom_execute_handler`, `with_configuration`, and `with_input_batches`. ([Docs.rs][2])

Canonical application posture:

```text id="v0s7gk"
Arrow-native write:
  DeltaTable::write(Vec<RecordBatch>)

DataFusion-native write:
  DeltaTable::write(Vec::<RecordBatch>::new())
    .with_input_plan(LogicalPlan)
    .with_session_state(Arc<SessionState>)

Avoid:
  DeltaOps(table).write(...)
  deprecated with_input_execution_plan(...)
  raw Parquet writes into a Delta table directory
```

---

## 5.2 Cargo profile

```toml id="de6vv1"
[package]
edition = "2024"
rust-version = "1.94.1"

[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
serde_json = "1"
```

The `deltalake` write path is Arrow-centered, and the `datafusion` feature is required for DataFusion planning/expression integration. The `deltalake` crate also re-exports DataFusion-related modules, while DataFusion itself uses Arrow as its in-memory format. ([Docs.rs][3])

---

## 5.3 Current API surface: use `DeltaTable::write`

### 5.3.1 Minimal append from Arrow `RecordBatch`

```rust id="s5zrsg"
use std::sync::Arc;

use arrow_array::{Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use deltalake::protocol::SaveMode;
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn append_arrow_batch(path: &str) -> anyhow::Result<DeltaTable> {
    let table_url = Url::from_directory_path(path)
        .map_err(|_| anyhow::anyhow!("invalid local Delta table path: {path}"))?;

    let table = open_table(table_url).await?;

    let schema = Arc::new(Schema::new(vec![
        Field::new("simulation_id", DataType::Utf8, false),
        Field::new("run_id", DataType::Utf8, false),
        Field::new("value", DataType::Int64, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["sim-a", "sim-a"])),
            Arc::new(StringArray::from(vec!["run-001", "run-002"])),
            Arc::new(Int64Array::from(vec![Some(10), None])),
        ],
    )?;

    let table = table
        .write(vec![batch])
        .with_save_mode(SaveMode::Append)
        .await?;

    Ok(table)
}
```

`DeltaTable::write` accepts an `IntoIterator<Item = RecordBatch>` and returns a `WriteBuilder`; append and overwrite examples in the delta-rs docs use `table.write(...).with_save_mode(SaveMode::Append/Overwrite).await?`. ([Docs.rs][4])

### 5.3.2 No metrics tuple in `1.0.0`

```rust id="gyzaic"
// Current 1.0.0 shape:
let table: deltalake::DeltaTable = table
    .write(vec![batch])
    .with_save_mode(deltalake::protocol::SaveMode::Append)
    .await?;

// Not current 1.0.0 shape:
// let (table, metrics) = DeltaOps(table).write(vec![batch]).await?;
```

The `WriteBuilder` `IntoFuture` implementation documents its output as `Result<DeltaTable, DeltaTableError>`, so write metrics are not returned as a tuple by the current Rust builder. ([Docs.rs][2])

---

## 5.4 Save modes

`SaveMode` at the 1.0.0 pinned rev has four variants (unchanged from 0.32.x): `Append`, `Overwrite`, `ErrorIfExists`, and `Ignore`. The source defines append as adding files, overwrite as overwriting the target, error-if-exists as failing when files exist for the target, and ignore as not proceeding or changing data when files exist. ([GitHub][5])

```rust id="qho6lo"
use deltalake::protocol::SaveMode;

SaveMode::Append;
SaveMode::Overwrite;
SaveMode::ErrorIfExists;
SaveMode::Ignore;
```

### 5.4.1 Append

```rust id="4yjv7c"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .await?;
```

Use append for:

```text id="z8lbo5"
event logs
simulation output facts
immutable run records
time-series ingest
append-only audit streams
batch materializations where duplicate prevention is handled upstream
```

Append anti-patterns:

```text id="zrjtex"
append without idempotency key
append one tiny batch per source row/file
append into tables needing key-level replacement
append after upstream schema drift without explicit schema_mode
append with unbounded concurrent single-file writers
```

### 5.4.2 Full-table overwrite

```rust id="ke8x8w"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Overwrite)
    .await?;
```

Delta overwrite is transactional and logical: previous data files are removed from the active table state, but old data files are not physically deleted immediately; the docs demonstrate time travel back to an earlier version after overwrite. ([Delta IO][6])

Use full overwrite for:

```text id="t91uae"
small dimension replacement
complete snapshot materialization
derived table rebuild
test fixture reset
deterministic full-output simulation artifact
```

Avoid full overwrite for:

```text id="bfe3su"
large fact tables
hot serving tables
partial partition replacement
user-level correction
concurrent-write-heavy tables
tables with long time-travel retention constraints
```

### 5.4.3 Error-if-exists

```rust id="h1g1l2"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::ErrorIfExists)
    .await?;
```

Use `ErrorIfExists` for:

```text id="zac7ky"
create-once output artifacts
immutable table initialization
pipeline stage output uniqueness
CI tests expecting empty target
single-run materialization directories
```

### 5.4.4 Ignore

```rust id="j10bl3"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Ignore)
    .await?;
```

Use `Ignore` sparingly:

```text id="1zeevj"
idempotent create-if-absent bootstrap
best-effort seed tables
local/dev fixture setup
```

Avoid `Ignore` when the caller must know whether the intended data was written.

---

## 5.5 Schema modes

`SchemaMode` has two variants at the 1.0.0 pinned rev (unchanged from 0.32.x): `Overwrite` and `Merge`. `Overwrite` replaces the schema with the new schema; `Merge` appends the new schema to the existing schema. ([Docs.rs][7])

```rust id="mgk5ms"
use deltalake::operations::write::SchemaMode;

SchemaMode::Merge;
SchemaMode::Overwrite;
```

### 5.5.1 Strict default

```rust id="qxhbuq"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .await?;
```

Default posture: incoming Arrow schema must match the table schema according to the writer’s validation/coercion path. For production, treat absence of `with_schema_mode(...)` as **strict schema governance**.

Use strict mode for:

```text id="ocdz0t"
stable simulation result tables
financial outputs
contracted downstream tables
DataFusion query surfaces with compiled schemas
tables shared with Spark/PyArrow/Databricks
```

### 5.5.2 Merge schema

```rust id="u6udxl"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_schema_mode(SchemaMode::Merge)
    .await?;
```

Use `SchemaMode::Merge` for:

```text id="e9gztw"
new nullable columns
progressive simulation telemetry enrichment
sparse optional attributes
backward-compatible source evolution
append-mode schema expansion
```

Guardrails:

```text id="n945cs"
- require explicit migration approval
- require all new columns nullable or default-compatible
- run golden schema diff
- emit schema-version commit metadata
- notify DataFusion consumers before query compilation
```

### 5.5.3 Overwrite schema

```rust id="4serlr"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Overwrite)
    .with_schema_mode(SchemaMode::Overwrite)
    .await?;
```

Use `SchemaMode::Overwrite` only for:

```text id="aieyt3"
full table replacement
test fixture rebuild
controlled schema migration
derived table contract replacement
partition-layout replacement under full-overwrite constraints
```

Partition caveat: for existing tables, `with_partition_columns(...)` must match current partitioning unless doing a full table overwrite with schema overwrite and no `replaceWhere` predicate; only that full-overwrite path may replace partitioning. ([Docs.rs][2])

---

## 5.6 Cast safety

```rust id="kyfp19"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_cast_safety(false)
    .await?;
```

`with_cast_safety(safe)` controls how cast failures are handled: `safe = true` returns `NULL` on cast failure, while `safe = false` returns an error. ([Docs.rs][2])

Recommended production default:

```text id="kwt8eh"
with_cast_safety(false)
```

Use fail-fast casting for:

```text id="i4p22s"
simulation numeric outputs
schema-controlled data marts
financial/engineering calculations
foreign-key-like identifiers
partition columns
timestamps
decimals
```

Use null-on-failed-cast only for:

```text id="h69hch"
bronze/raw ingest
quarantine-first ingestion
non-critical exploratory telemetry
explicit missingness-tolerant fields
```

---

## 5.7 Partitioned writes

### New partitioned table or matching existing partitioning

```rust id="qr1axb"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_partition_columns(["simulation_id", "run_date"])
    .await?;
```

Partitioning is applied to new tables; for existing tables, the specified partition columns must match the existing table partitioning except for the narrow full-overwrite + schema-overwrite + no-`replaceWhere` case. ([Docs.rs][2])

Partitioning guidance for simulation outputs:

```text id="2a4uu8"
good candidates:
  run_date
  scenario_family
  model_version
  simulation_domain
  tenant_id, if isolation and query pruning align

bad candidates:
  unique run_id if most queries span many runs
  timestamp at second/millisecond precision
  high-cardinality continuous variable
  user-entered free text
  floating-point parameter value
```

Operational invariant:

```text id="bw6hv5"
partition columns are physical layout, not merely logical metadata.
Changing them is table-layout migration, not a routine write option.
```

---

## 5.8 `replaceWhere` / predicate overwrite

### Rust expression form

```rust id="wwj6t8"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::protocol::SaveMode;

let table = table
    .write(batches)
    .with_save_mode(SaveMode::Overwrite)
    .with_replace_where(col("run_date").eq(lit("2026-06-02")))
    .await?;
```

Without a predicate, overwrite replaces the entire table. With `replaceWhere`, overwrite replaces only records or partitions matching the predicate; delta-rs docs state that input data must conform to the predicate, otherwise the operation fails. ([Delta IO][8])

Use `replaceWhere` for:

```text id="a4n3np"
single partition refresh
one simulation date rerun
one scenario-family rebuild
tenant-scoped correction
bounded materialized view refresh
late-arriving data correction by partition
```

Do not use `replaceWhere` for:

```text id="y1n8et"
arbitrary upserts by primary key
multi-key deduplication
slowly changing dimensions
row-level reconciliation
complex source-target matching
```

Use `merge` for key-based row-level upsert semantics; use `replaceWhere` for bounded predicate replacement.

Predicate-conformance guard:

```rust id="diqo9y"
pub fn require_single_run_date(
    batch: &arrow_array::RecordBatch,
    expected: &str,
) -> anyhow::Result<()> {
    let idx = batch
        .schema()
        .index_of("run_date")
        .map_err(|_| anyhow::anyhow!("missing run_date column"))?;

    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<arrow_array::StringArray>()
        .ok_or_else(|| anyhow::anyhow!("run_date must be Utf8/StringArray"))?;

    for row in 0..arr.len() {
        if arr.is_null(row) || arr.value(row) != expected {
            anyhow::bail!(
                "replaceWhere predicate violation: row {row} has run_date={:?}, expected={expected}",
                if arr.is_null(row) { None } else { Some(arr.value(row)) }
            );
        }
    }

    Ok(())
}
```

---

## 5.9 Writing from DataFusion `LogicalPlan`

### 5.9.1 SQL/DataFrame-to-Delta write

```rust id="7g80jy"
use std::sync::Arc;

use arrow_array::RecordBatch;
use datafusion::prelude::SessionContext;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::protocol::SaveMode;
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn write_query_result_to_delta(
    ctx: &SessionContext,
    target_uri: &str,
) -> anyhow::Result<DeltaTable> {
    let target = open_table(Url::parse(target_uri)?).await?;

    let df = ctx
        .sql(
            r#"
            SELECT
              simulation_id,
              run_date,
              unit_id,
              SUM(output_value) AS output_value
            FROM staging_simulation_outputs
            GROUP BY simulation_id, run_date, unit_id
            "#,
        )
        .await?;

    let (session_state, logical_plan) = df.into_parts();

    let table = target
        .write(Vec::<RecordBatch>::new())
        .with_input_plan(logical_plan)
        .with_session_state(Arc::new(session_state))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_save_mode(SaveMode::Append)
        .await?;

    Ok(table)
}
```

`with_input_plan` accepts a DataFusion `LogicalPlan` that produces data to be written, and `with_session_state` sets the DataFusion session used for planning/execution. The provided session should wrap a concrete `SessionState`; if it does not, the default is to warn and fall back to internal defaults unless `SessionFallbackPolicy::RequireSessionState` is selected. ([Docs.rs][2])

### 5.9.2 Why `DataFrame::into_parts` instead of `into_unoptimized_plan`

```rust id="6glkfr"
let (session_state, logical_plan) = df.into_parts();
```

DataFusion documents `DataFrame::into_parts()` as returning both the `SessionState` and `LogicalPlan`; it also warns that `into_unoptimized_plan()` loses the session-state snapshot and should not be used outside testing. ([Docs.rs][9])

Agent rule:

```text id="kqqh5b"
For production DataFusion writes:
  use df.into_parts()
  pass logical_plan to with_input_plan(...)
  pass Arc::new(session_state) to with_session_state(...)
  set RequireSessionState unless fallback behavior is intentionally accepted
```

---

## 5.10 Session fallback policy

```rust id="6d1d7z"
use deltalake::operations::write::SessionFallbackPolicy;

let table = table
    .write(Vec::<arrow_array::RecordBatch>::new())
    .with_input_plan(logical_plan)
    .with_session_state(std::sync::Arc::new(session_state))
    .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
    .await?;
```

Default fallback exists for compatibility, but production services should prefer `RequireSessionState` because it prevents silent loss of custom DataFusion runtime configuration, UDF registration, object-store registry state, optimizer config, execution config, catalog bindings, or tenant-specific state. `WriteBuilder` docs state that fallback defaults to `InternalDefaults` and that strict behavior is obtained with `RequireSessionState`. ([Docs.rs][2])

Production consequences of accidental fallback:

```text id="c4hf4p"
- missing UDFs
- missing object-store mappings
- different execution config
- wrong catalog/table resolution
- tenant isolation failure
- inconsistent now()/random/session variables
- unexpected optimizer behavior
```

---

## 5.11 Target file size and row-group/write batch sizing

### Target data-file size

```rust id="i11gc7"
use std::num::NonZeroU64;

let target_file_size = NonZeroU64::new(256 * 1024 * 1024);

let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_target_file_size(target_file_size)
    .await?;
```

`with_target_file_size` accepts `Option<NonZero<u64>>` and specifies the target file size for data files written to the Delta table. ([Docs.rs][2])

Recommended starting points:

```text id="zzjq9k"
interactive/small tables:
  64 MiB - 128 MiB target files

analytics fact tables:
  128 MiB - 512 MiB target files

large sequential scans:
  256 MiB - 1 GiB target files, benchmarked

high-concurrency small writes:
  buffer upstream; do not emit one tiny Delta file per source object
```

### Row-group/write batch size

```rust id="fc7pap"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_write_batch_size(8192)
    .await?;
```

`with_write_batch_size` specifies the target batch size for row groups written to Parquet files. ([Docs.rs][2])

Heuristic:

```text id="ka9ebg"
increase when:
  wide tables are stable
  large scans dominate
  compression efficiency matters

decrease when:
  memory pressure is high
  batches are extremely wide
  low-latency smaller writes dominate

always:
  benchmark with actual schema width, cardinality, compression, and query filters
```

---

## 5.12 Parquet writer properties

```rust id="vqxrun"
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

let writer_properties = WriterProperties::builder()
    .set_compression(Compression::SNAPPY)
    .set_write_batch_size(8192)
    .set_max_row_group_size(128 * 1024)
    .build();

let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_writer_properties(writer_properties)
    .await?;
```

`WriteBuilder::with_writer_properties` specifies the Parquet writer properties used when writing data files. The Python writer docs expose the same conceptual Rust Parquet writer properties surface, including page size, dictionary page size, data-page row-count limit, write batch size, max row-group size, compression, compression level, statistics truncation, and per-column properties. ([Docs.rs][2])

Recommended property posture:

```text id="pvoqm4"
default:
  use defaults until benchmark identifies a bottleneck

compression:
  SNAPPY for broad compatibility and fast CPU
  ZSTD for higher compression when CPU budget allows
  avoid exotic codecs for cross-engine compatibility unless validated

row groups:
  tune against query predicate selectivity
  avoid tiny row groups on large scans
  avoid huge row groups on memory-constrained services

statistics:
  preserve useful stats for skipping
  be cautious truncating min/max stats for high-value filter columns
```

---

## 5.13 Commit properties / commit metadata

```rust id="6ylxng"
use deltalake::kernel::transaction::CommitProperties;

let commit_properties = CommitProperties::default();

let table = table
    .write(batches)
    .with_save_mode(SaveMode::Append)
    .with_commit_properties(commit_properties)
    .await?;
```

`with_commit_properties` adds additional metadata to commit info. ([Docs.rs][2])

Production metadata keys to standardize:

```text id="xm2fho"
application_id
application_version
pipeline_name
job_id
simulation_id
source_table_versions
input_snapshot_pin
schema_contract_version
tenant_id_hash
request_id
trace_id
git_sha
build_id
```

Governance rule:

```text id="cggrw2"
Commit metadata is audit/provenance data, not table schema.
Do not rely on it as the only durable queryable business index.
Do not log secrets into commit metadata.
```

---

## 5.14 Custom execute handler

```rust id="dc2hn3"
let table = table
    .write(batches)
    .with_custom_execute_handler(handler)
    .await?;
```

`with_custom_execute_handler` sets a custom execution handler for pre- and post-execution hooks. ([Docs.rs][2])

Use cases:

```text id="0r4dwb"
instrument write execution
enforce service-level guardrails
trace pre/post write timings
inject diagnostics
wrap DataFusion execution behavior
collect custom write telemetry
```

Do not use custom handlers for:

```text id="nrcpz8"
business validation better handled before write
credential injection
schema evolution approval
retry loops around non-idempotent writes
manual mutation of Delta log internals
```

---

## 5.15 Created-table configuration during write

```rust id="dk776p"
let table = table
    .write(batches)
    .with_save_mode(SaveMode::ErrorIfExists)
    .with_table_name("simulation_outputs")
    .with_description("Canonical simulation output facts")
    .with_configuration([
        ("delta.enableChangeDataFeed", Some("true")),
        ("delta.appendOnly", Some("true")),
    ])
    .await?;
```

`WriteBuilder` supports `with_table_name`, `with_description`, and `with_configuration` for table metadata/configuration on created tables. ([Docs.rs][2])

Recommended table-property policy:

```text id="wk5zlo"
set at creation:
  CDF expectation
  append-only expectation
  retention policy
  data-skipping policy
  target file size where table-level property is preferred

do not set ad hoc:
  partition columns
  schema-affecting properties
  protocol/table features
```

---

## 5.16 Idempotent write patterns

### 5.16.1 Append with deterministic dedupe key

```text id="1smdhu"
required columns:
  job_id
  source_file_id
  source_file_offset
  simulation_id
  run_id
  generated_at
```

Pattern:

```text id="fgbrj8"
1. Compute deterministic ingestion key.
2. Append rows with ingestion key.
3. Deduplicate downstream by key or enforce uniqueness in derived materialization.
4. On retry, either skip if key already committed or use merge/replaceWhere instead of blind append.
```

Use for high-volume logs where duplicate cleanup is acceptable.

### 5.16.2 `replaceWhere` by deterministic partition

```rust id="ttny9l"
let run_date = "2026-06-02";

let table = table
    .write(batches)
    .with_save_mode(SaveMode::Overwrite)
    .with_replace_where(col("run_date").eq(lit(run_date)))
    .await?;
```

Pattern:

```text id="bhnsg1"
1. Materialize exactly one bounded partition/slice.
2. Validate every row matches predicate.
3. Overwrite only that slice.
4. Retry safely because same predicate replacement produces same active slice.
```

Best for simulation reruns where a complete slice can be regenerated.

### 5.16.3 Full overwrite with versioned output path

```text id="zxfu63"
s3://bucket/simulation_outputs/model_version=v17/build_id=...
```

Pattern:

```text id="8p61pr"
1. Write to versioned table/path.
2. Use ErrorIfExists for immutability.
3. Publish pointer/catalog entry after successful commit.
4. Never overwrite canonical path directly in hot path.
```

Best for audit-critical model outputs.

---

## 5.17 Retry safety and atomic commit behavior

Delta write behavior is transaction-log mediated: append normally adds active files; overwrite adds new files and logically removes old files. The append/overwrite docs demonstrate that overwrite does not physically remove old data immediately, preserving time-travel access until retention/vacuum removes files. ([Delta IO][6])

Retry taxonomy:

```text id="z28ui8"
safe to retry automatically:
  transient object-store GET/LIST/PUT failures before commit
  DataFusion execution failure before files are committed
  lock acquisition retry when no data commit occurred
  create-if-absent with ErrorIfExists when existence is accepted as success by caller

not blindly safe:
  append after unknown commit result
  append with no idempotency key
  overwrite after partial business validation
  schema evolution writes
  concurrent writes to same replaceWhere predicate

requires reconciliation:
  object-store timeout after commit attempt
  version mismatch after commit
  transaction conflict
  commit succeeded but local table state failed to update
```

Production retry algorithm:

```text id="r1yl65"
1. Assign operation_id.
2. Add operation_id to commit properties where possible.
3. Write data.
4. On error after possible commit:
   a. reload table
   b. inspect latest history/version
   c. detect operation_id or deterministic output slice
   d. mark success if already committed
   e. otherwise retry with backoff only if operation is idempotent
```

S3 note: safe concurrent writes on AWS S3 require explicit locking configuration; otherwise use single-writer mode or only controlled unsafe rename in tests/single-writer environments.

---

## 5.18 Small-file avoidance

Bad pattern:

```text id="w879fs"
for each input object:
  parse object
  write one tiny RecordBatch
  commit one Delta version
```

Likely result:

```text id="grge36"
many tiny Parquet files
many Delta log JSON files
high object-store list/get overhead
slow table load
slow writer conflict resolution
cost amplification
expensive optimize/vacuum burden
```

Better pattern:

```text id="r3ltx4"
for each micro-batch:
  collect N source objects or M rows or T seconds
  coalesce into Arrow batches
  write target-sized Parquet files
  commit one Delta transaction
```

Practical policy:

```text id="8ymkz0"
minimum rows per write:
  table-specific; benchmark

minimum bytes per write:
  prefer tens/hundreds of MiB logical input where feasible

target files:
  128-512 MiB for analytical fact tables

maintenance:
  schedule optimize/compaction for streaming-like ingestion tables
```

---

## 5.19 DataFusion runtime/object-store integration before write

When writing from a DataFusion logical plan that scans Delta/object-store tables, ensure the source and target object stores are registered in the same DataFusion runtime. `DeltaTable::update_datafusion_session` registers the table root object store if missing and is idempotent, but it does not overwrite stale mappings. ([Docs.rs][4])

```rust id="b2vqxl"
pub async fn prepare_delta_for_datafusion_write(
    ctx: &datafusion::prelude::SessionContext,
    source_tables: &[deltalake::DeltaTable],
) -> anyhow::Result<()> {
    let state = ctx.state();

    for table in source_tables {
        table.update_datafusion_session(&state)?;
    }

    Ok(())
}
```

Runtime rule:

```text id="pcoi20"
All DataFusion source scans used by with_input_plan must resolve in the SessionState passed to with_session_state.
```

---

## 5.20 End-to-end production write function

```rust id="b1xgxv"
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arrow_array::RecordBatch;
use deltalake::operations::write::SchemaMode;
use deltalake::protocol::SaveMode;
use deltalake::{open_table_with_storage_options, DeltaTable};
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use url::Url;

#[derive(Debug, Clone)]
pub struct DeltaAppendConfig {
    pub table_uri: String,
    pub storage_options: HashMap<String, String>,
    pub partition_columns: Vec<String>,
    pub target_file_size_bytes: Option<u64>,
    pub write_batch_size: usize,
    pub allow_schema_merge: bool,
}

pub async fn append_simulation_batches(
    cfg: DeltaAppendConfig,
    batches: Vec<RecordBatch>,
) -> anyhow::Result<DeltaTable> {
    anyhow::ensure!(!batches.is_empty(), "refusing to write zero RecordBatches");

    let table_url = Url::parse(&cfg.table_uri)?;
    let table = open_table_with_storage_options(table_url, cfg.storage_options).await?;

    let writer_properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_write_batch_size(cfg.write_batch_size)
        .build();

    let mut writer = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .with_partition_columns(cfg.partition_columns)
        .with_write_batch_size(cfg.write_batch_size)
        .with_target_file_size(cfg.target_file_size_bytes.and_then(NonZeroU64::new))
        .with_writer_properties(writer_properties)
        .with_cast_safety(false);

    if cfg.allow_schema_merge {
        writer = writer.with_schema_mode(SchemaMode::Merge);
    }

    let table = writer.await?;

    Ok(table)
}
```

---

## 5.21 End-to-end DataFusion plan write function

```rust id="a949qm"
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arrow_array::RecordBatch;
use datafusion::prelude::SessionContext;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::protocol::SaveMode;
use deltalake::{open_table_with_storage_options, DeltaTable};
use url::Url;

pub async fn write_datafusion_query_to_delta(
    ctx: &SessionContext,
    target_uri: &str,
    storage_options: HashMap<String, String>,
    sql: &str,
    target_file_size_bytes: Option<u64>,
) -> anyhow::Result<DeltaTable> {
    let target = open_table_with_storage_options(
        Url::parse(target_uri)?,
        storage_options,
    )
    .await?;

    let df = ctx.sql(sql).await?;
    let (session_state, logical_plan) = df.into_parts();

    let table = target
        .write(Vec::<RecordBatch>::new())
        .with_input_plan(logical_plan)
        .with_session_state(Arc::new(session_state))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_save_mode(SaveMode::Append)
        .with_target_file_size(target_file_size_bytes.and_then(NonZeroU64::new))
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

Use this for:

```text id="f4lmsu"
SQL-defined simulation materializations
DataFusion DataFrame transformations
derived Delta table writes
query-result persistence
agent-generated calculation pipelines
```

---

## 5.22 Validation before write

```rust id="1eanxy"
pub fn validate_batches_for_delta_write(
    batches: &[RecordBatch],
) -> anyhow::Result<arrow_schema::SchemaRef> {
    anyhow::ensure!(!batches.is_empty(), "no batches to write");

    let schema = batches[0].schema();

    for (idx, batch) in batches.iter().enumerate() {
        anyhow::ensure!(
            batch.num_rows() > 0,
            "batch {idx} has zero rows; compact or filter before write"
        );

        anyhow::ensure!(
            batch.schema().as_ref() == schema.as_ref(),
            "batch {idx} schema differs from first batch schema"
        );
    }

    Ok(schema)
}
```

Pre-write validation checklist:

```text id="kl6epy"
schema:
  exact field names
  exact Arrow data types
  nullability policy
  decimal precision/scale
  timestamp unit/timezone
  nested type compatibility

partition:
  partition columns exist
  partition columns non-null where required
  replaceWhere data satisfies predicate
  partition cardinality acceptable

batching:
  no one-row writes
  no unbounded single huge batch
  row group size aligned
  target file size set or table property defined

governance:
  SaveMode selected explicitly
  SchemaMode selected explicitly for evolution
  cast safety selected explicitly
  commit metadata redacted
```

---

## 5.23 Error taxonomy

```text id="v7dbjb"
schema mismatch:
  input Arrow schema incompatible with table schema
  fix: cast upstream, use SchemaMode::Merge, or controlled SchemaMode::Overwrite

cast failure:
  incoming field cannot be safely cast
  fix: with_cast_safety(false) for fail-fast; clean data upstream

partition mismatch:
  with_partition_columns differs from existing table
  fix: match existing partitioning or perform full overwrite + schema overwrite without replaceWhere

predicate violation:
  replaceWhere predicate not satisfied by all incoming rows
  fix: validate input rows before write

DataFusion session fallback:
  non-SessionState session passed
  fix: use df.into_parts and RequireSessionState

object-store failure:
  credentials, permissions, endpoint, timeout, lock provider
  fix: backend-specific storage options and IAM/locking validation

concurrent transaction conflict:
  competing writer committed overlapping transaction
  fix: reload latest, reconcile, retry only if idempotent

unknown commit outcome:
  network/object-store timeout during commit
  fix: reload history/version, detect operation_id/slice, do not blind append retry
```

---

## 5.24 Testing matrix

```text id="j9tshm"
unit:
  RecordBatch schema validation
  partition predicate validation
  write config construction
  SaveMode parsing / selection
  SchemaMode policy

local integration:
  append creates new version
  overwrite logically removes old data
  load old version after overwrite
  replaceWhere only replaces bounded slice
  schema mismatch fails by default
  schema merge adds nullable/new columns
  schema overwrite full-table migration
  partition mismatch fails
  cast failure behavior safe=false

DataFusion integration:
  SQL query result writes to Delta
  DataFrame into_parts preserves SessionState
  missing UDF fails under RequireSessionState
  object-store mapping available to plan execution
  provider refreshed after write

cloud integration:
  S3 append with DynamoDB lock
  concurrent append conflict/retry
  MinIO/LocalStack endpoint options
  credential failure
  unknown commit outcome reconciliation

performance:
  target file size behavior
  row-group/write-batch sensitivity
  small-file load degradation
  compaction need threshold
```

---

## 5.25 Best practices

```text id="js0nyr"
API:
  - prefer DeltaTable::write over DeltaOps
  - use with_input_plan, not with_input_execution_plan
  - use df.into_parts for DataFusion writes
  - use RequireSessionState for production DataFusion plan writes

save mode:
  - always specify SaveMode explicitly
  - use Append for immutable facts
  - use Overwrite only for full/slice replacement
  - use ErrorIfExists for immutable artifacts
  - use Ignore only for bootstrap idempotency

schema:
  - default strict
  - Merge only for approved additive evolution
  - Overwrite only for controlled full migrations
  - fail-fast casts for governed tables

partition:
  - partition by query-pruning dimensions
  - never partition by high-cardinality accidental identifiers
  - validate replaceWhere rows before write
  - do not try to change partitioning during append

DataFusion:
  - pass the same SessionState that produced the LogicalPlan
  - ensure object-store mappings exist before execution
  - keep UDF/catalog/runtime state tenant-scoped

performance:
  - buffer micro-batches
  - target large enough Parquet files
  - tune row groups with benchmarks
  - schedule optimize for streaming-like ingestion

retries:
  - retry only idempotent writes automatically
  - include operation_id in commit metadata where possible
  - reconcile unknown outcomes by reloading table history/version
```

---

## 5.26 Anti-patterns

```text id="iz17cy"
- DeltaOps(table).write(...) in new documentation
- expecting (table, metrics) from WriteBuilder.await
- using into_unoptimized_plan in production DataFusion writes
- omitting with_session_state for with_input_plan
- accepting InternalDefaults fallback silently
- writing raw Parquet files into a Delta table directory
- one source object = one Delta commit
- one row = one RecordBatch = one Parquet file
- Append retries without idempotency key
- full-table overwrite for partition refresh
- replaceWhere without pre-validating incoming rows
- schema merge as a default ingestion convenience
- schema overwrite with replaceWhere
- changing partition columns during append
- logging storage options or commit metadata with secrets
```

---

## 5.27 LLM-agent checklist

```text id="xklnp5"
Before generating write code:
  1. Use DeltaTable::write, not DeltaOps.
  2. Confirm deltalake/datafusion feature when using expressions or LogicalPlan.
  3. Construct Arrow RecordBatch with Arrow 58 types.
  4. Validate all batches share identical schema.
  5. Select SaveMode explicitly.
  6. Select SchemaMode only when intentionally evolving schema.
  7. Select cast safety explicitly.
  8. Validate partition columns exist in input.
  9. Validate replaceWhere predicate against all rows.
  10. Set target_file_size for production fact writes.
  11. Set write_batch_size or benchmark default.
  12. Set Parquet writer properties only with benchmark/compatibility rationale.
  13. For DataFusion writes, call df.into_parts.
  14. Pass Arc::new(session_state) to with_session_state.
  15. Use SessionFallbackPolicy::RequireSessionState.
  16. Add commit metadata without secrets.
  17. Design retry path around idempotency.
  18. Reload/check version after write.
  19. Test local and object-store backends.
```

---

## 5.28 Value case

A correct Rust `deltalake` write layer gives your DataFusion/Arrow simulation platform:

```text id="zbx8mb"
Arrow-native zero-copy-friendly ingress shape
DataFusion query-result persistence
transactional overwrite/append semantics
schema-governed output contracts
bounded partition/slice replacement
Parquet tuning without bypassing Delta
cloud/object-store portability
audit-friendly commit metadata
deterministic simulation replay
safe migration from raw Parquet materialization
```

Core invariant:

```text id="un0xso"
Every write must declare: input source, save mode, schema policy, partition policy, cast policy, file-sizing policy, commit identity, and retry semantics.
```

Current API invariant:

```text id="n3v3l9"
In deltalake 1.0.0 (pinned rev), write builders produce a new DeltaTable, not a (DeltaTable, metrics) tuple; DeltaOps is compatibility/deprecated style, and DeltaTable::write is the canonical service API.
```



## 5.29 Delta action-path URI encoding hardening (latest pin)

Delta `Add`, `Remove`, and CDF action paths are URI paths in the transaction log. The current pin fixes serialization so a literal space is percent-encoded as `%20`, while path hierarchy `/` and Hive-style `=` delimiters remain literal. Existing logs containing legacy unencoded spaces remain readable, and already percent-encoded Spark-written paths round-trip without double-encoding.

Application rule:

```text
- Never hand-assemble Delta action path strings for transaction-log writes.
- Treat action paths as Delta/object-store URI identifiers and let delta-rs serialize them.
- Do not decode/re-encode Add/Remove paths merely for display and then reuse the display value as identity.
- Test filenames/partition values containing spaces and percent escapes in any custom maintenance tooling.
```

This change is especially relevant to custom log/maintenance tooling; normal `DeltaTable::write` users receive the fix automatically.

[1]: https://docs.rs/deltalake/latest/deltalake/struct.DeltaOps.html "DeltaOps in deltalake - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html "WriteBuilder in deltalake::operations::write - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/ "deltalake - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[5]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/protocol/mod.rs "delta-rs/crates/core/src/protocol/mod.rs at rev 9f922319 · delta-io/delta-rs · GitHub"
[6]: https://delta-io.github.io/delta-rs/usage/appending-overwriting-delta-lake-table/ "Append/overwrite tables - Delta Lake Documentation"
[7]: https://docs.rs/deltalake/0.32.3/deltalake/operations/write/enum.SchemaMode.html "SchemaMode in deltalake::operations::write - Rust (0.32.3 docs.rs page; surface unchanged at the 1.0.0 pinned rev)"
[8]: https://delta-io.github.io/delta-rs/usage/writing/ "Writing Delta Tables - Delta Lake Documentation"
[9]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"


# 6. Reading and querying through DataFusion — Rust `deltalake`

Version target:

```text id="2eld8j"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required deltalake feature: ["datafusion"]
Typical cloud profile: ["rustls", "datafusion", "s3"]
```

Core current-version correction: `open_table` takes a `url::Url`, not a raw `&str`; and `table.table_provider().await?` awaits a `TableProviderBuilder` into `Arc<dyn TableProvider>`. The `delta_datafusion` module docs show the exact pattern: open table, register `table.table_provider().await.unwrap()`, run `ctx.sql(...).collect().await`. ([Docs.rs][1])

---

## 6.1 Mental model

```text id="3s7hwm"
DeltaTable:
  loaded Delta transaction-log state
  active file set
  schema/protocol/metadata
  object-store/log-store handles

Delta TableProvider:
  DataFusion-facing table abstraction
  exposes Delta schema to SQL/DataFrame planning
  builds scans from Delta active files
  enables Delta-aware file pruning / skipping / metadata handling

SessionContext:
  DataFusion catalog + SQL planner + execution context
  owns runtime env, object-store registry, UDF registry, configs
  executes SQL/DataFrame logical plans over registered providers
```

`deltalake` exposes `delta_datafusion` specifically as “Datafusion integration for Delta Table,” while the crate re-exports DataFusion because DataFusion is an Arrow-based extensible query engine. ([Docs.rs][1])

---

## 6.2 Cargo baseline

```toml id="bdzgd9"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
```

Do not let DataFusion, Arrow, Parquet, or `object_store` float independently from the `deltalake` release baseline. The `datafusion` crate version in this baseline is `54.0.0`, and its dependency tree is Arrow `58`, Parquet `58`, and `object_store 0.13.x`. ([Docs.rs][2])

---

## 6.3 Minimal SQL query path

```rust id="v1eemp"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let table_url = Url::parse("s3://bucket/events")?;
    let table = open_table(table_url).await?;

    let ctx = SessionContext::new();

    // Register object-store mapping when using custom/runtime-backed sessions.
    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    ctx.register_table("events", table.table_provider().await?)?;

    let batches = ctx
        .sql("SELECT id, ts FROM events WHERE id > 100")
        .await?
        .collect()
        .await?;

    for batch in batches {
        println!("{batch:?}");
    }

    Ok(())
}
```

`update_datafusion_session` prepares a DataFusion session by registering the table root object store in the runtime environment when missing; it is idempotent and does not overwrite an existing mapping. ([Docs.rs][3])

---

## 6.4 Minimal DataFrame path

```rust id="4hy8r3"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

pub async fn query_delta_dataframe(uri: &str) -> anyhow::Result<()> {
    let table = open_table(Url::parse(uri)?).await?;

    let ctx = SessionContext::new();
    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let provider = table.table_provider().await?;

    let df = ctx
        .read_table(provider)?
        .filter(col("id").gt(lit(100)))?
        .select(vec![col("id"), col("ts")])?;

    let batches = df.collect().await?;

    tracing::info!(batch_count = batches.len(), "delta dataframe query complete");

    Ok(())
}
```

Use the DataFrame path when the query is generated programmatically from UI state, simulation configuration, a DSL, or LLM-produced expression trees. Use SQL when the query text itself is the contract.

---

## 6.5 `DeltaTable` as `TableProvider`

`DeltaTable::table_provider()` returns a `TableProviderBuilder`. The builder can be awaited directly into `Arc<dyn TableProvider>`, or `.build().await` can produce a lower-level `DeltaScan`. The builder can use a log store, `Snapshot`, or eager snapshot; if a snapshot is supplied, no IO is performed when building the provider. ([Docs.rs][4])

```rust id="2w8k2j"
use datafusion::catalog::TableProvider;
use deltalake::DeltaTable;
use std::sync::Arc;

pub async fn provider_from_table(table: &DeltaTable) -> anyhow::Result<Arc<dyn TableProvider>> {
    Ok(table.table_provider().await?)
}
```

Snapshot-binding implication:

```text id="26oo97"
Provider freshness is not magic.
If the DeltaTable is refreshed to a newer version and query correctness requires that newer version:
  rebuild provider
  deregister old table name if needed
  register new provider
```

---

## 6.6 Registering with explicit table names

```rust id="1fq1q7"
pub async fn register_delta_as(
    ctx: &datafusion::prelude::SessionContext,
    table_name: &str,
    uri: &str,
) -> anyhow::Result<u64> {
    let table = deltalake::open_table(url::Url::parse(uri)?).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("loaded Delta table has no version"))?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    ctx.register_table(table_name, table.table_provider().await?)?;

    Ok(version)
}
```

Naming policy:

```text id="12h0ex"
Use stable SQL names:
  events
  simulation_outputs
  crude_properties
  scenario_results

Avoid:
  s3_bucket_path_names
  names with hyphens
  names derived from untrusted user input
  accidental case-mixed names unless DeltaSessionContext/case-sensitivity policy is deliberate
```

---

## 6.7 Multi-table registration

```rust id="q41sa0"
use datafusion::prelude::SessionContext;
use std::collections::HashMap;

pub async fn register_delta_tables(
    ctx: &SessionContext,
    tables: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, u64>> {
    let mut versions = HashMap::new();

    for (name, uri) in tables {
        let table = deltalake::open_table(url::Url::parse(&uri)?).await?;

        let version = table
            .version()
            .ok_or_else(|| anyhow::anyhow!("table {name} has no loaded version"))?;

        let state = ctx.state();
        table.update_datafusion_session(&state)?;

        ctx.register_table(&name, table.table_provider().await?)?;
        versions.insert(name, version);
    }

    Ok(versions)
}
```

Register all Delta sources before planning cross-table joins, especially when object-store mappings differ by bucket/account/container.

---

## 6.8 SQL path

```rust id="0lxlv0"
pub async fn run_sql(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let df = ctx.sql(sql).await?;
    let batches = df.collect().await?;
    Ok(batches)
}
```

Recommended SQL lifecycle:

```text id="pz4s9v"
1. Register Delta providers.
2. Register non-Delta providers.
3. Register UDFs/UDAFs/window functions.
4. Validate SQL against allowlist / generated grammar.
5. Run EXPLAIN for diagnostics when needed.
6. Collect, stream, or write DataFrame result to another Delta table.
```

SQL is ideal for analyst-facing and agent-generated query materializations, but untrusted SQL requires catalog isolation, table allowlisting, function allowlisting, execution limits, and timeout/memory governance.

---

## 6.9 DataFrame path

```rust id="la2jgm"
use datafusion::prelude::*;

pub async fn programmatic_query(ctx: &SessionContext) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let df = ctx
        .table("simulation_outputs")
        .await?
        .filter(
            col("simulation_id")
                .eq(lit("sim-001"))
                .and(col("output_value").gt(lit(0.0))),
        )?
        .select(vec![
            col("simulation_id"),
            col("unit_id"),
            col("output_value"),
        ])?;

    Ok(df.collect().await?)
}
```

Use DataFrame APIs for:

```text id="6vsv5v"
LLM-generated safe query builders
UI filter forms
typed simulation-selection compilation
policy-injected filters
tenant filters
parameterized query construction
query-template expansion
```

Avoid string-concatenated SQL when a query is structurally generated from application state.

---

## 6.10 Predicate pushdown

`DeltaScanConfig` has `enable_parquet_pushdown`, which defaults to true and allows scan-filter pushdown. It also has schema/view-type and partition-value wrapping controls. ([Docs.rs][5])

```rust id="ndxoea"
use deltalake::delta_datafusion::DeltaScanConfig;

let scan_config = DeltaScanConfig::new()
    .with_parquet_pushdown(true);
```

Predicate pushdown stack:

```text id="t3ve6s"
DataFusion filter expression
  -> Delta scan planning
    -> partition pruning from Delta partition values
    -> Delta file skipping from transaction-log statistics
    -> Parquet row-group/page pruning where available
    -> Arrow record-batch filtering
```

Delta Lake’s DataFusion integration guide states that Delta stores file-level metadata in the transaction log, can skip entire files, and can then skip row groups inside individual files. ([Delta IO][6])

---

## 6.11 Projection pushdown

Projection pushdown means DataFusion requests only required columns from the Delta provider/Parquet reader.

Good:

```sql id="7ju43d"
SELECT simulation_id, unit_id, output_value
FROM simulation_outputs
WHERE run_date = '2026-06-02'
```

Bad:

```sql id="3da6e6"
SELECT *
FROM simulation_outputs
WHERE run_date = '2026-06-02'
```

Agent rule:

```text id="rm3r7v"
Always project explicit columns unless the user explicitly asks for all columns.
Avoid SELECT * in generated queries over wide simulation tables.
```

---

## 6.12 Partition pruning

Partition pruning occurs when filters constrain partition columns.

```sql id="tkbz78"
SELECT unit_id, output_value
FROM simulation_outputs
WHERE run_date = '2026-06-02'
  AND scenario_family = 'base_case'
```

Layout guidance:

```text id="9761jt"
Partition columns should match common WHERE predicates.
Partition columns should be low/medium cardinality.
Do not partition by continuous values, unique run IDs, timestamps to the second, or high-cardinality free text.
```

Partition pruning happens before file reads; file skipping uses transaction-log statistics for non-partition columns. Delta’s performance guide for DataFusion highlights the value of file-level metadata for skipping whole files. ([Delta IO][6])

---

## 6.13 File skipping and data skipping

Delta query acceleration depends on metadata quality:

```text id="npvsan"
effective when:
  files contain useful min/max stats
  query predicates target stats-covered columns
  data layout clusters related values
  file count is controlled
  partitioning and Z-order/compaction align with query patterns

weak when:
  tiny files dominate
  stats missing/truncated for filter columns
  predicates wrap columns in non-pushdown functions
  filters are non-deterministic
  string stats are insufficient for workload
```

Recommended generated-query style:

```sql id="lcjy9o"
-- pushdown-friendly
WHERE output_value > 0.0

-- often weaker
WHERE ABS output_value > 0.0
```

Use explicit simple comparisons on physical columns where possible.

---

## 6.14 `DeltaScanConfig`

Fields documented for `DeltaScanConfig`:

```text id="ur94de"
file_column_name: Option<String>
wrap_partition_values: bool
enable_parquet_pushdown: bool
schema_force_view_types: bool
schema: Option<Arc<Schema>>
```

The docs state `file_column_name` includes source path per record, `wrap_partition_values` dictionary-encodes partition values by default, `enable_parquet_pushdown` defaults to true, and `schema_force_view_types` reads UTF-8/binary columns as view types. ([Docs.rs][5])

Example:

```rust id="xmr83c"
use deltalake::delta_datafusion::DeltaScanConfig;

let config = DeltaScanConfig::new()
    .with_file_column_name("_delta_file_path")
    .with_parquet_pushdown(true)
    .with_wrap_partition_values(true);
```

Use file-column metadata for:

```text id="kukx50"
debugging unexpected rows
auditing file-level provenance
small-file diagnostics
skew inspection
testing pruning behavior
```

Do not expose file paths to untrusted tenants unless object-store paths are non-sensitive.

---

## 6.15 Provider with file column

`TableProviderBuilder::with_file_column(...)` appends a source-file-path column to the scan. ([Docs.rs][4])

```rust id="d5abuu"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

pub async fn register_with_file_column(ctx: &SessionContext, uri: &str) -> anyhow::Result<()> {
    let table = open_table(Url::parse(uri)?).await?;
    let state = ctx.state();

    table.update_datafusion_session(&state)?;

    let provider = table
        .table_provider()
        .with_file_column("_delta_file_path")
        .await?;

    ctx.register_table("events_debug", provider)?;

    Ok(())
}
```

Debug query:

```sql id="s8z4ns"
SELECT _delta_file_path, COUNT(*) AS rows
FROM events_debug
GROUP BY _delta_file_path
ORDER BY rows DESC
```

---

## 6.16 `DeltaSessionContext`

`DeltaSessionContext` is a wrapper around DataFusion’s `SessionContext` with Delta-specific defaults such as case-sensitive identifiers and Delta planner configuration. It exposes `new`, `with_runtime_env`, `state`, and `into_inner`. ([Docs.rs][7])

```rust id="4vzp4c"
use deltalake::delta_datafusion::DeltaSessionContext;

pub fn new_delta_context() -> datafusion::prelude::SessionContext {
    DeltaSessionContext::new().into_inner()
}
```

Use `DeltaSessionContext` when:

```text id="hgab1k"
you want Delta-specific defaults
case-sensitive Delta column handling matters
Delta logical/physical planner integration is needed
you want a standard session template for all Delta query services
```

Use plain `SessionContext` when:

```text id="ikqi8f"
you already own DataFusion session construction
you have strict app-wide DataFusion config
you need custom catalog/runtime/UDF registration
you call update_datafusion_session explicitly per table
```

---

## 6.17 `DeltaRuntimeEnvBuilder`

The `delta_datafusion` module lists `DeltaRuntimeEnvBuilder` as a builder for configuring DataFusion `RuntimeEnv` with Delta-specific defaults. ([Docs.rs][1])

Recommended role:

```text id="puc6r7"
Use DeltaRuntimeEnvBuilder when:
  building a Delta-first query service
  centralizing object-store/runtime defaults
  pairing with DeltaSessionContext
  enabling spill/runtime policies consistently

Use DataFusion RuntimeEnvBuilder directly when:
  your service already has a mature DataFusion runtime abstraction
  memory pools, disk managers, and cache managers are app-owned
```

Do not mix multiple runtime environments accidentally for tables that must join within one query.

---

## 6.18 Spill configuration

DataFusion’s `RuntimeEnv` manages memory, disk, cache, and object-store registry resources; its runtime docs describe `MemoryPool`, `DiskManager`, `CacheManager`, and `ObjectStoreRegistry`. ([Docs.rs][8])

Delta’s `delta_datafusion` module exposes `create_session_state_with_spill_config`, described as creating a `SessionState` with optional spill-to-disk configuration. ([Docs.rs][1])

Deployment posture:

```text id="yqqgdt"
For production analytical queries:
  configure spill directory on fast local disk
  isolate spill path per service/container
  enforce disk quota externally
  do not spill to network filesystems unless benchmarked
  scrub spill data if sensitive
  monitor spill bytes and spill time
```

Practical service config:

```text id="679biz"
DATAFUSION_SPILL_DIR=/var/lib/my-service/spill
DATAFUSION_TARGET_PARTITIONS=<cpu_or_io_tuned_value>
DATAFUSION_MEMORY_LIMIT_BYTES=<service_budget>
```

---

## 6.19 Physical execution plan diagnostics

```rust id="lb0cti"
pub async fn explain_query(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let explain_sql = format!("EXPLAIN VERBOSE {sql}");
    Ok(ctx.sql(&explain_sql).await?.collect().await?)
}
```

Use `EXPLAIN` to verify:

```text id="34msyf"
Delta provider appears, not raw ListingTable over Parquet
projection pushdown reduces schema width
filter appears near scan
partition predicates are visible
joins use expected strategy
limit/order/window operations are placed correctly
```

Diagnostics to log:

```text id="4yp0y4"
registered_table_name
delta_version
logical_plan_text
physical_plan_text
input_partitions
output_batch_count
output_row_count
elapsed_ms
object_store_scheme
spill_enabled
```

---

## 6.20 Joining Delta with in-memory `MemTable`

```rust id="eaddl7"
use std::sync::Arc;

use arrow_array::{Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use datafusion::datasource::MemTable;
use datafusion::prelude::*;

pub async fn join_delta_with_memory(ctx: &SessionContext) -> anyhow::Result<()> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("priority", DataType::Int64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec!["U1", "U2"])),
            Arc::new(Int64Array::from(vec![10, 20])),
        ],
    )?;

    let mem_table = MemTable::try_new(schema, vec![vec![batch]])?;
    ctx.register_table("unit_priority", Arc::new(mem_table))?;

    let rows = ctx
        .sql(
            r#"
            SELECT s.unit_id, s.output_value, p.priority
            FROM simulation_outputs s
            JOIN unit_priority p
              ON s.unit_id = p.unit_id
            "#,
        )
        .await?
        .collect()
        .await?;

    tracing::info!(batch_count = rows.len(), "join complete");

    Ok(())
}
```

Use for:

```text id="4dw54y"
small lookup tables
request-scoped parameters
UI selections
scenario parameter tables
ad hoc what-if inputs
```

---

## 6.21 Joining Delta with Parquet / CSV / custom providers

```rust id="fep4gk"
use datafusion::prelude::*;

pub async fn register_external_files(ctx: &SessionContext) -> anyhow::Result<()> {
    ctx.register_parquet(
        "baseline_parquet",
        "s3://bucket/baseline/",
        ParquetReadOptions::default(),
    )
    .await?;

    ctx.register_csv(
        "input_csv",
        "s3://bucket/input.csv",
        CsvReadOptions::new(),
    )
    .await?;

    Ok(())
}
```

Recommended pattern:

```text id="zgtmlp"
Delta:
  durable governed tables
  transactional simulation outputs
  versioned inputs

Parquet:
  external immutable datasets
  one-off vendor drops
  export/interchange files

CSV:
  small staging inputs
  user uploads
  diagnostics

Custom TableProvider:
  virtual simulation state
  generated parameter spaces
  remote service-backed tables
  policy-filtered table views
```

Avoid joining Delta to raw files from a different object-store registry unless all stores are registered in the same DataFusion runtime.

---

## 6.22 Object-store registration and avoiding duplicates

`update_datafusion_session` is idempotent and does not overwrite existing object-store mappings. This is safe for normal registration but dangerous if an existing mapping points to a wrong endpoint, stale credentials, or a different test backend. ([Docs.rs][3])

Safe registry pattern:

```rust id="9sjwaw"
pub async fn prepare_delta_table_for_ctx(
    ctx: &datafusion::prelude::SessionContext,
    table: &deltalake::DeltaTable,
) -> anyhow::Result<()> {
    let state = ctx.state();
    table.update_datafusion_session(&state)?;
    Ok(())
}
```

Operational rules:

```text id="q88aey"
One SessionContext should have one intended mapping per object-store URL prefix.
Do not mix prod S3 and LocalStack under the same URL prefix.
Do not assume update_datafusion_session repairs a bad mapping.
Create fresh SessionContext for isolated tests with different endpoints.
```

---

## 6.23 Table alias and catalog/schema naming

Recommended naming tiers:

```text id="m8ygin"
DataFusion catalog:
  tenant / workspace / environment

DataFusion schema:
  domain / product area / simulation namespace

Table name:
  stable logical table name

SQL alias:
  short query-local symbol
```

Example:

```sql id="u257dt"
SELECT
  r.run_id,
  o.unit_id,
  o.output_value
FROM simulation_outputs AS o
JOIN simulation_runs AS r
  ON o.run_id = r.run_id
WHERE r.model_version = 'v17'
```

Agent rules:

```text id="nny4s9"
Use aliases in generated joins.
Never reuse the same alias for two tables.
Qualify ambiguous columns.
Prefer snake_case identifiers.
Quote identifiers only when required by existing schema.
```

---

## 6.24 Freshness and provider lifecycle

```rust id="rh6j4m"
pub async fn refresh_and_reregister(
    ctx: &datafusion::prelude::SessionContext,
    table: &mut deltalake::DeltaTable,
    name: &str,
) -> anyhow::Result<u64> {
    table.update_incremental(None).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("table not initialized after refresh"))?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    ctx.deregister_table(name)?;
    ctx.register_table(name, table.table_provider().await?)?;

    Ok(version)
}
```

Policy:

```text id="fw1qh9"
LATEST_STRICT:
  refresh table
  rebuild provider
  register provider
  run query
  return resolved version

PINNED_VERSION:
  build table at exact version
  register version-scoped provider
  do not refresh during query

LATEST_EVENTUAL:
  refresh on timer or cache invalidation
  tolerate stale provider inside SLA
```

---

## 6.25 Querying pinned Delta versions

```rust id="8s2xer"
use deltalake::DeltaTableBuilder;
use datafusion::prelude::*;
use url::Url;

pub async fn register_pinned_delta_version(
    ctx: &SessionContext,
    name: &str,
    uri: &str,
    version: u64,
) -> anyhow::Result<()> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .with_version(version)
        .load()
        .await?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    ctx.register_table(name, table.table_provider().await?)?;

    Ok(())
}
```

Use pinned providers for:

```text id="ccwnam"
simulation replay
report reproducibility
debugging historical outputs
cross-engine validation
before/after optimization benchmarking
```

---

## 6.26 Case sensitivity and Delta column names

Delta schemas can be case-sensitive in ways that normal SQL users may not expect. `DeltaColumn` exists to preserve case sensitivity during string conversion, and `DeltaSessionContext` is documented as using Delta-specific defaults including case-sensitive identifiers. ([Docs.rs][9])

Policy:

```text id="ki6py2"
For new tables:
  use lowercase snake_case
  avoid columns that differ only by case

For existing mixed-case tables:
  use DeltaSessionContext or explicit session configuration
  quote identifiers where needed
  test filters/projections on mixed-case columns
```

---

## 6.27 Diagnostics table for query failures

```text id="8030co"
No table named X:
  provider not registered
  catalog/schema mismatch
  typo or alias issue

Object store not found:
  update_datafusion_session not called
  wrong RuntimeEnv
  URI scheme not registered

No such column:
  schema drift
  case-sensitivity mismatch
  unqualified ambiguous column
  stale provider

Slow query:
  no partition predicate
  SELECT *
  missing file stats
  tiny-file explosion
  disabled parquet pushdown
  high-cardinality partition layout
  join strategy mismatch
  spill disabled or slow disk

Wrong/stale result:
  provider built from old snapshot
  table refreshed but provider not rebuilt
  query intended latest but used pinned version
```

---

## 6.28 Metrics and observability

Minimum query event:

```rust id="fs2jsu"
#[derive(Debug, serde::Serialize)]
pub struct DeltaQueryEvent {
    pub table_names: Vec<String>,
    pub table_versions: Vec<(String, u64)>,
    pub sql_hash: String,
    pub output_batches: usize,
    pub output_rows: usize,
    pub elapsed_ms: u128,
}
```

Collect row counts:

```rust id="v4vfrw"
pub fn count_rows(batches: &[arrow_array::RecordBatch]) -> usize {
    batches.iter().map(|b| b.num_rows()).sum()
}
```

Observability fields:

```text id="mq1px6"
delta_table_name
delta_table_uri_hash
delta_version
datafusion_catalog
datafusion_schema
sql_hash
logical_plan_hash
physical_plan_hash
input_partition_count
output_row_count
elapsed_ms
spill_bytes
object_store_scheme
```

---

## 6.29 Security and governance

```text id="v0vsvf"
Required controls:
  table allowlist
  column allowlist for user-facing query builders
  tenant filter injection
  SQL parser/AST validation for untrusted SQL
  function allowlist
  query timeout
  output row limit
  memory/spill quota
  object-store credential isolation
  no raw object-store path leakage through file columns
```

Never expose `with_file_column` output to untrusted users if paths include bucket names, tenant IDs, workspace names, or directory conventions.

---

## 6.30 Testing matrix

```text id="3r98a3"
compile:
  SessionContext import
  open_table Url path
  table_provider await
  update_datafusion_session
  SQL collect
  DataFrame collect

local integration:
  create/write local Delta table
  register provider
  SELECT explicit columns
  SELECT with WHERE filter
  JOIN with MemTable
  EXPLAIN query

freshness:
  provider at version N
  append version N+1
  old provider behavior
  rebuild provider
  new provider behavior

pushdown:
  EXPLAIN with filter
  EXPLAIN with projection
  compare SELECT * vs projected query
  partition-filter query over partitioned table

object store:
  S3/MinIO registration
  missing update_datafusion_session failure
  wrong endpoint isolation
  multiple buckets in one SessionContext

governance:
  mixed-case column query
  ambiguous join columns
  file-column leakage check
  tenant-filter injection
```

---

## 6.31 Best practices

```text id="iy0zut"
API:
  - use url::Url with open_table
  - call update_datafusion_session before registering/querying object-store-backed tables
  - await table.table_provider()
  - rebuild providers after freshness-relevant table refreshes

SQL:
  - project explicit columns
  - qualify columns in joins
  - use pushdown-friendly predicates
  - run EXPLAIN for generated queries in debug/test mode

DataFrame:
  - prefer expressions for programmatic filters
  - use DataFrame path for UI/DSL/agent-generated queries
  - avoid string concatenation for untrusted filter values

Performance:
  - partition by common filters
  - compact tiny files
  - preserve data-skipping stats
  - enable parquet pushdown
  - configure spill for large joins/sorts/aggregations

Runtime:
  - use one coherent SessionContext per query domain
  - avoid duplicated object-store mappings
  - isolate tenants with separate contexts/catalogs/credentials when needed
  - log resolved Delta versions

Governance:
  - expose logical table names, not storage paths
  - use stable alias conventions
  - restrict file-path metadata columns
  - validate SQL/function/table access
```

---

## 6.32 Anti-patterns

```text id="spkk9h"
- registering raw Parquet folders instead of Delta providers
- assuming provider auto-refreshes after commits
- skipping update_datafusion_session with custom object-store backends
- using SELECT * over wide tables
- string-concatenating user filters into SQL
- mixing production and test object-store mappings in one SessionContext
- exposing _delta_file_path to end users
- using high-cardinality partition columns and expecting pruning to help
- forgetting case sensitivity in mixed-case Delta schemas
- rebuilding SessionContext for every tiny query without reason
- opening/loading Delta table per row
```

---

## 6.33 LLM-agent checklist

```text id="qxoeux"
Before generating Delta/DataFusion read code:
  1. Confirm deltalake feature includes "datafusion".
  2. Parse table URI into url::Url.
  3. Open/load DeltaTable.
  4. Capture resolved Delta version.
  5. Create or reuse SessionContext.
  6. Call table.update_datafusion_session(&ctx.state()).
  7. Await table.table_provider().
  8. Register provider under stable logical name.
  9. Use SQL or DataFrame path deliberately.
  10. Project explicit columns.
  11. Use pushdown-friendly filters.
  12. Qualify join columns.
  13. Run EXPLAIN for diagnostics.
  14. Rebuild provider after refresh if latest correctness matters.
  15. Log table name, version, plan hash, elapsed time, output rows.
```

---

## 6.34 Value case

Correct Delta/DataFusion integration gives your Rust simulation workbench:

```text id="uh1871"
transactionally governed table reads
Arrow-native execution
SQL and programmatic DataFrame query paths
version-pinned reproducibility
file skipping and partition pruning
object-store portability
join interoperability with MemTable/Parquet/CSV/custom providers
schema-aware query planning
runtime-level spill/memory/object-store control
LLM-agent-safe query generation boundaries
```

Core invariant:

```text id="49fw09"
Register Delta tables as Delta TableProviders, not as raw Parquet directories.
```

Freshness invariant:

```text id="6is4j7"
A DataFusion query sees the snapshot bound into the registered provider; refresh table state and rebuild provider when latest-version correctness matters.
```

---

## 6.35 Next-scan file selection: `FileSelection` and `MissingSelectedFilePolicy` (new at the 1.0.0 pinned rev)

The kernel-backed “next” scan grew an explicit file-selection surface at the pinned rev. The public re-exports at `deltalake::delta_datafusion` now include:

```rust
use deltalake::delta_datafusion::{
    DeletionVectorSelection,       // per-file DV keep-mask (filepath + Vec<bool>)
    DeltaScanNext,                 // next-scan builder (next::DeltaScan re-exported under this name)
    DeltaScanExec,                 // the ExecutionPlan the provider produces
    FileSelection,                 // NEW: limit a scan to explicit files
    MissingSelectedFilePolicy,     // NEW: Error (default) | Ignore
};
```

`FileSelection` limits a scan to explicit files — e.g. paths from `DeltaTable::get_files_by_partitions` or `Add` actions produced by maintenance tasks — while all file metadata (deletion vectors, partition values, statistics, column mapping, tags) still comes from the scan snapshot:

```rust
use deltalake::delta_datafusion::{FileSelection, MissingSelectedFilePolicy};

// From Add actions (only the path is used):
let selection = FileSelection::from_adds(adds);

// From explicit paths (relative to table root, or absolute URLs under it):
let selection = FileSelection::from_file_paths(["part-0001.parquet", "part-0002.parquet"])
    .with_missing_file_policy(MissingSelectedFilePolicy::Ignore);
```

Semantics verified in source at the pinned rev:

```text
- Default policy is MissingSelectedFilePolicy::Error: any selected file not
  active in the scan snapshot fails the scan.
- Ignore skips inactive selected files (maintenance-rewrite friendly), but
  malformed paths, paths outside the table root, protocol/object-store/planning
  failures still error.
- Empty selections produce empty scans; duplicate inputs are deduplicated.
- Absolute URLs must be under the table root; credentials/query/fragment are
  stripped before storage; out-of-root paths are rejected with redacted errors.
- Query pruning does NOT mark a selected file as missing.
- The next-scan builder accepts selections via with_file_selection(FileSelection),
  or the shorthands with_adds(...) / with_file_paths(...).
```

Use cases: partition-scoped maintenance reads, file-level backfills/repairs, targeted quality scans over known-bad files, and deterministic re-reads of an exact file set under `MissingSelectedFilePolicy::Error`.



## 6.36 Delta-aware physical-expression/schema adaptation at the latest pin

The current kernel-backed provider wraps DataFusion's default `PhysicalExprAdapterFactory` with a Delta-specific adapter for nested nullability. This is a correctness/interoperability change rather than a new public query method: predicates and projections can now be planned against Spark-written files whose nested Parquet nullability is looser than the logical Delta contract, while the strict logical result schema is restored after file adaptation.

The latest commit also excludes nested fields from top-level partition-column mapping. Partition columns are top-level Delta columns; a nested field sharing the same name is not a partition candidate.

Agent rule: do not reproduce this adaptation in application code. Register the Delta `TableProvider` and allow delta-rs to own the logical-to-physical schema bridge. Custom raw-Parquet providers over a Delta table path would bypass these fixes.

[1]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/index.html "deltalake::delta_datafusion - Rust"
[2]: https://docs.rs/datafusion/latest/src/datafusion/lib.rs.html?utm_source=chatgpt.com "datafusion/ lib.rs"
[3]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.TableProviderBuilder.html "TableProviderBuilder in deltalake::delta_datafusion - Rust"
[5]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaScanConfig.html "DeltaScanConfig in deltalake::delta_datafusion - Rust"
[6]: https://delta-io.github.io/delta-rs/integrations/delta-lake-datafusion/ "DataFusion - Delta Lake Documentation"
[7]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaSessionContext.html "DeltaSessionContext in deltalake::delta_datafusion - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/execution/runtime_env/struct.RuntimeEnv.html?utm_source=chatgpt.com "RuntimeEnv in datafusion::execution::runtime_env - Rust"
[9]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaColumn.html?utm_source=chatgpt.com "DeltaColumn in deltalake::delta_datafusion - Rust"


# 7. DataFusion + Arrow integration track — exhaustive Rust `deltalake` version

Version target:

```text id="f9q4q4"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required deltalake features:
  ["datafusion"]
Typical production features:
  ["rustls", "datafusion", "s3"]
```

Current-version correction: `DeltaTable::table_provider()` returns a `TableProviderBuilder`, and awaiting that builder yields a DataFusion `TableProvider`; `update_datafusion_session` takes a `&dyn Session`, so the robust pattern is `let state = ctx.state(); table.update_datafusion_session(&state)?;`, not blindly passing a raw path or assuming the session object-store registry is already configured. The docs state that `update_datafusion_session` registers the table root object store with the session runtime when missing and does not overwrite an existing mapping. ([Docs.rs][1])

**DataFusion 53→54 migration pointer:** every snippet in this chapter was re-validated against DataFusion `54.0.0`. If you are porting your own DataFusion extension code (custom `TableProvider`/`ExecutionPlan`/UDF impls, proto codecs, pruning statistics), consult the companion change catalog `docs/library_ref/datafusion_54vs53.md` — the highest-impact items are the `as_any()` removal from major extension traits (use trait upcasting: `plan.downcast_ref::<T>()`), `ExecutionPlan::partition_statistics` returning `Arc<Statistics>`, and datafusion-proto decode paths now requiring a `TaskContext`. Plain Arrow-array downcasts (`array.as_any().downcast_ref::<StringArray>()`) are unaffected.

---

# 7.0 Integration mental model

```text id="wxtsma"
DeltaTable:
  transaction-log-backed logical table handle
  loaded version / snapshot
  schema / protocol / metadata
  active file set
  object-store + log-store references

DataFusion TableProvider:
  logical table binding for query planning
  exposes Arrow schema
  constructs scans over active Delta files
  participates in projection/filter/partition/file pruning

Arrow:
  in-memory columnar data format
  RecordBatch input/output boundary
  DataFusion execution batch format
  Delta write input format
  Parquet writer input format

RuntimeEnv:
  memory pool
  disk manager / spill
  cache manager
  object-store registry
  per-session execution resource context
```

DataFusion is an extensible Rust query engine using Apache Arrow as its in-memory format, while `deltalake::delta_datafusion` is the Delta/DataFusion integration module. ([Docs.rs][2])

Canonical invariant:

```text id="81el1s"
Delta is the table-state authority.
DataFusion is the query/execution engine.
Arrow is the batch interchange format.
Parquet is the persisted data-file format.
```

---

# 7.1 TableProvider integration

## 7.1.1 Minimal provider registration

```rust id="jf4csr"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

pub async fn register_delta_provider(
    ctx: &SessionContext,
    name: &str,
    uri: &str,
) -> anyhow::Result<u64> {
    let table = open_table(Url::parse(uri)?).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("Delta table loaded without a version"))?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let provider = table.table_provider().await?;
    ctx.register_table(name, provider)?;

    Ok(version)
}
```

`DeltaTable::table_provider()` is the public entry point for building a DataFusion table provider from a loaded Delta table. The `delta_datafusion` module’s own example follows the same shape: open a table, call `table.table_provider().await`, register it in a `SessionContext`, then query via SQL. ([Docs.rs][1])

## 7.1.2 `TableProviderBuilder`

`TableProviderBuilder` can be built from a log store, a `Snapshot`, or an eager snapshot. If a snapshot is provided directly, the builder uses it without IO while building the provider; it also supports attaching a session, specifying a table version, and adding a file-path metadata column. ([Docs.rs][3])

```rust id="g0oqp3"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

pub async fn register_delta_debug_provider(
    ctx: &SessionContext,
    uri: &str,
) -> anyhow::Result<()> {
    let table = open_table(Url::parse(uri)?).await?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let provider = table
        .table_provider()
        .with_session(std::sync::Arc::new(state.clone()))
        .with_file_column("_delta_file_path")
        .await?;

    ctx.register_table("delta_debug", provider)?;

    Ok(())
}
```

Use `with_file_column` only for diagnostics, lineage, small-file analysis, pruning validation, and debugging. It appends a column containing the source file path for each record, which can leak bucket, tenant, environment, or workspace structure if exposed to users. ([Docs.rs][3])

## 7.1.3 Provider construction cost

```text id="r3yj48"
Provider construction cost depends on:
  - whether table state is already loaded
  - whether a snapshot/eager snapshot is supplied
  - table version pinning
  - Delta log replay/checkpoint load cost
  - active file count
  - metadata/statistics volume
  - object-store latency
```

Provider construction is cheap only when the required snapshot is already available and no additional log/object-store IO is needed. `TableProviderBuilder` docs explicitly distinguish log-store construction from snapshot/eager-snapshot construction, and state that snapshot-provided construction performs no IO while building the provider. ([Docs.rs][3])

## 7.1.4 Snapshot binding and freshness

```rust id="39uknu"
pub async fn refresh_and_reregister_provider(
    ctx: &SessionContext,
    table: &mut deltalake::DeltaTable,
    table_name: &str,
) -> anyhow::Result<u64> {
    table.update_incremental(None).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("Delta table not initialized after refresh"))?;

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    ctx.deregister_table(table_name)?;
    ctx.register_table(table_name, table.table_provider().await?)?;

    Ok(version)
}
```

A `DeltaTable` has an in-memory state representing the table as of the most recent loaded Delta log entry, and `version()` returns the currently loaded version. Therefore, provider freshness should be treated as an explicit lifecycle decision: refresh table state, rebuild provider, and re-register when a query must see the latest version. ([Docs.rs][1])

## 7.1.5 Provider registration naming

```text id="fjvq4v"
Recommended:
  simulation_outputs
  simulation_runs
  crude_properties
  unit_operations
  scenario_inputs

Avoid:
  s3_bucket_prod_model_v17
  user-provided raw names
  names with hyphens
  case-sensitive names unless deliberate
  paths encoded as SQL identifiers
```

Agent naming rule:

```text id="92z066"
Storage URI is infrastructure.
DataFusion table name is semantic API.
SQL alias is query-local ergonomics.
```

## 7.1.6 Multi-tenant provider registration

```rust id="j55b8t"
use datafusion::catalog::MemTable;
use datafusion::prelude::*;
use std::collections::HashMap;

pub async fn register_tenant_delta_tables(
    ctx: &SessionContext,
    tenant_schema_prefix: &str,
    tables: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, u64>> {
    let mut versions = HashMap::new();

    for (logical_name, uri) in tables {
        let table = deltalake::open_table(url::Url::parse(&uri)?).await?;
        let version = table
            .version()
            .ok_or_else(|| anyhow::anyhow!("Delta table {logical_name} has no loaded version"))?;

        let state = ctx.state();
        table.update_datafusion_session(&state)?;

        let sql_name = format!("{tenant_schema_prefix}_{logical_name}");
        ctx.register_table(&sql_name, table.table_provider().await?)?;

        versions.insert(sql_name, version);
    }

    Ok(versions)
}
```

For serious tenant isolation, prefer separate `SessionContext`s, catalogs, schemas, credentials, or object-store registries rather than only name prefixes. DataFusion `SessionContext` is the connection/query interface and can register custom data sources that SQL can reference. ([Docs.rs][4])

---

# 7.2 DataFusion session/runtime integration

## 7.2.1 `SessionContext`, `SessionState`, `RuntimeEnv`

```text id="f8h2z8"
SessionContext:
  user/query interface
  SQL planner entrypoint
  DataFrame factory
  table/catalog/function registry surface

SessionState:
  query-planning state snapshot
  config/options
  catalog list
  function registry
  optimizer/planner state
  passed into write paths and Delta scan configuration

RuntimeEnv:
  memory_pool
  disk_manager
  cache_manager
  object_store_registry
  parquet encryption registry
```

DataFusion documents `SessionContext` as the main interface for executing queries and maintaining user/engine connection state; `RuntimeEnv` manages memory, disk, cache, and object-store mappings. ([Docs.rs][4])

## 7.2.2 Correct `update_datafusion_session` pattern

```rust id="gl9qbm"
use datafusion::prelude::*;
use deltalake::DeltaTable;

pub fn prepare_delta_table_for_context(
    ctx: &SessionContext,
    table: &DeltaTable,
) -> anyhow::Result<()> {
    let state = ctx.state();
    table.update_datafusion_session(&state)?;
    Ok(())
}
```

`update_datafusion_session` registers the Delta table’s root object store with the session runtime environment if missing; it is idempotent and explicitly will not overwrite an existing mapping. ([Docs.rs][1])

## 7.2.3 Avoiding accidental object-store override

```text id="guohlo"
Safe:
  fresh SessionContext per environment/test isolation
  register table object stores once
  fail fast on wrong endpoint/credentials
  hash/log object-store target without secrets

Dangerous:
  prod S3 and LocalStack using same s3://bucket URI in one process
  reusing SessionContext across tenants with different credentials
  assuming update_datafusion_session overwrites a stale mapping
  hidden global registration of object stores
```

Because `update_datafusion_session` will not overwrite an existing object-store mapping, use a fresh `SessionContext` when switching between prod S3, MinIO, LocalStack, or tenant-specific credential domains. ([Docs.rs][1])

## 7.2.4 Custom runtime with memory/spill posture

```rust id="v6z2tv"
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::memory_pool::GreedyMemoryPool;
use datafusion::prelude::*;
use std::sync::Arc;

pub fn session_with_memory_limit(bytes: usize) -> anyhow::Result<SessionContext> {
    let runtime = RuntimeEnvBuilder::new()
        .with_memory_pool(Arc::new(GreedyMemoryPool::new(bytes)))
        .build()?;

    Ok(SessionContext::new_with_config_rt(
        SessionConfig::new(),
        Arc::new(runtime),
    ))
}
```

`RuntimeEnvBuilder` can create a runtime with a custom memory pool, and `RuntimeEnv` owns the disk manager used for temporary files during execution. ([Docs.rs][5])

## 7.2.5 Delta spill helper

```rust id="bqr3yh"
use datafusion::prelude::*;
use deltalake::delta_datafusion::create_session_state_with_spill_config;

pub fn delta_session_with_spill(
    max_spill_size: Option<usize>,
    max_temp_directory_size: Option<u64>,
) -> SessionContext {
    let state = create_session_state_with_spill_config(
        max_spill_size,
        max_temp_directory_size,
    );

    SessionContext::new_with_state(state)
}
```

`create_session_state_with_spill_config` creates a `SessionState` with optional spill-to-disk configuration; when either spill parameter is set, a `FairSpillPool` memory pool and sized disk manager are wired into the runtime so DataFusion can spill intermediate results instead of running out of memory. ([Docs.rs][6])

## 7.2.6 `DeltaSessionContext`

```rust id="dyak03"
use datafusion::prelude::SessionContext;
use deltalake::delta_datafusion::DeltaSessionContext;

pub fn new_delta_session_context() -> SessionContext {
    DeltaSessionContext::new().into_inner()
}
```

`DeltaSessionContext` wraps DataFusion’s `SessionContext` with Delta-specific defaults, including case-sensitive identifiers and Delta planner configuration; it can be created with a default runtime or with a custom `RuntimeEnv`. ([Docs.rs][7])

## 7.2.7 Registering many Delta roots

```rust id="vbpwx9"
use datafusion::prelude::*;
use std::collections::HashMap;

pub async fn register_many_delta_roots(
    ctx: &SessionContext,
    name_to_uri: &HashMap<String, String>,
) -> anyhow::Result<HashMap<String, u64>> {
    let mut loaded = HashMap::new();

    for (name, uri) in name_to_uri {
        let table = deltalake::open_table(url::Url::parse(uri)?).await?;
        let version = table.version().ok_or_else(|| anyhow::anyhow!("no version"))?;

        let state = ctx.state();
        table.update_datafusion_session(&state)?;

        ctx.register_table(name, table.table_provider().await?)?;
        loaded.insert(name.clone(), version);
    }

    Ok(loaded)
}
```

Registering many tables should be batched at service startup, workspace activation, or query compilation time, not repeatedly per row or inside a tight execution loop.

---

# 7.3 SQL API path

## 7.3.1 Basic SQL execution

```rust id="nn4nyu"
use datafusion::prelude::*;

pub async fn collect_sql(
    ctx: &SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let df = ctx.sql(sql).await?;
    let batches = df.collect().await?;
    Ok(batches)
}
```

`SessionContext` executes SQL queries and creates DataFrames over registered tables. DataFrames are lazy: operations build plans, while `collect()` executes the plan and returns Arrow `RecordBatch`es. ([Docs.rs][4])

## 7.3.2 Pushdown-friendly SQL

```sql id="b3vso3"
SELECT
  simulation_id,
  run_date,
  unit_id,
  output_value
FROM simulation_outputs
WHERE run_date = '2026-06-02'
  AND output_value > 0.0
```

Preferred:

```text id="cyrqsl"
explicit projection
simple comparisons
direct column predicates
partition-column filters
filter columns with Delta stats
stable aliases
qualified join columns
```

Avoid:

```text id="6h59x4"
SELECT *
functions wrapping filter columns
unqualified duplicate column names
implicit casts on partition/filter columns
case-ambiguous identifiers
non-deterministic filters where pruning is expected
```

Delta’s DataFusion guide explains that Delta stores file-level metadata in the transaction log, allowing entire-file skipping before Parquet row-group skipping; DataFusion’s config also exposes Parquet pruning and page-index settings. ([Delta IO][8])

## 7.3.3 `EXPLAIN` and `EXPLAIN ANALYZE`

```rust id="qn7qrv"
pub async fn explain_verbose(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    Ok(ctx
        .sql(&format!("EXPLAIN VERBOSE {sql}"))
        .await?
        .collect()
        .await?)
}

pub async fn explain_analyze_verbose(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    Ok(ctx
        .sql(&format!("EXPLAIN ANALYZE VERBOSE {sql}"))
        .await?
        .collect()
        .await?)
}
```

DataFusion’s SQL reference states that `EXPLAIN` shows logical and physical execution plans and supports `EXPLAIN [ANALYZE] [VERBOSE] [FORMAT format] statement`. ([datafusion.apache.org][9])

Plan validation checklist:

```text id="fvt4ej"
scan:
  Delta provider / Delta scan path appears
  not raw ungoverned Parquet directory unless intentional

projection:
  scan schema reduced to required columns

filter:
  predicate visible near scan
  no avoidable post-scan-only filters

partition:
  partition filter visible
  selected files/partitions reduced when diagnostics available

join:
  expected join type
  expected join side
  no accidental cartesian join

sort/aggregate/window:
  spill-sensitive operators identified
```

## 7.3.4 SQL compatibility limitations

```text id="er1pmu"
Treat SQL compatibility as DataFusion SQL compatibility, not Spark SQL compatibility.
Do not assume all Databricks/Spark SQL syntax compiles in DataFusion.
Compile generated SQL with ctx.sql(sql).await before exposing as canonical recipe.
Prefer DataFrame Expr generation for application-generated filters.
```

## 7.3.5 Identifier escaping

```sql id="xsrkkk"
SELECT
  "MixedCaseColumn",
  "column-with-dash"
FROM "delta_table"
```

New table policy:

```text id="1f0x7l"
lowercase snake_case columns
no spaces
no punctuation
no case-only duplicates
no reserved keyword identifiers
```

Delta has a `DeltaColumn` wrapper to preserve case sensitivity during string conversion, and `DeltaSessionContext` is documented as using case-sensitive Delta defaults. ([Docs.rs][10])

## 7.3.6 UDF compatibility over Delta tables

```rust id="1wg4v6"
use datafusion::prelude::*;

pub async fn query_with_registered_udf(ctx: &SessionContext) -> anyhow::Result<()> {
    // Register scalar UDFs/UDAFs/window UDFs before planning SQL.
    // ctx.register_udf(...);

    let _batches = ctx
        .sql(
            r#"
            SELECT simulation_id, my_udf(output_value) AS adjusted
            FROM simulation_outputs
            WHERE run_date = '2026-06-02'
            "#,
        )
        .await?
        .collect()
        .await?;

    Ok(())
}
```

DataFusion UDFs operate above the table-provider layer, so they can be used over Delta tables after provider registration; but UDF-wrapped predicates often reduce pushdown/skipping opportunities unless rewritten into pushdown-friendly primitive comparisons.

---

# 7.4 DataFrame API path

## 7.4.1 Direct `read_table`

```rust id="9fdf86"
use datafusion::prelude::*;
use deltalake::open_table;
use url::Url;

pub async fn dataframe_read_table(uri: &str) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let table = open_table(Url::parse(uri)?).await?;
    let ctx = SessionContext::new();

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let df = ctx
        .read_table(table.table_provider().await?)?
        .filter(col("id").gt(lit(100)))?
        .select(vec![col("id"), col("value")])?;

    Ok(df.collect().await?)
}
```

DataFusion’s DataFrame docs describe DataFrames as logical row sets built through operations like `filter`, `select`, `aggregate`, and `limit`, then executed with `collect()`. ([Docs.rs][11])

## 7.4.2 Registered table path

```rust id="y5q8qc"
use datafusion::prelude::*;

pub async fn dataframe_registered_table(
    ctx: &SessionContext,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let df = ctx
        .table("simulation_outputs")
        .await?
        .filter(
            col("run_date")
                .eq(lit("2026-06-02"))
                .and(col("output_value").gt(lit(0.0))),
        )?
        .select(vec![
            col("simulation_id"),
            col("unit_id"),
            col("output_value"),
        ])?;

    Ok(df.collect().await?)
}
```

Use `ctx.table(...)` after registration when many queries share the same provider name; use `ctx.read_table(provider)` for direct short-lived provider use.

## 7.4.3 Safe expression generation from UI/config

```rust id="aztq4d"
use datafusion::prelude::*;

#[derive(Debug, Clone)]
pub enum UiFilter {
    RunDateEquals(String),
    OutputGreaterThan(f64),
    UnitIn(Vec<String>),
}

pub fn compile_filter(filters: &[UiFilter]) -> datafusion::error::Result<Expr> {
    let mut expr = lit(true);

    for filter in filters {
        let next = match filter {
            UiFilter::RunDateEquals(v) => col("run_date").eq(lit(v.clone())),
            UiFilter::OutputGreaterThan(v) => col("output_value").gt(lit(*v)),
            UiFilter::UnitIn(values) => {
                let values = values.iter().cloned().map(lit).collect::<Vec<_>>();
                col("unit_id").in_list(values, false)
            }
        };

        expr = expr.and(next);
    }

    Ok(expr)
}
```

Safe expression compiler rules:

```text id="hfu4xs"
allowlist columns
allowlist operators
type-check literals before expression construction
reject raw SQL fragments
map UI fields to canonical physical columns
preserve pushdown-friendly forms
emit diagnostics with compiled Expr Debug output
```

## 7.4.4 Compile-time query construction pattern

```rust id="pvsspm"
pub struct SimulationQuery {
    pub run_date: String,
    pub min_output_value: Option<f64>,
    pub columns: Vec<String>,
}

pub async fn run_simulation_query(
    ctx: &datafusion::prelude::SessionContext,
    q: SimulationQuery,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    use datafusion::prelude::*;

    let mut df = ctx.table("simulation_outputs").await?;
    df = df.filter(col("run_date").eq(lit(q.run_date)))?;

    if let Some(min) = q.min_output_value {
        df = df.filter(col("output_value").gt_eq(lit(min)))?;
    }

    let projection = q.columns.into_iter().map(col).collect::<Vec<_>>();
    df = df.select(projection)?;

    Ok(df.collect().await?)
}
```

This is the preferred shape for your simulation UI: configuration maps to a constrained expression AST, not arbitrary SQL text.

---

# 7.5 DataFusion expressions inside Delta operations

## 7.5.1 Expression imports

```rust id="jc8o5y"
use deltalake::datafusion::logical_expr::{col, lit};
```

The `deltalake` crate re-exports DataFusion modules, and delta-rs uses DataFusion to provide SQL-related Delta features such as update/merge expressions and constraints. ([Docs.rs][12])

## 7.5.2 Delete predicate

```rust id="palzo0"
use deltalake::datafusion::logical_expr::{col, lit};

pub async fn delete_negative_outputs(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .delete()
        .with_predicate(col("output_value").lt(lit(0.0)))
        .await?;

    Ok(table)
}
```

Delta-rs docs note that delete predicates must be deterministic; omitting a predicate deletes all records. The DataFusion guide also states that DataFusion is used for SQL-related Delta features including update and merge expressions. ([Delta IO][8])

## 7.5.3 Replace-where predicate

```rust id="bopirg"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::protocol::SaveMode;

pub async fn replace_run_date(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
    run_date: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Overwrite)
        .with_replace_where(col("run_date").eq(lit(run_date.to_owned())))
        .await?;

    Ok(table)
}
```

Use expressions for programmatic predicates and SQL strings for user-facing / declarative config only after parsing/validation. Programmatic `Expr` generation is safer for UI-derived filters because allowed columns, literal types, and operators can be enforced before planning.

## 7.5.4 Update predicate and assignments

```rust id="m4wxyu"
use deltalake::datafusion::logical_expr::{col, lit};

pub async fn mark_failed_runs(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .update()
        .with_predicate(col("status").eq(lit("FAILED")))
        .with_update("needs_review", lit(true))?
        .await?;

    Ok(table)
}
```

Treat this as syntax to compile against your exact dependency set: update-builder method names are more likely to change than simple scan/provider APIs, so all DML sections should be CI-compiled before promotion.

## 7.5.5 Merge predicate

```rust id="wqi2h0"
use deltalake::datafusion::logical_expr::{col, lit};

pub async fn merge_updates(
    target: deltalake::DeltaTable,
    source: datafusion::dataframe::DataFrame,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = target
        .merge(
            source,
            col("target.run_id").eq(col("source.run_id")),
        )
        .with_target_alias("target")
        .with_source_alias("source")
        .when_matched_update(|u| {
            u.update("output_value", col("source.output_value"))
        })
        .when_not_matched_insert(|i| {
            i.set("run_id", col("source.run_id"))
             .set("output_value", col("source.output_value"))
        })
        .await?;

    Ok(table)
}
```

## 7.5.6 Null and type semantics

```text id="793cwk"
NULL = unknown, not false.
col("x").eq(lit(None)) is not equivalent to IS NULL.
Use explicit null predicates where available.
Avoid implicit casts in partition predicates.
Normalize literal types before Expr generation.
```

Agent diagnostics:

```text id="v92u6i"
Predicate rejected:
  check determinism
  check column existence
  check case sensitivity
  check type coercion
  check null semantics
  check whether expression was built from SQL string or Expr
```

---

# 7.6 Arrow batch interoperability

## 7.6.1 Basic `RecordBatch`

```rust id="x6sq96"
use std::sync::Arc;

use arrow_array::{Float64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};

pub fn make_simulation_batch() -> anyhow::Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("simulation_id", DataType::Utf8, false),
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("output_value", DataType::Float64, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["sim-001", "sim-001"])),
            Arc::new(StringArray::from(vec!["unit-a", "unit-b"])),
            Arc::new(Float64Array::from(vec![Some(1.25), None])),
        ],
    )?;

    Ok(batch)
}
```

Delta writes accept Arrow `RecordBatch` input through `DeltaTable::write`, and DataFusion execution returns Arrow `RecordBatch` outputs through `collect()`. ([Docs.rs][1])

## 7.6.2 Null arrays

```rust id="2gktg4"
use arrow_array::{new_null_array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

pub fn null_metric_batch() -> anyhow::Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("output_value", DataType::Float64, true),
    ]));

    let values = new_null_array(&DataType::Float64, 1024);

    Ok(RecordBatch::try_new(schema, vec![values])?)
}
```

Use null arrays for placeholder columns only when schema is intentional and downstream null semantics are documented.

## 7.6.3 Decimal arrays

```rust id="i1lj6b"
use arrow_array::{Decimal128Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

pub fn decimal_batch() -> anyhow::Result<RecordBatch> {
    let values = Decimal128Array::from(vec![Some(12345_i128), Some(67890_i128)])
        .with_precision_and_scale(18, 2)?;

    let schema = Arc::new(Schema::new(vec![
        Field::new("price", DataType::Decimal128(18, 2), true),
    ]));

    Ok(RecordBatch::try_new(schema, vec![Arc::new(values)])?)
}
```

Decimal policy:

```text id="bjw0zp"
always specify precision and scale
do not use Float64 for financial/accounting quantities
round/cast before batch creation
test Spark/PyArrow/DataFusion round trips
```

## 7.6.4 Timestamp arrays

```rust id="i5w7ps"
use arrow_array::{RecordBatch, TimestampMicrosecondArray};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use std::sync::Arc;

pub fn timestamp_batch() -> anyhow::Result<RecordBatch> {
    let ts = TimestampMicrosecondArray::from(vec![
        Some(1_717_300_000_000_000_i64),
        Some(1_717_300_060_000_000_i64),
    ]);

    let schema = Arc::new(Schema::new(vec![
        Field::new(
            "event_ts",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            true,
        ),
    ]));

    Ok(RecordBatch::try_new(schema, vec![Arc::new(ts)])?)
}
```

Timestamp policy:

```text id="8t2l5f"
standardize time unit
standardize timezone strategy
avoid mixing ns/us/ms across producers
test stats-based skipping on timestamp predicates
```

## 7.6.5 Dictionary arrays

```rust id="8fzcex"
use arrow_array::{DictionaryArray, RecordBatch, StringArray};
use arrow_array::types::Int32Type;
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

pub fn dictionary_batch() -> anyhow::Result<RecordBatch> {
    let raw = StringArray::from(vec![
        Some("base"),
        Some("stress"),
        Some("base"),
    ]);

    let dict: DictionaryArray<Int32Type> =
        raw.try_into()?;

    let schema = Arc::new(Schema::new(vec![
        Field::new(
            "scenario_family",
            DataType::Dictionary(
                Box::new(DataType::Int32),
                Box::new(DataType::Utf8),
            ),
            true,
        ),
    ]));

    Ok(RecordBatch::try_new(schema, vec![Arc::new(dict)])?)
}
```

Dictionary encoding is useful for low-cardinality string columns, but test compatibility across DataFusion planning, Delta writes, Parquet encoding, and external readers before standardizing dictionary arrays as a write boundary.

## 7.6.6 Batch chunking

```rust id="5d5uh2"
pub fn validate_batches(batches: &[arrow_array::RecordBatch]) -> anyhow::Result<()> {
    anyhow::ensure!(!batches.is_empty(), "empty write");

    let schema = batches[0].schema();

    for (idx, batch) in batches.iter().enumerate() {
        anyhow::ensure!(
            batch.schema().as_ref() == schema.as_ref(),
            "batch {idx} schema mismatch"
        );
    }

    Ok(())
}
```

Batching guidance:

```text id="rwzg8v"
avoid:
  one row per batch
  one tiny batch per commit
  mixed schemas within one write
  unbounded monster batches

prefer:
  stable schema per write
  moderate batch size
  coalesced micro-batches
  target-file-size-aware ingestion
  explicit write_batch_size in Delta writer when tuned
```

## 7.6.7 Avoiding Arrow version skew

```bash id="gvw7kf"
cargo tree -d
cargo tree -i arrow-array
cargo tree -i arrow-schema
cargo tree -i datafusion
cargo tree -i deltalake
```

Because Arrow `RecordBatch`, `Schema`, and array types are concrete Rust crate types, duplicate Arrow crate versions can produce trait/type errors that look like “expected RecordBatch, found RecordBatch.” Pin Arrow, Parquet, DataFusion, and `object_store` to the Delta baseline for documentation and CI.

---

# 7.7 Writing from DataFusion plans

## 7.7.1 Query-result-to-Delta pipeline

```rust id="hhx1tz"
use std::sync::Arc;

use arrow_array::RecordBatch;
use datafusion::prelude::*;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn materialize_query_to_delta(
    ctx: &SessionContext,
    target_uri: &str,
    sql: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let target = deltalake::open_table(Url::parse(target_uri)?).await?;

    let df = ctx.sql(sql).await?;
    let (session_state, logical_plan) = df.into_parts();

    let table = target
        .write(Vec::<RecordBatch>::new())
        .with_input_plan(logical_plan)
        .with_session_state(Arc::new(session_state))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_save_mode(SaveMode::Append)
        .await?;

    Ok(table)
}
```

`WriteBuilder::with_input_plan` accepts a DataFusion `LogicalPlan`; `with_session_state` sets the DataFusion session used for planning/execution; and strict session handling is controlled by `SessionFallbackPolicy::RequireSessionState`. ([Docs.rs][13])

## 7.7.2 Why `SessionState` matters

```text id="p7cnzi"
LogicalPlan alone is insufficient for governed service execution:
  UDF registry
  catalog bindings
  object-store registry
  runtime config
  optimizer settings
  tenant/session policy
  time zone
  memory/spill/runtime settings
```

`WriteBuilder` docs state that the provided session should wrap a concrete `SessionState`; otherwise, the default policy warns and falls back to internal defaults unless strict fallback behavior is configured. ([Docs.rs][13])

## 7.7.3 CTAS-like materialization

```rust id="b28wyv"
pub async fn create_derived_delta_from_registered_sources(
    ctx: &datafusion::prelude::SessionContext,
    target_uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let sql = r#"
        SELECT
          simulation_id,
          run_date,
          unit_id,
          SUM(output_value) AS total_output
        FROM simulation_outputs
        WHERE run_date >= '2026-01-01'
        GROUP BY simulation_id, run_date, unit_id
    "#;

    materialize_query_to_delta(ctx, target_uri, sql).await
}
```

Use for:

```text id="u53j6n"
derived simulation result marts
materialized scenario summaries
agent-generated calculation outputs
validated transform pipelines
read-optimized tables
cross-table aggregation artifacts
```

## 7.7.4 Error boundaries

```text id="u1k2vk"
Planning errors:
  unresolved table
  unresolved column
  missing UDF
  invalid type coercion
  unsupported SQL

Execution errors:
  object-store read failure
  Parquet decode failure
  memory limit / spill limit
  UDF runtime error

Delta commit errors:
  transaction conflict
  schema mismatch
  object-store write failure
  lock failure
  unknown commit outcome
```

Recovery policy:

```text id="uypofa"
planning error:
  fail fast, return diagnostic, do not retry

execution error before commit:
  retry only if source reads are idempotent and transient

unknown commit outcome:
  reload target history/state
  reconcile operation identity
  never blind append retry

transaction conflict:
  reload target
  re-plan if query depends on target current state
  retry only with idempotent semantics
```

---

# 7.8 File skipping, pruning, and performance

## 7.8.1 Performance stack

```text id="kzvmz9"
SQL/DataFrame predicate
  -> DataFusion logical optimization
    -> projection pushdown
    -> filter pushdown
    -> Delta partition pruning
    -> Delta file skipping from transaction-log file stats
    -> Parquet row-group pruning
    -> Parquet page-index pruning, if available/enabled
    -> Arrow batch filtering/projection
```

Delta’s DataFusion integration guide explains that Delta can skip entire files using transaction-log file-level metadata, then skip row groups within individual files; DataFusion’s config documents Parquet pruning, page index, and filter-pushdown settings. ([Delta IO][8])

## 7.8.2 `DeltaScanConfig`

```rust id="adtm2x"
use deltalake::delta_datafusion::DeltaScanConfig;

pub fn scan_config_for_debug() -> DeltaScanConfig {
    DeltaScanConfig::new()
        .with_file_column_name("_delta_file_path")
        .with_wrap_partition_values(true)
        .with_parquet_pushdown(true)
}
```

`DeltaScanConfig` fields include `file_column_name`, `wrap_partition_values`, `enable_parquet_pushdown`, `schema_force_view_types`, and an optional schema. Its defaults include dictionary-wrapped partition values and enabled scan-filter pushdown; `with_schema` can use a compatible schema, for example string view types. ([Docs.rs][14])

## 7.8.3 Projection pruning

```sql id="xowr3j"
-- good
SELECT run_id, unit_id, output_value
FROM simulation_outputs
WHERE run_date = '2026-06-02';

-- bad default
SELECT *
FROM simulation_outputs
WHERE run_date = '2026-06-02';
```

Agent projection rule:

```text id="uxgu7u"
Never generate SELECT * for production analytical queries unless explicitly requested.
Always project the minimum output contract.
```

## 7.8.4 Partition pruning

```sql id="97uig5"
SELECT unit_id, output_value
FROM simulation_outputs
WHERE run_date = '2026-06-02'
  AND scenario_family = 'base_case'
```

Partitioning works best when common predicates hit low/medium-cardinality layout columns. Do not partition by unique `run_id`, nanosecond timestamps, continuous numeric values, or free-text fields unless query patterns prove it.

## 7.8.5 File skipping and stats

```text id="bqncyn"
file skipping works best when:
  files have min/max/null-count stats
  filters target stats-covered columns
  data is clustered by common predicates
  file sizes are controlled
  compaction reduces tiny-file count
  Z-order or equivalent layout aligns with query workload
```

Delta file skipping uses file-level statistics stored in the transaction log and is enhanced by partitioning, Z-ordering, and compaction when those layout choices match query patterns. ([Delta IO][15])

## 7.8.6 Small-file effects

```text id="6qkwnd"
too many tiny files cause:
  expensive transaction-log state
  expensive object-store metadata/list/get
  more scan tasks
  worse planning overhead
  lower Parquet row-group effectiveness
  higher compaction burden
```

Remediation:

```text id="o8sj6x"
batch writes
target larger files
schedule optimize/compaction
avoid one commit per row/object
partition by query patterns, not ingestion accident
benchmark after compaction
```

## 7.8.7 Target partitions

DataFusion’s `datafusion.execution.target_partitions` controls the number of execution partitions and defaults to CPU core count when set to `0`. Increasing partitions can increase concurrency, but excessive partitions can increase scheduling overhead. ([datafusion.apache.org][16])

```rust id="6trxrh"
use datafusion::prelude::*;

pub fn context_with_target_partitions(n: usize) -> SessionContext {
    let config = SessionConfig::new()
        .with_target_partitions(n);

    SessionContext::new_with_config(config)
}
```

Tuning guidance:

```text id="x8npx1"
increase:
  many files
  object-store latency
  CPU underutilization
  independent scan-heavy workload

decrease:
  too many tiny tasks
  memory pressure
  excessive spill
  small tables
  high coordination overhead
```

## 7.8.8 Runtime metrics

```rust id="e27vg7"
pub async fn explain_analyze(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    Ok(ctx
        .sql(&format!("EXPLAIN ANALYZE VERBOSE {sql}"))
        .await?
        .collect()
        .await?)
}
```

`EXPLAIN ANALYZE` runs the query and prints an annotated physical plan with execution metrics; the underlying `Analyze` logical node is documented as running the actual plan and printing the physical plan with metrics. ([Docs.rs][17])

## 7.8.9 Benchmark harness

```rust id="5y1j0g"
use std::time::Instant;

pub async fn benchmark_query(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<(usize, u128)> {
    let start = Instant::now();
    let batches = ctx.sql(sql).await?.collect().await?;
    let elapsed_ms = start.elapsed().as_millis();

    let rows = batches.iter().map(|b| b.num_rows()).sum();

    Ok((rows, elapsed_ms))
}
```

Benchmark dimensions:

```text id="7682xa"
query shape:
  projection width
  partition filter presence
  stats-column filter presence
  joins
  aggregates
  sorts

layout:
  file size
  file count
  row-group size
  partitioning
  Z-order / clustering
  stats availability

runtime:
  target_partitions
  memory limit
  spill enabled
  object-store endpoint latency
  cache behavior
```

---

# 7.9 End-to-end service skeleton

```rust id="e89ac2"
use datafusion::prelude::*;
use deltalake::delta_datafusion::create_session_state_with_spill_config;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeltaSource {
    pub name: String,
    pub uri: String,
    pub storage_options: HashMap<String, String>,
}

pub async fn build_query_context(
    sources: Vec<DeltaSource>,
    max_spill_memory_bytes: Option<usize>,
    max_spill_disk_bytes: Option<u64>,
) -> anyhow::Result<(SessionContext, HashMap<String, u64>)> {
    let state = create_session_state_with_spill_config(
        max_spill_memory_bytes,
        max_spill_disk_bytes,
    );

    let ctx = SessionContext::new_with_state(state);
    let mut versions = HashMap::new();

    for source in sources {
        let table = deltalake::open_table_with_storage_options(
            url::Url::parse(&source.uri)?,
            source.storage_options,
        )
        .await?;

        let version = table
            .version()
            .ok_or_else(|| anyhow::anyhow!("Delta table has no loaded version: {}", source.name))?;

        let state = ctx.state();
        table.update_datafusion_session(&state)?;

        ctx.register_table(&source.name, table.table_provider().await?)?;

        versions.insert(source.name, version);
    }

    Ok((ctx, versions))
}

pub async fn query_simulation_outputs(
    ctx: &SessionContext,
    run_date: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let df = ctx
        .table("simulation_outputs")
        .await?
        .filter(col("run_date").eq(lit(run_date.to_owned())))?
        .select(vec![
            col("simulation_id"),
            col("unit_id"),
            col("output_value"),
        ])?;

    Ok(df.collect().await?)
}
```

---

# 7.10 Production best practices

```text id="l9unwm"
TableProvider:
  - register Delta providers, not raw Parquet directories
  - use stable logical table names
  - rebuild provider after table refresh when latest correctness matters
  - expose file column only for trusted diagnostics

Runtime:
  - one coherent SessionContext per query domain
  - isolate tenants by context/catalog/credential domain
  - configure spill for large joins/aggregates/sorts
  - avoid stale object-store registry mappings
  - use fresh contexts for LocalStack/MinIO/prod switching

SQL:
  - explicit projections
  - pushdown-friendly predicates
  - EXPLAIN generated queries in tests
  - qualify join columns
  - quote identifiers only when necessary

DataFrame:
  - compile UI/config to Expr
  - allowlist columns and operators
  - type-check literals
  - prefer DataFrame for generated filters

Arrow:
  - validate schema equality across batches
  - pin Arrow/DataFusion versions
  - standardize decimals/timestamps
  - avoid tiny batches and tiny commits

Write from plans:
  - use df.into_parts
  - pass SessionState with with_session_state
  - use SessionFallbackPolicy::RequireSessionState
  - design idempotent commit/retry semantics

Performance:
  - partition by common filters
  - compact small files
  - benchmark target_partitions
  - preserve useful stats
  - use EXPLAIN ANALYZE for metrics
```

---

# 7.11 Anti-patterns

```text id="eerjyp"
- registering a Delta table path as raw Parquet
- assuming table_provider auto-refreshes after new commits
- calling update_datafusion_session and expecting bad mappings to be overwritten
- SELECT * in generated production queries
- string-concatenating user filters into SQL
- using UDF-wrapped predicates where pushdown is expected
- mixing different Arrow crate versions
- using LogicalPlan without its originating SessionState
- accepting SessionFallbackPolicy::InternalDefaults silently in production writes
- exposing _delta_file_path to untrusted users
- one SessionContext shared across tenants with different credentials
- one tiny RecordBatch per Delta commit
```

---

# 7.12 LLM-agent checklist

```text id="cz03md"
Before generating DataFusion + Delta read code:
  1. Confirm deltalake feature includes "datafusion".
  2. Open Delta table with url::Url.
  3. Capture loaded Delta version.
  4. Create/reuse SessionContext.
  5. Get ctx.state().
  6. Call table.update_datafusion_session(&state).
  7. Build provider with table.table_provider().await?.
  8. Register provider under stable semantic name.
  9. Use SQL or DataFrame path intentionally.
  10. Use explicit projection.
  11. Use pushdown-friendly filters.
  12. Use EXPLAIN / EXPLAIN ANALYZE for diagnostics.
  13. Rebuild provider after refresh if freshness matters.
  14. Log table versions with query events.

Before generating DataFusion-plan write code:
  1. Build DataFrame from SQL/DataFrame API.
  2. Call df.into_parts().
  3. Pass logical_plan to with_input_plan.
  4. Pass Arc::new(session_state) to with_session_state.
  5. Set SessionFallbackPolicy::RequireSessionState.
  6. Set SaveMode explicitly.
  7. Design idempotency/retry boundary.
```

---

# 7.13 Value case

A properly engineered DataFusion + Arrow + Delta integration layer gives your Rust simulation workbench:

```text id="cxbq8k"
transactionally correct Delta table reads
Arrow-native execution and interchange
DataFusion SQL and DataFrame programmability
safe UI/config-to-query compilation
version-aware reproducibility
partition pruning and file skipping
DataFusion-native joins across Delta, MemTable, CSV, Parquet, and custom providers
runtime control over memory, spill, object stores, catalogs, and UDFs
query-result materialization back into Delta
LLM-agent-safe syntax patterns with explicit freshness and dependency boundaries
```

Core invariant:

```text id="b67jzf"
Delta controls table state; DataFusion controls query execution; Arrow controls batch representation.
```

Operational invariant:

```text id="8q7h0l"
A query is only correct when its provider snapshot, session runtime, object-store registry, schema assumptions, and freshness policy are all explicit.
```

[1]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/?utm_source=chatgpt.com "datafusion - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.TableProviderBuilder.html "TableProviderBuilder in deltalake::delta_datafusion - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/execution/runtime_env/struct.RuntimeEnv.html "RuntimeEnv in datafusion::execution::runtime_env - Rust"
[6]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/fn.create_session_state_with_spill_config.html "create_session_state_with_spill_config in deltalake::delta_datafusion - Rust"
[7]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaSessionContext.html "DeltaSessionContext in deltalake::delta_datafusion - Rust"
[8]: https://delta-io.github.io/delta-rs/integrations/delta-lake-datafusion/ "DataFusion - Delta Lake Documentation"
[9]: https://datafusion.apache.org/user-guide/sql/explain.html "EXPLAIN — Apache DataFusion  documentation"
[10]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaColumn.html?utm_source=chatgpt.com "DeltaColumn in deltalake::delta_datafusion - Rust"
[11]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html "DataFrame in datafusion::dataframe - Rust"
[12]: https://docs.rs/deltalake/latest/deltalake/?utm_source=chatgpt.com "deltalake - Rust"
[13]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html "WriteBuilder in deltalake::operations::write - Rust"
[14]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaScanConfig.html "DeltaScanConfig in deltalake::delta_datafusion - Rust"
[15]: https://delta-io.github.io/delta-rs/how-delta-lake-works/delta-lake-file-skipping/?utm_source=chatgpt.com "File skipping - Delta Lake Documentation"
[16]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[17]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.Analyze.html?utm_source=chatgpt.com "Analyze in datafusion::logical_expr - Rust"


# 8. Create-table workflows — Rust `deltalake`

Version target:

```text id="2upcmu"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required feature for DataFusion-backed creation: ["datafusion"]
Typical production feature set: ["rustls", "datafusion", "s3"]
```

Current API posture: use `CreateBuilder` or `DeltaTable::create()` for explicit table creation; use `DeltaTable::write(...)` when creation is naturally driven by Arrow batches or DataFusion plans. `DeltaOps` remains visible but is deprecated in current docs, with the preferred pattern being methods directly on `DeltaTable`, such as `delta_table.create()`. `CreateBuilder` awaits to `Result<DeltaTable, DeltaTableError>`. ([Docs.rs][1])

---

## 8.1 Creation mental model

```text id="a49eba"
Delta table creation path:
  create _delta_log/00000000000000000000.json
  write protocol action
  write metadata action
  optionally write configuration/table properties
  optionally declare partition columns
  optionally declare schema fields
  optionally include commit properties
  no Parquet data files required for an empty table

Data bootstrap path:
  create table metadata first
  then append Arrow/DataFusion-produced data
  or write data directly into a not-yet-initialized DeltaTable target

Parquet conversion path:
  scan existing Parquet files
  infer/declare schema and partitions
  create Delta transaction log tracking existing Parquet files
  do not rewrite data files unless separate compaction/rewrite is later performed
```

Creation is a **metadata transaction**, not necessarily a data write. A “blank” Delta table still needs a protocol, metadata, and normally a schema; a “schema-less table” is not a useful production contract.

---

## 8.2 Primary construction APIs

### 8.2.1 Explicit creation with `CreateBuilder`

```rust id="yy6n5u"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

let table = CreateBuilder::new()
    .with_location("s3://bucket/events")
    .with_table_name("events")
    .with_save_mode(SaveMode::ErrorIfExists)
    .with_columns(schema.fields().cloned())
    .await?;
```

`CreateBuilder` supports table location, save mode, table name, table comment, columns, partition columns, configuration, configuration properties, storage options, commit properties, custom execution hooks, and low-level actions. ([Docs.rs][2])

### 8.2.2 Explicit creation from a `DeltaTable` handle

```rust id="f9smp2"
use deltalake::{DeltaTable, DeltaTableBuilder};
use deltalake::kernel::{DataType, StructField};
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn create_from_table_handle(uri: &str) -> anyhow::Result<DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .build()?;

    let table = table
        .create()
        .with_table_name("events")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            StructField::not_null("id", DataType::LONG),
            StructField::nullable("event_type", DataType::STRING),
        ])
        .await?;

    Ok(table)
}
```

Use this when the service already owns a `DeltaTable` lifecycle abstraction, storage options, log store, or runtime setup.

### 8.2.3 Create by writing data

```rust id="z7rkmd"
use deltalake::DeltaTableBuilder;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn create_by_first_write(
    uri: &str,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .build()?;

    let table = table
        .write(batches)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_description("Canonical simulation output facts")
        .with_partition_columns(["run_date"])
        .await?;

    Ok(table)
}
```

Use create-by-write when the Arrow schema is already the source of truth. Use explicit `CreateBuilder` when schema, metadata, table properties, and governance must exist before first data arrives.

---

## 8.3 Empty table creation

```rust id="m2yczl"
use deltalake::kernel::{DataType, StructField};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_empty_events_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("events")
        .with_comment("Event facts; initially empty")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            StructField::not_null("id", DataType::LONG),
            StructField::not_null("event_ts", DataType::TIMESTAMP),
            StructField::nullable("event_type", DataType::STRING),
            StructField::nullable("payload_json", DataType::STRING),
        ])
        .await?;

    Ok(table)
}
```

`StructField` carries name, data type, nullability, and metadata. `DataType` supports primitive, array, map, struct, and variant categories, with constants such as `STRING`, `LONG`, `INTEGER`, `DOUBLE`, `BOOLEAN`, `BINARY`, `DATE`, `TIMESTAMP`, and `TIMESTAMP_NTZ`. ([Docs.rs][3])

Operational rule:

```text id="ygsp7g"
Empty table != unconstrained table.
Empty table = schema/protocol/metadata exists, active data file set is empty.
```

---

## 8.4 Create table with schema

### 8.4.1 Primitive schema

```rust id="3d0u1u"
use deltalake::kernel::{DataType, StructField};

pub fn simulation_output_fields() -> Vec<StructField> {
    vec![
        StructField::not_null("simulation_id", DataType::STRING),
        StructField::not_null("run_id", DataType::STRING),
        StructField::not_null("run_date", DataType::DATE),
        StructField::not_null("unit_id", DataType::STRING),
        StructField::nullable("output_value", DataType::DOUBLE),
        StructField::nullable("output_unit", DataType::STRING),
        StructField::not_null("created_at", DataType::TIMESTAMP),
    ]
}
```

### 8.4.2 Decimal schema

```rust id="j2kahi"
use deltalake::kernel::{DataType, StructField};

pub fn financial_fields() -> anyhow::Result<Vec<StructField>> {
    Ok(vec![
        StructField::not_null("account_id", DataType::STRING),
        StructField::nullable("amount_usd", DataType::decimal(18, 2)?),
        StructField::not_null("posting_date", DataType::DATE),
    ])
}
```

### 8.4.3 Create with schema function

```rust id="0l41hl"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_simulation_outputs(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Canonical simulation output facts")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .await?;

    Ok(table)
}
```

Best practice:

```text id="wwkltm"
Schema source of truth:
  one Rust function/module per canonical table
  stable column ordering
  explicit nullability
  explicit decimal precision/scale
  explicit timestamp policy
  no inferred production schema without approval
```

---

## 8.5 Column metadata and field governance

```rust id="vvxttm"
use deltalake::kernel::{DataType, StructField};
use serde_json::json;
use std::collections::HashMap;

pub fn field_with_metadata() -> StructField {
    let mut metadata = HashMap::new();
    metadata.insert("semantic_type".to_owned(), json!("simulation_identifier"));
    metadata.insert("owner".to_owned(), json!("simulation_engine"));
    metadata.insert("governance.required".to_owned(), json!(true));

    StructField::not_null("simulation_id", DataType::STRING)
        .with_metadata(metadata)
}
```

`StructField` supports metadata via `with_metadata` and `add_metadata`, and `CreateBuilder::with_column` can also accept column metadata directly. ([Docs.rs][3])

Governance guidance:

```text id="egug0i"
Use field metadata for:
  semantic type
  unit family
  source system
  data classification
  generated vs user-entered
  model-contract version
  UI display hints, if stable

Do not use field metadata for:
  secrets
  row-level access policy
  high-cardinality business values
  frequently changing runtime state
```

---

## 8.6 Table metadata

```rust id="htx44x"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_with_metadata(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Canonical, append-governed simulation output table")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .await?;

    Ok(table)
}
```

Delta table metadata includes an ID, optional name, optional description, partition columns, created time, schema, and a configuration map. ([Delta IO][4])

Metadata design:

```text id="rps2jr"
table_name:
  stable logical name
  not storage path
  not tenant secret
  not deployment environment unless table itself is environment-scoped

comment:
  human-readable purpose
  owner/domain
  mutability contract
  not operational logs

configuration:
  table properties
  protocol-affecting features
  retention/features/governance flags
```

---

## 8.7 Partitioned table creation

```rust id="zs7o03"
use deltalake::kernel::{DataType, StructField};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_partitioned_simulation_outputs(
    uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Simulation outputs partitioned by run_date and scenario_family")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            StructField::not_null("simulation_id", DataType::STRING),
            StructField::not_null("run_id", DataType::STRING),
            StructField::not_null("run_date", DataType::DATE),
            StructField::not_null("scenario_family", DataType::STRING),
            StructField::not_null("unit_id", DataType::STRING),
            StructField::nullable("output_value", DataType::DOUBLE),
        ])
        .with_partition_columns(["run_date", "scenario_family"])
        .await?;

    Ok(table)
}
```

`CreateBuilder::with_partition_columns` declares table partitioning at creation. For write-created tables, `WriteBuilder::with_partition_columns` applies partitioning to new tables, while existing tables must match current partition columns except for narrow full-overwrite/schema-overwrite cases. ([Docs.rs][2])

Partition design:

```text id="7m8dyl"
Good partition keys:
  run_date
  scenario_family
  tenant_id, when credential/authorization isolation aligns
  model_version, if queries commonly filter by it

Bad partition keys:
  unique run_id
  timestamp to second/millisecond
  floating-point parameter value
  high-cardinality string
  free-text user label
```

---

## 8.8 Table properties

### 8.8.1 CDF-enabled table

```rust id="wlgt8l"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_cdf_enabled_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Simulation outputs with Change Data Feed enabled from version 0")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

Change Data Feed is not enabled by default; it must be enabled explicitly, and only changes made after enabling CDF are captured. The delta-rs CDF docs also show `delta.enableChangeDataFeed` being set during table creation before later scanning CDF ranges. ([Delta Lake][5])

Use CDF for:

```text id="ya9vxi"
downstream incremental sync
audit/event capture
derived materialization refresh
CDC-like state propagation
simulation-output change tracking
```

CDF caveat:

```text id="70vi0x"
Enable CDF before first relevant write.
Do not assume historical changes before CDF enablement are reconstructible as CDF rows.
```

### 8.8.2 Append-only table

```rust id="59kji0"
pub async fn create_append_only_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_event_log")
        .with_comment("Append-only simulation event log")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            deltalake::kernel::StructField::not_null("event_id", deltalake::kernel::DataType::STRING),
            deltalake::kernel::StructField::not_null("event_ts", deltalake::kernel::DataType::TIMESTAMP),
            deltalake::kernel::StructField::nullable("event_type", deltalake::kernel::DataType::STRING),
        ])
        .with_configuration([
            ("delta.appendOnly", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

The table metadata configuration map can include `delta.appendOnly`; when true, the table is not meant to have data deleted from it. ([Delta IO][4])

Append-only use cases:

```text id="1rvluo"
event logs
raw ingestion history
simulation run audit records
immutable model outputs
append-only observability facts
```

Avoid append-only when:

```text id="dwz22n"
GDPR deletion workflow is required
row-level correction is expected
merge/update/delete workflows are normal
partition replacement is part of maintenance
```

### 8.8.3 CDF + append-only

```rust id="gh9cux"
pub async fn create_cdf_append_only_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_event_log")
        .with_save_mode(deltalake::protocol::SaveMode::ErrorIfExists)
        .with_columns([
            deltalake::kernel::StructField::not_null("event_id", deltalake::kernel::DataType::STRING),
            deltalake::kernel::StructField::not_null("event_ts", deltalake::kernel::DataType::TIMESTAMP),
            deltalake::kernel::StructField::nullable("event_type", deltalake::kernel::DataType::STRING),
        ])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
            ("delta.appendOnly", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

This is appropriate for immutable event streams where downstream consumers need incremental changes and the table should reject destructive semantics.

---

## 8.9 Storage options during creation

```rust id="xxgvfd"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use std::collections::HashMap;

pub async fn create_s3_table_with_options(
    uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let mut storage_options = HashMap::new();
    storage_options.insert("AWS_REGION".to_owned(), "us-east-1".to_owned());
    storage_options.insert("AWS_S3_LOCKING_PROVIDER".to_owned(), "dynamodb".to_owned());
    storage_options.insert("DELTA_DYNAMO_TABLE_NAME".to_owned(), "delta_log".to_owned());

    let table = CreateBuilder::new()
        .with_location(uri)
        .with_storage_options(storage_options)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .await?;

    Ok(table)
}
```

`CreateBuilder::with_storage_options` supplies object-store options for creation; the docs note that storage options can also come from environment variables. ([Docs.rs][2])

Production storage rule:

```text id="0i5200"
S3 production creation:
  require explicit region
  require locking provider for concurrent writers
  require least-privilege IAM
  avoid unsafe rename
  avoid credentials in logs

Local creation:
  use tempdir fixtures
  no cloud credentials
  no concurrent-writer assumptions
```

---

## 8.10 Save modes for creation/idempotency

```rust id="rupn3w"
use deltalake::protocol::SaveMode;

SaveMode::ErrorIfExists;
SaveMode::Ignore;
SaveMode::Overwrite;
SaveMode::Append;
```

Save-mode meaning for creation workflows:

```text id="v3xpjf"
ErrorIfExists:
  preferred production default
  fail if table/data already exists
  safe create-once contract

Ignore:
  bootstrap convenience
  no-op when target already exists
  useful for local/dev seed creation

Overwrite:
  destructive logical replacement
  use only for controlled create-or-replace
  dangerous in production unless scoped/approved

Append:
  not a primary explicit-create mode
  relevant for write-created/bootstrap data workflows
```

Delta save modes include append, overwrite, error-if-exists, and ignore; overwrite logically removes prior active data while preserving physical files until retention cleanup, enabling time travel until vacuum/retention removes old files. ([Delta Lake][6])

---

## 8.11 Create-or-replace patterns

### 8.11.1 Local/dev reset

```rust id="z3ghvp"
pub async fn recreate_dev_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("dev_simulation_outputs")
        .with_save_mode(SaveMode::Overwrite)
        .with_columns(simulation_output_fields())
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

### 8.11.2 Production create-once

```rust id="3o0fpo"
pub async fn create_prod_table_once(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

### 8.11.3 Production replace through new location + pointer swap

```text id="rsh6mk"
Preferred production replacement:
  1. create new table at versioned/staged location
  2. validate schema/protocol/properties
  3. validate row counts and query checks
  4. update catalog/pointer/metadata registry
  5. retire old location after retention window
```

Avoid blind `SaveMode::Overwrite` against a canonical production location unless the table is small, isolated, recoverable, and operationally locked.

---

## 8.12 Convert existing Parquet to Delta

```rust id="fmi1z0"
use deltalake::kernel::{DataType, StructField};
use deltalake::operations::convert_to_delta::{
    ConvertToDeltaBuilder,
    PartitionStrategy,
};
use deltalake::protocol::SaveMode;

pub async fn convert_hive_partitioned_parquet(
    uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = ConvertToDeltaBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_partition_strategy(PartitionStrategy::Hive)
        .with_partition_schema([
            StructField::not_null("run_date", DataType::DATE),
            StructField::not_null("scenario_family", DataType::STRING),
        ])
        .with_comment("Converted from existing Hive-partitioned Parquet dataset")
        .await?;

    Ok(table)
}
```

`convert_to_delta` converts a Parquet table to a Delta table in place. `ConvertToDeltaBuilder` supports location, storage options, partition schema, partition strategy, save mode, table name, comment, configuration, configuration properties, commit properties, and custom execution hooks. The documented `PartitionStrategy` is currently Hive partitioning. ([Docs.rs][7])

Conversion runbook:

```text id="n8ygbk"
Before conversion:
  freeze writers to Parquet location
  inventory file count and partition layout
  validate all files are compatible Parquet
  determine partition schema
  confirm no existing _delta_log unless intentional
  back up metadata/canonical pointer

During conversion:
  use ErrorIfExists by default
  specify Hive partition strategy when partitioned by path
  declare partition schema explicitly
  add table name/comment/configuration
  capture commit properties

After conversion:
  open Delta table
  inspect schema/protocol/metadata
  query through DataFusion Delta provider
  compare counts/checksums with raw Parquet
  block future raw Parquet writes into table path
```

Critical anti-pattern:

```text id="77f2fg"
Do not continue writing raw Parquet files into a converted Delta table directory.
All future writes must go through Delta transactions.
```

---

## 8.13 Bootstrapping from Arrow batches

### 8.13.1 Create metadata first, then append

```rust id="47r3s1"
use deltalake::protocol::SaveMode;

pub async fn bootstrap_from_arrow_batches(
    uri: &str,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .await?;

    Ok(table)
}
```

### 8.13.2 Direct create-by-write

```rust id="hukjqe"
use deltalake::DeltaTableBuilder;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn bootstrap_by_write(
    uri: &str,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let target = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .build()?;

    let table = target
        .write(batches)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_description("Created from first Arrow batch bootstrap")
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

`WriteBuilder` can create a new table from input batches and supports table name, description, configuration, partition columns, save mode, and other write settings. ([Docs.rs][8])

Selection rule:

```text id="ndz70h"
Use create-then-append when:
  metadata/properties must exist before first data
  schema is centrally governed
  CDF/append-only must be enabled at version 0
  permissions/checks separate DDL from DML

Use write-to-create when:
  Arrow batch schema is authoritative
  bootstrap is a one-shot materialization
  table is derived from a known data source
```

---

## 8.14 Bootstrapping from DataFusion query results

```rust id="1884et"
use std::sync::Arc;

use arrow_array::RecordBatch;
use datafusion::prelude::*;
use deltalake::DeltaTableBuilder;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn bootstrap_from_datafusion_query(
    ctx: &SessionContext,
    target_uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let df = ctx
        .sql(
            r#"
            SELECT
              simulation_id,
              run_id,
              run_date,
              unit_id,
              SUM(output_value) AS output_value
            FROM staging_simulation_outputs
            GROUP BY simulation_id, run_id, run_date, unit_id
            "#,
        )
        .await?;

    let (session_state, logical_plan) = df.into_parts();

    let target = DeltaTableBuilder::from_url(Url::parse(target_uri)?)?
        .build()?;

    let table = target
        .write(Vec::<RecordBatch>::new())
        .with_input_plan(logical_plan)
        .with_session_state(Arc::new(session_state))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_description("Created from DataFusion query result")
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

DataFusion `DataFrame` is lazy and wraps a logical plan plus session state; `WriteBuilder` supports `with_input_plan`, `with_session_state`, and session fallback policy, which are required for robust DataFusion-plan-to-Delta materialization. ([Docs.rs][9])

Production rules:

```text id="ltlwx8"
Always use df.into_parts().
Always pass SessionState with with_session_state.
Use SessionFallbackPolicy::RequireSessionState.
Register source object stores before planning/execution.
Set SaveMode explicitly.
Design target creation idempotency before running the query.
```

---

## 8.15 Local/dev/prod creation differences

```text id="h1cvpx"
Local/dev:
  local filesystem or tempdir
  SaveMode::Overwrite acceptable for fixture reset
  SaveMode::Ignore acceptable for seed bootstrap
  in-memory tables acceptable for unit tests only
  unsafe cloud options only in isolated tests

CI:
  tempdir for fast unit/integration tests
  MinIO/LocalStack for object-store path
  deterministic table names/locations
  no shared production buckets
  no broad credentials

Production:
  SaveMode::ErrorIfExists default
  explicit storage options or workload identity
  S3 locking required for concurrent writers
  CDF/append-only/retention properties reviewed before creation
  no blind overwrite of canonical locations
  table registry/catalog pointer created after validation
```

`DeltaTable::new_in_memory()` creates an uninitialized in-memory table for testing and does not persist changes beyond the table object lifetime. ([Docs.rs][10])

---

## 8.16 Table initialization idempotency

### 8.16.1 Create-once helper

```rust id="psvd4i"
pub async fn create_once(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .await
        .map_err(Into::into)
}
```

### 8.16.2 Create-if-absent helper

```rust id="zkuq2h"
pub async fn create_if_absent(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::Ignore)
        .with_columns(simulation_output_fields())
        .await?;

    Ok(table)
}
```

### 8.16.3 Explicit open-or-create

```rust id="u7sne3"
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn open_or_create_simulation_outputs(
    uri: &str,
) -> anyhow::Result<DeltaTable> {
    match open_table(Url::parse(uri)?).await {
        Ok(table) => Ok(table),
        Err(open_err) => {
            tracing::warn!(error = %open_err, "open failed; attempting create");

            let table = CreateBuilder::new()
                .with_location(uri)
                .with_table_name("simulation_outputs")
                .with_save_mode(SaveMode::ErrorIfExists)
                .with_columns(simulation_output_fields())
                .with_partition_columns(["run_date"])
                .await?;

            Ok(table)
        }
    }
}
```

Production idempotency policy:

```text id="eibc06"
Preferred:
  explicit create migration job
  SaveMode::ErrorIfExists
  separate validation
  table registry update after success

Acceptable:
  open-or-create during controlled service startup
  one creator process
  locking/coordination around table creation

Risky:
  many service replicas all creating same table
  create with Overwrite on startup
  create-if-absent without schema/property validation
```

---

## 8.17 Commit properties for creation

```rust id="jldhx5"
use deltalake::kernel::transaction::CommitProperties;

pub async fn create_with_commit_properties(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let commit_properties = CommitProperties::default();

    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(simulation_output_fields())
        .with_commit_properties(commit_properties)
        .await?;

    Ok(table)
}
```

`CreateBuilder::with_commit_properties` adds metadata to the commit info. ([Docs.rs][2])

Recommended creation commit metadata:

```text id="fv7es9"
application_id
application_version
migration_id
schema_contract_version
created_by_service
deployment_environment
git_sha
build_id
ticket_or_change_id
```

Never include credentials, tokens, secrets, raw user PII, or full storage options in commit metadata.

---

## 8.18 Advanced: low-level actions

```rust id="jq33oh"
let table = CreateBuilder::new()
    .with_location(uri)
    .with_actions(actions)
    .await?;
```

`CreateBuilder::with_actions` allows additional actions in the first transaction, but docs warn that manually creating inconsistent actions can have undesirable effects. ([Docs.rs][2])

Guidance:

```text id="1ha2eq"
Do not use with_actions for normal application table creation.
Use high-level methods for schema, properties, partitioning, metadata, and commits.
Reserve raw actions for protocol-level tooling, migration tools, or delta-rs contributors.
```

---

## 8.19 Validation after creation

```rust id="x0uqsq"
pub fn validate_created_table(
    table: &deltalake::DeltaTable,
    expected_version: u64,
) -> anyhow::Result<()> {
    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("created table is not loaded"))?;

    anyhow::ensure!(
        version == expected_version,
        "unexpected created table version: expected {expected_version}, got {version}"
    );

    let state = table.snapshot()?;
    let metadata = state.metadata();
    let protocol = state.protocol();
    let schema = state.schema();
    let config = state.table_config();

    tracing::info!(
        version,
        ?metadata,
        ?protocol,
        ?schema,
        ?config,
        "created Delta table validated"
    );

    Ok(())
}
```

Post-create checklist:

```text id="22x06q"
version loaded
schema matches expected contract
partition columns match expected contract
table properties present
CDF enabled if required
append-only enabled if required
protocol compatible with service
DataFusion provider registration succeeds
basic SELECT COUNT(*) succeeds
write smoke succeeds if table is writable
```

---

## 8.20 Testing matrix

```text id="ewbx2c"
unit:
  schema constructor output
  field metadata constructor
  table-property constructor
  storage-options constructor
  save-mode policy selection

local integration:
  create empty table
  create with partition columns
  create CDF-enabled table
  create append-only table
  create-if-absent idempotency
  create overwrite fixture reset
  open created table
  inspect metadata/schema/config

Arrow bootstrap:
  create then append batches
  write-to-create from batches
  schema mismatch behavior
  partition write behavior

DataFusion bootstrap:
  query-result table creation
  SessionState required
  missing source provider failure
  target table open/query after creation

Parquet conversion:
  unpartitioned Parquet conversion
  Hive-partitioned conversion
  partition schema mismatch failure
  raw Parquet count vs Delta count validation

object-store:
  S3 create with locking
  MinIO/LocalStack create
  bad credential failure
  duplicate create conflict
  unsafe overwrite blocked by policy
```

---

## 8.21 Best practices

```text id="mupmr5"
API:
  - use CreateBuilder or DeltaTable::create for explicit DDL
  - use DeltaTable::write for data-driven creation
  - use ConvertToDeltaBuilder for existing Parquet datasets
  - avoid DeltaOps in new docs/code

schema:
  - centralize schema constructors
  - explicit nullability
  - explicit decimals/timestamps
  - deterministic column order
  - no production schema inference without validation

properties:
  - enable CDF at creation if required
  - set append-only at creation for immutable logs
  - set governance properties before first write
  - keep commit metadata secret-free

partitioning:
  - define partition columns at creation
  - align partitioning to query predicates
  - avoid high-cardinality partitions
  - treat partition change as migration

idempotency:
  - production default SaveMode::ErrorIfExists
  - local fixture reset SaveMode::Overwrite
  - bootstrap convenience SaveMode::Ignore only when no-op is acceptable
  - avoid multiple replicas racing to create canonical tables

conversion:
  - freeze raw Parquet writers
  - declare partition schema
  - validate counts and schema after conversion
  - block future raw Parquet writes
```

---

## 8.22 Anti-patterns

```text id="dkvzh3"
- treating table creation as just mkdir + Parquet files
- writing raw Parquet files into a Delta table root
- using SaveMode::Overwrite in production startup
- enabling CDF after historical changes and expecting old CDF rows
- setting append-only on tables that need delete/update/merge
- partitioning by unique run_id
- relying on inferred schema for governed simulation outputs
- using create-if-absent without validating existing schema/properties
- converting active Parquet directories while writers are still running
- exposing storage paths as table names
- storing secrets in table configuration, field metadata, or commit metadata
- using with_actions for ordinary table creation
```

---

## 8.23 LLM-agent checklist

```text id="lttaa2"
Before generating create-table code:
  1. Select construction path:
     explicit CreateBuilder
     DeltaTable::create
     write-to-create
     ConvertToDeltaBuilder
  2. Confirm target URI and backend feature flags.
  3. Add storage_options for cloud/test backends.
  4. Select SaveMode explicitly.
  5. Define schema with StructField/DataType.
  6. Define nullability explicitly.
  7. Define partition columns explicitly.
  8. Add table name and comment.
  9. Add CDF property if downstream incremental reads need it.
  10. Add append-only property only for immutable tables.
  11. Add commit properties without secrets.
  12. For DataFusion bootstrap, use df.into_parts + with_session_state.
  13. For Parquet conversion, freeze writers and specify partition strategy/schema.
  14. Validate created table metadata/schema/properties.
  15. Register and query through DataFusion provider as smoke test.
```

---

## 8.24 Value case

Correct create-table workflows give your Rust DataFusion/Arrow simulation platform:

```text id="tbd86d"
schema-governed table contracts from version 0
transactionally initialized storage roots
DataFusion-queryable tables immediately after creation
controlled CDF enablement
append-only immutability where required
partition layout aligned to query performance
safe local/dev/prod idempotency patterns
in-place migration path from Parquet datasets
Arrow/DataFusion bootstrap paths for derived tables
LLM-agent-safe DDL generation templates
```

Core invariant:

```text id="r40j81"
Create the Delta contract before depending on the Delta data.
```

Operational invariant:

```text id="58fpy6"
A production table creation must explicitly declare location, save mode, schema, partitioning, table properties, storage backend behavior, and idempotency semantics.
```

---

## 8.25 Checkpoint writing is kernel-owned: `add`-struct nullability and cross-version reader compatibility

At the 1.0.0 pinned rev, `deltalake::checkpoints::create_checkpoint` (and `create_checkpoint_from_table_url_and_cleanup`) delegate checkpoint writing to the Delta kernel: the implementation builds a kernel `Snapshot` (`Snapshot::builder_for(table_root).at_version(v).build(engine)`) and calls `snapshot.checkpoint(engine, None)` — there is no independent delta-rs checkpoint serializer anymore.

Consequence for the Parquet checkpoint schema (verified in kernel source): the checkpoint’s top-level action columns (`add`, `remove`, `metaData`, `protocol`, `txn`, …) are nullable — each row is one action — but the **required fields inside the `add` struct are written non-nullable** (`nullable=false` / Parquet `required` repetition): `path`, `partitionValues`, `size`, `modificationTime`, `dataChange`. Optional fields (`stats`, `tags`, `deletionVector`, `baseRowId`, `defaultRowCommitVersion`, `clusteringProvider`) stay nullable.

Cross-version compatibility posture:

At the latest pin, `TableFeature::V2Checkpoint` is included in delta-rs's supported **reader and writer feature sets**. A table declaring the feature therefore no longer fails the generic protocol checker solely because `v2Checkpoint` is present. This is a compatibility-gate improvement; it should not be read as a promise that every V2 checkpoint authoring/maintenance control is exposed through a new high-level Rust builder.

```text
- Modern readers (Spark, delta-rs >= 0.26-era, kernel-based engines) read these
  checkpoints fine: the Delta protocol marks those add fields as required.
- Very old delta-rs readers (<= 0.25.5) expected the add fields to be optional
  and can fail on kernel-written checkpoints (delta-rs issue #3527).
- This behavior is inherited from the kernel-backed checkpoint path of late
  0.32.x — it is not new at 1.0.0 — but the 1.0.0 baseline makes the kernel
  writer the only writer.
- If any consumer in your fleet runs a pre-0.26 delta-rs, upgrade it before
  checkpointing tables it must read; there is no writer-side opt-out.
```

Operational guidance: treat checkpoint creation as a protocol-compatibility event in mixed-engine fleets — inventory readers before enabling automated checkpointing on shared tables, and test one checkpointed table copy against every reader engine version you support.

[1]: https://docs.rs/deltalake/latest/deltalake/struct.DeltaOps.html "DeltaOps in deltalake - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/operations/create/struct.CreateBuilder.html "CreateBuilder in deltalake::operations::create - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/struct.StructField.html "StructField in deltalake - Rust"
[4]: https://delta-io.github.io/delta-rs/usage/examining-table/ "Examining a table - Delta Lake Documentation"
[5]: https://docs.delta.io/delta-change-data-feed/?utm_source=chatgpt.com "Change data feed"
[6]: https://delta.io/blog/2022-11-01-pyspark-save-mode-append-overwrite-error/?utm_source=chatgpt.com "Why PySpark append and overwrite write operations are ..."
[7]: https://docs.rs/deltalake/latest/deltalake/operations/convert_to_delta/index.html "deltalake::operations::convert_to_delta - Rust"
[8]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html "WriteBuilder in deltalake::operations::write - Rust"
[9]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"
[10]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"


# 9. DML: delete, update, and merge — Rust `deltalake`

Version target:

```text id="q71l1j"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required deltalake feature: ["datafusion"]
Typical production features: ["rustls", "datafusion", "s3"]
```

Current API correction: prefer `DeltaTable::delete()`, `DeltaTable::update()`, and `DeltaTable::merge(source_df, predicate)` over `DeltaOps(table).delete()/merge()/update()` in new service code. `DeltaOps` is still visible but marked deprecated; `DeltaTable` exposes `delete() -> DeleteBuilder`, `update() -> UpdateBuilder`, and `merge(source: DataFrame, predicate) -> MergeBuilder`. Unlike `WriteBuilder`, DML builders return `(DeltaTable, Metrics)` tuples when awaited. ([Docs.rs][1])

---

## 9.1 DML mental model

```text id="b02y87"
Delta DML operation:
  read current snapshot
  plan affected files using DataFusion expressions
  scan files / target rows
  rewrite affected Parquet files
  emit Add actions for rewritten files
  emit Remove actions for replaced/deleted files
  commit transaction-log update
  return new DeltaTable + operation metrics
```

Delta DML is **file-rewrite-based mutable table behavior**, not in-place row mutation. Delete with a predicate scans the Delta table to identify files containing matching records, then rewrites files without those records; delete without a predicate deletes all records. The delete docs also state that predicates must be deterministic, because non-deterministic predicates can create undefined behavior during scan/rewrite. ([Docs.rs][2])

---

## 9.2 Cargo baseline

```toml id="43dq1l"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Use the `datafusion` feature because predicates, updates, and merge clauses are expressed through Delta/DataFusion expression integration, and merge sources are DataFusion `DataFrame`s. The crate re-exports DataFusion and has a dedicated `delta_datafusion` integration module. ([Docs.rs][1])

---

## 9.3 Expression imports

```rust id="sdv1u7"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::operations::write::SessionFallbackPolicy;
```

Recommended expression policy:

```text id="3jx30i"
Use DataFusion Expr for:
  generated predicates
  UI/config-derived filters
  delete/update/merge conditions
  replaceWhere predicates
  safe LLM-agent-generated code

Use SQL strings only when:
  user-facing SQL is the product
  SQL parser/validator/allowlist exists
  expression is logged and tested
```

---

## 9.4 Delete by predicate

```rust id="6wg6vi"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn delete_one_id(uri: &str, id: i64) -> anyhow::Result<(DeltaTable, deltalake::operations::delete::DeleteMetrics)> {
    let table = open_table(Url::parse(uri)?).await?;

    let (table, metrics) = table
        .delete()
        .with_predicate(col("id").eq(lit(id)))
        .await?;

    Ok((table, metrics))
}
```

`DeleteBuilder::with_predicate` accepts any value convertible into a Delta/DataFusion `Expression`; `DeleteBuilder` also supports session-state injection, session fallback policy, commit properties, Parquet writer properties for rewritten files, and custom execute handlers. Awaiting `DeleteBuilder` returns `(DeltaTable, DeleteMetrics)`. ([Docs.rs][3])

Production guardrails:

```text id="yfd0yw"
- require deterministic predicates
- require explicit predicate unless full-table delete is intentionally approved
- prefer partition-aligned predicates when deleting large slices
- expect affected files to be rewritten
- capture DeleteMetrics
- validate table is not append-only
- validate DML is compatible with table protocol/features
```

---

## 9.5 Delete all rows

```rust id="gk0274"
pub async fn delete_all_rows(uri: &str) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::delete::DeleteMetrics)> {
    let table = deltalake::open_table(url::Url::parse(uri)?).await?;

    let (table, metrics) = table.delete().await?;

    Ok((table, metrics))
}
```

Delete without a predicate removes all records from the table. For full-file metadata-only deletes, `DeleteMetrics::num_deleted_rows` is `Option<usize>` because exact row counts may be unavailable when file metadata cannot derive them; row-rewrite deletes can derive the deleted count from execution metrics. ([Docs.rs][2])

Full-table delete policy:

```text id="wurvfn"
Allowed:
  local/dev fixture reset
  small temporary table cleanup
  controlled GDPR erase of isolated per-user table
  explicitly approved operational runbook

Disallowed by default:
  canonical production fact table
  shared simulation output table
  append-only table
  table with downstream incremental consumers unless coordinated
```

---

## 9.6 Delete metrics

```rust id="1faig2"
#[derive(Debug, serde::Serialize)]
pub struct DeleteMetricLog {
    pub num_added_files: usize,
    pub num_removed_files: usize,
    pub num_deleted_rows: Option<usize>,
    pub num_copied_rows: usize,
    pub execution_time_ms: u64,
    pub scan_time_ms: u64,
    pub rewrite_time_ms: u64,
}

impl From<deltalake::operations::delete::DeleteMetrics> for DeleteMetricLog {
    fn from(m: deltalake::operations::delete::DeleteMetrics) -> Self {
        Self {
            num_added_files: m.num_added_files,
            num_removed_files: m.num_removed_files,
            num_deleted_rows: m.num_deleted_rows,
            num_copied_rows: m.num_copied_rows,
            execution_time_ms: m.execution_time_ms,
            scan_time_ms: m.scan_time_ms,
            rewrite_time_ms: m.rewrite_time_ms,
        }
    }
}
```

`DeleteMetrics` includes added/removed file counts, optional deleted-row count, copied-row count, total execution time, scan time, and rewrite time. Treat `num_deleted_rows = None` as “unknown,” not zero. ([GitHub][4])

---

## 9.7 Update rows by predicate

```rust id="ysxwcb"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::{open_table, DeltaTable};
use url::Url;

pub async fn mark_failed_runs_for_review(
    uri: &str,
) -> anyhow::Result<(DeltaTable, deltalake::operations::update::UpdateMetrics)> {
    let table = open_table(Url::parse(uri)?).await?;

    let (table, metrics) = table
        .update()
        .with_predicate(col("status").eq(lit("FAILED")))
        .with_update("needs_review", lit(true))
        .with_update("review_reason", lit("failed_run"))
        .with_safe_cast(false)
        .await?;

    Ok((table, metrics))
}
```

`UpdateBuilder` supports `with_predicate`, `with_update(column, expression)`, `with_session_state`, `with_session_fallback_policy`, `with_commit_properties`, `with_writer_properties`, `with_safe_cast`, and `with_custom_execute_handler`. Awaiting `UpdateBuilder` returns `(DeltaTable, UpdateMetrics)`. ([Docs.rs][5])

Update semantics:

```text id="oykdje"
No predicate:
  predicate defaults to true internally
  update applies to all rows

No update expressions:
  operation returns default metrics and does not rewrite files

Predicate matches no files:
  no commit; table version may remain unchanged

Predicate matches files:
  affected files rewritten
  copied rows = unaffected rows in rewritten files
```

The current update implementation resolves a missing predicate to `lit(true)`, skips commit when no actions are produced, and uses writer properties for rewritten files. ([GitHub][6])

---

## 9.8 Update metrics

```rust id="3p2f7s"
#[derive(Debug, serde::Serialize)]
pub struct UpdateMetricLog {
    pub num_added_files: usize,
    pub num_removed_files: usize,
    pub num_updated_rows: usize,
    pub num_copied_rows: usize,
    pub execution_time_ms: u64,
    pub scan_time_ms: u64,
}

impl From<deltalake::operations::update::UpdateMetrics> for UpdateMetricLog {
    fn from(m: deltalake::operations::update::UpdateMetrics) -> Self {
        Self {
            num_added_files: m.num_added_files,
            num_removed_files: m.num_removed_files,
            num_updated_rows: m.num_updated_rows,
            num_copied_rows: m.num_copied_rows,
            execution_time_ms: m.execution_time_ms,
            scan_time_ms: m.scan_time_ms,
        }
    }
}
```

`UpdateMetrics` includes added/removed files, updated rows, copied rows, total execution time, and scan time. ([GitHub][6])

---

## 9.9 Safe cast behavior for update and merge

```rust id="hvpp6e"
let (table, metrics) = table
    .update()
    .with_predicate(col("id").eq(lit(123)))
    .with_update("integer_column", lit("123"))
    .with_safe_cast(false)
    .await?;
```

For update, `with_safe_cast(true)` converts failed casts to null instead of returning an error; for example, a string like `Test123` cast into an integer column becomes null under safe casting. For merge, `with_safe_cast(true)` similarly yields null for failed casts into nullable target columns, while failed safe casts into non-nullable columns still fail write constraints. ([Docs.rs][5])

Production default:

```text id="om7957"
with_safe_cast(false)
```

Use fail-fast casting for governed engineering/simulation outputs. Use safe casting only for bronze/raw ingestion or explicitly null-tolerant workflows.

---

## 9.10 Merge source into target

### 9.10.1 Source as DataFusion `DataFrame`

```rust id="9n4u2h"
use datafusion::prelude::*;
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::{open_table, DeltaTable};
use std::sync::Arc;
use url::Url;

pub async fn upsert_from_registered_staging(
    ctx: &SessionContext,
    target_uri: &str,
) -> anyhow::Result<(DeltaTable, deltalake::operations::merge::MergeMetrics)> {
    let target = open_table(Url::parse(target_uri)?).await?;

    let source = ctx
        .sql(
            r#"
            SELECT
              id,
              value,
              modified_at
            FROM staging_updates
            "#,
        )
        .await?;

    let state = Arc::new(ctx.state());

    let (table, metrics) = target
        .merge(source, col("target.id").eq(col("source.id")))
        .with_source_alias("source")
        .with_target_alias("target")
        .with_session_state(state)
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .when_matched_update(|u| {
            u.update("value", col("source.value"))
             .update("modified_at", col("source.modified_at"))
        })?
        .when_not_matched_insert(|i| {
            i.set("id", col("source.id"))
             .set("value", col("source.value"))
             .set("modified_at", col("source.modified_at"))
        })?
        .with_safe_cast(false)
        .await?;

    Ok((table, metrics))
}
```

Merge uses a source `DataFrame` and a join predicate; internally it performs a full outer join that classifies rows as matched, source-only, or target-only, and users can attach update, delete, and insert operations to those categories. The merge operation order matters. ([Docs.rs][7])

---

## 9.11 Merge clause families

Merge supports three semantic groups:

```text id="8s2ngi"
when_matched_*:
  source row + target row match join predicate

when_not_matched_*:
  source row has no target match

when_not_matched_by_source_*:
  target row has no source match
```

Current Rust methods include `when_matched_update`, `when_matched_delete`, `when_not_matched_insert`, `when_not_matched_by_source_update`, and `when_not_matched_by_source_delete`. The code comments document that multiple clauses can be specified and only the first satisfied clause executes; therefore clause order is part of semantics. ([GitHub][8])

---

## 9.12 Clause ordering

```rust id="bv62wd"
let (table, metrics) = target
    .merge(source, col("target.id").eq(col("source.id")))
    .with_source_alias("source")
    .with_target_alias("target")
    // More-specific matched clause first.
    .when_matched_delete(|d| {
        d.predicate(col("source.delete_flag").eq(lit(true)))
    })?
    // General matched update second.
    .when_matched_update(|u| {
        u.update("value", col("source.value"))
         .update("modified_at", col("source.modified_at"))
    })?
    .when_not_matched_insert(|i| {
        i.set("id", col("source.id"))
         .set("value", col("source.value"))
         .set("modified_at", col("source.modified_at"))
    })?
    .await?;
```

Ordering rule:

```text id="t2rghx"
specific destructive clauses first
specific update clauses next
general update clauses last
insert clauses specific-to-general
not-matched-by-source clauses carefully isolated
```

If a general update clause precedes a delete clause, the delete clause may never execute for the same match class because only the first satisfied clause is used. ([GitHub][8])

---

## 9.13 Source alias and target alias

```rust id="nk8iys"
.merge(source, col("target.id").eq(col("source.id")))
.with_source_alias("source")
.with_target_alias("target")
```

`with_source_alias` prefixes source columns as `alias.original_column_name`, and `with_target_alias` prefixes target columns similarly. Use aliases always in generated merge code to avoid ambiguity and to make expression provenance machine-checkable. ([GitHub][8])

Alias policy:

```text id="dvr4w1"
source alias:
  "source" or "s"

target alias:
  "target" or "t"

Do not:
  reference unqualified columns in merge predicates
  use aliases that collide with real column names
  vary alias names across generated snippets without reason
```

---

## 9.14 Upsert pattern

```rust id="j3wvfz"
pub async fn upsert_dimension_rows(
    ctx: &datafusion::prelude::SessionContext,
    target_uri: &str,
    source_sql: &str,
) -> anyhow::Result<deltalake::operations::merge::MergeMetrics> {
    use deltalake::datafusion::logical_expr::col;

    let target = deltalake::open_table(url::Url::parse(target_uri)?).await?;
    let source = ctx.sql(source_sql).await?;
    let state = std::sync::Arc::new(ctx.state());

    let (_table, metrics) = target
        .merge(source, col("target.business_key").eq(col("source.business_key")))
        .with_source_alias("source")
        .with_target_alias("target")
        .with_session_state(state)
        .with_session_fallback_policy(deltalake::operations::write::SessionFallbackPolicy::RequireSessionState)
        .when_matched_update(|u| {
            u.update("attr_1", col("source.attr_1"))
             .update("attr_2", col("source.attr_2"))
             .update("updated_at", col("source.updated_at"))
        })?
        .when_not_matched_insert(|i| {
            i.set("business_key", col("source.business_key"))
             .set("attr_1", col("source.attr_1"))
             .set("attr_2", col("source.attr_2"))
             .set("created_at", col("source.created_at"))
             .set("updated_at", col("source.updated_at"))
        })?
        .with_safe_cast(false)
        .await?;

    Ok(metrics)
}
```

Upsert requirements:

```text id="w66d8v"
- source contains one row per merge key
- merge key is stable and non-null
- source is deduplicated before merge
- matched update is deterministic
- insert sets all required non-null target columns
- retry path is idempotent
```

---

## 9.15 Slowly changing dimension patterns

### 9.15.1 SCD Type 1: overwrite current attributes

```rust id="crgugb"
let (table, metrics) = target
    .merge(source, col("target.customer_id").eq(col("source.customer_id")))
    .with_source_alias("source")
    .with_target_alias("target")
    .when_matched_update(|u| {
        u.update("name", col("source.name"))
         .update("segment", col("source.segment"))
         .update("updated_at", col("source.effective_at"))
    })?
    .when_not_matched_insert(|i| {
        i.set("customer_id", col("source.customer_id"))
         .set("name", col("source.name"))
         .set("segment", col("source.segment"))
         .set("created_at", col("source.effective_at"))
         .set("updated_at", col("source.effective_at"))
    })?
    .await?;
```

### 9.15.2 SCD Type 2: expire missing/changed current rows

```rust id="hx05z3"
let (table, metrics) = target
    .merge(
        source,
        col("target.customer_id").eq(col("source.customer_id"))
            .and(col("target.is_current").eq(lit(true))),
    )
    .with_source_alias("source")
    .with_target_alias("target")
    .when_matched_update(|u| {
        u.predicate(col("target.hash").not_eq(col("source.hash")))
         .update("is_current", lit(false))
         .update("valid_to", col("source.effective_at"))
    })?
    .when_not_matched_insert(|i| {
        i.set("customer_id", col("source.customer_id"))
         .set("name", col("source.name"))
         .set("segment", col("source.segment"))
         .set("hash", col("source.hash"))
         .set("is_current", lit(true))
         .set("valid_from", col("source.effective_at"))
         .set("valid_to", lit(null::<String>()))
    })?
    .await?;
```

SCD2 warning:

```text id="izzyz6"
A single merge that expires old records may not insert a new current version for already-matched changed rows unless the source/target join design deliberately creates an unmatched insert path. Many SCD2 pipelines use a staged source with separate expire and insert records, or two transactions: expire old current rows, then append/merge new current rows.
```

---

## 9.16 CDC ingestion pattern

Recommended source staging schema:

```text id="2jwzr7"
business_key
operation          -- I/U/D
event_ts
sequence_number
payload columns
source_batch_id
source_record_id
```

CDC merge:

```rust id="ojyvp2"
let (table, metrics) = target
    .merge(source, col("target.id").eq(col("source.id")))
    .with_source_alias("source")
    .with_target_alias("target")
    .when_matched_delete(|d| {
        d.predicate(col("source.operation").eq(lit("D")))
    })?
    .when_matched_update(|u| {
        u.predicate(col("source.operation").eq(lit("U")))
         .update("value", col("source.value"))
         .update("updated_at", col("source.event_ts"))
    })?
    .when_not_matched_insert(|i| {
        i.predicate(col("source.operation").not_eq(lit("D")))
         .set("id", col("source.id"))
         .set("value", col("source.value"))
         .set("created_at", col("source.event_ts"))
         .set("updated_at", col("source.event_ts"))
    })?
    .await?;
```

CDC requirements:

```text id="35m1z5"
- source ordered/deduplicated per key
- tombstones handled explicitly
- event-time and ingestion-time distinguished
- late events policy documented
- idempotency key stored
- duplicate batch detection implemented
```

---

## 9.17 GDPR/right-to-delete workflow

```rust id="pfm7mg"
pub async fn gdpr_delete_subject(
    uri: &str,
    subject_id: &str,
) -> anyhow::Result<deltalake::operations::delete::DeleteMetrics> {
    use deltalake::datafusion::logical_expr::{col, lit};

    let table = deltalake::open_table(url::Url::parse(uri)?).await?;

    let (_table, metrics) = table
        .delete()
        .with_predicate(col("subject_id").eq(lit(subject_id.to_owned())))
        .await?;

    Ok(metrics)
}
```

Right-to-delete runbook:

```text id="agkge0"
1. Identify all Delta tables containing subject_id.
2. Ensure table is not append-only or has approved exception path.
3. Execute deterministic delete predicate.
4. Capture table version before and after.
5. Capture DeleteMetrics.
6. Validate no remaining rows for subject_id through Delta provider query.
7. Coordinate retention/vacuum policy separately.
8. Update deletion audit table without storing erased PII.
```

DML deletion removes rows from the active table state by rewriting/removing files, but old files can remain physically present until retention/vacuum. Treat legal erasure as a two-step process: logical active-state delete first, physical retention/vacuum process second.

---

## 9.18 Merge metrics

```rust id="tcwtjw"
#[derive(Debug, serde::Serialize)]
pub struct MergeMetricLog {
    pub num_source_rows: usize,
    pub num_target_rows_inserted: usize,
    pub num_target_rows_updated: usize,
    pub num_target_rows_deleted: usize,
    pub num_target_rows_copied: usize,
    pub num_output_rows: usize,
    pub num_target_files_scanned: usize,
    pub num_target_files_skipped_during_scan: usize,
    pub num_target_files_added: usize,
    pub num_target_files_removed: usize,
    pub execution_time_ms: u64,
    pub scan_time_ms: u64,
    pub rewrite_time_ms: u64,
}

impl From<deltalake::operations::merge::MergeMetrics> for MergeMetricLog {
    fn from(m: deltalake::operations::merge::MergeMetrics) -> Self {
        Self {
            num_source_rows: m.num_source_rows,
            num_target_rows_inserted: m.num_target_rows_inserted,
            num_target_rows_updated: m.num_target_rows_updated,
            num_target_rows_deleted: m.num_target_rows_deleted,
            num_target_rows_copied: m.num_target_rows_copied,
            num_output_rows: m.num_output_rows,
            num_target_files_scanned: m.num_target_files_scanned,
            num_target_files_skipped_during_scan: m.num_target_files_skipped_during_scan,
            num_target_files_added: m.num_target_files_added,
            num_target_files_removed: m.num_target_files_removed,
            execution_time_ms: m.execution_time_ms,
            scan_time_ms: m.scan_time_ms,
            rewrite_time_ms: m.rewrite_time_ms,
        }
    }
}
```

`MergeMetrics` includes source rows, inserted/updated/deleted/copied target rows, output rows, target files scanned/skipped/added/removed, and execution/scan/rewrite timings. ([GitHub][8])

Metric interpretation:

```text id="rue7qd"
high num_target_files_scanned:
  weak join predicate pruning
  missing partition predicate
  poor clustering / small-file problem

high num_target_rows_copied:
  many unaffected rows copied during file rewrites
  row-level change touches broad files
  optimize/Z-order may improve layout

high rewrite_time_ms:
  large affected files
  slow object store
  poor target file sizing
  expensive Parquet writer settings

zero rows but many files scanned:
  predicate not selective enough for file skipping
```

---

## 9.19 Session-state injection for DML

```rust id="d7ihhv"
use datafusion::prelude::*;
use deltalake::operations::write::SessionFallbackPolicy;
use std::sync::Arc;

pub async fn delete_with_service_session(
    ctx: &SessionContext,
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::delete::DeleteMetrics)> {
    let state = Arc::new(ctx.state());

    let result = table
        .delete()
        .with_predicate(col("status").eq(lit("INVALID")))
        .with_session_state(state)
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .await?;

    Ok(result)
}
```

Delete, update, and merge builders all accept a DataFusion session and a session fallback policy. The docs state that the provided session should wrap a concrete `SessionState`; otherwise the default fallback uses internal defaults unless `RequireSessionState` is selected. ([Docs.rs][3])

Production default:

```text id="s34jqh"
with_session_state(Arc::new(ctx.state()))
with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
```

Reason:

```text id="y23mdv"
- preserves object-store registry
- preserves UDF registry
- preserves runtime/spill config
- preserves catalog bindings
- preserves tenant/session policy
- prevents silent fallback to internal defaults
```

---

## 9.20 Commit properties

```rust id="2wg8ju"
use deltalake::kernel::transaction::CommitProperties;

let commit_properties = CommitProperties::default();

let (table, metrics) = table
    .delete()
    .with_predicate(col("id").eq(lit(123)))
    .with_commit_properties(commit_properties)
    .await?;
```

DML builders support commit properties: delete adds “additional information to write to the commit,” update adds metadata to commit info, and merge supports commit properties too. ([Docs.rs][3])

Recommended DML commit metadata:

```text id="0sdit4"
operation_id
application_id
application_version
request_id
trace_id
job_id
simulation_id
source_snapshot_versions
idempotency_key
actor_type
change_reason
```

Never put secrets, tokens, full storage options, or raw sensitive payloads in commit metadata.

---

## 9.21 Writer properties for rewritten files

```rust id="pdt8j4"
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

let writer_properties = WriterProperties::builder()
    .set_compression(Compression::SNAPPY)
    .build();

let (table, metrics) = table
    .update()
    .with_predicate(col("status").eq(lit("REPROCESS")))
    .with_update("status", lit("READY"))
    .with_writer_properties(writer_properties)
    .await?;
```

Delete, update, and merge support Parquet writer properties for rewritten files. This matters because DML rewrites affected data files, so compression/row-group/file-stat behavior on rewritten files should match table policy. ([Docs.rs][3])

---

## 9.22 Conflict detection and retry posture

Delta Lake uses optimistic concurrency control: writes proceed in stages, and conflicts are detected during validation/commit rather than by locking rows upfront. The official concurrency docs describe this as read → write → validate-and-commit, where concurrent transactions can fail if conflicting changes have been committed since the transaction’s read snapshot. ([Delta Lake][9])

DML retry taxonomy:

```text id="xyyzqb"
Safe to retry automatically:
  deterministic delete by partition/key after verifying prior commit did not happen
  idempotent merge with deduplicated source and stable keys
  update setting same deterministic value by same predicate

Unsafe to retry blindly:
  append-like insert-only merge with duplicate-prone source
  update using non-deterministic values
  merge with source containing duplicate keys
  DML after unknown commit outcome
  DML over concurrently modified same target keys/files
```

Production retry skeleton:

```rust id="g6t8wq"
pub async fn retry_idempotent_dml<F, Fut, T>(
    mut op: F,
    max_attempts: usize,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 1..=max_attempts {
        match op().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                tracing::warn!(attempt, error = %err, "DML attempt failed");
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("DML failed without error")))
}
```

Retry requirement:

```text id="ybgqku"
Before retrying an unknown DML outcome:
  reload table
  inspect latest version/history
  detect operation_id/idempotency_key if available
  verify whether intended change already committed
  retry only if the operation is provably idempotent
```

---

## 9.23 Idempotent merge design

Required source constraints:

```text id="h5qa4m"
one row per merge key
stable merge key
deterministic source projection
deduplicated CDC batch
batch_id / operation_id available
no random/now-based updates unless fixed upstream
```

Source dedup before merge:

```rust id="77f3gn"
let source = ctx
    .sql(
        r#"
        SELECT
          id,
          ANY_VALUE(value) AS value,
          MAX(modified_at) AS modified_at
        FROM staging_updates
        GROUP BY id
        "#,
    )
    .await?;
```

Better deterministic CDC dedup:

```sql id="9pag62"
SELECT id, value, modified_at, sequence_number
FROM (
  SELECT
    *,
    ROW_NUMBER() OVER (
      PARTITION BY id
      ORDER BY sequence_number DESC, modified_at DESC
    ) AS rn
  FROM staging_updates
)
WHERE rn = 1
```

Idempotency invariant:

```text id="dl2md8"
Running the same merge source against the same target state twice should not create additional business rows.
```

---

## 9.24 Duplicate match avoidance

Merge anti-pattern:

```text id="z5uv61"
source:
  id=123 value=A
  id=123 value=B

target:
  id=123 value=old

merge key:
  target.id = source.id
```

Required pre-merge validation:

```rust id="b2d75f"
pub async fn assert_unique_source_keys(
    ctx: &datafusion::prelude::SessionContext,
    source_table: &str,
    key: &str,
) -> anyhow::Result<()> {
    let sql = format!(
        r#"
        SELECT {key}, COUNT(*) AS n
        FROM {source_table}
        GROUP BY {key}
        HAVING COUNT(*) > 1
        LIMIT 1
        "#
    );

    let batches = ctx.sql(&sql).await?.collect().await?;
    let duplicate_rows: usize = batches.iter().map(|b| b.num_rows()).sum();

    anyhow::ensure!(duplicate_rows == 0, "source contains duplicate merge keys");

    Ok(())
}
```

Merge correctness depends on stable match cardinality. Always validate source uniqueness for upsert/CDC unless the clause logic explicitly handles duplicate source rows.

---

## 9.25 Predicate determinism

Good:

```rust id="ia70b3"
col("id").eq(lit(123))
col("run_date").eq(lit("2026-06-02"))
col("status").eq(lit("FAILED"))
col("updated_at").lt(lit("2026-06-02T00:00:00Z"))
```

Bad or high-risk:

```text id="1jwarj"
random()
now()
uuid()
non-deterministic UDF
external-service UDF
clock-dependent expression
predicate with mutable lookup table semantics
```

Delete docs explicitly require deterministic predicates; use the same discipline for update and merge predicates because DML planning separates scan, rewrite, and commit phases. ([Docs.rs][2])

---

## 9.26 File rewrite behavior

```text id="3hv8o9"
delete:
  removes matched rows
  rewrites partially affected files
  metadata-only full-file deletes may remove files without rewrite

update:
  rewrites files containing matched rows
  updates matching rows
  copies non-matching rows from affected files

merge:
  joins source and target
  rewrites affected target files
  inserts source-only rows
  updates/deletes matched rows
  copies unaffected rows from rewritten files
```

Metrics reveal rewrite intensity:

```text id="v5r9qa"
delete:
  num_copied_rows / num_deleted_rows

update:
  num_copied_rows / num_updated_rows

merge:
  num_target_rows_copied
  num_target_files_scanned
  num_target_files_skipped_during_scan
  num_target_files_added
  num_target_files_removed
```

High copied-row counts mean sparse row changes are rewriting broad files. Improve layout with partitioning, compaction, clustering/Z-ordering, or smaller target files for high-churn tables.

---

## 9.27 Append-only and column-mapping limitations

Delete and update check append-only protocol/table constraints and reject unsupported column-mapping writes. In the current source, delete and update call protocol write checks and return `unsupported_column_mapping_write("DELETE"/"UPDATE")` when column mapping mode is not `None`. ([GitHub][4])

Operational guard:

```rust id="bbunqk"
pub fn require_dml_allowed(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let state = table.snapshot()?;
    let config = state.table_config();

    tracing::info!(?config, "validating Delta table DML properties");

    // Add explicit service-level checks:
    // - reject append-only tables for delete/update/merge
    // - reject unsupported column mapping modes
    // - reject unsupported table features
    // - reject protocol versions outside certified matrix

    Ok(())
}
```

---

## 9.28 DataFusion expression generation

```rust id="otbgt9"
use deltalake::datafusion::logical_expr::{col, lit, Expr};

#[derive(Debug, Clone)]
pub enum DmlPredicate {
    IdEquals(i64),
    RunDateEquals(String),
    StatusEquals(String),
}

pub fn compile_dml_predicate(p: DmlPredicate) -> Expr {
    match p {
        DmlPredicate::IdEquals(id) => col("id").eq(lit(id)),
        DmlPredicate::RunDateEquals(v) => col("run_date").eq(lit(v)),
        DmlPredicate::StatusEquals(v) => col("status").eq(lit(v)),
    }
}
```

Generated DML expression rules:

```text id="kd1ndr"
- allowlist columns
- allowlist operators
- type-check literals
- reject raw SQL fragments
- reject non-deterministic functions
- require partition predicate for broad DML where possible
- log debug representation of compiled expression
```

---

## 9.29 Production DML service wrapper

```rust id="ncl4j8"
use deltalake::datafusion::logical_expr::Expr;
use deltalake::operations::write::SessionFallbackPolicy;
use std::sync::Arc;

#[derive(Debug, serde::Serialize)]
pub struct DmlResult<M> {
    pub before_version: Option<u64>,
    pub after_version: Option<u64>,
    pub metrics: M,
}

pub async fn governed_delete(
    ctx: &datafusion::prelude::SessionContext,
    table: deltalake::DeltaTable,
    predicate: Expr,
) -> anyhow::Result<DmlResult<deltalake::operations::delete::DeleteMetrics>> {
    let before_version = table.version();
    require_dml_allowed(&table)?;

    let (table, metrics) = table
        .delete()
        .with_predicate(predicate)
        .with_session_state(Arc::new(ctx.state()))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .await?;

    Ok(DmlResult {
        before_version,
        after_version: table.version(),
        metrics,
    })
}
```

Use wrapper functions for all production DML to centralize:

```text id="8f0ndv"
table-feature validation
append-only rejection
session-state injection
commit metadata
retry policy
metrics logging
version before/after capture
authorization checks
predicate validation
```

---

## 9.30 Diagnostics and error taxonomy

```text id="0zv3iw"
Predicate planning error:
  missing column
  bad alias
  unsupported expression
  type mismatch

Execution error:
  DataFusion scan failure
  object-store read failure
  Parquet decode failure
  memory/spill failure
  UDF error

Rewrite error:
  Parquet writer failure
  cast failure
  constraint violation
  object-store write failure

Commit error:
  concurrent modification conflict
  log-store lock failure
  protocol/table-feature violation
  unknown commit outcome

Semantic error:
  duplicate source keys
  clause order wrong
  not-matched-by-source clause too broad
  delete-all accidentally invoked
```

Recommended DML diagnostic payload:

```rust id="rbwnvu"
#[derive(Debug, serde::Serialize)]
pub struct DmlDiagnostic {
    pub operation: String,
    pub table_uri_hash: String,
    pub before_version: Option<u64>,
    pub after_version: Option<u64>,
    pub predicate_debug: Option<String>,
    pub source_row_count: Option<usize>,
    pub metrics_json: serde_json::Value,
    pub elapsed_ms: u128,
}
```

---

## 9.31 Testing matrix

```text id="pnwyca"
delete:
  delete by primary key
  delete by partition predicate
  delete all rows
  delete predicate matches zero rows
  num_deleted_rows Some vs None behavior
  deterministic predicate only
  append-only rejection

update:
  update by predicate
  update all rows intentionally
  update zero rows
  update multiple columns
  safe_cast=false failure
  safe_cast=true nullable behavior
  append-only rejection

merge:
  update-only matched
  insert-only not-matched
  upsert update+insert
  matched delete
  not-matched-by-source update
  not-matched-by-source delete
  clause ordering
  duplicate source key rejection
  source alias/target alias required
  metrics validation

concurrency:
  concurrent update same key
  concurrent merge overlapping keys
  concurrent delete overlapping partition
  retry idempotent merge
  unknown commit outcome reconciliation

performance:
  high copied-row ratio
  file scan/skipped ratio
  partition-aligned DML
  unpartitioned DML
  compaction before/after DML
```

---

## 9.32 Best practices

```text id="q58feu"
API:
  - use DeltaTable::delete/update/merge
  - capture returned metrics
  - capture before/after versions
  - inject SessionState and RequireSessionState

Predicates:
  - deterministic only
  - partition-aligned where possible
  - expression-generated from allowlisted config
  - no accidental delete-all/update-all

Merge:
  - always use source/target aliases
  - deduplicate source keys
  - order clauses specific-to-general
  - make source deterministic
  - design idempotency before retrying
  - validate metrics after execution

Performance:
  - avoid sparse row changes across huge files
  - monitor copied-row ratios
  - compact small files
  - cluster/Z-order on merge/delete keys where useful
  - tune rewritten-file writer properties

Governance:
  - reject append-only tables
  - reject unsupported column mapping/features
  - attach commit metadata
  - log metrics without secrets
  - run post-DML validation queries
```

---

## 9.33 Anti-patterns

```text id="z3mc77"
- DeltaOps(table).merge(...) in new code
- omitting predicate accidentally on delete/update
- non-deterministic predicates
- merge source with duplicate keys
- unqualified merge columns
- generic update clause before specific delete/update clause
- broad not_matched_by_source_delete without partition/scope guard
- blind retry after unknown commit outcome
- ignoring num_target_rows_copied / num_target_files_scanned
- using DML against append-only tables
- using DML against unsupported column-mapping tables
- logging raw subject IDs in GDPR delete audit logs
```

---

## 9.34 LLM-agent checklist

```text id="ui12ho"
Before generating DML:
  1. Confirm deltalake feature includes "datafusion".
  2. Use DeltaTable methods, not DeltaOps.
  3. Load latest or intentionally pinned table state.
  4. Validate DML allowed by table properties/protocol.
  5. Build deterministic DataFusion Expr predicate.
  6. Inject SessionState.
  7. Set SessionFallbackPolicy::RequireSessionState.
  8. Set with_safe_cast(false) unless null-on-failed-cast is intentional.
  9. Add commit metadata without secrets.
  10. Capture before_version.
  11. Await operation and capture metrics.
  12. Capture after_version.
  13. Validate metrics against expected bounds.
  14. Run post-DML query validation.
  15. Retry only if idempotent and reconciled.
```

---

## 9.35 Value case

DML turns Delta from an append-only persistence target into a **transactional mutable lakehouse table layer** for your Rust DataFusion/Arrow simulation workbench:

```text id="l2ooyn"
delete:
  right-to-delete
  invalid row removal
  scoped table cleanup
  partition/key correction

update:
  status correction
  derived field refresh
  operational state transitions
  deterministic mass adjustments

merge:
  upsert
  CDC ingestion
  dimension maintenance
  staged simulation result reconciliation
  source-to-target synchronization
```

Core invariant:

```text id="ldrl9h"
Every Delta DML operation is a transaction over files, not an in-place row mutation.
```

Production invariant:

```text id="q70tzz"
Every DML call must declare predicate, source uniqueness, clause order, session state, commit identity, metric expectations, and retry semantics.
```

[1]: https://docs.rs/deltalake/latest/deltalake/ "deltalake - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/operations/delete/index.html "deltalake::operations::delete - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/operations/delete/struct.DeleteBuilder.html "DeleteBuilder in deltalake::operations::delete - Rust"
[4]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/operations/delete.rs "delta-rs/crates/core/src/operations/delete.rs at rev 9f922319 · delta-io/delta-rs · GitHub"
[5]: https://docs.rs/deltalake/0.32.3/deltalake/operations/update/struct.UpdateBuilder.html "UpdateBuilder in deltalake::operations::update - Rust (0.32.3 docs.rs page; surface unchanged at the 1.0.0 pinned rev)"
[6]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/operations/update.rs "delta-rs/crates/core/src/operations/update.rs at rev 9f922319 · delta-io/delta-rs · GitHub"
[7]: https://docs.rs/deltalake/latest/deltalake/operations/merge/index.html "deltalake::operations::merge - Rust"
[8]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/operations/merge/mod.rs "delta-rs/crates/core/src/operations/merge/mod.rs at rev 9f922319 · delta-io/delta-rs · GitHub"
[9]: https://docs.delta.io/concurrency-control/?utm_source=chatgpt.com "Concurrency control"


# 10. Change Data Feed — Rust `deltalake` incremental consumption

Version target:

```text id="iu7g8u"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required feature: ["datafusion"]
Typical production feature set: ["rustls", "datafusion", "s3"]
Primary API:
  DeltaTable::scan_cdf()
  CdfLoadBuilder
  CdfLoadBuilder::build(&ctx.state(), filters)
```

CDF is a **row-level change stream over Delta table versions**. It records table-row changes after `delta.enableChangeDataFeed = true` is enabled and exposes the changes as Arrow/DataFusion-readable output with data columns plus metadata columns such as `_change_type`, `_commit_version`, and `_commit_timestamp`. Change Data Feed is not enabled by default; only changes after enablement are captured. ([Delta Lake][1])

---

## 10.1 CDF mental model

```text id="pv59kw"
Delta table:
  append/update/delete/merge/overwrite commits

CDF-enabled table:
  row-level change events become queryable by version/timestamp range

CDF reader:
  scan_cdf()
    -> CdfLoadBuilder
      -> DataFusion ExecutionPlan
        -> Arrow RecordBatch stream

Consumer:
  stores checkpoint = last consumed Delta commit version
  reads [last_consumed + 1, latest_available]
  applies insert/update_preimage/update_postimage/delete semantics
  commits downstream checkpoint atomically with downstream effects
```

CDF is not “listen to object-store events.” It is a **bounded replay over committed Delta table versions**. New CDF records become available with the same transaction that makes the new data available in the table. ([Delta Lake][1])

---

## 10.2 Cargo baseline

```toml id="vigink"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
arrow-cast = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

`scan_cdf` is exposed on `DeltaTable`, and `CdfLoadBuilder::build` returns an `Arc<dyn ExecutionPlan>`, so this section assumes DataFusion integration is enabled and the consumer is comfortable executing DataFusion physical plans into Arrow `RecordBatch`es. ([Docs.rs][2])

---

## 10.3 Enable CDF at table creation

### 10.3.1 Explicit create-table path

```rust id="zvxzvk"
use deltalake::kernel::{DataType, StructField};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_cdf_enabled_simulation_table(
    uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_comment("Canonical simulation outputs with Change Data Feed enabled")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            StructField::not_null("simulation_id", DataType::STRING),
            StructField::not_null("run_id", DataType::STRING),
            StructField::not_null("run_date", DataType::DATE),
            StructField::not_null("unit_id", DataType::STRING),
            StructField::nullable("output_value", DataType::DOUBLE),
            StructField::not_null("created_at", DataType::TIMESTAMP),
        ])
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

For new Delta tables, set the table property `delta.enableChangeDataFeed = true` at creation. This is the safest production posture because CDF does not retroactively capture earlier table changes. ([Delta Lake][1])

### 10.3.2 Write-to-create path with CDF

```rust id="d3i38f"
use deltalake::DeltaTableBuilder;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn create_cdf_table_from_first_batches(
    uri: &str,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let target = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .build()?;

    let table = target
        .write(batches)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_description("Created from first Arrow batch; CDF enabled")
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await?;

    Ok(table)
}
```

`WriteBuilder::with_configuration` sets configuration on a created table, which allows CDF to be turned on during write-driven table initialization. ([Docs.rs][3])

---

## 10.4 Enabling CDF on existing tables

Delta Lake supports setting `delta.enableChangeDataFeed = true` on existing tables, but only changes after enablement are captured. In Rust `deltalake`, prefer enabling CDF during table creation unless your migration workflow explicitly sets table properties and validates the resulting protocol/configuration. ([Delta Lake][1])

Migration posture:

```text id="ttkuuq"
Existing table before CDF:
  versions 0..N do not have CDF semantics

Enable CDF at version N+1:
  CDF read must start at N+1 or later
  downstream consumers need an initial full snapshot load
  then consume CDF for versions after enablement
```

Recommended migration pattern:

```text id="ufphjx"
1. Stop or coordinate writers.
2. Enable table property delta.enableChangeDataFeed=true.
3. Record activation version.
4. Materialize downstream baseline from full table snapshot at activation version.
5. Start CDF consumer from activation_version + 1.
6. Store consumer checkpoint.
```

Do not claim historical row-level changes exist before CDF activation.

---

## 10.5 `scan_cdf` API

```rust id="6lldib"
use datafusion::prelude::SessionContext;
use deltalake::open_table;
use url::Url;

pub async fn build_cdf_execution_plan(
    uri: &str,
    starting_version: u64,
    ending_version: u64,
) -> anyhow::Result<std::sync::Arc<dyn datafusion::physical_plan::ExecutionPlan>> {
    let table = open_table(Url::parse(uri)?).await?;
    let ctx = SessionContext::new();

    let cdf = table
        .scan_cdf()
        .with_starting_version(starting_version)
        .with_ending_version(ending_version)
        .build(&ctx.state(), None)
        .await?;

    Ok(cdf)
}
```

`CdfLoadBuilder` exposes `with_starting_version`, `with_ending_version`, `with_starting_timestamp`, `with_ending_timestamp`, `with_allow_out_of_range`, `with_session_state`, and `build`. Starting version defaults to version `0` if not provided; ending version is inclusive; `build` returns an `Arc<dyn ExecutionPlan>`. ([Docs.rs][4])

---

## 10.6 Version range semantics

```rust id="xvz7dt"
let plan = table
    .scan_cdf()
    .with_starting_version(10)
    .with_ending_version(25)
    .build(&ctx.state(), None)
    .await?;
```

Version and timestamp starts/ends are **inclusive** in Delta CDF queries. Supplying only a start version/timestamp reads from that point through the latest available version; supplying a lower start than the CDF activation point raises an error because the feed was not enabled for that earlier range. ([Delta Lake][1])

Consumer checkpoint rule:

```text id="s8ivtr"
If checkpoint.last_consumed_version = 25:
  next_starting_version = 26
  next_ending_version = latest_available_version
```

Never re-read from the last consumed version unless your sink is idempotent and deduplicates by `_commit_version` plus primary key/change type.

---

## 10.7 Timestamp range semantics

```rust id="eb0vbx"
use chrono::{TimeZone, Utc};
use datafusion::prelude::SessionContext;
use deltalake::open_table;
use url::Url;

pub async fn cdf_by_timestamp_range(
    uri: &str,
) -> anyhow::Result<std::sync::Arc<dyn datafusion::physical_plan::ExecutionPlan>> {
    let table = open_table(Url::parse(uri)?).await?;
    let ctx = SessionContext::new();

    let start = Utc
        .with_ymd_and_hms(2026, 6, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid start timestamp"))?;

    let end = Utc
        .with_ymd_and_hms(2026, 6, 2, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid end timestamp"))?;

    let plan = table
        .scan_cdf()
        .with_starting_timestamp(start)
        .with_ending_timestamp(end)
        .build(&ctx.state(), None)
        .await?;

    Ok(plan)
}
```

Prefer version checkpoints for production consumers. Use timestamps for user-facing ad hoc reads, audits, and backfills where exact Delta versions are not known.

---

## 10.8 Out-of-range behavior

```rust id="dg5x5z"
let plan = table
    .scan_cdf()
    .with_starting_version(start)
    .with_ending_version(end)
    .with_allow_out_of_range()
    .build(&ctx.state(), None)
    .await?;
```

`with_allow_out_of_range` allows an ending version or timestamp to exceed the last commit. This is useful for polling loops that compute a speculative end boundary, but it should not hide a stale checkpoint or a missing table-history problem. ([Docs.rs][4])

Production rule:

```text id="jr77mk"
Use with_allow_out_of_range for:
  polling consumers
  bounded catch-up to "latest or less"
  eventually consistent scheduler windows

Avoid it for:
  compliance replay
  exact audit extraction
  migration verification
  "must include version X" semantics
```

---

## 10.9 CDF schema

CDF output contains the table’s data columns plus these metadata columns:

```text id="3z23d0"
_change_type: String
  values:
    insert
    update_preimage
    update_postimage
    delete

_commit_version: Long
  Delta log/table version containing the change

_commit_timestamp: Timestamp
  commit timestamp used by the CDF reader; at this pin, `inCommitTimestamp`
  is preferred when present, with ordinary `commitInfo.timestamp` as fallback
```

`update_preimage` is the value before an update; `update_postimage` is the value after an update. Delta documentation states that CDF reads use the latest table version’s schema by default, with column-mapping caveats. ([Delta Lake][1])

Canonical consumer row identity:

```text id="x62ns2"
event identity:
  table_uri_hash
  _commit_version
  _change_type
  business_key
  optional row hash
```

Do not assume `_commit_timestamp` is unique or monotonic enough for exactly-once ordering by itself. `_commit_version` is the authoritative ordering key. The current reader understands Delta **in-commit timestamps** for timestamp-range filtering and `_commit_timestamp` projection when they are present; delta-rs still does not rely on that as the consumer checkpoint identity.

---

## 10.10 Change type semantics

```text id="uwlqt1"
insert:
  new row appears in table

delete:
  row removed from active table state

update_preimage:
  row before update

update_postimage:
  row after update
```

Consumer mapping:

```text id="0anc0c"
insert:
  apply upsert/insert to downstream target

delete:
  apply delete/tombstone to downstream target

update_preimage:
  normally ignore for current-state materialization
  retain for audit/history/event sourcing

update_postimage:
  apply upsert/update to downstream current-state target
```

For audit/event-sourcing tables, preserve all four change types. For current-state projections, usually consume `insert`, `delete`, and `update_postimage`, while using `update_preimage` only for debugging or history.

---

## 10.11 Executing CDF as a DataFusion physical plan

The delta-rs CDF Rust docs build a CDF `ExecutionPlan` and then execute every output partition with `stream.execute(partition, ctx.task_ctx())`, collecting each resulting Arrow stream into `RecordBatch`es. ([delta-io.github.io][5])

```rust id="aw79gq"
use arrow_array::RecordBatch;
use datafusion::physical_plan::{collect_sendable_stream, ExecutionPlan};
use datafusion::prelude::SessionContext;
use std::sync::Arc;

pub async fn collect_cdf_plan(
    ctx: SessionContext,
    plan: Arc<dyn ExecutionPlan>,
) -> anyhow::Result<Vec<RecordBatch>> {
    let mut batches = Vec::new();
    let partitions = plan.properties().output_partitioning().partition_count();

    for partition in 0..partitions {
        let stream = plan.execute(partition, ctx.task_ctx())?;
        let partition_batches = collect_sendable_stream(stream).await?;
        batches.extend(partition_batches);
    }

    Ok(batches)
}
```

End-to-end CDF batch extraction:

```rust id="jv2x1q"
use datafusion::prelude::SessionContext;
use deltalake::open_table;
use url::Url;

pub async fn read_cdf_batches(
    uri: &str,
    starting_version: u64,
    ending_version: u64,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let table = open_table(Url::parse(uri)?).await?;
    let ctx = SessionContext::new();

    let state = ctx.state();
    table.update_datafusion_session(&state)?;

    let plan = table
        .scan_cdf()
        .with_starting_version(starting_version)
        .with_ending_version(ending_version)
        .build(&state, None)
        .await?;

    collect_cdf_plan(ctx, plan).await
}
```

Call `update_datafusion_session` when your table is object-store-backed or when using custom DataFusion runtime state; it registers the table’s root object store if missing and does not overwrite stale mappings. ([Docs.rs][2])

---

## 10.12 Filtering CDF output

`CdfLoadBuilder::build` accepts an optional physical expression filter, but the ergonomic path for most service code is to read the bounded CDF range into a DataFusion-compatible batch/table stage or to apply downstream predicates after collection. The builder signature is:

```rust id="t48hde"
pub async fn build(
    &self,
    session: &dyn Session,
    filters: Option<&Arc<dyn PhysicalExpr>>,
) -> Result<Arc<dyn ExecutionPlan>, DeltaTableError>
```

The current docs expose this `filters` parameter as an optional `Arc<dyn PhysicalExpr>`, which is lower-level than normal `Expr` / DataFrame filtering. ([Docs.rs][4])

Policy:

```text id="tn34so"
Use version windows first:
  smallest required [start, end] range

Use physical filters only when:
  you already generate DataFusion physical expressions safely
  you have tests proving plan behavior
  you need early filtering for large CDF windows

Otherwise:
  materialize bounded CDF batches
  register MemTable or write to staging Delta
  filter with DataFrame/SQL in a governed path
```

---

## 10.13 Incremental consumer checkpoint

```rust id="g7vezc"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CdfCheckpoint {
    pub source_table_uri: String,
    pub source_table_id: Option<String>,
    pub last_consumed_version: u64,
    pub updated_at_utc: String,
    pub consumer_name: String,
}

impl CdfCheckpoint {
    pub fn next_starting_version(&self) -> u64 {
        self.last_consumed_version + 1
    }
}
```

Checkpoint invariant:

```text id="t7s2my"
Consumer checkpoint must advance only after downstream side effects are durable.
```

For Delta-to-Delta materialization:

```text id="tmrxco"
1. Read CDF range [checkpoint + 1, latest].
2. Apply CDF changes to downstream table.
3. Commit downstream table.
4. Commit checkpoint with ending version.
```

For external sink:

```text id="z7a3ta"
1. Read CDF range.
2. Write changes to sink with idempotency key.
3. Verify sink commit/ack.
4. Update checkpoint.
```

Do not store checkpoint only in process memory.

---

## 10.14 Polling loop skeleton

```rust id="h3fm3o"
use deltalake::open_table;
use url::Url;

pub async fn consume_available_cdf_once(
    uri: &str,
    checkpoint: &mut CdfCheckpoint,
) -> anyhow::Result<Option<Vec<arrow_array::RecordBatch>>> {
    let table = open_table(Url::parse(uri)?).await?;

    let latest = table.get_latest_version().await?;
    let start = checkpoint.next_starting_version();

    if start > latest {
        return Ok(None);
    }

    let end = latest;
    let batches = read_cdf_batches(uri, start, end).await?;

    // Downstream processing must happen before advancing checkpoint.
    checkpoint.last_consumed_version = end;
    checkpoint.updated_at_utc = chrono::Utc::now().to_rfc3339();

    Ok(Some(batches))
}
```

Use bounded windows for large catch-up:

```text id="ylw4m8"
max_versions_per_batch = 100
end = min(latest, start + max_versions_per_batch - 1)
```

---

## 10.15 Incremental downstream materialization

### Current-state projection

```text id="vfg5fq"
input CDF:
  insert
  update_postimage
  delete
  update_preimage

current-state sink:
  insert -> upsert row
  update_postimage -> upsert row
  delete -> delete/tombstone row
  update_preimage -> ignore
```

Example classifier:

```rust id="6tcwh2"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdfChangeType {
    Insert,
    UpdatePreimage,
    UpdatePostimage,
    Delete,
}

impl std::str::FromStr for CdfChangeType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "insert" => Ok(Self::Insert),
            "update_preimage" => Ok(Self::UpdatePreimage),
            "update_postimage" => Ok(Self::UpdatePostimage),
            "delete" => Ok(Self::Delete),
            other => anyhow::bail!("unknown CDF _change_type: {other}"),
        }
    }
}
```

### Audit/event table

```text id="plrxsc"
store all CDF rows:
  business columns
  _change_type
  _commit_version
  _commit_timestamp
  ingestion_timestamp
  source_table_id
  source_table_uri_hash
  consumer_batch_id
```

Audit storage should preserve `update_preimage` and `update_postimage`; current-state projections usually do not.

---

## 10.16 Simulation-state event sourcing

For your engineering workbench, CDF can be used as a **state-change event bus** for simulation table changes.

Recommended event streams:

```text id="qn7r95"
simulation_runs_cdf:
  run status, created, completed, failed, invalidated

simulation_outputs_cdf:
  result rows inserted/replaced/deleted

simulation_parameters_cdf:
  user/model parameter changes

scenario_catalog_cdf:
  scenario definitions and metadata changes
```

Event-sourced state projection:

```text id="gjbcwk"
source Delta table:
  CDF-enabled simulation_outputs

consumer:
  reads CDF by version range

projection:
  materialized latest output by run_id/unit_id/metric_id

checkpoint:
  last_consumed_source_version

audit:
  full CDF history table for replay/debug
```

Value for simulation software:

```text id="ze3fkk"
incremental UI refresh
dependency invalidation
downstream cache invalidation
scenario result replay
agent-visible lineage
debugging "why did this value change?"
```

---

## 10.17 CDC interoperability

CDF can bridge Delta writes to external CDC-like systems:

```text id="jlim1t"
Delta CDF -> Kafka topic
Delta CDF -> PostgreSQL upsert/delete sink
Delta CDF -> Redis/cache invalidation
Delta CDF -> downstream Delta silver/gold tables
Delta CDF -> audit/search index
```

Mapping policy:

```text id="zp5cgj"
insert:
  CDC op = c / upsert

update_preimage:
  CDC before image

update_postimage:
  CDC after image / update

delete:
  CDC op = d / tombstone
```

External sink idempotency key:

```text id="8lj2hw"
source_table_id
_commit_version
_change_type
business_key
optional row_hash
```

External sinks should deduplicate against this key because CDF consumers may reprocess a version range after crash/retry.

---

## 10.18 Retention, vacuum, and history availability

CDF data in `_change_data` follows the table retention policy and is removed by `VACUUM` when outside the retention window. Delta may also compute some CDF changes directly from the log rather than storing separate `_change_data` files; insert-only operations and full partition deletes may not generate files in `_change_data`. ([Delta Lake][1])

Retention rule:

```text id="2ex271"
CDF consumer lag must be less than:
  Delta log retention
  deleted-file retention
  VACUUM policy
  _change_data retention
```

Operational guard:

```text id="f0ss7o"
Before VACUUM:
  check all CDF consumer checkpoints
  verify no consumer needs old versions
  alert on lagging consumers
  do not vacuum away required CDF ranges
```

If consumers can be offline longer than retention, maintain a durable CDF audit table or downstream event log.

---

## 10.19 Schema evolution and compatibility

CDF output uses table data columns plus metadata columns. By default, Delta documentation says CDF batch reads use the latest table version’s schema; column-mapping tables have additional limitations, especially across non-additive schema changes. ([Delta Lake][1])

Schema-evolution policy:

```text id="lkmc97"
Additive nullable columns:
  generally easiest for CDF consumers

Rename/drop/change type/nullability:
  high-risk
  may break CDF readers
  split reads by version range
  migrate consumers before schema change
  create compatibility projection if needed

Column mapping:
  treat as CDF high-risk
  test against the pinned 1.0.0 rev (9f922319) specifically
```

As of the current open Rust issue, `delta-rs` maintainers note that column mapping is not supported in CDF scans and that an early error was expected but not yet enforced in the reported path. Treat column-mapping + CDF in Rust as unsupported/high-risk until your pinned version proves otherwise in CI. ([GitHub][6])

---

## 10.20 Initial snapshot + CDF catch-up pattern

Since CDF only captures changes after enablement, most production materialization uses:

```text id="bi8utm"
1. Load source table full snapshot at version V.
2. Materialize baseline downstream table.
3. Store checkpoint last_consumed_version = V.
4. Consume CDF from V + 1 onward.
```

Rust baseline capture:

```rust id="b2xh04"
pub async fn initialize_consumer_from_snapshot(
    uri: &str,
    consumer_name: &str,
) -> anyhow::Result<CdfCheckpoint> {
    let table = deltalake::open_table(url::Url::parse(uri)?).await?;

    let version = table
        .version()
        .ok_or_else(|| anyhow::anyhow!("source table has no loaded version"))?;

    // Materialize full snapshot separately through DataFusion provider or direct scan.

    Ok(CdfCheckpoint {
        source_table_uri: uri.to_owned(),
        source_table_id: None,
        last_consumed_version: version,
        updated_at_utc: chrono::Utc::now().to_rfc3339(),
        consumer_name: consumer_name.to_owned(),
    })
}
```

This pattern avoids trying to interpret historical versions before CDF activation.

---

## 10.21 CDF to downstream Delta table

Recommended design:

```text id="qg77h2"
source:
  CDF-enabled base table

staging:
  raw CDF rows for bounded version range

projection:
  merge/upsert/delete into current-state Delta table

checkpoint:
  consumer checkpoint Delta table keyed by source + consumer
```

Pseudo-runbook:

```text id="i1z1g5"
1. read source latest version
2. compute [start, end]
3. scan CDF
4. write CDF batch to audit/staging Delta table
5. merge projected changes into downstream Delta table
6. write checkpoint row end_version
```

Do not update checkpoint before downstream merge completes.

---

## 10.22 CDF validation queries

After collecting CDF batches, validate metadata columns exist:

```rust id="12ac4v"
pub fn validate_cdf_schema(batch: &arrow_array::RecordBatch) -> anyhow::Result<()> {
    let schema = batch.schema();

    for required in ["_change_type", "_commit_version", "_commit_timestamp"] {
        anyhow::ensure!(
            schema.index_of(required).is_ok(),
            "CDF output missing required metadata column: {required}"
        );
    }

    Ok(())
}
```

Validate monotonic/version range:

```rust id="4p2x0d"
pub fn validate_cdf_range_metadata(
    batches: &[arrow_array::RecordBatch],
    start: u64,
    end: u64,
) -> anyhow::Result<()> {
    for batch in batches {
        validate_cdf_schema(batch)?;
        let idx = batch.schema().index_of("_commit_version")?;
        let versions = batch
            .column(idx)
            .as_any()
            .downcast_ref::<arrow_array::Int64Array>()
            .ok_or_else(|| anyhow::anyhow!("_commit_version must be Int64Array"))?;

        for row in 0..versions.len() {
            if versions.is_null(row) {
                anyhow::bail!("_commit_version is null at row {row}");
            }

            let v = versions.value(row);
            anyhow::ensure!(v >= start as i64 && v <= end as i64, "CDF commit version out of range: {v}");
        }
    }

    Ok(())
}
```

---

## 10.23 Error taxonomy

```text id="z39s0n"
ChangeDataNotEnabled:
  CDF was not enabled for requested table/range
  fix: enable CDF at creation or start at activation version

InvalidVersion:
  requested start/end version outside history
  fix: check latest version, retention, checkpoint state

ChangeDataInvalidVersionRange:
  ending version earlier than starting version
  fix: validate [start, end] before scan

ChangeDataTimestampGreaterThanCommit:
  timestamp range exceeds commits and out-of-range flag not enabled
  fix: use latest version or with_allow_out_of_range for polling

Object-store/runtime error:
  DataFusion session lacks object-store mapping
  fix: update_datafusion_session(&ctx.state())

Schema incompatibility:
  schema evolved incompatibly across read range
  fix: split ranges, migrate consumer, avoid non-additive changes

Column mapping limitation:
  CDF scan unsupported/high-risk in current Rust path
  fix: avoid column mapping for CDF sources or validate pinned version behavior
```

The delta-rs source tests explicitly cover non-CDF tables returning `ChangeDataNotEnabled`, invalid version ranges, out-of-range versions, timestamp out-of-range behavior, and `with_allow_out_of_range` returning empty results for out-of-range cases. ([GitHub][7])

---

## 10.24 Testing matrix

```text id="1wmxy2"
table creation:
  create CDF-enabled table
  create non-CDF table and assert scan_cdf fails
  enable CDF at creation before first write

range reads:
  starting_version only
  starting_version + ending_version inclusive
  starting_timestamp + ending_timestamp
  invalid start > end
  start below CDF activation version
  end beyond latest with and without allow_out_of_range

change types:
  append -> insert
  delete -> delete
  update -> update_preimage + update_postimage
  merge update/insert/delete behavior
  overwrite behavior

schema:
  additive column before CDF range
  additive column inside CDF range
  non-additive schema change rejection/handling
  column mapping table behavior

retention:
  lagging checkpoint
  vacuumed CDF data
  missing historical version

consumer:
  checkpoint start=end+1
  crash before checkpoint
  crash after downstream write before checkpoint
  duplicate replay idempotency
  downstream current-state materialization
  downstream audit materialization
```

---

## 10.25 Best practices

```text id="jj5mcl"
Enablement:
  - enable CDF at table creation for all incrementally consumed tables
  - store activation version if enabled later
  - do not assume historical CDF exists

Consumption:
  - checkpoint by Delta version, not timestamp
  - read bounded version windows
  - advance checkpoint only after downstream commit
  - deduplicate downstream by source table + commit version + business key/change type
  - preserve all change types in audit sinks

Schema:
  - prefer additive nullable schema evolution
  - avoid column mapping on CDF source tables in Rust until validated
  - avoid non-additive schema changes across active CDF ranges
  - test CDF consumers before schema migrations

Retention:
  - monitor consumer lag
  - coordinate vacuum with CDF checkpoint state
  - persist CDF audit history when long replay windows are required

DataFusion:
  - call update_datafusion_session for object-store-backed tables
  - treat scan_cdf output as an ExecutionPlan
  - collect all partitions
  - validate required metadata columns
```

---

## 10.26 Anti-patterns

```text id="s9tb14"
- enabling CDF after years of history and expecting old row changes
- checkpointing by _commit_timestamp only
- advancing checkpoint before downstream effects are durable
- ignoring update_preimage rows in audit use cases
- treating update_preimage as current-state update
- vacuuming without checking consumer lag
- reading huge unbounded CDF ranges in one batch
- relying on _change_data folder existence for every change
- using column mapping with CDF in Rust without explicit compatibility tests
- performing non-additive schema changes across unconsumed CDF ranges
- assuming scan_cdf returns a DataFrame instead of an ExecutionPlan
```

---

## 10.27 LLM-agent checklist

```text id="h0wb2u"
Before generating CDF code:
  1. Confirm deltalake feature includes "datafusion".
  2. Confirm table property delta.enableChangeDataFeed=true.
  3. Confirm requested start version is at or after CDF activation.
  4. Load DeltaTable with url::Url.
  5. Create/reuse SessionContext.
  6. Call table.update_datafusion_session(&ctx.state()) for object-store tables.
  7. Build scan_cdf().
  8. Set with_starting_version(start).
  9. Set with_ending_version(end) for bounded extraction.
  10. Use with_allow_out_of_range only for polling semantics.
  11. build(&ctx.state(), None).await?.
  12. Execute every output partition.
  13. Validate _change_type, _commit_version, _commit_timestamp columns.
  14. Apply change-type semantics.
  15. Commit downstream effects.
  16. Advance checkpoint to ending version.
  17. Monitor lag vs retention/vacuum policy.
```

---

## 10.28 Value case

CDF gives your Rust DataFusion/Arrow simulation workbench a **transactionally ordered incremental-change substrate**:

```text id="qk9aq7"
incremental UI refresh
derived-table maintenance
cache invalidation
simulation-state event sourcing
audit/change history
CDC export to external systems
debugging value lineage
downstream materialization without full rescans
```

Core invariant:

```text id="ztbujh"
CDF is version-ordered Delta change data; the consumer’s correctness boundary is the persisted last-consumed Delta version.
```

Production invariant:

```text id="1ed0i4"
Enable CDF before the changes you need, consume bounded version windows, commit downstream effects before checkpoint advancement, and keep consumer lag inside retention.
```



## 10.29 In-commit timestamp support in CDF reads (latest pin)

The latest pin adds read-side support for Delta **in-commit timestamps (ICT)**. When a `CommitInfo` contains `inCommitTimestamp`, CDF timestamp-range filtering and the emitted `_commit_timestamp` prefer that value; otherwise the reader falls back to the ordinary `CommitInfo.timestamp`. Version filtering remains the stronger reproducibility boundary, and production checkpoints should continue to persist `_commit_version`.

```text
CDF timestamp precedence:
  CommitInfo.inCommitTimestamp, when present
  -> CommitInfo.timestamp
  -> 0 only for malformed/absent timestamp fallback paths internal to the reader

Consumer checkpoint:
  continue to use Delta version, not timestamp
```

The change improves interoperability with tables written by engines that use the ICT protocol behavior. It does not turn timestamp into a unique transaction identity.

[1]: https://docs.delta.io/delta-change-data-feed/ "Change data feed | Delta Lake"
[2]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html?utm_source=chatgpt.com "WriteBuilder in deltalake::operations::write - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/operations/load_cdf/struct.CdfLoadBuilder.html "CdfLoadBuilder in deltalake::operations::load_cdf - Rust"
[5]: https://delta-io.github.io/delta-rs/usage/read-cdf/ "Reading Change Data - Delta Lake Documentation"
[6]: https://github.com/delta-io/delta-rs/issues/4474?utm_source=chatgpt.com "[Bug]: cdf scan doesn't raise if column mapping is enabled ..."
[7]: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/operations/load_cdf.rs "delta-rs/crates/core/src/operations/load_cdf.rs at rev 9f922319 · delta-io/delta-rs · GitHub"


# 11. Constraints, properties, and governance — Rust `deltalake`

Version target:

```text id="19aeyr"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required feature for expression-backed constraints/DML validation: ["datafusion"]
Typical production feature set: ["rustls", "datafusion", "s3"]
Primary APIs:
  DeltaTable::add_constraint()
  DeltaTable::drop_constraints()
  DeltaTable::set_tbl_properties()
  DeltaTable::add_feature()
  DeltaTable::snapshot()?.protocol()
  DeltaTable::snapshot()?.metadata()
  DeltaTable::snapshot()?.table_config()
```

Current API posture: use `DeltaTable` methods rather than `DeltaOps` for new code. In `deltalake` 1.0.0 @ the pinned rev, `DeltaTable` exposes `add_constraint`, `drop_constraints`, `set_tbl_properties`, `add_feature`, `update_field_metadata`, and `update_table_metadata`; the public crate still exposes `DeltaOps`, but docs mark it deprecated. ([Docs.rs][1])

---

## 11.1 Governance mental model

```text id="owxu1w"
Governance layers:

Schema constraints:
  NOT NULL
  field names
  data types
  nested schema
  partition columns
  field metadata

Check constraints:
  named boolean predicates
  table-level invariants
  enforced on writes
  stored as table properties / metadata

Table properties:
  append-only
  CDF enabled
  retention
  data skipping
  column mapping
  deletion vectors
  checkpoint behavior
  isolation level
  target file size

Protocol:
  min reader version
  min writer version
  reader features
  writer features
  compatibility gate for engines

Service governance:
  allowed operations
  tenant visibility
  schema contract
  property migration policy
  feature enablement approval
```

Delta constraints make tables reject non-conforming data; the delta-rs constraints guide shows adding `id_gt_0` and then receiving an invariant violation when appending invalid rows. ([delta-io.github.io][2])

---

## 11.2 Cargo baseline

```toml id="n9ecv0"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

The constraint builder accepts expressions convertible into Delta/DataFusion expressions and supports session state, commit properties, and custom execution hooks, so keep DataFusion aligned with the `deltalake` baseline for constraint-heavy code. ([Docs.rs][3])

---

## 11.3 Check constraints

### 11.3.1 Add one constraint

```rust id="6kfbn9"
use deltalake::open_table;
use url::Url;

pub async fn add_positive_id_constraint(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = open_table(Url::parse(uri)?).await?;

    let table = table
        .add_constraint()
        .with_constraint("id_gt_0", "id > 0")
        .await?;

    Ok(table)
}
```

`ConstraintBuilder::with_constraint(name, expression)` adds a named constraint, and awaiting the builder returns an updated `DeltaTable`. The official Delta semantics for `ADD CONSTRAINT` require existing rows to satisfy the constraint before the constraint is added. ([Docs.rs][3])

### 11.3.2 Add multiple constraints

```rust id="ru63k2"
use std::collections::HashMap;

pub async fn add_simulation_constraints(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let mut constraints = HashMap::new();

    constraints.insert("output_value_finite", "output_value IS NULL OR output_value = output_value");
    constraints.insert("run_id_not_empty", "length(run_id) > 0");
    constraints.insert("rate_non_negative", "rate IS NULL OR rate >= 0");

    let table = table
        .add_constraint()
        .with_constraints(constraints)
        .await?;

    Ok(table)
}
```

`ConstraintBuilder::with_constraints` accepts a `HashMap` of names to expressions. ([Docs.rs][3])

### 11.3.3 Add constraint with service session + commit metadata

```rust id="qk4ypk"
use deltalake::kernel::transaction::CommitProperties;
use std::sync::Arc;

pub async fn governed_add_constraint(
    ctx: &datafusion::prelude::SessionContext,
    table: deltalake::DeltaTable,
    name: &str,
    expression: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let commit_properties = CommitProperties::default();

    let table = table
        .add_constraint()
        .with_constraint(name, expression)
        .with_session_state(Arc::new(ctx.state()))
        .with_commit_properties(commit_properties)
        .await?;

    Ok(table)
}
```

Use session injection when constraint validation depends on your service’s DataFusion runtime, object-store mappings, function registry, or execution policy.

---

## 11.4 Constraint naming policy

```text id="x5szlb"
Constraint name:
  lowercase_snake_case
  stable across migrations
  semantic, not implementation-only
  no table name prefix unless globally necessary

Good:
  id_gt_0
  output_value_non_negative
  unit_rate_positive
  run_date_not_future
  distillation_cutpoint_order_valid

Bad:
  constraint1
  test
  CHECK_ID
  old_validation_fix
  generated_2026_06_02_abcdef
```

Constraint expression policy:

```text id="zlkbms"
- deterministic only
- no clock-dependent predicates
- no random/uuid functions
- no external-service UDFs
- avoid expensive expressions over large existing tables
- prefer simple column comparisons
- handle nullable columns deliberately
```

---

## 11.5 Constraint violation behavior

```rust id="18x6bo"
use std::sync::Arc;

use arrow_array::{Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use deltalake::protocol::SaveMode;

pub async fn append_invalid_id(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![Arc::new(Int64Array::from(vec![-1]))],
    )?;

    let table = table
        .write(vec![batch])
        .with_save_mode(SaveMode::Append)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

Expected failure class:

```text id="u2er5k"
DeltaProtocolError / invariant violation:
  Check or Invariant (...) violated by value in row: [...]
```

The delta-rs constraints guide shows exactly this pattern: after adding `id > 0`, appending `id = -1` fails with an invariant violation. ([delta-io.github.io][2])

---

## 11.6 Drop constraints

```rust id="zapnxe"
pub async fn drop_constraint(
    table: deltalake::DeltaTable,
    name: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .drop_constraints()
        .with_constraint(name)
        .with_raise_if_not_exists(true)
        .await?;

    Ok(table)
}
```

`DropConstraintBuilder` removes constraints from a table, supports `with_constraint(name)`, `with_raise_if_not_exists(bool)`, commit properties, custom execution hooks, and awaits to an updated `DeltaTable`. ([Docs.rs][4])

Drop policy:

```text id="weq1xe"
Allowed:
  constraint is demonstrably incorrect
  constraint is superseded by stronger invariant
  schema migration requires temporary removal
  test/dev fixture reset

Disallowed by default:
  bypassing bad upstream data
  reducing domain integrity silently
  allowing nullable/negative/invalid values without migration approval
```

---

## 11.7 NOT NULL constraints

```rust id="eoa1s4"
use deltalake::kernel::{DataType, StructField};

pub fn governed_fields() -> Vec<StructField> {
    vec![
        StructField::not_null("simulation_id", DataType::STRING),
        StructField::not_null("run_id", DataType::STRING),
        StructField::not_null("run_date", DataType::DATE),
        StructField::nullable("output_value", DataType::DOUBLE),
    ]
}
```

Delta `NOT NULL` constraints are schema-level constraints specified when creating/changing table schema, not check constraints added with `add_constraint`. Delta documentation also notes nested `NOT NULL` behavior: a not-null field inside a struct makes the parent struct not null, while columns nested inside array or map types do not accept `NOT NULL`. ([Delta Lake][5])

Schema-governance rule:

```text id="zb8q34"
Use NOT NULL for:
  identity keys
  partition columns
  required timestamps
  required foreign-key-like business identifiers

Use CHECK for:
  numeric ranges
  enum-domain values
  inter-column relationships
  conditional validity rules
```

### Dropping a NOT NULL constraint (new at the 1.0.0 pinned rev)

The 1.0.0 line adds a dedicated operation for relaxing nullability — the equivalent of `ALTER TABLE t ALTER COLUMN c DROP NOT NULL`:

```rust
use deltalake::DeltaTable;

// DeltaTable::drop_column_not_null() -> DropColumnNotNullBuilder
let table: DeltaTable = table
    .drop_column_not_null()
    .with_column("output_unit")
    .await?;
```

Verified contract at the pinned rev (`operations/drop_column_not_null.rs`):

```text
- Builder: DropColumnNotNullBuilder — with_column(name),
  with_commit_properties(CommitProperties), with_custom_execute_handler(...)
- Awaits to DeltaResult<DeltaTable> (metadata-only commit; no data rewrite).
- Top-level columns only.
- One-directional by design: only non-nullable -> nullable is supported.
  Tightening (nullable -> NOT NULL) is intentionally NOT provided because it
  would require validating existing data and/or a default value.
- Also reachable via the deprecated DeltaOps::drop_column_not_null, which the
  source marks: "Use DeltaTable::drop_column_not_null instead".
```

---

## 11.8 Table properties

### 11.8.1 Set properties

```rust id="1aeige"
use std::collections::HashMap;

pub async fn set_table_properties(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let properties = HashMap::from([
        ("delta.enableChangeDataFeed".to_owned(), "true".to_owned()),
        ("delta.appendOnly".to_owned(), "true".to_owned()),
    ]);

    let table = table
        .set_tbl_properties()
        .with_properties(properties)
        .with_raise_if_not_exists(false)
        .await?;

    Ok(table)
}
```

`DeltaTable::set_tbl_properties` is listed on `DeltaTable` at the 1.0.0 pinned rev, and the builder supports `with_properties(HashMap<String,String>)`, `with_raise_if_not_exists`, commit properties, and custom hooks. The docs.rs builder page currently resolves to an older-version page for this exact path, so CI-compile this snippet against the pinned rev before promoting it into canonical code templates. ([Docs.rs][1])

### 11.8.2 Create with properties instead of post-create mutation

```rust id="zqg79t"
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_governed_table(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(governed_fields())
        .with_partition_columns(["run_date"])
        .with_configuration([
            ("delta.enableChangeDataFeed", Some("true")),
            ("delta.appendOnly", Some("false")),
        ])
        .await?;

    Ok(table)
}
```

Prefer setting foundational properties at table creation: CDF must be enabled before the changes you need, append-only changes mutability semantics, and protocol-affecting properties can affect cross-engine compatibility.

---

## 11.9 Typed property keys

`TableProperty` is a non-exhaustive enum of typed Delta property keys; current variants include `AppendOnly`, `EnableChangeDataFeed`, `EnableDeletionVectors`, `ColumnMappingMode`, retention properties, data-skipping properties, min reader/writer versions, target file size, isolation level, and checkpoint properties. ([Docs.rs][6])

Important properties:

```text id="ufcrbe"
delta.appendOnly:
  existing records cannot be deleted or updated

delta.enableChangeDataFeed:
  enables change data feed after activation

delta.deletedFileRetentionDuration:
  shortest duration to keep logically deleted files before physical deletion

delta.logRetentionDuration:
  transaction log retention horizon

delta.dataSkippingNumIndexedCols:
  number of indexed columns for stats collection

delta.dataSkippingStatsColumns:
  explicit stats columns

delta.columnMapping.mode:
  column mapping; compatibility-sensitive

delta.enableDeletionVectors:
  deletion vectors; compatibility-sensitive
```

The `TableProperty::AppendOnly` docs state that append-only tables cannot have existing records deleted or existing values updated; `EnableChangeDataFeed` enables CDF; and `DeletedFileRetentionDuration` governs how long logically deleted files are retained before physical deletion. ([Docs.rs][6])

---

## 11.10 Append-only tables

```rust id="pkfb4q"
pub async fn create_append_only_event_log(uri: &str) -> anyhow::Result<deltalake::DeltaTable> {
    deltalake::operations::create::CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_event_log")
        .with_columns([
            deltalake::kernel::StructField::not_null("event_id", deltalake::kernel::DataType::STRING),
            deltalake::kernel::StructField::not_null("event_ts", deltalake::kernel::DataType::TIMESTAMP),
            deltalake::kernel::StructField::nullable("event_type", deltalake::kernel::DataType::STRING),
        ])
        .with_configuration([
            ("delta.appendOnly", Some("true")),
            ("delta.enableChangeDataFeed", Some("true")),
        ])
        .await
        .map_err(Into::into)
}
```

Use append-only for:

```text id="sjj8h4"
event logs
raw ingest facts
immutable simulation run ledger
audit trails
lineage records
append-only telemetry
```

Do not use append-only for:

```text id="t5bxna"
tables requiring delete/update/merge
GDPR/right-to-delete active tables
deduplicated current-state dimensions
replaceWhere partition refresh targets
SCD Type 1/2 tables
```

---

## 11.11 CDF property governance

```rust id="5c4axh"
pub async fn enable_cdf(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let properties = std::collections::HashMap::from([
        ("delta.enableChangeDataFeed".to_owned(), "true".to_owned()),
    ]);

    let table = table
        .set_tbl_properties()
        .with_properties(properties)
        .await?;

    Ok(table)
}
```

Governance policy:

```text id="uidrhn"
Prefer:
  enable CDF at table creation

If enabling later:
  record activation version
  baseline downstream consumers from full snapshot
  start CDF consumption from activation_version + 1
```

CDF only captures changes after it is enabled, so late enablement is a migration event, not a retroactive history generator. ([Docs.rs][6])

---

## 11.12 Protocol inspection

```rust id="ai7fg4"
pub fn inspect_protocol(table: &deltalake::DeltaTable) -> anyhow::Result<()> {
    let state = table.snapshot()?;
    let protocol = state.protocol();

    tracing::info!(
        min_reader_version = protocol.min_reader_version(),
        min_writer_version = protocol.min_writer_version(),
        reader_features = ?protocol.reader_features(),
        writer_features = ?protocol.writer_features(),
        "Delta protocol inspected"
    );

    Ok(())
}
```

`Protocol` exposes `min_reader_version`, `min_writer_version`, `reader_features`, and `writer_features`. These are compatibility gates that tell clients whether they can safely read or write a table. ([Docs.rs][7])

Protocol-governance invariant:

```text id="1xrc8v"
Do not enable a table feature until every required reader and writer in the fleet is certified against the resulting protocol/features.
```

---

## 11.13 Feature compatibility across engines

Delta feature compatibility is table-scoped: enabling some features breaks forward compatibility for clients that do not understand those features. Delta’s compatibility docs list features that require client upgrades, including check constraints, generated columns, column mapping, change data feed, and deletion vectors. ([Delta Lake][8])

Compatibility dimensions:

```text id="azd3vu"
Readers:
  Rust deltalake
  DataFusion
  PyArrow / Python deltalake
  Spark / Databricks
  Polars
  Trino/Presto connectors
  Fabric / OneLake consumers
  external BI engines

Writers:
  Rust deltalake service
  Python deltalake jobs
  Spark jobs
  Databricks jobs
  ingestion workers
  migration tools
```

Before enabling:

```text id="h9vgxv"
- check protocol before
- simulate feature enablement in staging
- open table with every reader
- write with every writer
- run CDF/DML/constraint tests where relevant
- record new protocol/features
- update compatibility matrix
```

---

## 11.14 Add table features

```rust id="4k3yar"
pub async fn add_table_feature(
    table: deltalake::DeltaTable,
    feature_name: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .add_feature()
        .with_feature(feature_name)
        .with_allow_protocol_versions_increase(false)
        .await?;

    Ok(table)
}
```

`AddTableFeatureBuilder` enables table features, supports `with_feature`, `with_features`, and `with_allow_protocol_versions_increase`; that last option controls whether protocol versions may be increased. ([Docs.rs][9])

Production policy:

```text id="kjpo97"
Default:
  with_allow_protocol_versions_increase(false)

Only allow protocol increase when:
  compatibility matrix is complete
  reader/writer fleet is upgraded
  rollback plan exists
  staging validation passed
```

---

## 11.15 Metadata governance

```rust id="rggnve"
pub fn inspect_table_governance(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<()> {
    let state = table.snapshot()?;

    tracing::info!(
        version = state.version(),
        metadata = ?state.metadata(),
        protocol = ?state.protocol(),
        schema = ?state.schema(),
        table_config = ?state.table_config(),
        "Delta governance snapshot"
    );

    Ok(())
}
```

Delta metadata governs:

```text id="7h2d6u"
table id
table name
description/comment
schema
partition columns
configuration/table properties
creation time
```

Do not encode secrets, tokens, user PII, raw credentials, or ephemeral runtime state in table metadata, field metadata, or table properties.

---

## 11.16 Schema governance

Schema invariants:

```text id="5f2q4c"
column name:
  canonical snake_case
  no case-only duplicates
  no user-generated identifiers

type:
  stable Delta/Arrow-compatible type
  decimal precision/scale explicit
  timestamp unit/timezone policy explicit

nullability:
  NOT NULL for identity and partition columns
  nullable only when semantically meaningful

metadata:
  semantic type
  unit family
  source system
  governance classification
```

Recommended schema contract object:

```rust id="12te9i"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaContract {
    pub table_name: String,
    pub version: String,
    pub required_columns: Vec<String>,
    pub partition_columns: Vec<String>,
    pub required_properties: std::collections::HashMap<String, String>,
    pub required_constraints: std::collections::HashMap<String, String>,
}
```

Validation skeleton:

```rust id="es4owx"
pub fn validate_schema_contract(
    table: &deltalake::DeltaTable,
    contract: &SchemaContract,
) -> anyhow::Result<()> {
    let state = table.snapshot()?;
    let schema = state.schema();
    let config = state.table_config();

    for col in &contract.required_columns {
        anyhow::ensure!(
            schema.field_with_name(col).is_ok(),
            "missing required column: {col}"
        );
    }

    for (key, expected) in &contract.required_properties {
        let actual = config.raw.get(key);
        anyhow::ensure!(
            actual == Some(expected),
            "table property mismatch for {key}: expected={expected:?}, actual={actual:?}"
        );
    }

    Ok(())
}
```

Treat this as representative; the exact `table_config` raw accessor should be compiled against your pinned `deltalake` version.

---

## 11.17 Domain-specific simulation invariants

Recommended constraints for simulation/workbench tables:

```text id="uhg4zh"
simulation_outputs:
  output_value IS NULL OR output_value = output_value
  output_value IS NULL OR output_value >= 0              -- only for naturally nonnegative metrics
  length(simulation_id) > 0
  length(run_id) > 0
  run_date >= DATE '2000-01-01'

crude_assays:
  api_gravity > 0
  sulfur_wt_pct >= 0 AND sulfur_wt_pct <= 100
  density_kg_m3 > 0
  boiling_point_c IS NULL OR boiling_point_c > -273.15

unit_operations:
  capacity_bpd IS NULL OR capacity_bpd >= 0
  yield_fraction IS NULL OR (yield_fraction >= 0 AND yield_fraction <= 1)
  temperature_c IS NULL OR temperature_c > -273.15
  pressure_kpa IS NULL OR pressure_kpa >= 0

scenario_inputs:
  parameter_name <> ''
  parameter_value IS NOT NULL
  lower_bound IS NULL OR upper_bound IS NULL OR lower_bound <= upper_bound
```

Caveat:

```text id="4gkvdg"
Only encode universal truths as constraints.
Do not encode scenario-specific assumptions as global constraints unless the table is scenario-scoped.
```

---

## 11.18 Constraint test harness

```rust id="nm1rsv"
use std::sync::Arc;

use arrow_array::{Float64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use deltalake::protocol::SaveMode;

pub async fn assert_constraint_rejects_invalid_batch(
    table: deltalake::DeltaTable,
) -> anyhow::Result<()> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("run_id", DataType::Utf8, false),
        Field::new("output_value", DataType::Float64, true),
    ]));

    let invalid = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["run-001"])),
            Arc::new(Float64Array::from(vec![Some(-1.0)])),
        ],
    )?;

    let result = table
        .write(vec![invalid])
        .with_save_mode(SaveMode::Append)
        .with_cast_safety(false)
        .await;

    anyhow::ensure!(
        result.is_err(),
        "constraint test failed: invalid batch was accepted"
    );

    Ok(())
}
```

Test categories:

```text id="k8jaxi"
positive:
  valid rows accepted
  nullable fields behave as intended
  boundary values accepted

negative:
  invalid rows rejected
  nulls rejected where NOT NULL
  out-of-range values rejected
  enum-domain violations rejected

migration:
  adding constraint to clean existing table succeeds
  adding constraint to dirty existing table fails
  dropping constraint changes write behavior only after migration approval
```

---

## 11.19 Property migration across versions

```rust id="fz1qgk"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyMigration {
    pub migration_id: String,
    pub expected_before_version: Option<u64>,
    pub set_properties: std::collections::HashMap<String, String>,
    pub reason: String,
}

pub async fn apply_property_migration(
    table: deltalake::DeltaTable,
    migration: PropertyMigration,
) -> anyhow::Result<deltalake::DeltaTable> {
    if let Some(expected) = migration.expected_before_version {
        anyhow::ensure!(
            table.version() == Some(expected),
            "unexpected table version before migration"
        );
    }

    let table = table
        .set_tbl_properties()
        .with_properties(migration.set_properties)
        .with_raise_if_not_exists(false)
        .await?;

    Ok(table)
}
```

Migration runbook:

```text id="a4a3rv"
1. Read current table version.
2. Inspect protocol.
3. Inspect table properties.
4. Compare against expected previous contract.
5. Apply property change.
6. Reload table.
7. Validate new properties.
8. Validate new protocol/features.
9. Run read/write compatibility tests.
10. Persist migration audit.
```

---

## 11.20 Governance guard before writes/DML

```rust id="pyu4mn"
pub fn require_table_governance(
    table: &deltalake::DeltaTable,
    allow_mutation: bool,
) -> anyhow::Result<()> {
    let state = table.snapshot()?;
    let protocol = state.protocol();
    let config = state.table_config();

    tracing::debug!(
        min_reader_version = protocol.min_reader_version(),
        min_writer_version = protocol.min_writer_version(),
        reader_features = ?protocol.reader_features(),
        writer_features = ?protocol.writer_features(),
        table_config = ?config,
        "checking Delta table governance before operation"
    );

    if !allow_mutation {
        // Add service-specific rejection path.
    }

    Ok(())
}
```

Use this guard before:

```text id="wum7hp"
append
overwrite
replaceWhere
delete
update
merge
schema evolution
constraint add/drop
property migration
table-feature enablement
vacuum
```

---

## 11.21 Error taxonomy

```text id="rtb7gv"
Constraint violation:
  invalid batch rejected during write
  fix upstream data or revise constraint through migration

Constraint add failure:
  existing data violates proposed constraint
  fix existing data first or use narrower table

Constraint drop missing:
  drop requested non-existent constraint
  choose with_raise_if_not_exists(false) only for idempotent migrations

Append-only violation:
  delete/update/merge attempted on append-only table
  route to approved exception or reject operation

Protocol incompatibility:
  table requires unsupported reader/writer version or feature
  upgrade client or avoid feature

Feature enablement failure:
  protocol increase disallowed or unsupported
  complete compatibility matrix first

Property mismatch:
  table does not match service contract
  block reads/writes until migration or config correction

Schema governance failure:
  missing required column, nullability drift, type drift
  block dependent operations
```

---

## 11.22 Testing matrix

```text id="dsh6vs"
constraints:
  add one constraint
  add multiple constraints
  add constraint to invalid existing table fails
  valid append succeeds
  invalid append fails
  drop existing constraint succeeds
  drop missing constraint with raise=true fails
  drop missing constraint with raise=false succeeds/no-ops

properties:
  create with CDF enabled
  create append-only table
  set table properties post-create
  validate table_config after migration
  reject unapproved property drift

protocol:
  inspect min reader/writer versions
  inspect reader/writer features
  add feature with protocol increase disallowed
  add feature with protocol increase allowed in staging
  cross-engine open/read/write smoke

governance:
  schema contract validation
  field metadata validation
  table metadata validation
  write guard before append
  DML guard before delete/update/merge
  vacuum guard against retention/CDF consumers
```

---

## 11.23 Best practices

```text id="abibg3"
Constraints:
  - add constraints early
  - use stable names
  - use deterministic predicates
  - test positive and negative rows
  - avoid scenario-specific assumptions in global tables

Properties:
  - set foundational properties at creation
  - enable CDF before needed changes
  - use append-only only for immutable tables
  - manage retention with reader/CDF lag
  - avoid protocol-affecting changes without compatibility review

Protocol:
  - inspect before reads/writes
  - maintain engine compatibility matrix
  - block unknown reader/writer features
  - avoid feature enablement in hot paths

Governance:
  - centralize schema/property/constraint contracts
  - store migration IDs in commit metadata
  - validate after every metadata migration
  - make failures hard, explicit, and diagnostic-rich
```

---

## 11.24 Anti-patterns

```text id="1fuwxi"
- using constraints as late-stage data cleaning
- dropping constraints to force bad data through
- enabling CDF after historical changes and expecting old CDF rows
- making mutable tables append-only
- enabling protocol/table features without testing external readers
- setting table properties through scattered string literals
- logging secrets in metadata/property/commit fields
- treating protocol increase as harmless
- using broad constraints that encode scenario-local assumptions
- ignoring null semantics in CHECK expressions
```

---

## 11.25 LLM-agent checklist

```text id="v79mbn"
Before generating constraint/property/governance code:
  1. Open/load DeltaTable.
  2. Inspect table version.
  3. Inspect schema.
  4. Inspect table_config.
  5. Inspect protocol reader/writer versions.
  6. Inspect reader/writer features.
  7. Decide if change is schema constraint, CHECK constraint, table property, or protocol feature.
  8. For CHECK: use add_constraint().with_constraint(name, expression).
  9. For DROP CHECK: use drop_constraints().with_constraint(name).
  10. For properties: use set_tbl_properties().with_properties(...).
  11. For features: use add_feature(); default to protocol-increase disallowed.
  12. Add commit metadata without secrets.
  13. Run compatibility tests.
  14. Run valid/invalid write tests.
  15. Persist migration audit.
```

---

## 11.26 Value case

Constraints, properties, and protocol governance turn Delta tables into enforceable contracts for your Rust DataFusion/Arrow simulation platform:

```text id="xbrzml"
constraints:
  reject invalid domain rows before they become lakehouse state

properties:
  declare mutability, CDF, retention, statistics, and table behavior

protocol/features:
  prevent incompatible readers/writers from corrupting or misreading tables

metadata/schema governance:
  make tables self-describing and service-verifiable

migration controls:
  make table evolution auditable and reproducible
```

Core invariant:

```text id="05nh6d"
A Delta table is not just files; it is a protocol-governed contract.
```

Production invariant:

```text id="gk3ucv"
Every table mutation should pass schema, property, constraint, and protocol governance before it reaches object storage.
```

---

## 11.27 Strict table-feature validation: declared-but-unsupported features fail the whole operation

The 1.0.0 baseline validates table features **wholesale, by declaration** — not by whether the touched data actually uses them. Two verified enforcement layers exist at the pinned rev:

**Layer 1 — delta-rs `ProtocolChecker`** (`kernel/transaction/protocol.rs`): `can_read_from` / `can_write_to` compute the set difference between the features the table protocol declares (reader features for protocol v3+, writer features for v7+, versioned legacy sets below) and the features this build supports. Any non-empty difference fails with `TransactionError::UnsupportedTableFeatures(Vec<TableFeature>)` — even if no row you touch exercises the feature. `can_write_to` also requires all reader features first (writers must be readers).

**Layer 2 — kernel operation gate** (`table_configuration.rs`): kernel scan and CDF construction call `ensure_operation_supported(Operation::Scan|Cdf)`, and kernel writes call `ensure_write_supported`; each iterates **all enabled** reader/writer features and errors on the first one without kernel support, plus min reader/writer version range checks.

Supported sets compiled into the pinned rev (default `datafusion` build):

```text
Reader: timestampNtz, deletionVectors, variantType, variantType-preview,
        v2Checkpoint, columnMapping;
        (+ timestampNanos with the nanosecond-timestamps feature)
Writer: appendOnly, timestampNtz, variantType, variantType-preview,
        v2Checkpoint, changeDataFeed, invariants, checkConstraints,
        generatedColumns, columnMapping, deletionVectors;
        (+ timestampNanos with the nanosecond-timestamps feature)
NOT supported for writes (examples): rowTracking, identityColumns,
        typeWidening, icebergCompat* — a table declaring any of these in writerFeatures
        fails can_write_to() even for writes that never touch them.
```

Change vs 0.32.x: `columnMapping` is in the supported **writer** set (subject to operation-specific restrictions in §4.25), and the current post-baseline pin additionally recognizes **`V2Checkpoint`** in both reader and writer feature sets. Deletion vectors remain supported on both sides. Recognition of `V2Checkpoint` means the protocol checker no longer rejects a table solely because it declares that feature; it does not by itself imply that every V2-checkpoint creation/maintenance workflow is exposed as a new public Rust API.

Operational posture:

```text
- Diagnose failures by reading the protocol first (§11.12): the error names the
  offending features exactly.
- Do not "fix" UnsupportedTableFeatures by editing _delta_log by hand; either
  upgrade delta-rs to a build supporting the feature or stop foreign engines
  from enabling it on shared tables.
- Spark/Databricks can silently enable writer features (e.g. rowTracking on
  DBR-managed tables); gate shared-table creation paths accordingly.
- Test governance with a foreign-engine-created table fixture per feature you
  expect to encounter.
```

[1]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[2]: https://delta-io.github.io/delta-rs/usage/constraints/ "Adding a constraint - Delta Lake Documentation"
[3]: https://docs.rs/deltalake/latest/deltalake/operations/constraints/struct.ConstraintBuilder.html "ConstraintBuilder in deltalake::operations::constraints - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/operations/drop_constraints/struct.DropConstraintBuilder.html "DropConstraintBuilder in deltalake::operations::drop_constraints - Rust"
[5]: https://docs.delta.io/delta-constraints/ "Constraints | Delta Lake"
[6]: https://docs.rs/deltalake/latest/deltalake/enum.TableProperty.html "TableProperty in deltalake - Rust"
[7]: https://docs.rs/deltalake/latest/deltalake/kernel/struct.Protocol.html "Protocol in deltalake::kernel - Rust"
[8]: https://docs.delta.io/versioning/?utm_source=chatgpt.com "How does Delta Lake manage feature compatibility?"
[9]: https://docs.rs/deltalake/latest/deltalake/operations/add_feature/struct.AddTableFeatureBuilder.html "AddTableFeatureBuilder in deltalake::operations::add_feature - Rust"


# 12. Partitioning, layout, and file skipping — Rust `deltalake`

Version target:

```text id="3bqgt5"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required feature: ["datafusion"]
Typical production features: ["rustls", "datafusion", "s3"]
Primary APIs:
  DeltaTable::write(...)
  WriteBuilder::with_partition_columns(...)
  WriteBuilder::with_replace_where(...)
  PartitionFilter
  DeltaTable::get_file_uris(...)
  DeltaTable::get_file_uris_by_partitions(...)
  DeltaTableState::add_actions_table(...)
  DeltaScanConfig::with_parquet_pushdown(...)
  DeltaTable::optimize(...)
```

Current API correction: avoid new examples using `DeltaOps(table).write(...)`; `DeltaOps` is visible but marked deprecated in `deltalake` 1.0.0 @ the pinned rev, and the current write path is `DeltaTable::write(...)`. Also, `WriteBuilder.await` returns a `DeltaTable`, not `(DeltaTable, metrics)`. Optimize/DML builders may return metrics, but write does not. ([Docs.rs][1])

---

## 12.1 Physical layout mental model

```text id="wozng5"
Delta logical table:
  schema
  protocol
  metadata
  transaction log
  active Add actions
  removed-file Tombstone actions

Physical layout:
  Parquet files
  optional Hive-style partition directories
  file-level Delta stats in transaction log
  Parquet row-group/page metadata
  optional compaction/Z-order maintenance

Query pruning stack:
  DataFusion projection pruning
  DataFusion predicate pushdown
  Delta partition pruning
  Delta file skipping from transaction-log min/max/null-count stats
  Parquet row-group/page pruning
  Arrow-level residual filtering
```

Partitioning and file skipping are complementary. Partition pruning skips directory/file groups using known partition values; file skipping uses transaction-log file statistics such as min/max values and null counts; Parquet pruning can then skip row groups or pages after file selection. Delta-rs’ file-skipping docs describe reading transaction-log file paths, sizes, and min/max values, pushing predicates down, and reading only the minimal required file subset. ([delta-io.github.io][2])

---

## 12.2 Cargo baseline

```toml id="ti73wy"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

---

## 12.3 Partition columns

Partition columns are a **physical layout decision**. They determine directory/file organization and pruning behavior, not merely metadata labeling.

### 12.3.1 Create a partitioned table

```rust id="5p0qto"
use deltalake::kernel::{DataType, StructField};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;

pub async fn create_partitioned_simulation_outputs(
    uri: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = CreateBuilder::new()
        .with_location(uri)
        .with_table_name("simulation_outputs")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns([
            StructField::not_null("simulation_id", DataType::STRING),
            StructField::not_null("run_id", DataType::STRING),
            StructField::not_null("run_date", DataType::DATE),
            StructField::not_null("scenario_family", DataType::STRING),
            StructField::not_null("unit_id", DataType::STRING),
            StructField::nullable("output_value", DataType::DOUBLE),
        ])
        .with_partition_columns(["run_date", "scenario_family"])
        .await?;

    Ok(table)
}
```

### 12.3.2 Write to a partitioned new table

```rust id="nchczi"
use deltalake::DeltaTableBuilder;
use deltalake::protocol::SaveMode;
use url::Url;

pub async fn write_new_partitioned_table(
    uri: &str,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let target = DeltaTableBuilder::from_url(Url::parse(uri)?)?
        .build()?;

    let table = target
        .write(batches)
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_table_name("simulation_outputs")
        .with_partition_columns(["run_date", "scenario_family"])
        .await?;

    Ok(table)
}
```

`WriteBuilder::with_partition_columns` applies partitioning for new tables. For existing tables, the specified partition columns must match the table’s current partitioning, except for the narrow case of a full-table overwrite with schema overwrite and no `replaceWhere`, where partitioning may be replaced. ([Docs.rs][3])

---

## 12.4 Low-cardinality partitioning

Good partition columns have:

```text id="vl2o5z"
- frequent equality/range filters
- low or moderate distinct-value count
- sufficiently large data per partition
- stable semantics
- bounded directory count
- operational relevance for overwrite/repair/compaction
```

Examples for simulation/workbench tables:

```text id="zg3xgg"
Good:
  run_date
  scenario_family
  model_version
  tenant_id, when authorization/storage isolation aligns
  simulation_domain
  source_system

Sometimes good:
  refinery_id
  process_area
  region
  crude_family

Usually bad:
  unique run_id
  unit_id, if thousands/millions of values
  simulation_id, if every run is unique
  timestamp to hour/minute/second
  floating-point parameter value
  arbitrary user label
```

Delta’s best-practice guidance is explicit: do not partition by very high-cardinality columns, and partition only when you expect each partition to contain substantial data; the docs use date as the most common partition example and warn against partitioning by a million-user-ID-style column. ([Delta Lake][4])

---

## 12.5 High-cardinality anti-patterns

```text id="y63pxq"
Anti-pattern: partition_by = ["simulation_id", "run_id"]

Likely effects:
  one directory per simulation/run
  tiny files per partition
  expensive object-store listing
  poor compaction efficiency
  slow metadata operations
  weak cross-run analytical scans
  no benefit when queries aggregate across many runs
```

Use this decision matrix:

```text id="me7yjc"
Filter column appears in 80% of queries:
  candidate

Distinct values per day/week are bounded:
  candidate

Average data per partition is large enough:
  candidate

Column is unique per run/row:
  reject

Column is continuous numeric:
  reject

Column is user-entered free text:
  reject

Column changes meaning over time:
  reject
```

Practical simulation example:

```text id="j2ztqs"
Table: simulation_outputs
Typical queries:
  WHERE run_date = ?
  WHERE scenario_family = ?
  WHERE model_version = ?
  WHERE simulation_id IN (...)
  GROUP BY unit_id, metric_id

Layout:
  partition by run_date, scenario_family
  collect stats on simulation_id, run_id, unit_id, metric_id, output_value
  optionally Z-order on simulation_id/run_id/unit_id after compaction
```

---

## 12.6 Partition overwrite vs predicate overwrite

### 12.6.1 Predicate overwrite with `replaceWhere`

```rust id="gomtx2"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::protocol::SaveMode;

pub async fn replace_one_run_date(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
    run_date: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Overwrite)
        .with_replace_where(col("run_date").eq(lit(run_date.to_owned())))
        .await?;

    Ok(table)
}
```

`with_replace_where` is specifically tied to overwrite mode and replaces data matching a predicate; this is the canonical “bounded slice replacement” path for partition/slice refresh. ([Docs.rs][3])

### 12.6.2 Multi-column predicate overwrite

```rust id="r6v4ej"
use deltalake::datafusion::logical_expr::{col, lit};
use deltalake::protocol::SaveMode;

pub async fn replace_scenario_slice(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
    run_date: &str,
    scenario_family: &str,
) -> anyhow::Result<deltalake::DeltaTable> {
    let predicate = col("run_date")
        .eq(lit(run_date.to_owned()))
        .and(col("scenario_family").eq(lit(scenario_family.to_owned())));

    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Overwrite)
        .with_replace_where(predicate)
        .await?;

    Ok(table)
}
```

Use `replaceWhere` for complete, deterministic replacement of a partition-aligned slice:

```text id="dwmoq1"
Good:
  replace one run_date
  replace one run_date + scenario_family
  replace one tenant_id + model_version
  replace one source_system ingestion day

Bad:
  row-level upsert by business key
  CDC application
  sparse correction across many files
  arbitrary partial-row updates
```

For row-level reconciliation, use `merge`; for slice regeneration, use `replaceWhere`.

---

## 12.7 Pre-validating `replaceWhere` input

`replaceWhere` should be treated as a contract: every incoming row must belong to the predicate’s replacement slice.

```rust id="k1x0x7"
use arrow_array::{Array, RecordBatch, StringArray};

pub fn require_string_column_equals(
    batch: &RecordBatch,
    column: &str,
    expected: &str,
) -> anyhow::Result<()> {
    let idx = batch.schema().index_of(column)?;
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow::anyhow!("{column} must be Utf8/StringArray"))?;

    for row in 0..arr.len() {
        anyhow::ensure!(!arr.is_null(row), "{column} is null at row {row}");
        anyhow::ensure!(
            arr.value(row) == expected,
            "{column} mismatch at row {row}: expected={expected}, actual={}",
            arr.value(row)
        );
    }

    Ok(())
}

pub fn validate_replace_slice(
    batches: &[RecordBatch],
    run_date: &str,
    scenario_family: &str,
) -> anyhow::Result<()> {
    for batch in batches {
        require_string_column_equals(batch, "run_date", run_date)?;
        require_string_column_equals(batch, "scenario_family", scenario_family)?;
    }
    Ok(())
}
```

Agent rule:

```text id="8nxp0m"
Before with_replace_where:
  validate every batch row satisfies the predicate
  validate partition columns exist
  validate no nulls in partition columns unless null partitioning is intentional
  log predicate + batch row count + target table version
```

---

## 12.8 Partition filters

`PartitionFilter` has a `key` and `value` and implements tuple conversions from `(key, op, value)` and `(key, op, list)` forms. `DeltaTable` exposes partition-filtered active logical files, relative paths, and full file URIs. ([Docs.rs][5])

### 12.8.1 File URI listing by partition

```rust id="8bkco5"
use deltalake::PartitionFilter;

pub async fn file_uris_for_run_date(
    table: &deltalake::DeltaTable,
    run_date: &str,
) -> anyhow::Result<Vec<String>> {
    let filters = vec![
        PartitionFilter::try_from(("run_date", "=", run_date))?,
    ];

    let uris = table.get_file_uris_by_partitions(&filters).await?;
    Ok(uris)
}
```

### 12.8.2 Multi-value partition filter

```rust id="df7vkr"
use deltalake::PartitionFilter;

pub async fn file_paths_for_scenario_families(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<Vec<deltalake::Path>> {
    let families = ["base_case", "stress_case"];

    let filters = vec![
        PartitionFilter::try_from(("scenario_family", "in", families.as_slice()))?,
    ];

    let paths = table.get_files_by_partitions(&filters).await?;
    Ok(paths)
}
```

### 12.8.3 Active file count diagnostic

```rust id="98derh"
pub fn active_file_count(table: &deltalake::DeltaTable) -> anyhow::Result<usize> {
    Ok(table.get_file_uris()?.count())
}
```

`get_file_uris()` returns URI strings for all active files in the current table version; `get_file_uris_by_partitions` returns URI strings for matching partitions. ([Docs.rs][6])

---

## 12.9 File statistics

Delta keeps active-file metadata in Add actions; `DeltaTableState::add_actions_table(flatten)` exposes an Arrow `RecordBatch` with one row per active file. With `flatten = true`, partition values are prefixed as `partition.`, null counts as `null_count.`, minimum values as `min.`, and maximum values as `max.`; the documented fields include path, file size, modification time, null counts, record counts, min values, max values, and partition values. ([Docs.rs][7])

```rust id="ryo6jz"
pub fn add_action_metadata_batch(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<arrow_array::RecordBatch> {
    let state = table.snapshot()?;
    let batch = state.add_actions_table(true)?;
    Ok(batch)
}
```

Use `add_actions_table(true)` to build diagnostics:

```text id="6b77tj"
- file count by partition
- file size distribution
- missing stats detection
- min/max range analysis
- null-count diagnostics
- small-file inventory
- compaction target selection
- layout regression testing
```

Stats are not guaranteed to exist for every column/file; generated code must treat missing min/max/null-count fields as “cannot prune,” not “no matching rows.”

---

## 12.10 Min/max skipping

```text id="rmsuhu"
Predicate:
  output_value > 100

File A:
  min.output_value = 0
  max.output_value = 50
  prune file

File B:
  min.output_value = 25
  max.output_value = 250
  keep file

File C:
  min/max unavailable
  keep file
```

DataFusion’s pruning model uses statistics such as min/max and null counts to prove that a predicate cannot evaluate to true for a container; if it can prove impossibility, the row group/file/container can be skipped, otherwise it must be kept. ([Docs.rs][8])

Pushdown-friendly predicate patterns:

```sql id="tf0usl"
-- good
WHERE output_value > 100.0

-- good
WHERE run_date = DATE '2026-06-02'

-- often weaker for skipping
WHERE ABS(output_value) > 100.0

-- often weaker / harder to reason about
WHERE CAST(output_value AS STRING) = '100'
```

Agent rule:

```text id="qvux5h"
Generate direct comparisons on physical columns whenever pruning is expected.
Avoid wrapping filter columns in functions unless required.
Normalize literal types before query generation.
```

---

## 12.11 DataFusion predicate pushdown

Delta’s DataFusion scan config has `enable_parquet_pushdown`, and the default is true; the same struct also supports a file-path metadata column, dictionary-wrapped partition values, and a compatible read schema. ([Docs.rs][9])

```rust id="1a7v30"
use deltalake::delta_datafusion::DeltaScanConfig;

pub fn default_pruning_scan_config() -> DeltaScanConfig {
    DeltaScanConfig::new()
        .with_parquet_pushdown(true)
        .with_wrap_partition_values(true)
}
```

Production query shape:

```sql id="yuiauj"
SELECT
  simulation_id,
  run_id,
  unit_id,
  output_value
FROM simulation_outputs
WHERE run_date = DATE '2026-06-02'
  AND scenario_family = 'base_case'
  AND output_value > 0.0
```

Avoid:

```sql id="6ip16n"
SELECT *
FROM simulation_outputs
WHERE CAST(run_date AS VARCHAR) = '2026-06-02'
```

DataFusion’s configuration surface includes Parquet predicate pushdown/pruning-related settings, and its pruning predicate system is designed to skip containers using statistics such as min/max values and null counts. ([datafusion.apache.org][10])

---

## 12.12 Stats-loading pitfalls

`DeltaTableConfig::skip_stats` skips parsing per-file statistics when opening the table. The docs state this defaults to false, and that if stats are skipped, predicated queries using that table instance will scan every file because the cache has no stats; partition pruning is unaffected. ([Docs.rs][11])

Bad for query services:

```rust id="6b33tv"
// Avoid for DataFusion query serving when file skipping matters.
let table = deltalake::DeltaTableBuilder::from_url(url::Url::parse(uri)?)?
    .with_config(deltalake::DeltaTableConfig {
        skip_stats: true,
        ..Default::default()
    })
    .load()
    .await?;
```

Use `skip_stats = true` only for workflows that never need file-stat pruning:

```text id="hmkd37"
- append-only metadata-light writers
- vacuum/filesystem-check-only tooling
- one-off metadata inspection
- services that never run predicate queries
```

---

## 12.13 Table properties for data skipping

Delta table properties include `DataSkippingNumIndexedCols` and `DataSkippingStatsColumns`. `DataSkippingNumIndexedCols` controls how many columns Delta collects statistics for, while `DataSkippingStatsColumns` is a comma-separated list of specific columns and takes precedence over the numeric property. The property docs also warn that changing the numeric stats property changes future stats collection and data-skipping behavior, but does not automatically recollect historical stats. ([Docs.rs][12])

Create or update with explicit stats columns:

```rust id="hri5ru"
use std::collections::HashMap;

pub async fn set_data_skipping_columns(
    table: deltalake::DeltaTable,
) -> anyhow::Result<deltalake::DeltaTable> {
    let properties = HashMap::from([
        (
            "delta.dataSkippingStatsColumns".to_owned(),
            "simulation_id,run_id,unit_id,metric_id,output_value".to_owned(),
        ),
    ]);

    let table = table
        .set_tbl_properties()
        .with_properties(properties)
        .await?;

    Ok(table)
}
```

Stats-column selection:

```text id="gsc3o5"
Good stats columns:
  simulation_id
  run_id
  unit_id
  metric_id
  output_value
  event_ts
  model_version

Less useful stats columns:
  high-cardinality random UUID where files are unclustered
  long free-text payloads
  JSON strings
  columns rarely used in filters
```

---

## 12.14 Layout strategy for simulation outputs

Recommended initial table family:

```text id="a4xzzx"
simulation_outputs:
  partition_by:
    run_date
    scenario_family
  stats_columns:
    simulation_id
    run_id
    unit_id
    metric_id
    output_value
  Z-order candidates:
    simulation_id
    run_id
    unit_id
    metric_id
  target files:
    128 MiB - 512 MiB initial benchmark range

simulation_runs:
  partition_by:
    run_date or model_version
  stats_columns:
    run_id
    simulation_id
    status
    created_at
    completed_at

simulation_events:
  partition_by:
    event_date
  stats_columns:
    simulation_id
    run_id
    event_type
    event_ts
```

Avoid this:

```text id="1x26rl"
partition_by:
  simulation_id, run_id, unit_id, metric_id

Result:
  many tiny partitions
  tiny files
  weak cross-run scans
  expensive object-store metadata operations
```

Better:

```text id="citvsl"
partition_by:
  run_date, scenario_family

Then:
  compact
  collect stats on high-value filter columns
  Z-order on simulation_id/run_id/unit_id if queries filter by those columns
```

---

## 12.15 Scenario/date/run/unit decision table

```text id="og6o19"
Column: run_date
  Partition: usually yes
  Stats: yes
  Z-order: rarely needed if partitioned

Column: scenario_family
  Partition: yes if small bounded enum and frequent filter
  Stats: yes
  Z-order: maybe

Column: simulation_id
  Partition: usually no if high cardinality
  Stats: yes
  Z-order: yes if common equality filter

Column: run_id
  Partition: usually no
  Stats: yes
  Z-order: yes if point lookups / reruns are common

Column: unit_id
  Partition: no unless low cardinality and partition-sized data is large
  Stats: yes
  Z-order: maybe for unit-centric diagnostics

Column: metric_id
  Partition: no/rarely
  Stats: yes
  Z-order: maybe for metric dashboards

Column: output_value
  Partition: no
  Stats: yes
  Z-order: rarely, unless range filters dominate
```

Partitioning should be based on query pruning and partition size, not on the semantic importance of the column.

---

## 12.16 Partition evolution risk

`WriteBuilder::with_partition_columns` documents a strict rule for existing tables: requested partitioning must match current partitioning, except full-table overwrite with schema overwrite and no `replaceWhere` may replace partitioning. This means partition evolution is a table migration, not a routine write option. ([Docs.rs][3])

Migration path:

```text id="0yct50"
1. Create new table at new location with new partitioning.
2. Backfill from old table through DataFusion.
3. Validate row counts, checksums, schemas, constraints, CDF policy.
4. Register new table in catalog/pointer layer.
5. Freeze old table or keep it for retention.
6. Vacuum/retire old table after retention and consumer migration.
```

Avoid:

```text id="uzpi3a"
- changing partition columns inside append jobs
- using full overwrite on canonical production location as migration
- mixing files written under old and new partition logic
- allowing writers with different partition specs
```

---

## 12.17 Partition compaction

Compaction is necessary when frequent writes create many small files. Delta’s best-practice docs warn that continuous small-batch writes accumulate many files and degrade read efficiency; the delta-rs optimize builder supports compact and Z-order modes, partition filters, target file size, max concurrent tasks, writer properties, and DataFusion session injection. ([Delta Lake][4])

### 12.17.1 Compact one partition slice

```rust id="ub7vd5"
use deltalake::operations::optimize::OptimizeType;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::PartitionFilter;
use std::num::NonZeroU64;
use std::sync::Arc;

pub async fn compact_run_date_partition(
    ctx: &datafusion::prelude::SessionContext,
    table: deltalake::DeltaTable,
    run_date: &str,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let filters = vec![
        PartitionFilter::try_from(("run_date", "=", run_date))?,
    ];

    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_filters(&filters)
        .with_target_size(NonZeroU64::new(256 * 1024 * 1024).unwrap())
        .with_session_state(Arc::new(ctx.state()))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .await?;

    Ok((table, metrics))
}
```

### 12.17.2 Z-order on common filter keys

```rust id="gef9uk"
use deltalake::operations::optimize::OptimizeType;

pub async fn z_order_simulation_lookup_columns(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::ZOrder(vec![
            "simulation_id".to_owned(),
            "run_id".to_owned(),
            "unit_id".to_owned(),
        ]))
        .await?;

    Ok((table, metrics))
}
```

`OptimizeType` has `Compact` and `ZOrder(Vec<String>)` variants; compaction packs files into predetermined bins, while Z-order lays out files based on provided columns. ([Docs.rs][13])

---

## 12.18 File size strategy

Delta-rs’ file-skipping docs state that smaller files can improve skipping but excessive small files add I/O overhead and slow queries; they recommend “right-sized” files and give 100 MB to 1 GB as a broad range, with compaction available through optimize. ([delta-io.github.io][2])

Initial target-file sizing:

```text id="zzim7b"
small interactive tables:
  64 MiB - 128 MiB

general analytical fact tables:
  128 MiB - 512 MiB

large scan-heavy tables:
  256 MiB - 1 GiB

high-churn DML tables:
  smaller files may reduce rewrite amplification, but benchmark
```

Write path:

```rust id="j7maha"
use std::num::NonZeroU64;
use deltalake::protocol::SaveMode;

pub async fn append_with_file_sizing(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .with_target_file_size(NonZeroU64::new(256 * 1024 * 1024))
        .with_write_batch_size(8192)
        .await?;

    Ok(table)
}
```

`WriteBuilder` exposes `with_target_file_size` for data files and `with_write_batch_size` for Parquet row groups. ([Docs.rs][3])

---

## 12.19 Query-pattern-driven partition design

Build partitioning from real query classes:

```text id="b8ywl4"
Query class A:
  dashboard daily run status
  WHERE run_date = ?
  GROUP BY status
  -> partition run_date

Query class B:
  compare base vs stress scenarios for a date
  WHERE run_date = ? AND scenario_family IN (...)
  -> partition run_date, scenario_family

Query class C:
  inspect one simulation run
  WHERE simulation_id = ? AND run_id = ?
  -> stats + Z-order on simulation_id/run_id
  -> do not partition by run_id unless partitions are large

Query class D:
  aggregate all units for one metric over date range
  WHERE metric_id = ? AND run_date BETWEEN ? AND ?
  -> partition run_date
  -> stats/Z-order metric_id if selective

Query class E:
  unit-level diagnostics
  WHERE unit_id = ? AND run_date = ?
  -> partition run_date
  -> stats/Z-order unit_id if common
```

Partition selection algorithm:

```text id="3byycg"
1. Collect top query predicates.
2. Separate equality/range filters from projections/groupings.
3. Estimate cardinality and bytes per candidate partition.
4. Reject high-cardinality candidates.
5. Select 1-2 partition columns max for most tables.
6. Select stats columns for non-partition filters.
7. Select Z-order columns only after measuring query pain.
8. Set file-size targets.
9. Benchmark with EXPLAIN ANALYZE.
10. Revisit after real workload telemetry.
```

---

## 12.20 Diagnostics: add-actions layout report

```rust id="vtbmzp"
use arrow_array::RecordBatch;

pub fn layout_metadata_batch(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<RecordBatch> {
    let state = table.snapshot()?;
    Ok(state.add_actions_table(true)?)
}
```

DataFusion-based report pattern:

```rust id="x2au8v"
use datafusion::datasource::MemTable;
use datafusion::prelude::*;
use std::sync::Arc;

pub async fn register_add_actions_metadata(
    ctx: &SessionContext,
    table: &deltalake::DeltaTable,
) -> anyhow::Result<()> {
    let batch = layout_metadata_batch(table)?;
    let schema = batch.schema();

    let mem = MemTable::try_new(schema, vec![vec![batch]])?;
    ctx.register_table("_delta_add_actions", Arc::new(mem))?;

    Ok(())
}
```

Example layout SQL:

```sql id="1xczbj"
SELECT
  "partition.run_date" AS run_date,
  COUNT(*) AS file_count,
  SUM(size_bytes) AS total_bytes,
  MIN(size_bytes) AS min_file_bytes,
  MAX(size_bytes) AS max_file_bytes,
  AVG(size_bytes) AS avg_file_bytes
FROM _delta_add_actions
GROUP BY "partition.run_date"
ORDER BY run_date
```

This works because flattened add-action metadata includes file path, file size, partition values, record counts, null counts, and min/max stats when available. ([Docs.rs][7])

---

## 12.21 Query-plan validation

```rust id="4nkxxm"
pub async fn explain_analyze(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> anyhow::Result<Vec<arrow_array::RecordBatch>> {
    let explain_sql = format!("EXPLAIN ANALYZE VERBOSE {sql}");
    Ok(ctx.sql(&explain_sql).await?.collect().await?)
}
```

Validate:

```text id="pjjkq7"
- explicit projection; no unnecessary SELECT *
- filters visible near scan
- partition filters visible in scan/planning diagnostics where exposed
- file count reduced relative to full table
- row group pruning active where available
- no accidental casts around partition/filter columns
- target_partitions not creating excessive overhead
```

DataFusion supports a `target_partitions` execution setting, and too many or too few partitions should be benchmarked against table size, object-store latency, and query shape. ([datafusion.apache.org][14])

---

## 12.22 Layout policy object

```rust id="sh6u6z"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaLayoutPolicy {
    pub table_name: String,
    pub partition_columns: Vec<String>,
    pub stats_columns: Vec<String>,
    pub z_order_columns: Vec<String>,
    pub target_file_size_bytes: u64,
    pub max_small_file_bytes: u64,
    pub min_files_before_compaction: usize,
}

impl DeltaLayoutPolicy {
    pub fn simulation_outputs() -> Self {
        Self {
            table_name: "simulation_outputs".to_owned(),
            partition_columns: vec![
                "run_date".to_owned(),
                "scenario_family".to_owned(),
            ],
            stats_columns: vec![
                "simulation_id".to_owned(),
                "run_id".to_owned(),
                "unit_id".to_owned(),
                "metric_id".to_owned(),
                "output_value".to_owned(),
            ],
            z_order_columns: vec![
                "simulation_id".to_owned(),
                "run_id".to_owned(),
                "unit_id".to_owned(),
            ],
            target_file_size_bytes: 256 * 1024 * 1024,
            max_small_file_bytes: 32 * 1024 * 1024,
            min_files_before_compaction: 100,
        }
    }
}
```

Use a policy object to avoid layout decisions scattered across writers, compaction jobs, UI-generated query services, and LLM agents.

---

## 12.23 Production write wrapper with layout policy

```rust id="b59s5s"
use deltalake::protocol::SaveMode;
use std::num::NonZeroU64;

pub async fn append_with_layout_policy(
    table: deltalake::DeltaTable,
    batches: Vec<arrow_array::RecordBatch>,
    policy: &DeltaLayoutPolicy,
) -> anyhow::Result<deltalake::DeltaTable> {
    let table = table
        .write(batches)
        .with_save_mode(SaveMode::Append)
        .with_partition_columns(policy.partition_columns.clone())
        .with_target_file_size(NonZeroU64::new(policy.target_file_size_bytes))
        .with_write_batch_size(8192)
        .with_cast_safety(false)
        .await?;

    Ok(table)
}
```

For existing tables, `with_partition_columns` must match current partitioning, so this wrapper should validate current table metadata before writing rather than silently trying to change layout. ([Docs.rs][3])

---

## 12.24 Small-file detector

```rust id="w7j2l8"
use arrow_array::{Array, Int64Array, StringArray};

#[derive(Debug, Clone)]
pub struct FileSizeSummary {
    pub total_files: usize,
    pub small_files: usize,
    pub total_bytes: i64,
}

pub fn summarize_file_sizes(
    table: &deltalake::DeltaTable,
    small_file_threshold_bytes: i64,
) -> anyhow::Result<FileSizeSummary> {
    let batch = table.snapshot()?.add_actions_table(true)?;

    let size_idx = batch.schema().index_of("size_bytes")?;
    let sizes = batch
        .column(size_idx)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| anyhow::anyhow!("size_bytes must be Int64Array"))?;

    let mut total_files = 0;
    let mut small_files = 0;
    let mut total_bytes = 0_i64;

    for row in 0..sizes.len() {
        if sizes.is_null(row) {
            continue;
        }

        let size = sizes.value(row);
        total_files += 1;
        total_bytes += size;

        if size < small_file_threshold_bytes {
            small_files += 1;
        }
    }

    Ok(FileSizeSummary {
        total_files,
        small_files,
        total_bytes,
    })
}
```

Trigger compaction when:

```text id="ob3dui"
- small_files / total_files exceeds threshold
- active file count per partition exceeds threshold
- query planning time grows
- object-store GET/LIST cost grows
- EXPLAIN ANALYZE shows scan task explosion
```

---

## 12.25 Partition-specific compaction trigger

```rust id="cjv0m0"
#[derive(Debug, Clone)]
pub struct PartitionCompactionCandidate {
    pub partition_column: String,
    pub partition_value: String,
    pub file_count: usize,
    pub total_bytes: i64,
    pub avg_file_bytes: i64,
}
```

Algorithm:

```text id="f70uc3"
for each partition:
  compute file_count
  compute total_bytes
  compute avg_file_size
  if file_count > min_files_before_compaction
     and avg_file_size < target_file_size / 4:
       schedule optimize with PartitionFilter
```

Use `OptimizeBuilder::with_filters` for partition-scoped compaction; it optimizes only files matching the provided partition filter. ([Docs.rs][15])

---

## 12.26 Layout strategy by table type

```text id="xy376i"
Raw ingestion / bronze:
  partition by ingestion_date
  moderate file sizes
  collect stats on source identifiers
  compact frequently
  avoid excessive Z-order until query patterns stabilize

Simulation outputs / fact:
  partition by run_date + scenario_family
  stats on simulation_id, run_id, unit_id, metric_id, output_value
  target files 128-512 MiB
  Z-order after workload evidence

Dimension/current-state:
  avoid over-partitioning
  stats on business key
  merge-friendly file sizing
  consider Z-order on merge key

Event/audit:
  partition by event_date
  append-only
  CDF as needed
  compact by date partitions

Small lookup tables:
  no partitioning
  compact into few files
  consider in-memory/DataFusion MemTable for hot service joins
```

---

## 12.27 Error taxonomy

```text id="fn1j4v"
Partition mismatch:
  with_partition_columns differs from existing table
  fix: match existing table partitioning or perform explicit migration

replaceWhere predicate mismatch:
  input data outside predicate slice
  fix: pre-validate batches before write

High-cardinality partition explosion:
  many tiny directories/files
  fix: migrate table; partition by coarser dimensions

Missing file stats:
  weak data skipping
  fix: enable/adjust stats columns for future writes; rewrite/optimize to regenerate stats

skip_stats=true load:
  predicated query scans every file
  fix: load with stats for query-serving handles

Tiny-file slowdown:
  high active file count, small avg file size
  fix: optimize/compact partition slices

Query pushdown failure:
  casts/functions around filter columns
  fix: generate direct typed predicates
```

---

## 12.28 Testing matrix

```text id="ko9a3x"
partition creation:
  create partitioned table
  write first partitioned batches
  verify metadata partition columns

existing-table write:
  append with matching partition columns
  append with mismatched partition columns fails
  full overwrite + schema overwrite migration path tested separately

replaceWhere:
  valid slice accepted
  invalid row outside slice rejected/prevented
  partition-aligned replace
  non-partition predicate replace, if allowed by workload policy

partition filters:
  get_file_uris all files
  get_file_uris_by_partitions single value
  get_files_by_partitions multi-value
  missing partition value behavior

stats:
  add_actions_table(true) contains min/max/null_count where expected
  skip_stats=false query path
  skip_stats=true query path scans all files
  stats-column property change affects future writes

performance:
  SELECT * vs explicit projection
  partition filter vs no partition filter
  stats filter vs no stats filter
  compacted vs tiny-file table
  Z-order before/after benchmark
```

---

## 12.29 Best practices

```text id="n9yl8j"
Partitioning:
  - choose partition columns from query predicates
  - avoid high-cardinality partition columns
  - use date/time coarse buckets, not precise timestamps
  - keep partition count operationally bounded
  - treat partition changes as migrations

Writing:
  - use DeltaTable::write, not DeltaOps
  - specify partition columns only when creating or matching existing table
  - use replaceWhere for bounded slice replacement
  - pre-validate replaceWhere input rows

File sizing:
  - avoid one tiny commit/file per source object
  - target 128-512 MiB initially for fact tables
  - benchmark 100 MiB-1 GiB range
  - compact small files by partition

Stats/skipping:
  - keep skip_stats=false for query-serving table handles
  - set dataSkippingStatsColumns for important non-partition filters
  - keep predicates simple and typed
  - inspect add_actions_table for stats coverage

DataFusion:
  - enable parquet pushdown
  - project explicit columns
  - run EXPLAIN ANALYZE for representative queries
  - tune target_partitions with object-store and table-size benchmarks
```

---

## 12.30 Anti-patterns

```text id="vpidsi"
- DeltaOps(table).write(...) in new code
- partitioning by unique simulation_id/run_id by default
- partitioning by unit_id before proving large per-unit partitions
- partitioning by floating-point parameter values
- using SELECT * over wide fact tables
- loading query-serving tables with skip_stats=true
- relying on partitioning alone for all pruning
- using replaceWhere without validating incoming rows
- changing partition columns during normal append jobs
- allowing many writers to create tiny files indefinitely
- optimizing entire table when only one partition is pathological
- enabling Z-order without query evidence
```

---

## 12.31 LLM-agent checklist

```text id="kyta9p"
Before generating layout/write/query code:
  1. Identify top query predicates.
  2. Classify candidate partition columns by cardinality and bytes per partition.
  3. Reject high-cardinality partition candidates.
  4. Select 0-2 partition columns for most tables.
  5. Select stats columns for non-partition predicates.
  6. Use DeltaTable::write.
  7. Use with_partition_columns only for new table or matching existing layout.
  8. Use with_replace_where for slice overwrite.
  9. Validate replaceWhere rows before write.
  10. Set target_file_size for production writes.
  11. Keep skip_stats=false for query-serving handles.
  12. Use DataFusion explicit projections and typed predicates.
  13. Inspect add_actions_table for file size/stats diagnostics.
  14. Compact partition slices when small-file thresholds are exceeded.
  15. Treat partition evolution as a migration.
```

---

## 12.32 Value case

Partitioning, layout, and file skipping make Delta practical for your Rust DataFusion/Arrow simulation platform at scale:

```text id="iq849q"
- lower object-store IO
- lower Parquet scan volume
- faster DataFusion planning/execution
- bounded partition/slice replacement
- better simulation replay/debug queries
- cheaper daily/scenario refreshes
- tractable compaction jobs
- measurable query-shape-to-layout feedback loop
```

Core invariant:

```text id="y0n4f7"
Partitioning is for coarse physical pruning; file statistics and clustering are for fine-grained skipping.
```

Production invariant:

```text id="f8p39k"
Every Delta table should have a documented layout policy: partition columns, stats columns, target file size, compaction threshold, Z-order candidates, and migration procedure.
```

---

## 12.33 Selective stats materialization at the 1.0.0 pinned rev (behavior note)

The pinned rev reworks how per-file statistics are materialized during log replay and scan planning (PRs #4403 “selective stats projection primitives”, #4421 “selective stats materialization policy”, #4428/#4429 memory and cached-stats-field cleanups). This is an **internal behavior change** — the types involved (`StatsProjection`, `AddStatsPolicy` in `kernel/snapshot/`) are `pub(crate)` — but it changes the performance and memory profile that §12.9–§12.12 reasoning rests on:

```text
Verified internal model (crates/core/src/kernel/snapshot/stats_projection.rs, mod.rs):

StatsProjection — what gets parsed into stats_parsed columns:
  None              no parsed file statistics materialized
  Full              full stats schema supported by the table configuration
  NumRecordsOnly    only numRecords
  PredicateColumns  numRecords + min/max/nullCount for the physical data
                    columns referenced by the scan predicate

AddStatsPolicy — what a file-listing request carries:
  None      skip stats during log replay (cached parsed batches may still serve)
  RawJson   keep raw JSON stats + row count, no parsed stats  (datafusion builds)
  Parsed    keep parsed stats and raw JSON stats
```

Practical consequences:

```text
- DeltaScan planning materializes stats for predicate-relevant columns instead
  of eagerly parsing the full stats struct for every file — large-table scans
  with narrow predicates parse far less JSON and hold smaller Arrow buffers.
- Peak memory during snapshot load/validation is lower (duplicate-validation
  memory reduction, PR #4428).
- Partition columns are excluded from the add-stats schema (PR #4408) — do not
  expect partition columns to appear in stats_parsed; pruning on partition
  values uses partitionValues_parsed, not file stats.
- The public `DeltaTableConfig::skip_stats` contract in §12.12 remains: a
  normal predicated query using a table instance opened with `skip_stats=true`
  cannot rely on its file cache for data-skipping statistics and may scan every
  file. Partition pruning is unaffected.
- **New at the latest pin:** internal consumers can request a stronger stats
  capability than the resident cache provides. A lazy snapshot or a
  stats-free materialized cache can replay active adds with raw/parsed stats
  for conflict checking or other explicit internal operations without mutating
  the snapshot's resident cache. Thus `skip_stats` is now a cache/load policy,
  not a blanket claim that no internal operation can ever replay stats.
- Nothing in the public query API needs migration; re-baseline performance
  fixtures that assumed one eager/full-stats materialization model.
```

[1]: https://docs.rs/deltalake/latest/deltalake/ "deltalake - Rust"
[2]: https://delta-io.github.io/delta-rs/how-delta-lake-works/delta-lake-file-skipping/ "File skipping - Delta Lake Documentation"
[3]: https://docs.rs/deltalake/latest/deltalake/operations/write/struct.WriteBuilder.html "WriteBuilder in deltalake::operations::write - Rust"
[4]: https://docs.delta.io/best-practices/ "Best practices | Delta Lake"
[5]: https://docs.rs/deltalake/latest/deltalake/struct.PartitionFilter.html "PartitionFilter in deltalake - Rust"
[6]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[7]: https://docs.rs/deltalake/latest/deltalake/table/state/struct.DeltaTableState.html "DeltaTableState in deltalake::table::state - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_optimizer/pruning/struct.PruningPredicate.html "PruningPredicate in datafusion::physical_optimizer::pruning - Rust"
[9]: https://docs.rs/deltalake/latest/deltalake/delta_datafusion/struct.DeltaScanConfig.html "DeltaScanConfig in deltalake::delta_datafusion - Rust"
[10]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[11]: https://docs.rs/deltalake/latest/deltalake/table/builder/struct.DeltaTableConfig.html "DeltaTableConfig in deltalake::table::builder - Rust"
[12]: https://docs.rs/deltalake/latest/deltalake/enum.TableProperty.html "TableProperty in deltalake - Rust"
[13]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/enum.OptimizeType.html "OptimizeType in deltalake::operations::optimize - Rust"
[14]: https://datafusion.apache.org/ballista/user-guide/tuning-guide.html?utm_source=chatgpt.com "Tuning Guide — Apache DataFusion Ballista documentation"
[15]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/struct.OptimizeBuilder.html "OptimizeBuilder in deltalake::operations::optimize - Rust"


# 13. Optimize, compaction, Z-order, and vacuum — Rust `deltalake`

Version target:

```text id="yys1h5"
deltalake: 1.0.0 (git rev 9f922319, pre-release)
Rust edition: 2024
Rust toolchain/MSRV: 1.94.1
Arrow: 58
Parquet: 58
DataFusion: 54.0.0
object_store: 0.13.2
Required feature for DataFusion-backed optimize execution: ["datafusion"]
Typical production features: ["rustls", "datafusion", "s3"]
Primary APIs:
  DeltaTable::optimize()
  DeltaTable::vacuum()
  DeltaTable::restore()
  DeltaTable::filesystem_check()
  OptimizeType::{Compact, ZOrder}
  VacuumBuilder::{with_dry_run, with_retention_period, with_keep_versions, with_mode}
```

Current API correction: prefer `DeltaTable::optimize()`, `DeltaTable::vacuum()`, `DeltaTable::restore()`, and `DeltaTable::filesystem_check()` over `DeltaOps(table).optimize()/vacuum()/restore()`. `DeltaOps` is still present but deprecated, and its own docs direct users to call methods directly on `DeltaTable`. `OptimizeBuilder` and `VacuumBuilder` still await to `(DeltaTable, Metrics)` / `(DeltaTable, VacuumMetrics)`. ([Docs.rs][1])

---

## 13.1 Maintenance mental model

```text id="qedb6g"
Append / DML / overwrite:
  creates new active Parquet files
  tombstones old files through Remove actions
  does not necessarily delete old files physically

Optimize:
  rewrites many small active files into fewer larger active files
  creates new Add actions
  creates Remove actions for compacted files
  does not physically delete removed files

Vacuum:
  physically deletes stale/tombstoned/unreferenced files after retention policy
  can break time travel beyond retention

Restore:
  creates a new transaction that re-adds files from an older version and removes newer-state files
  requires old files to still exist unless missing files are ignored

Filesystem check:
  audits active log entries against storage
  can remove missing active files from the log
```

Optimize is a logical Delta transaction that compacts files and increments the table version, but it does not delete old storage files; vacuum is the physical cleanup operation. Delta-rs docs explicitly state that optimize creates remove actions for optimized files and that `vacuum` is required to delete those removed files from storage. ([Docs.rs][2])

---

## 13.2 Cargo baseline

```toml id="h1zhgm"
[dependencies]
deltalake = { git = "https://github.com/delta-io/delta-rs.git", rev = "9f9223197469897ef05ae4369eb4fd1390174e65", default-features = false, features = ["rustls", "datafusion", "s3"] }

datafusion = "=54.0.0"
arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-schema = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

`OptimizeBuilder` supports DataFusion session injection, session fallback policy, writer properties, target size, partition filters, concurrency, minimum commit interval, commit properties, and custom execution hooks; therefore production optimize jobs should use the same DataFusion/runtime governance posture as query and DML paths. ([Docs.rs][3])

---

## 13.3 Small-file problem

```text id="70qskw"
Small-file causes:
  frequent micro-batch appends
  one source object = one Delta commit
  streaming-like ingestion without coalescing
  over-partitioning
  high-cardinality partition keys
  repeated DML rewrites
  partition-scoped replaceWhere jobs with tiny batches

Small-file symptoms:
  high active file count
  low average file size
  many object-store GET/LIST operations
  slower query planning
  worse scan scheduling overhead
  higher transaction-log metadata load
  high compaction/vacuum maintenance burden
```

Delta-rs’ optimize guide gives a concrete example: appending every 10 minutes creates 52,560 files per year, and if the table is additionally partitioned by 100 values, the table can grow to millions of files; the docs recommend running optimize periodically and preferably after a partition has finished receiving writes. ([delta-io.github.io][4])

---

## 13.4 Optimize compaction

```rust id="dmd8vx"
use deltalake::operations::optimize::OptimizeType;
use deltalake::open_table;
use url::Url;

pub async fn compact_table(uri: &str) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let table = open_table(Url::parse(uri)?).await?;

    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .await?;

    Ok((table, metrics))
}
```

`OptimizeType::Compact` compacts files into predetermined bins; if `with_type` is omitted, `OptimizeBuilder` defaults to `OptimizeType::Compact`. Optimize’s output type is `(DeltaTable, optimize::Metrics)`. ([Docs.rs][5])

Use compaction for:

```text id="celt2o"
- append-heavy tables
- streaming-like micro-batch tables
- high active file count
- small average file size
- daily partition closure
- post-DML rewrite cleanup
- query-planning overhead reduction
```

Do not use compaction as a substitute for:

```text id="0maamk"
- fixing bad partition design
- preventing duplicate appends
- row-level correctness
- GDPR physical deletion
- table-version retention
- logical DML
```

---

## 13.5 Target file sizes

```rust id="kbd388"
use std::num::NonZeroU64;
use deltalake::operations::optimize::OptimizeType;

pub async fn compact_with_target_size(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let target_size = NonZeroU64::new(256 * 1024 * 1024)
        .ok_or_else(|| anyhow::anyhow!("target size must be non-zero"))?;

    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_target_size(target_size)
        .await?;

    Ok((table, metrics))
}
```

`OptimizeBuilder::with_target_size` sets the target file size, and if no target is supplied, optimize first consults `delta.targetFileSize` from table configuration, otherwise using a default. ([Docs.rs][3])

Starting policy:

```text id="pfjslf"
small interactive tables:
  64 MiB - 128 MiB

general analytical fact tables:
  128 MiB - 512 MiB

large scan-heavy tables:
  256 MiB - 1 GiB

high-churn DML tables:
  benchmark smaller files to reduce rewrite amplification
```

---

## 13.6 Z-order clustering

```rust id="lyhixd"
use deltalake::operations::optimize::OptimizeType;

pub async fn z_order_simulation_keys(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::ZOrder(vec![
            "simulation_id".to_owned(),
            "run_id".to_owned(),
            "unit_id".to_owned(),
        ]))
        .await?;

    Ok((table, metrics))
}
```

`OptimizeType::ZOrder(Vec<String>)` Z-orders files based on the provided columns; Z-ordering is a layout operation that colocates similar values and can improve file skipping when queries filter on those columns. ([Docs.rs][5])

Use Z-order when:

```text id="p3dq2p"
- queries frequently filter on non-partition columns
- those columns have useful min/max/statistics behavior
- table is already compacted enough to avoid tiny-file noise
- predicate selectivity is high
- workload telemetry justifies maintenance cost
```

Avoid Z-order when:

```text id="xxv3yc"
- no stable query pattern exists
- all important filters are already partition columns
- columns are random/unclusterable
- table is very small
- write rate is so high that layout constantly decays
```

Simulation defaults:

```text id="3148hp"
simulation_outputs:
  partition: run_date, scenario_family
  Z-order candidates: simulation_id, run_id, unit_id, metric_id

simulation_runs:
  partition: run_date or model_version
  Z-order candidates: simulation_id, run_id, status

current-state dimensions:
  partition: often none
  Z-order candidates: business_key
```

---

## 13.7 Partition-scoped optimize

```rust id="272u3a"
use deltalake::operations::optimize::OptimizeType;
use deltalake::PartitionFilter;
use std::num::NonZeroU64;

pub async fn compact_run_date_partition(
    table: deltalake::DeltaTable,
    run_date: &str,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let filters = vec![
        PartitionFilter::try_from(("run_date", "=", run_date))?,
    ];

    let target_size = NonZeroU64::new(256 * 1024 * 1024).unwrap();

    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_filters(&filters)
        .with_target_size(target_size)
        .await?;

    Ok((table, metrics))
}
```

`OptimizeBuilder::with_filters` restricts optimization to files that match the provided `PartitionFilter`s. Partition-scoped optimize is normally preferable to whole-table optimize for large partitioned tables because it limits object-store IO, reduces rewrite scope, and avoids repeatedly compacting open/hot partitions. ([Docs.rs][3])

Partition optimize policy:

```text id="6l7qxo"
Optimize closed partitions:
  yesterday's run_date
  completed scenario batch
  completed model_version
  inactive tenant/workspace slice

Avoid optimizing hot partitions:
  actively receiving writes
  high concurrent DML
  incomplete streaming day
  uncertain late-arriving data window
```

---

## 13.8 Optimize with DataFusion session state

```rust id="p5f5zt"
use deltalake::operations::optimize::OptimizeType;
use deltalake::operations::write::SessionFallbackPolicy;
use std::sync::Arc;

pub async fn optimize_with_service_session(
    ctx: &datafusion::prelude::SessionContext,
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_session_state(Arc::new(ctx.state()))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_max_concurrent_tasks(8)
        .await?;

    Ok((table, metrics))
}
```

`OptimizeBuilder::with_session_state` sets the DataFusion session used for planning/execution, and the docs state that if the supplied session is not a concrete `SessionState`, the default behavior is to warn and fall back to internal defaults unless `RequireSessionState` is configured. ([Docs.rs][3])

Production default:

```text id="rm7jgw"
with_session_state(Arc::new(ctx.state()))
with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
```

Reason:

```text id="j09h1f"
- preserve object-store registry
- preserve runtime/spill configuration
- preserve catalog/function state
- prevent silent fallback
- keep maintenance jobs consistent with service runtime
```

---

## 13.9 Parquet writer properties for optimize

```rust id="buntmk"
use deltalake::operations::optimize::OptimizeType;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

pub async fn optimize_with_writer_properties(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::optimize::Metrics)> {
    let writer_properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_write_batch_size(8192)
        .build();

    let (table, metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_writer_properties(writer_properties)
        .await?;

    Ok((table, metrics))
}
```

Optimize rewrites data files, so Parquet writer properties should match the table’s performance and compatibility posture; `OptimizeBuilder::with_writer_properties` passes properties to the Parquet writer. ([Docs.rs][3])

Guidance:

```text id="n21pm1"
SNAPPY:
  fast, broadly compatible, default-friendly

ZSTD:
  better compression, higher CPU cost, benchmark first

Row group sizing:
  align with query predicates and memory budget

Stats:
  preserve useful min/max/null-count behavior for file skipping
```

---

## 13.10 Optimize metrics interpretation

```rust id="cg0cf5"
#[derive(Debug, serde::Serialize)]
pub struct OptimizeMetricLog {
    pub num_files_added: u64,
    pub num_files_removed: u64,
    pub partitions_optimized: u64,
    pub num_batches: u64,
    pub total_considered_files: usize,
    pub total_files_skipped: usize,
}

impl From<deltalake::operations::optimize::Metrics> for OptimizeMetricLog {
    fn from(m: deltalake::operations::optimize::Metrics) -> Self {
        Self {
            num_files_added: m.num_files_added,
            num_files_removed: m.num_files_removed,
            partitions_optimized: m.partitions_optimized,
            num_batches: m.num_batches,
            total_considered_files: m.total_considered_files,
            total_files_skipped: m.total_files_skipped,
        }
    }
}
```

`optimize::Metrics` includes file counts added/removed, detailed add/remove metrics, partitions optimized, number of batches written, total considered files, total skipped files, planner strategy, and stable-order fields. ([Docs.rs][6])

Interpretation:

```text id="5tsy8b"
num_files_removed >> num_files_added:
  successful compaction

num_files_removed == 0:
  no eligible files
  target size too small
  partition filters too narrow
  already compacted table

total_files_skipped high:
  many files were considered but not compacted
  likely already large enough or ineligible

partitions_optimized high:
  broad operation scope
  consider partition-scoped scheduling

num_batches high:
  many output batches/files
  review target size and table skew
```

---

## 13.11 Scheduling optimize jobs

Recommended scheduling triggers:

```text id="xy04qd"
time-based:
  daily after partition close
  hourly for high-ingest tables
  weekly for medium-churn dimensions

threshold-based:
  file_count_per_partition > N
  avg_file_size < target_file_size / 4
  small_file_ratio > 0.5
  query planning time above SLA
  object-store request cost spike

event-based:
  after bulk backfill
  after large merge/update/delete
  after daily simulation run finalization
  before benchmark/report freeze
```

Delta-rs docs recommend optimizing less frequently than appending and, for date-partitioned tables, running optimize after the table has finished writing to a partition so the same data does not need to be compacted repeatedly. ([delta-io.github.io][4])

---

## 13.12 Maintenance policy object

```rust id="up2zh5"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaMaintenancePolicy {
    pub table_name: String,
    pub compact_enabled: bool,
    pub z_order_enabled: bool,
    pub z_order_columns: Vec<String>,
    pub target_file_size_bytes: u64,
    pub min_files_before_compaction: usize,
    pub max_small_file_bytes: u64,
    pub vacuum_enabled: bool,
    pub vacuum_retention_hours: i64,
    pub vacuum_dry_run_first: bool,
    pub keep_versions: Vec<u64>,
}

impl DeltaMaintenancePolicy {
    pub fn simulation_outputs() -> Self {
        Self {
            table_name: "simulation_outputs".to_owned(),
            compact_enabled: true,
            z_order_enabled: true,
            z_order_columns: vec![
                "simulation_id".to_owned(),
                "run_id".to_owned(),
                "unit_id".to_owned(),
            ],
            target_file_size_bytes: 256 * 1024 * 1024,
            min_files_before_compaction: 100,
            max_small_file_bytes: 32 * 1024 * 1024,
            vacuum_enabled: true,
            vacuum_retention_hours: 24 * 30,
            vacuum_dry_run_first: true,
            keep_versions: vec![],
        }
    }
}
```

Use a policy object to prevent table maintenance from being encoded as scattered job-specific constants.

---

## 13.13 Vacuum dry run

```rust id="45y5mn"
use deltalake::open_table;
use url::Url;

pub async fn vacuum_dry_run(
    uri: &str,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::vacuum::VacuumMetrics)> {
    let table = open_table(Url::parse(uri)?).await?;

    let (table, metrics) = table
        .vacuum()
        .with_dry_run(true)
        .await?;

    tracing::info!(
        dry_run = metrics.dry_run,
        candidate_file_count = metrics.files_deleted.len(),
        "vacuum dry run complete"
    );

    Ok((table, metrics))
}
```

`VacuumBuilder::with_dry_run(true)` only determines which files should be deleted; the delta-rs table-management docs state that vacuum dry-runs by default to prevent accidental deletion and that `dry_run=false` is required for physical deletion. ([Docs.rs][7])

Dry-run policy:

```text id="fy3xih"
Always dry-run before:
  first vacuum on a table
  retention-policy change
  post-backfill cleanup
  post-restore cleanup
  GDPR physical cleanup
  production full vacuum
```

---

## 13.14 Vacuum execute

```rust id="rbecjd"
use chrono::TimeDelta;

pub async fn vacuum_execute_with_retention(
    table: deltalake::DeltaTable,
    retention_hours: i64,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::vacuum::VacuumMetrics)> {
    let retention = TimeDelta::try_hours(retention_hours)
        .ok_or_else(|| anyhow::anyhow!("invalid retention hours"))?;

    let (table, metrics) = table
        .vacuum()
        .with_retention_period(retention)
        .with_enforce_retention_duration(true)
        .with_dry_run(false)
        .await?;

    Ok((table, metrics))
}
```

`VacuumBuilder::with_retention_period` overrides the default deletion retention period, and `with_enforce_retention_duration` checks whether the requested retention period is lower than the table minimum. The vacuum docs recommend not using retention under 7 days because old snapshots and uncommitted files can still be used by concurrent readers/writers, and vacuum can break time travel older than the retention period. ([Docs.rs][7])

---

## 13.15 Vacuum keep versions

```rust id="6u3owb"
use chrono::TimeDelta;

pub async fn vacuum_keep_selected_versions(
    table: deltalake::DeltaTable,
    keep_versions: &[u64],
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::vacuum::VacuumMetrics)> {
    let retention = TimeDelta::try_days(30)
        .ok_or_else(|| anyhow::anyhow!("invalid retention"))?;

    let (table, metrics) = table
        .vacuum()
        .with_retention_period(retention)
        .with_keep_versions(keep_versions)
        .with_dry_run(true)
        .await?;

    Ok((table, metrics))
}
```

`VacuumBuilder::with_keep_versions` lets the caller specify table versions to retain for time travel, preventing deletion of files required by those versions. ([Docs.rs][7])

Use `keep_versions` for:

```text id="ylhu5c"
- benchmark baseline versions
- regulatory freeze versions
- model validation snapshots
- reproducibility anchor points
- release-candidate simulation outputs
```

Do not treat `keep_versions` as a replacement for a real archival policy if long-term evidence retention is required.

---

## 13.16 Vacuum modes: lite vs full

```rust id="d7quhg"
use deltalake::operations::vacuum::VacuumMode;

pub async fn full_vacuum_dry_run(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::vacuum::VacuumMetrics)> {
    let (table, metrics) = table
        .vacuum()
        .with_mode(VacuumMode::Full)
        .with_dry_run(true)
        .await?;

    Ok((table, metrics))
}
```

`VacuumBuilder::with_mode` overrides the default mode, which the builder docs describe as “lite.” The `VacuumMode` docs describe `Lite` as removing files referenced by remove actions in `_delta_log`, while `Full` scans storage for data files no longer actively referenced by the transaction log. The `VacuumMode` docs.rs page currently resolves to an older version page in search, so compile this exact variant path in your pinned-rev test harness before making it canonical. ([Docs.rs][7])

Mode policy:

```text id="k7kvv7"
Lite:
  normal scheduled vacuum
  transaction-log-known tombstones
  lower storage listing scope

Full:
  orphan-file cleanup
  after failed/raw file writes
  after table repair
  periodic deep audit
  more expensive object-store scan
```

---

## 13.17 Vacuum metrics

```rust id="18vnxd"
#[derive(Debug, serde::Serialize)]
pub struct VacuumMetricLog {
    pub dry_run: bool,
    pub files_deleted_count: usize,
    pub files_deleted: Vec<String>,
}

impl From<deltalake::operations::vacuum::VacuumMetrics> for VacuumMetricLog {
    fn from(m: deltalake::operations::vacuum::VacuumMetrics) -> Self {
        Self {
            dry_run: m.dry_run,
            files_deleted_count: m.files_deleted.len(),
            files_deleted: m.files_deleted,
        }
    }
}
```

`VacuumMetrics` contains `dry_run` and `files_deleted`; despite the field name, in a dry run this vector represents files that would be deleted, not files physically removed. ([Docs.rs][8])

Metric interpretation:

```text id="ibhz7z"
dry_run=true, files_deleted_count > 0:
  candidates identified; review before execution

dry_run=false, files_deleted_count > 0:
  physical cleanup occurred

dry_run=false, files_deleted_count == 0:
  no eligible tombstoned/orphan files
  retention too long
  append-only table
  optimize/DML did not produce old eligible files
```

---

## 13.18 Time travel breakage after vacuum

Vacuum physically deletes files marked for deletion and older than the retention threshold; after those files are removed, time travel to older versions that require them may fail. Delta’s vacuum docs and Delta Lake blog both emphasize that vacuum saves storage but limits time travel; vacuum does not make current queries faster because tombstoned files are ignored by the transaction-log-selected active file set. ([Docs.rs][9])

Safety invariant:

```text id="m481e0"
Do not vacuum a version range that any user, job, report, model-validation artifact, CDF consumer, or audit workflow still needs.
```

Pre-vacuum checks:

```text id="o0oo75"
- latest table opens successfully
- selected historical versions still open before vacuum
- all CDF consumers are caught up inside retention
- no active writers or long readers over old snapshots
- dry-run candidate count reviewed
- keep_versions supplied for pinned versions
- table retention policy matches business retention policy
```

---

## 13.19 Restore before/after maintenance

```rust id="j21cvc"
pub async fn restore_to_version(
    table: deltalake::DeltaTable,
    version: u64,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::restore::RestoreMetrics)> {
    let (table, metrics) = table
        .restore()
        .with_version_to_restore(version)
        .with_ignore_missing_files(false)
        .with_protocol_downgrade_allowed(false)
        .await?;

    Ok((table, metrics))
}
```

`RestoreBuilder` can restore to a version or datetime, optionally ignore missing files, optionally allow protocol downgrade, and awaits to `(DeltaTable, RestoreMetrics)`. The restore algorithm reads latest state and target state, computes files to re-add and remove, checks availability of restored files unless missing-file ignore is enabled, and commits a new Delta transaction. ([Docs.rs][10])

Restore runbook:

```text id="limx8c"
Before optimize:
  restore normally unnecessary; optimize is logically reversible by time travel until vacuum

Before vacuum:
  validate restore target versions if they must remain usable
  pass keep_versions to vacuum if supported by policy
  do not vacuum away files required by rollback targets

After vacuum:
  restore to old version may fail if required files were physically removed
  with_ignore_missing_files=true is a degraded repair path, not exact restore
```

---

## 13.20 Filesystem check

```rust id="62qtx6"
pub async fn filesystem_check_dry_run(
    table: deltalake::DeltaTable,
) -> anyhow::Result<(deltalake::DeltaTable, deltalake::operations::filesystem_check::FileSystemCheckMetrics)> {
    let (table, metrics) = table
        .filesystem_check()
        .with_dry_run(true)
        .await?;

    Ok((table, metrics))
}
```

`filesystem_check` audits active files in the Delta log against the underlying filesystem/object store; it can remove missing active file entries from the log, and its builder supports dry run, commit properties, and custom execution hooks. Use this as a repair operation when files were accidentally or deliberately deleted outside Delta, not as routine compaction/vacuum. ([Docs.rs][11])

Filesystem-check policy:

```text id="4fg8ys"
Use:
  corrupted-file incident
  accidental manual deletion
  object-store inconsistency investigation
  post-recovery repair workflow

Avoid:
  normal table cleanup
  replacing vacuum
  hiding data-loss incidents
  automated destructive repair without alerting
```

---

## 13.21 Operational runbooks

### 13.21.1 Daily partition compaction

```text id="da5z05"
Inputs:
  table_uri
  partition predicate, e.g. run_date = yesterday
  target_file_size
  max_concurrent_tasks
  maintenance_job_id

Steps:
  1. open latest table
  2. inspect active file count and avg file size for partition
  3. skip if below threshold
  4. run partition-scoped optimize compact
  5. record metrics
  6. run DataFusion query smoke test
  7. schedule vacuum only after retention policy permits
```

### 13.21.2 Z-order maintenance

```text id="uy8lcx"
Inputs:
  table_uri
  columns
  partition scope
  query benchmark set

Steps:
  1. benchmark representative queries before
  2. run compact if table is fragmented
  3. run Z-order on selected columns
  4. benchmark after
  5. compare file skipping / runtime
  6. keep only if benefit exceeds maintenance cost
```

### 13.21.3 Vacuum production run

```text id="covvrw"
Inputs:
  table_uri
  retention_hours
  keep_versions
  mode
  dry_run_candidate_threshold

Steps:
  1. verify no active unsafe writers/readers
  2. inspect table retention and business retention
  3. inspect pinned versions / CDF consumers
  4. dry run
  5. review candidate count/path scope
  6. execute with dry_run=false
  7. validate latest table read
  8. validate retained versions
  9. log metrics
```

---

## 13.22 End-to-end maintenance function

```rust id="wn2e8v"
use chrono::TimeDelta;
use deltalake::operations::optimize::OptimizeType;
use deltalake::operations::vacuum::VacuumMode;
use deltalake::operations::write::SessionFallbackPolicy;
use deltalake::PartitionFilter;
use std::num::NonZeroU64;
use std::sync::Arc;

pub async fn maintain_closed_run_date_partition(
    ctx: &datafusion::prelude::SessionContext,
    table: deltalake::DeltaTable,
    run_date: &str,
) -> anyhow::Result<(
    deltalake::DeltaTable,
    deltalake::operations::optimize::Metrics,
)> {
    let filters = vec![
        PartitionFilter::try_from(("run_date", "=", run_date))?,
    ];

    let target_size = NonZeroU64::new(256 * 1024 * 1024)
        .ok_or_else(|| anyhow::anyhow!("invalid target size"))?;

    let (table, optimize_metrics) = table
        .optimize()
        .with_type(OptimizeType::Compact)
        .with_filters(&filters)
        .with_target_size(target_size)
        .with_session_state(Arc::new(ctx.state()))
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_max_concurrent_tasks(8)
        .await?;

    Ok((table, optimize_metrics))
}

pub async fn dry_run_monthly_vacuum(
    table: deltalake::DeltaTable,
    keep_versions: &[u64],
) -> anyhow::Result<(
    deltalake::DeltaTable,
    deltalake::operations::vacuum::VacuumMetrics,
)> {
    let retention = TimeDelta::try_days(30)
        .ok_or_else(|| anyhow::anyhow!("invalid retention"))?;

    let (table, metrics) = table
        .vacuum()
        .with_mode(VacuumMode::Lite)
        .with_retention_period(retention)
        .with_keep_versions(keep_versions)
        .with_enforce_retention_duration(true)
        .with_dry_run(true)
        .await?;

    Ok((table, metrics))
}
```

This shape intentionally separates optimize from physical deletion: optimize improves active-file layout, while vacuum is retention-governed physical cleanup that can affect historical readability. ([Docs.rs][2])

---

## 13.23 Maintenance policies by table class

```text id="1hqi7q"
simulation_outputs:
  optimize:
    daily partition-scoped compact after run_date closes
    optional Z-order on simulation_id/run_id/unit_id
  vacuum:
    retention based on replay/audit needs
    keep benchmark/release versions
  caution:
    CDF consumers and model-validation snapshots

simulation_event_log:
  optimize:
    periodic compaction by event_date
  vacuum:
    conservative or disabled for audit tables
  caution:
    append-only/event-source retention

current_state_dimensions:
  optimize:
    after heavy merge/update periods
    Z-order on business key
  vacuum:
    moderate retention after DML
  caution:
    restore may be needed for bad merge recovery

raw_ingestion_bronze:
  optimize:
    frequent compaction if micro-batch heavy
  vacuum:
    based on reprocessing policy
  caution:
    raw data may be legal/audit source

temporary/staging:
  optimize:
    usually unnecessary unless large
  vacuum:
    aggressive after job completion
  caution:
    isolate paths from production tables
```

Vacuum is not a query-performance feature; it primarily saves storage by physically deleting tombstoned files, while optimize is the feature intended to reduce small-file read overhead. ([Delta Lake][12])

---

## 13.24 Safety checks before optimize

```rust id="t4havb"
pub fn validate_optimize_allowed(
    table: &deltalake::DeltaTable,
) -> anyhow::Result<()> {
    let state = table.snapshot()?;

    tracing::info!(
        version = state.version(),
        protocol = ?state.protocol(),
        table_config = ?state.table_config(),
        "validating optimize preconditions"
    );

    Ok(())
}
```

Optimize safety checklist:

```text id="z8a9az"
- table opens at latest version
- target partition is closed/inactive
- no concurrent overwrite/delete/merge expected for same files
- object-store credentials are valid
- DataFusion session state is injected
- target file size is policy-compliant
- metrics will be logged
```

Optimize will fail if a concurrent write removes files from the table, such as overwrite; it should succeed if concurrent writers are only appending. ([Docs.rs][2])

---

## 13.25 Safety checks before vacuum

```rust id="k4xwz0"
#[derive(Debug, Clone)]
pub struct VacuumPrecheck {
    pub latest_version: u64,
    pub keep_versions: Vec<u64>,
    pub cdf_consumers_caught_up: bool,
    pub dry_run_required: bool,
    pub active_writer_lockout: bool,
}

pub fn require_vacuum_precheck(precheck: &VacuumPrecheck) -> anyhow::Result<()> {
    anyhow::ensure!(precheck.cdf_consumers_caught_up, "CDF consumers are not caught up");
    anyhow::ensure!(precheck.active_writer_lockout, "active writer lockout not confirmed");
    anyhow::ensure!(precheck.dry_run_required, "vacuum dry-run approval missing");
    Ok(())
}
```

Vacuum safety checklist:

```text id="bad13o"
- dry run completed
- candidate file count reviewed
- retention >= table/business minimum
- long readers finished
- writers paused or safe
- CDF consumers inside retention
- keep_versions supplied for required time-travel points
- latest table query smoke passes
- historical retained versions still load
```

The vacuum module warns that retention shorter than seven days is not recommended because old snapshots and uncommitted files may still be used by concurrent readers or writers; it also warns that vacuum can break time travel to versions older than the retention period. ([Docs.rs][9])

---

## 13.26 Metrics and observability

```rust id="c0ack9"
#[derive(Debug, serde::Serialize)]
pub struct MaintenanceEvent {
    pub table_name: String,
    pub operation: String,
    pub before_version: Option<u64>,
    pub after_version: Option<u64>,
    pub dry_run: Option<bool>,
    pub files_added: Option<u64>,
    pub files_removed: Option<u64>,
    pub files_deleted_count: Option<usize>,
    pub partitions_optimized: Option<u64>,
    pub total_considered_files: Option<usize>,
    pub total_files_skipped: Option<usize>,
}
```

Log:

```text id="kh27ww"
optimize:
  before_version
  after_version
  optimize_type
  partition_filters
  target_file_size
  num_files_added
  num_files_removed
  partitions_optimized
  total_considered_files
  total_files_skipped

vacuum:
  dry_run
  retention_period
  mode
  keep_versions
  candidate/deleted file count
  table_version
  approval/job_id

restore:
  target version/datetime
  ignore_missing_files
  protocol_downgrade_allowed
  files_added/restored
  files_removed

filesystem_check:
  dry_run
  missing active files
  removed log entries
```

`OptimizeBuilder`, `VacuumBuilder`, `RestoreBuilder`, and `FileSystemCheckBuilder` all return metric structs when awaited, making metrics capture a first-class part of maintenance job design. ([Docs.rs][3])

---

## 13.27 Error taxonomy

```text id="15c4lx"
Optimize conflict:
  concurrent overwrite/delete/merge removed files selected by optimize
  fix: retry after refreshing latest state; avoid hot partitions

Optimize no-op:
  no eligible files
  fix: inspect target size, filters, current file sizes

Z-order poor benefit:
  wrong columns, no query selectivity, data too small
  fix: benchmark, change columns, or disable

Vacuum retention failure:
  requested retention below table minimum
  fix: increase retention or explicitly disable enforcement only in controlled cases

Vacuum time-travel breakage:
  historical version needs deleted file
  fix: restore from backup/object-store versioning if available; otherwise version is unrecoverable

Restore missing files:
  vacuum or manual deletion removed required files
  fix: recover physical files or use ignore_missing_files only as degraded repair

Filesystem-check destructive repair:
  active files missing; operation removes log references
  fix: dry run first, investigate root cause, treat as data-loss incident
```

---

## 13.28 Testing matrix

```text id="n630ix"
optimize:
  compact small files
  compact already compacted table no-op
  partition-scoped compact
  Z-order columns
  concurrent append compatibility
  concurrent overwrite conflict
  metrics shape

vacuum:
  dry-run candidate list
  execute vacuum with safe retention
  retention enforcement failure
  keep_versions preserves selected versions
  Lite vs Full behavior where supported
  latest version readable after vacuum
  old version fails after vacuum when expected

restore:
  restore to version before optimize
  restore to version before overwrite
  restore after vacuum failure path
  ignore_missing_files=false failure
  protocol downgrade blocked

filesystem_check:
  dry-run missing active file
  repair missing active file
  query behavior before/after repair
  metrics captured

policy:
  append-only table maintenance
  CDF consumer lag guard
  audit-table no-vacuum policy
  staging-table aggressive vacuum policy
```

---

## 13.29 Best practices

```text id="4cyraj"
Optimize:
  - run less frequently than writes
  - compact closed partitions
  - benchmark target file size
  - use Z-order only with query evidence
  - inject DataFusion SessionState
  - log metrics and before/after versions

Vacuum:
  - always dry-run first
  - keep retention conservative
  - never treat vacuum as performance optimization
  - coordinate with time travel, restore, CDF, and audit requirements
  - use keep_versions for pinned reproducibility versions
  - do not disable retention enforcement casually

Restore:
  - validate before vacuum if rollback matters
  - keep protocol downgrade disabled by default
  - keep missing-file ignore disabled by default
  - treat failed restore after vacuum as expected if files are gone

Filesystem check:
  - dry-run first
  - use only as repair
  - alert on missing active files
  - do not silently hide storage corruption
```

---

## 13.30 Anti-patterns

```text id="pr63vh"
- DeltaOps(table).optimize() in new code
- vacuuming immediately after optimize with unsafe retention
- assuming vacuum speeds up queries
- vacuuming audit tables without retention review
- Z-ordering every column
- optimizing hot partitions repeatedly
- using optimize to fix bad partition design
- disabling retention enforcement in production by default
- forgetting CDF consumers before vacuum
- running filesystem_check as routine cleanup
- allowing restore with protocol downgrade without approval
- ignoring optimize/vacuum metrics
```

---

## 13.31 LLM-agent checklist

```text id="rymo1p"
Before generating optimize code:
  1. Use DeltaTable::optimize().
  2. Select OptimizeType::Compact or OptimizeType::ZOrder.
  3. Use partition filters when table is partitioned.
  4. Set target file size deliberately.
  5. Inject SessionState.
  6. Set RequireSessionState.
  7. Capture before/after version.
  8. Capture optimize metrics.
  9. Avoid hot partitions.
  10. Benchmark with representative queries.

Before generating vacuum code:
  1. Use DeltaTable::vacuum().
  2. Start with with_dry_run(true).
  3. Set retention explicitly for production jobs.
  4. Keep enforcement enabled by default.
  5. Add keep_versions for pinned snapshots.
  6. Check CDF consumer lag.
  7. Confirm no long readers/writers.
  8. Execute dry_run=false only after approval.
  9. Validate latest table read.
  10. Validate retained versions.

Before generating restore/filesystem-check code:
  1. Prefer restore before vacuum removes required files.
  2. Keep ignore_missing_files=false by default.
  3. Keep protocol_downgrade_allowed=false by default.
  4. Dry-run filesystem_check first.
  5. Treat filesystem_check repair as data-loss incident handling.
```

---

## 13.32 Value case

Optimize, compaction, Z-order, and vacuum provide the operational lifecycle for keeping Delta tables performant, cost-controlled, and recoverable:

```text id="5ri6ky"
optimize:
  reduces active small-file count
  improves read scheduling
  improves file skipping when paired with layout/Z-order
  reduces object-store request pressure

Z-order:
  clusters high-value filter columns
  improves min/max-based file skipping
  accelerates point/range lookup workloads

vacuum:
  removes physically stale files
  controls storage cost
  supports compliance cleanup after logical deletes
  trades away older time travel

restore/filesystem_check:
  provides rollback/repair tools
  closes operational safety loop
```

Core invariant:

```text id="ns7zea"
Optimize changes active layout; vacuum changes physical retention.
```

Production invariant:

```text id="322l0m"
Never run maintenance as a blind cleanup job: every optimize, Z-order, vacuum, restore, and filesystem check should have a policy, scope, metrics, retention decision, and rollback/validation plan.
```

### SMARTREF Command Boundary

All SMARTREF Delta mutations enter through `DeltaOperationCommand` and return
`DeltaOperationReceipt`. PyO3 and Python surfaces parse request data into
commands and project receipts; runtime-owned execution methods invoke the Delta
adapter. Direct write, CDF, maintenance, and migration helpers are adapter
internals or test fixtures, not public mutation authority.



## 13.29 Latest-pin `OPTIMIZE` interoperability fixes for nested schemas

Two post-baseline fixes materially improve compaction reliability:

1. **Spark-written nested nullability.** `OPTIMIZE` previously could fail during DataFusion planning when a Delta nested field was logically non-nullable but the Spark-written Parquet field was physically optional. The scan now uses a Delta-aware physical-expression adapter that relaxes nested read nullability and restores the strict logical schema after adaptation; actual illegal null values still fail validation.
2. **Nested field name equals partition column.** The current tip prevents nested struct/list fields from being compared against the table's top-level partition-column list. This avoids accidental partition-style dictionary mapping and the resulting `Expected string, got Dictionary(...)` compaction failure.

Add these cases to maintenance conformance tests even if current production schemas are mostly flat, because CodeFabric-like metadata tables often contain `List`, `Map`, or `Struct` payloads.

[1]: https://docs.rs/deltalake/latest/deltalake/struct.DeltaOps.html "DeltaOps in deltalake - Rust"
[2]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/index.html "deltalake::operations::optimize - Rust"
[3]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/struct.OptimizeBuilder.html "OptimizeBuilder in deltalake::operations::optimize - Rust"
[4]: https://delta-io.github.io/delta-rs/usage/optimize/small-file-compaction-with-optimize/ "Small file compaction - Delta Lake Documentation"
[5]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/enum.OptimizeType.html "OptimizeType in deltalake::operations::optimize - Rust"
[6]: https://docs.rs/deltalake/latest/deltalake/operations/optimize/struct.Metrics.html "Metrics in deltalake::operations::optimize - Rust"
[7]: https://docs.rs/deltalake/latest/deltalake/operations/vacuum/struct.VacuumBuilder.html "VacuumBuilder in deltalake::operations::vacuum - Rust"
[8]: https://docs.rs/deltalake/latest/deltalake/operations/vacuum/struct.VacuumMetrics.html "VacuumMetrics in deltalake::operations::vacuum - Rust"
[9]: https://docs.rs/deltalake/latest/deltalake/operations/vacuum/index.html "deltalake::operations::vacuum - Rust"
[10]: https://docs.rs/deltalake/latest/deltalake/operations/restore/struct.RestoreBuilder.html "RestoreBuilder in deltalake::operations::restore - Rust"
[11]: https://docs.rs/deltalake/latest/deltalake/table/struct.DeltaTable.html "DeltaTable in deltalake::table - Rust"
[12]: https://delta.io/blog/remove-files-delta-lake-vacuum-command/ "Remove old files with the Delta Lake Vacuum Command | Delta Lake"

