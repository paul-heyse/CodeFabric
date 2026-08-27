// @generated from lane-owned semantic fragments; do not edit.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticLane {
    Shared,
    Python,
    Rust,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticIngestContract {
    pub contract_id: &'static str,
    pub lane: SemanticLane,
    pub observation_schema_ids: &'static [&'static str],
    pub output_table_codes: &'static [i16],
    pub required_fields: &'static [&'static str],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticContextContract {
    pub contract_id: &'static str,
    pub lane: SemanticLane,
    pub context_kinds: &'static [&'static str],
    pub discovery_port: &'static str,
    pub partition_keys: &'static [&'static str],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticInvalidationContract {
    pub contract_id: &'static str,
    pub lane: SemanticLane,
    pub trigger_kinds: &'static [&'static str],
    pub invalidated_table_codes: &'static [i16],
    pub scope: &'static str,
}

pub const SEMANTIC_INGEST_CONTRACTS: &[SemanticIngestContract] = &[
    SemanticIngestContract {
        contract_id: "INGEST_SHARED_FACT_SUBSTRATE_RUFF_V1",
        lane: SemanticLane::Shared,
        observation_schema_ids: &["codefabric.ruff.semantic.v1"],
        output_table_codes: &[100, 110, 130],
        required_fields: &["entity_id", "fact_id", "evidence_id"],
    },
    SemanticIngestContract {
        contract_id: "INGEST_SHARED_CANONICAL_TYPES_V1",
        lane: SemanticLane::Shared,
        observation_schema_ids: &[
            "codefabric.pyrefly.module.v1",
            "codefabric.rustc.owned-mir.v1",
        ],
        output_table_codes: &[180, 190],
        required_fields: &[
            "type_id",
            "type_kind_code",
            "type_role_code",
            "origin_code",
            "certainty_code",
        ],
    },
    SemanticIngestContract {
        contract_id: "INGEST_RUFF_SEMANTIC_V1",
        lane: SemanticLane::Python,
        observation_schema_ids: &["codefabric.ruff.semantic.v1"],
        output_table_codes: &[10, 100, 110, 130, 200, 210, 220, 230, 240, 250, 260, 270],
        required_fields: &[
            "module_name",
            "provider_image_fingerprint",
            "scopes_json",
            "bindings_json",
            "references_json",
            "unknown_symbols_json",
            "edges_json",
            "imports_json",
            "exports_json",
            "export_status",
            "callables_json",
            "parameters_json",
            "callable_syntax_json",
            "call_sites_json",
            "call_arguments_json",
            "unknown_argument_sets_json",
            "members_json",
            "call_diagnostics_json",
        ],
    },
    SemanticIngestContract {
        contract_id: "INGEST_PYREFLY_MODULE_V1",
        lane: SemanticLane::Python,
        observation_schema_ids: &["codefabric.pyrefly.module.v1"],
        output_table_codes: &[180, 190],
        required_fields: &[
            "module_id",
            "module_name",
            "type_table_json",
            "callees_json",
            "diagnostics_json",
        ],
    },
    SemanticIngestContract {
        contract_id: "INGEST_RUSTC_MIR_OWNER_V1",
        lane: SemanticLane::Rust,
        observation_schema_ids: &["codefabric.rustc.owned-mir.v1"],
        output_table_codes: &[180, 190],
        required_fields: &["name", "item_kind", "statement_kinds", "terminator_kinds"],
    },
];

pub const SEMANTIC_CONTEXT_CONTRACTS: &[SemanticContextContract] = &[
    SemanticContextContract {
        contract_id: "CONTEXT_SHARED_DISCOVERY_PORT_V1",
        lane: SemanticLane::Shared,
        context_kinds: &["python", "rust"],
        discovery_port: "crate::analysis_context::AnalysisContextDiscoveryPort",
        partition_keys: &["workspace_id", "analysis_context_id", "source_generation"],
    },
    SemanticContextContract {
        contract_id: "CONTEXT_PYTHON_MANIFEST_V1",
        lane: SemanticLane::Python,
        context_kinds: &["python"],
        discovery_port: "crate::analysis_context::AnalysisContextDiscoveryPort",
        partition_keys: &["workspace_id", "analysis_context_id", "source_generation"],
    },
    SemanticContextContract {
        contract_id: "CONTEXT_RUST_BUILD_UNIT_V1",
        lane: SemanticLane::Rust,
        context_kinds: &["rust"],
        discovery_port: "crate::analysis_context::AnalysisContextDiscoveryPort",
        partition_keys: &["workspace_id", "analysis_context_id", "source_generation"],
    },
];

pub const SEMANTIC_INVALIDATION_CONTRACTS: &[SemanticInvalidationContract] = &[
    SemanticInvalidationContract {
        contract_id: "INVALIDATE_SHARED_SEMANTIC_TYPES_V1",
        lane: SemanticLane::Shared,
        trigger_kinds: &["source-content-change", "context-manifest-change"],
        invalidated_table_codes: &[180, 190],
        scope: "owner-and-analysis-context",
    },
    SemanticInvalidationContract {
        contract_id: "INVALIDATE_PYTHON_MODULE_V1",
        lane: SemanticLane::Python,
        trigger_kinds: &["python-source-change", "python-manifest-change"],
        invalidated_table_codes: &[180, 190],
        scope: "module-owner-and-analysis-context",
    },
    SemanticInvalidationContract {
        contract_id: "INVALIDATE_PYTHON_SCOPE_BINDING_V1",
        lane: SemanticLane::Python,
        trigger_kinds: &["python-source-change", "python-manifest-change"],
        invalidated_table_codes: &[10, 200, 210, 220, 230, 240, 250, 260, 270],
        scope: "module-owner-and-analysis-context",
    },
    SemanticInvalidationContract {
        contract_id: "INVALIDATE_RUST_CRATE_V1",
        lane: SemanticLane::Rust,
        trigger_kinds: &["rust-source-change", "rust-build-manifest-change"],
        invalidated_table_codes: &[180, 190],
        scope: "crate-owner-and-analysis-context",
    },
];
