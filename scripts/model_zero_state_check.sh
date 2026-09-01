#!/usr/bin/env bash
# Permanent reintroduction guard for the superseded catalog/compiler/proof surfaces.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

fail() {
  printf 'model zero-state check failed: %s\n' "$1" >&2
  exit 1
}

for path in \
  contracts/bundles \
  contracts/generated \
  contracts/faults/fault-point-registry.yaml \
  contracts/fixtures/model-index-decode-differential.json \
  contracts/fixtures/model-packs \
  contracts/fixtures/rebuild-comparison-manifest-v1.json \
  contracts/fixtures/registries/enum-flag-v1-vectors.json \
  contracts/adapter/adapter-model-ir.json \
  contracts/manifests/fixture-oracles.json \
  contracts/manifests/requirements.jsonl \
  contracts/manifests/suite-manifest.json \
  contracts/manifests/traceability.jsonl \
  contracts/observability/semantic-provider-telemetry-contract.yaml \
  contracts/governance/design-principle-baseline.yaml \
  contracts/identity/fingerprint-domain-registry.yaml \
  contracts/comparison/comparison-ignore-registry.yaml \
  contracts/registry \
  contracts/registry/design-principle-detector-registry.yaml \
  contracts/registry/design-principle-registry.yaml \
  contracts/registry/model-pack.schema.json \
  contracts/registry/provider-resource-profile-registry.yaml \
  contracts/registry/transformation-pass-registry.yaml \
  contracts/schema/arrow-delta \
  contracts/schema/schema-contract-ir.json \
  contracts/schema/serving-snapshot.schema.json \
  contracts/security/security-corpus-manifest.yaml \
  contracts/semantic-fragments \
  contracts/query/query-form-contract.json \
  contracts/toolchain/toolchain-identity.json \
  src/bin/codefabric_model/main.rs \
  src/bin/codefabric_model/legacy_model_importer.rs \
  src/bin/codefabric-contracts.rs \
  src/contracts/artifacts.rs \
  src/contracts/catalog.rs \
  src/contracts/compiler.rs \
  src/contracts/index.rs \
  src/contracts/models.rs \
  src/contracts/registry_models.rs \
  src/contracts/schema_artifacts.rs \
  src/contracts/schema_models.rs \
  src/domain_conformance.rs \
  src/fabric/epoch.rs \
  src/fabric/mutation.rs \
  src/fabric/publication.rs \
  src/fabric/serving.rs \
  src/fabric/snapshot_catalog.rs \
  src/generated/digest_frames.rs \
  src/generated/fact_row_encoders.rs \
  src/generated/id_domains.rs \
  src/generated/model.rs \
  src/generated/model_identity_recipes.rs \
  src/generated/model_query_forms.rs \
  src/generated/model_schema_tables.rs \
  src/generated/model_semantic_lane_fragments.rs \
  src/generated/provider_raw_kinds.rs \
  src/generated/registries.rs \
  src/generated/ontology_program_bundle.rs \
  src/governed_session.rs \
  src/ontology_activation.rs \
  src/ontology_candidate.rs \
  src/ontology_contract.rs \
  src/ontology_executor.rs \
  src/ontology_gate.rs \
  src/ontology_plane.rs \
  src/ontology_program.rs \
  src/ontology_relational_program.rs \
  src/ontology_rules.rs \
  src/provider_runtime/fixture.rs \
  src/relational_model \
  src/semantic_query.rs \
  src/snapshot_runtime.rs \
  src/gate_b_candidate.rs \
  src/gate_b_candidate \
  src/gate_b_release.rs \
  src/functional_golden.rs \
  src/functional_golden \
  src/functional_scenario.rs \
  src/golden_corpus.rs \
  rules/domain-conformance-exhaustive.yml \
  rules/governed-datafusion-ingress-only.yml \
  rules/model-no-direct-authority-write.yml \
  rules/model-no-legacy-control-plane.yml \
  rules/model-no-positional-cbef-construction.yml \
  rules/model-no-raw-governed-code-or-flag.yml \
  rules/ontology-activation-fastmcp-forbidden.yml \
  rules/ontology-activation-query-ingress-forbidden.yml \
  rules/ontology-candidate-receipt-opaque.yml \
  rules/ontology-candidate-session-sealed.yml \
  rules/ontology-global-result-version-forbidden.yml \
  rules/ontology-operation-dispatch-generic.yml \
  rules/ontology-process-local-activation-forbidden.yml \
  rules/ontology-raw-planner-sealed.yml \
  rules/semantic-code-literal-forbidden.yml \
  rules/semantic-phrase-binding-required.yml \
  rules/semantic-phrase-fallback-forbidden.yml \
  rules/serving-projections-generated-only.yml \
  rustc-extractor/src/generated/digest_frames.rs \
  rule-tests/model-no-direct-authority-write-test.yml \
  rule-tests/model-no-legacy-control-plane-test.yml \
  rule-tests/model-no-positional-cbef-construction-test.yml \
  rule-tests/model-no-raw-governed-code-or-flag-test.yml \
  rule-tests/__snapshots__/model-no-direct-authority-write-snapshot.yml \
  rule-tests/__snapshots__/model-no-legacy-control-plane-snapshot.yml \
  rule-tests/__snapshots__/model-no-positional-cbef-construction-snapshot.yml \
  rule-tests/__snapshots__/model-no-raw-governed-code-or-flag-snapshot.yml \
  rule-tests/serving-projections-generated-only-test.yml \
  rule-tests/__snapshots__/serving-projections-generated-only-snapshot.yml \
  contracts/acceptance/relational-fabric-v1/README.md \
  contracts/acceptance/relational-fabric-v1/acceptance-transaction.json \
  contracts/acceptance/relational-fabric-v1/comparator-manifest.json \
  contracts/acceptance/relational-fabric-v1/expectation.schema.json \
  contracts/acceptance/relational-fabric-v1/expectations.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/adapter-fingerprints.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/adapter-package-data.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/adapter-schemas.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/fingerprints.py \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/index.py \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query-form-contract.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query_forms.py \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/schemas.py \
  codefabric-cpg-mcp/tests/fixtures/production-tool-manifest-v1.json \
  codefabric-cpg-mcp/tests/test_registries.py \
  tooling/contracts/generate_adapter_models.py \
  tooling/model/provider_inventory.rs \
  tooling/model/schema_consumer.rs \
  tooling/ci/model_design_contracts.py \
  tooling/ci/digest_domain_contracts.py \
  tooling/ci/model_handoff.py \
  tooling/ci/error_registry_closure.py \
  tooling/ci/design_principle_alignment.py \
  tooling/ci/released_fixture_verifier.py \
  tooling/ci/relational_fabric_evidence.py \
  tooling/ci/semantic_provider_contracts.py \
  tooling/ci/test_relational_fabric_evidence.py \
  tooling/ci/test_model_design_contracts.py \
  tooling/ci/test_digest_domain_contracts.py \
  tooling/ci/test_model_handoff.py \
  tooling/ci/test_error_registry_closure.py \
  tooling/ci/test_design_principle_alignment.py \
  tooling/ci/test_released_fixture_verifier.py \
  tooling/ci/property_registry_closure.py \
  tooling/ci/test_property_registry_closure.py \
  tooling/ci/test_registry_authority_contracts.py \
  fuzz/fuzz_targets/registry.rs \
  tooling/ci/proof-coverage.json \
  tooling/ci/proof_coverage.py \
  scripts/adapter_contract_governance_check.sh \
  scripts/compilation_units_check.sh \
  scripts/contracts_negative_check.sh \
  scripts/contracts_repro_check.sh \
  scripts/model_exec.sh \
  scripts/model_handoff_check.sh \
  scripts/model_repro_check.sh \
  scripts/model_release_check.sh \
  tests/integration/data_fabric_upgrade.rs; do
  [ ! -e "$path" ] || fail "superseded path remains: $path"
done

if rg -n \
  '(^|[[:space:]])(from|import)[[:space:]]+tooling\.contracts|model-derived grpc_tools|catalog primary semantic identity' \
  tooling/proto/generate.py tooling/proto/toolchain-identity.json \
  src/generated codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated; then
  fail 'retained Protobuf compatibility tooling regained predecessor model authority'
fi

if rg -n \
  '\b(PublicationScope|PublicationPins|PublicationRequest|OwnerPublicationWrite|PublicationTableRecord|CurrentPublicationRecord|PublicationOutcome|PublicationReferenceViolation|PublicationFaultPoint|MutationPhase|MutationPhaseSpec|PreparedMutation|MutationJournal|OwnerMutationRequest|MutationFaultPoint|MutationResult|DeltaAccessProfile|DeltaMaterializationPosture|ProfiledDeltaHandle|DeltaHandleFactory|SnapshotOverlayProviderFactory|EmptySnapshotOverlay|SnapshotConstructionStage|SnapshotConstructionMetrics|SnapshotProviderRecord|SnapshotProviderCatalog|LocalProviderRequest|LocalProviderFactory|WorkspaceNamespace|FabricTable|WorkspaceFabric|CommonRepositoryRecord|bootstrap_workspace|bootstrap_workspace_with_repository|validate_open_table|validate_open_table_retained|exact_provider|exact_provider_retained|publish_canonical|publish_canonical_set|current_publication|abandon_publication)\b' \
  src tests; then
  fail 'retired publication, pointer, mutation, snapshot-provider, or latest-reopen authority remains'
fi

retired_generated_schema_sources=(
  src/schema_registry.rs
  src/id_domain_extensions.rs
)
if rg -n \
  'table_mutation_operation|hot_overlay_manifest|serving_snapshot_manifest|active_snapshot|snapshot_lease|active_pointer_generation|active_serving_snapshot|ontology_(candidate|candidate_exact_table|gate_execution|gate_receipt|gate_artifact|owner_decision|activation_request|acceptance|active_pointer|recovery|result_authority|serving_epoch)|ontology_epoch_identity|result_authority_identity' \
  "${retired_generated_schema_sources[@]}"; then
  fail 'retired mutation, snapshot/current, ontology candidate, gate, activation, pointer, authority, or epoch schema remains'
fi

retired_snapshot_authority_sources=(
  src/operational_store.rs
  src/workspace_registry.rs
  src/source_image.rs
  src/snapshot.rs
  src/identity_recipes.rs
)
if rg -n \
  '\b(hot_overlay_manifest|serving_snapshot_manifest|active_snapshot|snapshot_lease|active_pointer_generation|active_serving_snapshot|ServingSnapshotManifest|ServingSnapshotManifestBody|SnapshotSource|SnapshotBaseTable|SnapshotBasePublication|SnapshotOverlayTable|SnapshotOverlay|SnapshotIndexes|SnapshotBundles|SnapshotActivationRecord|ServingSnapshotFields|SnapshotBaseTableVersions)\b|SourceBlobHolderKind::ServingSnapshot|acquire_serving_snapshot_lease' \
  "${retired_snapshot_authority_sources[@]}"; then
  fail 'retired internal snapshot manifest, pointer, lease, or overlay authority remains'
fi

retired_generated_registry_sources=(
  src/registry_contract_data.rs
  src/registries.rs
)
if rg -n \
  '\b(RegistryDomainEntry|REGISTRY_DOMAINS|PhraseEntry|PHRASE_ENTRIES|PHRASE_IDS|OntologyCodeEntry|ENTITY_KIND_(IDS|CODES)|RELATION_KIND_(IDS|CODES)|PROPERTY_KIND_(IDS|CODES)|FACT_KIND_(IDS|CODES)|UNKNOWN_IDS|PROJECTION_IDS|SUMMARY_PROFILE_IDS|DerivationEntry|DERIVATION_(ENTRIES|IDS)|QUERY_FORM_VALUES|QueryForm|PROVIDER_IDS|PROVIDER_NORMALIZATION_IDS|PROVIDER_RESOURCE_PROFILE_IDS|PUBLIC_ERROR_IDS|ProviderFieldRoleEntry|PROVIDER_FIELD_ROLE_CROSSWALK|ProviderEventMappingEntry|PROVIDER_EVENT_MAPPINGS|FlagEntry|FactFlags|FACT_FLAGS_FLAGS|ProviderNodeFlags|PROVIDER_NODE_FLAGS_FLAGS|OntologyCandidateLifecycle|ONTOLOGY_CANDIDATE_LIFECYCLE_(VALUES|TRANSITIONS)|WORKSPACE_LIFECYCLE_TRANSITIONS|SOURCE_TRUST_STATE_TRANSITIONS|EVENT_STREAM_HEALTH_TRANSITIONS|GIT_ACCELERATION_STATUS_TRANSITIONS|UPDATE_WAVE_STATE_TRANSITIONS|PROVIDER_RUN_STATE_TRANSITIONS|OWNER_CAPABILITY_STATE_TRANSITIONS|DURABLE_PUBLICATION_STATE_TRANSITIONS|SERVING_ACTIVATION_STATE_TRANSITIONS|SNAPSHOT_LEASE_STATE_TRANSITIONS|QUERY_EXECUTION_STATE_TRANSITIONS|ARTIFACT_STATE_TRANSITIONS)\b' \
  "${retired_generated_registry_sources[@]}"; then
  fail 'retired ontology, phrase, query-form, generated-census, flag-shell, or lifecycle registry authority remains'
fi

recipe_names="$(just --dump --dump-format json | jq -r '.recipes | keys[]')"
for recipe in \
  contracts-tooling-lint schema-check adapter-contracts-governance \
  adapter-contracts-check adapter-contracts-repro-check proof-coverage-check \
  compilation-units-check contracts-verify \
  contracts-verify-released contracts-repro-check contracts-gen \
  adapter-contracts-gen model-bootstrap-check model-inventory-check \
  model-release-census-check model-plan-check model-plan model-check \
  model-family-check model-repro-check model-transaction-check \
  model-incremental-check model-assurance-check model-watch model-explain \
  model-release-check model-sync fixture-candidates \
  model-release-census-candidate model-accept model-handoff-check \
  ontology-program-compiler-check ontology-program-packaging-check \
  ontology-calculation-catalog-check ontology-program-causality-check \
  ontology-gate-result-checksum-check ontology-gate-execution-artifact-check \
  ontology-runtime-resource-check id-domain-plan-enforcement-check \
  ontology-candidate-receipt-check ontology-candidate-delta-binding-check \
  ontology-decision-integrity-check ontology-activation-recovery-check \
  ontology-activation-route-check result-authority-lease-check \
  ontology-datafabric-integration-check ontology-datafabric-legacy-zero-state-check \
  ontology-plan-artifact-boundary-check id-domain-extension-check \
  ontology-relational-closure-check ontology-dimension-check \
  ontology-self-description-check model-design-contract-check model-tooling-lint \
  property-registry-closure-check \
  structure-classification-check gate-b-check gate-b-owner-acceptance-check \
  gate-b-candidate-check gate-b-rejected-candidate-zero-state-check \
  gate-b-public-vertical-check gate-b-causal-check \
  gate-b-delivery-equivalence-check gate-b-projection-registry-check \
  gate-b-review-bundle-check gate-b-functional-candidate-check \
  gate-b-predecessor-check gate-b-functional-candidate-emit \
  gate-b-owner-accept semantic-oracle-mutants-check \
  functional-golden-contract-check functional-golden-independence-check \
  rebuild-equivalence-check wp72-acceptance-check \
  design-principle-traceability-check alignment-detector-check \
  audit-baseline-check released-fixture-check semantic-fault-point-check \
  semantic-observability-contract-check public-error-closure-check \
  semantic-provider-legacy-zero-state-check digest-domain-contract-check; do
  if printf '%s\n' "$recipe_names" | rg -qx "$recipe"; then
    fail "superseded recipe remains: $recipe"
  fi
done
if printf '%s\n' "$recipe_names" | rg -q '^mutants-wp'; then
  fail 'packet-specific mutation recipe remains'
fi

if rg -n \
  '\bFabricEpochBuilder\b|\bProviderAdmissionOutcome\b|\bpub fn admit_provider_relations[[:space:]]*\(|\bModelEpoch\b|\bReplayEngine\b|\bSchemaContractModelRows\b|\bPythonModelPack\b|\benabled_model_packs\b' \
  src/provider_admission.rs src/fabric src/*.rs; then
  fail 'predecessor model, epoch, schema-projection, or provider-admission seam remains'
fi

programmatic_epoch_consumers=(
  src/provider_admission.rs
  src/programmatic_derived_analysis.rs
  src/fabric/activation_control_delta.rs
  src/fabric/child_session.rs
  src/fabric/programmatic_epoch.rs
  src/fabric/programmatic_observation_delta.rs
  src/fabric/programmatic_workspace.rs
  src/fabric/programmatic_workspace_vertical_tests.rs
  src/fabric/relational_query_runtime.rs
)
if rg -n \
  '(crate::fabric::epoch|super::epoch|super::super::epoch)::?[^;]*(FABRIC_CATALOG|FabricEpochId|FabricEpochRuntimeConfig|FabricSchemaRole)' \
  "${programmatic_epoch_consumers[@]}"; then
  fail 'programmatic consumer still imports authority-neutral contracts from predecessor epoch owner'
fi

scan_roots=(
  Cargo.toml justfile README.md AGENTS.md .github scripts src tooling/proto tooling/ci
  codefabric-cpg-mcp/src codefabric-cpg-mcp/pyproject.toml docs/authoritative_design docs/spec_index
)
if rg -n \
  'contracts-tooling|target/debug/codefabric-contracts|tooling/contracts/generate_adapter_models\.py|artifact-index\.json|PUBLIC_SCHEMA_ARTIFACTS|sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests' \
  "${scan_roots[@]}" \
  -g '!scripts/model_zero_state_check.sh' \
  -g '!scripts/stable_graph_check.sh' \
  -g '!tooling/ci/test_*.py' \
  -g '!**/__pycache__/**' >/dev/null; then
  rg -n \
    'contracts-tooling|target/debug/codefabric-contracts|tooling/contracts/generate_adapter_models\.py|artifact-index\.json|PUBLIC_SCHEMA_ARTIFACTS|sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests' \
    "${scan_roots[@]}" \
    -g '!scripts/model_zero_state_check.sh' \
    -g '!scripts/stable_graph_check.sh' \
    -g '!tooling/ci/test_*.py' \
    -g '!**/__pycache__/**' >&2
  fail 'superseded control-plane text remains in a live surface'
fi

ast-grep test --skip-snapshot-tests >/dev/null

# Repository-wide governance findings are owned by `just governance-scan`. This check is the
# permanent, zero-match-safe guard for the retired model/ontology authority and must not conflate
# an unrelated structural diagnostic with predecessor reintroduction.

printf 'model zero-state check passed\n'
