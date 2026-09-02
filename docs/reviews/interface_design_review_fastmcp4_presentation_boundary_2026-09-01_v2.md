---
artifact: interface-design-review
date: 2026-09-01
version: v2
status: complete
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md
serving_specification: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
fastmcp_reference: docs/library_ref/fastmcp_python_advanced_reference_4.0.0.md
amends: docs/reviews/interface_design_review_fastmcp4_presentation_boundary_2026-09-01_v1.md
composes_with: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md
reviewed_head: 6e74cfbbe23da73dd110a2adb232276e00f9a3ad
working_tree: dirty-pre-existing-and-in-progress
baseline: intentionally-not-taken
verdict: aligned
target_status: accepted
---

# FastMCP 4 presentation boundary: framework-owned UI capability amendment

## 0. Decision

Amend the accepted v1 target only where it required a literal empty MCP extension advertisement.
FastMCP 4.0.0 unconditionally advertises the inert library-owned capability
`io.modelcontextprotocol/ui` on the modern protocol, even when the application registers no
`ServerExtension`, provider, transform, App, UI tool, or UI resource. The exact pinned release has
no supported public switch that removes this advertisement.

The target therefore becomes:

- CodeFabric registers no application-owned or custom server extension;
- CodeFabric publishes no App, UI tool, UI resource, Prefab, Generative UI, Tool Search, or Code
  Mode component;
- the only permitted extension advertisement is the exact empty framework-owned
  `io.modelcontextprotocol/ui: {}` capability emitted by an otherwise bare FastMCP 4.0.0 server;
- CodeFabric never consumes that advertisement as authorization, semantic capability, task state,
  workflow state, or evidence that a UI component exists; and
- any other extension identifier, non-empty UI settings, application extension registration, or
  UI component is rejected by the public-surface and zero-state oracles.

All other decisions, ownership boundaries, requirements, legacy dispositions, acceptance
obligations, and replan triggers in v1 remain accepted and unchanged.

## 1. Executable evidence and native-capability judgment

An isolated exact-stack probe used FastMCP 4.0.0, MCP SDK 2.1.1, Pydantic 2.13.4, and Python
3.14.7. A server constructed with `FastMCP("probe", tasks=False)`, with no extensions or
components registered, reported an empty application extension registry while modern live
discovery still returned:

```text
extensions = {"io.modelcontextprotocol/ui": {}}
```

The behavior is produced by
`fastmcp.server.low_level.LowLevelServer.get_capabilities`, which unconditionally merges
`fastmcp.apps.config.UI_EXTENSION_ID` independently of the application extension registry.
`fastmcp inspect` does not expose this modern discovery detail, while a live modern client does.
The implementation must therefore use live `server/discover`/client evidence for the wire
advertisement and a separate application-registry/component observation for CodeFabric-owned zero
state.

The rejected alternatives are:

1. **Unsupported internal override or monkey patch.** This would couple CodeFabric to a private
   FastMCP implementation seam and make the presentation adapter less robust.
2. **A custom proxy or second MCP implementation.** This adds a larger and less tested protocol
   surface only to erase an inert framework capability.
3. **Treating the advertisement as a CodeFabric feature.** Advertisement is not component
   registration, authorization, semantic meaning, or behavioral proof.
4. **Retaining the literal-empty expectation.** That expectation is unexecutable on the pinned
   supported stack and would incentivize a worse architecture.

Using the framework-owned advertisement while proving the application registry and UI component
surface are empty is the highest viable native rung. It preserves P3 (one owner), P14 (native
library leverage), P18 (identity is not correctness), P20 (truthful capability claims), P21
(metadata is not enforcement), P27 (causal declarations), and P36 (executable governance).

## 2. Revised invariants

The following clauses replace only the v1 phrases that said “no extensions” or “no phantom
extension” without distinguishing framework and application ownership:

1. **Application extension zero state.** `FastMCP._extensions` (or the stable public equivalent
   available at implementation time) contains no CodeFabric registration, and no
   `ServerExtension` is constructed or added by CodeFabric.
2. **UI component zero state.** The resolved catalogs contain no App/UI tool, App/UI resource,
   provider, transform, Prefab, or Generative UI component.
3. **Bounded framework advertisement.** Modern discovery contains exactly the empty
   `io.modelcontextprotocol/ui` advertisement supplied by the pinned FastMCP release and no other
   extension entry. Legacy behavior is not a product profile.
4. **No semantic consumption.** No CodeFabric branch, policy, authorization decision, result
   shape, or daemon request depends on the UI advertisement.
5. **Drift fails closed.** A new identifier, non-empty settings, registered handler/component, or
   disappearance that changes the pinned library observation is dependency drift requiring review;
   it is not silently restamped.

## 3. Acceptance consequences

The successor expectation release and implementation oracles must distinguish three observations:

| Observation | Required result | Falsifying fault |
|---|---|---|
| Live modern discovery | exactly `io.modelcontextprotocol/ui: {}` and no other extension advertisement | add another identifier or non-empty UI settings |
| Application extension registry | zero registered CodeFabric/custom extensions | call `add_extension` or construct a CodeFabric `ServerExtension` |
| Resolved component catalogs | no App/UI tools, resources, providers, transforms, or prompts | register any UI/App component or expose a UI resource |

`fastmcp4-modern-protocol-check` owns the live discovery observation.
`fastmcp4-public-surface-check` and `fastmcp4-adapter-authority-zero-state-check` own application
registration and component zero state. `fastmcp4-decommission-zero-state-check` rejects all
predecessor/custom extension code and dependencies. `fastmcp inspect` may remain a catalog/schema
observation, but cannot prove modern extension or completion behavior by itself.

## 4. Replan trigger

Reopen this amendment if the pinned FastMCP release exposes a supported public disable switch, the
advertisement gains non-empty settings or runtime behavior without an application component, a
target host interprets the empty advertisement as a functional UI requirement, or live discovery
cannot distinguish framework advertisement from application registration. Do not reach into
private FastMCP internals merely to restore a cosmetically empty capability map.
