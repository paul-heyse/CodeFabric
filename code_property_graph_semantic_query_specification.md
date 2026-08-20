# Composable Semantic Query Specification for Present-State Code Property Graph Facts

**Status:** Draft normative specification  
**Specification version:** 1.0  
**Primary consumers:** LLM programming agents and code-intelligence services  
**Supported ontology profiles:** Language-neutral core, Python, and Rust  
**Companion ontology:** `code_property_graph_present_state_fact_ontology_specification.md`  
**Companion schemas:**

- `cpg_semantic_query_request.schema.json`
- `cpg_semantic_query_response.schema.json`

---

## 1. Purpose

This document specifies a structured, semantic-first query interface for retrieving objective present-state facts from the Code Property Graph defined by the companion ontology.

The interface is designed so that an LLM programming agent can ask for rich codebase facts without knowing:

- graph storage layout;
- table or column names;
- canonical edge labels;
- parser, compiler, or type-checker identifiers;
- graph traversal syntax;
- query-planner syntax;
- regular expressions;
- bytecode or intermediate-representation encodings;
- implementation-specific join or indexing strategies.

The agent supplies **plain-language semantic values inside a small set of structured request forms**. The query service resolves those values against the ontology, executes the required graph operations internally, and returns all requested result sets in one logical response.

The governing objective is:

> **Make the entire present-state CPG fact substrate directly and compositionally queryable by an LLM programming agent, while preserving semantic precision, uncertainty, provenance, and representation boundaries.**

---

## 2. Scope

### 2.1 Included query outputs

The query interface SHALL retrieve only:

1. source facts;
2. semantic facts;
3. compiler or lowered facts;
4. mechanically derived graph facts;
5. deterministic summaries of fact sets;
6. explicit unknown or unresolved facts;
7. exact source and syntax context supporting those facts;
8. metadata needed to interpret fact certainty, ownership, provenance, and completeness.

The interface SHALL cover all ontology domains, including:

- files, spans, tokens, comments, documentation, and directives;
- syntax occurrences and syntax structure;
- semantic declarations, bindings, references, and scopes;
- modules, imports, exports, and code-declared dependencies;
- declared, inferred, expected, narrowed, and computed types;
- members, inheritance, traits, protocols, implementations, and overrides;
- call sites, receivers, arguments, parameter bindings, dispatch, and targets;
- control flow, exceptional flow, dominance, post-dominance, and loops;
- values, definitions, uses, reaching definitions, and value flow;
- abstract memory, reads, writes, access paths, aliasing, and points-to facts;
- program-point initialization and state facts;
- effects, exceptions, panic, unwind, resource lifetime, and cleanup;
- async, generators, tasks, threads, channels, locks, and captures;
- generated and lowered code;
- generic specializations and Rust monomorphized instances;
- Python-specific semantics;
- Rust MIR, ownership, borrowing, moves, copies, drops, macros, unsafe, and FFI;
- structural metrics and interprocedural summaries;
- explicit unknown symbols, types, call targets, members, modules, memory, and effects.

### 2.2 Excluded query outputs

The query service SHALL reject requests whose requested output is primarily an evaluative judgment, recommendation, prediction, or task-specific conclusion.

Examples of excluded outputs include:

```text
whether a refactor is safe
which tests are impacted
whether code is risky
whether a design is good or bad
whether a vulnerability is exploitable
what code should be changed
which change should be prioritized
whether a class is a god object
whether complexity is high
```

The fact-equivalent forms remain valid. For example:

```text
Rejected: Is this function too complex?
Allowed:  Return the function's cyclomatic complexity, branch count, and loop nesting depth.

Rejected: Which tests are impacted by this function?
Allowed:  Return test callables that directly or transitively call this function.

Rejected: Is this refactor safe?
Allowed:  Return callers, callees, reads, writes, aliases, overrides, implementations, and unresolved targets.
```

### 2.3 Present-state boundary

Every request SHALL run against one atomically consistent present-state CPG snapshot.

The query interface SHALL NOT mix:

- source facts from one snapshot with semantic facts from another;
- current syntax with stale lowered code;
- facts produced under incompatible indexed semantic contexts;
- independent query blocks from different graph generations.

Historical revisions, semantic diffs, Git history, runtime traces, and live environment state are outside this specification.

---

## 3. Normative language

The key words **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

- **SHALL / SHALL NOT** indicate conformance requirements.
- **SHOULD / SHOULD NOT** indicate strong recommendations that may be departed from only with a documented reason.
- **MAY** indicates optional behavior.

---

## 4. Core design principles

### 4.1 Semantic values inside structured forms

The request envelope and query forms are structured JSON or YAML.

The values that describe code meaning SHALL be plain-language semantic phrases.

Example:

```yaml
request: follow code relationships
relationship: direct and possible calls made by each function
```

The agent SHALL NOT need to write:

```text
MATCH (f)-[:CONTAINS_CALL]->(c)-[:MAY_CALL]->(g)
CALLS_EXACT_TARGET
SELECT * FROM call_edges
```

The service MAY compile the semantic request into any internal representation, but internal syntax SHALL remain behind the interface.

### 4.2 Fact-only request forms

The specification defines eight request forms:

1. **find code entities**;
2. **retrieve facts about code**;
3. **follow code relationships**;
4. **find connecting fact paths**;
5. **match a code fact pattern**;
6. **combine result sets**;
7. **summarize objective facts**;
8. **retrieve source and syntax context**.

There is no request form for engineering judgment or recommendations.

### 4.3 Arbitrary composition

A request MAY contain any number of independent or dependent query blocks.

A query block MAY reference:

- entities returned by another query;
- facts returned by another query;
- subjects or objects of returned facts;
- entities or facts on returned paths;
- named bindings from a pattern result;
- groups or members from a deterministic summary;
- source contexts returned by another query.

The resulting dependency structure is a directed acyclic graph, not a fixed pipeline.

### 4.4 One logical response

All query blocks SHALL be represented in one response envelope.

The transport MAY stream or chunk a large payload, but the chunks SHALL constitute one logical response to one request.

The service SHALL NOT silently omit a query block, silently truncate a result set, or require a second query merely to retrieve another result set from the same request.

### 4.5 Representation boundaries remain explicit

The query interface SHALL preserve distinctions required by the ontology, including:

```text
syntax occurrence != semantic entity
declaration != reference
type syntax != semantic type
call expression != call site != callable
declared function != executable specialization
value != memory location
read != write
copy != move
borrow != raw address taking
normal flow != exceptional or unwind flow
direct fact != transitive fact
resolved target != possible target != unknown target
source-authored entity != generated or lowered entity
```

A response MAY group related representations for convenience, but SHALL NOT collapse their identities.

### 4.6 Unknown is returned as data

An unresolved fact is not an omitted fact.

Relevant queries SHALL return explicit unknown entities and relations such as:

```text
unknown symbol
unknown type
unknown call target
unknown member
unknown module
unknown memory
unknown effect
unknown external implementation
```

### 4.7 Exact, possible, heuristic, and unresolved facts remain separate

The service SHALL NOT merge candidate targets into one invented exact target.

The response SHALL distinguish at least:

- exact;
- statically resolved;
- sound possible;
- possible but not proven;
- modelled;
- heuristic;
- unresolved.

These are resolution classes, not one probability score.

### 4.8 Direct and transitive facts remain separate

A direct fact SHALL NOT be relabeled as transitive merely because it is also included in a transitive closure.

A transitive result SHALL identify:

- that it is transitive;
- the graph projection used;
- the path or supporting facts when requested;
- any unresolved or possible edges that contribute to it.

### 4.9 Deterministic response semantics

Equivalent requests against the same snapshot SHALL produce semantically equivalent results and deterministic ordering, subject only to explicitly documented service limits.

### 4.10 Absence is not assumed from missing data

A negative statement MAY be returned only when:

1. the queried fact family is declared complete for the relevant owners and scope; or
2. the CPG contains an explicit negative fact, such as a proven `does not alias` relation.

Otherwise, the response SHALL report that absence is indeterminate.

---

## 5. Interface overview

The interface consists of:

```text
Request envelope
  ├─ shared present-state scope
  ├─ shared semantic defaults
  ├─ delivery policy
  └─ one or more query blocks
        ├─ semantic request form
        ├─ semantic inputs
        ├─ semantic conditions
        ├─ references to prior result sets
        └─ requested result projection

Response envelope
  ├─ pinned snapshot metadata
  ├─ deduplicated entity dictionary
  ├─ deduplicated fact dictionary
  ├─ path dictionary
  ├─ deterministic-summary dictionary
  ├─ source-context dictionary
  └─ one result record per query block
```

The canonical serialization is JSON. YAML MAY be accepted as an equivalent authoring form.

---

## 6. Canonical request envelope

### 6.1 Full shape

```yaml
specification: composable semantic CPG fact query
version: "1.0"
request_id: optional-client-request-id

scope:
  codebase: the current indexed workspace
  languages:
    - Rust
    - Python
  source_boundaries:
    - include source files under crates and packages
    - exclude build output directories
  semantic_context: the indexed project context used to construct this graph snapshot
  representations:
    - source-authored semantic entities
    - source syntax when explicitly requested
    - generated and lowered counterparts when relevant, kept separate
  external_entities: include referenced external declarations and unknown external implementations
  freshness: use one current atomically consistent snapshot

defaults:
  entity_ambiguity: return all matching entities and explain how each matched
  phrase_ambiguity: reject the affected query block rather than silently choosing a meaning
  uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
  unknowns: include explicit unknown entities and relationships whenever relevant
  absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
  evidence: include source locations, producer, resolution class, and derivation provenance
  representation: do not collapse source occurrences, semantic entities, call sites, executable instances, or lowered entities
  ordering: use deterministic semantic ordering
  deduplication: deduplicate only by canonical application-owned identity
  limits: return every requested result unless a query block explicitly sets a limit

delivery:
  logical_response: return every query result in one response envelope
  large_result_handling: stream chunks as one logical response when necessary
  truncation: never truncate silently

queries:
  - query_id: example
    request: find code entities
    looking_for: the Rust function named build_graph
    return:
      include:
        - canonical semantic identity
        - qualified name
        - source location
```

### 6.2 Required fields

The request envelope requires:

```text
specification
version
queries
```

Each query block requires:

```text
query_id
request
request-form-specific required fields
```

### 6.3 Query identifiers

`query_id` is an application-local handle used only for composition and response correlation.

It SHALL:

- be unique within the request;
- begin with a letter;
- contain only letters, digits, underscore, period, and hyphen;
- not be interpreted as a codebase entity identifier.

### 6.4 Request labels

A query block MAY include a human-readable `label`.

The label SHALL NOT affect query meaning.

---

## 7. Shared scope

### 7.1 `codebase`

Describes the indexed codebase boundary in semantic language.

Examples:

```yaml
codebase: the current indexed workspace
codebase: the repository named codefabric
codebase: the workspace and all indexed source dependencies
```

### 7.2 `languages`

Restricts the request to named language profiles.

Examples:

```yaml
languages: [Rust]
languages: [Python, Rust]
```

### 7.3 `source_boundaries`

Expresses inclusion and exclusion semantically.

Examples:

```yaml
source_boundaries:
  - include source files under crates/cpg
  - include tests under crates/cpg/tests
  - exclude generated files under target
```

The agent does not need to provide glob syntax. The service chooses the physical path-matching mechanism.

### 7.4 `semantic_context`

Identifies the indexed semantic context under which facts were produced.

Examples:

```yaml
semantic_context: the default indexed Rust target and feature configuration
semantic_context: every indexed Rust configuration, returned separately
semantic_context: the Python module-resolution context stored in the graph snapshot
```

This field selects indexed facts. It does not query the live host environment.

### 7.5 `representations`

Specifies which ontology layers may participate.

Recommended phrases include:

```text
source syntax occurrences
source-authored semantic entities
generated entities
lowered entities
compiler instances
Rust MIR entities
concrete Rust monomorphized instances
unknown entities
```

### 7.6 `external_entities`

Controls inclusion of declarations and implementations outside the primary workspace.

Examples:

```yaml
external_entities: include referenced external declarations, but include bodies only when indexed
external_entities: return external symbols as endpoints without traversing their unindexed bodies
```

### 7.7 `freshness`

The default SHALL require one current atomically consistent snapshot.

If required fact families are unavailable for the current snapshot, the service SHALL report them as unavailable rather than silently substituting stale facts.

---

## 8. Shared defaults

### 8.1 Entity ambiguity

Recommended default:

```yaml
entity_ambiguity: return all matching entities and explain how each matched
```

This handles ordinary cases such as:

- overloads;
- same-named functions in multiple modules;
- methods with the same name on multiple types;
- declarations in source and stubs;
- generic declarations and concrete specializations.

### 8.2 Semantic phrase ambiguity

Recommended default:

```yaml
phrase_ambiguity: reject the affected query block rather than silently choosing a meaning
```

Example ambiguity:

```text
"type of x" could mean declared type, inferred type, expected type, narrowed type, or all of them.
```

The response SHOULD return candidate interpretations.

### 8.3 Uncertainty

Recommended default:

```yaml
uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
```

### 8.4 Unknowns

Recommended default:

```yaml
unknowns: include explicit unknown entities and relationships whenever relevant
```

### 8.5 Absence

Recommended default:

```yaml
absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
```

### 8.6 Evidence

Recommended default:

```yaml
evidence: include source locations, producer, resolution class, and derivation provenance
```

### 8.7 Representation

Recommended default:

```yaml
representation: do not collapse source occurrences, semantic entities, call sites, executable instances, or lowered entities
```

### 8.8 Ordering

Recommended default:

```yaml
ordering: use deterministic semantic ordering
```

### 8.9 Deduplication

Recommended default:

```yaml
deduplication: deduplicate only by canonical application-owned identity
```

Text equality, source position, display name, compiler-local ID, and provider-local ID SHALL NOT be used as universal identity.

### 8.10 Limits

Recommended default:

```yaml
limits: return every requested result unless a query block explicitly sets a limit
```

---

## 9. Delivery policy

### 9.1 One logical response

Recommended value:

```yaml
logical_response: return every query result in one response envelope
```

### 9.2 Large result handling

Recommended value:

```yaml
large_result_handling: stream chunks as one logical response when necessary
```

Streaming SHALL preserve:

- one snapshot identifier;
- one entity and fact identity space;
- deterministic order;
- final completeness metadata.

### 9.3 Truncation

Recommended value:

```yaml
truncation: never truncate silently
```

If an explicit query limit is reached, the query result SHALL say so.

If a hard service limit prevents completion and same-response streaming is unavailable, the service SHOULD reject the affected query before presenting a partial result as complete.

---

## 10. Semantic inputs and result references

### 10.1 Direct semantic reference

A direct input may be a plain-language reference:

```yaml
about:
  - the Rust function `crate::module::build_graph`
```

Equivalent explicit form:

```yaml
about:
  - semantic_reference: the Rust function `crate::module::build_graph`
```

### 10.2 Canonical entity reference

```yaml
about:
  - entity_id: entity:callable:4d88...
```

Canonical IDs are service-issued, application-owned identities.

### 10.3 Fact reference

```yaml
about:
  - fact_id: fact:call-target:3a21...
```

This is useful for retrieving provenance, supporting facts, or source context for a prior fact.

### 10.4 Source-location reference

```yaml
about:
  - source_location:
      source_file: crates/cpg/src/query.rs
      start_line: 120
      start_column: 9
      semantic_location: the call expression beginning on this line
```

Byte offsets MAY be supplied when already known, but are not required for ordinary agent queries.

### 10.5 Prior-result reference

```yaml
starting_from:
  - results_of: find_entrypoints
    select: the returned callable entities
```

The `select` value is semantic.

Examples:

```yaml
select: the target entities of the returned call facts
select: the entities bound as `implementation`
select: every memory location written by the returned facts
select: all entities on the returned paths
select: members of groups whose language is Rust
```

### 10.6 Filtering a prior-result reference

```yaml
starting_from:
  - results_of: direct_calls
    select: the possible target entities
    where:
      - the target is declared inside the current workspace
```

### 10.7 Type safety of references

A result reference SHALL fail clearly when its selected output cannot satisfy the consuming field.

Example:

```text
A path query requiring starting entities cannot consume a selected scalar count.
```

The response SHALL identify:

- the producing query;
- the selected result meaning;
- the expected input meaning;
- the incompatibility.


---

# Part I — Query Request Forms

## 11. Common query-block fields

Every query form supports the following common fields.

### 11.1 `query_id`

Unique result and dependency handle.

### 11.2 `request`

One of the eight plain-language request-form names defined by this specification.

### 11.3 `label`

Optional human-readable label with no semantic effect.

### 11.4 `where`

A list of semantic conditions.

Example:

```yaml
where:
  - the entity is source-authored
  - the entity is defined under crates/cpg
  - the fact is direct rather than transitive
  - the target is exact, sound possible, or unresolved
```

The service resolves physical comparison, text matching, range filtering, and graph predicates internally.

### 11.5 `return`

Specifies the requested result projection and presentation.

### 11.6 `on_ambiguity`

Overrides the shared ambiguity policy for this query block.

### 11.7 `on_unavailable`

Defines behavior when a requested fact family is not available in the snapshot.

Recommended values:

```text
return the available facts and identify the unavailable fact families
fail this query block without affecting independent query blocks
```

### 11.8 `extensions`

Reserved for implementation-specific fields that do not change base semantics.

An extension SHALL NOT reinterpret a standard field.

---

## 12. Return specification

### 12.1 Canonical shape

```yaml
return:
  include:
    - canonical semantic identity
    - semantic kind and representation
    - qualified name
    - source location
    - requested facts with certainty and provenance
  exclude:
    - full source text
  result_shape: one entity record with nested fact references for each starting entity
  group_by:
    - starting entity
  order_by:
    - source file path
    - source position
    - semantic kind
    - qualified name
  deduplicate_by: canonical semantic identity
  supporting_facts: include the lower-level facts needed to support derived results
  include_query_result: true
  limit:
    maximum_results: 500
    per: starting entity
    when_exceeded: return the deterministic first results and mark the query incomplete
```

### 12.2 `include`

A list of semantic fields or fact details to return.

Common phrases:

```text
canonical semantic identity
semantic kind
representation layer
language
name and qualified name
source location
source text
owner
call site details
certainty and resolution status
producer and producer version
derivation method
supporting lower-level facts
```

### 12.3 `exclude`

Explicitly removes otherwise default details.

Exclusion SHALL be reported in result coverage so the consumer does not mistake omitted fields for unavailable facts.

### 12.4 `result_shape`

Recommended shapes:

```text
flat fact records
entity records with nested fact references
one record per starting entity
one record per matched pattern binding row
one record per path
one record per deterministic group
source-context records grouped by file
```

### 12.5 `group_by`

Groups returned records without changing fact semantics.

Examples:

```text
starting entity
source file
semantic owner
call site
certainty class
directness
language
```

### 12.6 `order_by`

Specifies semantic order.

The service chooses physical sort syntax.

### 12.7 `deduplicate_by`

Recommended values:

```text
canonical semantic identity
fact identity
source occurrence identity
path identity
```

The service SHALL reject a deduplication rule that would collapse distinct required representations.

### 12.8 `supporting_facts`

Controls support returned for derived or summary facts.

Examples:

```text
include every supporting fact
include one shortest witness path
include only supporting fact identifiers
omit support but retain derivation metadata
```

### 12.9 `include_query_result`

When false, the query MAY serve as an intermediate set while its underlying entity and fact records remain available to dependent queries.

The default is true.

### 12.10 `limit`

Limits SHALL be explicit.

A limited result SHALL report:

- that a limit was applied;
- the limit scope;
- the complete or incomplete status;
- the deterministic ordering used.

---

## 13. Request form: `find code entities`

### 13.1 Purpose

Locates source, semantic, compiler, lowered, generated, specialization, value, memory, control-flow, or unknown entities.

### 13.2 Schema

```yaml
query_id: required-unique-id
request: find code entities
looking_for: a plain-language description of the entities
within:
  - optional semantic scope or prior-result reference
where:
  - optional semantic condition
return:
  include:
    - requested entity details
```

### 13.3 Examples

Find a semantic declaration:

```yaml
- query_id: build_graph
  request: find code entities
  looking_for: the Rust function named `build_graph` in the `cpg` crate
  return:
    include:
      - canonical semantic identity
      - qualified name
      - callable signature
      - source location
```

Find source occurrences rather than declarations:

```yaml
- query_id: all_build_graph_references
  request: find code entities
  looking_for: identifier occurrences that refer to the semantic function returned by `build_graph`
  within:
    - results_of: build_graph
      select: the returned semantic function
```

Find lowered entities:

```yaml
- query_id: rust_instances
  request: find code entities
  looking_for: every concrete Rust executable specialization of the generic function returned by `build_graph`
  within:
    - results_of: build_graph
      select: the returned function
```

Find unknowns:

```yaml
- query_id: unresolved_calls
  request: find code entities
  looking_for: unknown call targets referenced by call sites under crates/cpg
```

### 13.4 Resolution rules

- The noun **function** defaults to a semantic callable declaration.
- **function syntax** or **function definition syntax** selects syntax occurrences.
- **call** defaults to a call-site entity, not a callee.
- **identifier** defaults to a source occurrence.
- **type** defaults to a semantic type.
- **type annotation** selects type syntax.
- **Rust function instance** selects a concrete executable specialization.
- **memory location** selects an abstract state location, not a value.

The resolved interpretation SHALL be returned.

---

## 14. Request form: `retrieve facts about code`

### 14.1 Purpose

Retrieves properties, relationships, state, summaries, provenance, and explicit unknowns about entities or facts.

### 14.2 Schema

```yaml
query_id: required-unique-id
request: retrieve facts about code
about:
  - one or more semantic, canonical, source, or prior-result references
facts:
  - one or more plain-language fact requests
at: optional program point or semantic location
where:
  - optional conditions
return:
  include:
    - requested details
```

### 14.3 Examples

Retrieve a callable contract:

```yaml
- query_id: callable_contract
  request: retrieve facts about code
  about:
    - results_of: build_graph
      select: the returned callable entities
  facts:
    - the complete callable contract, including parameters, parameter kinds, defaults, receiver semantics, generic parameters, return type, async status, ABI, and unsafe status
```

Retrieve type distinctions:

```yaml
- query_id: variable_types
  request: retrieve facts about code
  about:
    - the local variable `graph` inside `build_graph`
  facts:
    - its declared type
    - its inferred or computed type
    - its contextual expected type
    - every narrowing result at each program point
```

Retrieve call-site facts:

```yaml
- query_id: calls_in_build_graph
  request: retrieve facts about code
  about:
    - results_of: build_graph
      select: the returned function
  facts:
    - every directly contained call site
    - each call site's callee expression, receiver, arguments, and argument-to-parameter bindings
    - each call site's dispatch mechanism, declared target, exact target, possible targets, concrete executable instances, and unknown target state
```

Retrieve provenance for a derived fact:

```yaml
- query_id: dominance_support
  request: retrieve facts about code
  about:
    - fact_id: fact:dominates:abc123
  facts:
    - the graph projection, derivation method, producer version, owner, and supporting control-flow facts
```

Retrieve program-point state:

```yaml
- query_id: ownership_at_drop
  request: retrieve facts about code
  about:
    - the Rust place `state.buffer` inside `process`
  facts:
    - initialization, ownership, move, borrow, and liveness state
  at: immediately before the drop terminator for `state`
```

### 14.4 Breadth control

A broad phrase such as:

```text
all facts about this function
```

MAY be supported, but the service SHOULD expand it into explicit fact families in `resolved_semantics` so the agent can see what was included and unavailable.

---

## 15. Request form: `follow code relationships`

### 15.1 Purpose

Traverses one semantic relationship or relationship family from a starting set.

It is the preferred form for:

- callers and callees;
- declarations and references;
- member resolution;
- inheritance and implementation;
- control-flow successors or predecessors;
- definitions and uses;
- reads and writes;
- alias and points-to relations;
- ownership and borrow relations;
- generated/source correspondence;
- direct or transitive reachability.

### 15.2 Schema

```yaml
query_id: required-unique-id
request: follow code relationships
starting_from:
  - one or more inputs
relationship: a plain-language relationship description
direction: optional semantic direction
distance: optional semantic distance
stop_when:
  - optional semantic stopping condition
where:
  - optional node or fact conditions
return:
  include:
    - endpoints, facts, and evidence
```

### 15.3 Distance semantics

Recommended phrases:

```text
one relationship step
two relationship steps
up to five relationship steps
transitively until no new entities remain
transitively within the current crate
```

The default is one relationship step.

### 15.4 Examples

Direct callees:

```yaml
- query_id: direct_callees
  request: follow code relationships
  starting_from:
    - results_of: build_graph
      select: the returned callable entities
  relationship: calls from each callable to exact, possible, or unknown target callables
  direction: from caller to callee
  distance: one relationship step
  return:
    include:
      - the originating call site
      - the target entity
      - dispatch mechanism
      - certainty and resolution status
```

Transitive callers:

```yaml
- query_id: all_callers
  request: follow code relationships
  starting_from:
    - results_of: build_graph
      select: the returned callable entities
  relationship: callers that can reach the starting callable through call-site facts
  direction: from callee to caller
  distance: transitively until no new callers remain
```

Definitions reaching a use:

```yaml
- query_id: reaching_defs
  request: follow code relationships
  starting_from:
    - the use of `config` in the return expression of `build_graph`
  relationship: definitions that reach this use
  direction: from use to reaching definition
  distance: one derived dataflow relationship
```

Rust source to MIR:

```yaml
- query_id: mir_body
  request: follow code relationships
  starting_from:
    - results_of: build_graph
      select: the returned source-level function
  relationship: lowered Rust MIR body corresponding to the source-level function
  direction: from source entity to lowered entity
  distance: one relationship step
```

### 15.5 Traversal defaults

- Cycles SHALL not cause infinite traversal.
- Canonical entity identity SHALL control revisit detection.
- Cycle membership facts SHOULD be returned when relevant.
- A transitive traversal across possible or unresolved edges SHALL preserve those edge classes.
- The service SHALL not silently replace a relationship traversal with name matching.

---

## 16. Request form: `find connecting fact paths`

### 16.1 Purpose

Finds ordered fact paths between starting and ending sets through one or more semantic relationship families.

This request form is appropriate when the path itself is part of the requested fact output.

### 16.2 Schema

```yaml
query_id: required-unique-id
request: find connecting fact paths
starting_from:
  - one or more inputs
ending_at:
  - one or more inputs
through:
  - one or more semantic relationship families
path_policy: a semantic path-selection rule
direction: optional semantic direction
maximum_length: optional positive integer
where:
  - optional path, entity, or fact conditions
return:
  include:
    - ordered entities and facts
```

### 16.3 Path policies

Recommended values:

```text
all shortest paths
one deterministic shortest path for each reachable pair
all simple paths up to the maximum length
all paths that stay within the owning callable
all call paths that contain no unknown target edge
```

A request for every unrestricted path in a cyclic graph SHOULD be rejected as unbounded.

### 16.4 Examples

Call path to I/O:

```yaml
- query_id: path_to_io
  request: find connecting fact paths
  starting_from:
    - results_of: build_graph
      select: the returned callable entities
  ending_at:
    - callables or external implementations that perform file, network, process, or console I/O
  through:
    - call-site relationships
  path_policy: all shortest paths, preserving exact, possible, and unknown edges separately
  maximum_length: 12
```

Value-to-write path:

```yaml
- query_id: parameter_to_write
  request: find connecting fact paths
  starting_from:
    - the parameter value for `change`
  ending_at:
    - abstract memory locations written by `apply_change`
  through:
    - definition-use relationships
    - value-flow relationships
    - data-dependency relationships
    - write relationships
  path_policy: all shortest paths within the owning callable
```

Control condition to effect:

```yaml
- query_id: conditions_governing_write
  request: find connecting fact paths
  starting_from:
    - branch conditions in `update_graph`
  ending_at:
    - writes to the graph store
  through:
    - control-dependence relationships
    - control-flow relationships
    - write relationships
  path_policy: all shortest mechanically justified paths within the function
```

### 16.5 Path result requirements

Every path SHALL preserve:

- ordered entity identifiers;
- ordered fact identifiers;
- start and end identity;
- path length;
- relationship semantics;
- certainty of each fact;
- a certainty summary that does not hide weaker edges.

---

## 17. Request form: `match a code fact pattern`

### 17.1 Purpose

Matches a conjunctive graph pattern with named semantic bindings.

This is the most expressive request form for questions that require several fact conditions to hold together.

### 17.2 Schema

```yaml
query_id: required-unique-id
request: match a code fact pattern
bindings:
  - name: local-binding-name
    match: a plain-language entity or value description
    from:
      - optional input set
relationships:
  - subject: binding-name
    relationship: a plain-language relationship
    object: binding-name
    bind_fact_as: optional-fact-binding
where:
  - optional semantic conditions over bindings or facts
return:
  result_shape: one record per matched binding row
```

### 17.3 Binding rules

Binding names are request-local handles.

They SHALL NOT be interpreted as code identifiers unless the `match` or `where` phrase says so.

Each returned row maps bindings to canonical entity, fact, or scalar identities.

### 17.4 Examples

Functions that directly write a field and are reachable from an entry point:

```yaml
- query_id: reachable_field_writers
  request: match a code fact pattern
  bindings:
    - name: entry
      match: a public callable designated as an application entry point
    - name: writer
      match: a callable
    - name: field
      match: an abstract field location named `status`
  relationships:
    - subject: entry
      relationship: transitively calls
      object: writer
      bind_fact_as: reachability_fact
    - subject: writer
      relationship: directly writes
      object: field
      bind_fact_as: write_fact
  where:
    - the write fact is exact or statically resolved
  return:
    include:
      - every binding
      - both relationship facts
      - one shortest call witness path
```

Dynamic dispatch candidates:

```yaml
- query_id: trait_dispatch_candidates
  request: match a code fact pattern
  bindings:
    - name: call_site
      match: a Rust dynamic trait call site
    - name: contract
      match: the invoked trait method contract
    - name: implementation
      match: a possible concrete implementation method
  relationships:
    - subject: call_site
      relationship: invokes the trait contract
      object: contract
    - subject: call_site
      relationship: may dynamically dispatch to
      object: implementation
  where:
    - include unknown external implementations as explicit matches
```

Python attribute resolution with descriptor semantics:

```yaml
- query_id: descriptor_resolution
  request: match a code fact pattern
  bindings:
    - name: access
      match: a Python attribute access named `value`
    - name: member
      match: a resolved or possible member
    - name: descriptor
      match: a descriptor or property declaration
  relationships:
    - subject: access
      relationship: resolves or may resolve to
      object: member
    - subject: member
      relationship: is governed by descriptor or property semantics from
      object: descriptor
```

### 17.5 Alternatives and negation

Alternative facts SHOULD be expressed as semantic conditions, for example:

```yaml
where:
  - the callable either directly allocates or directly performs I/O
```

Negated conditions SHALL follow the absence rules in Section 30.

A query such as:

```text
match functions that do not call logging
```

is valid only if direct call-target coverage is complete for each candidate function. Otherwise the negative condition is indeterminate.

---

## 18. Request form: `combine result sets`

### 18.1 Purpose

Combines previously returned sets without repeating graph retrieval.

### 18.2 Schema

```yaml
query_id: required-unique-id
request: combine result sets
inputs:
  - results_of: producing-query-id
    select: a semantic subset of the result
  - results_of: another-producing-query-id
    select: a semantic subset of the result
combination: a semantic set operation
identity: optional identity rule
preserve_origin: optional provenance rule
return:
  include:
    - requested combined records
```

### 18.3 Supported combination meanings

Recommended phrases:

```text
union without duplicate canonical identities
intersection by canonical semantic identity
items in the first set but not the second
symmetric difference by fact identity
pair items that share the same semantic owner
merge fact records while preserving every originating query
```

### 18.4 Examples

Union exact and possible targets while preserving certainty:

```yaml
- query_id: all_targets
  request: combine result sets
  inputs:
    - results_of: calls_in_build_graph
      select: exact target entities
    - results_of: calls_in_build_graph
      select: possible and unknown target entities
  combination: union without duplicate canonical identities, while preserving every target's resolution class
  identity: canonical semantic identity
  preserve_origin: retain the originating call site and query for every member
```

Intersection of writers and callers:

```yaml
- query_id: callers_that_write_state
  request: combine result sets
  inputs:
    - results_of: all_callers
      select: returned caller entities
    - results_of: state_writers
      select: returned writer entities
  combination: intersection by canonical semantic identity
```

### 18.5 Combination safety

The service SHALL reject a combination that attempts to equate incompatible identity domains without an explicit semantic mapping.

Example:

```text
A source occurrence and a semantic declaration are not equal merely because they share a name and span.
```

---

## 19. Request form: `summarize objective facts`

### 19.1 Purpose

Produces deterministic counts, groupings, sets, minima, maxima, totals, and mechanically defined metrics from a fact set.

It SHALL NOT produce labels such as high, risky, fragile, bad, or recommended.

### 19.2 Schema

```yaml
query_id: required-unique-id
request: summarize objective facts
input:
  - one or more direct or prior-result references
summaries:
  - one or more deterministic summary requests
group_by:
  - optional semantic grouping dimensions
include_support: optional supporting-fact policy
where:
  - optional conditions
return:
  include:
    - requested values and support
```

### 19.3 Examples

Objective callable metrics:

```yaml
- query_id: function_metrics
  request: summarize objective facts
  input:
    - results_of: build_graph
      select: the returned callable entities
  summaries:
    - statement count
    - expression count
    - basic block count
    - control-flow edge count
    - cyclomatic complexity
    - loop count and maximum loop nesting depth
    - direct call count and unique direct callee count
    - read count and write count
  include_support: retain the owner and derivation metadata for each metric
```

Group unresolved calls by owner:

```yaml
- query_id: unresolved_by_callable
  request: summarize objective facts
  input:
    - results_of: unresolved_calls
      select: every returned unknown call target fact
  summaries:
    - count facts
    - count distinct call sites
  group_by:
    - owning callable
    - dispatch mechanism
```

Interprocedural effect summary:

```yaml
- query_id: effects_summary
  request: summarize objective facts
  input:
    - results_of: build_graph
      select: the returned callable entities
  summaries:
    - direct and transitive reads, kept separate
    - direct and transitive writes, kept separate
    - parameters read and parameters mutated
    - whether allocation, deallocation, I/O, blocking, raising, panic, unwind, spawn, await, unsafe, or FFI may occur
    - whether any effect remains unknown
  include_support: include supporting fact identifiers and one witness path for each transitive effect
```

### 19.4 Summary provenance

Every summary SHALL remain attributable to:

- its input result set;
- its underlying fact families;
- its semantic owner or grouping key;
- its derivation method;
- its supporting facts when requested.

A summary SHALL NOT replace the lower-level facts in the response identity space.

---

## 20. Request form: `retrieve source and syntax context`

### 20.1 Purpose

Returns exact source text and syntax context for entities, facts, paths, or result bindings.

### 20.2 Schema

```yaml
query_id: required-unique-id
request: retrieve source and syntax context
for:
  - one or more inputs
context:
  - one or more semantic context requests
text_handling: optional source-preservation rule
where:
  - optional source conditions
return:
  include:
    - requested source and syntax details
```

### 20.3 Context meanings

Recommended phrases:

```text
the exact source span
the complete enclosing declaration
the complete enclosing callable
the enclosing class, trait, impl, or module
adjacent documentation and comments
applicable directives or attributes
the complete call expression with receiver and arguments
the syntax subtree with field names and child ordering
all source spans corresponding to this generated or lowered entity
the source-authored entity from which this lowered entity was generated
the parse error or missing-syntax nodes in the enclosing declaration
```

### 20.4 Examples

Source for call sites:

```yaml
- query_id: call_source
  request: retrieve source and syntax context
  for:
    - results_of: calls_in_build_graph
      select: every returned call site
  context:
    - the complete call expression with receiver and arguments
    - the complete enclosing statement
    - adjacent comments and applicable directives
  text_handling: preserve exact source text and line endings from the pinned snapshot
```

Source/MIR correspondence:

```yaml
- query_id: mir_source_correspondence
  request: retrieve source and syntax context
  for:
    - results_of: mir_body
      select: MIR statements, terminators, and call sites
  context:
    - every source span corresponding to each lowered entity
    - the source-authored enclosing function
```

### 20.5 Exactness

Source text SHALL be tied to:

- source file identity;
- source digest;
- canonical half-open byte range;
- optional line and column presentation.

Line and column presentation SHALL NOT replace canonical byte offsets in the response record.


---

# Part II — Composition and Execution Semantics

## 21. Query dependency graph

### 21.1 Dependency inference

A query block depends on another query block whenever it contains a `results_of` reference to that query.

Dependencies SHALL be inferred from references. A separate dependency list is not required.

### 21.2 Independent branches

Independent query blocks MAY execute concurrently.

Example:

```text
find Rust entry points ───────┐
                              ├─ same request, same snapshot
find Python dynamic imports ──┘
```

### 21.3 Dependent branches

Dependent query blocks execute only after their inputs are available.

Example:

```text
find function
    ↓
retrieve its call sites
    ├─ follow direct callees
    ├─ retrieve source context
    └─ summarize dispatch classes
```

### 21.4 Fan-in

A query MAY depend on multiple prior result sets.

Example:

```text
find direct callers ─┐
                     ├─ intersection by canonical identity
find state writers ──┘
```

### 21.5 Fan-out

Any prior result MAY feed multiple later queries without repeating retrieval.

### 21.6 Cycles

Circular query dependencies SHALL be rejected before graph execution.

The error SHALL list the dependency cycle.

### 21.7 Query order

Array order is presentation order, not the authoritative execution order.

The response SHOULD preserve request order while reporting actual dependency semantics.

---

## 22. Snapshot consistency

### 22.1 Pin before resolution

The service SHALL pin one CPG snapshot before resolving entity references.

This prevents a name from resolving against one snapshot and its facts from being read from another.

### 22.2 One identity space

All canonical entity and fact IDs in the response SHALL belong to the pinned snapshot identity space.

### 22.3 Snapshot change during execution

If the indexed graph advances during request execution, the service SHALL either:

1. continue against the pinned snapshot; or
2. restart the entire request against one newer snapshot.

It SHALL NOT mix results across snapshots.

### 22.4 Provider failure

If a provider required for the pinned snapshot is unavailable:

- available fact families MAY still be returned;
- the unavailable fact families SHALL be identified;
- completeness SHALL be downgraded;
- dependent negative claims SHALL not be asserted.

---

## 23. Semantic phrase resolution

### 23.1 Controlled semantic language

Semantic fields use controlled natural language rather than an internal graph language.

The resolver SHALL understand:

- entity noun phrases;
- relationship verb phrases;
- fact-property phrases;
- semantic conditions;
- result-selection phrases;
- grouping and ordering phrases;
- source-context phrases.

### 23.2 Code identifiers

Backticks SHOULD be used around exact code identifiers and qualified names.

Example:

```text
the Rust method `GraphStore::commit`
```

Code identifiers are case-sensitive according to the source language.

### 23.3 Ordinary words versus identifiers

The phrase:

```text
functions named `read`
```

selects the literal identifier `read`.

The phrase:

```text
functions that read state
```

selects a semantic read relationship.

### 23.4 Path values

Repository-relative paths MAY be included literally.

The agent need not provide glob, regex, or database path syntax.

### 23.5 Synonyms

The service SHOULD accept ordinary semantic synonyms.

Examples:

```text
callee / called function / call target
caller / calling function
field write / write to a field location
reference / name use / identifier occurrence that resolves to
base class / superclass
trait implementation / impl of a trait
```

### 23.6 Canonical interpretation

The response SHALL include a plain-language `resolved_semantics` record for each query block.

It SHOULD state:

- interpreted entity classes;
- interpreted relationship families;
- direction;
- directness;
- certainty classes included;
- representation layers included;
- graph projections used;
- completeness assumptions;
- exclusions applied.

### 23.7 Hidden physical plan

The service MAY use any physical implementation.

The response SHALL NOT require the agent to understand:

- canonical internal edge labels;
- storage schema;
- query planner operators;
- graph-library APIs;
- parser IDs;
- compiler IDs;
- database execution syntax.

A diagnostic extension MAY expose a physical plan for service developers, but it is not part of semantic query meaning.

---

## 24. Ambiguity semantics

### 24.1 Entity ambiguity

Multiple matching entities are often legitimate.

The service SHOULD return all matches and include match reasons such as:

```text
exact qualified-name match
exact local-name match within requested module
source declaration corresponding to requested syntax
concrete specialization of requested generic declaration
possible member reached through receiver type and MRO
```

### 24.2 Semantic ambiguity

When a phrase has materially different ontology meanings, the service SHALL not guess silently.

Examples:

```text
"type" could mean type syntax or semantic type
"call" could mean call syntax, call site, callable declaration, or call edge
"writes x" could mean writes the binding, the value, or an abstract memory location
"implementation" could mean source impl block, method implementation, concrete dispatch target, or external implementation
```

### 24.3 Query-block failure isolation

An ambiguous query block MAY fail while independent query blocks succeed.

Dependent query blocks SHALL report that their input dependency failed.

### 24.4 Candidate interpretations

An ambiguity error SHOULD return candidate plain-language interpretations that the agent could place back into the same field.

---

## 25. Result-reference selection semantics

### 25.1 Selection operates on semantic result roles

A prior-result selector MAY request:

```text
all returned entities
only source-authored entities
target entities of call facts
subject entities of write facts
unknown endpoints
entities bound as `implementation`
all facts on returned paths
all members of returned groups
source files referenced by returned facts
```

### 25.2 Selection preserves identity

A selector filters or projects records. It SHALL not manufacture a new semantic identity.

### 25.3 Selection over grouped results

A selector MAY refer to group keys, objective values, or members.

Example:

```yaml
- results_of: unresolved_by_callable
  select: callables in groups with at least three unresolved call sites
```

This remains an objective threshold filter.

### 25.4 Selection over paths

A selector MAY extract:

- path starts;
- path ends;
- all intermediate entities;
- all path facts;
- only facts of a named semantic family;
- entities at a specified path position.

### 25.5 Selection over pattern rows

A selector SHOULD name the binding.

Example:

```yaml
select: the entities bound as `implementation`
```

---

## 26. Representation semantics

### 26.1 Source occurrence

A concrete occurrence in source text.

Examples:

- identifier token;
- call expression syntax;
- type annotation syntax;
- macro invocation syntax.

### 26.2 Semantic entity

A language-level entity independent of one source occurrence.

Examples:

- function declaration;
- local binding;
- class;
- trait;
- semantic type;
- abstract memory location.

### 26.3 Generated entity

An entity produced by a language or framework mechanism while remaining semantically attributable to source.

Examples:

- Python framework-synthesized member;
- Rust macro-expanded item.

### 26.4 Lowered entity

A compiler or semantic lowering representation.

Examples:

- Rust MIR body;
- MIR statement;
- coroutine state;
- drop glue.

### 26.5 Executable specialization

A concrete executable instance distinct from its generic source declaration.

Example:

- Rust monomorphized function instance.

### 26.6 Unknown entity

An explicit endpoint representing unresolved semantics.

### 26.7 Default interpretation table

| Agent phrase | Default entity interpretation |
|---|---|
| function, method, class, trait | semantic declaration |
| identifier, token, expression syntax | source occurrence |
| call | call-site entity |
| call expression | syntax occurrence linked to a call site |
| callee or call target | callable semantic entity or executable instance as requested |
| type | semantic type |
| type annotation | type syntax |
| variable | semantic binding or variable entity |
| value | value entity |
| field or memory written | abstract memory location |
| Rust MIR block | lowered control-flow entity |
| Rust instance | executable specialization |

### 26.8 Grouping related representations

The response MAY provide correspondence links such as:

```text
source occurrence denotes semantic declaration
source declaration lowers to MIR body
macro invocation expands to generated item
source generic declaration monomorphizes to executable instance
```

Each endpoint remains independently addressable.

---

## 27. Fact classes

Each fact record SHALL identify one of the following semantic classes:

```text
source fact
semantic fact
compiler or lowered fact
derived graph fact
deterministic summary fact
```

### 27.1 Source fact

Directly observable from source.

### 27.2 Semantic fact

Determined by language semantics or a semantic provider.

### 27.3 Compiler or lowered fact

Exposed by compiler or intermediate representation.

### 27.4 Derived graph fact

Mechanically computed from lower-level facts.

### 27.5 Deterministic summary fact

A reproducible compression of a fact set.

---

## 28. Certainty and resolution

### 28.1 Required classes

The response SHOULD use plain-language forms of these ontology categories:

| Response value | Meaning |
|---|---|
| `exact` | The target or proposition is exact under the provider model. |
| `statically resolved` | Determined by language semantics or a semantic provider. |
| `sound possible` | A conservative possible relation that should not omit modeled valid targets. |
| `possible` | A plausible modeled relation not asserted as sound-complete. |
| `modelled` | Produced by an explicit semantic model, such as synthesized framework behavior. |
| `heuristic` | Produced by a documented fallback heuristic. |
| `unresolved` | The endpoint or proposition remains unknown. |

### 28.2 No probability conflation

The service SHALL NOT convert these classes into a single confidence percentage unless an optional provider-specific extension defines a separately documented probability model.

### 28.3 Certainty filtering

A query MAY request:

```text
exact targets only
exact and statically resolved targets
all exact, possible, heuristic, and unresolved targets
sound possible targets but not heuristic targets
```

The response coverage SHALL list excluded resolution classes.

### 28.4 Composite facts

A path or transitive fact containing a weaker edge SHALL not be labeled exact overall.

The response SHOULD retain per-edge certainty and provide a conservative certainty summary.

---

## 29. Directness and transitivity

### 29.1 Default directness

Unless the semantic phrase explicitly requests transitive reachability, a relationship request defaults to one direct relationship step.

### 29.2 Direct facts

Examples:

```text
directly contains a call site
directly calls through this call site
directly writes a memory location
directly imports a module
directly inherits from a type
```

### 29.3 Transitive facts

Examples:

```text
transitively reaches through calls
transitively writes through callees
transitively reads through callees
transitively depends on modules
```

### 29.4 Separate output

When both are requested, responses SHALL use separate fact records or separate summary fields.

### 29.5 Supporting paths

A transitive fact SHOULD be able to return:

- one shortest witness path;
- every shortest witness path;
- all supporting direct edges;
- only supporting fact IDs.

### 29.6 Recursive graphs

Transitive call results SHOULD expose recursion or strongly connected component membership when relevant rather than unrolling cycles indefinitely.

---

## 30. Unknowns, negative facts, and absence

### 30.1 Explicit unknowns

Relevant result sets SHALL preserve unknown endpoints.

Example:

```text
call site C may call unknown call target U
```

### 30.2 Empty result categories

An empty query result SHALL identify which of the following applies:

1. **proven empty within the complete queried scope**;
2. **empty after explicit request filtering**;
3. **indeterminate because resolution is unknown**;
4. **unavailable because the fact family was not produced**;
5. **incomplete because a provider or owner failed**;
6. **limited by an explicit result limit**.

### 30.3 Negative property requests

The service MAY return an explicit negative property only when it is represented or mechanically proven.

Example:

```text
proven not to alias
```

The service SHALL NOT infer:

```text
does not alias
```

merely because no `may alias` edge is present.

### 30.4 Closed-world negative matching

A negative pattern such as:

```text
functions that do not directly call logging
```

requires complete direct-call coverage for each candidate function.

For owners without complete coverage, the result SHALL be indeterminate rather than included as a negative match.

### 30.5 Unsupported fact family

If a provider does not expose detailed borrow-loan liveness, a request for loan liveness SHALL return the fact family as unavailable. It SHALL not substitute ordinary borrow syntax and label it loan liveness.

---

## 31. Fact provenance

### 31.1 Required provenance

Every fact SHOULD expose:

```text
producer
producer version
semantic owner
source location where applicable
certainty
resolution status
whether it is derived
```

### 31.2 Derived provenance

A derived fact SHOULD additionally expose:

```text
derivation meaning
underlying fact families
graph projection
supporting fact identifiers or witness paths when requested
```

### 31.3 Summary provenance

A deterministic summary SHALL expose:

- input set identity;
- grouping semantics;
- aggregation semantics;
- supporting fact references or counts;
- derivation producer and version.

### 31.4 Provider-local identity

Provider-local parser, type-checker, compiler, MIR, or graph IDs MAY appear only as diagnostic metadata.

They SHALL NOT be canonical entity identity.

---

## 32. Canonical identity

### 32.1 Application-owned IDs

The response SHALL use application-owned canonical IDs for entities and facts.

### 32.2 Source identity

Source references SHALL include:

```text
source file identity
source digest
half-open byte range
```

Line and column positions are presentation fields.

### 32.3 Semantic identity

Semantic entities SHALL not use source position alone as identity.

### 32.4 Anonymous identity

Closures, lambdas, comprehensions, anonymous blocks, and synthesized entities SHOULD use owner-relative structural identity.

### 32.5 Result-set identity

Set operations SHALL use the identity domain stated in the request or the type-appropriate canonical default.

### 32.6 Fact identity

Two facts with the same display sentence are not necessarily the same fact.

Fact identity SHALL account for semantic owner, endpoints or value, program point where relevant, directness, resolution class, and provenance.

---

## 33. Deterministic ordering

### 33.1 Entity order

Default entity ordering SHOULD be:

1. source file path where present;
2. canonical start byte;
3. semantic kind;
4. qualified name;
5. canonical entity ID.

Entities without source locations SHOULD follow source-located entities and sort by semantic kind, qualified name, and ID.

### 33.2 Fact order

Default fact ordering SHOULD be:

1. semantic owner;
2. source location;
3. fact class;
4. relationship meaning;
5. subject ID;
6. object ID or canonical value representation;
7. fact ID.

### 33.3 Path order

Default path ordering SHOULD be:

1. path length;
2. start ID;
3. end ID;
4. ordered fact-ID sequence.

### 33.4 Group order

Groups SHOULD sort by canonical group-key representation.

### 33.5 Stability

Ordering rules SHALL remain stable within a specification version unless explicitly overridden by the request.

---

## 34. Limits and bounded execution

### 34.1 Explicit limits

An agent MAY limit results for context efficiency.

Example:

```yaml
limit:
  maximum_results: 20
  per: starting callable
  when_exceeded: return the first results in deterministic order and mark the query incomplete
```

### 34.2 No implicit semantic limit

The service SHALL not reinterpret:

```text
all callers
```

as:

```text
the first 100 callers
```

without reporting a hard service limit.

### 34.3 Unbounded path requests

Requests for all unrestricted paths through a cyclic graph MAY be rejected as unbounded.

The error SHOULD recommend a semantic bound such as:

```text
all shortest paths
all simple paths up to ten steps
transitive reachable endpoints without enumerating every path
```

### 34.4 Resource governance

The service MAY enforce hard resource limits, but SHALL distinguish:

- query rejection before execution;
- explicitly limited result;
- provider failure;
- incomplete result.

### 34.5 Same-response streaming

Large complete result sets SHOULD be streamed within the same logical response when supported.

---

## 35. Query-block error isolation

### 35.1 Independent failure

An independent query failure SHALL not invalidate unrelated successful queries.

### 35.2 Dependency failure

A query whose required input failed SHALL return:

```text
not executed because a dependency failed
```

### 35.3 Partial fact-family availability

A fact query MAY return available requested fact families and separately report unavailable families.

### 35.4 No silent fallback

The service SHALL not silently replace:

- semantic type resolution with syntax text;
- exact targets with name matches;
- borrow facts with reference syntax;
- runtime configuration with source-declared configuration;
- current facts with stale facts.


---

# Part III — Canonical Response

## 36. Response envelope

### 36.1 Canonical shape

```yaml
specification: composable semantic CPG fact query response
version: "1.0"
request_id: optional-client-request-id
status: complete

snapshot:
  snapshot_id: snapshot:...
  semantic_description: one current atomically consistent CPG snapshot
  consistency: every returned fact belongs to this snapshot
  semantic_context: the indexed project context used for the request
  providers:
    - producer: source frontend
      producer_version: ...
      status: complete
      fact_families:
        - source and syntax facts
    - producer: semantic provider
      producer_version: ...
      status: complete
      fact_families:
        - symbol, type, and call-resolution facts
  unavailable_fact_families: []

entities:
  entity:...:
    entity_id: entity:...
    semantic_kind: function
    representation: source-authored semantic entity
    language: Rust
    name: build_graph
    qualified_name: crate::module::build_graph
    source_references: []
    properties: {}

facts:
  fact:...:
    fact_id: fact:...
    statement: call site C has exact target function F
    fact_class: semantic fact
    subject_entity_id: entity:call-site:...
    relationship: has exact call target
    object_entity_id: entity:function:...
    directness: direct
    certainty: exact
    resolution_status: resolved
    provenance: {}

paths: {}
groups: {}
source_contexts: {}

query_results:
  - query_id: build_graph
    request: find code entities
    status: complete
    resolved_semantics: {}
    entity_ids:
      - entity:...
    fact_ids: []
    path_ids: []
    group_ids: []
    source_context_ids: []
    coverage: {}
    notices: []
```

### 36.2 Deduplicated dictionaries

The response stores canonical records once and refers to them by ID from each query result.

This reduces repeated tokens when:

- the same entity appears in several query blocks;
- multiple facts share one source location;
- the same path supports several derived facts;
- independent query branches converge on the same entities.

### 36.3 Inline previews

A transport MAY add concise inline previews, but the canonical record remains the dictionary entry.

### 36.4 Query-result order

`query_results` SHOULD follow request order even when execution order differs.

---

## 37. Snapshot record

The snapshot record SHALL include:

```text
snapshot_id
consistency statement
semantic context
provider status
unavailable fact families
```

### 37.1 Provider status

Each provider record SHOULD include:

```text
producer
producer version
status
fact families
notice where relevant
```

### 37.2 Snapshot status and fact status

A snapshot may be internally consistent while a requested fact family is unavailable.

The response SHALL represent both separately.

### 37.3 No stale substitution

If a semantic provider failed for the current source snapshot, the service SHALL not present prior semantic facts as current.

It MAY identify that current semantic facts are unavailable.

---

## 38. Entity record

### 38.1 Required fields

```text
entity_id
semantic_kind
representation
```

### 38.2 Recommended fields

```text
language
name
qualified_name
owner_entity_id
source_references
requested properties
external status
identity notes
```

### 38.3 Semantic kind

`semantic_kind` SHOULD use readable language such as:

```text
source file
identifier occurrence
syntax node
scope
binding
reference
function
method
call site
class
struct
trait
impl
semantic type
value
definition event
use event
abstract memory location
basic block
MIR statement
unknown call target
```

### 38.4 Representation

Recommended values:

```text
source occurrence
source-authored semantic entity
generated semantic entity
lowered compiler entity
concrete executable specialization
abstract value or state entity
explicit unknown entity
external semantic entity
```

### 38.5 Owner

The owner should be the replacement or recomputation unit appropriate to the fact domain, such as:

```text
source file
module
scope
callable
class or type
MIR body
crate
```

### 38.6 Properties

Only requested or required interpretive properties SHOULD be returned.

Examples:

```text
visibility
mutability
async status
unsafe status
callable signature
type arguments
source syntax kind
raw provider kind
program-point ordinal within an owner
```

---

## 39. Source reference record

A source reference SHALL preserve:

```text
source_file_id
repository-relative or workspace-relative path
source digest
start byte
end byte
```

It SHOULD also provide:

```text
start line
start column
end line
end column
```

The byte interval is half-open:

```text
[start_byte, end_byte)
```

Line and column values are presentation metadata and SHALL remain consistent with the source digest.

---

## 40. Fact record

### 40.1 Required fields

```text
fact_id
statement
fact_class
```

### 40.2 Relation-shaped facts

A relation-shaped fact SHOULD include:

```text
subject_entity_id
relationship
object_entity_id
```

### 40.3 Property-shaped facts

A property-shaped fact SHOULD include:

```text
subject_entity_id
relationship or property meaning
value
```

### 40.4 Fact statement

`statement` is a concise semantic rendering.

Example:

```text
The call site at query.rs:120 has sound possible target `dyn Trait::run` implementation `Worker::run`.
```

The statement is for agent readability. Canonical identity and structured fields remain authoritative.

### 40.5 Directness

Recommended values:

```text
direct
transitive
not applicable
```

### 40.6 Certainty

Uses the classes in Section 28.

### 40.7 Resolution status

Recommended values:

```text
resolved
partially resolved
candidate set resolved
unknown endpoint retained
unavailable
```

### 40.8 Program point

Program-point-dependent facts SHOULD include `program_point_entity_id`.

Examples:

- initialization state;
- liveness;
- move state;
- active loan;
- narrowed type;
- possible constant set.

### 40.9 Supporting facts

Derived or summary facts MAY list `supporting_fact_ids`.

The response SHOULD avoid duplicating those full fact records inline.

---

## 41. Provenance record

Recommended fields:

```text
producer
producer version
is derived
derivation meaning
underlying fact families
graph projection
```

### 41.1 Source and semantic facts

A non-derived fact may still have producer provenance.

### 41.2 Derived graph fact

Example:

```yaml
provenance:
  producer: CPG derived-fact service
  producer_version: 1.0.0
  is_derived: true
  derivation: immediate dominator computed from the callable's control-flow graph including normal and unwind edges
  underlying_fact_families:
    - control-flow blocks
    - normal control-flow edges
    - unwind control-flow edges
  graph_projection: control-flow graph for callable entity:...
```

### 41.3 Summary fact

Example:

```yaml
provenance:
  producer: CPG summary service
  is_derived: true
  derivation: transitive write summary over call-site facts and direct write facts
  underlying_fact_families:
    - call sites and exact or possible targets
    - direct memory writes
  graph_projection: call graph joined with memory-access graph
```

---

## 42. Path record

A path record SHALL include:

```text
path_id
ordered entity_ids
ordered fact_ids
length
```

It SHOULD include:

```text
start_entity_id
end_entity_id
path_policy
certainty_summary
```

### 42.1 Alternation invariant

For an ordinary relationship path:

```text
entity count = fact count + 1
```

unless the path representation explicitly includes property facts or hyper-relational records.

### 42.2 Certainty summary

Recommended values:

```text
all path facts exact or statically resolved
contains sound possible facts
contains heuristic facts
contains unresolved endpoints
```

The per-fact records remain authoritative.

### 42.3 Path identity

Path identity SHALL depend on the ordered entity and fact sequence, not merely start and end entities.

---

## 43. Group record

A deterministic summary group SHALL include:

```text
group_id
group_keys
objective_values
```

It MAY include:

```text
entity_ids
fact_ids
supporting_fact_ids
```

Example:

```yaml
group_keys:
  owning callable: entity:function:...
  dispatch mechanism: dynamic trait dispatch
objective_values:
  unresolved call-site count: 4
  distinct unknown target count: 1
```

---

## 44. Source-context record

A source-context record SHALL include:

```text
context_id
source_reference
context_kind
text
```

It MAY include:

```text
associated entity IDs
associated fact IDs
syntax outline
```

### 44.1 Context kind

Examples:

```text
exact source span
complete enclosing declaration
complete enclosing callable
call expression
adjacent documentation and comments
syntax subtree
source correspondence for lowered entity
```

### 44.2 Syntax outline

A syntax outline MAY include normalized and raw syntax kinds, named fields, child ordering, error flags, and missing-syntax flags.

---

## 45. Query-result record

Every query block SHALL have one result record.

### 45.1 Required fields

```text
query_id
request
status
```

### 45.2 Result references

A query result MAY contain:

```text
entity_ids
fact_ids
path_ids
group_ids
source_context_ids
binding_rows
```

### 45.3 Status values

Recommended values:

```text
complete
complete with explicit unknowns
complete after explicit filtering
incomplete because an explicit limit was reached
partially available
failed
not executed because a dependency failed
```

### 45.4 Resolved semantics

The `resolved_semantics` object SHOULD contain plain-language values for:

```text
request meaning
input meaning
entity classes
relationship families
direction
distance or path policy
directness
certainty classes
representation layers
graph projections
conditions
result selection
```

### 45.5 Binding rows

Pattern queries SHALL return binding rows.

Example:

```yaml
binding_rows:
  - entry: entity:function:entry
    writer: entity:function:writer
    field: entity:memory:status
    reachability_fact: fact:transitive-call:...
    write_fact: fact:direct-write:...
```

---

## 46. Coverage record

Coverage metadata is mandatory whenever uncertainty, absence, provider availability, limits, or negative predicates can affect interpretation.

### 46.1 Recommended fields

```text
result completeness
scope completeness
fact-family status
matched count
unresolved count
explicit unknown count
items excluded by request
whether a limit was applied
truncation status
meaning of absence
```

### 46.2 Fact-family status

For each requested family, the service SHOULD report:

```text
fact family
status
owner count
complete owner count
notice
```

Status examples:

```text
complete for every queried owner
complete for 28 of 30 owners
available but conservative
available with heuristic fallback
unavailable in this snapshot
```

### 46.3 Absence meaning

Examples:

```text
No matching fact exists within the complete queried scope.
No result remains after the request excluded possible and unresolved targets.
Absence is indeterminate because call-target resolution is incomplete for three owners.
The requested loan-liveness fact family is unavailable.
```

### 46.4 Limits

A limited query SHALL not use `complete` unless the result is complete within the explicitly requested limit semantics.

---

## 47. Error record

A failed query SHOULD return:

```text
code
message
field
semantic phrase
candidate interpretations
failed dependency query ID where applicable
```

Recommended error codes are readable phrases:

```text
invalid request schema
unknown request form
ambiguous semantic phrase
unresolved input reference
incompatible result reference
circular query dependency
unsupported fact family
request is not for objective facts
negative condition lacks complete coverage
snapshot unavailable
unbounded path request
hard service limit prevents completion
internal execution failure
```

The service SHALL preserve independent successful query results in the same response.

---

## 48. Response-level status

Recommended values:

```text
complete
complete with query-level failures
failed before any query could run
```

A response-level `complete` requires every included query result to have completed under its requested semantics.

---

# Part IV — Semantic Fact Vocabulary

## 49. Vocabulary model

The phrase catalog below defines recommended semantic meanings, not mandatory exact wording.

A conforming service SHOULD accept semantically equivalent phrases and return its canonical interpretation.

Each phrase is resolved to one or more ontology fact families behind the interface.

---

## 50. Source and lexical phrases

Recommended entity phrases:

```text
source files
a source span
tokens
identifier tokens
comments
documentation or docstrings
directives, pragmas, or attributes
parse errors
missing syntax inserted during error recovery
```

Recommended relationship and fact phrases:

```text
contains this source span
is a token of this syntax node
lexically precedes
is documentation for
this directive applies to
exact source text
raw token kind
normalized token kind
source ordering
```

Required distinctions:

- exact source text versus normalized syntax;
- comments versus language-recognized documentation;
- parse errors versus missing syntax;
- byte range versus line and column presentation.

---

## 51. Syntax phrases

Recommended entities:

```text
syntax node
statement
expression
pattern
declaration syntax
type syntax
parameter syntax
argument syntax
block
literal
operation
attribute or member access
subscript or index access
call expression
assignment
branch
loop
return
yield
await
raise or panic syntax
import or use syntax
```

Recommended facts:

```text
raw language-specific syntax kind
normalized syntax kind
source span
whether the node is named
whether the node is an error
whether the node is missing
ordered AST children
AST field name
parent syntax node
enclosing syntax node
lexically next syntax node
```

Required distinction:

```text
call expression syntax != semantic call site
```

---

## 52. Semantic identity, scopes, bindings, and references

Recommended entities:

```text
module
namespace
scope
symbol
declaration
binding
reference
function
method
closure
lambda
constructor
parameter
class
struct
enum
union
trait
protocol
interface
enum variant
field
property
member
variable
local
global
static
constant
type alias
type parameter
lifetime parameter
const parameter
external symbol
synthesized symbol
generated symbol
```

Recommended facts:

```text
declares
defined in
owned by
contains
has scope
enclosing scope
binds
refers to
may refer to
shadows
captures
captured from
aliases
rebinds
visible bindings
free variables
captured variables
reference classification
```

Reference classifications include:

```text
declaration
definition
read
write
read and write
import binding
parameter binding
capture
type reference
call reference
member reference
```

Unknown phrase:

```text
reference resolves to an unknown symbol
```

---

## 53. Module and dependency phrases

Recommended entities:

```text
module
package
crate
import declaration
import binding
export
re-export
external dependency reference
```

Recommended facts:

```text
imports module
imports symbol
exports
re-exports
aliases imported name
defined in module
depends on module
resolved module
resolved imported symbol
local imported binding
```

Required distinctions:

```text
import syntax != local import binding != resolved module != resolved imported symbol != re-export
```

---

## 54. Type phrases

Recommended entities:

```text
unknown type
error type
any or dynamic type
never or bottom type
null or None type
primitive type
nominal type
class object type
type object
literal type
union type
intersection type
callable type
bound method type
tuple, array, list, sequence, or mapping type
structural type
generic type
type parameter or type variable
associated type
type alias
reference type
pointer type
```

Recommended facts:

```text
declared type
inferred type
computed type
expected contextual type
type of
parameter type
return type
field type
type parameter of
type argument
instantiates
subtype of
supertype of
bounded by
constrained by
coerces to
casts to
narrows to
```

Required distinctions:

```text
declared type != inferred or computed type != expected type != narrowing result
```

---

## 55. Members and object-model phrases

Recommended entities:

```text
member
field
method
property
descriptor
associated item
```

Recommended facts:

```text
declares member
has member
inherits
implements
implements trait
implements method
overrides
overridden by
resolves member
may resolve member
receiver type
declaring type
resolved owner type
static or instance status
class-member status
read-only or writeable
final or abstract
```

Unknown phrase:

```text
member resolution reaches an unknown member
```

---

## 56. Callable contract phrases

Recommended facts:

```text
name and qualified name
parameter count and ordering
parameter kinds
default values or default expressions
receiver semantics
variadic status
generic parameters
return type
async status
generator status
ABI or calling convention
unsafe status
const status
```

Recommended relationships:

```text
has parameter
returns type
has type parameter
has generic constraint
captures
```

---

## 57. Call-site and dispatch phrases

Recommended entities:

```text
call site
callee expression
receiver
argument
argument binding
callable declaration
concrete callable instance
unknown call target
```

Recommended facts:

```text
contains call site
has callee expression
has receiver
has argument
argument binds to parameter
calls declared callable contract
has exact call target
calls concrete executable instance
may call possible target
calls unknown target
references callable
takes function address
passes callable as a value
returns callable as a value
```

Dispatch phrases:

```text
direct dispatch
static method dispatch
bound method dispatch
constructor dispatch
closure dispatch
function-pointer dispatch
callable-object dispatch
static trait dispatch
dynamic trait dispatch
vtable dispatch
virtual override dispatch
intrinsic call
foreign call
compiler shim
drop glue
unknown dynamic dispatch
```

Required distinctions:

```text
declared target != exact executable target != possible target set != unknown target
```

---

## 58. Control-flow phrases

Recommended entities:

```text
control-flow graph
entry
exit
basic block
instruction
operation
branch
switch
loop header
return point
exceptional exit
```

Recommended direct relationships:

```text
next control-flow block
true branch
false branch
case branch
loop-back edge
break edge
continue edge
return edge
exception edge
unwind edge
call-return edge
predecessor
successor
```

Recommended derived facts:

```text
reachable block
unreachable block from the callable entry
dominates
strictly dominates
immediate dominator
post-dominates
immediate post-dominator
control dependent on
back edge
loop member
loop header
loop nesting depth
control-flow strongly connected component
```

Required distinction:

```text
normal control flow != exceptional or unwind control flow
```

---

## 59. Values, definitions, uses, and dataflow phrases

Recommended entities:

```text
value
constant value
parameter value
return value
temporary value
merged value
unknown value
definition event
use event
```

Recommended facts:

```text
produces value
consumes value
operand
result
defines
uses
definition reaches use
definition-use relationship
data dependency
value flows to
reaching definition
live at program point
kills definition
```

Definition categories:

```text
initialization
assignment
parameter initialization
mutation
return assignment
merged definition
```

Use categories:

```text
read
argument
condition
return
receiver
index
dereference
```

---

## 60. Memory, state, alias, and points-to phrases

Recommended locations:

```text
local location
parameter location
global location
static location
field location
instance-member location
class-member location
indexed location
container-element location
dereferenced location
heap object
unknown memory
```

Recommended facts:

```text
structured access path
reads
writes
mutates
initializes
deinitializes
takes address of
dereferences
must alias
may alias
proven not to alias
points to
may point to
alias set
points-to set
```

Required distinctions:

```text
value != memory location
must alias != may alias != proven not to alias
```

---

## 61. Program-point state phrases

Recommended facts:

```text
initialized at
uninitialized at
may be uninitialized at
known constant at
possible constant set
null at
non-null at
may be null at
known variant at
possible variants at
```

Every such fact SHALL identify its program point.

---

## 62. Effect phrases

Recommended direct effects:

```text
reads state
writes state
mutates argument
allocates
deallocates
may raise
may panic
may unwind
performs I/O
may block
spawns task
spawns thread
awaits
acquires lock
releases lock
calls foreign code
uses unsafe operation
uses inline assembly
```

Recommended summary phrases:

```text
direct effect
transitive effect through callees
directly writes
transitively writes
directly performs I/O
may perform I/O through callees
unknown effect
```

---

## 63. Exceptional-flow phrases

Recommended entities:

```text
raise site
panic site
assert site
handler
catch clause
except clause
finally region
cleanup region
unwind edge
```

Recommended facts:

```text
raises
may raise
handled by
may be handled by
propagates to
unwinds to
executes cleanup
```

---

## 64. Resource-lifetime phrases

Recommended entities:

```text
resource creation
resource acquisition
resource use
resource release
resource drop
```

Recommended facts:

```text
creates resource
acquires resource
owns resource
transfers resource
uses resource
releases resource
drops resource
```

The base interface SHALL not infer a `resource leak` label.

---

## 65. Async and concurrency phrases

Recommended entities:

```text
coroutine
future
generator
task
thread
channel
lock
```

Recommended facts:

```text
creates future
spawns
awaits
yields
resumes
joins
sends
receives
acquires lock
releases lock
may run concurrently with
happens before
```

Concurrency facts SHALL remain mechanically justified.

---

## 66. Closure and capture phrases

Recommended facts:

```text
captures symbol
captured from scope
captures by value
captures by reference
captures mutably
```

---

## 67. Generated, lowered, generic, and specialization phrases

Recommended entities:

```text
source entity
generated entity
expansion
lowered entity
compiler instance
generic declaration
generic parameter
generic argument
specialization
```

Recommended facts:

```text
generated from
expanded from
expands to
lowers to
corresponds to
specializes
monomorphizes
has generic parameter
type argument
const argument
lifetime argument
instantiates
substitutes
```

Required distinction:

```text
generic declaration != concrete specialization
```

---

## 68. Derived graph and metric phrases

Recommended generic graph facts:

```text
in-degree
out-degree
strongly connected component identifier
strongly connected component size
is a recursive strongly connected component
connected component
transitively reaches
transitively reached by
shortest graph distance
```

Call-graph facts:

```text
direct caller
direct callee
transitive caller
transitive callee
call strongly connected component
recursive function
mutually recursive set
```

Control facts:

```text
dominates
post-dominates
control dependent on
back edge
loop member
control-flow strongly connected component
```

Structural metrics:

```text
statement count
expression count
basic block count
control-flow edge count
cyclomatic complexity
loop count
loop nesting depth
direct call count
unique direct callee count
direct caller count
parameter count
generic parameter count
branch count
return count
raise or panic count
read count
write count
```

The service SHALL return scalar values, not evaluative categories.

---

## 69. Interprocedural summary phrases

Recommended callable summaries:

```text
direct callees
possible callees
direct reads
transitive reads
direct writes
transitive writes
parameters read
parameters mutated
possible return types
possible return values or value classes
may allocate
may deallocate
may perform I/O
may block
may raise
may panic
may unwind
may spawn
may await
may use unsafe
may cross FFI
unknown effect
```

The response SHALL preserve direct versus transitive fields.

---

## 70. Explicit unknown phrases

Recommended phrases:

```text
unknown symbol
unknown type
unknown call target
unknown member
unknown module
unknown memory
unknown effect
unknown external implementation
```

Queries MAY target these entities directly.


---

# Part V — Python Semantic Query Vocabulary

## 71. Python scopes and bindings

Recommended scope phrases:

```text
module scope
function scope
class scope
lambda scope
comprehension scope
annotation scope
type-parameter scope
```

Recommended binding phrases:

```text
local binding
parameter binding
global binding
nonlocal binding
import binding
class-member binding
instance-member binding
comprehension target
loop target
with target
exception target
match capture
walrus binding
type-parameter binding
type-alias binding
free variable
cell variable
built-in reference
```

Example:

```yaml
- query_id: captured_nonlocals
  request: match a code fact pattern
  bindings:
    - name: closure
      match: a Python closure or lambda
    - name: binding
      match: a nonlocal, free-variable, or cell-variable binding
  relationships:
    - subject: closure
      relationship: captures
      object: binding
```

---

## 72. Python type phrases

Recommended Python-specific types:

```text
Any
unknown
Never
None type
class instance
class object
module type
literal type
union type
intersection type
callable
bound method
overload
type variable
parameter specification
type-variable tuple
Self
protocol
typed dictionary
type alias
Annotated type
Unpack type
type guard
type-is narrowing type
```

Recommended provenance phrases:

```text
explicit annotation
inferred type
contextual expected type
flow-narrowed type
```

Example:

```yaml
- query_id: python_type_states
  request: retrieve facts about code
  about:
    - the reference to `item` inside the final branch of `dispatch`
  facts:
    - explicit annotation, inferred type, contextual expected type, and every flow-narrowed type at this program point
```

---

## 73. Python object-model phrases

Recommended facts:

```text
method-resolution order precedes
metaclass of
descriptor for
property for
getter for
setter for
deleter for
class method of
static method of
resolves attribute
may resolve attribute
```

An attribute-resolution result SHOULD preserve:

- receiver type;
- declaring class;
- method-resolution-order step;
- descriptor or property semantics;
- instance versus class binding;
- dynamic or unknown fallback.

Example:

```yaml
- query_id: attribute_resolution
  request: retrieve facts about code
  about:
    - every Python attribute access named `model` in `service.py`
  facts:
    - receiver type
    - exact and possible members reached through method-resolution order
    - descriptor or property behavior
    - dynamic or unknown fallback
```

---

## 74. Python call phrases

Recommended call kinds:

```text
direct function call
bound method call
class-method call
static-method call
constructor call
callable-object call
super call
decorator application
async-function call
generator creation
```

Constructor semantics MAY separately request:

```text
resolved __new__ target
resolved __init__ target
```

Callable-object semantics MAY request:

```text
resolved __call__ target
```

Required distinction:

```text
calling an async or generator function != executing or resuming its body
```

---

## 75. Python dynamic-semantics phrases

Recommended factual observations:

```text
uses eval
uses exec
uses getattr
uses setattr
uses delattr
uses __dict__
uses globals
uses locals
uses vars
performs dynamic import
uses star import
writes a monkey-patched member
performs dynamic attribute write
```

Example:

```yaml
- query_id: dynamic_constructs
  request: find code entities
  looking_for: Python syntax or semantic entities that use dynamic name, attribute, or import mechanisms
  where:
    - include eval, exec, getattr, setattr, delattr, __dict__, globals, locals, vars, dynamic imports, star imports, monkey-patch writes, and dynamic attribute writes
```

The response SHALL return observations and unknown-resolution consequences, not a risk label.

---

## 76. Python decorators

Recommended facts:

```text
decorated by
decorator application call
framework-generated member produced by decorator semantics
```

Required distinction:

```text
structural decorator relationship != executable decorator application
```

---

## 77. Python pattern matching

Recommended entities:

```text
match statement
match case
pattern
pattern binding
guard
```

Recommended facts:

```text
match subject
case pattern
bindings introduced by pattern
guard controlling case
control-flow edges for case and guard
possible variants at the match point
```

---

## 78. Python comprehensions

Recommended entities:

```text
comprehension
comprehension scope
generator clause
comprehension target
comprehension iterable
comprehension filter
comprehension result
```

Comprehension-local bindings SHALL remain distinct from surrounding bindings.

---

## 79. Python context managers

Recommended entities and facts:

```text
context manager
enter call
exit call
async enter call
async exit call
exceptional flow through exit logic
```

---

## 80. Python async and generators

Recommended entities:

```text
async function
coroutine object
await site
async iterator
async context manager
generator function
generator object
yield site
yield-from site
```

Recommended facts:

```text
calling creates coroutine object
calling creates generator object
await resumes coroutine execution
yield suspends generator execution
yield from delegates to another iterator or generator
```

---

# Part VI — Rust Semantic Query Vocabulary

## 81. Rust source-semantic entities

Recommended phrases:

```text
crate
module
use declaration
function
method
closure
struct
enum
union
variant
field
trait
impl
associated function
associated type
associated constant
type alias
opaque type
const
static
macro declaration
macro invocation
macro expansion
extern block
foreign function
```

Recommended declaration properties:

```text
visibility
mutability
unsafe
async
const
extern
ABI
variadic
defaultness
representation attribute
all attached Rust attributes
```

---

## 82. Rust generics and lifetimes

Recommended entities:

```text
type parameter
lifetime parameter
const parameter
where predicate
trait bound
lifetime bound
type argument
lifetime argument
const argument
```

Recommended facts:

```text
bounded by
outlives
implements
associated with
has generic parameter
substitutes generic parameter
```

Example:

```yaml
- query_id: generic_contract
  request: retrieve facts about code
  about:
    - the Rust function `build_index`
  facts:
    - every type, lifetime, and const parameter
    - every where predicate, trait bound, and lifetime bound
    - every concrete specialization and its structured arguments
```

---

## 83. Rust type and adjustment phrases

Recommended Rust-specific types:

```text
bool
char
integer
float
str
never
algebraic data type
tuple
array
slice
reference
raw pointer
function definition type
function pointer
closure type
coroutine type
dynamic trait type
opaque type
generic parameter
associated type
projection type
type alias
```

Recommended type adjustments:

```text
automatically dereferences to
automatically borrows or references as
unsizes to
coerces to
reifies as function pointer
```

Additional properties:

```text
mutability
region or lifetime
generic arguments
ABI
```

---

## 84. Rust MIR entities and structure

Recommended entities:

```text
MIR body
MIR local
MIR basic block
MIR statement
MIR terminator
place
place projection
operand
rvalue
MIR call site
drop site
assert site
```

Recommended facts:

```text
owned by MIR body
corresponds to source-level owner
raw MIR variant
normalized semantic kind
block successor
statement ordering within block
terminator kind
source correspondence
```

Example:

```yaml
- query_id: mir_cfg
  request: retrieve facts about code
  about:
    - the MIR body corresponding to `crate::processor::run`
  facts:
    - every MIR local, basic block, statement, and terminator
    - every normal and unwind successor
    - each raw MIR variant and normalized semantic meaning
    - source correspondence for each entity
```

---

## 85. Rust places and projections

Recommended projection phrases:

```text
dereference projection
field projection
index projection
constant-index projection
subslice projection
downcast projection
opaque-cast projection
```

A place SHALL be returned structurally as:

```text
base local + ordered projections
```

Example semantic access path:

```text
`x.foo[i].bar` as base `x`, field `foo`, index `i`, field `bar`
```

It SHALL NOT be returned only as an opaque serialized string.

---

## 86. Rust MIR state-transition phrases

Required distinct phrases:

```text
read
write
copy
move
shared borrow
mutable borrow
reborrow
raw address taking
storage live
storage dead
initialize
deinitialize
drop
```

The service SHALL NOT treat `copy` and `move` as synonyms.

---

## 87. Rust ownership, loans, and regions

Recommended entities and facts:

```text
owns
moves to
copies to
borrows shared
borrows mutably
reborrows
loan
loan created at
loan live at
region
outlives
region contains
move path
owned at program point
moved at program point
shared-borrowed at program point
mutably borrowed at program point
uninitialized at program point
```

Example:

```yaml
- query_id: ownership_flow
  request: find connecting fact paths
  starting_from:
    - the parameter place `buffer` in `consume`
  ending_at:
    - every drop site, transfer, or return destination that receives ownership
  through:
    - move relationships
    - copy relationships
    - borrow and reborrow relationships
    - ownership and drop relationships
  path_policy: all shortest ownership paths, keeping moves, copies, and borrows distinct
```

---

## 88. Rust calls and executable instances

Recommended call kinds:

```text
direct function call
static trait dispatch
dynamic trait dispatch
function-pointer call
closure call
intrinsic call
foreign call
drop glue
compiler shim
coroutine resume
unknown indirect call
```

Required distinction:

```text
declared function != monomorphized executable instance
```

Recommended facts:

```text
monomorphizes to
type argument
lifetime argument
const argument
calls executable instance
```

---

## 89. Rust traits, implementations, and dynamic dispatch

Recommended entities:

```text
trait
trait method
impl
impl method
dynamic trait type
vtable
vtable entry
```

Recommended facts:

```text
implements trait
implements method
invokes trait contract
statically resolves to
unsizes to dynamic trait
uses vtable
may dispatch to
unknown external implementation
```

Required distinction:

```text
static trait resolution != dynamic trait dispatch != conservative candidate set
```

---

## 90. Rust macros

Recommended entities:

```text
macro definition
macro invocation
expansion
expanded item
```

Recommended facts:

```text
invokes macro
expands to
generated from
source correspondence
hygiene context
expansion context
```

Example:

```yaml
- query_id: macro_generated_calls
  request: match a code fact pattern
  bindings:
    - name: invocation
      match: a Rust macro invocation
    - name: generated
      match: an item generated by that invocation
    - name: call_site
      match: a call site inside the generated item
  relationships:
    - subject: invocation
      relationship: expands to
      object: generated
    - subject: generated
      relationship: contains call site
      object: call_site
```

---

## 91. Rust drop and destruction

Recommended entities:

```text
drop site
Drop implementation
drop glue
```

Recommended facts:

```text
drops
invokes Drop implementation
invokes drop glue
drops field
```

Compiler-generated destruction SHALL be queryable even without an explicit source call to `drop`.

---

## 92. Rust async and coroutine lowering

Recommended entities:

```text
async function
future type
coroutine body
coroutine state
suspend point
resume point
```

Recommended facts:

```text
lowers to coroutine
creates future
has coroutine state
suspends at
resumes at
```

Required distinction:

```text
calling an async function != executing or resuming its body
```

---

## 93. Rust unsafe and FFI

Recommended entities:

```text
unsafe block
unsafe function
raw-pointer dereference
raw address
inline assembly
foreign function
extern block
```

Recommended facts:

```text
contains unsafe operation
calls foreign function
crosses FFI
uses inline assembly
```

The response SHALL return facts and source evidence, not a vulnerability or risk label.

---

## 94. Rust constants, statics, and compile-time evaluation

Recommended entities:

```text
const item
static item
constant value
compile-time evaluation result
constant allocation
```

Recommended facts:

```text
references const
references static
evaluates to
references constant allocation
```

Fact availability SHALL reflect what compiler providers expose reliably.


---

# Part VII — Composite Query Examples

## 95. Full composite review of one callable

This example demonstrates:

- entity discovery;
- fact retrieval;
- call traversal;
- path finding;
- set composition;
- deterministic summaries;
- source retrieval;
- one response containing every result.

```yaml
specification: composable semantic CPG fact query
version: "1.0"
request_id: inspect-apply-change

scope:
  codebase: the current indexed workspace
  languages:
    - Rust
  source_boundaries:
    - include source under crates/cpg
  semantic_context: the default indexed Rust target and feature configuration
  representations:
    - source-authored semantic entities
    - concrete executable specializations
    - Rust MIR entities when relevant
    - explicit unknown entities
  external_entities: include referenced external declarations and unknown external implementations
  freshness: use one current atomically consistent snapshot

defaults:
  entity_ambiguity: return all matching entities and explain how each matched
  phrase_ambiguity: reject the affected query block rather than silently choosing a meaning
  uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
  unknowns: include explicit unknown entities and relationships whenever relevant
  absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
  evidence: include source locations, producer, resolution class, and derivation provenance
  representation: do not collapse source occurrences, semantic entities, call sites, executable instances, or lowered entities
  ordering: use deterministic semantic ordering
  deduplication: deduplicate only by canonical application-owned identity
  limits: return every requested result unless a query block explicitly sets a limit

delivery:
  logical_response: return every query result in one response envelope
  large_result_handling: stream chunks as one logical response when necessary
  truncation: never truncate silently

queries:
  - query_id: target
    request: find code entities
    looking_for: the Rust function `apply_change` defined in the CPG update subsystem
    return:
      include:
        - canonical semantic identity
        - qualified name
        - source location
        - callable signature

  - query_id: contract
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned source-authored callable entities
    facts:
      - the complete callable contract
      - declared, computed, and expected types for every parameter and return value
      - generic parameters, bounds, lifetimes, and concrete executable specializations
      - closure captures if any

  - query_id: direct_calls
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned source-authored callable entities
    facts:
      - every directly contained call site
      - each call site's receiver, arguments, and argument-to-parameter bindings
      - declared targets, exact targets, possible targets, executable instances, dispatch mechanism, and unknown target state
    return:
      result_shape: one record per call site
      order_by:
        - source location

  - query_id: exact_targets
    request: follow code relationships
    starting_from:
      - results_of: direct_calls
        select: every returned call site
    relationship: exact or statically resolved target callable of each call site
    direction: from call site to target
    distance: one relationship step

  - query_id: uncertain_targets
    request: follow code relationships
    starting_from:
      - results_of: direct_calls
        select: every returned call site
    relationship: sound possible, possible, heuristic, or unknown target callable of each call site
    direction: from call site to target
    distance: one relationship step

  - query_id: all_direct_targets
    request: combine result sets
    inputs:
      - results_of: exact_targets
        select: every returned target entity
      - results_of: uncertain_targets
        select: every returned target entity
    combination: union without duplicate canonical identities, preserving every target's resolution class and originating call sites
    identity: canonical semantic identity
    preserve_origin: retain every originating query, call site, and target fact

  - query_id: transitive_calls
    request: follow code relationships
    starting_from:
      - results_of: target
        select: the returned callable entities
    relationship: callables reachable through exact, possible, or unknown call-site relationships
    direction: from caller to callee
    distance: transitively until no new callable entities remain
    stop_when:
      - do not traverse bodies of unindexed external entities
    return:
      include:
        - each reachable callable
        - directness and shortest distance
        - one shortest witness path
        - certainty of every supporting call fact

  - query_id: direct_writes
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned callable entities
    facts:
      - every abstract memory location directly written, mutated, initialized, or deinitialized
      - the structured access path for each location
      - the write or mutation program point

  - query_id: transitive_writes
    request: summarize objective facts
    input:
      - results_of: target
        select: the returned callable entities
    summaries:
      - direct and transitive writes, kept separate
      - parameters directly or transitively mutated
      - whether any write effect remains unknown
    include_support: include one shortest call-and-write witness path for each transitive write

  - query_id: parameter_flow
    request: find connecting fact paths
    starting_from:
      - the parameter value named `change` in the callable returned by `target`
    ending_at:
      - results_of: direct_writes
        select: every written or mutated abstract memory location
    through:
      - definition-use relationships
      - reaching-definition relationships
      - value-flow relationships
      - data-dependency relationships
      - write or mutation relationships
    path_policy: all shortest paths within the owning callable

  - query_id: control_conditions
    request: find connecting fact paths
    starting_from:
      - branch conditions in the callable returned by `target`
    ending_at:
      - results_of: direct_writes
        select: every write or mutation fact
    through:
      - control-dependence relationships
      - control-flow relationships
      - write or mutation relationships
    path_policy: all shortest mechanically justified paths within the callable

  - query_id: rust_ownership
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned callable entities
    facts:
      - every MIR move, copy, shared borrow, mutable borrow, reborrow, raw address taking, initialization, deinitialization, and drop
      - every structured MIR place involved
      - loan and region facts where available
      - every source correspondence

  - query_id: effects
    request: summarize objective facts
    input:
      - results_of: target
        select: the returned callable entities
    summaries:
      - direct and transitive effects, kept separate
      - allocation and deallocation
      - I/O and blocking
      - raise, panic, and unwind
      - task or thread spawn and await
      - lock acquisition and release
      - unsafe operations, inline assembly, foreign calls, and FFI crossings
      - unknown effects
    include_support: include supporting fact identifiers and one witness path for each transitive effect

  - query_id: metrics
    request: summarize objective facts
    input:
      - results_of: target
        select: the returned callable entities
    summaries:
      - statement count
      - expression count
      - basic block count
      - control-flow edge count
      - cyclomatic complexity
      - loop count and loop nesting depth
      - branch count
      - return count
      - direct call count
      - unique direct callee count
      - read count
      - write count

  - query_id: source
    request: retrieve source and syntax context
    for:
      - results_of: target
        select: the returned callable entities
      - results_of: direct_calls
        select: every returned call site
      - results_of: direct_writes
        select: every returned write or mutation fact
    context:
      - the complete enclosing callable
      - exact call expressions with receiver and arguments
      - complete statements containing each write
      - adjacent documentation, comments, and applicable attributes
    text_handling: preserve exact source text and line endings from the pinned snapshot
```

---

## 96. Find all code that may mutate a parameter

```yaml
queries:
  - query_id: parameter
    request: find code entities
    looking_for: the parameter named `request` of `handle_request`

  - query_id: mutations
    request: follow code relationships
    starting_from:
      - results_of: parameter
        select: the parameter's abstract location and value entities
    relationship: direct or transitive mutations of the parameter or memory reachable from it
    direction: from the parameter to mutation sites and mutated locations
    distance: transitively through value flow, alias, points-to, call, and mutation relationships
    return:
      include:
        - mutation site
        - mutated access path
        - intervening aliases or pointers
        - call site and argument binding when mutation occurs through a callee
        - exact, possible, or unknown status
```

This request returns facts. A downstream agent may then decide what those facts imply for a change.

---

## 97. Find Python call targets without hiding dynamic uncertainty

```yaml
queries:
  - query_id: call_site
    request: find code entities
    looking_for: the Python call site `service.model.run(payload)` in `handlers.py`

  - query_id: call_details
    request: retrieve facts about code
    about:
      - results_of: call_site
        select: the returned call site
    facts:
      - receiver expression and receiver type
      - declared, inferred, expected, and narrowed receiver types
      - attribute resolution through method-resolution order
      - descriptor or property semantics
      - bound-method semantics
      - exact targets, possible targets, overloads, callable-object targets, and unknown targets
      - argument-to-parameter bindings
      - dynamic constructs that prevent complete resolution
```

The result SHALL not choose one arbitrary target from a union or overload set.

---

## 98. Query Rust destruction semantics

```yaml
queries:
  - query_id: value
    request: find code entities
    looking_for: the Rust local place `guard` in `process_batch`

  - query_id: drop_paths
    request: find connecting fact paths
    starting_from:
      - results_of: value
        select: the local place and value entities
    ending_at:
      - drop sites, Drop implementations, and compiler-generated drop glue that can destroy this value or its fields
    through:
      - move relationships
      - ownership relationships
      - normal and unwind control-flow relationships
      - drop relationships
    path_policy: all shortest destruction paths, preserving normal and unwind paths separately

  - query_id: source_and_mir
    request: retrieve source and syntax context
    for:
      - results_of: drop_paths
        select: every drop site, Drop implementation, drop glue entity, and supporting fact
    context:
      - source correspondence
      - complete enclosing Rust declaration
      - MIR block, statement, or terminator context
```

---

## 99. Independent queries in one request

A request may ask unrelated fact questions and receive every answer together.

```yaml
queries:
  - query_id: rust_ffi
    request: find code entities
    looking_for: every Rust foreign call, raw-pointer dereference, inline-assembly operation, and FFI crossing

  - query_id: python_dynamic_imports
    request: find code entities
    looking_for: every Python dynamic import and star import

  - query_id: recursive_callables
    request: find code entities
    looking_for: every callable in a recursive or mutually recursive call strongly connected component

  - query_id: parse_errors
    request: find code entities
    looking_for: every parse error and missing-syntax recovery node in the current snapshot
```

No artificial chain is required.

---

## 100. Fact-pattern example: externally reachable state writers

```yaml
queries:
  - query_id: externally_reachable_writers
    request: match a code fact pattern
    bindings:
      - name: entry
        match: a public entry-point callable
      - name: writer
        match: a callable inside the current workspace
      - name: location
        match: a global, static, field, or instance-member abstract memory location
    relationships:
      - subject: entry
        relationship: transitively calls through exact or possible call-site facts
        object: writer
        bind_fact_as: call_reachability
      - subject: writer
        relationship: directly writes or mutates
        object: location
        bind_fact_as: write_fact
    where:
      - the entry point and writer are source-authored
    return:
      result_shape: one record per entry, writer, and location binding row
      include:
        - one shortest call witness path
        - the exact write site
        - certainty and unknown-state metadata
```

The output does not label the locations sensitive or the writers risky.

---

## 101. Objective comparison through set composition

Although historical or evaluative comparisons are outside scope, present-state set relations are allowed.

Example: functions that are both direct callers of `commit` and direct writers of `transaction_state`.

```yaml
queries:
  - query_id: commit
    request: find code entities
    looking_for: the callable `GraphStore::commit`

  - query_id: callers
    request: follow code relationships
    starting_from:
      - results_of: commit
        select: the returned callable
    relationship: direct callers through call-site facts
    direction: from callee to caller
    distance: one relationship step

  - query_id: writers
    request: find code entities
    looking_for: callables that directly write the abstract location `transaction_state`

  - query_id: both
    request: combine result sets
    inputs:
      - results_of: callers
        select: direct caller entities
      - results_of: writers
        select: writer callable entities
    combination: intersection by canonical semantic identity
```

---

## 102. Invalid request examples

### 102.1 Evaluative conclusion

```yaml
request: retrieve facts about code
facts:
  - whether this refactor is safe
```

Required response:

```text
request is not for objective facts
```

Suggested factual reformulation MAY list relevant fact families, but the service SHALL not answer the judgment.

### 102.2 Unbounded cyclic path request

```yaml
request: find connecting fact paths
path_policy: every path of any length
```

Required response:

```text
unbounded path request
```

### 102.3 Negative request without complete coverage

```yaml
request: find code entities
looking_for: functions that never call external code
```

If call-target coverage is incomplete, the service SHALL return:

```text
negative condition lacks complete coverage
```

### 102.4 Representation collapse

```yaml
deduplicate_by: same name and source line
```

Required response:

```text
invalid deduplication because it may collapse distinct semantic identities
```

---

# Part VIII — Schema Artifacts

## 103. Request JSON Schema

The normative machine-validation artifact is:

```text
cpg_semantic_query_request.schema.json
```

It validates:

- the request envelope;
- shared scope and defaults;
- delivery policy;
- input and result references;
- all eight query request forms;
- return projections;
- pattern bindings and relationships;
- explicit limits.

Semantic strings remain intentionally open to ontology-aware resolution.

---

## 104. Response JSON Schema

The normative machine-validation artifact is:

```text
cpg_semantic_query_response.schema.json
```

It validates:

- snapshot metadata;
- deduplicated entities;
- facts and provenance;
- paths;
- deterministic groups;
- source contexts;
- query-level results;
- coverage and completeness;
- structured errors.

---

## 105. Schema evolution

### 105.1 Versioning

Breaking changes require a new major specification version.

Backward-compatible additions MAY increment a minor version in future revisions.

### 105.2 Semantic phrase evolution

Adding synonyms does not change query meaning and is backward-compatible.

Changing the canonical meaning of a previously accepted phrase is breaking unless the phrase was previously reported as ambiguous.

### 105.3 New request forms

A new request form is a specification extension.

It SHALL remain fact-only and SHALL not duplicate an existing form merely to expose a physical execution mechanism.

### 105.4 New ontology domains

New fact domains MAY be queried through the existing generic forms when semantic phrases are sufficient.

A new request form is not required merely because a new entity or relationship kind is added to the ontology.

---

# Part IX — Conformance

## 106. Core request conformance

A conforming request implementation SHALL support:

1. one request envelope with multiple query blocks;
2. one atomically consistent present-state snapshot;
3. direct semantic references;
4. canonical entity and fact references;
5. prior-result references;
6. arbitrary acyclic composition;
7. every query result in one logical response;
8. semantic phrase resolution with reported interpretation;
9. explicit uncertainty and unknowns;
10. direct and transitive separation;
11. canonical identity separation;
12. coverage and absence semantics;
13. structured query-level errors.

---

## 107. Query-form conformance

A fully conforming implementation SHALL support all eight request forms:

```text
find code entities
retrieve facts about code
follow code relationships
find connecting fact paths
match a code fact pattern
combine result sets
summarize objective facts
retrieve source and syntax context
```

A partial implementation SHALL advertise unsupported request forms before query execution.

---

## 108. Core fact-domain conformance

The service SHALL be able to query at least:

- source spans and syntax;
- declarations, bindings, references, and scopes;
- semantic types;
- call sites and targets;
- control flow;
- values and def-use;
- state reads and writes;
- unresolved semantic facts;
- objective derived graph facts.

---

## 109. Python query conformance

A Python-conformant service SHOULD support semantic phrases for:

- Python scopes and bindings;
- declared, inferred, expected, and narrowed types;
- method-resolution order;
- descriptors and properties;
- constructor and callable-object calls;
- decorators;
- comprehensions;
- pattern matching;
- async and generator semantics;
- dynamic constructs and explicit unknowns.

---

## 110. Rust query conformance

A Rust-conformant service SHOULD support semantic phrases for:

- crates, modules, items, generics, and lifetimes;
- traits and impls;
- Rust semantic types and adjustments;
- macros and expansion;
- MIR bodies, blocks, statements, terminators, places, operands, and rvalues;
- reads, writes, moves, copies, borrows, reborrows, loans, and regions;
- static and dynamic trait dispatch;
- function pointers and closures;
- monomorphized instances;
- drop glue and compiler shims;
- async and coroutine lowering;
- unsafe operations, inline assembly, and FFI;
- constants, statics, and compile-time evaluation.

---

## 111. Advanced derived-fact conformance

An advanced service SHOULD query and return:

```text
dominators
post-dominators
control dependence
reaching definitions
definition-use facts
liveness
loops
strongly connected components
recursion
transitive reachability
alias and points-to sets
objective callable effect summaries
structural metrics
```

Every derived fact SHALL identify its graph projection and provenance.

---

## 112. Response conformance

A conforming response SHALL:

- contain one query-result record per requested query block;
- use one snapshot identity;
- deduplicate canonical entities and facts across query blocks;
- preserve requested representation distinctions;
- preserve certainty and directness;
- report unavailable fact families;
- identify explicit unknowns;
- distinguish every empty-result category;
- avoid silent truncation;
- avoid provider-local IDs as canonical identities.

---

## 113. Semantic resolution conformance tests

An implementation SHOULD maintain test fixtures proving that ordinary semantic phrases resolve correctly.

Minimum tests SHOULD include:

```text
function vs function syntax
call vs call site vs callable target
type vs type annotation
declared vs inferred vs expected type
exact vs possible vs unknown target
direct vs transitive call
value vs memory location
read vs write
copy vs move
borrow vs raw address taking
normal vs unwind flow
source declaration vs monomorphized instance
Python bound method vs callable object
Rust static trait dispatch vs dynamic trait dispatch
```

---

## 114. Composition conformance tests

An implementation SHOULD test:

```text
independent parallel query blocks
linear dependencies
fan-out
fan-in
set union, intersection, and difference
pattern binding selection
path-result selection
source retrieval from fact IDs
query-level ambiguity failure
independent-query success despite another failure
dependency-failure propagation
cycle rejection
same-snapshot identity across all branches
```

---

## 115. Completeness conformance tests

An implementation SHOULD prove distinct responses for:

```text
proven empty
filtered empty
unresolved
fact family unavailable
provider incomplete
explicit limit reached
hard service limit rejection
```

---

# Part X — Agent Authoring Guidance

## 116. Minimal query-writing procedure

An agent SHOULD:

1. identify the code entities needed;
2. decide whether it needs properties, relationships, paths, patterns, sets, summaries, or source;
3. write each need as one query block;
4. reference prior result roles semantically;
5. specify direct versus transitive where material;
6. specify exact versus possible versus unknown where material;
7. specify source, semantic, generated, lowered, or executable representation where material;
8. request only the fields needed for the next reasoning step;
9. retain uncertainty and coverage metadata;
10. use explicit limits only for context efficiency.

---

## 117. Compact request template

```yaml
specification: composable semantic CPG fact query
version: "1.0"
request_id: optional-id

scope:
  codebase: the current indexed workspace
  languages: [Python, Rust]
  semantic_context: the indexed project context used to construct this graph snapshot
  representations:
    - source-authored semantic entities
    - generated and lowered counterparts when relevant, kept separate
    - explicit unknown entities
  freshness: use one current atomically consistent snapshot

defaults:
  entity_ambiguity: return all matching entities and explain how each matched
  phrase_ambiguity: reject the affected query block rather than silently choosing a meaning
  uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
  unknowns: include explicit unknown entities and relationships whenever relevant
  absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
  evidence: include source locations, producer, resolution class, and derivation provenance
  representation: do not collapse distinct ontology representations
  ordering: use deterministic semantic ordering
  deduplication: deduplicate only by canonical application-owned identity
  limits: return every requested result unless a query block explicitly sets a limit

delivery:
  logical_response: return every query result in one response envelope
  large_result_handling: stream chunks as one logical response when necessary
  truncation: never truncate silently

queries:
  - query_id: find_target
    request: find code entities
    looking_for: the target code entities

  - query_id: get_facts
    request: retrieve facts about code
    about:
      - results_of: find_target
        select: the returned entities
    facts:
      - the exact objective facts needed
```

---

## 118. Query-form decision table

| Need | Request form |
|---|---|
| Locate declarations, occurrences, values, blocks, locations, unknowns | `find code entities` |
| Retrieve properties or immediate fact families | `retrieve facts about code` |
| Traverse callers, callees, refs, types, flow, aliases, ownership | `follow code relationships` |
| Preserve an ordered chain of facts between endpoints | `find connecting fact paths` |
| Require several entities and facts to co-occur | `match a code fact pattern` |
| Union, intersect, subtract, or merge prior sets | `combine result sets` |
| Count, group, or compute deterministic metrics/summaries | `summarize objective facts` |
| Return exact text, syntax trees, or source correspondence | `retrieve source and syntax context` |

---

## 119. High-value phrasing rules

Prefer:

```text
direct exact call targets
sound possible call targets
unknown call targets
transitive callees with one shortest witness path
abstract memory locations directly written
values that flow to this return value
conditions on which this write is control-dependent
Rust moves, copies, and borrows kept separate
Python declared, inferred, expected, and narrowed types kept separate
source-authored declaration and concrete executable specializations kept separate
```

Avoid underspecified phrases such as:

```text
dependencies
uses
related code
all types
calls
state
```

unless the desired broad expansion is intentional.

---

## 120. Context-efficiency guidance

The interface is designed for complete retrieval, but an agent SHOULD avoid asking for unnecessary payload.

Examples:

- request entity IDs, names, locations, and fact IDs before requesting full source;
- request one shortest witness path instead of every path when one path is sufficient;
- request deterministic summaries for large transitive sets, then expand selected groups;
- use shared dictionaries rather than repeating entity records;
- apply explicit per-owner limits only when complete enumeration is not needed;
- never remove unknowns merely to reduce output size without recording that exclusion.

---

# Part XI — Final Specification Principle

## 121. Governing rule

Every request field and every returned record SHALL satisfy this test:

> **Does this request or return an objective fact about the present-state program, a mechanically derived property of those facts, deterministic source context, or metadata required to interpret those facts?**

If yes, it belongs in this interface.

If it instead asks:

> **What should an engineer conclude or do?**

then it belongs to downstream LLM reasoning, not the CPG query interface.

The target architecture is:

```text
LLM programming agent
        ↓
structured semantic fact request
        ↓
ontology-aware semantic resolution
        ↓
hidden physical query planning and graph execution
        ↓
one atomically consistent fact response
        ↓
explicit entities, facts, paths, summaries, source, unknowns, and coverage
        ↓
LLM programming-agent reasoning
```

The specification stops at the complete fact response boundary.

---

# Appendix A — Canonical Request-Form Names

```text
find code entities
retrieve facts about code
follow code relationships
find connecting fact paths
match a code fact pattern
combine result sets
summarize objective facts
retrieve source and syntax context
```

---

# Appendix B — Required Response Distinctions

```text
source occurrence != semantic entity
declaration != reference
type syntax != semantic type
call expression != call site != callable
declared target != exact target != possible target != unknown target
declared function != executable specialization
value != memory location
read != write
copy != move
borrow != raw address taking
normal flow != exceptional or unwind flow
direct fact != transitive fact
source-authored != generated != lowered
proven empty != filtered empty != unresolved != unavailable != incomplete != limited
```

---

# Appendix C — Recommended Default Policies

```yaml
entity_ambiguity: return all matching entities and explain how each matched
phrase_ambiguity: reject the affected query block rather than silently choosing a meaning
uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
unknowns: include explicit unknown entities and relationships whenever relevant
absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
evidence: include source locations, producer, resolution class, and derivation provenance
representation: do not collapse source occurrences, semantic entities, call sites, executable instances, or lowered entities
ordering: use deterministic semantic ordering
deduplication: deduplicate only by canonical application-owned identity
limits: return every requested result unless a query block explicitly sets a limit
```

---

# Appendix D — Explicitly Rejected Output Classes

```text
refactor safety
predicted test impact
risk scores
bug likelihood
architecture quality
vulnerability exploitability
recommendations
remediation plans
change prioritization
historical change summaries
runtime behavior not represented by present-state static facts
live external environment state
```

