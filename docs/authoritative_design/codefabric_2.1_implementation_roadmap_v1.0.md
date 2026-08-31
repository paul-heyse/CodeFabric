---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-roadmap
suite_id: codefabric-relational-data-fabric
suite_version: 2.1.0
artifact_tag: RM
artifact_version: 1.0.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_2.0_implementation_roadmap_v1.0.md
---

# CodeFabric 2.1 relational data-fabric implementation roadmap

## 0. Authority and boundary

This roadmap has stable artifact identity `codefabric-relational-data-fabric-roadmap`. It is
subordinate to SUITE, ONT, GEN, FAB, QRY, LIFE, and SRV. It orders capability and decommission
work but cannot weaken, reinterpret, or certify their contracts. The approved versioned
implementation plan owns exact packet dependencies, acceptance checks, proving commits, and
decommission batches.

V2.1 preserves the v2.0 product scope and every v2 implementation outcome not invalidated by the
independent review. Its architectural correction is narrow and decisive: bootstrap/migration
replay is replaced by exact provider batches, explicit typed inputs, typed programmatic
transformations, and observations derived from the installed DataFusion session. This is not a
license to discard unrelated behavior or an excuse to retain the invalid authority behind a
compatibility layer.

## 1. Sequencing invariants

1. V2.1 is the sole current suite; v2.0 and v1.3 are immutable predecessor history.
2. The complete authority, outcome, and L-20--L-55 disposition ledgers are fixed before code
   changes resume.
3. The real daemon route consumes the target composition before predecessor composition is
   removed; target and predecessor never own production mutation simultaneously.
4. Bootstrap/model/dual-epoch consumers cut over early and their obsolete files, symbols,
   features, recipes, fixtures, and package edges are removed at the same dependency-safe boundary.
5. DataFusion child-session/cache/schema closure and Delta exact-history/recovery closure precede
   provider, derived-analysis, query, and lifecycle release acceptance.
6. Exact provider batches and explicit typed inputs precede transformations that consume them;
   plan-derived schemas and fixed-point observations are the sole catalog/schema authority.
7. Every proof-bearing intermediate required for restart, audit, incrementality, or provenance is
   persisted at an exact Delta root/version; transient execution buffers remain Arrow.
8. Independently authored expected rows and causal faults are accepted after the production
   vertical exists but before release cutover, so they exercise real composition without deriving
   expectations from it.
9. Every surface receives a targeted retain/reshape/replace/delete reason, replacement/cutover,
   positive oracle, and negative oracle. Lack of connection to v2.1 is not a deletion reason.
10. No stage introduces a production dual write, bootstrap fallback, model replay fallback, legacy
    query fallback, or silent compatibility route.
11. Final release requires all packet oracles, physical legacy zero state, and independent release
    evidence at one trusted HEAD.

## 2. Capability stages

### 2.1 Stage 0 -- Authority and scope closure (`WP28`)

Establish sole-target v2.1 routing, enumerate every v2 outcome and L-20--L-55 surface, classify
authority and durability, and bind each changed disposition to a target consumer and a positive and
negative proof. Exit capability: no implementation work can silently lose prior scope or retain the
rejected authority.

### 2.2 Stage 1 -- Production programmatic composition (`WP29`)

Compose the real daemon factory from exact batches, explicit typed inputs,
`ProgrammaticTransformation`, one governed runtime, one command manager, one activation authority,
and one query backend. Exit capability: cold production construction uses the target route rather
than `bootstrap_fabrics` or a default empty backend.

### 2.3 Stage 2 -- Early predecessor-authority cutover (`WP30`)

Cut all production consumers from bootstrap workspace creation, relational-model replay/release/
schema authority, generated schema registries, model migration, old epoch builders and dual epoch
pins, replay compiler wrappers, legacy provider admission, and model-pinned query entry points.
Extract only proven authority-neutral primitives already consumed by the target, then delete the
owning predecessor surfaces. Exit capability: the rejected authority is impossible to select.

### 2.4 Stage 3 -- DataFusion and Delta closure (`WP31`--`WP32`)

`WP31` completes plan-derived `SchemaContract` validation, native-first transformation lowering,
fixed-point programmatic observations, authorized child catalog closure, and bounded fully keyed
DataFusion caches. No physical-plan or result cache exists.

`WP32` completes the durability ledger, exact Delta root/version selection, CDF transport,
full-statistics serving, one-writer transaction/provenance protocol, exact-vector activation,
candidate-free recovery, and non-authoritative receipt reconciliation.

Exit capability: every admitted epoch is self-describing and reconstructible from exact durable
history without bootstrap, replay, `latest`, raw Parquet listing, or cache authority.

### 2.5 Stage 4 -- Provider, analysis, query, and lifecycle verticals (`WP33`--`WP36`)

`WP33` completes exact Tree-sitter/Ruff/Pyrefly/rustc provider-native Arrow batches,
relation-scoped IPC, coverage/remainder/unknown trailers, trust launch, and exclusive admission.

`WP34` completes Python, Rust MIR-derived, common graph, effect/resource, and interprocedural
application producers; every accepted family has exactly one producer or an explicit unsupported
remainder.

`WP35` compiles all eight semantic request forms and bounded graph operations through authorized
child catalogs, retaining public forms while denying SQL, physical names, and internal handles.

`WP36` connects repository lifecycle, source truth, command/activation/query managers, resource
governance, immutable Arrow delivery, UDS gRPC, and the presentation-only FastMCP adapter.

Exit capability: real source/provider/Delta inputs flow through one admitted programmatic epoch to
all released public behaviors.

### 2.6 Stage 5 -- First-principles evidence (`WP37`)

Replace `bootstrap_model_semantics`, replay agreement, and the mandatory predecessor comparator
with independently authored typed rows, negative cases, released-wire expectations, and causal
faults. Re-execute semantic, security, resource, cache, recovery, clean-build, incremental, Delta,
and public-compatibility claims against the production vertical. Exit capability: evidence can
falsify the implementation without borrowing its answer.

### 2.7 Stage 6 -- Remaining purge, release proof, cutover, and certification (`WP38`--`WP41`)

`WP38` removes the remaining cross-cutting generated authorities, obsolete governance selectors,
features, recipes, rules, fixtures, dependencies, package data, and predecessor evidence machinery
whose last consumers disappeared in earlier stages. Released wire/identity contracts and immutable
history remain.

`WP39` re-executes the complete semantic, security, resource, recovery, clean/incremental, Delta,
provenance, and public-compatibility matrix against the post-purge production tree.

`WP40` performs the fenced forward-only production cutover. Target mutation cannot begin until the
exact predecessor binary is mechanically denied binding, serving, and writing across restart and
reboot; rollback after target mutation is forward repair only.

`WP41` proves complete disposition coverage, exact reconstruction, legacy zero state, all four
build domains, feature graphs, governance, and final release gates at one trusted HEAD.

## 3. Dependency spine and permitted parallelism

```text
WP28 -> WP29 -> WP30 -> {WP31, WP32}
WP31 -> WP33 -> WP34 -> WP35
WP29 + WP30 + WP31 + WP32 + WP33 + WP34 + WP35 -> WP36
WP36 -> WP37 -> WP38 -> WP39 -> WP40 -> WP41
```

`WP31` and `WP32` may proceed in parallel after the production authority seam is singular.
Provider implementation in `WP33` may inventory reusable exact adapters during that work but cannot
close until DataFusion closure passes. File disjointness alone is not dependency closure.

## 4. Milestone exits

- `M01` -- Authority, prior-outcome, and L-20--L-55 scope ledgers are complete.
- `M02` -- Production composition is programmatic and predecessor authority is physically absent.
- `M03` -- DataFusion/catalog/cache and Delta/durability/recovery closure are proved.
- `M04` -- Provider, analysis, query, lifecycle, daemon, and FastMCP production vertical is proved.
- `M05` -- First-principles release evidence and remaining targeted legacy purge are accepted.
- `M06` -- Fenced cutover and final certification pass at one trusted HEAD.

## 5. Decommission discipline

Every disposition names: current surface, retained outcome, decision (`retain`, `reshape`,
`replace`, or `delete`), reason, target consumer, cutover packet, deletion packet, positive oracle,
and negative zero-state oracle. Prior v2 deletion targets remain deletion targets unless the v3
design identifies a genuinely authority-neutral primitive already consumed by the target.
Additional surfaces become legacy only where programmatic authority makes their meaning duplicative
or their route bypasses the target.

Deletion follows consumer-to-authority order and occurs at the earliest dependency-safe boundary.
Historical design, plan, review, release, and tombstone evidence is never deleted merely because
runtime authority is removed.

## 6. Evidence discipline

Each packet discovers current-tree impact before edits, implements immediate consumers and attached
decommission coherently, runs exactly four unique named packet oracles including demonstrated
negative fixtures, and receives a proving commit before completion. Checks prove behavior; state
labels, digests, captures, file presence, and self-generated comparators do not. Negative claims
combine construction/compiler proof, relational oracles where representable, structural search,
hidden-aware textual search, and skipped-file accounting.

## 7. Roadmap completion criterion

The roadmap is complete only when `WP28`--`WP41`, `M01`--`M06`, all attached decommission batches,
and the final gate matrix are proved at one trusted HEAD; the current runtime reconstructs from
exact batches, explicit inputs, typed transformations, and exact Delta versions; every prior scope
outcome is either preserved or explicitly superseded; and obsolete functionality is physically
absent outside immutable history.
