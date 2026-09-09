# CodeFabric agent instructions

Read [STATUS.md](STATUS.md) for current work, known failures, and the selected plan.
The [suite governance](docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md)
selects target documents; [spec_index](docs/spec_index/README.md) is navigation.
The pragmatic target preserves the full Python/Rust CPG. Preparation is not product completion.

## Product and build boundaries

- One Rust workspace daemon owns source/context state, providers, graph publication,
  query execution, and processing status. Supervisor/launcher own process lifecycle.
  FastMCP is presentation only.
- Arrow carries facts, DataFusion performs relational work, and Delta stores versioned
  relations. Keep coherent exact snapshot selection and ordinary operation ownership.
- Keep four build domains: stable root; `rustc-extractor/` on its dated nightly;
  `pyrefly-sidecar/` with its pinned integration; `codefabric-cpg-mcp/` with its own uv environment.
  Do not create a crate solely to organize code or put Python in the data plane.
- Preserve application-owned identity, raw/normalized kinds, syntax/entity distinctions,
  call sites, provider isolation, effective analysis context, and conflict provenance.
- Emit facts and mechanically derived facts, not risk/refactoring judgments. Git history,
  execution coverage, and environment inventory are not graph domains.
- Missing facts are unknown/incomplete until coverage supports absence. Exclude invalidated
  semantics from current results; identify historical results and pending scope honestly.
- The revised resource target uses shared budgets, bounded application work/state,
  owned lifetimes, containment, headroom, backpressure, and recovery. It does not promise
  every native allocation is pre-admitted or that OOM is impossible. See STATUS for the
  implemented Linux resource controls and remaining deployment/provider work.

## Workstation resource policy

The primary development workstation has 16 physical cores, 32 threads and 192 GB RAM.
Allow substantial resource use when it serves full Python/Rust CPG construction and querying.
Large consumption alone is not a defect. Measure actual workload cost before reducing
capability or adding machinery solely to fit an unnecessarily small budget.

Use broad shared operating budgets, useful CPU parallelism, actual system headroom and owned
cleanup. Distinguish physical memory from virtual mappings and reserved thread stacks. Keep
protocol/frame limits, cancellation and finite retention where they address a concrete failure;
scaling a complete source inventory may require batching or scheduling design rather than
merely raising every message bound. Memory capacity does not imply free disk space.

## Working loop

Choose a useful outcome, inspect the immediate path, implement and integrate a small change,
run relevant checks, exercise behavior, and update the handoff. Honor user scope/exclusions.
Use one canonical working tree and small coherent commits; preserve pre-existing changes and
attribute concurrent work. Avoid long-lived independent interface designs.

Plans and decisions are ordinary Git-tracked Markdown edited in place. Keep one current
backlog linked from `STATUS.md`; no activation transaction, state JSON, proving-commit chain,
digest table, or fixed test quota is required. Design/review resolves consequential uncertainty
or an explicit request; it is not a compulsory cycle.

Runtime actions leave compact outcomes and input/output references. Source edits, internal
function calls, and allocations need no individual artifacts. Detailed intermediate capture
belongs in diagnostics. Keep compatibility/reproduction information where it has a consumer.

## Commands and validation

Start with `just --list`. Recipes work in fresh non-interactive shells without direnv,
sourcing scripts, or manual Python activation.

| Change | Useful checks |
|---|---|
| Docs/skills | relevant spelling, links, navigation |
| Tooling/configuration | lint, focused tests, shell/config and affected consumer checks |
| Rust logic | affected check/Clippy and relevant tests |
| Adapter/wire | adapter tests/types/lint and cross-language protocol checks |
| Dependency/feature | resolved source/type compatibility, affected builds and real consumers |
| Product behavior | independent expected facts and real provider-to-query scenarios |
| Invalidation/recovery | clean/incremental, stale-completion, cancellation/restart cases |
| Performance | representative before/after samples and relevant correctness checks |

Use affected checks while editing and integrated checks when the claim warrants them.
Do not run full CI before every edit. Known failures stay visible; empty selectors fail.
`nextest` does not run doctests. Report command, revision/date, relevant configuration,
result, and limits. Reuse attributable evidence when relevant inputs have not changed.

Automation should justify its consequence, recurrence, and maintenance cost. Types and short
algorithm arguments are useful; two equal runs or one corpus do not prove universal correctness.
Choose fuzzing, mutation, and fault injection by risk rather than for every clause.

Mutating commands are intentional, with reviewed diffs, and are not gate dependencies.
Routine crate changes need no licensing or bespoke source-artifact approval. Investigate actual
downgrades/conflicts, preserve selected source identities, and reconcile locks normally.

## Environment and navigation

- Keep exact toolchains/dependencies and the existing cache setup. `.cargo/config.toml`
  uses supervised sccache. Check/incremental recipes select their mode. Do not routinely
  `cargo clean` or reconfigure a healthy service.
- Stable root/sidecar share `target/`; nightly/extractor work uses isolated targets.
  Bacon owns continuous Cargo checking. Do not duplicate it in editor check-on-save.
- Python tooling uses the adapter dev environment; there is no root uv project.
- Keep one Rust integration target, `tests/integration.rs`; add cases below it.
- Use `rg`/`rg --files` for bounded searches. `.claude/` is hidden. Avoid `rg -uu` over
  `.envrc.local`, which may contain a capability token. Never print secrets.
- Use `just spec-outline` for domain specs and `just lib-outline` for library references.
  Cite section/title and consult exact local library sources before assuming APIs.
- Manifests/locks identify current source selection. Target pins do not imply that local
  native patches have been removed.

## Skills and references

`product-delivery` owns routine planning/execution/handoff. `implementation-review`,
`design-development`, and `library-capability-research` are concise opt-in tools.
`skill-eval` is only for explicit workflow evaluation. Library navigators route API questions
without imposing additional doctrine/artifact workflows.

`.claude/skills` is the single source exposed through `.codex/skills` and `.agents/skills`.
`CLAUDE.md` imports this file. User instructions govern skill scope.

Consult [infrastructure rationale](docs/repository_infrastructure_reference.md), the
[repository specification](docs/rust_core_python_interface_repository_specification_2026-08-20.md),
and relevant library references on demand. Historical plans/reviews explain prior work;
they do not select live gates or authorize resuming retired workflows.
