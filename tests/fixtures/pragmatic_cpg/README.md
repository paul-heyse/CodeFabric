# Small independent Python/Rust corpus

`workspace/` contains typed declarations, imports, direct calls and indirect uncertainty in both languages. `expectations.json` is derived from reading that source, not captured daemon output. Names identify semantic relationships; actual application IDs may differ but their relationships and source positions must agree. A syntax call is not automatically a resolved semantic call.

`edits.json` covers edit, rename/import repair, deletion, Rust compile failure/repair and Cargo feature-context change. Run it only in a copied temporary workspace. After each change, the real service must report pending/failed scope honestly and eventually compare with a clean rebuild where convergence is possible. Missing imports/compile errors should converge to explicit failure/remainder, never stale-current facts.

Current harness support and runtime-dependent adapters are described in [product tooling](../../../tooling/product/README.md). These fixtures do not claim the current daemon already provides the expected semantics. Extend them alongside the production backlog; no automatically accepted snapshots or generated expectations.
