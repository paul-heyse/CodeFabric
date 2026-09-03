# CodeFabric operational API (spec section 14).
#
# This file is the first command surface an agent should inspect (spec sections 59 and
# 92). Recipes express intent, not tool flags, so implementations can change without
# invalidating what callers know.
#
# Two rules govern everything below:
#
#   1. Mutating recipes are never dependencies of a validation recipe (spec section
#      14.1). Everything in the [mutating] group must be invoked deliberately, and its
#      diff inspected.
#   2. Availability is not a mandate to run (spec section 73.1). Pick the smallest tool
#      set that answers the risk question in the section 60 change-risk table, then
#      escalate. Running every deep tool after every edit is an anti-pattern.

set shell := ["./scripts/repo-shell.sh"]

# Variadic recipes forward their arguments with "$@" rather than {{ args }}.
# Without this, just re-expands the interpolated string and a quoted argument
# containing a space is silently re-split -- so `just spec-outline <path>
# --match '^5. Authority'` would search for `^5.` and treat `Authority` as a
# second path, returning a wrong outline instead of an error.
set positional-arguments

default:
    @just --list

# ------------------------------------------------------------------ environment

[doc("Environment report: toolchains, versions, required tools, direnv state")]
[group('environment')]
doctor:
    ./scripts/bootstrap.sh

[doc("Idempotently prepare exact tools, the adapter, and supervised compiler cache")]
[group('environment')]
setup: setup-tools setup-adapter setup-sccache

[doc("Check exact workstation CLI and Rustup identities")]
[group('environment')]
tools-doctor:
    ./scripts/tool-versions.sh check

[doc("Reject drift between the tool manifest and operational Rust/CI configuration")]
[group('gate')]
tool-version-contract-check:
    ./scripts/tool-version-contract-check.sh

[doc("Idempotently install the exact workstation CLI and Rustup contract")]
[group('environment')]
setup-tools:
    ./scripts/tool-versions.sh install

[doc("Prove a contaminated caller cannot alter repository Python or Rust resolution")]
[group('environment')]
environment-contract-check:
    ./scripts/environment_contract_check.sh

[doc("Synchronize the locked adapter environment without activating it")]
[group('environment')]
setup-adapter:
    uv sync --frozen --project "$CF_ROOT/codefabric-cpg-mcp"

[doc("Idempotently ensure the mandatory per-user sccache service")]
[group('environment')]
setup-sccache:
    ./scripts/sccache-service.sh install

[doc("Run the routine gate once and cache its verdict under target/")]
[group('environment')]
baseline:
    ./scripts/bootstrap.sh --baseline

[doc("Capture a non-secret tooling inventory to target/tooling-inventory.txt")]
[group('environment')]
inventory:
    ./scripts/tooling_inventory.sh
    @echo "wrote target/tooling-inventory.txt"

[doc("Resolved package metadata, no dependencies")]
[group('environment')]
metadata:
    cargo metadata --locked --format-version 1 --no-deps

# sccache is a committed build prerequisite (.cargo/config.toml). Spec section 13.2:
# measure representative cache effectiveness rather than assuming the wrapper helps.

[doc("Advanced sccache statistics for the supervised CodeFabric service")]
[group('environment')]
cache-stats:
    ./scripts/sccache-service.sh stats

[doc("Validate current sccache version, configuration, endpoint, and storage")]
[group('environment')]
sccache-doctor:
    ./scripts/sccache-service.sh doctor

[doc("Validate sccache service lifecycle, wrapper probes, and Cargo probe-cache recovery")]
[group('environment')]
sccache-service-template-check:
    ./scripts/sccache-service-template-check.sh

[doc("Prove sccache transport/storage and reject cached Cargo wrapper failures")]
[group('environment')]
sccache-canary:
    ./scripts/sccache-service.sh canary

[doc("Restart the supervised per-user sccache service")]
[group('environment')]
sccache-restart:
    ./scripts/sccache-service.sh restart

[doc("Repeatedly measure cold-target/warm-cache client and server modes")]
[group('perf')]
sccache-effectiveness mode="both" *args:
    ./scripts/sccache-effectiveness.sh "{{ mode }}" "$@"

[doc("Build with non-incremental compiler outputs reusable through sccache")]
[group('perf')]
build-shared *args:
    CARGO_INCREMENTAL=0 cargo build --locked "$@"

[doc("Build with rustc incremental compilation and no wrapper for comparison")]
[group('perf')]
build-incremental *args:
    RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --locked "$@"

[doc("Compare the pinned default Linux linker with mold without changing config")]
[group('perf')]
linker-benchmark *args:
    ./scripts/linker-benchmark.sh "$@"

[doc("Zero sccache statistics before a measurement run")]
[group('environment')]
cache-zero-stats:
    ./scripts/sccache-service.sh zero-stats

[doc("Navigate docs/authoritative_design by section without reading whole specs")]
[group('environment')]
spec-outline *args:
    ./scripts/spec-outline.sh "$@"

[doc("Prove the exact eight-master design suite, generated identities, navigation, and sole live authority root")]
[group('contracts')]
authoritative-design-conformance-check:
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest tooling/ci/test_authoritative_design_conformance.py

[doc("Verify released Protobuf outputs, compatibility, and the independent generator identity")]
[group('contracts')]
proto-check:
    ./scripts/proto_dependency_check.sh
    cargo check --locked --no-default-features --features proto-tooling --bin codefabric-proto-gen
    cargo clippy --locked --no-default-features --features proto-tooling --bin codefabric-proto-gen -- -D warnings
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff format --check tooling/proto/generate.py tooling/proto/test_generate.py
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff check tooling/proto/generate.py tooling/proto/test_generate.py
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest tooling/proto/test_generate.py codefabric-cpg-mcp/tests/test_proto.py
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/proto/generate.py check

[doc("Generate released Protobuf bindings twice in isolated roots and compare exact bytes")]
[group('contracts')]
proto-repro-check: proto-check
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/proto/generate.py repro-check

[doc("Reject executable predecessor authority while retaining named historical evidence")]
[group('gate')]
remaining-legacy-zero-state-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest tooling/ci/test_remaining_legacy_zero_state.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/remaining_legacy_zero_state.py

[doc("Reconstruct identical programmatic epochs from activation-selected exact Delta versions")]
[group('gate')]
delta-exact-version-reconstruction-check:
    cargo nextest run --locked --lib -E 'test(/(stable_delta_histories_reopen_exactly_and_expose_only_the_current_epoch|exact_delta_append_readback_and_marker_reconciliation_round_trip)/)' --no-tests=fail

[doc("Prove durable activation append, exact readback, marker recovery, and cold epoch reconstruction")]
[group('gate')]
fabric-activation-recovery-check:
    cargo nextest run --locked --lib -E 'test(/(exact_delta_append_readback_and_marker_reconciliation_round_trip|unknown_commit_recovers_only_from_exact_operation_marker_and_chain|recovery_rejects_a_reversible_version_vector_substitution|exact_delta_evidence_reconstructs_event_and_chain_without_attempt_token)/)' --no-tests=fail

[doc("Prove control recovery derives selection or nonselection from exact Delta evidence without retry")]
[group('gate')]
fabric-control-recovery-check:
    cargo nextest run --locked --lib -E 'test(/(exact_delta_append_readback_and_marker_reconciliation_round_trip|explicit_nonselection_marker_is_the_only_recovery_abort_path|exact_readback_knowledge_cannot_regress_to_marker_nonselection|unknown_marker_keeps_restart_admission_closed_and_requires_reconciliation)/)' --no-tests=fail

[doc("Reject invalid activation forks, predecessors, generations, pins, and reversible version vectors")]
[group('gate')]
activation-chain-validity-check:
    cargo nextest run --locked --lib -E 'test(/(derives_one_head_from_unordered_events|rollback_reselects_an_epoch_without_cycling_event_history|fork_missing_predecessor_and_generation_regression_fail_closed|command_target_proof_and_pin_disagreement_are_rejected|recovery_rejects_a_reversible_version_vector_substitution)/)' --no-tests=fail

[doc("Exercise activation boundary faults and competing writers without promoting partial progress")]
[group('gate')]
activation-fault-matrix-check:
    cargo nextest run --locked --lib -E 'test(/(fault_matrix_never_promotes_partial_progress_to_success|competing_activator_cannot_cross_the_same_admission_barrier|forward_retry_writes_the_active_execution_fence_not_the_admitted_fence)/)' --no-tests=fail

[doc("Hold predecessor leases across activation and keep restart admission closed until reconciliation")]
[group('gate')]
fabric-epoch-pinning-check:
    cargo nextest run --locked --lib -E 'test(/(wp32_ops_gate_leases_pin_predecessor_across_successor_handoff|restart_recovery_keeps_admission_closed_until_marker_and_cache_reconciliation|recovery_requires_the_exact_durable_head)/)' --no-tests=fail

[doc("Navigate docs/library_ref by chapter without reading whole references")]
[group('environment')]
lib-outline *args:
    ./scripts/lib-outline.sh "$@"

# --------------------------------------------------- formatting / static feedback

[doc("Check formatting of stable-domain Rust sources")]
[group('static')]
root-fmt:
    cargo fmt --all -- --check

# The default local profile and the featureless substrate are both load-bearing.

[doc("Type-check default local and featureless stable-domain surfaces")]
[group('static')]
root-check:
    ./scripts/cargo-check-mode.sh cargo check --locked --all-targets
    ./scripts/cargo-check-mode.sh cargo check --locked --all-targets --no-default-features

# Edit-loop check, not a gate. Measured 2026-08-30 against the repository target/, warm,
# after a one-line edit to a private fn: this recipe ~7.5s against `root-check` ~15s
# (n=3). `root-check` costs double because it checks two surfaces. When nothing has
# changed both are ~1s, so the win is only in the edit loop.
#
# Scope is the whole reason: `--all-targets` compiles the library, its test
# configuration, and every current production binary and binary test. Isolated-tree
# control, n=5 hyperfine 1.20.0: `--lib` 6.22s +/- 0.31, `--all-targets` 10.23s +/- 0.96.
#
# The trade is diagnostic reach: this surface does not type-check #[cfg(test)] modules,
# the bins, or tests/integration.rs. `root-check` remains the gate and must still pass
# before a change is done; this only shortens the inner loop.

[doc("Fast library-only type-check for the edit loop -- root-check remains the gate")]
[group('static')]
root-check-fast:
    ./scripts/cargo-check-mode.sh cargo check --locked --lib

[doc("Clippy on default local and featureless stable-domain surfaces")]
[group('static')]
root-clippy:
    ./scripts/cargo-check-mode.sh cargo clippy --locked --all-targets -- -D warnings
    ./scripts/cargo-check-mode.sh cargo clippy --locked --all-targets --no-default-features -- -D warnings

[doc("Spelling and identifier hygiene")]
[group('static')]
typos:
    typos

# ------------------------------------------------------------------------ tests

[doc("Rust tests via nextest (does NOT include doctests)")]
[group('test')]
root-test-rust:
    cargo nextest run --locked

[doc("Focused incremental nextest edit loop -- root-test remains the gate")]
[group('test')]
root-test-incremental *args:
    ./scripts/cargo-check-mode.sh cargo nextest run --locked "$@"

# nextest does not run doctests. This is a separate, mandatory step -- never report "all
# Rust tests passed" from nextest alone (spec sections 18.2 and 62.2).

[doc("Rust doctests -- nextest does not cover these")]
[group('test')]
root-doctest:
    cargo test --locked --doc

[doc("Everything in the stable root: Rust tests and doctests")]
[group('test')]
root-test: root-test-rust root-doctest

[doc("Run the current Wave-2 daemon/control-plane acceptance oracles")]
[group('test')]
wave2-integration-check:
    cargo nextest run --locked -E 'test(/wp1[2-8]/)' --no-tests=fail
    cargo test --locked --doc

[doc("Run the DataFusion 55, Arrow 59, and delta 43a0cf10 behavioral contract")]
[group('test')]
data-fabric-upgrade-check:
    cargo nextest run --locked --test integration -E 'test(/(arrow59_|wp03_|wp05_|wp06_(behavioral|structural|negative)|delta_43a0cf10_|data_fabric_(old_write_new_read|new_write_old_read)_compatibility|data_fabric_(target_stack_release|old_live_authority|current_reference_routing))/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(datafusion_55_|delta_43a0cf10_|wp03_operational|wp05_|wp2[12]_)/)' --no-tests=fail

[doc("Prove effective-relation statistics, pushdown truth, and observed runtime evidence")]
[group('test')]
provider-statistics-contract-check:
    cargo nextest run --locked --lib -E 'test(/(wp58_(behavioral|operational)_acceptance|datafusion_55_effective_provider_statistics_contract|exact_statistics_and_structured_scan_survive_governance_wrapper)/)' --no-tests=fail

[doc("Exercise the fail-closed semantic-provider sandbox escape matrix on the current host")]
[group('test')]
semantic-sandbox-host-matrix-check:
    cargo nextest run --locked --lib -E 'test(/semantic_sandbox_current_host_escape_matrix/)' --no-tests=fail
    @if rg -n 'ObservationMessage|CanonicalFact|encode_selected|with_skip_validation' src/ rustc-extractor/src/; then echo 'provider protocol legacy or unsafe IPC validation bypass remains' >&2; exit 1; fi
    @rg -n 'StreamDecoder::new\(\)\.with_require_alignment\(false\)' src/relation_ipc.rs >/dev/null

[doc("Prove predecessor data-fabric identities survive only in reviewed historical locations")]
[group('static')]
data-fabric-old-authority-check:
    ./scripts/data_fabric_old_authority_check.sh

[doc("Run the complete Wave-4 source/provider/core-fact acceptance slice")]
[group('test')]
wave4-integration-check:
    cargo nextest run --locked -E 'test(/wp(27|28|29|30|31|32|33)/)' --no-tests=fail
    cargo test --locked --doc

[doc("Run the complete Wave-5 query and public-delivery acceptance surface")]
[group('test')]
wave5-integration-check: semantic-request-contract-integrity-check semantic-request-program-check query-unknown-negative-proof-check graph-query-resource-operations-check
    cd rustc-extractor && cargo test --locked wp35
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest codefabric-cpg-mcp/tests

[doc("Prove daemon activation, authenticated UDS reachability, CORE_SOURCE_V1 status, and joined cancellation")]
[group('test')]
query-daemon-activation-check:
    cargo nextest run --locked -E 'test(/wp63_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)/)' --no-tests=fail

[doc("Validate the exact eight-form request contract, application ingress mapping, and public projection")]
[group('test')]
semantic-request-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(released_query_form_vocabulary_is_closed_without_a_generated_registry|released_request_parser_is_authority_neutral_and_canonical|released_request_parser_rejects_unreleased_fields|programmatic_query_contract_identity_is_typed_ordered_and_causal|port_bundle_requires_application_and_every_component_identity)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(all_eight_forms_project_exact_rows_pins_repetitions_and_dependencies)' --no-tests=fail
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest codefabric-cpg-mcp/tests/test_proto.py -k released_query_schema_exposes_exact_eight_forms

[doc("Compile and execute all eight request forms through typed ingress, native programs, authorized children, and the production backend")]
[group('test')]
semantic-request-program-check:
    cargo test --locked --no-default-features --features release-compiler --lib semantic_release::tests::compiled_release_query_program_operations
    cargo test --locked --no-default-features --features semantic-release --lib semantic_release::tests::semantic_release_provider_to_proof_fixture
    cargo test --locked --no-default-features --features semantic-release --lib semantic_release::tests::compiled_release_query_program_executes_datafusion_fixture
    cargo nextest run --locked --lib -E 'test(/(all_eight_released_forms_compile_from_typed_program_rows|epoch_bound_ingress_consumes_every_typed_relation_row_once|epoch_bound_direct_compiler_lowers_exact_programs_returns_and_handoffs)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(all_eight_epoch_bound_forms_execute_through_one_real_authorized_child|relational_program_executes_only_through_authorized_child_inputs)/)' --no-tests=fail

[doc("Validate full-meaning idempotency, bound cursors, manifest-last pages, and exact package reopen")]
[group('test')]
query-result-package-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/wp36_int_/)' --no-tests=fail

[doc("Execute authorized semantic programs as bounded DataFusion streams and independent Arrow pages")]
[group('test')]
scheduled-streamed-semantic-query-check:
    cargo nextest run --locked --lib -E 'test(/wp36_beh_/) | test(all_eight_epoch_bound_forms_execute_through_one_real_authorized_child)' --no-tests=fail

[doc("Reject coordinator, canonicality, capacity, terminal, create-only, and materialization bypasses")]
[group('test')]
query-admission-materialization-bypass-rejection-check:
    cargo nextest run --locked --lib -E 'test(/wp36_neg_/)' --no-tests=fail
    ast-grep test --filter 'production-query-streaming-only|bounded-query-coordinator-only'

[doc("Prove query cancellation cleanup, retention release, expiry, and LOST-on-restart behavior")]
[group('test')]
query-retention-cancellation-restart-check:
    cargo nextest run --locked --lib -E 'test(/wp36_ops_/)' --no-tests=fail

[doc("Prove daemon-rooted cancellation propagation, durable replay, joined cleanup, and task faults")]
[group('test')]
cancellation-tree-check:
    cargo nextest run --locked --lib -E 'test(/(parent_propagates_without_cancelling_a_sibling_from_its_child|structured_task_tree_ownership_integrity|unjoined_task_and_cleanup_reserve_faults|completed_tasks_are_observed_before_capacity_is_reused|cancellation_terminal_and_sibling_semantics|wp45_cancel_is_idempotent_for_queued_running_and_terminal_work|cancellation_restart_drain_operations|wp45_watch_iteration_deadline_releases_admission)/)' --no-tests=fail
    ast-grep test --filter '^cancellation-inward-boundary-only$'
    ast-grep scan --rule rules/cancellation-inward-boundary-only.yml --error src

[doc("Validate the exact production workspace factory, typed inputs, ports, and release pins")]
[group('test')]
production-composition-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(semantic_catalog_authority_rejects_pin_and_program_drift|request_owned_limits_identity_is_complete_and_stable|workspace_public_identity_is_strictly_canonical|port_bundle_requires_application_and_every_component_identity)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(compiled_release_has_one_unsubstitutable_suite_identity|lifecycle_rejects_skips_stale_writers_and_false_ready|empty_workspace_slot_never_falls_back)/)' --no-tests=fail

[doc("Exercise the real typed-input daemon composition and causal query/activation vertical")]
[group('test')]
programmatic-production-composition-check:
    cargo nextest run --locked --lib -E 'test(lifecycle_authority_is_the_only_semantic_admission_gate)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation)' --no-tests=fail

[doc("Reject default, bootstrap, empty, and arbitrary-label production authority")]
[group('test')]
daemon-bootstrap-route-denial-check:
    cargo nextest run --locked --lib -E 'test(/(production_partial_multi_workspace_fencing_releases_every_earlier_owner|lifecycle_rejects_skips_stale_writers_and_false_ready|arbitrary_epoch_labels_cannot_authorize_a_sealed_epoch)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp37_neg_codefabricd_rejects_direct_start_without_supervisor_control)' --no-tests=fail

[doc("Prove bounded cancellation, shutdown, restart, and durable runtime reconstruction")]
[group('test')]
programmatic-runtime-lifecycle-check:
    cargo nextest run --locked --lib -E 'test(/(production_partial_multi_workspace_fencing_releases_every_earlier_owner|cancelled_transaction_never_consumes_epoch_capacity_or_result_lease|ordered_shutdown_closes_every_ingress_clone|shutdown_all_attempts_every_runtime_and_aggregates_in_workspace_order|sqlite_rehydrates_exact_delta_request_and_reconciliation_after_process_reopen)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints)' --no-tests=fail

[doc("Prove released lifecycle identity, Protobuf descriptors, UDS peer policy, deadlines, and frame limits")]
[group('test')]
public-lifecycle-wire-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(public_lifecycle_identity_contract_rejects_substitution_and_duplicate_workspaces|negotiated_query_session_binds_workspace_and_host_profile|programmatic_query_contract_identity_is_typed_ordered_and_causal)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(/(wp10_behavioral_acceptance|missing_or_mismatched_identity_is_rejected_before_handler_dispatch|rust_client_deadline_cancels_a_slow_rpc|rust_client_and_server_apply_symmetric_four_mib_limits)/)' --no-tests=fail
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest codefabric-cpg-mcp/tests/test_proto.py codefabric-cpg-mcp/tests/test_settings.py -k 'wp10 or generated_descriptors or python_channel or daemon_scheme'

[doc("Exercise the target production binary lifecycle without predecessor serving composition")]
[group('test')]
lifecycle-production-vertical-check:
    cargo nextest run --locked --test integration -E 'test(/(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation|wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints|wp37_neg_codefabricd_rejects_direct_start_without_supervisor_control)/)' --no-tests=fail

[doc("Reject predecessor physical/config/role revival and supervisor or activation substitution")]
[group('test')]
predecessor-restart-revocation-check:
    cargo nextest run --locked --lib -E 'test(/(operational_receipt_nonauthority_faults|clean_restart_rejects_wrong_epoch_and_keeps_admission_closed|restart_recovery_keeps_admission_closed_until_marker_and_cache_reconciliation)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp63_ops_installed_restart_reconstructs_only_exact_activation_authority)' --no-tests=fail

[doc("Reconcile every interrupted cutover edge from exact command, Delta, and supervisor readback")]
[group('test')]
unknown-cutover-reconciliation-check:
    cargo nextest run --locked --lib -E 'test(/(unknown_commit_recovers_only_from_exact_operation_marker_and_chain|commit_persists_the_exact_unknown_ticket_without_recovery|reconcile_uses_marker_recovery_only_and_preserves_evidence)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback)' --no-tests=fail

[doc("Prove retired bootstrap, model, ontology, dual-epoch, and candidate authority stays absent")]
[group('test')]
bootstrap-model-decommission-integrity-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/wp30_authority_zero_state.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_wp30_authority_zero_state.py -k 'int_'
    ./scripts/model_zero_state_check.sh

[doc("Prove the compiled release reaches honest target lifecycle states without predecessor inputs")]
[group('test')]
compiled-release-consumer-cutover-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/wp30_authority_zero_state.py
    cargo nextest run --locked --lib -E 'test(compiled_release_has_one_unsubstitutable_suite_identity)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation)' --no-tests=fail

[doc("Reject every live legacy path, symbol, feature, target, package, recipe, and selector")]
[group('test')]
bootstrap-ontology-authority-zero-state-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/wp30_authority_zero_state.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_wp30_authority_zero_state.py -k 'neg_'
    ./scripts/model_zero_state_check.sh

[doc("Rebuild, package, drain, stop, and restart the target binary without model artifacts")]
[group('test')]
programmatic-model-free-restart-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/wp30_authority_zero_state.py
    cargo nextest run --locked --lib -E 'test(sqlite_rehydrates_exact_delta_request_and_reconciliation_after_process_reopen)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints)' --no-tests=fail

[doc("Validate the plan-derived schema, transformation, native-rung, and observation contract matrix")]
[group('test')]
datafusion-contract-matrix-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(constructs_qualified_schema_and_round_trips_index_mappings|direct_observed_schemas_bind_policy_casts_and_public_phase_output|transformation_contract_metadata_is_typed_and_queryable|programmatic_bindings_compile_from_live_schema_contracts_without_model_rows|recursive_plan_remains_optimizer_visible_and_has_no_extension|observation_fixed_point_policy_rejects_every_zero_bound|recursion_policy_fails_closed_without_a_native_iteration_cap|determinism_policy_rejects_nonimmutable_and_inert_volatility|schema_contract_cache_identity_frames_typed_policy_without_debug_text|compiled_relation_census_and_schema_contracts_are_exhaustive|provider_descriptor_rejects_an_unclassified_field|typed_identity_framing_distinguishes_field_boundaries|released_request_parser_is_authority_neutral_and_canonical|recipe_composes_compiled_v2_ports_with_shared_scope_authority|release_owned_compiler_resolves_exact_epoch_relations_and_fields)/)' --no-tests=fail

[doc("Execute the isolated generic Arrow, DataFusion, proof, and exact-Delta fabric")]
[group('test')]
data-fabric-core-check:
    cargo nextest run --locked --no-default-features --features data-fabric --lib -E 'test(/(synthetic_arrow_fabric_end_to_end|provider_plan_schema_observations_and_query_share_one_sealed_session)/)' --no-tests=fail

[doc("Reject loss of structured scan arguments or transparent physical-plan properties")]
[group('test')]
datafusion-scan-contract-check:
    cargo nextest run --locked --lib -E 'test(/(forwards_complete_structured_scan_and_preserves_native_plan|legacy_scan_also_uses_structured_path_and_delegates_truthful_metadata|datafusion_scan_and_property_loss_faults)/)' --no-tests=fail

[doc("Prove plan-derived schemas, fixed-point observations, child views, and shared logical reuse")]
[group('test')]
datafusion-plan-schema-cache-check:
    cargo nextest run --locked --lib -E 'test(/(provider_filter_projection_derives_and_registers_its_schema|provider_plan_schema_observations_and_query_share_one_sealed_session|fully_granted_view_graph_is_rebuilt_against_child_providers|shared_cache_tracks_concrete_registry_authority_not_only_opaque_pins|compiles_projection_filter_aggregate_sort_and_limit_to_native_nodes|native_view_control_loses_relation_metadata_and_identity_boundary_restores_it|compiled_release_builds_all_eight_epoch_checked_programs|all_eight_forms_project_exact_rows_pins_repetitions_and_dependencies)/)' --no-tests=fail

[doc("Reject caller semantic substitution, schema drift, dependency leaks, and cache-selected authority")]
[group('test')]
caller-defined-semantic-authority-denial-check:
    cargo nextest run --locked --lib -E 'test(/(output_schema_declaration_is_assertion_only_and_mismatch_fails|unresolved_and_cyclic_transformations_fail_before_plan_building|granted_view_graph_rejects_a_transitively_denied_parent_provider|unresolved_relation_and_function_name_authority_fail_closed|logical_plan_cache_rejects_materialization_collision_for_complete_key|logical_plan_cache_does_not_treat_equal_renderings_as_equal_capabilities|observation_fixed_point_iteration_bound_fails_closed|observation_relation_row_bound_fails_closed_before_sealing|observation_relation_memory_bound_fails_closed_before_sealing|observation_total_row_bound_fails_closed_before_sealing|observation_total_memory_bound_fails_closed_before_sealing|cached_plan_reference_validation_rejects_opaque_extension_capabilities|compiled_provider_authority_denies_caller_authored_provider_admission|provider_descriptor_rejects_a_cross_relation_known_field_name|operational_inputs_cannot_substitute_release_owned_forms_or_scopes|compiled_constructor_is_v2_only_and_has_no_caller_mapping_parameters|compiled_v2_scope_authorization_rejects_omitted_or_changed_causal_operands|released_request_parser_rejects_v1_3_instead_of_translating_legacy_globals|compiled_operand_and_epoch_schema_mutations_fail_closed|non_closed_executed_row_is_rejected_as_a_closure_violation|port_bundle_requires_application_and_every_component_identity|compiled_transformation_authority_is_required_and_composition_input_is_not_public|release_owned_compiler_rejects_alternate_missing_and_drifted_inputs)/)' --no-tests=fail

[doc("Exercise entry bounds, deterministic cache-pressure accounting, TTL refresh, resources, and fresh execution")]
[group('test')]
datafusion-cache-resource-operations-check:
    cargo nextest run --locked --lib -E 'test(/(datafusion_stream_cache_resource_operations|object_list_ttl_is_a_refresh_bound_not_validity_or_authority|proof_execution_enforces_row_and_memory_resource_bounds|relational_program_executes_only_through_authorized_child_inputs|logical_plan_cache_bypasses_oversized_entries_without_changing_semantics)/)' --no-tests=fail

[doc("Prove Delta history creation, transaction identity, exact proof rows, and activation readback integrity")]
[group('test')]
delta-durability-protocol-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(wp32_int_|exact_delta_activation_authority_split)/)' --no-tests=fail

[doc("Prove partial-publication invisibility, exact activation, pinned reopen, competing-writer rejection, and unknown-outcome reconciliation")]
[group('test')]
delta-publication-contract-check:
    cargo nextest run --locked --lib -E 'test(/(fault_matrix_never_promotes_partial_progress_to_success|exact_delta_activation_authority_split|semantic_read_stays_on_an_older_exact_version_after_a_newer_commit_exists|restart_readback_uses_the_loaded_exact_commit_after_the_head_advances|concurrent_advance_is_a_typed_conflict_and_never_retries|conflict_and_unknown_outcomes_both_require_durable_reconciliation)/)' --no-tests=fail

[doc("Reconstruct the activation-selected exact Delta versions and decoded rows, including an older selected version")]
[group('test')]
delta-exact-reconstruction-v4-check:
    cargo nextest run --locked --lib -E 'test(/wp32_beh_/)' --no-tests=fail

[doc("Reconstruct exact semantic, CDF, checkpoint, and nine-relation proof state from pinned Delta versions")]
[group('test')]
delta-exact-reconstruction-v3-check:
    cargo nextest run --locked --lib -E 'test(/(exact_process_reopen_restores_all_nine_relations|snapshot_recipe_reconstructs_the_exact_pin_without_a_version_selector|exact_provider_read_retains_full_stats_and_marks_missing_values_unknown|exact_cdf_range_has_an_inclusive_end_and_excludes_a_newer_head|reconstructed_checkpoint_restarts_at_the_next_exact_version)/)' --no-tests=fail

[doc("Reject receipt, cache, predecessor-schema, and reversible-vector substitution as activation authority")]
[group('test')]
activation-receipt-nonauthority-check:
    cargo nextest run --locked --lib -E 'test(/(wp32_neg_|operational_receipt_nonauthority_faults)/)' --no-tests=fail

[doc("Recover admission-closed from durable Delta evidence without a process-local candidate")]
[group('test')]
candidate-free-recovery-check:
    cargo nextest run --locked --lib -E 'test(/(wp32_ops_|state_capability_reconstruction_operations)/)' --no-tests=fail

[doc("Compile-probe typed launch settings, authenticated control, private sockets, and replacement-safe cleanup")]
[group('test')]
supervisor-launch-platform-check:
    cargo nextest run --locked --lib -E 'test(/wp44_(int|beh|neg|ops)_/)' --no-tests=fail

[doc("Prove strict startup, policy, and binary-to-library process contracts")]
[group('test')]
fastmcp4-startup-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(wp44_int_)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(/(wp44_int_thin_binaries_delegate_to_strict_library_settings|wp44_beh_real_project_venv_launch_preserves_distribution_authority)/)' --no-tests=fail

[doc("Reach Ready from no activation head only through exact durable command readback")]
[group('test')]
fresh-activation-ready-reconciliation-check:
    cargo nextest run --locked --test integration -E 'test(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation)' --no-tests=fail

[doc("Reject direct daemon start, unsafe endpoints, claim expansion, and unauthenticated control")]
[group('test')]
supervisor-startup-boundary-rejection-check:
    cargo nextest run --locked --lib -E 'test(wp44_neg_)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(wp37_neg_codefabricd_rejects_direct_start_without_supervisor_control)' --no-tests=fail

[doc("Restart the real daemon and prove drain, join, and exact endpoint cleanup")]
[group('test')]
supervisor-restart-join-operations-check:
    cargo nextest run --locked --lib -E 'test(wp44_ops_)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(/(wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints|wp44_ops_real_signal_orders_drain_before_shutdown_and_joins_owned_endpoints|wp44_ops_real_pre_ready_child_failure_keeps_admission_closed_and_cleans_endpoints|wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback|wp44_ops_real_launch_capacity_recovers_pending_failure_and_abrupt_exit)/)' --no-tests=fail

# Every WP45 packet oracle crosses the generated Rust and Python clients through
# the real Tonic service over a private UDS. Direct service/unit tests below are
# deliberately supplemental to this shared interoperability boundary.
_wp45-generated-client-uds-interop:
    cargo nextest run --locked --test integration -E 'test(/wp45_rust_tonic_uds_accepts_generated_(rust|python)_client/)' --run-ignored all --no-tests=fail

[doc("Validate the atomic v2 source, descriptor, generated clients, and pure preparation boundary")]
[group('test')]
fastmcp4-daemon-wire-contract-check: proto-check proto-repro-check _wp45-generated-client-uds-interop
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_proto.py -k 'v2_atomic_start_and_event_contracts_are_closed or atomic_start_oneofs_replace_previous_variants or independent_atomic_start_wire_fixtures_decode_exact_outcomes'
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_daemon_client.py -k 'rejects_additive_status_and_unsafe_diagnostic_reference or rejects_unallowlisted_progress_stage'

[doc("Prove one atomic start outcome across bounded multi-round guarded input")]
[group('test')]
fastmcp4-atomic-start-check: _wp45-generated-client-uds-interop
    cargo nextest run --locked --lib -E 'test(/(wp45_explicit_validation_is_pure_and_creates_no_start_authority|wp45_start_replay_precedes_changed_catalog_preparation|wp45_atomic_task_slot_reservation_rejects_concurrent_overacceptance|wp36_int_full_operation_idempotency_cursor_and_journal_bindings_are_exact)/)' --no-tests=fail
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_proto.py -k 'v2_atomic_start_and_event_contracts_are_closed or atomic_start_oneofs_replace_previous_variants or independent_atomic_start_wire_fixtures_decode_exact_outcomes'

[doc("Reauthorize daemon-minted public resource handles and reference projections on every use")]
[group('test')]
fastmcp4-resource-authority-check: _wp45-generated-client-uds-interop
    cargo nextest run --locked --lib -E 'test(/streamed_result_registry::tests::(reference_handle_reauthorizes_every_binding_and_releases_without_query|reference_registry_capacity_is_strict_and_publish_preserves_live_tombstones|wp45_resource_scoped_manifest_pages_release_only_after_last_handle|wp45_restart_reissues_fresh_handles_from_the_durable_package_locator|wp45_restart_cleanup_uses_only_pre_result_ready_object_intent_and_retries|wp45_expiry_reclaims_handles_and_retries_failed_object_cleanup)/)' --no-tests=fail

[doc("Prove cancellation, restart reissue, expiry cleanup, and reserved control capacity")]
[group('test')]
fastmcp4-daemon-security-recovery-check: _wp45-generated-client-uds-interop
    cargo nextest run --locked --lib -E 'test(/wp45_(int_exact_publication_intent_precedes_every_object_write|cancel_is_idempotent_for_queued_running_and_terminal_work|neg_policy_revocation_and_session_sharing_bind_authorization_and_cursor|ops_restart_after_cancellation_side_effect_before_ack_reports_replay|neg_durable_cancellation_identity_ledger_is_strictly_bounded|restart_after_publication_intent_recovers_every_pre_result_ready_kill_point|restart_after_result_ready_before_terminal_preserves_exact_cleanup_locator|restart_retains_locator_and_release_denies_reissue_durably|restart_reissues_fresh_handles_from_the_durable_package_locator|restart_cleanup_uses_only_pre_result_ready_object_intent_and_retries|expiry_stages_durable_cleanup_before_deleting_the_query_tombstone|expiry_reclaims_handles_and_retries_failed_object_cleanup|lazy_service_retention_reclaims_coordinator_authority|ready_admission_linearizes_acceptance_before_drain|reserved_control_keeps_cancel_and_release_admissible|relative_budget_is_consumed_across_the_operation_lifetime|queued_execution_deadline_releases_reservations|watch_iteration_deadline_releases_admission|read_iteration_deadline_releases_admission|challenge_expiry_remains_typed_after_unrelated_prune|challenge_expiry_and_replay_are_distinct_typed_outcomes|guard_invalid_answer_closes_and_replays_stable_rejection|challenge_output_is_bounded_after_typed_projection|beh_private_journal_events_do_not_create_public_sequence_gaps|safe_metadata_and_keys_are_typed_and_control_character_free)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(missing_or_mismatched_identity_is_rejected_before_handler_dispatch)' --no-tests=fail
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_daemon_client.py::test_watch_reconnects_with_fresh_session_without_resubmitting_start codefabric-cpg-mcp/tests/test_daemon_client.py::test_watch_reconnect_rejects_changed_replayed_result_without_resubmitting_start codefabric-cpg-mcp/tests/test_daemon_client.py::test_watch_reconnect_settings_renewal_obeys_one_lifetime_budget_without_resubmitting_start

[doc("Round-trip all bounded guarded-input outcomes through generated v2 clients")]
[group('test')]
fastmcp4-guard-roundtrip-check: _wp45-generated-client-uds-interop
    cargo nextest run --locked --lib -E 'test(/wp45_(guard_three_round_ledger_replays_each_truthful_outcome|guard_invalid_answer_closes_and_replays_stable_rejection|challenge_expiry_and_replay_are_distinct_typed_outcomes|challenge_output_is_bounded_after_typed_projection|programmatic_guard_carries_live_catalog_execution_value)/)' --no-tests=fail
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_proto.py::test_challenge_contract_is_typed_bounded_and_contains_no_opaque_json codefabric-cpg-mcp/tests/test_daemon_client.py::test_daemon_port_maps_closed_v2_contract_without_python_authority

[doc("Project only authorized bounded completion values and hide denied existence")]
[group('test')]
fastmcp4-completion-authorization-check: _wp45-generated-client-uds-interop
    cargo nextest run --locked --lib -E 'test(/(wp45_live_reference_completion_caps_total_and_filters_the_shared_relation|wp45_reference_denials_do_not_disclose_selector_existence|reference_workspace_scope_requires_exactly_one_authorized_workspace|streamed_result_registry::tests::reference_)/)' --no-tests=fail

[doc("Pin the exact FastMCP 4 stack, public imports, and bridge-off dependency contract")]
[group('adapter')]
fastmcp4-dependency-contract-check: adapter-lint
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python -c 'import tomllib; from importlib.metadata import version; from pathlib import Path; expected=["blake3==1.0.9","fastmcp==4.0.0","grpcio==1.83.0","mcp==2.1.1","opentelemetry-api==1.44.0","protobuf==7.36.0","pydantic==2.13.4","rfc8785==0.1.4"]; actual=tomllib.loads(Path("codefabric-cpg-mcp/pyproject.toml").read_text())["project"]["dependencies"]; assert actual == expected, actual; assert (version("fastmcp"),version("mcp"),version("pydantic"),version("grpcio"),version("protobuf"),version("opentelemetry-api")) == ("4.0.0","2.1.1","2.13.4","1.83.0","7.36.0","1.44.0")'
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_adapter_contracts.py codefabric-cpg-mcp/tests/test_settings.py codefabric-cpg-mcp/tests/test_proto.py

[doc("Serve only protocol 2026-07-28 and reject every legacy era before daemon dispatch")]
[group('adapter')]
fastmcp4-modern-protocol-check:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_server.py -k 'legacy_initialize_is_rejected_before_business_dispatch or guard_roundtrip_reenters_with_new_leg_correlation_and_daemon_state or atomic_query_publishes_scoped_resources_and_releases_out_of_order_reads or status_reference_resource_and_completion_delegate_to_daemon or replacement_generation_supplies_live_timeouts_and_resource_bounds or resource_unit_may_span_multiple_negotiated_chunks or allowlisted_spans_exclude_handles_payload_answers_and_exception_prose or unexpected_daemon_failure_is_safe_on_wire_and_stderr or valid_guard_state_is_bound_to_the_original_tool_arguments or hostile_request_id_is_not_used_for_spans_or_daemon_correlation'
    just adapter-stdio-test

[doc("Prove the adapter retains no semantic, session, task, cache, Arrow, or resource authority")]
[group('adapter')]
fastmcp4-adapter-authority-zero-state-check:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_daemon_client.py codefabric-cpg-mcp/tests/test_server.py -k 'source_has_no_displaced_authority or maps_closed_v2_contract_without_python_authority or tampered_guard_state_fails_before_daemon_continuation or incomplete_resource_read_never_releases_its_handle or host_cancellation_uses_daemon_query_identity_and_reraises or rejects_additive_status_and_unsafe_diagnostic_reference or rejects_unallowlisted_progress_stage or allowlisted_spans_exclude_handles_payload_answers_and_exception_prose or unexpected_daemon_failure_is_safe_on_wire_and_stderr or rejects_malformed_input_requirements or valid_guard_state_is_bound_to_the_original_tool_arguments or hostile_request_id_is_not_used_for_spans_or_daemon_correlation'
    @if rg -n -g '*.py' 'fastmcp_slim|fastmcp\.tasks|pydantic_settings|pyarrow|polars|ResourceLease|_resource_leases|mcp_call_id|rpc_attempt_id' codefabric-cpg-mcp/src; then echo 'retired adapter authority/import surface remains live' >&2; exit 1; fi

[doc("Inspect the exact installed FastMCP tool, resource, completion, prompt, task, and extension surface")]
[group('adapter')]
fastmcp4-public-surface-check: adapter-wheel-test
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q codefabric-cpg-mcp/tests/test_server.py::test_fastmcp_registers_exact_modern_target_surface codefabric-cpg-mcp/tests/test_adapter_contracts.py::test_reference_projects_only_a_public_daemon_resource codefabric-cpg-mcp/tests/test_server.py::test_status_reference_resource_and_completion_delegate_to_daemon
    just adapter-stdio-test

[doc("Observe the fixed modern FastMCP contract through a freshly installed wheel and real supervisor topology")]
[group('test')]
fastmcp4-contract-observation-check:
    cargo nextest run --locked --test integration -E 'test(wp47_int_real_installed_wheel_modern_contract_observation)' --no-tests=fail

[doc("Execute guarded query, resource, reference, and completion behavior through installed FastMCP 4")]
[group('test')]
fastmcp4-stdio-vertical-check:
    cargo nextest run --locked --test integration -E 'test(wp47_beh_real_installed_wheel_guard_query_resource_and_completion)' --no-tests=fail

[doc("Reject legacy framing, cross-agent handles, and secret projection in the installed topology")]
[group('test')]
fastmcp4-security-negative-check:
    cargo nextest run --locked --test integration -E 'test(wp47_neg_real_agent_scope_legacy_framing_and_secret_denial)' --no-tests=fail

[doc("Prove cancellation, restart reconnection, and two-agent isolation through installed FastMCP 4")]
[group('test')]
fastmcp4-cancellation-recovery-check:
    cargo nextest run --locked --test integration -E 'test(wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation)' --no-tests=fail

[doc("Validate complete live-surface coverage and the exact retained FastMCP 4 target")]
[group('gate')]
fastmcp4-post-purge-surface-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fastmcp4_post_purge_assurance.py -k 'int_'
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_post_purge_assurance.py surface

[doc("Reject every displaced compiled-release, marker, profile, feature, and v5 live-tooling class")]
[group('gate')]
compiled-release-legacy-zero-state-check: feature-architecture-check provider-type-boundary-check generated-type-boundary-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_compiled_release_zero_state.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/compiled_release_zero_state.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fresh_activation_assurance.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fresh_activation_assurance.py

[doc("Rerun representative daemon, guard, resource, completion, cancellation, and installed-adapter behavior after purge")]
[group('test')]
fastmcp4-retained-target-behavior-check: fastmcp4-atomic-start-check fastmcp4-guard-roundtrip-check fastmcp4-resource-authority-check fastmcp4-completion-authorization-check fastmcp4-modern-protocol-check fastmcp4-contract-observation-check fastmcp4-stdio-vertical-check fastmcp4-security-negative-check fastmcp4-cancellation-recovery-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fastmcp4_post_purge_assurance.py -k 'beh_'
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_post_purge_assurance.py retained

[doc("Reject every displaced serving, adapter-authority, schema, recipe, package, and evidence surface")]
[group('gate')]
fastmcp4-decommission-zero-state-check: remaining-legacy-zero-state-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fastmcp4_post_purge_assurance.py -k 'neg_'
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_post_purge_assurance.py zero-state

[doc("Build and inventory all retained domains, features, generated bindings, wheels, entry points, recipes, and services")]
[group('gate')]
fastmcp4-package-build-check: root-check extractor-check sidecar-check adapter-wheel-test stable-graph-check features-each proto-repro-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fastmcp4_post_purge_assurance.py -k 'ops_'
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_post_purge_assurance.py package

[doc("Prove the installed target uses one compiled release and exact provider/fabric authority")]
[group('test')]
installed_target_authority_integrity: compiled-release-legacy-zero-state-check provider-trust-coverage-remainder-check
    cargo nextest run --locked --test integration -E 'test(/(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation|wp47_int_real_installed_wheel_modern_contract_observation)/)' --no-tests=fail

[doc("Prove a pre-registered source mutation changes only its dependent installed FastMCP result")]
[group('test')]
real_source_to_fastmcp_causal_vertical:
    cargo nextest run --locked --test integration -E 'test(wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent)' --no-tests=fail

[doc("Execute the installed source-to-provider-to-Delta-to-UDS-to-FastMCP causal vertical")]
[group('test')]
semantic-release-vertical-check: installed_target_authority_integrity real_source_to_fastmcp_causal_vertical

[doc("Discard process-local state and reconstruct only the exact activation-selected release")]
[group('test')]
semantic-release-restart-reconstruction-check: candidate-free-recovery-check predecessor-restart-revocation-check
    cargo nextest run --locked --test integration -E 'test(/(wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback|wp63_ops_installed_restart_reconstructs_only_exact_activation_authority)/)' --no-tests=fail

[doc("Fault provider, IPC, proof, Delta, authorization, and clean reconstruction boundaries")]
[group('test')]
installed_vertical_fault_and_recovery_matrix: provider-admission-exclusivity-check provider-ipc-contract-integrity-check delta-publication-contract-check fastmcp4-security-negative-check semantic-release-restart-reconstruction-check

[doc("Prove pull-driven UDS streams stay bounded, cancellable, ordered, retained, and joined")]
[group('test')]
slow_consumer_cancel_restart_operations: fastmcp4-cancellation-recovery-check
    cargo nextest run --locked --test integration -E 'test(wp63_ops_generated_uds_slow_consumers_remain_bounded_and_cancellable)' --no-tests=fail

[doc("Exercise generated UDS clients while event and resource consumers remain unread")]
[group('test')]
grpc-slow-consumer-check: slow_consumer_cancel_restart_operations

[doc("Read supported host deployment surfaces and fail if a product predecessor exists")]
[group('gate')]
deployment-predecessor-census-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fresh_activation_assurance.py

[doc("Prove one exact evolving Delta authority is the sole FreshActivation authority")]
[group('gate')]
fresh_activation_sole_authority_integrity: deployment-predecessor-census-check compiled-release-legacy-zero-state-check
    cargo nextest run --locked --lib -E 'test(exact_delta_activation_authority_split)' --no-tests=fail

[doc("Create an empty production root and serve its exact genesis through installed FastMCP")]
[group('test')]
empty_root_fresh_activation_semantics:
    cargo nextest run --locked --test integration -E 'test(/(wp44_beh_real_supervisor_ready_requires_durable_fresh_activation|wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent)/)' --no-tests=fail

[doc("Reject predecessor state, seed, receipt, hash, latest, service, and process routes")]
[group('gate')]
predecessor_seed_hash_latest_route_faults: activation-receipt-nonauthority-check predecessor-restart-revocation-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fresh_activation_assurance.py -k 'neg_'

[doc("Prove unknown append, readback, restart, and exact forward-repair operations")]
[group('test')]
fresh_activation_reconciliation_operations: fabric-activation-recovery-check fabric-control-recovery-check activation-fault-matrix-check fabric-epoch-pinning-check unknown-cutover-reconciliation-check candidate-free-recovery-check supervisor-restart-join-operations-check
    cargo nextest run --locked --test integration -E 'test(/(wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback|wp63_ops_installed_restart_reconstructs_only_exact_activation_authority)/)' --no-tests=fail

[doc("Execute the target-only empty-root FreshActivation, restart, fault, census, and zero-state matrix")]
[group('gate')]
compiled-release-fresh-activation-check: fresh_activation_sole_authority_integrity empty_root_fresh_activation_semantics predecessor_seed_hash_latest_route_faults fresh_activation_reconciliation_operations

# WP65's measurement recipes are intentionally private implementation details of the frozen
# method. Each selects one real product scenario and emits no synthetic candidate observation.
_wp65-measure-release-compile target:
    cargo build --locked --release --target-dir "{{target}}" --bin codefabric --bin codefabricd

_wp65-measure-fresh-activation:
    cargo test --locked --test integration integration::daemon::wp44_beh_real_supervisor_ready_requires_durable_fresh_activation -- --exact --nocapture

_wp65-measure-retained-reconstruction:
    cargo test --locked --test integration integration::daemon::wp63_ops_installed_restart_reconstructs_only_exact_activation_authority -- --exact --nocapture

_wp65-measure-inprocess-providers:
    cargo test --locked --lib provider_native_syntax::job_tests::wp65_measure_inprocess_tree_sitter_ruff -- --exact --nocapture

_wp65-measure-pyrefly:
    cd pyrefly-sidecar && cargo test --locked pyrefly_link::tests::wp65_measure_retained_pyrefly -- --exact --nocapture

_wp65-measure-rustc-provider:
    cargo test --locked --lib rustc_service::tests::rustc_provider_process_lifecycle -- --exact --nocapture

_wp65-measure-datafusion:
    cargo test --locked --lib fabric::child_session::tests::wp65_measure_datafusion_stream -- --exact --nocapture

_wp65-measure-delta:
    cargo test --locked --lib fabric::activation_control_delta::tests::exact_delta_activation_authority_split -- --exact --nocapture

_wp65-measure-grpc:
    cargo test --locked --test integration integration::rpc::wp63_ops_generated_uds_slow_consumers_remain_bounded_and_cancellable -- --exact --nocapture

_wp65-measure-installed-fastmcp:
    cargo test --locked --test integration integration::daemon::wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent -- --exact --nocapture

[doc("Validate the pre-registered target-only WP65 method and reject expectation drift")]
[group('gate')]
fastmcp4-expectation-drift-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_release_performance.py method-integrity
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_fastmcp4_release_performance.py

[doc("Validate raw final-topology samples, structural runs, bounds, review, and git lineage")]
[group('test')]
compiled_release_resource_performance_envelope:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_release_performance.py method-integrity
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_release_performance.py validate

[doc("Certify the frozen target-only WP65 resource and performance envelope")]
[group('gate')]
compiled-release-resource-performance-check: fastmcp4-expectation-drift-check compiled_release_resource_performance_envelope

[doc("Execute application-owned provider job, result, coverage, gap, resource, and admission contracts")]
[group('test')]
provider-job-contract-check:
    cargo test --locked --no-default-features --features provider-contracts --lib provider_contracts::tests
    cargo test --locked --no-default-features --features release-compiler --lib semantic_release::tests::release_provider_preparation_and_admission_are_causal

[doc("Compile every behavior-bearing release program and execute independent operand faults")]
[group('test')]
release-program-contract-check:
    cargo test --locked --no-default-features --features release-compiler --lib semantic_release::tests
    cargo test --locked --lib current_v23_release_compiles_all_exact_provider_relation_schemas

[doc("Reject stale suite identity, categorical conflation, and forged release jobs")]
[group('test')]
compiled-suite-identity-check:
    cargo test --locked --no-default-features --features release-compiler --lib semantic_release::tests::compiled_release_program_identity_integrity
    cargo test --locked --no-default-features --features release-compiler --lib semantic_release::tests::compiled_release_forgery_and_conflation_faults

[doc("Validate one or all independently useful target features and forbidden dependency edges")]
[group('gate')]
feature-architecture-check scope="all":
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/feature_architecture.py "{{scope}}"
    @if [ "{{scope}}" = "state" ]; then scopes='repository-input operational-state'; elif [ "{{scope}}" = "all" ]; then scopes='provider-contracts release-compiler fact-generation data-fabric repository-input operational-state semantic-release daemon cancellation'; else scopes='{{scope}}'; fi; for scope in $scopes; do if [ "$scope" = cancellation ]; then feature=daemon; else feature="$scope"; fi; ast-grep test --filter "^${scope}-inward-boundary-only$"; cargo check --locked --no-default-features --features "$feature"; done

[doc("Validate exact Arrow IPC identities, schemas, pinned provider batches, and cross-process control contracts")]
[group('test')]
provider-ipc-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/(round_trip_keeps_one_schema_and_dictionary_scope|registration_binds_schema_fingerprint_and_exact_arrow_universe|production_provider_schemas_interoperate_with_the_relation_stream_boundary|workspace_transaction_aggregates_all_four_exact_provider_lanes|observation_service_rejects_mismatched_launch_plan_binding_before_listen)/)' --no-tests=fail
    cd pyrefly-sidecar && cargo test --locked pyrefly_protocol_conformance
    cd rustc-extractor && cargo test --locked wp34_beh_rustc_exact_relation_schema_and_values_are_application_owned

[doc("Validate exhaustive provider relation descriptors, exact schemas, and generated IPC identity")]
[group('test')]
provider-relation-descriptor-contract-check:
    cargo nextest run --locked --lib -E 'test(/wp34_int_/)' --no-tests=fail
    just proto-check

[doc("Prove exact typed provider-native Arrow rows, decoded values, gaps, and incremental semantics")]
[group('test')]
exact-provider-batch-check:
    cargo nextest run --locked --lib -E 'test(/(wp34_beh_|tree_sitter_ruff_arrow_job_semantics)/)' --no-tests=fail
    cargo test --locked --no-default-features --features fact-generation --test integration isolated_fact_generation_executes_provider_jobs_to_arrow
    cd pyrefly-sidecar && cargo test --locked wp34_beh_
    cd rustc-extractor && cargo test --locked wp34_beh_

[doc("Prove bounded Tree-sitter state, Ruff reparse truth, cancellation, and clean-equivalent incremental output")]
[group('test')]
inprocess-provider-lifecycle-check:
    cargo nextest run --locked --no-default-features --features fact-generation --lib -E 'test(/(tree_sitter_job_drives_exact_parse_and_bounded_revisions|tree_sitter_job_limits_and_cancellation_are_causal|ruff_job_drives_owned_snapshot_and_honest_whole_file_reparse|ruff_job_limits_cancellation_and_lane_are_causal|inprocess_provider_incremental_lifecycle)/)' --no-tests=fail

[doc("Reject provider-native, fabric, state, release, daemon, and transport types outside their owning adapter boundaries")]
[group('gate')]
provider-type-boundary-check:
    ast-grep test --filter '^(fact-generation-inward-boundary-only|tree-sitter-boundary-only|ruff-boundary-only|generated-rustc-types-transport-only|no-pyrefly-public-api)$'
    ast-grep scan --filter '^(fact-generation-inward-boundary-only|tree-sitter-boundary-only|ruff-boundary-only|generated-rustc-types-transport-only)$' src
    ast-grep scan --filter '^no-pyrefly-public-api$' pyrefly-sidecar/src

[doc("Reject generated compiler transport values outside the Rust extractor adapter")]
[group('gate')]
generated-type-boundary-check:
    ast-grep test --filter '^(generated-rustc-types-transport-only|generated-cpg-types-transport-only|daemon-inward-boundary-only)$'
    ast-grep scan --filter '^(generated-rustc-types-transport-only|generated-cpg-types-transport-only|daemon-inward-boundary-only)$' src

[doc("Prove release injection, bounded data admission, reserved control capacity, and real UDS cancellation")]
[group('test')]
grpc-flow-control-contract-check:
    cargo nextest run --locked --lib -E 'test(/(injected_release_runtime_boundary_integrity|single_release_consumer_composition|grpc_flow_control_and_release_mismatch_faults|reserved_control_keeps_cancel_and_release_admissible|watch_iteration_deadline_releases_admission|read_iteration_deadline_releases_admission)/)' --no-tests=fail
    cargo nextest run --locked --test integration -E 'test(released_uds_transport_operations)' --no-tests=fail

[doc("Prove job-bound rustc conversion, provider results, cancellation, faults, and joined cleanup")]
[group('test')]
rustc-provider-lifecycle-check:
    cargo nextest run --locked --lib -E 'test(/rustc_(generated_type_termination_integrity|provider_application_result_semantics|ipc_and_admission_faults|provider_process_lifecycle)/)' --no-tests=fail

[doc("Prove retained Pyrefly native state, exact reconstruction, bounded memory, cancellation, and joined drain")]
[group('test')]
pyrefly-incremental-lifecycle-check:
    cargo nextest run --locked --lib -E 'test(/(pyrefly_native_state_containment_integrity|pyrefly_context_trust_and_memory_faults|pyrefly_cooperative_drain_reconstruction|wp34_ops_pyrefly_stale_generation_rejection_falsification)/)' --no-tests=fail
    cd pyrefly-sidecar && cargo test --locked pyrefly_

[doc("Reject descriptor gaps, empty success, opaque payloads, schema shortcuts, and provider-local canonical identity")]
[group('test')]
provider-gap-schema-shortcut-rejection-check:
    cargo nextest run --locked --lib -E 'test(/wp34_neg_/)' --no-tests=fail

[doc("Exercise relation IPC process faults, truncation, backpressure, cancellation, and cleanup")]
[group('test')]
relation-ipc-provider-operations-check:
    cargo build --locked --manifest-path pyrefly-sidecar/Cargo.toml --bin codefabric-pyrefly-sidecar
    CODEFABRIC_PYREFLY_SIDECAR_BIN="$CF_ROOT/target/debug/codefabric-pyrefly-sidecar" cargo nextest run --locked --lib -E 'test(/(wp34_ops_|rustc_provider_process_lifecycle)/)' --no-tests=fail
    cd pyrefly-sidecar && cargo test --locked wp34_ops_
    cd rustc-extractor && cargo test --locked wp34_ops_

[doc("Reject incomplete, opaque, corrupt, parallel, or trust-bypassing provider admission")]
[group('test')]
provider-admission-exclusivity-check:
    cargo nextest run --locked --lib -E 'test(/(duplicate_and_out_of_order_sequences_fail_closed|truncation_and_corruption_are_distinct_typed_failures|missing_trailer_and_terminal_are_not_complete|opaque_schema_carriers_are_rejected_before_any_provider_bytes|raw_json_cannot_masquerade_as_semantic_row_payload|exact_programmatic_admission_rejects_missing_pyrefly_coverage_relation|changed_source_or_rustc_receipt_binding_invalidates_workspace_authority|later_provider_failure_drops_the_partially_registered_builder|typed_relation_ingress_rejects_unknown_and_opaque_referenced_payloads|orchestrated_trusted_local_bypass_is_rejected_before_spawn|rustc_provider_process_lifecycle|orchestrated_context_mismatch_never_invokes_supervisor|pyrefly_stale_generation_rejection_falsification|inprocess_provider_admission_faults)/)' --no-tests=fail
    just remaining-legacy-zero-state-check

[doc("Exercise provider coverage, remainder, flow-control, cancellation, containment, and resource bounds")]
[group('test')]
provider-trust-coverage-remainder-check:
    cargo nextest run --locked --lib -E 'test(/(partial_and_unknown_coverage_are_explicit_and_counted|flow_control_credit_is_bounded_and_cancellation_is_terminal|frame_count_byte_budget_and_backpressure_are_enforced_before_allocation|cancellation_is_terminal_after_ipc_end_or_coverage_trailer|invalid_source_keeps_syntax_and_materializes_semantic_remainders|absent_language_lanes_return_explicit_unknowns_without_fake_tables|policy_closure_and_all_resource_limits_are_enforced|credential_proxy_agent_and_unknown_environment_are_rejected|cancellation_is_plan_bound_and_escalates_the_complete_group|surviving_process_group_fails_without_a_false_receipt|sampled_accounting_fails_closed_before_an_untrusted_spawn|orchestrated_untrusted_accounting_failure_never_executes_host_cargo|cargo_run_tracks_and_cancels_multiple_compilation_units_independently)/)' --no-tests=fail
    cd pyrefly-sidecar && cargo test --locked relation_acknowledgements_return_only_explicitly_accepted_credit
    cd rustc-extractor && cargo test --locked wp34_ops_rustc_relation_ipc_process_round_trip_is_exact_and_repeatable

[doc("Validate closed producer identities, dependencies, algorithms, inputs, and remainders")]
[group('test')]
analysis-producer-contract-integrity-check:
    cargo nextest run --locked --lib -E 'test(/wp35_int_/)' --no-tests=fail

[doc("Execute every currently realized typed producer against live catalog relations")]
[group('test')]
analysis-producer-semantic-check:
    cargo nextest run --locked --lib -E 'test(/wp35_beh_/)' --no-tests=fail

[doc("Reject zero or ambiguous producers, missing provider facts, and empty semantic success")]
[group('test')]
ambiguous-producer-empty-success-rejection-check:
    cargo nextest run --locked --lib -E 'test(/wp35_neg_/)' --no-tests=fail

[doc("Prove changed catalog inputs causally change typed producer outputs")]
[group('test')]
analysis-causal-fault-check:
    cargo nextest run --locked --lib -E 'test(/changed_catalog_inputs_causally_change_real_producer_outputs/)' --no-tests=fail

[doc("Enforce aggregate producer/fixed-point resource bounds without partial epochs")]
[group('test')]
analysis-fixed-point-resource-check:
    cargo nextest run --locked --lib -E 'test(/wp35_ops_/)' --no-tests=fail

[doc("Reject unknown-as-empty results, unauthorized child capabilities, predecessor selectors, SQL escapes, and parent authority leaks")]
[group('test')]
query-unknown-negative-proof-check:
    cargo nextest run --locked --lib -E 'test(/(invalid_closure_and_explicit_remainder_do_not_fallback|epoch_bound_direct_compiler_preserves_unknown_producer_closure|unknown_coverage_is_explicit_and_never_transport_truncation|empty_relation_retains_its_schema_and_explicit_completion|empty_result_retains_exact_projected_schema|zero_edges_preserve_the_contract_output_schema)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(reduced_catalog_exposes_allowed_table_and_physically_omits_denied_table|granted_view_graph_rejects_a_transitively_denied_parent_provider|shared_cache_tracks_concrete_registry_authority_not_only_opaque_pins|cached_reference_validation_reaches_subqueries_and_function_capabilities|child_owns_fresh_closed_registries_and_resources|unresolved_relation_and_function_name_authority_fail_closed|installs_only_exact_supplied_function_variable_and_store_capabilities|scope_authorization_can_only_narrow_baseline_capabilities|no_epoch_workspace_and_denied_relation_fail_at_their_authority_boundaries|epoch_bound_ingress_program_id_not_compatibility_form_selects_execution|provider_authority_and_judgment_are_rejected)/)' --no-tests=fail
    @if rg -n 'SELECT |SessionContext::sql|\.sql\(|sql_expr\(|query_sql|order_sensitive_checksum|SemanticQueryPlanner|QueryFormCrosswalk|model_epoch_pin|bootstrap_model' src/query_service.rs src/semantic_query_contract.rs src/relational_semantic_query.rs src/fabric/programmatic_ingress_port.rs src/fabric/programmatic_query_backend.rs src/fabric/child_session.rs src/fabric/relational_query_runtime.rs src/fabric/graph_program.rs; then echo 'retired query authority or SQL/name escape remains on the semantic query path' >&2; exit 1; fi

[doc("Prove native graph planning, layout determinism, bounded resources, cancellation, cache isolation, pagination, and cleanup")]
[group('test')]
graph-query-resource-operations-check:
    cargo nextest run --locked --lib -E 'test(/(cycles_and_duplicate_paths_produce_unique_minimum_depth_rows|zero_edges_preserve_the_contract_output_schema|depth_and_output_limits_are_enforced_without_partial_success|recursive_plan_remains_optimizer_visible_and_has_no_extension|invalid_bindings_and_bounds_fail_before_plan_construction|partition_and_batch_layout_preserve_deterministic_graph_rows|cancellation_is_typed_reusable_and_releases_graph_resources)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(success_is_arrow_native_deterministic_and_causally_observed|cancelled_transaction_never_consumes_epoch_capacity_or_result_lease|row_batch_schema_and_ipc_resource_overflow_fail_without_publication|result_reads_reauthorize_owner_and_token_and_release_is_terminal|published_result_retains_exact_predecessor_epoch_across_swap_until_release|caller_admitted_execution_cannot_mix_compilation_and_execution_epochs)/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(row_and_ipc_byte_overflow_fail_without_truncation|chunks_reassemble_exact_bytes_and_checksum|owner_token_resource_release_tombstone_and_expiry_are_explicit|result_release_returns_epoch_capacity_after_causal_backpressure|canonical_batch_checksum_is_row_order_independent|wp64_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)|logical_plan_cache_(is_collision_safe_bounded_and_lru|bypasses_oversized_entries_without_changing_semantics|rejects_materialization_collision_for_complete_key))/)' --no-tests=fail

[doc("Prove parameter-neutral plan identity and partition-independent Arrow result checksums")]
[group('test')]
query-determinism-check:
    cargo nextest run --locked --lib -E 'test(/(wp64_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)|wp64_production_replay_is_partition_and_batch_independent)/)' --no-tests=fail

[doc("Prove execution-scoped persisted plan artifacts use the exact served plan without diagnostic re-execution")]
[group('test')]
query-artifact-single-execution-check:
    cargo nextest run --locked --lib -E 'test(/(query_failure_artifact_closure|query_terminal_journal_authority|query_artifact_no_diagnostic_reexecution|query_artifact_failure_operational_gate)/)' --no-tests=fail
    @if rg -n 'AnalyzeExec::new|LogicalPlan::Analyze|EXPLAIN ANALYZE' src/query_service.rs src/fabric/query_artifact.rs src/fabric/programmatic_query_backend.rs; then echo 'governed serving must not construct AnalyzeExec or EXPLAIN ANALYZE' >&2; exit 1; fi

[doc("Run the complete Wave-6 continuous-update acceptance surface")]
[group('test')]
wave6-integration-check:
    cargo nextest run --locked -E 'test(/(wp(23|24|25|26|4[1-8])|wp66_)/)' --no-tests=fail

[doc("Compare Git-accelerated candidates and state with authoritative fallback")]
[group('test')]
git-parity-check:
    cargo nextest run --locked --test integration -E 'test(/wp(49|50|51|52|53)/)' --no-tests=fail

[doc("Run the complete Wave-7 Git-aware lifecycle acceptance surface")]
[group('test')]
wave7-integration-check: git-parity-check source-capture-race-check

[doc("Run the Wave-8 Python-local semantic acceptance slice; WP02-WP07 populate the selector")]
[group('test')]
wave8-integration-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(py_context_(discovery_conformance|manifest_identity_parity|guess_rejection_falsification|invalidation_operational_gate)|py_scope_binding_fixture_conformance|ruff_semantic_isolation_parity|py_unresolved_reference_unknown_falsification|py_scope_binding_owner_replacement_gate|py_import_export_fixture_conformance|py_import_syntax_semantic_distinction_parity|py_dynamic_export_unknown_falsification|py_module_fact_replacement_gate|py_callable_call_site_fixture_conformance|py_call_site_first_class_parity|py_dynamic_splat_unknown_argument_falsification|py_callable_contract_replacement_gate|py_cfg_fixture_conformance|py_cfg_wellformedness_parity|py_cfg_exceptional_edge_falsification|py_cfg_owner_invalidation_gate|py_defuse_fixture_conformance|py_semantic_profile_partial_parity|py_parse_error_capability_gap_falsification|wave8_integration_operational_gate)$/)' --no-tests=fail

[doc("Run every retained Wave-2 and Wave-4 through Wave-7 integration gate")]
[group('test')]
wave-acceptance-check: wave2-integration-check wave4-integration-check wave5-integration-check wave6-integration-check wave7-integration-check

[doc("Validate that a vacuum dry-run cannot include retained snapshot files")]
[group('test')]
vacuum-dry-run-check:
    cargo nextest run --locked -E 'test(/(wp24_negative_zero_state|wp66_negative_zero_state)/)' --no-tests=fail

[doc("Run the seeded 10,000-attempt three-size source-capture race campaign")]
[group('test')]
source-capture-race-check:
    cargo test --locked wp16_source_capture_race_campaign -- --ignored

# ----------------------------------------------------------------- fast / PR gates

[doc("Fast unused-dependency hygiene")]
[group('gate')]
deps-fast:
    cargo machete
    cargo shear --deny-warnings

[doc("Dependency policy and known-advisory scan")]
[group('gate')]
policy:
    ./scripts/advisory_policy_check.sh --audit
    cargo deny check --hide-inclusion-graph advisories bans sources

[doc("Validate exact advisory exceptions against lockfile, deny, and RustSec")]
[group('gate')]
advisory-policy-check:
    ./scripts/advisory_policy_check.sh

# The routine baseline. Run it before editing and record pre-existing failures separately
# from anything the edit causes (spec section 59.1).

[doc("Validate the exact resolved stable dependency and feature graph")]
[group('gate')]
stable-graph-check:
    ./scripts/stable_graph_check.sh

[doc("Run repository structural governance rules")]
[group('gate')]
governance-scan:
    ast-grep test
    ast-grep scan \
      --globs '!contracts/generated/**' \
      --globs '!src/generated/**' \
      --globs '!codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/**' \
      --globs '!rustc-extractor/src/generated/**' \
      --globs '!pyrefly-sidecar/src/generated/**'

[doc("The routine stable-root gate")]
[group('gate')]
root-ci-fast: root-fmt root-check root-clippy root-test typos deps-fast stable-graph-check

# --------------------------------------------------------- independent build domains

[doc("Format-check the dated-nightly rustc extractor")]
[group('extractor')]
extractor-fmt:
    cd rustc-extractor && cargo fmt --all -- --check

[doc("Compile and lint the dated-nightly rustc extractor")]
[group('extractor')]
extractor-check:
    cd rustc-extractor && cargo-check-mode.sh cargo check --all-targets --locked
    cd rustc-extractor && cargo-check-mode.sh cargo clippy --all-targets --locked -- -D warnings

[doc("Test the dated-nightly rustc extractor")]
[group('extractor')]
extractor-test:
    cd rustc-extractor && cargo test --locked

[doc("Launch the built extractor directly and verify exact stderr-only identity")]
[group('extractor')]
extractor-identity:
    #!/usr/bin/env bash
    set -euo pipefail
    repo_root="$(pwd)"
    (cd rustc-extractor && "$repo_root/scripts/cargo" build --locked)
    temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-extractor-identity.XXXXXX")"
    trap 'rm -rf "$temporary_root"' EXIT
    ./scripts/run_rustc_extractor.sh --identity >"$temporary_root/stdout" 2>"$temporary_root/stderr"
    test ! -s "$temporary_root/stdout"
    cmp rustc-extractor/toolchain-identity.json "$temporary_root/stderr"

[doc("Run the complete extractor gate")]
[group('extractor')]
extractor-ci-fast: extractor-fmt extractor-check extractor-test extractor-identity

[doc("Format-check the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-fmt:
    cd pyrefly-sidecar && cargo fmt --all -- --check

[doc("Compile and lint the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-check:
    cd pyrefly-sidecar && cargo-check-mode.sh cargo check --all-targets --locked
    cd pyrefly-sidecar && cargo-check-mode.sh cargo clippy --all-targets --locked -- -D warnings

[doc("Test the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-test:
    cd pyrefly-sidecar && cargo test --locked

[doc("Check sidecar advisories, bans, and sources (licenses excluded)")]
[group('sidecar')]
sidecar-policy:
    cd pyrefly-sidecar && cargo deny check --hide-inclusion-graph advisories bans sources
    cd pyrefly-sidecar && cargo audit

[doc("Run the complete sidecar routine gate")]
[group('sidecar')]
sidecar-ci-fast: sidecar-fmt sidecar-check sidecar-test

[doc("Check adapter Ruff formatting and lint")]
[group('adapter')]
adapter-lint:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff format --check codefabric-cpg-mcp
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff check codefabric-cpg-mcp

[doc("Type-check the configured adapter source and test trees")]
[group('adapter')]
adapter-type:
    cd codefabric-cpg-mcp && uv run --frozen pyrefly check

[doc("Test the locked FastMCP adapter")]
[group('adapter')]
adapter-test:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest codefabric-cpg-mcp/tests

[doc("Test locked-command STDIO startup, shutdown, and protocol silence")]
[group('adapter')]
adapter-stdio-test:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest codefabric-cpg-mcp/tests/test_stdio.py

[doc("Build and import the adapter wheel with its canonical artifact-index resource")]
[group('adapter')]
adapter-wheel-test:
    ./scripts/adapter_wheel_test.sh

[doc("Run the complete adapter gate")]
[group('adapter')]
adapter-ci-fast: adapter-lint adapter-type adapter-test

# ------------------------------------------------------ assurance governance

[doc("Check formatting and lint for plan-governance helpers")]
[group('gate')]
governance-tooling-lint:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff format --check tooling/ci
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" ruff check tooling/ci

[doc("Validate active plan, review, and schema-2 execution-state contracts")]
[group('gate')]
artifacts-check: governance-tooling-lint
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest tooling/ci/test_artifact_contracts.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/artifact_contracts.py artifacts-check

[doc("Run reproducible non-normative semantic substrate warm/cold workloads")]
[group('perf')]
semantic-profile-bench:
    uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/benchmarks/semantic_profile_bench.py

[doc("Validate governed oracle criteria, substantive definitions, and zero-match-safe selectors")]
[group('gate')]
oracle-substance-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest tooling/ci/test_plan_assurance.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/plan_assurance.py oracle-substance-check
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/plan_assurance.py current-packet-oracle-check

[doc("Validate the active packet DAG and disposition every unordered known-touch overlap")]
[group('gate')]
plan-dependency-check *args:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/plan_assurance.py dependency-check "$@"

[doc("Validate committed name-coupled nextest selectors and zero-selection failure semantics")]
[group('gate')]
gate-filter-census:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" pytest -q tooling/ci/test_gate_filter_census.py
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python scripts/gate_filter_census.py check

[doc("Execute exactly four substantive acceptance oracles for one implementation packet")]
[group('test')]
packet-oracle-check packet:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/plan_assurance.py packet-oracle-check "{{packet}}"

[doc("Derive active-plan input freshness and proving-commit trust")]
[group('gate')]
plan-status:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/artifact_contracts.py plan-status

[doc("Reject Cargo target outputs in the index or reachable HEAD history")]
[group('gate')]
tracked-target-zero-state-check:
    @PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/artifact_contracts.py tracked-target-zero-state-check

[doc("Prove family duplicate policy and its expected-failure fixture")]
[group('gate')]
duplicate-family-check:
    ./scripts/duplicate_family_check.sh

[doc("Prove retired native-extension seed and root packaging surfaces stay absent")]
[group('gate')]
seed-zero-state-check:
    ./scripts/seed_zero_state_check.sh

[doc("Prove superseded catalog, writer, proof-manifest, and packet-mutation surfaces stay absent")]
[group('gate')]
model-zero-state-check:
    ./scripts/model_zero_state_check.sh

[doc("Run structural, artifact, provenance, compatibility, and zero-state governance")]
[group('gate')]
governance: tool-version-contract-check governance-scan authoritative-design-conformance-check proto-check model-zero-state-check remaining-legacy-zero-state-check artifacts-check plan-status tracked-target-zero-state-check duplicate-family-check seed-zero-state-check oracle-substance-check plan-dependency-check

[doc("Run the routine gate across all four build domains")]
[group('gate')]
ci-fast: root-ci-fast extractor-ci-fast sidecar-ci-fast adapter-ci-fast governance

[doc("Fresh-shell regression: environment, cache, extractor, adapter, and stable-root tests")]
[group('gate')]
environment-regression: environment-contract-check sccache-canary doctor extractor-check adapter-lint adapter-test root-test

[doc("ci-fast plus policy, the ci nextest profile, and snapshot review state")]
[group('gate')]
ci-pr: ci-fast policy sidecar-policy wave-acceptance-check
    cargo nextest run --locked -P ci
    cargo test --locked --doc
    cargo insta pending-snapshots

# ------------------------------------------------------- coverage / test quality

# Coverage answers what executed, not whether assertions constrain behavior. No percentage
# threshold is configured; section 21.1 warns against adopting one merely because a tool
# supports it.

[doc("Rust line coverage to target/coverage/lcov.info")]
[group('quality')]
coverage:
    mkdir -p target/coverage
    cargo llvm-cov nextest --locked \
      --features local-workstation \
      --lcov \
      --output-path target/coverage/lcov.info

[doc("Interactively review pending snapshot diffs")]
[group('quality')]
snapshots-review:
    cargo insta review

[doc("Mutation-test one changed file")]
[group('quality')]
mutants-file path:
    cargo mutants -f {{path}}

# Nightly is the extractor's production toolchain and remains isolated from this root.
# Miri explores executions; it never proves soundness (spec section 24.2). Record
# toolchain, seed range, and exclusions with any finding.

[doc("Miri UB check on the exact dated assurance toolchain")]
[group('quality')]
miri:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly-2026-08-18 miri test --locked

[doc("Miri across a range of randomized seeds")]
[group('quality')]
miri-seeds seeds="16":
    CARGO_TARGET_DIR=target/nightly-assurance MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly-2026-08-18 miri test --locked

[doc("Compiler-oriented unused-dependency adjudication")]
[group('quality')]
udeps:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly-2026-08-18 udeps --locked --all-targets --all-features

# Bounded runs only; long campaigns belong in scheduled infrastructure (spec section 23).
# WP06's canonical JSON decoder is the first production-path untrusted-input surface;
# its fuzz harness exercises the same parser and serializer used by the verifier.

[doc("Bounded fuzz run against one target")]
[group('quality')]
fuzz target seconds="60":
    rust_host="$(rustc +nightly-2026-08-18 -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly-2026-08-18 fuzz run --target "$rust_host" --target-dir "target/fuzz/$rust_host" \
      {{target}} "$runtime_corpus" -- -max_total_time={{seconds}}

[doc("Coverage of a fuzz corpus")]
[group('quality')]
fuzz-coverage target:
    rust_host="$(rustc +nightly-2026-08-18 -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly-2026-08-18 fuzz coverage --target "$rust_host" \
      --target-dir "target/fuzz/$rust_host" {{target}} "$runtime_corpus"

# --------------------------------------------------------- feature / compatibility

# `--all-features` validates only the maximal additive union and can hide accidental
# coupling between features (spec sections 26 and 62.6).

[doc("Check every feature in isolation")]
[group('compat')]
features-each:
    cargo hack check --locked --each-feature

[doc("Check with no default features")]
[group('compat')]
features-no-default:
    cargo hack check --locked --no-default-features

[doc("Verify the declared MSRV (inert until Cargo.toml declares rust-version)")]
[group('compat')]
msrv:
    cargo msrv verify

[doc("Rust API compatibility against a baseline revision")]
[group('compat')]
semver baseline:
    cargo semver-checks --baseline-rev {{baseline}}

# ------------------------------------------------------- dependency / supply chain

# cargo-vet is deliberately NOT adopted (spec sections 32 and 93.4): maintaining human
# audit attestations is real work, and a supply-chain/ directory should not be created
# just to have one. Add a `vet` recipe when that workflow is genuinely intended.
#
# Geiger's count is an inventory, not a vulnerability score (spec section 33). Use it to
# decide where Miri, fuzzing, and manual review should focus.

[doc("Unsafe-code surface inventory")]
[group('supply-chain')]
unsafe-surface:
    cargo geiger

# ------------------------------------------------ artifact / performance investigation

# `target/` disk usage is not artifact size (spec sections 39 and 82.2). Measure the
# final wheel or binary. Profiling locates a hotspot; only a controlled benchmark
# verifies an improvement (spec section 40.4).

[doc("Attribute release code size by crate")]
[group('perf')]
bloat:
    cargo bloat --release --crates

[doc("Symbols in the release artifact")]
[group('perf')]
symbols:
    cargo nm --release

[doc("Section sizes of the release artifact")]
[group('perf')]
sections:
    cargo size --release

[doc("MIR, LLVM IR, or assembly for one function")]
[group('perf')]
asm *args:
    cargo asm "$@"

[doc("Release codegen with symbols preserved, for samply or flamegraph")]
[group('perf')]
profile-build:
    cargo build --locked --profile profiling

# ------------------------------------------------------------- mutating operations
#
# Everything below changes source, manifests, or the environment. None of it is a
# dependency of any gate. After running one: inspect the diff, identify the semantic
# impact, rerun the relevant validation, and disclose what the tool changed
# (spec section 63).

[confirm("Execute the frozen WP65 workloads and create exclusive raw/review evidence. Continue?")]
[doc("MUTATES: create target-only WP65 measurement evidence and an isolated release target")]
[group('mutating')]
compiled-release-resource-performance-capture:
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/benchmarks/fastmcp4_release_benchmark.py run
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/fastmcp4_release_performance.py summarize

[confirm("Create schema-2 state and activate the approved plan atomically. Continue?")]
[doc("MUTATES: create validated execution state before switching the active-plan pointer")]
[group('mutating')]
plan-activate plan:
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/ci/artifact_contracts.py activate-plan --plan "{{plan}}"

[doc("MUTATES: rewrite Rust formatting in place")]
[group('mutating')]
root-fmt-write:
    cargo fmt --all

[confirm("Regenerate the released descriptor census and Rust/Python Protobuf bindings. Continue?")]
[doc("MUTATES: regenerate released Protobuf outputs without changing the compatibility baseline")]
[group('mutating')]
proto-gen:
    PYTHONPATH=. uv run --frozen --project "$CF_ROOT/codefabric-cpg-mcp" python tooling/proto/generate.py write

[confirm("typos -w rewrites source in place; identifier fixes can be API changes. Continue?")]
[doc("MUTATES: apply spelling corrections to source")]
[group('mutating')]
typos-write:
    typos -w

[confirm("Accept all pending snapshots without semantic review. Continue?")]
[doc("MUTATES: accept pending snapshots -- never a CI step or automatic fix")]
[group('mutating')]
snapshots-accept:
    cargo insta accept

[confirm("cargo shear --fix edits Cargo.toml; scanners produce hypotheses, not permission. Continue?")]
[doc("MUTATES: remove dependencies cargo-shear reports as unused")]
[group('mutating')]
deps-fix:
    cargo shear --fix

[doc("List available updates for globally installed Cargo executables")]
[group('mutating')]
tool-updates-check:
    cargo install-update --list
