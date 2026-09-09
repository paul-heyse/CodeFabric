---
artifact: design-dossier
design_id: codefabric-real-time-cpg-planning-contracts
version: v2
date: 2026-09-07
status: accepted
baseline_commit: e24627a29ee44bc3f288eeb07197b66e6dec7c7e
primary_scope:
  - src
  - rustc-extractor
  - pyrefly-sidecar
  - codefabric-cpg-mcp
  - third_party/native
  - tooling/native-dependencies
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
predecessor_design_path: docs/designs/codefabric_real_time_cpg_planning_contracts_design_v1_2026-09-04.md
source_review_path: docs/reviews/implementation_status_codefabric_real_time_cpg_implementation_plan_v2_2026-09-04_2026-09-07_v2.md
approval_basis: The user explicitly approved the proposed native-resource amendment and authorized plan revision and resumed execution on 2026-09-07.
---

# Real-time CPG planning contracts

Revised 2026-09-08 under the consolidated pragmatic review. This updates the target, not the installed native source graph. Old packet/proving obligations are historical.

## D-RT02 — Runtime artifacts

Runtime actions retain compact source/context/provider/snapshot identities, coverage/remainder, outcome and diagnostics for explanation and recovery. Detailed intermediate traces are optional and bounded. Source edits use Git and relevant checks; they do not produce source bundles, replay manifests or proof receipts.

## D-RT05 — Resource contract

Share DataFusion memory/spill resources across live work; bound queues, jobs, batches, retained application state and result bytes. Own/join tasks, contain providers, measure RSS/disk headroom, apply backpressure and recover. Do not promise universal allocation pre-admission or immunity from OOM.

## LD-RT09 — Native resource simplification

Replace consumers of generalized native allocation receipts with the reduced D-RT05 contract. Keep useful cancellation, retained lifetime and bounded native configuration behavior. Classify each existing patch before removal; preserve genuine upstream correctness/maintenance fixes. Upstream origins and currently resolved local patches are different facts. Production dependencies and native sources remain unchanged during preparation.

## LD-RT08 — Native maintenance and retention

Delta owns log planning, checkpoint, compaction and vacuum. Protect current/history/leased versions with finite retention and test reopen/recovery. Replace unsafe or missing native maintenance paths at their actual consumers; avoid an application shadow Delta engine. A resource simplification does not implement safe maintenance by implication.

## Other real-time contracts

Preserve source authority, canonical identity, exact snapshots, provider isolation, conservative invalidation, rejection of obsolete completions, two-speed syntax/semantic delivery and all eight query forms from the selected suite. All eventual analyses remain in the production plan. The original v1 design is historical detail; its generalized proof and packet requirements do not return through incorporation.
