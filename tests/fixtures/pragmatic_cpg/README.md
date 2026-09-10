# Small independent Python/Rust corpus

`workspace/` contains typed declarations, imports, direct calls and indirect uncertainty in both languages. `expectations.json` is derived from reading that source, not captured daemon output. Names identify semantic relationships; actual application IDs may differ but their relationships and source positions must agree. A syntax call is not automatically a resolved semantic call.

`edits.json` covers edit, rename/import repair, deletion, Rust compile failure/repair and Cargo feature-context change. Run it only in a copied temporary workspace. After each change, the real service must report pending/failed scope honestly and eventually compare with a clean rebuild where convergence is possible. Missing imports/compile errors should converge to explicit failure/remainder, never stale-current facts.

`analysis_cases.json` preserves the source examples and target fact/absence expectations from the retired runtime proof program. The Rust test currently exercises real syntax and explicit missing-analysis scope for its branch, loop, return and nested-callable cases. The target semantic assertions still need the outcome 7 analysis adapter; they are not claimed to pass today.

The `first-release-queries` case in [product tooling](../../../tooling/product/README.md) now copies
this workspace and reads `expectations.json` through the installed Python/Rust provider, daemon and
FastMCP stack. All four initial forms, declarations, parameter/return types, imports, resolved and
unknown calls, definition bytes/positions and exact reopen pass (2026-09-10). The test preserves
canonical identities and validates disclosure-bound source handles separately. Optional exhaustion
observations remain optional; source-authored expectations establish the required facts.

The edit corpus and advanced `analysis_cases.json` remain separate acceptance work. Extend them
alongside the production backlog; no automatically accepted snapshots or generated expectations.
