# Fact-domain map for the relational suite

This is a derived navigation aid. ONT owns vocabulary, GEN owns production,
FAB owns representation/execution, QRY owns request semantics, LIFE owns
freshness, and SRV owns delivery.

## 1. End-to-end relation flow

| Layer | Owned relation classes | Required closure |
|---|---|---|
| source | workspace, source image, semantic environment, source generation | byte identity and requested owner set |
| provider | provider run, raw observation, coverage, remainder, diagnostic, provenance | requested/completed/remainder partition |
| canonical | entity, occurrence, relation, coordinate, type/value, conflict, unknown | modeled authority and normalization |
| analysis | owner-local flow, MIR-derived flow, graph, effect/resource, interprocedural summary | exactly one producer or explicit unsupported remainder |
| epoch | exact input/program/provider/application releases, observed schema contract, table pin, policy, proof, activation, lease | one reconstructible programmatic FabricEpoch |
| request | request form, phrase, projection, bound, ordering, page | closed compositional grammar |
| result | canonical row, evidence, capability, truncation, artifact resource | exact epoch/result contract, daemon public handle, and stable bounded projection |

## 2. Provider authority

Tree-sitter owns recoverable concrete syntax observations. Ruff owns Python
tokens, typed AST, trivia, scopes, and bindings exposed by its pinned crates.
Pyrefly authority is split across its exact Query, TSP/module resolver,
selected Glean/internal, and LSP surfaces. rustc public owns public MIR/item/
instance observations; a narrow private seam owns only stable compiler keys,
source/hygiene, and exact borrow data that public APIs do not expose.

Application analyses are never labeled provider-native.

## 3. Identity and coordinates

Canonical IDs are application-owned fixed-width values. Provider-local IDs,
MIR indices, syntax node IDs, and Pyrefly keys remain provenance. Every source
coordinate is bound to source bytes and an explicit coordinate basis before it
can enter canonical relations.

## 4. Unknown and conflict

Missing output is not none. Every requested family closes through completed
coverage, remainder, diagnostic, conflict, or unknown rows. Query results
propagate that state and never silently filter it into apparent absence.

## 5. Query composition

All eight request forms compile from request relations inside an authorized
child catalog. Public clients cannot name internal tables, functions, SQL, or
plans. Results always identify the pinned epoch and the evidence/capability
state that qualifies them.

## 6. Freshness

FabricCommand turns source/repository changes into invalidation relations and one update wave.
Publication creates exact Delta history versions and immutable Arrow segments. Activation selects
a proved exact relation-root/version vector only after readback. Candidate-free recovery rebuilds
the session from that vector before admission, and a query lease never mixes generations.

## 7. Modern serving projection

QRY preparation may expose typed missing-input requirements without creating accepted work. SRV
maps those requirements through guarded FastMCP request state while the daemon retains continuation
authority. Accepted results remain daemon packages; public handles, completion candidates,
cancellation, and every resource read/release are daemon-authorized projections rather than new
fact, provider, or fabric authority.
