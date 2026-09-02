# Relational contract discovery

The v1.3 index enumerated 84 AC-G rows by hand. That table is historical. The
current suite preserves those IDs and dispositions but does not maintain a
second static census.

## 1. Current contract authority

Runtime contract authority is constructed from explicit typed inputs and typed programmatic
transformations, then observed from the installed session:

| Relation | Meaning |
|---|---|
| input.contract | non-derivable stable identity, owner, version, and compatibility commitment |
| contract_consumer | exact consuming relation, compiler, query, policy, proof, or public projection |
| contract_dependency | dependency and closure edge |
| contract_disposition | preserve, supersede, migrate, tombstone, or remove |
| proof_obligation | executable oracle and negative/causal expectation |
| release_decision | accountable acceptance or rejection |
| system.programmatic_relation_observation | relation actually installed in the candidate session |
| system.programmatic_field_observation | fields actually derived from admitted providers/plans |
| system.programmatic_schema_observation | complete observed schema identities and mappings |
| system.programmatic_dependency_observation | installed relation/program dependency closure |
| system.programmatic_provenance_observation | exact input/provider/transformation/release provenance |

Programmatic assembly rejects missing owners, duplicate authority, unresolved dependencies,
incompatible versions, declared schemas that differ from provider/plan-derived schemas, inert
typed inputs, and proof obligations without executable discriminators.

## 2. Historical AC-G identities

AC-G-01 through AC-G-84 remain immutable historical identifiers. Reviewed released commitments
enter as ordinary explicit typed inputs; current dispositions are programmatic relations. A
historical ID may support allocation provenance or evidence without keeping an importer, generated
registry, manifest, replay engine, or code generator alive.

## 3. Discovery

Current contract questions are answered through fixed-point self-description queries over the
installed candidate catalog. Before production activation, the accepted typed inputs and
independently authored expectations are the review boundary. Generated contract files and model
migrations are historical/decommission evidence only and cannot answer current authority.

## 4. Completeness proof

Completeness requires:

- every released ID has one disposition;
- every current contract has one owner;
- every consumer resolves through dependencies;
- every proof obligation has an executable discriminator;
- removed contracts have no selectable live consumer;
- unknown or skipped inventory is a failure, not implicit deletion.

## 5. V2.3 serving contract ownership

The atomic start outcome, input challenge/continuation, public resource handle, reference
completion, cancellation acknowledgement, and modern protocol/catalog contracts enter as released
explicit inputs owned by QRY/SRV. Their live availability, authorization, limits, and component
observations remain derived. The inert framework-owned UI advertisement is recorded separately
from the empty CodeFabric extension registry and empty UI component catalogs; none is semantic or
authorization authority.
