# CodeFabric data-fabric target-stack operational handoff

Status: accepted repository migration handoff; rollback window open.

This handoff records the execution judgments required by WP06 of the approved
DataFusion 55, Arrow 59, and delta-rs `43a0cf10` implementation plan. The repository
owner's 2026-08-24 instruction to implement the entire approved plan authorizes this
repository migration handoff. It does not authorize deleting a deployed predecessor
namespace or ending the rollback window.

## Immutable revisions

- Frozen predecessor/WP01 proving commit:
  `3561bf35dc496a6ba5bfdccd72a3feadccdb39a2`.
- Target behavioral, decommission, Gate B, and performance proving commit:
  `f2e6c5256fd1787a3418f946ba7173979808022a`.
- Target stack: Arrow/Parquet 59.2.0, DataFusion 55.0.0, `object_store`
  0.13.2, and delta-rs revision
  `43a0cf10a313e5077c48637ad786a05359136bbb`.

## Preserved namespaces and evidence

- The immutable old-stack persisted-state oracle is
  `tests/fixtures/data_fabric_upgrade/old_stack/delta_table` at the WP01 commit and
  remains tracked at current HEAD. Its manifest and Arrow IPC fixture remain historical
  compatibility evidence.
- Cross-revision target namespaces are created only under validated temporary
  directories by `scripts/data_fabric_revision_check.sh`; the harness never points the
  frozen old binary at target state for writes.
- No production or operator-managed Delta namespace was deployed or mutated during this
  repository migration. Therefore there is no external namespace location to record or
  authorize for deletion. A later deployment must record both predecessor and target
  object-store URIs before its first target write.

## Protocol and maintenance freeze

- Rollback-compatible tables remain at minimum reader version 1 and minimum writer
  version 2 with no reader or writer feature lists.
- Change Data Feed, deletion vectors, type widening, V2 checkpoints, and column mapping
  remain disabled. Reopen validation fails closed if these or another unapproved feature
  appears.
- CodeFabric retains coordinator-owned retries with `with_max_retries(0)`, application
  transactions, predecessor checks, and unknown-outcome reconciliation.
- Vacuum execution over rollback-required files is prohibited. Only dry-run evidence is
  present, and retained-version protection is proved for the rollback fixture.

## Rollback procedure

Before any deployed target write, restore the WP01 code revision and continue serving the
preserved predecessor namespace. After a deployed target write, freeze mutation and vacuum,
quarantine the target namespace without deleting files, and serve the preserved predecessor
namespace. The frozen old binary may read the bounded target fixture for compatibility proof,
but it must never write target-produced state.

The rollback window is **OPEN**. It has no inferred date or elapsed-time expiry. It ends only
after an explicit repository/operator-owner decision that identifies the deployed namespace,
confirms the required retention horizon, and separately authorizes predecessor-file cleanup.

## Certification evidence

- `just data-fabric-stack-compat 3561bf35dc496a6ba5bfdccd72a3feadccdb39a2 f2e6c5256fd1787a3418f946ba7173979808022a`
  passed in both directions, with the reverse direction read-only.
- `just data-fabric-upgrade-bench 3561bf35dc496a6ba5bfdccd72a3feadccdb39a2 f2e6c5256fd1787a3418f946ba7173979808022a`
  passed for activation, first filtered query, warmed filtered and full queries, owner
  replacement/publication, checkpoint reopen, correctness, and peak RSS. Both revisions
  stayed below 80 MiB RSS against the 1 GiB ceiling.
- The Gate B corpus member bytes are unchanged from their owner-authored introduction at
  commit `35fc632`; only the initially incorrect digest metadata and matching KAT were
  corrected to the deterministic `b3:45205c097bae69e22afe344003fd356f9a6311714af015c4fce2521179b07dfd`.
- `data_fabric_old_live_authority_zero_state` and
  `data_fabric_current_reference_routing_contract` prove DB01 and DB02 across tracked live
  scopes with reviewed historical and negative-assurance exclusions.
