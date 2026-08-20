# SmartRef Code-Intelligence Routing

Use this reference for repository research. Keep `SKILL.md` files concise and
consult the live catalogs and the project reference documents for detailed
schemas.

Canonical project references:

- `docs/library_ref/mcp_code_intel_usage.md`
- `docs/library_ref/mcp_code_intel_for_skills.md`

## Mandatory tripwires

| Before you... | Required structural inquiry |
|---|---|
| Change a function or method signature | One-hop neighborhood, then transitive callers across the plausible consumer universe |
| Add or change a Protocol/trait method | All implementations/adapters, then each implementation's current interface |
| Change a base-class contract | All subclasses and construction sites |
| Add or remove a contract field | Constructors, structuring/deserialization, factories, fixtures, and direct consumers |
| Delete a symbol/module/rule | Callers/references plus structural or rule impact and final zero-state proof |
| Claim a repository-wide invariant | Saved/inline rule or equivalent complete structural query |
| Use an existing API in a design exemplar | Definition/type lookup at the pinned current version |

## Tool selection

Use `smartref-code-intel` by default for relational, semantic, inventory,
cross-language, and change-proof questions. Use `Read`, `Glob`, `Grep`, and
`Ripgrep` for:

- the exact file about to be edited;
- small configuration or markdown files;
- simple literal searches;
- fallback when the code-intelligence coverage envelope is incomplete;
- analysis of the code-intelligence implementation while it is being modified.

Do not rely on the tool under modification to certify its own behavior.

## Live discovery over copied catalogs

The tool catalog changes. Before invoking an unfamiliar `core/*` plan or
primitive, consult the live plan/primitive catalog or completion surface.
Never invoke a plan solely because an older skill listed it. In particular,
do not assume a service-graph enumeration plan exists unless the live catalog
confirms it.

Batch independent queries that share scope. Narrow the selected consumer
universe without narrowing it so far that external callers disappear.

## Result trust

- Inspect `coverage`, unresolved files, failed files, parser diagnostics, and
  graph evidence before making negative or global claims.
- For Rust cold-start or semantic-index failures, prefer tree-sitter
  inventories and current-tree source reads rather than repeatedly spinning.
- Retry a deferred/cold query once when appropriate, then use the best
  available fallback and record the limitation.
- Exact-file source reads may remain available even when graph completeness is
  partial; do not confuse source availability with graph completeness.

## Planning and execution usage

### Design and planning

Use structural queries to establish:

- current boundaries, ownership, and service/pass/contract inventories;
- consumer and implementation surfaces;
- duplicate or parallel implementations;
- legacy-pattern extent;
- likely test and governance surfaces;
- whether proposed new abstractions already exist.

Do not enumerate the whole repository when a bounded subsystem answers the
question.

### Execution

Re-run impact probes immediately before changing a load-bearing interface.
Plans are evidence-backed hypotheses, not a license to skip current-tree
verification.

### Review and decommission

Use both positive and negative proof:

- positive: the target symbol, contract, behavior, or route exists and is used;
- negative: the old symbol, route, registration, alias, rule match, and
  authority have reached zero within complete coverage.
