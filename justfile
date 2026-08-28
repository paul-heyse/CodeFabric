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

set shell := ["bash", "-euo", "pipefail", "-c"]

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
    cargo metadata --format-version 1 --no-deps

# sccache is a committed build prerequisite (.cargo/config.toml). Spec section 13.2:
# watch the hit rate rather than assuming the wrapper helps.

[doc("sccache hit rate and cache state")]
[group('environment')]
cache-stats:
    sccache --show-stats

[doc("Zero sccache statistics before a measurement run")]
[group('environment')]
cache-zero-stats:
    sccache --zero-stats

[doc("Navigate docs/authoritative_design by section without reading whole specs")]
[group('environment')]
spec-outline *args:
    ./scripts/spec-outline.sh "$@"

[doc("Prove the exact eight-master design suite, generated identities, navigation, and sole live authority root")]
[group('contracts')]
authoritative-design-conformance-check:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_authoritative_design_conformance.py

[doc("Compile every authored ontology operation into the normalized Arrow program package")]
[group('contracts')]
ontology-program-compiler-check:
    cargo nextest run --locked --lib -E 'test(/ontology_program_bundle_(semantic_parity|model_rebuild)/)' --no-tests=fail

[doc("Prove ontology-program identity acyclicity and byte-reproducible Arrow IPC packaging")]
[group('contracts')]
ontology-program-packaging-check:
    cargo nextest run --locked --lib -E 'test(/ontology_program_bundle_(digest_acyclicity|ipc_reproducibility)/)' --no-tests=fail

[doc("Prove a bijective current-profile catalog of native DataFusion calculations")]
[group('test')]
ontology-calculation-catalog-check:
    cargo nextest run --locked --lib -E 'test(/ontology_(compiled_program_native_profile|calculation_catalog_bijection)/)' --no-tests=fail

[doc("Mutate compiled operators and phrase operands and prove governed DataFusion outcomes change")]
[group('test')]
ontology-program-causality-check:
    cargo nextest run --locked --lib -E 'test(/ontology_(compiled_program_causality_matrix|phrase_binding_fail_closed)/)' --no-tests=fail

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
    cargo check --all-targets
    cargo check --all-targets --no-default-features

[doc("Clippy on default local and featureless stable-domain surfaces")]
[group('static')]
root-clippy:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --no-default-features -- -D warnings

[doc("Spelling and identifier hygiene")]
[group('static')]
typos:
    typos

# ------------------------------------------------------------------------ tests

[doc("Rust tests via nextest (does NOT include doctests)")]
[group('test')]
root-test-rust:
    cargo nextest run

# nextest does not run doctests. This is a separate, mandatory step -- never report "all
# Rust tests passed" from nextest alone (spec sections 18.2 and 62.2).

[doc("Rust doctests -- nextest does not cover these")]
[group('test')]
root-doctest:
    cargo test --doc

[doc("Everything in the stable root: Rust tests and doctests")]
[group('test')]
root-test: root-test-rust root-doctest

[doc("Run the current Wave-2 daemon/control-plane acceptance oracles")]
[group('test')]
wave2-integration-check:
    cargo nextest run --locked -E 'test(/wp1[2-8]/)' --no-tests=fail
    cargo test --doc

[doc("Run the current Wave-3 fabric/publication acceptance oracles")]
[group('test')]
wave3-integration-check:
    cargo nextest run --locked -E 'test(/wp(19|2[0-6])/)' --no-tests=fail
    cargo test --doc

[doc("Prove generated foreign keys over the complete candidate publication state")]
[group('test')]
publication-referential-integrity-check:
    cargo nextest run --locked --lib -E 'test(/wp74_/)' --no-tests=fail

[doc("Run the DataFusion 55, Arrow 59, and delta 43a0cf10 behavioral contract")]
[group('test')]
data-fabric-upgrade-check:
    cargo nextest run --locked --test integration -E 'test(/(arrow59_|wp03_|wp05_|wp06_(behavioral|structural|negative)|delta_43a0cf10_|data_fabric_(old_write_new_read|new_write_old_read)_compatibility|data_fabric_(target_stack_release|old_live_authority|current_reference_routing|gate_b_empty))/)' --no-tests=fail
    cargo nextest run --locked --lib -E 'test(/(datafusion_55_|delta_43a0cf10_|wp03_operational|wp05_|wp2[12]_)/)' --no-tests=fail

[doc("Prove FixedSizeBinary(16) extension preservation, fallback, and staged schema instances")]
[group('test')]
id16-extension-contract-check:
    cargo nextest run --locked --lib -E 'test(/wp58_(structural_acceptance|negative_zero_state)/)' --no-tests=fail
    just model-family-check schemas

[doc("Prove effective-relation statistics, pushdown truth, and observed runtime evidence")]
[group('test')]
provider-statistics-contract-check:
    cargo nextest run --locked --lib -E 'test(/(wp58_(behavioral|operational)_acceptance|datafusion_55_effective_provider_statistics_contract)/)' --no-tests=fail

[doc("Prove the common direct/IPC provider fact protocol, schema census, and rejection posture")]
[group('test')]
provider-protocol-check:
    cargo nextest run --locked --lib -E 'test(/wp59_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)/)' --no-tests=fail

[doc("Exercise the fail-closed semantic-provider sandbox escape matrix on the current host")]
[group('test')]
semantic-sandbox-host-matrix-check:
    cargo nextest run --locked --lib -E 'test(/semantic_sandbox_current_host_escape_matrix/)' --no-tests=fail
    @if rg -n 'ObservationMessage|CanonicalFact|encode_selected|with_skip_validation' src/ rustc-extractor/src/; then echo 'provider protocol legacy or unsafe IPC validation bypass remains' >&2; exit 1; fi
    @rg -n 'StreamDecoder::new\(\)\.with_require_alignment\(false\)' src/fact_ingest.rs >/dev/null

[doc("Prove predecessor data-fabric identities survive only in reviewed historical locations")]
[group('static')]
data-fabric-old-authority-check:
    ./scripts/data_fabric_old_authority_check.sh

[doc("Run the complete Wave-4 source/provider/core-fact acceptance slice")]
[group('test')]
wave4-integration-check:
    cargo nextest run --locked -E 'test(/wp(27|28|29|30|31|32|33)/)' --no-tests=fail
    cargo test --doc

[doc("Run the complete Wave-5 vertical golden-slice acceptance surface")]
[group('test')]
wave5-integration-check:
    cargo nextest run --locked -E 'test(/(wp(20|3[4-9]|40|62|63|64|65)|qry_v13_graph_forms_conformance|semantic_query_(mixed_dag_contract|graph_adversarial_conformance|graph_operational_gate)|production_eight_form_semantic_query_conformance)/)' --no-tests=fail
    cd rustc-extractor && cargo test --locked wp35
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests

[doc("Prove daemon activation, authenticated UDS reachability, CORE_SOURCE_V1 status, and joined cancellation")]
[group('test')]
query-daemon-activation-check:
    cargo nextest run --locked -E 'test(/wp63_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)/)' --no-tests=fail

[doc("Prove all eight semantic query forms, mixed DAG execution, ordering, and absence semantics")]
[group('test')]
semantic-query-conformance-check:
    cargo nextest run --locked --lib -E 'test(/(qry_v13_(form_contract_conformance|relational_forms_conformance|graph_forms_conformance)|semantic_query_(relational_plan_visibility|relational_policy_and_absence|relational_operational_gate|mixed_dag_contract|graph_adversarial_conformance|graph_operational_gate)|production_eight_form_semantic_query_conformance|wp39_operational_acceptance)/)' --no-tests=fail

[doc("Prove the generated QRY 1.3 form authority, Rust/Python projections, and retired-slug zero state")]
[group('test')]
query-form-contract-check:
    cargo nextest run --locked --lib -E 'test(/(qry_v13_form_contract_conformance|query_form_projection_parity|qry_v13_connecting_path_schema_falsification|query_form_contract_operational_gate)/)' --no-tests=fail
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests/test_proto.py -k query_form_python_projection_parity
    just model-repro-check

[doc("Prove the five governed relational QRY forms compile and execute as native DataFusion plans")]
[group('test')]
semantic-query-relational-conformance-check:
    cargo nextest run --locked --lib -E 'test(/(qry_v13_relational_forms_conformance|semantic_query_relational_plan_visibility|semantic_query_relational_policy_and_absence|semantic_query_relational_operational_gate)/)' --no-tests=fail

[doc("Prove parameter-neutral plan identity and partition-independent Arrow result checksums")]
[group('test')]
query-determinism-check:
    cargo nextest run --locked --lib -E 'test(/(wp64_(behavioral_acceptance|structural_acceptance|negative_zero_state|operational_acceptance)|wp64_production_replay_is_partition_and_batch_independent)/)' --no-tests=fail

[doc("Prove execution-scoped persisted plan artifacts use the exact served plan without diagnostic re-execution")]
[group('test')]
query-artifact-single-execution-check:
    cargo nextest run --locked --lib -E 'test(/(query_failure_artifact_closure|query_terminal_journal_authority|query_artifact_no_diagnostic_reexecution|query_artifact_failure_operational_gate)/)' --no-tests=fail
    @if rg -n 'AnalyzeExec::new|LogicalPlan::Analyze|EXPLAIN ANALYZE' src/query_service.rs src/semantic_query.rs src/fabric/serving.rs; then echo 'governed serving must not construct AnalyzeExec or EXPLAIN ANALYZE' >&2; exit 1; fi

[doc("Prove the semantic query path contains no legacy SQL builder, string state fields, or order-sensitive checksum")]
[group('static')]
query-legacy-zero-state-check:
    @if rg -n 'SELECT ' src/semantic_query.rs src/query_service.rs; then echo 'legacy SQL remains on the semantic query path' >&2; exit 1; fi
    @if rg -n 'fn query_sql|f\(sql, snapshot\)|order_sensitive_checksum' src/semantic_query.rs src/query_service.rs src/fabric/; then echo 'legacy query identity or checksum remains' >&2; exit 1; fi
    @just alignment-detector-check DP-110
    cargo nextest run --locked --lib -E 'test(/(wp62_negative_zero_state|wp75_negative_zero_state|wp64_negative_zero_state)/)' --no-tests=fail

[doc("Verify accountable-owner acceptance and execute the immutable released Gate B corpus")]
[group('test')]
gate-b-check: gate-b-owner-acceptance-check wave5-integration-check wave6-integration-check adapter-wheel-test model-release-census-check
    cargo run --locked --bin codefabric-gate-b-candidate -- check-release . target/gate-b-release-check-scratch

[doc("Verify the Gate B owner authority, accepted candidate, immutable corpus, and current-version index")]
[group('test')]
gate-b-owner-acceptance-check:
    cargo run --locked --bin codefabric-gate-b-candidate -- verify-release .
    @just packet-oracle-check WP76

[doc("Execute the production Gate B vertical and verify functional-candidate isolation")]
[group('test')]
gate-b-candidate-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(gate_b_vertical_slice_produces_all_eleven_planes|gate_b_vertical_slice_adversarial|gate_b_candidate_independent_oracle_contract|gate_b_candidate_operational_gate)/)' --no-tests=fail
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest -q codefabric-cpg-mcp/tests/test_stdio.py codefabric-cpg-mcp/tests/test_adapter_contracts.py
    @if rg -n 'fn run_scenario|async fn run_(pyrefly|rustc)|functional_candidate_projection|normalize_gate_b_planes' src/gate_b_candidate.rs src/gate_b_candidate; then echo 'candidate-local scenario, provider, or comparison authority remains' >&2; exit 1; fi
    @if test -d tests/golden/review-candidates/codefabric-golden-v3.0.0-candidate.1; then cargo run --locked --bin codefabric-gate-b-candidate -- verify tests/golden/review-candidates/codefabric-golden-v3.0.0-candidate.1; fi
    @just packet-oracle-check WP06

[doc("Validate strict, human-authored Gate B semantic claims, anchors, scenarios, and proof universes")]
[group('test')]
functional-golden-contract-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(functional_golden_claim_schema_conformance|functional_golden_source_anchor_closure|functional_golden_negative_claim_requires_complete_universe|functional_golden_duplicate_key_rejected|functional_golden_contract_operational_gate)/)' --no-tests=fail
    @if rg -n '"(expected_digest|candidate_digest|canonical_row_hex|governed_key_hex|response_bytes_hex|descriptor_identity|registry_count|runtime_id|matches|requirement_checks)"|"b3:' tests/golden/codefabric-golden-v4; then echo 'captured output or integrity material remains in the functional expectation authority' >&2; exit 1; fi

[doc("Prove the rejected Gate B v3 candidate remains immutable and outside release routing")]
[group('test')]
gate-b-rejected-candidate-zero-state-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(gate_b_rejected_candidate_zero_state)' --no-tests=fail
    @if rg -n 'codefabric-golden-v3\.0\.0-candidate\.1' tests/golden/corpus-index.json tests/golden/codefabric-golden-v2/owner-acceptance.json; then echo 'rejected Gate B candidate entered current or accepted release metadata' >&2; exit 1; fi

[doc("Execute the independent logical evaluators and prove expectation/production isolation")]
[group('test')]
functional-golden-independence-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(reference_query_evaluator_laws|functional_golden_comparator_falsification|functional_golden_expectation_write_isolation|functional_golden_independence_operational_gate|reference_bag_multiplicity_law|reference_coverage_monotonicity_law)/)' --no-tests=fail
    @if rg -n 'use crate::(semantic_query|reconciliation|gate_b_candidate|gate_b_release|lifecycle)|use (datafusion|petgraph)' src/functional_golden.rs src/functional_golden; then echo 'functional evaluator imports a production semantic engine' >&2; exit 1; fi
    @if rg -n 'fs::(write|create_dir|remove|rename)|File::create|OpenOptions' src/functional_golden.rs src/functional_golden; then echo 'functional expectation closure contains a write path' >&2; exit 1; fi

[doc("Execute the first-principles Gate B provider, canonical, public-query, and scenario contract")]
[group('test')]
gate-b-public-vertical-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(gate_b_public_vertical_conformance|golden_scenario_semantic_transition_contracts|gate_b_public_vertical_operational_gate)/)' --no-tests=fail

[doc("Prove named fixture edits and producing/public seam interventions affect their dependent Gate B claims")]
[group('test')]
gate-b-causal-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(gate_b_causal_intervention_matrix|gate_b_named_fixture_query_causality)/)' --no-tests=fail

[doc("Prove UDS artifact readback and locked FastMCP STDIO preserve exact Gate B semantics")]
[group('test')]
gate-b-delivery-equivalence-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(gate_b_delivery_surface_semantic_equivalence)' --no-tests=fail

[doc("Prove the governed comparison projection is the only Gate B semantic-ignore authority")]
[group('static')]
gate-b-projection-registry-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(gate_b_projection_registry_closure)' --no-tests=fail
    @if rg -n 'semantic[^\n]*(ignore|normaliz)|ignore[^\n]*semantic' src/gate_b_candidate.rs src/gate_b_candidate; then echo 'candidate-local semantic comparison ignore or normalizer remains' >&2; exit 1; fi
    @if rg -n 'fn run_scenario|async fn run_(pyrefly|rustc)|codefabric-pyrefly-sidecar|run_rustc_extractor\.sh' src/gate_b_candidate.rs src/gate_b_candidate; then echo 'candidate-local scenario edit dispatcher or provider ownership remains' >&2; exit 1; fi

[doc("Execute every registered Gate B semantic mutant and reject survivors, collateral failures, or claim gaps")]
[group('test')]
semantic-oracle-mutants-check profile:
    @if test "{{profile}}" != "gate-b"; then echo 'only the gate-b semantic mutant profile is registered' >&2; exit 1; fi
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(semantic_oracle_required_mutants_are_killed|semantic_oracle_rejects_unregistered_or_surviving_mutant)/)' --no-tests=fail

[doc("Generate and validate the decoded claim-by-claim Gate B human review dossier")]
[group('test')]
gate-b-review-bundle-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(gate_b_human_review_bundle_contract)' --no-tests=fail

[doc("Validate functional candidate isolation and any committed successor digest chain")]
[group('test')]
gate-b-functional-candidate-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    cargo nextest run --locked --lib --no-fail-fast -E 'test(gate_b_functional_candidate_is_expectation_independent)' --no-tests=fail
    @if test -d tests/golden/review-candidates/codefabric-golden-v4.0.0-candidate.1; then cargo run --locked --bin codefabric-gate-b-candidate -- verify-functional tests/golden/review-candidates/codefabric-golden-v4.0.0-candidate.1; else echo 'functional Gate B successor is not candidate-ready or committed' >&2; exit 1; fi
    @if rg -n 'codefabric-golden-v4\.0\.0-candidate\.1' tests/golden/corpus-index.json; then echo 'unaccepted functional candidate advanced the corpus index' >&2; exit 1; fi

[doc("Verify immutable v2 release, rejected v3 candidate, and functional successor remain distinctly routed")]
[group('test')]
gate-b-predecessor-check:
    cargo run --locked --bin codefabric-gate-b-candidate -- verify tests/golden/review-candidates/codefabric-golden-v2.0.0-candidate.1
    cargo run --locked --bin codefabric-gate-b-candidate -- verify tests/golden/review-candidates/codefabric-golden-v3.0.0-candidate.1
    just gate-b-rejected-candidate-zero-state-check

[doc("Compare continuous effective state with the clean-rebuild oracle")]
[group('test')]
rebuild-equivalence-check:
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    CODEFABRIC_FULL_REBUILD_PROVIDERS=1 cargo nextest run --locked --lib -E 'test(/(full_golden_scenario_clean_rebuild_equivalence|clean_rebuild_independence_contract|clean_rebuild_equivalence_adversarial|clean_rebuild_operational_gate|wp72_rejects_either_noncurrent_comparison_input|wp72_structural_acceptance)/)' --no-tests=fail

[doc("Run the complete Wave-6 continuous-update acceptance surface")]
[group('test')]
wave6-integration-check:
    cargo nextest run --locked -E 'test(/(wp(23|24|25|26|4[1-8])|wp66_)/)' --no-tests=fail

[doc("Compare Git-accelerated candidates and state with authoritative fallback")]
[group('test')]
git-parity-check:
    cargo nextest run --locked --test integration -E 'test(/(wp(49|50|51|52|53)|wp72_operational_acceptance)/)' --no-tests=fail

[doc("Run the complete WP72 true-rebuild, comparator, Git parity, and process-closure oracle set")]
[group('test')]
wp72-acceptance-check:
    @just packet-oracle-check WP05
    cargo build --manifest-path pyrefly-sidecar/Cargo.toml --locked
    CARGO_TARGET_DIR=target/extractor cargo +nightly-2026-08-18 build --manifest-path rustc-extractor/Cargo.toml --locked
    CODEFABRIC_FULL_REBUILD_PROVIDERS=1 cargo nextest run --locked -E 'test(/(full_golden_scenario_clean_rebuild_equivalence|clean_rebuild_independence_contract|clean_rebuild_equivalence_adversarial|clean_rebuild_operational_gate|wp72_rejects_either_noncurrent_comparison_input|wp72_structural_acceptance|wp72_operational_acceptance)/)' --no-tests=fail

[doc("Run the complete Wave-7 Git-aware lifecycle acceptance surface")]
[group('test')]
wave7-integration-check: git-parity-check rebuild-equivalence-check wp72-acceptance-check source-capture-race-check

[doc("Run the Wave-8 Python-local semantic acceptance slice; WP02-WP07 populate the selector")]
[group('test')]
wave8-integration-check:
    cargo nextest run --locked --lib --no-fail-fast -E 'test(/(py_context_(discovery_conformance|manifest_identity_parity|guess_rejection_falsification|invalidation_operational_gate)|py_scope_binding_fixture_conformance|ruff_semantic_isolation_parity|py_unresolved_reference_unknown_falsification|py_scope_binding_owner_replacement_gate|py_import_export_fixture_conformance|py_import_syntax_semantic_distinction_parity|py_dynamic_export_unknown_falsification|py_module_fact_replacement_gate|py_callable_call_site_fixture_conformance|py_call_site_first_class_parity|py_dynamic_splat_unknown_argument_falsification|py_callable_contract_replacement_gate|py_cfg_fixture_conformance|py_cfg_wellformedness_parity|py_cfg_exceptional_edge_falsification|py_cfg_owner_invalidation_gate|py_defuse_fixture_conformance|py_semantic_profile_partial_parity|py_parse_error_capability_gap_falsification|wave8_integration_operational_gate)$/)' --no-tests=fail

[doc("Run every released Wave-2 through Wave-7 integration gate")]
[group('test')]
wave-acceptance-check: wave2-integration-check wave3-integration-check wave4-integration-check wave5-integration-check wave6-integration-check wave7-integration-check

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

[doc("Build the handwritten model compiler with every generated production output absent")]
[group('gate')]
model-bootstrap-check:
    ./scripts/model_bootstrap_check.sh

[doc("Compile the repository model and exercise Git/fallback/current-byte inventory semantics")]
[group('gate')]
model-inventory-check:
    ./scripts/model_inventory_check.sh

[doc("Verify the owner-accepted released-artifact census against the compiled model")]
[group('gate')]
model-release-census-check:
    ./scripts/model_release_census_check.sh

[doc("Validate typed action keys, affected closure, DesiredTree parity, and zero repository writes")]
[group('gate')]
model-plan-check:
    ./scripts/model_plan_check.sh

[doc("Print the structured read-only model action plan for optional changed paths")]
[group('environment')]
model-plan *paths:
    ./scripts/model_exec.sh plan "$@" --root .

# These private recipes are the small, reviewable profile roots. The model compiler derives
# every transitive capability from Just's live JSON graph; there is no sibling proof manifest.
_model-profile-edit: root-fmt model-plan-check

_model-profile-changed: _model-profile-edit model-family-check model-incremental-check governance-scan root-check root-clippy root-doctest extractor-ci-fast sidecar-ci-fast adapter-ci-fast stable-graph-check

_model-profile-tier-a: _model-profile-changed

_model-profile-release: _model-profile-tier-a model-bootstrap-check model-inventory-check model-release-census-check model-repro-check model-transaction-check model-zero-state-check adapter-wheel-test features-each policy seed-zero-state-check

[doc("Validate the read-only desired tree under a model assurance profile")]
[group('gate')]
model-check profile="edit":
    ./scripts/model_exec.sh check "{{profile}}" --root .

[doc("Validate one model family through typed render, independent decode, and staged consumers")]
[group('gate')]
model-family-check family="":
    ./scripts/model_family_check.sh "{{family}}"

[doc("Generate the complete model DesiredTree twice and compare exact path and byte identity")]
[group('gate')]
model-repro-check:
    ./scripts/model_repro_check.sh

[doc("Validate worktree-local locking, crash recovery, and exact transactional reconciliation")]
[group('gate')]
model-transaction-check:
    ./scripts/model_transaction_check.sh

[doc("Validate content-addressed family caching, affected closure, and watcher widening")]
[group('gate')]
model-incremental-check:
    ./scripts/model_incremental_check.sh

[doc("Compile live assurance evidence and prove conservative model profiles")]
[group('gate')]
model-assurance-check:
    ./scripts/model_assurance_check.sh

[doc("Watch repository hints and recompile the current-byte model after every batch")]
[group('environment')]
model-watch:
    ./scripts/model_exec.sh watch .

[doc("Explain a model artifact ID or repository path")]
[group('environment')]
model-explain target:
    ./scripts/model_exec.sh explain "{{target}}" .

[doc("Run repository structural governance rules")]
[group('gate')]
governance-scan: public-error-closure-check
    ast-grep test
    ast-grep scan \
      --globs '!contracts/generated/**' \
      --globs '!src/generated/**' \
      --globs '!codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/**' \
      --globs '!rustc-extractor/src/generated/**' \
      --globs '!pyrefly-sidecar/src/generated/**'

[doc("Reject public Rust error prefixes outside the generated error registry")]
[group('gate')]
public-error-closure-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_error_registry_closure.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/error_registry_closure.py

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
    cd rustc-extractor && cargo check --all-targets --locked
    cd rustc-extractor && cargo clippy --all-targets --locked -- -D warnings

[doc("Test the dated-nightly rustc extractor")]
[group('extractor')]
extractor-test:
    cd rustc-extractor && cargo test --locked

[doc("Launch the built extractor directly and verify exact stderr-only identity")]
[group('extractor')]
extractor-identity:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd rustc-extractor && cargo build --locked)
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
    cd pyrefly-sidecar && cargo check --all-targets --locked
    cd pyrefly-sidecar && cargo clippy --all-targets --locked -- -D warnings

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
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff format --check codefabric-cpg-mcp
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff check codefabric-cpg-mcp

[doc("Type-check the configured adapter source and test trees")]
[group('adapter')]
adapter-type:
    cd codefabric-cpg-mcp && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen pyrefly check

[doc("Test the locked FastMCP adapter")]
[group('adapter')]
adapter-test:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests

[doc("Test locked-command STDIO startup, shutdown, and protocol silence")]
[group('adapter')]
adapter-stdio-test:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests/test_stdio.py

[doc("Build and import the adapter wheel with its canonical artifact-index resource")]
[group('adapter')]
adapter-wheel-test:
    ./scripts/adapter_wheel_test.sh

[doc("Run the complete adapter gate")]
[group('adapter')]
adapter-ci-fast: adapter-lint adapter-type adapter-test

# -------------------------------------------------------- model governance

[doc("Check formatting and lint for the model compiler and plan-governance helpers")]
[group('gate')]
model-tooling-lint:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff format --check tooling/model tooling/ci
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff check tooling/model tooling/ci

[doc("Validate active plan, review, and schema-2 execution-state contracts")]
[group('gate')]
artifacts-check: model-tooling-lint
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_artifact_contracts.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py artifacts-check

[doc("Validate model-control ownership and single-active-program design contracts")]
[group('gate')]
model-design-contract-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_model_design_contracts.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/model_design_contracts.py

[doc("Validate P1-P25 normative ownership and DP-001-DP-124 packet traceability")]
[group('gate')]
design-principle-traceability-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_design_principle_alignment.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/design_principle_alignment.py traceability-check

[doc("Reject unregistered semantic properties and stale storage mappings before publication")]
[group('gate')]
property-registry-closure-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_property_registry_closure.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/property_registry_closure.py

[doc("Validate governed DB01-DB03 semantic-provider candidates and reviewed transition allows")]
[group('gate')]
semantic-provider-legacy-zero-state-check scope="all":
    ./scripts/semantic_provider_legacy_zero_state.sh "{{scope}}"

[doc("Validate the closed semantic-provider fault seam census")]
[group('gate')]
semantic-fault-point-check:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp python tooling/ci/semantic_provider_contracts.py faults

[doc("Validate bounded semantic-provider telemetry, containment, and shared dispatch")]
[group('gate')]
semantic-observability-contract-check:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp python tooling/ci/semantic_provider_contracts.py observability

[doc("Run reproducible non-normative semantic substrate warm/cold workloads")]
[group('perf')]
semantic-profile-bench:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp python tooling/benchmarks/semantic_profile_bench.py

[doc("Execute all current design-principle detectors, or one DP-NNN detector")]
[group('gate')]
alignment-detector-check detector_id="":
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/design_principle_alignment.py detector-check "{{detector_id}}"

[doc("Reject any dirty, deleted, or untracked path without an explicit owner disposition")]
[group('gate')]
audit-baseline-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/design_principle_alignment.py baseline-check

[doc("Validate governed oracle criteria, substantive definitions, and zero-match-safe selectors")]
[group('gate')]
oracle-substance-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_plan_assurance.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py oracle-substance-check
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py current-packet-oracle-check

[doc("Validate the active packet DAG and disposition every unordered known-touch overlap")]
[group('gate')]
plan-dependency-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py dependency-check

[doc("Validate committed name-coupled nextest selectors and zero-selection failure semantics")]
[group('gate')]
gate-filter-census:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python scripts/gate_filter_census.py check

[doc("Run the pinned ontology-fabric capability probes and emit target-only reports")]
[group('test')]
probe-suite:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python scripts/ontology_fabric_probe_suite.py run

[doc("Prove per-domain ID extensions, lowering, analyzer coverage, and retired generic typing")]
[group('gate')]
id-domain-extension-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP07
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP08

[doc("Execute compiled ontology FK, membership, conformance, cardinality, and one-of gates")]
[group('gate')]
ontology-relational-closure-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP11

[doc("Validate normalized ontology parity, relational closure, and serving decoration")]
[group('gate')]
ontology-dimension-check: ontology-relational-closure-check
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP09
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP10

[doc("Validate logical structure classification and the selected flat source-span lowering")]
[group('gate')]
structure-classification-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP12

[doc("Resolve the complete ontology plane dynamically from a leased catalog")]
[group('gate')]
ontology-self-description-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP17

[doc("Prove atomic Stage-2b acceptance, fault rollback, and idempotent pointer advance")]
[group('gate')]
ontology-stage2b-activation-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check WP17

[doc("Execute every released negative fixture and cited assurance registry")]
[group('gate')]
released-fixture-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_released_fixture_verifier.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/released_fixture_verifier.py

[doc("Validate purpose-classified hash APIs and the semantic fingerprint registry")]
[group('gate')]
digest-domain-contract-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_digest_domain_contracts.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/digest_domain_contracts.py check

[doc("Execute exactly four substantive acceptance oracles for one implementation packet")]
[group('test')]
packet-oracle-check packet:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/plan_assurance.py packet-oracle-check "{{packet}}"

[doc("Derive active-plan input freshness and proving-commit trust")]
[group('gate')]
plan-status:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py plan-status

[doc("Reject Cargo target outputs in the index or reachable HEAD history")]
[group('gate')]
tracked-target-zero-state-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py tracked-target-zero-state-check

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

[doc("Run the exhaustive read-only model release certification")]
[group('gate')]
model-release-check:
    ./scripts/model_release_check.sh

[doc("Validate the sealed inactive Waves successor without changing the active pointer")]
[group('gate')]
model-handoff-check:
    ./scripts/model_handoff_check.sh

[doc("Run model-derived structural, artifact, provenance, and zero-state governance")]
[group('gate')]
governance: governance-scan model-design-contract-check model-assurance-check model-zero-state-check artifacts-check plan-status tracked-target-zero-state-check duplicate-family-check seed-zero-state-check released-fixture-check oracle-substance-check plan-dependency-check design-principle-traceability-check alignment-detector-check

[doc("Run the routine gate across all four build domains")]
[group('gate')]
ci-fast: root-ci-fast extractor-ci-fast sidecar-ci-fast adapter-ci-fast governance

[doc("ci-fast plus policy, the ci nextest profile, and snapshot review state")]
[group('gate')]
ci-pr: ci-fast policy sidecar-policy wave-acceptance-check gate-b-check
    cargo nextest run -P ci
    cargo test --doc
    cargo insta pending-snapshots

# ------------------------------------------------------- coverage / test quality

# Coverage answers what executed, not whether assertions constrain behavior. No percentage
# threshold is configured; section 21.1 warns against adopting one merely because a tool
# supports it.

[doc("Rust line coverage to target/coverage/lcov.info")]
[group('quality')]
coverage:
    mkdir -p target/coverage
    cargo llvm-cov nextest \
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

[doc("Miri UB check on the default toolchain's nightly")]
[group('quality')]
miri:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly miri test

[doc("Miri across a range of randomized seeds")]
[group('quality')]
miri-seeds seeds="16":
    CARGO_TARGET_DIR=target/nightly-assurance MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly miri test

[doc("Compiler-oriented unused-dependency adjudication")]
[group('quality')]
udeps:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly udeps --all-targets --all-features

# Bounded runs only; long campaigns belong in scheduled infrastructure (spec section 23).
# WP06's canonical JSON decoder is the first production-path untrusted-input surface;
# its fuzz harness exercises the same parser and serializer used by the verifier.

[doc("Bounded fuzz run against one target")]
[group('quality')]
fuzz target seconds="60":
    rust_host="$(rustc +nightly -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly fuzz run --target "$rust_host" --target-dir "target/fuzz/$rust_host" \
      {{target}} "$runtime_corpus" -- -max_total_time={{seconds}}

[doc("Coverage of a fuzz corpus")]
[group('quality')]
fuzz-coverage target:
    rust_host="$(rustc +nightly -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly fuzz coverage --target "$rust_host" \
      --target-dir "target/fuzz/$rust_host" {{target}} "$runtime_corpus"

# --------------------------------------------------------- feature / compatibility

# `--all-features` validates only the maximal additive union and can hide accidental
# coupling between features (spec sections 26 and 62.6).

[doc("Check every feature in isolation")]
[group('compat')]
features-each:
    cargo hack check --each-feature

[doc("Check with no default features")]
[group('compat')]
features-no-default:
    cargo hack check --no-default-features

[doc("Verify the declared MSRV (inert until Cargo.toml declares rust-version)")]
[group('compat')]
msrv:
    cargo msrv verify

[doc("Exercise the persisted data-fabric contract in both directions across two revisions")]
[group('compat')]
data-fabric-stack-compat baseline_ref target_ref:
    ./scripts/data_fabric_revision_check.sh compat "{{baseline_ref}}" "{{target_ref}}"

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

[doc("Compare the predeclared data-fabric workload across two revisions")]
[group('perf')]
data-fabric-upgrade-bench baseline_ref target_ref:
    ./scripts/data_fabric_revision_check.sh benchmark "{{baseline_ref}}" "{{target_ref}}"

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
    cargo build --profile profiling

# ------------------------------------------------------------- mutating operations
#
# Everything below changes source, manifests, or the environment. None of it is a
# dependency of any gate. After running one: inspect the diff, identify the semantic
# impact, rerun the relevant validation, and disclose what the tool changed
# (spec section 63).

[confirm("Create schema-2 state and activate the approved plan atomically. Continue?")]
[doc("MUTATES: create validated execution state before switching the active-plan pointer")]
[group('mutating')]
plan-activate plan:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py activate-plan --plan "{{plan}}"

[doc("MUTATES: rewrite Rust formatting in place")]
[group('mutating')]
root-fmt-write:
    cargo fmt --all

[confirm("Reconcile every model-owned Derived output transactionally. Continue?")]
[doc("MUTATES: apply the complete validated model DesiredTree through the sole writer")]
[group('mutating')]
model-sync:
    ./scripts/model_exec.sh sync --confirm .

[doc("MUTATES: emit fixture candidates to an isolated review directory")]
[group('mutating')]
fixture-candidates output_dir="target/fixture-candidates":
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/model/fixture_candidates.py --output-dir "{{output_dir}}"

[doc("MUTATES DISPOSABLE STATE: emit the released-artifact census review candidate under target/")]
[group('mutating')]
model-release-census-candidate:
    ./scripts/model_exec.sh release-census-candidate .

[confirm("Generate the immutable functional Gate B successor for accountable-owner review. Continue?")]
[doc("MUTATES: emit the decoded behavior-first Gate B candidate without accepting or releasing it")]
[group('mutating')]
gate-b-functional-candidate-emit output_dir="tests/golden/review-candidates/codefabric-golden-v4.0.0-candidate.1":
    cargo run --locked --bin codefabric-gate-b-candidate -- emit-functional . target/gate-b-functional-candidate-emit-scratch "{{output_dir}}"

[confirm("Accept the exact reviewed Gate B candidate and publish immutable corpus v2. Continue?")]
[doc("MUTATES: record accountable-owner acceptance and publish immutable Gate B corpus v2 exactly once")]
[group('mutating')]
gate-b-owner-accept candidate_bundle="tests/golden/review-candidates/codefabric-golden-v2.0.0-candidate.1" acceptance_artifact="tests/golden/codefabric-golden-v2/owner-acceptance.json":
    cargo run --locked --bin codefabric-gate-b-candidate -- accept . "{{candidate_bundle}}" "{{acceptance_artifact}}" codefabric-repository-owner "Explicit accountable-owner approval of the exact WP71 candidate bundle recorded in the implementation-plan execution thread"

[confirm("Accept the reviewed released-artifact census as owner authority. Continue?")]
[doc("MUTATES: create the owner-accepted released-artifact census exactly once")]
[group('mutating')]
model-accept kind owner provenance:
    ./scripts/model_exec.sh accept "{{kind}}" --owner "{{owner}}" --provenance "{{provenance}}" --reviewed .

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
