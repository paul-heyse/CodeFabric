# Relational suite invariants and doctrine routing

This is a derived map. The normative doctrine is
docs/library_ref/full_data_fabric_design_principles_v2.md and the current SUITE
global-invariant section.

| Invariant | Primary owner | Executable proof family |
|---|---|---|
| one terminal suite and compiled semantic authority | SUITE, FAB | terminal-chain discovery, exact inputs/transformations, and bootstrap denial |
| source/provider/canonical/derived separation | ONT, GEN | provider authority split and producer closure |
| explicit unknown, conflict, and remainder | ONT, GEN, QRY | capability/coverage and query semantics |
| application-owned identity | ONT, GEN | identity conformance and rebuild equivalence |
| one provider/plan-derived logical/physical SchemaContract | FAB | phase validation plus no declared/model schema authority |
| optimizer-visible native-first planning | FAB, QRY | plan shape and extension conformance |
| sealed bound catalog authority | FAB, QRY | child catalog and bound dependency isolation |
| one immutable pinned programmatic FabricEpoch per query | FAB, LIFE | exact-vector epoch pinning and candidate-free activation recovery |
| phase-typed fresh activation and atomic ActiveWorkspace | FAB, LIFE | empty-head genesis, exact readback, coherent-horizon and restart faults |
| one idempotent durable command path | LIFE | FabricCommand and transaction contract |
| one fenced writer and activation chain | LIFE | writer fence and activation fault matrix |
| one supervisor/daemon plus attach-only presentation | LIFE, SRV | singleton, policy/grant, descriptor, UDS, v2 RPC, and STDIO boundary checks |
| one closed atomic start and daemon-authored guarded input | QRY, SRV | atomic-start, guard roundtrip, tamper/replay/expiry, and no-work-before-acceptance checks |
| daemon public handles and authorized reference completion | QRY, SRV, FAB | per-read/release authorization, restart invalidation, bounded completion, and denied-existence checks |
| modern-only FastMCP 4 application zero state | SUITE, SRV | bridge-off protocol admission, exact catalog, empty application extension/UI component registries, and bounded framework advertisement |
| explicit cancellation and observation recovery | QRY, LIFE, SRV | cancel acknowledgement, terminal observation, reconnect by query/cursor, and no-start-resubmission checks |
| independent execution proof | SUITE | semantic, causal, hostile, public, and recovery oracles |
| complete consumer-first legacy purge | SUITE, RM | inventory, disposition coverage, zero state, clean reconstruction |

## Staticness test

A declaration is static only when the architecture makes it immutable for that
version: released wire identity, suite role, or an accepted historical decision. Exact provider
batches, explicit non-derivable typed inputs, and reviewed typed transformation constructors are
load-bearing inputs; current schemas, dependencies, provenance, capabilities, lifecycle state, and
semantic meaning are provider/plan-derived relations and installed programs, not hand-maintained
duplicate tables.

## Proof rule

Executable beats derived beats recorded. A digest establishes integrity, not
meaning. Absence claims require a complete candidate universe, structural and
textual zero-state evidence, and compiler/type proof where available.
