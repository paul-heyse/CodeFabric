# CodeFabric data-fabric design principles v2

Revised 2026-09-08. The [selected suite](../authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md) owns product contracts; [AGENTS](../../AGENTS.md) owns development practice. This revision retains P1–P36 identifiers and removes the universal proof/process interpretation. Git preserves earlier text; historical v1 remains readable.

Product invariants (identity, coverage, snapshot consistency, authorization and ownership) are binding. Implementation preferences below are guidance applied with engineering judgment. No principle requires artifacts for code edits, a generalized semantic compiler, universal allocation admission, or a mandatory design/plan/audit cycle.

# P1 — Model semantics before implementing behavior

Clarify the user-visible semantics and uncertainty before choosing implementation details. A short decision is enough when the change is local.

# P2 — Make models executable, not merely descriptive

Implement semantics in ordinary typed Rust and DataFusion plans. Executable models are useful where they simplify the product, not a compulsory generalized semantic compiler.

# P3 — One authoritative owner for every concept

Give each mutable state and semantic concept one owner; avoid competing catalogs, writers or registries.

# P4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation

Use types and interfaces to express actual shared guarantees. Avoid inheritance or abstraction built only for hypothetical variants.

# P5 — Encode variability behind contracts, not throughout consumers

Keep provider differences behind application-owned adapters and explicit capabilities.

# P6 — Separate semantic meaning from execution strategy

Preserve query and fact meaning when changing execution strategy.

# P7 — Build a shared canonical data fabric

Use Arrow for facts, DataFusion for relational execution and native Delta for durable state. Python is presentation.

# P8 — Treat the common representation as infrastructure

Keep a consistent Arrow type universe and explicit schemas at boundaries.

# P9 — Record useful runtime provenance

Runtime actions leave compact source/context/provider/operation/snapshot and outcome records. Code edits use Git; no source-edit artifacts.

# P10 — Trace provenance as needed

Retain enough provenance to explain a fact and diagnose or rebuild its computation. Full independent provenance closure and permanent intermediate histories are not universal prerequisites.

# P11 — Prefer immutable snapshots and explicit state transitions

Queries pin coherent immutable snapshots. Publication is owned and atomic; exact persisted versions permit reopen.

# P12 — Schemas are executable contracts, not documentation

Check schemas at authoritative boundaries. Use typed constructors and focused compatibility tests; do not repeat identical checks and schema registries through every layer.

# P13 — Put governance at the authoritative boundary

Enforce authorization, schema, ownership and coverage where they can affect real operations.

# P14 — Prefer the highest-level extension that preserves the semantics

Prefer native library operations and the highest useful extension point. Custom graph/execution code needs a concrete gap.

# P15 — Preserve optimizer visibility

Keep filters, projections and relational expressions visible to DataFusion where possible.

# P16 — Make lifecycle ownership explicit

Make lifecycle ownership and terminal outcomes clear. Avoid a general phase compiler or ceremony for every intermediate step.

# P17 — Retain intermediates selectively

Retain intermediates when required for recovery, explanation or diagnosis; otherwise rerun from retained inputs when needed. Do not persist every calculation.

# P18 — Fingerprint for identity, never for correctness

Use hashes for identity/integrity, never as evidence that semantic answers are correct.

# P19 — Audit reproducibility proportionately

Use representative clean/incremental and reopen tests and occasional audits. Routine production updates and startup do not independently rerun all computation.

# P20 — Report support and coverage honestly

Separate installed support, per-snapshot processing coverage and release-test confidence. Expose unfinished scope; no generalized executable prover controls every advertised capability.

# P21 — Separate enforced semantics from advisory metadata

Distinguish enforced constraints from advisory statistics or diagnostics. Inexact metadata cannot justify exact negative answers.

# P22 — Use protocols and canonical boundaries for interoperability

Use stable application-owned wire/DTO boundaries. Keep provider lifetimes and backend objects private.

# P23 — Keep state ownership local and explicit

Share DataFusion budgets/spill, bound application work/state/results, own and join tasks, contain providers, measure headroom and recover. Do not promise all native allocations are pre-admitted or OOM is impossible.

# P24 — Make observability semantic, not merely operational

Make freshness, scope, precision and unfinished processing observable alongside operational failures.

# P25 — Validate according to risk

Choose meaningful tests according to risk. No per-clause oracle quota, four-oracle matrix or proving-commit chain.

# P26 — Use ordinary declarations appropriately

Ordinary Rust enums, schemas and dispatch are legitimate. Derive mutable facts and progress; avoid a static declaration pretending that runtime work completed.

# P27 — Every declaration must be causally load-bearing

Keep declarations that serve a real consumer; remove duplicate bookkeeping and dead policy structures.

# P28 — Compute product invalidation pragmatically

Use Git for code changes and conservative source invalidation for product updates. Do not build a generalized change-calculus metamodel before useful updates work.

# P29 — Choose the simplest suitable validation

Use DataFusion to query graph data. Small tooling checks, text searches and ordinary code assertions are appropriate for their own tasks; relational validation is not universal.

# P30 — Justify expected semantics independently

Justify expected answers independently of observed output. The same developer may write tests; do not require independent staffing or per-update expectation execution.

# P31 — Eliminate synchronization points that fail only by forgetting

Remove redundant synchronization. One editable plan and STATUS suffice for delivery; no plan activation, state JSON or digest tables.

# P32 — Validate by construction

Use types and constructors to make invalid states difficult to represent where this reduces code.

# P33 — Functional core, imperative shell

Keep calculation testable and isolate I/O and orchestration without imposing a rigid decomposition.

# P34 — One mutation path; commands idempotent and replayable

Own durable publication, reconcile duplicate/uncertain operations and recover cleanly. Compact runtime records support this; source-edit receipts do not.

# P35 — Inward, acyclic dependency structure

Keep dependency direction understandable and preserve the four justified build domains. Do not create packages just to organize concepts.

# P36 — Keep governance small and useful

Enforce a few valuable build, wire and structural boundaries with focused checks. Governance is subordinate to useful product delivery, not a parallel proof product.
