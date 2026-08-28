// @generated from codefabric.schema.contract-ir b3:92daa3bdca698f0dcdc09014c9e31c87220f9ec7ffc9f888d057f6973fd5109c; schema-contract-driver-v1; do not edit.

define_id_domain_extension!(
    WorkspaceIdExtension,
    "workspace",
    "codefabric.workspace_id",
    "codefabric.identity.workspace_v1",
    "1"
);
define_id_domain_extension!(
    RepositoryIdExtension,
    "repository",
    "codefabric.repository_id",
    "codefabric.identity.repository_v1",
    "1"
);
define_id_domain_extension!(
    WorktreeIdExtension,
    "worktree",
    "codefabric.worktree_id",
    "codefabric.identity.worktree_v1",
    "1"
);
define_id_domain_extension!(
    AnalysisContextIdExtension,
    "analysis_context",
    "codefabric.analysis_context_id",
    "codefabric.identity.analysis_context_v1",
    "1"
);
define_id_domain_extension!(
    AnalysisContextSetIdExtension,
    "analysis_context_set",
    "codefabric.analysis_context_set_id",
    "codefabric.identity.context_set_v1",
    "1"
);
define_id_domain_extension!(
    SourceFileIdExtension,
    "source_file",
    "codefabric.source_file_id",
    "codefabric.identity.source_file_v1",
    "1"
);
define_id_domain_extension!(
    OwnerIdExtension,
    "owner",
    "codefabric.owner_id",
    "codefabric.identity.owner_v1",
    "1"
);
define_id_domain_extension!(
    EntityIdExtension,
    "entity",
    "codefabric.entity_id",
    "codefabric.identity.entity_v1",
    "1"
);
define_id_domain_extension!(
    FactIdExtension,
    "fact",
    "codefabric.fact_id",
    "codefabric.identity.fact_union_v1",
    "1"
);
define_id_domain_extension!(
    TypeIdExtension,
    "type",
    "codefabric.type_id",
    "codefabric.identity.type_v1",
    "1"
);
define_id_domain_extension!(
    PublicationIdExtension,
    "publication",
    "codefabric.publication_id",
    "codefabric.identity.publication_v1",
    "1"
);
define_id_domain_extension!(
    ServingSnapshotIdExtension,
    "serving_snapshot",
    "codefabric.serving_snapshot_id",
    "codefabric.identity.serving_snapshot_v1",
    "1"
);
define_id_domain_extension!(
    ResultArtifactIdExtension,
    "result_artifact",
    "codefabric.result_artifact_id",
    "codefabric.identity.result_artifact_v1",
    "1"
);
define_id_domain_extension!(
    SourceContextIdExtension,
    "source_context",
    "codefabric.source_context_id",
    "codefabric.identity.source_context_v1",
    "1"
);
define_id_domain_extension!(
    EvidenceIdExtension,
    "evidence",
    "codefabric.evidence_id",
    "codefabric.identity.fact_evidence_v1",
    "1"
);
define_id_domain_extension!(
    DiagnosticIdExtension,
    "diagnostic",
    "codefabric.diagnostic_id",
    "codefabric.identity.diagnostic_v1",
    "1"
);
define_id_domain_extension!(
    ProviderRunIdExtension,
    "provider_run",
    "codefabric.provider_run_id",
    "codefabric.identity.provider_run_v1",
    "1"
);
define_id_domain_extension!(
    ObservationIdExtension,
    "observation",
    "codefabric.observation_id",
    "codefabric.identity.source_observation_v1",
    "1"
);
define_id_domain_extension!(
    PathIdExtension,
    "path",
    "codefabric.path_id",
    "codefabric.identity.result_path_v1",
    "1"
);
define_id_domain_extension!(
    BindingIdExtension,
    "binding",
    "codefabric.binding_id",
    "codefabric.identity.result_binding_v1",
    "1"
);
define_id_domain_extension!(
    GroupIdExtension,
    "group",
    "codefabric.group_id",
    "codefabric.identity.result_group_v1",
    "1"
);

define_hash32_extension!();

const GENERATED_ID_DOMAINS: &[GeneratedIdDomainSpec] = &[
    GeneratedIdDomainSpec {
        domain_slug: "workspace",
        extension_name: "codefabric.workspace_id",
        rust_type: "WorkspaceIdExtension",
        preimage_recipe_id: "codefabric.identity.workspace_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "repository",
        extension_name: "codefabric.repository_id",
        rust_type: "RepositoryIdExtension",
        preimage_recipe_id: "codefabric.identity.repository_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "worktree",
        extension_name: "codefabric.worktree_id",
        rust_type: "WorktreeIdExtension",
        preimage_recipe_id: "codefabric.identity.worktree_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "analysis_context",
        extension_name: "codefabric.analysis_context_id",
        rust_type: "AnalysisContextIdExtension",
        preimage_recipe_id: "codefabric.identity.analysis_context_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "analysis_context_set",
        extension_name: "codefabric.analysis_context_set_id",
        rust_type: "AnalysisContextSetIdExtension",
        preimage_recipe_id: "codefabric.identity.context_set_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "source_file",
        extension_name: "codefabric.source_file_id",
        rust_type: "SourceFileIdExtension",
        preimage_recipe_id: "codefabric.identity.source_file_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "owner",
        extension_name: "codefabric.owner_id",
        rust_type: "OwnerIdExtension",
        preimage_recipe_id: "codefabric.identity.owner_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "entity",
        extension_name: "codefabric.entity_id",
        rust_type: "EntityIdExtension",
        preimage_recipe_id: "codefabric.identity.entity_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "fact",
        extension_name: "codefabric.fact_id",
        rust_type: "FactIdExtension",
        preimage_recipe_id: "codefabric.identity.fact_union_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "type",
        extension_name: "codefabric.type_id",
        rust_type: "TypeIdExtension",
        preimage_recipe_id: "codefabric.identity.type_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "publication",
        extension_name: "codefabric.publication_id",
        rust_type: "PublicationIdExtension",
        preimage_recipe_id: "codefabric.identity.publication_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "serving_snapshot",
        extension_name: "codefabric.serving_snapshot_id",
        rust_type: "ServingSnapshotIdExtension",
        preimage_recipe_id: "codefabric.identity.serving_snapshot_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "result_artifact",
        extension_name: "codefabric.result_artifact_id",
        rust_type: "ResultArtifactIdExtension",
        preimage_recipe_id: "codefabric.identity.result_artifact_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "source_context",
        extension_name: "codefabric.source_context_id",
        rust_type: "SourceContextIdExtension",
        preimage_recipe_id: "codefabric.identity.source_context_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "evidence",
        extension_name: "codefabric.evidence_id",
        rust_type: "EvidenceIdExtension",
        preimage_recipe_id: "codefabric.identity.fact_evidence_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "diagnostic",
        extension_name: "codefabric.diagnostic_id",
        rust_type: "DiagnosticIdExtension",
        preimage_recipe_id: "codefabric.identity.diagnostic_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "provider_run",
        extension_name: "codefabric.provider_run_id",
        rust_type: "ProviderRunIdExtension",
        preimage_recipe_id: "codefabric.identity.provider_run_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "observation",
        extension_name: "codefabric.observation_id",
        rust_type: "ObservationIdExtension",
        preimage_recipe_id: "codefabric.identity.source_observation_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "path",
        extension_name: "codefabric.path_id",
        rust_type: "PathIdExtension",
        preimage_recipe_id: "codefabric.identity.result_path_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "binding",
        extension_name: "codefabric.binding_id",
        rust_type: "BindingIdExtension",
        preimage_recipe_id: "codefabric.identity.result_binding_v1",
        preimage_version: "1",
    },
    GeneratedIdDomainSpec {
        domain_slug: "group",
        extension_name: "codefabric.group_id",
        rust_type: "GroupIdExtension",
        preimage_recipe_id: "codefabric.identity.result_group_v1",
        preimage_version: "1",
    },
];

fn attach_generated_id_domain(field: Field, domain: &str) -> Result<Field, ArrowError> {
    match domain {
        "workspace" => Ok(field.with_extension_type(WorkspaceIdExtension::v1())),
        "repository" => Ok(field.with_extension_type(RepositoryIdExtension::v1())),
        "worktree" => Ok(field.with_extension_type(WorktreeIdExtension::v1())),
        "analysis_context" => Ok(field.with_extension_type(AnalysisContextIdExtension::v1())),
        "analysis_context_set" => {
            Ok(field.with_extension_type(AnalysisContextSetIdExtension::v1()))
        }
        "source_file" => Ok(field.with_extension_type(SourceFileIdExtension::v1())),
        "owner" => Ok(field.with_extension_type(OwnerIdExtension::v1())),
        "entity" => Ok(field.with_extension_type(EntityIdExtension::v1())),
        "fact" => Ok(field.with_extension_type(FactIdExtension::v1())),
        "type" => Ok(field.with_extension_type(TypeIdExtension::v1())),
        "publication" => Ok(field.with_extension_type(PublicationIdExtension::v1())),
        "serving_snapshot" => Ok(field.with_extension_type(ServingSnapshotIdExtension::v1())),
        "result_artifact" => Ok(field.with_extension_type(ResultArtifactIdExtension::v1())),
        "source_context" => Ok(field.with_extension_type(SourceContextIdExtension::v1())),
        "evidence" => Ok(field.with_extension_type(EvidenceIdExtension::v1())),
        "diagnostic" => Ok(field.with_extension_type(DiagnosticIdExtension::v1())),
        "provider_run" => Ok(field.with_extension_type(ProviderRunIdExtension::v1())),
        "observation" => Ok(field.with_extension_type(ObservationIdExtension::v1())),
        "path" => Ok(field.with_extension_type(PathIdExtension::v1())),
        "binding" => Ok(field.with_extension_type(BindingIdExtension::v1())),
        "group" => Ok(field.with_extension_type(GroupIdExtension::v1())),
        value => Err(ArrowError::InvalidArgumentError(format!(
            "unknown generated ID domain {value}"
        ))),
    }
}

fn generated_id_domain_registrations() -> Vec<ExtensionTypeRegistrationRef> {
    vec![
        id_domain_registration::<WorkspaceIdExtension>(),
        id_domain_registration::<RepositoryIdExtension>(),
        id_domain_registration::<WorktreeIdExtension>(),
        id_domain_registration::<AnalysisContextIdExtension>(),
        id_domain_registration::<AnalysisContextSetIdExtension>(),
        id_domain_registration::<SourceFileIdExtension>(),
        id_domain_registration::<OwnerIdExtension>(),
        id_domain_registration::<EntityIdExtension>(),
        id_domain_registration::<FactIdExtension>(),
        id_domain_registration::<TypeIdExtension>(),
        id_domain_registration::<PublicationIdExtension>(),
        id_domain_registration::<ServingSnapshotIdExtension>(),
        id_domain_registration::<ResultArtifactIdExtension>(),
        id_domain_registration::<SourceContextIdExtension>(),
        id_domain_registration::<EvidenceIdExtension>(),
        id_domain_registration::<DiagnosticIdExtension>(),
        id_domain_registration::<ProviderRunIdExtension>(),
        id_domain_registration::<ObservationIdExtension>(),
        id_domain_registration::<PathIdExtension>(),
        id_domain_registration::<BindingIdExtension>(),
        id_domain_registration::<GroupIdExtension>(),
        hash32_registration(),
    ]
}
