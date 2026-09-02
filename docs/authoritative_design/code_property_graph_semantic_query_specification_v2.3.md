---
artifact: authoritative-design
artifact_id: codefabric-composable-semantic-cpg-query
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: QRY
artifact_version: 2.3.0
authority_status: current
predecessor_path: docs/authoritative_design/code_property_graph_semantic_query_specification_v2.2.md
---

# Composable Semantic Query Specification v2.3

## 0. Authority, identity, and compatibility

The stable artifact ID is `codefabric-composable-semantic-cpg-query` (`QRY`). This document is
the current normative owner of CodeFabric's semantic request language, request composition,
resolution, deterministic response meaning, completeness/negative-proof behavior, limits, and
public semantic errors.

The v2.2 predecessor is immutable release history. V2.3 preserves the eight request-form names,
public ID grammar, request/result roles, orthogonal status dimensions, canonical error names,
one-logical-response behavior, and inline/resource semantic equivalence. The sole
production transport is `codefabric.cpgd.v2`; v1 request/response bytes remain
immutable allocation history and are never translated or selected by production.

The realization changes: request semantics, phrases, bindings, projections, policies, and
capabilities are explicit typed inputs and `ProgrammaticTransformation` values compiled by Rust
into authorized DataFusion plans. Their installed schemas and dependencies are observed from the
candidate session rather than replayed from a schema registry.
There is no static phrase registry, generated query bundle, form-to-executor crosswalk, stored
`PlanSpec` authority, SQL string, or adapter-side semantic interpreter.

V2.3 adds typed preparation and query-start semantics without changing any request-form meaning.
Ordinary execution has one atomic closed outcome: accepted work, a daemon-authored structured
input requirement, or validation rejection. The explicit validation operation remains pure data.
Presentation guards, sealed request state, and MCP completion are SRV mechanics; QRY owns the
semantic requirement, continuation constraints, acceptance meaning, and safe reference projection.

## 1. Scope and fact-only doctrine

The query language retrieves present-state objective facts and mechanically derived facts for
one authorized workspace and one immutable fabric epoch. It supports language-neutral, Python,
and Rust ontology profiles while preserving source, semantic, generated/lowered, executable,
value/memory, and explicit-unknown representations as distinct identities.

The service MUST reject requests for judgments such as `SAFE_TO_REFACTOR`, `TEST_IMPACTED`,
`HIGH_RISK`, `SHOULD_CHANGE`, recommendation, runtime behavior, coverage, Git-history meaning, or
environment inventory. When possible, it returns an objective fact-equivalent rewrite. It never
turns a missing provider row into proof of absence.

Public requests MUST NOT contain SQL, physical catalog/schema/table/column names, UDF names,
DataFrame/plan handles, serialized plans, provider objects, or object-store URLs. Controlled
semantic values live inside the structured forms below; physical realization remains private.

## 2. Canonical request envelope

### 2.1 Shape

The canonical transport representation is JSON. YAML may be used for human examples but is not
wire authority.

```yaml
specification: composable semantic CPG fact query
version: "2.0"
semantic_request_id: optional-client-correlation-id
scope:
  workspace_id: workspace:0123456789abcdef0123456789abcdef
  codebase: current authorized indexed workspace
  languages: [Rust, Python]
  source_boundaries: []
  analysis_contexts:
    mode: default | selected | all | source
    context_ids: []
  representations: []
  external_entities: endpoint-only
freshness:
  policy: require_current_for_targets
  target_scope: infer from query inputs
  deadline_ms: 120000
defaults: {}
queries:
  - query_id: example
    request: find code entities
    looking_for: the Rust function named `build_graph`
    return:
      include: [canonical semantic identity, qualified name, source location]
```

Required fields are `specification`, `version`, effective `scope.workspace_id`,
`freshness.policy`, and a nonempty `queries` list. Every query block requires a unique
`query_id`, one released `request` form, and its form-specific inputs.

A trusted transport may inject `workspace_id`; a supplied value MUST equal its credential
binding. `semantic_request_id` and daemon query ID are distinct. MCP request identity is
request-local correlation only; public MCP-call or RPC-attempt IDs do not enter the canonical
request, semantic identity, or result contract.
The transport preserves a valid semantic ID or generates and injects one before canonical
hashing; the response always echoes the effective value.

### 2.2 Scope and contexts

- `workspace_id` selects exactly one registered source instance; no repository-level default or
  cross-workspace body traversal is inferred.
- source boundaries only narrow the pre-authorized inventory.
- `default`, `selected`, `all`, and `source` context modes retain v1.3 meaning.
- context-sensitive entities/facts carry `analysis_context_id`; source-only facts use
  `context:source`.
- composition across incompatible contexts fails or executes context-wise with separate results.
- external declarations may be endpoints; their bodies are not traversed unless represented in
  the same epoch.

### 2.3 Freshness

The released policies retain their meanings:

```text
best_available_snapshot
await_latest
require_current_for_targets      default
require_source_current
require_semantic_current
```

Only explicit `best_available_snapshot` may return `POTENTIALLY_STALE`. Every current-required
policy returns a current epoch or a typed freshness/availability failure. The daemon applies the
freshness barrier before semantic resolution and pins one `Arc<FabricEpoch>` for the complete
request, result resources, and source-context reads.

### 2.4 Defaults

Unless overridden:

- entity ambiguity returns all candidates and matching evidence;
- phrase ambiguity fails only the affected block rather than guessing;
- exact, possible, heuristic, and unresolved evidence remain separate;
- explicit unknown entities/relations are included when relevant;
- absence is stated only with complete scoped coverage or an explicit negative fact;
- source occurrence, semantic entity, call site, executable instance, and lowered entity remain
  separate;
- ordering and canonical-ID deduplication are deterministic; and
- there is no implicit semantic result limit.

## 3. Common block, input, and return contracts

Every block supports `query_id`, `request`, optional `label`, `where`, `return`,
`on_ambiguity`, `on_unavailable`, and non-reinterpreting `extensions`.

Inputs may be:

```yaml
semantic_reference: the Rust function `crate::module::build_graph`
entity_id: entity:function:0123456789abcdef0123456789abcdef
fact_id: fact:call-target:0123456789abcdef0123456789abcdef
source_location:
  source_file: crates/cpg/src/query.rs
  start_line: 120
  start_column: 9
results_of: prior_query_id
select: the target entities of the returned call facts
```

Prior-result selection is typed by semantic role. A path-producing result cannot satisfy a
scalar input without an explicit valid selection. Type errors identify producer block, selected
role, expected role, and incompatibility.

The `return` object supports `include`, `exclude`, `result_shape`, `group_by`, `order_by`,
`deduplicate_by`, `supporting_facts`, `include_query_result`, and explicit `limit`. Exclusions
are reflected in coverage. Deduplication cannot collapse distinct required representations or
contexts. A reached limit reports scope, deterministic ordering, and incomplete status.

## 4. The eight request forms

These strings are released external symbols. Their implementation is compositional; they are
not eight public tools or eight hard-coded executors.

### 4.1 `find code entities`

```yaml
query_id: id
request: find code entities
looking_for: semantic description
within: []
where: []
return: {}
```

Finds source, syntax, semantic, generated/lowered, executable, control-flow, value/memory, and
unknown entities. `function` defaults to a semantic declaration; `function syntax` selects an
occurrence; `call` selects a call-site entity; `identifier` selects an occurrence; and `Rust
function instance` selects an executable specialization. The resolved interpretation is returned.

### 4.2 `retrieve facts about code`

```yaml
query_id: id
request: retrieve facts about code
about: []
facts: []
at: optional program point
where: []
return: {}
```

Retrieves relationships, properties, state, summaries, provenance, and explicit unknowns. A
broad phrase such as “all facts” is expanded into the actual requested/available families in
`resolved_semantics` and coverage.

### 4.3 `follow code relationships`

```yaml
query_id: id
request: follow code relationships
starting_from: []
relationship: semantic relationship
direction: optional
distance: one relationship step
stop_when: []
where: []
return: {}
```

Supports declarations/references, calls, members, implementations, control flow, definitions/
uses, reads/writes, alias/points-to, ownership, correspondence, and reachability. Distance is
explicit. Bounded relational reachability uses native joins or bounded `RecursiveQuery`; an
irreducible algorithm uses only the rung selected by `FAB`.

### 4.4 `find connecting fact paths`

```yaml
query_id: id
request: find connecting fact paths
from: []
to: []
using: semantic relationship families
path_policy: shortest | all shortest | simple with explicit bound
where: []
return: {}
```

Path records retain ordered entity/fact identity and evidence. Unrestricted all-path enumeration
through a cyclic graph is rejected with bounded alternatives.

### 4.5 `match a code fact pattern`

```yaml
query_id: id
request: match a code fact pattern
pattern: typed nodes, facts, bindings, alternatives, and scoped negation
where: []
return: {}
```

Bindings are typed. Alternatives retain which branch matched. A negated clause is legal only
when its fact-family/owner/context universe is complete; otherwise it is indeterminate.

### 4.6 `combine result sets`

```yaml
query_id: id
request: combine result sets
inputs: []
operation: union | intersection | difference | join | context-wise comparison
where: []
return: {}
```

Set operations use canonical identity and compatible semantic roles. They cannot silently merge
workspaces, contexts, representation layers, or certainty classes.

### 4.7 `summarize objective facts`

```yaml
query_id: id
request: summarize objective facts
about: []
measure: count, set, distribution, or deterministic mechanically derived value
group_by: []
return: {}
```

Summaries expose input-set, grouping, aggregation, producer, precision, completeness, and support
provenance. They do not turn measurements into risk, quality, impact, or recommendation labels.

### 4.8 `retrieve source and syntax context`

```yaml
query_id: id
request: retrieve source and syntax context
about: []
context: exact source span, surrounding lines, syntax outline, or related occurrence
return: {}
```

Source authorization is checked independently of fact authorization. Text is returned only when
losslessly decoded; otherwise a byte/base64 variant is used. Truncation reports exact omitted
bytes and never claims complete context.

## 5. Composition DAG and execution semantics

Rust decodes a request into typed request-block, dependency-edge, binding, selection, return,
limit, and scope relations. References imply a dependency DAG:

- independent branches may run concurrently after one epoch pin;
- dependent blocks wait for typed inputs;
- fan-out reuses one canonical prior result;
- fan-in is deterministically ordered;
- cycles are `QUERY_DEPENDENCY_CYCLE`; and
- a failed dependency yields `NOT_EXECUTED_DEPENDENCY` without invalidating independent blocks.

The semantic compiler joins request relations to typed query-form, phrase, binding, projection,
policy, producer-closure, and live capability inputs. It constructs native typed
DataFusion plans and validates logical/physical/output schemas under `FAB`'s `SchemaContract`.
Analyzer policy walks subqueries and all plan/expression variants. Optimizers may improve a plan
but cannot establish authorization, boundedness, or correctness absent from the input plan.

Equivalent semantic requests yield equivalent logical results despite provider batch order or
optimizer plan-shape changes. `EXPLAIN` and serialized plans are diagnostics/caches only.

## 6. Resolution, authorization, and bound authority

Phrase resolution is deterministic against the pinned epoch's typed query program and facts. Code identifiers,
ordinary words, paths, synonyms, representations, and relationship meanings are disambiguated
with returned candidate interpretations. The compiler never applies a syntax/name/stale fallback
for unavailable semantic meaning.

Before planning, policy derives an `AccessScopeId` and reduced child catalog. Hidden schemas,
tables, columns, providers, functions, extensions, variables, object stores, metadata, and
information-schema names are absent. Public views are rebuilt inside the child or recursively
verified for all bound dependencies as required by `FAB`. Authorization is applied before match,
cost, statistics, negative proof, source context, error construction, and artifact creation.

A complete result may be complete within the explicitly authorized universe. It MUST NOT be
worded as complete for the workspace when relevant partitions were denied.

## 7. Evidence, unknowns, absence, and provenance

Evidence dimensions remain orthogonal:

```text
certainty: source/syntax, static semantic, compiler/lowered, possible, heuristic, unresolved
resolution: exact, sound possible set, heuristic candidate, unresolved, unavailable
directness: direct, transitive, summary, materialized derivation
completeness: COMPLETE, PARTIAL, INDETERMINATE, UNAVAILABLE, NOT_APPLICABLE
```

Empty results distinguish:

1. proven empty under complete scoped coverage;
2. no match after filters;
3. unresolved inputs;
4. unavailable fact family;
5. partial provider/owner failure; and
6. explicit or hard limit.

Negative properties or pattern clauses require an explicit negative fact or complete relevant
owner/context coverage. `No may-alias edge` is not `proven not to alias`.

Every fact identifies producer/release, owner, source where applicable, certainty, resolution,
directness, and direct provenance. Derived facts additionally identify algorithm, precision,
input epoch/projection, completeness, support facts/witness paths, and provenance edges. Closure
is computed to source images, explicit typed-input and transformation decisions, provider runs,
table versions, the application/provider release vector, and independent expectations; it is not
a maintained boolean.

## 8. Canonical identity and ordering

Released public ID forms remain:

```text
workspace:<32-lowercase-hex>
context:<32-lowercase-hex>
context:source
context-set:<32-lowercase-hex>
snapshot:<32-lowercase-hex>
entity:<kind-slug>:<32-lowercase-hex>
fact:<kind-slug>:<32-lowercase-hex>
```

An additive v2 profile may expose `fabric-epoch:<32-lowercase-hex>` while retaining `snapshot_id`
as the stable public query pin. Source identity includes workspace, source file, digest, byte-safe
path, and half-open byte range. Display names, statements, line/column, provider IDs, or plan IDs
are never identity.

Default ordering is stable: source path/byte position, semantic kind, qualified name, canonical
ID for entities; owner/source/fact class/predicate/subject/object/fact ID for facts; length and
ordered fact sequence for paths; canonical key for groups. Request order determines
`query_results` order even when branches execute concurrently.

## 9. Limits, cancellation, and one logical response

There is no hidden semantic limit. Explicit limits return deterministic prefixes and
`EXPLICIT_LIMIT_REACHED`. A hard service budget rejects before or terminates execution with
`QUERY_HARD_LIMIT_EXCEEDED`; it is not reported as a complete partial list. Unbounded path,
pattern, fan-out, or result-reference amplification is rejected with a bounded rewrite.

All blocks share one epoch and one logical response. Transport may chunk or externalize it but
must preserve semantic rows, ordering, final coverage, checksum, and terminal status. Cancellation
and deadlines reach freshness waiting, DataFusion tasks, providers, graph work, materialization,
and resources. Ending an observation stream does not cancel accepted work; explicit daemon
cancellation changes the accepted query state. No computation may continue after that query reaches
a completed or cancelled terminal state.

## 10. Canonical response

### 10.1 Envelope

The canonical response preserves these top-level fields:

```yaml
specification: composable semantic CPG fact query response
version: negotiated-version
semantic_request_id: effective-id
execution_state: COMPLETE
availability_state: AVAILABLE
completeness_state: COMPLETE
freshness_state: CURRENT
limit_state: NOT_APPLIED
successful_query_count: 1
failed_query_count: 0
not_executed_dependency_count: 0
snapshot: {}
entities: {}
facts: {}
paths: {}
groups: {}
source_contexts: {}
query_results: []
errors: []
```

Canonical records appear once in deduplicated dictionaries and query results refer to IDs.
Inline previews are presentation only. Response validation checks dictionary referential
integrity, schemas, ordering, coverage, and checksum.

### 10.2 Public snapshot projection

The stable `snapshot` record remains the public projection of the pinned `FabricEpoch` and
contains:

```text
snapshot_id, workspace_id, optional repository_id/worktree_id,
source_generation, source_inventory_digest,
durable_base_publication, base_table_version_digest,
overlay_generation, overlay_checksum,
analysis_context_set_id, analysis_context_ids,
freshness_state, source_trust_state, event_stream_health,
git_acceleration_status, optional git_operation_summary,
pending_update_count, ontology/schema/provider/derivation/query versions,
capability_summaries, diagnostic_references
```

The negotiated v2 profile additionally exposes `fabric_epoch_id`, application/provider release
vector, programmatic assembly identity,
proof receipt, policy identity, and exact epoch compatibility class. Additions do not change the
v1.3 meanings above.

### 10.3 Record families

- Entity records require ID, semantic kind, and representation; requested names, owners,
  locations, and properties are additive.
- Fact records require ID/form/kind/class, workspace/context/owner, statement, certainty,
  resolution, directness, and provenance; relations and properties use typed fields.
- Path records contain ordered entity and fact IDs, length, policy, and certainty summary.
- Group records contain canonical group keys, objective values, and optional supporting IDs.
- Source-context records contain exact source reference, context kind, and a text-or-bytes union.
- Query-result records contain query/request plus execution, availability, completeness,
  freshness, limit, and dependency states; resolved semantics; typed result IDs/bindings;
  coverage; errors; and notices.

The orthogonal state enums retain the released vocabulary:

```text
execution: ACCEPTED | RUNNING | COMPLETE | FAILED | CANCELLED | DEADLINE_EXCEEDED |
           NOT_EXECUTED_DEPENDENCY
availability: AVAILABLE | PARTIAL | UNAVAILABLE | NOT_APPLICABLE
completeness: COMPLETE | PARTIAL | INDETERMINATE | UNAVAILABLE | NOT_APPLICABLE
freshness: CURRENT | POTENTIALLY_STALE | UNAVAILABLE
limit: NOT_APPLIED | EXPLICIT_LIMIT_REACHED | HARD_LIMIT_REJECTED
dependency: READY | FAILED_DEPENDENCY | NOT_APPLICABLE
```

## 11. Public semantic errors

A canonical query error contains `code`, `layer`, `retryable`, `safe_message`, optional
field/path/phrase, candidate interpretations, failed dependency query ID, and diagnostic ID.
Provider, DataFusion, filesystem, and internal plan messages are diagnostic evidence and are
never forwarded as public codes or unrestricted text.

The following released names retain their meaning and are append-only wire symbols:

```text
INVALID_REQUEST_SCHEMA
UNKNOWN_REQUEST_FORM
SEMANTIC_PHRASE_AMBIGUOUS
UNRESOLVED_INPUT_REFERENCE
INCOMPATIBLE_RESULT_REFERENCE
QUERY_DEPENDENCY_CYCLE
UNSUPPORTED_FACT_FAMILY
NOT_OBJECTIVE_FACT_REQUEST
NEGATIVE_PROOF_INDETERMINATE
WORKSPACE_NOT_FOUND
WORKSPACE_NOT_AUTHORIZED
WORKSPACE_BOOTSTRAPPING
CONTEXT_NOT_INDEXED
COMPOSITE_SNAPSHOT_UNSUPPORTED
CURRENT_FACTS_UNAVAILABLE
FRESHNESS_DEADLINE_EXCEEDED
UNBOUNDED_QUERY
QUERY_HARD_LIMIT_EXCEEDED
RESULT_EXPIRED
CANCELLED
CONTRACT_MISMATCH
INTERNAL_INVARIANT_VIOLATION
```

These stable declarations exist because external compatibility is their semantics. They do not
form a mutable runtime error census. One released wire-contract source owns them; generated
language bindings are derivable and regenerate/compare checked. New synonymous names are
forbidden.

## 12. Dynamic reference and capability behavior

Reference, schema, phrase, form, producer, capability, and proof views are filtered projections
of the pinned epoch. Changing accepted typed-input, transformation, or provider/proof rows changes
these surfaces after activation without editing a package bundle. A form is advertised only when
its required program bindings, producer closure, functions/extensions, policies, and executable
proof are present.

Validation may return normalized request relations, dependency graph, resolved semantics,
capability requirements, cost class, errors, and warnings without executing retrieval. It does
not publish a durable compiled plan or expose physical names.

## 13. Preparation, input requirements, atomic start, and live references

### 13.1 Pure preparation

`ValidateQuery` computes a typed `QueryPreparation` from the same released decoder, fact-only
policy, phrase resolver, capability/producer closure, authorization scope, bounds, and cost model
used by query start. Its output contains normalized request relations, dependency graph, resolved
semantics, input requirements, capability requirements, bounded resource estimate, errors, and
warnings. It creates no challenge record, acceptance, capacity reservation, task, query, package,
resource handle, lease, or mutable FastMCP state. Preparation data is advisory until the atomic
start operation revalidates it against one current authorized epoch.

### 13.2 Closed start outcome

Ordinary execution invokes one daemon operation with a closed result:

```text
StartQueryOutcome =
    Accepted(AcceptedQuery)
  | InputRequired(InputChallenge)
  | Rejected(ValidationFailure)
```

An absent or unknown variant fails closed. Start authenticates, normalizes, validates, applies the
freshness barrier, checks capacity and idempotency, and either creates one accepted query or creates
no accepted work. It never relies on a prior preparation result. The same start idempotency identity
and normalized immutable fields return the original outcome; changed content is a typed conflict.

`AcceptedQuery` identifies daemon-owned work and its observation/cancellation route.
`ValidationFailure` is public-safe typed rejection. `InputChallenge` means the request is not yet
accepted and cannot consume query execution, result, or resource capacity.

### 13.3 Typed input challenge and continuation

An input requirement contains a bounded set of stable semantic field IDs, allowlisted scalar/enum/
collection input kinds, strict constraints, safe presentation keys, optional authorized choices,
public explanation code, challenge/round identity, expiry, remaining-round bound, and an opaque
daemon continuation. Opaque JSON, prose-parsed constraints, adapter-authored defaults, and inferred
answers are forbidden.

The continuation binds principal, workspace, semantic request identity and normalized immutable
fields, session/daemon generation, challenge/round, issue/expiry, policy/revocation generation,
allowed answer shape, and original start idempotency identity. Every answer leg is reauthenticated,
reauthorized, shape-validated, and replay-checked. A valid continuation may accept exactly once;
tamper, expiry, replay, changed arguments, excess rounds, or cross-principal/workspace/generation
use rejects without creating work.

FastMCP sealing protects presentation continuation integrity but is not semantic authority. The
daemon token and validation above remain decisive. Adapter restart may invalidate unfinished
presentation state because no accepted query exists yet.

### 13.4 Authorized completion and resource projection

Reference completion is a bounded projection of the live authorized reference relation. Its typed
input is an approved reference-template variable, prefix/selector, and cap. It returns only safe
authorized candidate values plus bounded `total` and `has_more`; it never reveals denied existence,
result handles, repository paths, source/entity inventories, hidden capabilities, or principal
data. Completion is advisory and the eventual reference operation reauthorizes independently.

Result/reference delivery exposes a daemon-minted public resource handle only after authoritative
package/resource/lease state exists. The public handle is not an internal lease token, path, or
semantic identity. Read and release reauthorize principal/workspace, policy/revocation and daemon
generation, expiry, selectors/ranges, and release state on every request. Restart invalidates stale
generation handles; policy may mint a replacement only from an authorized retained package.

### 13.5 Cancellation and observation recovery

Cancellation is an explicit idempotent daemon operation over accepted query identity. It reserves
control capacity, returns a typed acknowledgement, and leads to an observed terminal state under
the query's policy. Cancelling or losing a watch affects observation only. Reconnect reauthenticates
and resumes by daemon query identity/cursor; it never resubmits start. Presentation request IDs are
correlation observations and do not become semantic, idempotency, query, challenge, epoch, package,
resource, session, or lease identity.

## 14. Executable acceptance obligations

| Contract | Required executable oracle |
|---|---|
| all eight forms and role composition | `just semantic-query-relational-conformance-check` |
| released request/response compatibility | `just semantic-query-conformance-check`; `just proto-contract-check` |
| DAG, cycles, fan-in/out, typed references | `just query-composition-dag-check` |
| deterministic result semantics | `just query-determinism-check` |
| no SQL/physical/plan public surface | `just public-query-port-check` |
| typed-input/transformation-to-plan causality | `just programmatic-schema-causality-check` |
| native plan and schema conformance | `just semantic-plan-conformance-check`; `just relational-schema-lifecycle-check` |
| authorization and bound closure | `just access-catalog-isolation-check`; `just authorized-view-bound-authority-check` |
| unknown/negative/coverage semantics | `just independent-semantic-oracle-check` |
| dynamic references and capability | `just dynamic-reference-delivery-check` |
| four-layer public equivalence | `just semantic-delivery-vertical-check` |
| pure preparation and closed atomic start | `just fastmcp4-atomic-start-check` |
| typed guarded continuation and replay rejection | `just fastmcp4-guard-roundtrip-check` |
| daemon public handles and per-read authority | `just fastmcp4-resource-authority-check` |
| authorized bounded completion | `just fastmcp4-completion-authorization-check` |
| explicit cancellation and resume without resubmit | `just fastmcp4-cancellation-recovery-check` |

Acceptance fixtures are independently authored and decoded. Production program/application output,
stored plan text, count/digest agreement, or the predecessor engine alone cannot author expected
semantics. Every required oracle MUST pass at the proving revision and retained public profiles
MUST reject incompatible versions before query acceptance.
