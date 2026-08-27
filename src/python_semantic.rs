//! Canonical Arrow projection for application-owned Ruff semantic observations.
//!
//! Ruff-owned arenas and indices have already been removed by [`crate::ruff_adapter`].
//! This boundary re-keys adapter-local identifiers into one owner-scoped canonical
//! namespace, materializes explicit unknowns, and validates every generated batch
//! before it can reach publication.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use serde_json::{Value, json};

use crate::core_facts::{
    CapabilityChild, aggregate_capability, registered_provider_observation_arrow_schema,
    validate_provider_semantic_observation,
};
use crate::fact_ingest::{
    AccessPathComponentRow, BindingDetailRow, CallArgumentDetailRow, CallSiteDetailRow,
    CallableDetailRow, CanonicalIngestOutput, CanonicalReconciliationEngine, CapabilityStatusRow,
    CfgEdgeDetailRow, CfgGraphRow, CfgNodeDetailRow, DataflowEventDetailRow, DiagnosticRow,
    EntityRow, FactEvidenceRow, FactIngestError, FactScope, MemoryLocationDetailRow,
    ModuleImportDetailRow, OperationDetailRow, OwnerRow, ParameterDetailRow, ProviderFactBatch,
    ProviderFactManifest, ProviderFactStream, ReferenceDetailRow, RelationRow, ScopeDetailRow,
    StreamTerminal, ValidatedFactBatch, ValueDetailRow, encode_access_path_components,
    encode_binding_details, encode_call_argument_details, encode_call_site_details,
    encode_callable_details, encode_capability_statuses, encode_cfg_edge_details,
    encode_cfg_graphs, encode_cfg_node_details, encode_dataflow_event_details,
    encode_diagnostics, encode_entities, encode_evidence, encode_memory_location_details,
    encode_module_import_details, encode_operation_details, encode_owners,
    encode_parameter_details, encode_reference_details, encode_relations, encode_scope_details,
    encode_value_details,
};
use crate::registries::{
    ArgumentBindingStatus, ArgumentSpreadKind, CallDispatchKind, Completeness, CompletenessState,
    Directness, EvidenceCertainty, Language, OwnerCapabilityState, OwnerKind, ParameterKind,
    ProviderCode, ResolutionClass, Severity, capability_code, capability_mask, entity_kind,
    fact_kind_code, relation_kind,
};
use crate::ruff_adapter::{
    PythonArgumentBindingStatus, PythonArgumentSpreadKind, PythonBindingKind,
    PythonCallableSyntaxRole, PythonCfgKind, PythonCfgNodeKind, PythonDataflowEventKind,
    PythonDataflowRelationKind, PythonDispatchKind, PythonExportStatus, PythonFrontendBatch,
    PythonImportKind, PythonMemberKind, PythonParameterKind, PythonReferenceClass,
    PythonResolution, PythonScopeKind, PythonSemanticEdgeKind, PythonTargetForm,
    validate_python_cfg,
};

const OBSERVATION_SCHEMA_ID: &str = "codefabric.ruff.semantic.v3";
const PROVIDER_VERSION: &str = "0.0.7";
const PYTHON_DATAFLOW_DERIVATION_CODE: i16 = 20;

#[derive(Clone, Copy)]
struct DerivedIdentity {
    id: [u8; 16],
    digest: [u8; 32],
}

/// Fully validated owner-scoped result of one Ruff semantic observation.
#[derive(Debug)]
pub struct PythonSemanticProjection {
    pub provider_run_id: [u8; 16],
    pub observation: RecordBatch,
    pub canonical: CanonicalIngestOutput,
    pub profile_completeness: Completeness,
}

impl PythonSemanticProjection {
    /// Read one validated generated table by stable table code.
    #[must_use]
    pub fn batch(&self, table_code: i16) -> Option<&ValidatedFactBatch> {
        self.canonical.batches.get(&table_code)
    }
}

/// Project a complete application-owned Ruff batch into canonical facts.
///
/// The caller supplies the authoritative module owner and source-file identity. Adapter-local
/// IDs are re-keyed with that owner, preventing identical files from colliding while retaining
/// deterministic identity for unchanged owners.
///
/// # Errors
///
/// Returns a protocol or generated batch-validation error if the observation is incomplete,
/// references an absent target, overflows canonical coordinates, or diverges from any generated
/// schema/registry contract.
#[allow(clippy::too_many_lines)] // The projection deliberately keeps the full evidence closure visible.
pub fn project_ruff_semantic_batch(
    scope: FactScope,
    file_id: [u8; 16],
    batch: &PythonFrontendBatch,
) -> Result<PythonSemanticProjection, FactIngestError> {
    if batch.terminal.terminal_state != "completed" || batch.terminal.failure_code.is_some() {
        return Err(FactIngestError::Protocol(format!(
            "Ruff semantic observation is not complete: terminal_state={} failure_code={:?}",
            batch.terminal.terminal_state, batch.terminal.failure_code
        )));
    }
    validate_reference_edges(batch)?;
    validate_import_export_facts(batch)?;
    validate_callable_facts(batch)?;
    validate_cfg_facts(batch)?;
    validate_dataflow_facts(batch)?;

    let observation_payloads = observation_payloads(batch)?;
    let observation = observation_batch(batch, &observation_payloads)?;
    validate_provider_semantic_observation(OBSERVATION_SCHEMA_ID, &observation)?;

    let provider_run = derived_identity(
        b"provider-run",
        &[
            &scope.workspace_id,
            &scope.analysis_context_id,
            &scope.owner_id,
            &scope.source_generation.to_be_bytes(),
            batch.module_name.as_bytes(),
            batch.provider_image_fingerprint.as_bytes(),
        ],
    );

    let mut canonical_ids = BTreeMap::new();
    for semantic in batch
        .scopes
        .iter()
        .map(|fact| (b"scope".as_slice(), fact.scope_id))
        .chain(
            batch
                .bindings
                .iter()
                .map(|fact| (b"binding".as_slice(), fact.binding_id)),
        )
        .chain(
            batch
                .references
                .iter()
                .map(|fact| (b"reference".as_slice(), fact.reference_id)),
        )
        .chain(
            batch
                .unknown_symbols
                .iter()
                .map(|fact| (b"unknown-symbol".as_slice(), fact.unknown_symbol_id)),
        )
    {
        let identity = derived_identity(semantic.0, &[&scope.owner_id, &semantic.1]);
        if canonical_ids.insert(semantic.1, identity).is_some() {
            return Err(FactIngestError::Protocol(
                "Ruff semantic observation reused one ID across fact forms".into(),
            ));
        }
    }
    for (domain, semantic) in std::iter::once((b"module".as_slice(), batch.module_id))
        .chain(batch.imports.iter().flat_map(|fact| {
            [
                (b"import-declaration".as_slice(), fact.import_id),
                (b"target-module".as_slice(), fact.target_module_id),
            ]
            .into_iter()
            .chain(
                fact.imported_entity_id
                    .map(|id| (b"imported-symbol".as_slice(), id)),
            )
        }))
        .chain(
            batch
                .exports
                .iter()
                .map(|fact| (b"export".as_slice(), fact.export_id)),
        )
        .chain(
            batch
                .callables
                .iter()
                .map(|fact| (b"callable".as_slice(), fact.callable_id)),
        )
        .chain(
            batch
                .parameters
                .iter()
                .map(|fact| (b"parameter".as_slice(), fact.parameter_id)),
        )
        .chain(
            batch
                .callable_syntax
                .iter()
                .map(|fact| (b"callable-syntax".as_slice(), fact.syntax_id)),
        )
        .chain(
            batch
                .call_sites
                .iter()
                .map(|fact| (b"call-site".as_slice(), fact.call_site_id)),
        )
        .chain(
            batch
                .call_arguments
                .iter()
                .map(|fact| (b"call-argument".as_slice(), fact.argument_id)),
        )
        .chain(batch.unknown_argument_sets.iter().map(|fact| {
            (
                b"unknown-argument-set".as_slice(),
                fact.unknown_argument_set_id,
            )
        }))
        .chain(
            batch
                .members
                .iter()
                .map(|fact| (b"member".as_slice(), fact.member_id)),
        )
        .chain(
            batch
                .cfgs
                .iter()
                .map(|fact| (b"cfg".as_slice(), fact.cfg_id)),
        )
        .chain(
            batch
                .cfg_nodes
                .iter()
                .map(|fact| (b"cfg-node".as_slice(), fact.cfg_node_id)),
        )
        .chain(
            batch
                .values
                .iter()
                .map(|fact| (b"dataflow-value".as_slice(), fact.value_id)),
        )
        .chain(
            batch
                .operations
                .iter()
                .map(|fact| (b"dataflow-operation".as_slice(), fact.operation_id)),
        )
        .chain(batch.dataflow_events.iter().map(|fact| {
            let domain = if fact.kind == PythonDataflowEventKind::Definition {
                b"definition-event".as_slice()
            } else {
                b"use-event".as_slice()
            };
            (domain, fact.event_id)
        }))
        .chain(
            batch
                .memory_locations
                .iter()
                .map(|fact| (b"memory-location".as_slice(), fact.location_id)),
        )
        .chain(batch.access_path_components.iter().map(|fact| {
            (b"access-path-component".as_slice(), fact.component_id)
        }))
    {
        canonical_ids
            .entry(semantic)
            .or_insert_with(|| derived_identity(domain, &[&scope.owner_id, &semantic]));
    }

    let module_kind = required_entity_kind("MODULE")?;
    let scope_kind = required_entity_kind("SCOPE")?;
    let declaration_kind = required_entity_kind("DECLARATION")?;
    let import_declaration_kind = required_entity_kind("IMPORT_DECLARATION")?;
    let import_binding_kind = required_entity_kind("IMPORT_BINDING")?;
    let export_entity_kind = required_entity_kind("EXPORT")?;
    let reexport_entity_kind = required_entity_kind("REEXPORT")?;
    let symbol_kind = required_entity_kind("SYMBOL")?;
    let reference_kind = required_entity_kind("REFERENCE")?;
    let unknown_kind = required_entity_kind("UNKNOWN")?;
    let callable_kind = required_entity_kind("CALLABLE")?;
    let parameter_kind = required_entity_kind("PARAMETER")?;
    let call_site_kind = required_entity_kind("CALL_SITE")?;
    let argument_kind = required_entity_kind("ARGUMENT")?;
    let unknown_argument_set_kind = required_entity_kind("UNKNOWN_ARGUMENT_SET")?;
    let expression_kind = required_entity_kind("EXPRESSION")?;
    let argument_syntax_kind = required_entity_kind("ARGUMENT_SYNTAX")?;
    let call_expression_kind = required_entity_kind("CALL_EXPRESSION")?;
    let declaration_syntax_kind = required_entity_kind("DECLARATION_SYNTAX")?;
    let cfg_block_kind = required_entity_kind("CFG_BLOCK")?;
    let value_entity_kind = required_entity_kind("VALUE")?;
    let operation_entity_kind = required_entity_kind("DATAFLOW_OPERATION")?;
    let definition_event_kind = required_entity_kind("DEFINITION_EVENT")?;
    let use_event_kind = required_entity_kind("USE_EVENT")?;
    let memory_location_kind = required_entity_kind("MEMORY_LOCATION")?;
    let semantic_type_kind = required_entity_kind("SEMANTIC_TYPE")?;
    let contains_kind = required_relation_kind("CONTAINS")?;
    let declares_kind = required_relation_kind("DECLARES")?;
    let refers_to_kind = required_relation_kind("REFERS_TO")?;
    let imports_module_kind = required_relation_kind("IMPORTS_MODULE")?;
    let imports_symbol_kind = required_relation_kind("IMPORTS_SYMBOL")?;
    let export_relation_kind = required_relation_kind("EXPORTS")?;
    let reexport_relation_kind = required_relation_kind("REEXPORTS")?;
    let aliases_kind = required_relation_kind("ALIASES")?;
    let defined_in_module_kind = required_relation_kind("DEFINED_IN_MODULE")?;
    let depends_on_module_kind = required_relation_kind("DEPENDS_ON_MODULE")?;
    let has_parameter_kind = required_relation_kind("HAS_PARAMETER")?;
    let has_type_parameter_kind = required_relation_kind("HAS_TYPE_PARAMETER")?;
    let has_decorator_kind = required_relation_kind("HAS_DECORATOR")?;
    let has_return_annotation_kind = required_relation_kind("HAS_RETURN_ANNOTATION")?;
    let has_annotation_kind = required_relation_kind("HAS_ANNOTATION")?;
    let has_default_kind = required_relation_kind("HAS_DEFAULT")?;
    let has_callee_expression_kind = required_relation_kind("HAS_CALLEE_EXPRESSION")?;
    let has_receiver_kind = required_relation_kind("HAS_RECEIVER")?;
    let has_argument_kind = required_relation_kind("HAS_ARGUMENT")?;
    let argument_binds_to_kind = required_relation_kind("ARGUMENT_BINDS_TO")?;
    let contains_call_kind = required_relation_kind("CONTAINS_CALL")?;
    let declares_member_kind = required_relation_kind("DECLARES_MEMBER")?;
    let reaching_definition_kind = required_relation_kind("REACHING_DEFINITION")?;
    let reaches_kind = required_relation_kind("REACHES")?;
    let def_use_kind = required_relation_kind("DEF_USE")?;
    let data_dep_kind = required_relation_kind("DATA_DEP")?;
    let value_flows_to_kind = required_relation_kind("VALUE_FLOWS_TO")?;
    let kills_definition_kind = required_relation_kind("KILLS_DEFINITION")?;

    let mut entities = Vec::with_capacity(
        batch.scopes.len()
            + batch.bindings.len()
            + batch.references.len()
            + batch.unknown_symbols.len()
            + batch.callables.len()
            + batch.parameters.len()
            + batch.callable_syntax.len()
            + batch.call_sites.len()
            + batch.call_arguments.len()
            + batch.unknown_argument_sets.len()
            + batch.members.len()
            + batch.cfg_nodes.len()
            + batch.values.len()
            + batch.operations.len()
            + batch.dataflow_events.len()
            + batch.memory_locations.len()
            + batch
                .cfg_edges
                .iter()
                .filter_map(|edge| edge.exception_category)
                .collect::<BTreeSet<_>>()
                .len(),
    );
    let mut scope_details = Vec::with_capacity(batch.scopes.len());
    let mut binding_details = Vec::with_capacity(batch.bindings.len());
    let mut reference_details = Vec::with_capacity(batch.references.len());
    let mut module_import_details = Vec::with_capacity(batch.imports.len());
    let mut callable_details = Vec::with_capacity(batch.callables.len());
    let mut parameter_details = Vec::with_capacity(batch.parameters.len());
    let mut call_site_details = Vec::with_capacity(batch.call_sites.len());
    let mut call_argument_details = Vec::with_capacity(batch.call_arguments.len());
    let mut cfg_graphs = Vec::with_capacity(batch.cfgs.len());
    let mut cfg_node_details = Vec::with_capacity(batch.cfg_nodes.len());
    let mut cfg_edge_details = Vec::with_capacity(batch.cfg_edges.len());
    let mut value_details = Vec::with_capacity(batch.values.len());
    let mut operation_details = Vec::with_capacity(batch.operations.len());
    let mut dataflow_event_details = Vec::with_capacity(batch.dataflow_events.len());
    let mut memory_location_details = Vec::with_capacity(batch.memory_locations.len());
    let mut access_path_components = Vec::with_capacity(batch.access_path_components.len());

    let module_identity = canonical_identity(&canonical_ids, batch.module_id, "module")?;
    entities.push(EntityRow {
        scope,
        entity_id: module_identity.id,
        language: Language::Python as i16,
        entity_family_code: module_kind.family_code,
        entity_kind_code: module_kind.code,
        raw_kind_code: None,
        file_id: Some(file_id),
        start_byte: None,
        end_byte: None,
        name: Some(batch.module_name.clone()),
        qualified_name: Some(batch.module_name.clone()),
        parent_entity_id: None,
        type_id: None,
        flags: 0,
        fact_hash64: digest_hash64(module_identity.digest),
    });

    for fact in &batch.scopes {
        let identity = canonical_identity(&canonical_ids, fact.scope_id, "scope")?;
        let parent = fact
            .parent_scope_id
            .map(|id| canonical_identity(&canonical_ids, id, "parent scope").map(|item| item.id))
            .transpose()?
            .or((fact.kind == PythonScopeKind::Module).then_some(module_identity.id));
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: scope_kind.family_code,
            entity_kind_code: scope_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: fact.name.clone(),
            qualified_name: fact.name.clone(),
            parent_entity_id: parent,
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        scope_details.push(ScopeDetailRow {
            scope,
            scope_id: identity.id,
            parent_scope_id: parent,
            scope_kind: scope_kind_name(fact.kind).into(),
            name: fact.name.clone(),
            start_byte: start,
            end_byte: end,
        });
    }

    for fact in &batch.bindings {
        let identity = canonical_identity(&canonical_ids, fact.binding_id, "binding")?;
        let owner_scope = canonical_identity(&canonical_ids, fact.scope_id, "binding scope")?.id;
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        let binding_entity_kind = if fact.target_form == PythonTargetForm::ImportAlias {
            import_binding_kind
        } else {
            declaration_kind
        };
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: binding_entity_kind.family_code,
            entity_kind_code: binding_entity_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        binding_details.push(BindingDetailRow {
            scope,
            binding_id: identity.id,
            scope_id: owner_scope,
            name: fact.name.clone(),
            binding_kind: binding_kind_name(fact.kind).into(),
            target_form: target_form_name(fact.target_form).into(),
            start_byte: start,
            end_byte: end,
        });
    }

    for fact in &batch.references {
        let identity = canonical_identity(&canonical_ids, fact.reference_id, "reference")?;
        let owner_scope = canonical_identity(&canonical_ids, fact.scope_id, "reference scope")?.id;
        let target = canonical_identity(&canonical_ids, fact.target_id, "reference target")?.id;
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: reference_kind.family_code,
            entity_kind_code: reference_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        reference_details.push(ReferenceDetailRow {
            scope,
            reference_id: identity.id,
            scope_id: owner_scope,
            target_id: target,
            name: fact.name.clone(),
            reference_class: reference_class_name(fact.class).into(),
            resolution: resolution_name(fact.resolution).into(),
            start_byte: start,
            end_byte: end,
            unknown_reason_code: fact.unknown_reason_code.clone(),
        });
    }

    for fact in &batch.unknown_symbols {
        let identity =
            canonical_identity(&canonical_ids, fact.unknown_symbol_id, "unknown symbol")?;
        let owner_scope =
            canonical_identity(&canonical_ids, fact.scope_id, "unknown-symbol scope")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: unknown_kind.family_code,
            entity_kind_code: unknown_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: None,
            end_byte: None,
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    for fact in &batch.imports {
        let import = canonical_identity(&canonical_ids, fact.import_id, "import declaration")?;
        let target_module = canonical_identity(
            &canonical_ids,
            fact.target_module_id,
            "import target module",
        )?;
        entities.push(EntityRow {
            scope,
            entity_id: import.id,
            language: Language::Python as i16,
            entity_family_code: import_declaration_kind.family_code,
            entity_kind_code: import_declaration_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: Some(fact.source_name.clone()),
            qualified_name: None,
            parent_entity_id: Some(module_identity.id),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(import.digest),
        });
        let target_unknown = fact.target_module_name.is_none();
        let target_kind = if target_unknown {
            unknown_kind
        } else {
            module_kind
        };
        entities.push(EntityRow {
            scope,
            entity_id: target_module.id,
            language: Language::Python as i16,
            entity_family_code: target_kind.family_code,
            entity_kind_code: target_kind.code,
            raw_kind_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            name: fact
                .target_module_name
                .clone()
                .or_else(|| Some(fact.source_name.clone())),
            qualified_name: fact.target_module_name.clone(),
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(target_module.digest),
        });
        if let Some(imported_id) = fact.imported_entity_id {
            let imported = canonical_identity(&canonical_ids, imported_id, "imported entity")?;
            let imported_kind = if target_unknown {
                unknown_kind
            } else {
                symbol_kind
            };
            entities.push(EntityRow {
                scope,
                entity_id: imported.id,
                language: Language::Python as i16,
                entity_family_code: imported_kind.family_code,
                entity_kind_code: imported_kind.code,
                raw_kind_code: None,
                file_id: None,
                start_byte: None,
                end_byte: None,
                name: fact.imported_name.clone(),
                qualified_name: fact.ruff_qualified_name.clone().or_else(|| {
                    fact.target_module_name
                        .as_ref()
                        .zip(fact.imported_name.as_ref())
                        .map(|(module, name)| format!("{module}.{name}"))
                }),
                parent_entity_id: Some(target_module.id),
                type_id: None,
                flags: 0,
                fact_hash64: digest_hash64(imported.digest),
            });
        }
        module_import_details.push(ModuleImportDetailRow {
            scope,
            import_id: import.id,
            source_module_id: module_identity.id,
            target_module_id: (!target_unknown).then_some(target_module.id),
            imported_entity_id: fact
                .imported_entity_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "imported entity").map(|item| item.id)
                })
                .transpose()?,
            local_binding_id: fact
                .local_binding_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "local import binding")
                        .map(|item| item.id)
                })
                .transpose()?,
            import_kind_code: import_kind_code(fact.kind),
            relative_level: fact.relative_level,
            source_name: fact.source_name.clone(),
            alias_name: fact.alias_name.clone(),
            star_import: fact.star_import,
            unknown_reason_code: fact.unknown_reason_code.as_ref().map(|_| 30),
        });
    }

    for fact in &batch.exports {
        let export = canonical_identity(&canonical_ids, fact.export_id, "export")?;
        let kind = if fact.reexport {
            reexport_entity_kind
        } else {
            export_entity_kind
        };
        entities.push(EntityRow {
            scope,
            entity_id: export.id,
            language: Language::Python as i16,
            entity_family_code: kind.family_code,
            entity_kind_code: kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: Some(fact.name.clone()),
            qualified_name: Some(format!("{}.{}", batch.module_name, fact.name)),
            parent_entity_id: Some(module_identity.id),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(export.digest),
        });
    }

    for fact in &batch.callables {
        let identity = canonical_identity(&canonical_ids, fact.callable_id, "callable")?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: callable_kind.family_code,
            entity_kind_code: callable_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: Some(fact.name.clone()),
            qualified_name: Some(fact.qualified_name.clone()),
            parent_entity_id: Some(
                canonical_identity(&canonical_ids, fact.owner_scope_id, "callable owner scope")?.id,
            ),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        callable_details.push(CallableDetailRow {
            scope,
            callable_id: identity.id,
            signature_id: None,
            return_type_id: None,
            parameter_count: fact.parameter_count,
            generic_parameter_count: fact.generic_parameter_count,
            calling_convention_code: None,
            abi_name: None,
            callable_flags: fact.flags,
        });
    }

    for fact in &batch.parameters {
        let identity = canonical_identity(&canonical_ids, fact.parameter_id, "parameter")?;
        let callable =
            canonical_identity(&canonical_ids, fact.callable_id, "parameter callable")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: parameter_kind.family_code,
            entity_kind_code: parameter_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(callable),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        parameter_details.push(ParameterDetailRow {
            scope,
            parameter_id: identity.id,
            callable_id: callable,
            ordinal: fact.ordinal,
            name: Some(fact.name.clone()),
            parameter_kind_code: parameter_kind_code(fact.kind),
            type_id: None,
            default_syntax_id: fact
                .default_syntax_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "default syntax").map(|item| item.id)
                })
                .transpose()?,
            flags: fact.flags,
        });
    }

    for fact in &batch.callable_syntax {
        let identity = canonical_identity(&canonical_ids, fact.syntax_id, "callable syntax")?;
        let kind = match fact.role {
            PythonCallableSyntaxRole::CallExpression => call_expression_kind,
            PythonCallableSyntaxRole::Argument => argument_syntax_kind,
            PythonCallableSyntaxRole::TypeParameter => declaration_syntax_kind,
            PythonCallableSyntaxRole::CalleeExpression
            | PythonCallableSyntaxRole::Receiver
            | PythonCallableSyntaxRole::Decorator
            | PythonCallableSyntaxRole::ReturnAnnotation
            | PythonCallableSyntaxRole::ParameterAnnotation
            | PythonCallableSyntaxRole::ParameterDefault => expression_kind,
        };
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: kind.family_code,
            entity_kind_code: kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: (!fact.text.is_empty()).then(|| fact.text.clone()),
            qualified_name: None,
            parent_entity_id: Some(
                canonical_identity(&canonical_ids, fact.owner_id, "callable syntax owner")?.id,
            ),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    for fact in &batch.call_sites {
        let identity = canonical_identity(&canonical_ids, fact.call_site_id, "call site")?;
        let caller = canonical_identity(&canonical_ids, fact.caller_id, "call-site caller")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: call_site_kind.family_code,
            entity_kind_code: call_site_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: None,
            qualified_name: None,
            parent_entity_id: Some(caller),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        call_site_details.push(CallSiteDetailRow {
            scope,
            call_site_id: identity.id,
            caller_id: caller,
            syntax_id: Some(canonical_identity(&canonical_ids, fact.syntax_id, "call syntax")?.id),
            callee_syntax_id: Some(
                canonical_identity(&canonical_ids, fact.callee_syntax_id, "callee syntax")?.id,
            ),
            receiver_value_id: fact
                .receiver_syntax_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "receiver syntax").map(|item| item.id)
                })
                .transpose()?,
            result_value_id: None,
            dispatch_kind_code: dispatch_kind_code(fact.dispatch_kind),
            declared_target_id: fact
                .declared_target_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "declared call target")
                        .map(|item| item.id)
                })
                .transpose()?,
            resolved_target_count: fact.resolved_target_count,
            call_flags: fact.flags,
        });
    }

    for fact in &batch.call_arguments {
        let identity = canonical_identity(&canonical_ids, fact.argument_id, "call argument")?;
        let call_site =
            canonical_identity(&canonical_ids, fact.call_site_id, "argument call site")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: argument_kind.family_code,
            entity_kind_code: argument_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: fact.start_byte.map(coordinate).transpose()?,
            end_byte: fact.end_byte.map(coordinate).transpose()?,
            name: fact.keyword_name.clone(),
            qualified_name: None,
            parent_entity_id: Some(call_site),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        call_argument_details.push(CallArgumentDetailRow {
            scope,
            argument_id: identity.id,
            call_site_id: call_site,
            ordinal: fact.ordinal,
            keyword_name: fact.keyword_name.clone(),
            argument_syntax_id: fact
                .argument_syntax_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "argument syntax").map(|item| item.id)
                })
                .transpose()?,
            argument_value_id: None,
            parameter_id: fact
                .parameter_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "argument binding target")
                        .map(|item| item.id)
                })
                .transpose()?,
            binding_status_code: argument_binding_status_code(fact.binding_status),
            spread_kind_code: Some(argument_spread_kind_code(fact.spread_kind)),
        });
    }

    for fact in &batch.unknown_argument_sets {
        let identity = canonical_identity(
            &canonical_ids,
            fact.unknown_argument_set_id,
            "unknown argument set",
        )?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: unknown_argument_set_kind.family_code,
            entity_kind_code: unknown_argument_set_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: None,
            end_byte: None,
            name: Some("UNKNOWN_ARGUMENT_SET".into()),
            qualified_name: None,
            parent_entity_id: Some(
                canonical_identity(
                    &canonical_ids,
                    fact.call_site_id,
                    "unknown argument call site",
                )?
                .id,
            ),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    for fact in &batch.members {
        let identity = canonical_identity(&canonical_ids, fact.member_id, "member candidate")?;
        let kind = required_entity_kind(member_kind_name(fact.kind))?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: kind.family_code,
            entity_kind_code: kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(
                canonical_identity(&canonical_ids, fact.class_id, "member class")?.id,
            ),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    for fact in &batch.cfg_nodes {
        let identity = canonical_identity(&canonical_ids, fact.cfg_node_id, "CFG node")?;
        let callable = canonical_identity(&canonical_ids, fact.owner_id, "CFG node owner")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: cfg_block_kind.family_code,
            entity_kind_code: cfg_block_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: fact.start_byte.map(coordinate).transpose()?,
            end_byte: fact.end_byte.map(coordinate).transpose()?,
            name: Some(fact.label.into()),
            qualified_name: None,
            parent_entity_id: Some(callable),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        cfg_node_details.push(CfgNodeDetailRow {
            scope,
            cfg_node_id: identity.id,
            cfg_id: canonical_identity(&canonical_ids, fact.cfg_id, "CFG node graph")?.id,
            node_kind_code: fact.kind.code(),
            syntax_id: None,
            mir_statement_id: None,
            ordinal: Some(fact.ordinal),
            flags: fact.flags,
        });
    }

    for fact in &batch.cfgs {
        cfg_graphs.push(CfgGraphRow {
            scope,
            cfg_id: canonical_identity(&canonical_ids, fact.cfg_id, "CFG")?.id,
            callable_id: Some(
                canonical_identity(&canonical_ids, fact.callable_id, "CFG callable")?.id,
            ),
            cfg_kind_code: fact.kind.code(),
            entry_node_id: canonical_identity(&canonical_ids, fact.entry_node_id, "CFG entry")?.id,
            exit_node_id: canonical_identity(&canonical_ids, fact.exit_node_id, "CFG exit")?.id,
            exceptional_exit_node_id: Some(
                canonical_identity(
                    &canonical_ids,
                    fact.exceptional_exit_node_id,
                    "CFG exceptional exit",
                )?
                .id,
            ),
            node_count: fact.node_count,
            edge_count: fact.edge_count,
            flags: fact.flags,
        });
    }

    for fact in &batch.values {
        let identity = canonical_identity(&canonical_ids, fact.value_id, "dataflow value")?;
        let owner = canonical_identity(&canonical_ids, fact.owner_id, "dataflow value owner")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: value_entity_kind.family_code,
            entity_kind_code: value_entity_kind.code,
            raw_kind_code: None,
            file_id: fact.start_byte.map(|_| file_id),
            start_byte: fact.start_byte.map(coordinate).transpose()?,
            end_byte: fact.end_byte.map(coordinate).transpose()?,
            name: None,
            qualified_name: None,
            parent_entity_id: Some(owner),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        value_details.push(ValueDetailRow {
            scope,
            value_id: identity.id,
            value_kind_code: fact.kind.code(),
            type_id: None,
            producer_operation_id: fact
                .producer_operation_id
                .map(|id| canonical_identity(&canonical_ids, id, "value producer operation"))
                .transpose()?
                .map(|identity| identity.id),
            constant_value_id: None,
            syntax_id: fact
                .syntax_id
                .map(|id| canonical_identity(&canonical_ids, id, "value syntax"))
                .transpose()?
                .map(|identity| identity.id),
            flags: fact.flags,
            precision_profile_id: fact.precision_profile_id.into(),
            derivation_bundle_id: fact.derivation_bundle_id.into(),
        });
    }
    for fact in &batch.operations {
        let identity =
            canonical_identity(&canonical_ids, fact.operation_id, "dataflow operation")?;
        let owner =
            canonical_identity(&canonical_ids, fact.owner_id, "dataflow operation owner")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: operation_entity_kind.family_code,
            entity_kind_code: operation_entity_kind.code,
            raw_kind_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            name: None,
            qualified_name: None,
            parent_entity_id: Some(owner),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        operation_details.push(OperationDetailRow {
            scope,
            operation_id: identity.id,
            cfg_node_id: fact
                .cfg_node_id
                .map(|id| canonical_identity(&canonical_ids, id, "operation CFG node"))
                .transpose()?
                .map(|identity| identity.id),
            operation_kind_code: fact.kind.code(),
            result_value_id: fact
                .result_value_id
                .map(|id| canonical_identity(&canonical_ids, id, "operation result value"))
                .transpose()?
                .map(|identity| identity.id),
            type_id: None,
            syntax_id: fact
                .syntax_id
                .map(|id| canonical_identity(&canonical_ids, id, "operation syntax"))
                .transpose()?
                .map(|identity| identity.id),
            raw_kind_code: None,
            flags: fact.flags,
            precision_profile_id: fact.precision_profile_id.into(),
            derivation_bundle_id: fact.derivation_bundle_id.into(),
        });
    }
    for fact in &batch.dataflow_events {
        let identity = canonical_identity(&canonical_ids, fact.event_id, "dataflow event")?;
        let owner = canonical_identity(&canonical_ids, fact.owner_id, "dataflow event owner")?.id;
        let kind = if fact.kind == PythonDataflowEventKind::Definition {
            definition_event_kind
        } else {
            use_event_kind
        };
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: kind.family_code,
            entity_kind_code: kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(coordinate(fact.start_byte)?),
            end_byte: Some(coordinate(fact.end_byte)?),
            name: None,
            qualified_name: None,
            parent_entity_id: Some(owner),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        dataflow_event_details.push(DataflowEventDetailRow {
            scope,
            event_id: identity.id,
            cfg_node_id: fact
                .cfg_node_id
                .map(|id| canonical_identity(&canonical_ids, id, "event CFG node"))
                .transpose()?
                .map(|identity| identity.id),
            event_kind_code: fact.kind.code(),
            binding_id: fact
                .binding_id
                .map(|id| canonical_identity(&canonical_ids, id, "event binding"))
                .transpose()?
                .map(|identity| identity.id),
            value_id: fact
                .value_id
                .map(|id| canonical_identity(&canonical_ids, id, "event value"))
                .transpose()?
                .map(|identity| identity.id),
            location_id: fact
                .location_id
                .map(|id| canonical_identity(&canonical_ids, id, "event memory location"))
                .transpose()?
                .map(|identity| identity.id),
            syntax_id: fact
                .syntax_id
                .map(|id| canonical_identity(&canonical_ids, id, "event syntax"))
                .transpose()?
                .map(|identity| identity.id),
            ordinal: fact.ordinal,
            flags: fact.flags,
            precision_profile_id: fact.precision_profile_id.into(),
            derivation_bundle_id: fact.derivation_bundle_id.into(),
        });
    }
    for fact in &batch.memory_locations {
        let identity = canonical_identity(&canonical_ids, fact.location_id, "memory location")?;
        let owner = canonical_identity(&canonical_ids, fact.owner_id, "memory location owner")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: memory_location_kind.family_code,
            entity_kind_code: memory_location_kind.code,
            raw_kind_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            name: fact.display_path.clone(),
            qualified_name: None,
            parent_entity_id: Some(owner),
            type_id: None,
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
        memory_location_details.push(MemoryLocationDetailRow {
            scope,
            location_id: identity.id,
            location_kind_code: fact.kind.code(),
            base_entity_id: fact
                .base_entity_id
                .map(|id| canonical_identity(&canonical_ids, id, "location base entity"))
                .transpose()?
                .map(|identity| identity.id),
            base_local_id: fact
                .base_local_id
                .map(|id| canonical_identity(&canonical_ids, id, "location base local"))
                .transpose()?
                .map(|identity| identity.id),
            type_id: None,
            parent_location_id: fact
                .parent_location_id
                .map(|id| canonical_identity(&canonical_ids, id, "parent memory location"))
                .transpose()?
                .map(|identity| identity.id),
            projection_depth: fact.projection_depth,
            canonical_path_hash: fact.canonical_path_hash,
            display_path: fact.display_path.clone(),
            flags: fact.flags,
            precision_profile_id: fact.precision_profile_id.into(),
            derivation_bundle_id: fact.derivation_bundle_id.into(),
        });
    }
    for fact in &batch.access_path_components {
        access_path_components.push(AccessPathComponentRow {
            scope,
            component_id: canonical_identity(
                &canonical_ids,
                fact.component_id,
                "access-path component",
            )?
            .id,
            location_id: canonical_identity(
                &canonical_ids,
                fact.location_id,
                "component memory location",
            )?
            .id,
            ordinal: fact.ordinal,
            projection_kind_code: fact.kind.code(),
            field_entity_id: fact
                .field_entity_id
                .map(|id| canonical_identity(&canonical_ids, id, "component field"))
                .transpose()?
                .map(|identity| identity.id),
            index_value_id: fact
                .index_value_id
                .map(|id| canonical_identity(&canonical_ids, id, "component index value"))
                .transpose()?
                .map(|identity| identity.id),
            variant_entity_id: None,
            constant_index: fact.constant_index,
            subslice_from: None,
            subslice_to: None,
            flags: fact.flags,
            precision_profile_id: fact.precision_profile_id.into(),
            derivation_bundle_id: fact.derivation_bundle_id.into(),
        });
    }

    let mut exception_categories = BTreeMap::new();
    for category in batch
        .cfg_edges
        .iter()
        .filter_map(|edge| edge.exception_category)
    {
        exception_categories.entry(category).or_insert_with(|| {
            derived_identity(
                b"python-cfg-exception-category",
                &[&scope.owner_id, category.as_bytes()],
            )
        });
    }
    for (category, identity) in &exception_categories {
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: semantic_type_kind.family_code,
            entity_kind_code: semantic_type_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: None,
            end_byte: None,
            name: Some(format!("EXCEPTION_CATEGORY:{category}")),
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    let mut relations = Vec::new();
    let module_scope = batch
        .scopes
        .iter()
        .find(|fact| fact.kind == PythonScopeKind::Module)
        .ok_or_else(|| FactIngestError::Protocol("Ruff module scope is absent".into()))?;
    push_relation(
        &mut relations,
        scope,
        file_id,
        contains_kind,
        module_identity.id,
        canonical_identity(&canonical_ids, module_scope.scope_id, "module scope")?.id,
        None,
        None,
        EvidenceCertainty::StaticSemantic as i16,
        ResolutionClass::StaticallyResolved as i16,
    );
    for fact in &batch.scopes {
        if let Some(parent) = fact.parent_scope_id {
            push_relation(
                &mut relations,
                scope,
                file_id,
                contains_kind,
                canonical_identity(&canonical_ids, parent, "parent scope")?.id,
                canonical_identity(&canonical_ids, fact.scope_id, "child scope")?.id,
                None,
                None,
                EvidenceCertainty::StaticSemantic as i16,
                ResolutionClass::StaticallyResolved as i16,
            );
        }
    }
    for fact in &batch.bindings {
        push_relation(
            &mut relations,
            scope,
            file_id,
            declares_kind,
            canonical_identity(&canonical_ids, fact.scope_id, "binding scope")?.id,
            canonical_identity(&canonical_ids, fact.binding_id, "binding")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
    }
    for fact in &batch.references {
        let (certainty, resolution) = canonical_resolution(fact.resolution);
        push_relation(
            &mut relations,
            scope,
            file_id,
            refers_to_kind,
            canonical_identity(&canonical_ids, fact.reference_id, "reference")?.id,
            canonical_identity(&canonical_ids, fact.target_id, "reference target")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            certainty,
            resolution,
        );
    }

    for fact in &batch.imports {
        let import = canonical_identity(&canonical_ids, fact.import_id, "import declaration")?.id;
        let target_module =
            canonical_identity(&canonical_ids, fact.target_module_id, "target module")?.id;
        let (certainty, resolution) = canonical_resolution(fact.resolution);
        for (kind, source, target) in [
            (imports_module_kind, import, target_module),
            (depends_on_module_kind, module_identity.id, target_module),
        ] {
            push_relation(
                &mut relations,
                scope,
                file_id,
                kind,
                source,
                target,
                Some(coordinate(fact.start_byte)?),
                Some(coordinate(fact.end_byte)?),
                certainty,
                resolution,
            );
        }
        if let Some(imported_id) = fact.imported_entity_id {
            let imported = canonical_identity(&canonical_ids, imported_id, "imported entity")?.id;
            push_relation(
                &mut relations,
                scope,
                file_id,
                imports_symbol_kind,
                import,
                imported,
                Some(coordinate(fact.start_byte)?),
                Some(coordinate(fact.end_byte)?),
                certainty,
                resolution,
            );
            push_relation(
                &mut relations,
                scope,
                file_id,
                defined_in_module_kind,
                imported,
                target_module,
                None,
                None,
                certainty,
                resolution,
            );
            if let Some(binding_id) = fact.local_binding_id {
                push_relation(
                    &mut relations,
                    scope,
                    file_id,
                    aliases_kind,
                    canonical_identity(&canonical_ids, binding_id, "local import binding")?.id,
                    imported,
                    Some(coordinate(fact.start_byte)?),
                    Some(coordinate(fact.end_byte)?),
                    certainty,
                    resolution,
                );
            }
        } else if let Some(binding_id) = fact.local_binding_id {
            push_relation(
                &mut relations,
                scope,
                file_id,
                aliases_kind,
                canonical_identity(&canonical_ids, binding_id, "local import binding")?.id,
                target_module,
                Some(coordinate(fact.start_byte)?),
                Some(coordinate(fact.end_byte)?),
                certainty,
                resolution,
            );
        }
    }

    for fact in &batch.exports {
        let export = canonical_identity(&canonical_ids, fact.export_id, "export")?.id;
        push_relation(
            &mut relations,
            scope,
            file_id,
            if fact.reexport {
                reexport_relation_kind
            } else {
                export_relation_kind
            },
            module_identity.id,
            export,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
        push_relation(
            &mut relations,
            scope,
            file_id,
            aliases_kind,
            export,
            canonical_identity(&canonical_ids, fact.target_id, "export target")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
    }

    for fact in &batch.callables {
        push_relation(
            &mut relations,
            scope,
            file_id,
            declares_kind,
            canonical_identity(&canonical_ids, fact.owner_scope_id, "callable scope")?.id,
            canonical_identity(&canonical_ids, fact.callable_id, "callable")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
    }
    for fact in &batch.parameters {
        push_ordered_relation(
            &mut relations,
            scope,
            file_id,
            has_parameter_kind,
            canonical_identity(&canonical_ids, fact.callable_id, "parameter callable")?.id,
            canonical_identity(&canonical_ids, fact.parameter_id, "parameter")?.id,
            fact.ordinal,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::NotApplicable as i16,
        );
    }
    for fact in &batch.callable_syntax {
        let relation = match fact.role {
            PythonCallableSyntaxRole::Decorator => Some(has_decorator_kind),
            PythonCallableSyntaxRole::ReturnAnnotation => Some(has_return_annotation_kind),
            PythonCallableSyntaxRole::ParameterAnnotation => Some(has_annotation_kind),
            PythonCallableSyntaxRole::ParameterDefault => Some(has_default_kind),
            PythonCallableSyntaxRole::TypeParameter => Some(has_type_parameter_kind),
            PythonCallableSyntaxRole::CallExpression
            | PythonCallableSyntaxRole::CalleeExpression
            | PythonCallableSyntaxRole::Receiver
            | PythonCallableSyntaxRole::Argument => None,
        };
        if let Some(relation) = relation {
            push_ordered_relation(
                &mut relations,
                scope,
                file_id,
                relation,
                canonical_identity(&canonical_ids, fact.owner_id, "callable syntax owner")?.id,
                canonical_identity(&canonical_ids, fact.syntax_id, "callable syntax")?.id,
                fact.ordinal.unwrap_or(0),
                Some(coordinate(fact.start_byte)?),
                Some(coordinate(fact.end_byte)?),
                EvidenceCertainty::SourceExact as i16,
                ResolutionClass::NotApplicable as i16,
            );
        }
    }
    for fact in &batch.call_sites {
        let call_site = canonical_identity(&canonical_ids, fact.call_site_id, "call site")?.id;
        push_relation(
            &mut relations,
            scope,
            file_id,
            contains_call_kind,
            canonical_identity(&canonical_ids, fact.caller_id, "caller")?.id,
            call_site,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::SourceExact as i16,
            ResolutionClass::NotApplicable as i16,
        );
        push_relation(
            &mut relations,
            scope,
            file_id,
            has_callee_expression_kind,
            call_site,
            canonical_identity(&canonical_ids, fact.callee_syntax_id, "callee syntax")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::SourceExact as i16,
            ResolutionClass::NotApplicable as i16,
        );
        if let Some(receiver) = fact.receiver_syntax_id {
            push_relation(
                &mut relations,
                scope,
                file_id,
                has_receiver_kind,
                call_site,
                canonical_identity(&canonical_ids, receiver, "receiver syntax")?.id,
                Some(coordinate(fact.start_byte)?),
                Some(coordinate(fact.end_byte)?),
                EvidenceCertainty::SourceExact as i16,
                ResolutionClass::NotApplicable as i16,
            );
        }
    }
    for fact in &batch.call_arguments {
        let argument = canonical_identity(&canonical_ids, fact.argument_id, "call argument")?.id;
        push_ordered_relation(
            &mut relations,
            scope,
            file_id,
            has_argument_kind,
            canonical_identity(&canonical_ids, fact.call_site_id, "argument call site")?.id,
            argument,
            fact.ordinal,
            fact.start_byte.map(coordinate).transpose()?,
            fact.end_byte.map(coordinate).transpose()?,
            EvidenceCertainty::SourceExact as i16,
            ResolutionClass::NotApplicable as i16,
        );
        if let Some(parameter) = fact.parameter_id {
            let (certainty, resolution) = match fact.binding_status {
                PythonArgumentBindingStatus::UnknownArgumentSet => (
                    EvidenceCertainty::Unresolved as i16,
                    ResolutionClass::Unresolved as i16,
                ),
                PythonArgumentBindingStatus::Duplicate
                | PythonArgumentBindingStatus::PositionalOnlyKeyword
                | PythonArgumentBindingStatus::TooManyPositional
                | PythonArgumentBindingStatus::UnmatchedKeyword => (
                    EvidenceCertainty::SoundMay as i16,
                    ResolutionClass::SoundPossible as i16,
                ),
                PythonArgumentBindingStatus::Bound
                | PythonArgumentBindingStatus::BoundReceiver
                | PythonArgumentBindingStatus::Defaulted
                | PythonArgumentBindingStatus::MissingRequired
                | PythonArgumentBindingStatus::UnresolvedTarget => (
                    EvidenceCertainty::StaticSemantic as i16,
                    ResolutionClass::StaticallyResolved as i16,
                ),
            };
            push_relation(
                &mut relations,
                scope,
                file_id,
                argument_binds_to_kind,
                argument,
                canonical_identity(&canonical_ids, parameter, "argument binding target")?.id,
                fact.start_byte.map(coordinate).transpose()?,
                fact.end_byte.map(coordinate).transpose()?,
                certainty,
                resolution,
            );
        }
    }
    for fact in &batch.members {
        push_relation(
            &mut relations,
            scope,
            file_id,
            declares_member_kind,
            canonical_identity(&canonical_ids, fact.class_id, "member class")?.id,
            canonical_identity(&canonical_ids, fact.member_id, "member candidate")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::SourceExact as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
    }
    let cfg_nodes_by_id = batch
        .cfg_nodes
        .iter()
        .map(|node| (node.cfg_node_id, node))
        .collect::<BTreeMap<_, _>>();
    for fact in &batch.cfg_edges {
        let relation_kind = required_relation_kind(fact.kind.registry_name())?;
        let source = canonical_identity(&canonical_ids, fact.source_node_id, "CFG edge source")?.id;
        let target = canonical_identity(&canonical_ids, fact.target_node_id, "CFG edge target")?.id;
        let source_node = cfg_nodes_by_id.get(&fact.source_node_id).ok_or_else(|| {
            FactIngestError::Protocol("CFG relation source detail is absent".into())
        })?;
        push_relation(
            &mut relations,
            scope,
            file_id,
            relation_kind,
            source,
            target,
            source_node.start_byte.map(coordinate).transpose()?,
            source_node.end_byte.map(coordinate).transpose()?,
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::NotApplicable as i16,
        );
        let relation = relations.last_mut().ok_or_else(|| {
            FactIngestError::Protocol("CFG relation projection produced no row".into())
        })?;
        relation.flags = fact.flags;
        let relation_id = relation.fact_id;
        let payload_text = fact
            .case_value_text
            .clone()
            .or_else(|| fact.exception_category.map(str::to_owned));
        cfg_edge_details.push(CfgEdgeDetailRow {
            scope,
            relation_id,
            cfg_id: canonical_identity(&canonical_ids, fact.cfg_id, "CFG edge graph")?.id,
            condition_id: fact
                .condition_node_id
                .map(|id| {
                    canonical_identity(&canonical_ids, id, "CFG edge condition")
                        .map(|identity| identity.id)
                })
                .transpose()?,
            case_value_hash: payload_text.as_deref().map(stable_text_hash64),
            case_value_text: payload_text,
            exception_type_id: fact
                .exception_category
                .and_then(|category| exception_categories.get(category))
                .map(|identity| identity.id),
            edge_flags: fact.flags,
        });
    }
    for fact in &batch.dataflow_relations {
        if fact.precision_profile_id != crate::ruff_adapter::PYTHON_DATAFLOW_PRECISION_PROFILE
            || fact.derivation_bundle_id != crate::ruff_adapter::PYTHON_DATAFLOW_BUNDLE_ID
        {
            return Err(FactIngestError::Protocol(
                "Python dataflow relation lacks the selected derivation stamps".into(),
            ));
        }
        let kind = match fact.kind {
            PythonDataflowRelationKind::ReachingDefinition => reaching_definition_kind,
            PythonDataflowRelationKind::Reaches => reaches_kind,
            PythonDataflowRelationKind::DefUse => def_use_kind,
            PythonDataflowRelationKind::DataDep => data_dep_kind,
            PythonDataflowRelationKind::ValueFlowsTo => value_flows_to_kind,
            PythonDataflowRelationKind::KillsDefinition => kills_definition_kind,
        };
        let identity = derived_identity(
            b"python-dataflow-relation",
            &[&scope.owner_id, &fact.relation_id],
        );
        relations.push(RelationRow {
            scope,
            fact_id: identity.id,
            language: Language::Python as i16,
            relation_family_code: kind.family_code,
            relation_kind_code: kind.code,
            source_id: canonical_identity(
                &canonical_ids,
                fact.source_id,
                "dataflow relation source",
            )?
            .id,
            target_id: canonical_identity(
                &canonical_ids,
                fact.target_id,
                "dataflow relation target",
            )?
            .id,
            ordinal: None,
            role_code: None,
            distance: None,
            directness_code: Directness::Direct as i16,
            file_id: None,
            start_byte: None,
            end_byte: None,
            certainty_code: EvidenceCertainty::StaticSemantic as i16,
            resolution_code: ResolutionClass::Exact as i16,
            producer_code: ProviderCode::CodefabricDerivation as i16,
            derivation_code: Some(PYTHON_DATAFLOW_DERIVATION_CODE),
            flags: fact.flags,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    entities.sort_by_key(|row| row.entity_id);
    entities.dedup_by_key(|row| row.entity_id);
    relations.sort_by_key(|row| row.fact_id);
    relations.dedup_by_key(|row| row.fact_id);
    scope_details.sort_by_key(|row| row.scope_id);
    binding_details.sort_by_key(|row| row.binding_id);
    reference_details.sort_by_key(|row| row.reference_id);
    module_import_details.sort_by_key(|row| row.import_id);
    callable_details.sort_by_key(|row| row.callable_id);
    parameter_details.sort_by_key(|row| row.parameter_id);
    call_site_details.sort_by_key(|row| row.call_site_id);
    call_argument_details.sort_by_key(|row| row.argument_id);
    cfg_graphs.sort_by_key(|row| row.cfg_id);
    cfg_node_details.sort_by_key(|row| row.cfg_node_id);
    cfg_edge_details.sort_by_key(|row| row.relation_id);
    value_details.sort_by_key(|row| row.value_id);
    operation_details.sort_by_key(|row| row.operation_id);
    dataflow_event_details.sort_by_key(|row| row.event_id);
    memory_location_details.sort_by_key(|row| row.location_id);
    access_path_components.sort_by_key(|row| row.component_id);

    let diagnostics = batch
        .call_diagnostics
        .iter()
        .map(|fact| DiagnosticRow {
            diagnostic_id: derived_identity(
                b"python-call-diagnostic",
                &[&scope.owner_id, &fact.diagnostic_id],
            )
            .id,
            workspace_id: scope.workspace_id,
            analysis_context_id: Some(scope.analysis_context_id),
            source_generation: scope.source_generation,
            owner_id: Some(scope.owner_id),
            diagnostic_code: call_diagnostic_code(fact.code),
            severity_code: Severity::Error as i16,
            message: fact.message.clone(),
            cold_payload: None,
            created_at_micros: 0,
        })
        .collect::<Vec<_>>();

    let entity_form = required_fact_form("ENTITY_EXISTENCE")?;
    let relation_form = required_fact_form("RELATION")?;
    let mut evidence = Vec::with_capacity(entities.len() + relations.len());
    for entity in &entities {
        let unresolved = entity.entity_kind_code == unknown_kind.code
            || entity.entity_kind_code == unknown_argument_set_kind.code;
        evidence.push(evidence_row(
            scope,
            provider_run.id,
            entity.entity_id,
            entity_form,
            entity.file_id,
            entity.start_byte,
            entity.end_byte,
            if unresolved {
                EvidenceCertainty::Unresolved as i16
            } else {
                EvidenceCertainty::StaticSemantic as i16
            },
            if unresolved {
                ResolutionClass::Unresolved as i16
            } else {
                ResolutionClass::NotApplicable as i16
            },
        ));
    }
    for relation in &relations {
        evidence.push(evidence_row(
            scope,
            provider_run.id,
            relation.fact_id,
            relation_form,
            relation.file_id,
            relation.start_byte,
            relation.end_byte,
            relation.certainty_code,
            relation.resolution_code,
        ));
    }
    evidence.sort_by_key(|row| row.evidence_id);

    let mut coverage_hasher = blake3::Hasher::new_derive_key("codefabric.ruff-coverage.v1");
    coverage_hasher.update(batch.provider_image_fingerprint.as_bytes());
    for payload in &observation_payloads {
        coverage_hasher.update(payload);
    }
    let coverage_scope_fingerprint = *coverage_hasher.finalize().as_bytes();
    let scopes_bindings = capability_code("SCOPES_BINDINGS")
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol("SCOPES_BINDINGS capability is absent".into()))?;
    let capability = CapabilityStatusRow {
        scope,
        snapshot_id: None,
        capability_code: scopes_bindings,
        owner_capability_state_code: OwnerCapabilityState::Current as i16,
        completeness_state_code: CompletenessState::Complete as i16,
        provider_run_id: Some(provider_run.id),
        producer_code: Some(ProviderCode::RuffPython as i16),
        reason_code: None,
        diagnostic_id: None,
        fallback_source_available: false,
        coverage_scope_fingerprint,
    };
    let import_resolution = capability_code("IMPORT_RESOLUTION")
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| {
            FactIngestError::Protocol("IMPORT_RESOLUTION capability is absent".into())
        })?;
    let import_incomplete = batch.export_status == PythonExportStatus::IncompleteDynamic
        || batch
            .imports
            .iter()
            .any(|fact| fact.resolution != PythonResolution::Resolved);
    let import_capability = CapabilityStatusRow {
        scope,
        snapshot_id: None,
        capability_code: import_resolution,
        owner_capability_state_code: OwnerCapabilityState::Current as i16,
        completeness_state_code: if import_incomplete {
            CompletenessState::Partial as i16
        } else {
            CompletenessState::Complete as i16
        },
        provider_run_id: Some(provider_run.id),
        producer_code: Some(ProviderCode::RuffPython as i16),
        reason_code: import_incomplete.then_some(30),
        diagnostic_id: None,
        fallback_source_available: false,
        coverage_scope_fingerprint,
    };
    let cfg_capability_code = capability_code("CFG")
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol("CFG capability is absent".into()))?;
    let cfg_capability = CapabilityStatusRow {
        scope,
        snapshot_id: None,
        capability_code: cfg_capability_code,
        owner_capability_state_code: OwnerCapabilityState::Current as i16,
        completeness_state_code: CompletenessState::Complete as i16,
        provider_run_id: Some(provider_run.id),
        producer_code: Some(ProviderCode::RuffPython as i16),
        reason_code: None,
        diagnostic_id: None,
        fallback_source_available: false,
        coverage_scope_fingerprint,
    };
    let def_use_code = capability_code("DEF_USE")
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol("DEF_USE capability is absent".into()))?;
    let dataflow_incomplete = batch
        .values
        .iter()
        .any(|fact| fact.kind == crate::ruff_adapter::PythonValueKind::Unknown)
        || batch
            .dataflow_events
            .iter()
            .any(|fact| fact.kind == PythonDataflowEventKind::DynamicUnknown)
        || batch
            .memory_locations
            .iter()
            .any(|fact| fact.kind == crate::ruff_adapter::PythonLocationKind::Unknown);
    let def_use_capability = CapabilityStatusRow {
        scope,
        snapshot_id: None,
        capability_code: def_use_code,
        owner_capability_state_code: if dataflow_incomplete {
            OwnerCapabilityState::Partial as i16
        } else {
            OwnerCapabilityState::Current as i16
        },
        completeness_state_code: if dataflow_incomplete {
            CompletenessState::Partial as i16
        } else {
            CompletenessState::Complete as i16
        },
        provider_run_id: Some(provider_run.id),
        producer_code: Some(ProviderCode::CodefabricDerivation as i16),
        reason_code: dataflow_incomplete.then_some(10),
        diagnostic_id: None,
        fallback_source_available: false,
        coverage_scope_fingerprint,
    };
    let unavailable_provider_capability = |name: &str| -> Result<CapabilityStatusRow, FactIngestError> {
        let code = capability_code(name)
            .and_then(|code| i16::try_from(code).ok())
            .ok_or_else(|| FactIngestError::Protocol(format!("{name} capability is absent")))?;
        Ok(CapabilityStatusRow {
            scope,
            snapshot_id: None,
            capability_code: code,
            owner_capability_state_code: OwnerCapabilityState::UnavailableProvider as i16,
            completeness_state_code: CompletenessState::Unavailable as i16,
            provider_run_id: None,
            producer_code: None,
            reason_code: Some(30),
            diagnostic_id: None,
            fallback_source_available: false,
            coverage_scope_fingerprint,
        })
    };
    let computed_types_capability = unavailable_provider_capability("COMPUTED_TYPES")?;
    let member_resolution_capability = unavailable_provider_capability("MEMBER_RESOLUTION")?;
    let call_targets_capability = unavailable_provider_capability("CALL_TARGETS")?;
    let profile_completeness = aggregate_capability(&[
        CapabilityChild {
            applicable: true,
            completeness: Completeness::Complete,
            has_facts: !batch.scopes.is_empty(),
            missing_remainder_characterized: true,
            required_context_covered: true,
            external_policy_allows_closure: true,
        },
        CapabilityChild {
            applicable: true,
            completeness: if import_incomplete {
                Completeness::Partial
            } else {
                Completeness::Complete
            },
            has_facts: true,
            missing_remainder_characterized: true,
            required_context_covered: true,
            external_policy_allows_closure: true,
        },
        CapabilityChild {
            applicable: true,
            completeness: if dataflow_incomplete {
                Completeness::Partial
            } else {
                Completeness::Complete
            },
            has_facts: !batch.dataflow_events.is_empty(),
            missing_remainder_characterized: true,
            required_context_covered: true,
            external_policy_allows_closure: true,
        },
        CapabilityChild {
            applicable: true,
            completeness: Completeness::Unavailable,
            has_facts: false,
            missing_remainder_characterized: true,
            required_context_covered: false,
            external_policy_allows_closure: true,
        },
        CapabilityChild {
            applicable: true,
            completeness: Completeness::Unavailable,
            has_facts: false,
            missing_remainder_characterized: true,
            required_context_covered: false,
            external_policy_allows_closure: true,
        },
        CapabilityChild {
            applicable: true,
            completeness: Completeness::Unavailable,
            has_facts: false,
            missing_remainder_characterized: true,
            required_context_covered: false,
            external_policy_allows_closure: true,
        },
    ]);

    let capability_bits = capability_mask(&[
        "SCOPES_BINDINGS",
        "IMPORT_RESOLUTION",
        "CFG",
        "DEF_USE",
    ])
        .and_then(|mask| i64::try_from(mask).ok())
        .ok_or_else(|| FactIngestError::Protocol("SCOPES_BINDINGS mask is absent".into()))?;
    let owner = OwnerRow {
        scope,
        parent_owner_id: None,
        owner_kind_code: OwnerKind::Module as i16,
        language: Language::Python as i16,
        file_id: Some(file_id),
        semantic_entity_id: Some(module_identity.id),
        start_byte: Some(coordinate(module_scope.start_byte)?),
        end_byte: Some(coordinate(module_scope.end_byte)?),
        source_fingerprint: Some(
            *blake3::hash(batch.provider_image_fingerprint.as_bytes()).as_bytes(),
        ),
        semantic_fingerprint: Some(coverage_scope_fingerprint),
        capability_mask: capability_bits,
    };

    let encoded = [
        (8, encode_owners(&[owner])?),
        (
            9,
            encode_capability_statuses(&[
                capability,
                import_capability,
                cfg_capability,
                def_use_capability,
                computed_types_capability,
                member_resolution_capability,
                call_targets_capability,
            ])?,
        ),
        (10, encode_diagnostics(&diagnostics)?),
        (100, encode_entities(&entities)?),
        (110, encode_relations(&relations)?),
        (130, encode_evidence(&evidence)?),
        (200, encode_scope_details(&scope_details)?),
        (210, encode_binding_details(&binding_details)?),
        (220, encode_reference_details(&reference_details)?),
        (230, encode_module_import_details(&module_import_details)?),
        (240, encode_callable_details(&callable_details)?),
        (250, encode_parameter_details(&parameter_details)?),
        (260, encode_call_site_details(&call_site_details)?),
        (270, encode_call_argument_details(&call_argument_details)?),
        (280, encode_cfg_graphs(&cfg_graphs)?),
        (290, encode_cfg_node_details(&cfg_node_details)?),
        (300, encode_cfg_edge_details(&cfg_edge_details)?),
        (310, encode_value_details(&value_details)?),
        (320, encode_operation_details(&operation_details)?),
        (330, encode_dataflow_event_details(&dataflow_event_details)?),
        (340, encode_memory_location_details(&memory_location_details)?),
        (350, encode_access_path_components(&access_path_components)?),
    ];
    let provider_batches = encoded
        .into_iter()
        .map(|(table_code, batch)| ProviderFactBatch { table_code, batch })
        .collect::<Vec<_>>();
    let declared_rows = provider_batches
        .iter()
        .map(|batch| batch.batch.num_rows())
        .sum();
    let schema_fingerprints = provider_batches
        .iter()
        .map(|batch| {
            crate::schema_registry::table_spec(batch.table_code)
                .map(|spec| (batch.table_code, spec.schema_digest.clone()))
                .ok_or_else(|| {
                    FactIngestError::Protocol(format!(
                        "generated table {} is absent",
                        batch.table_code
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let stream = ProviderFactStream {
        manifest: ProviderFactManifest {
            stream_id: derived_identity(b"stream", &[&provider_run.id, &scope.owner_id]).id,
            workspace_id: scope.workspace_id,
            analysis_context_id: scope.analysis_context_id,
            source_generation: scope.source_generation,
            provider_code: ProviderCode::RuffPython as i16,
            provider_version: PROVIDER_VERSION.into(),
            provider_run_id: provider_run.id,
            emitted_at_micros: 0,
            schema_fingerprints,
            declared_rows,
        },
        batches: provider_batches,
        terminal: StreamTerminal::Completed,
    };
    let canonical = CanonicalReconciliationEngine::default().ingest(
        scope,
        &[stream],
        &BTreeMap::from([(ProviderCode::RuffPython as i16, 0)]),
    )?;

    Ok(PythonSemanticProjection {
        provider_run_id: provider_run.id,
        observation,
        canonical,
        profile_completeness,
    })
}

fn validate_reference_edges(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    for reference in &batch.references {
        let expected = if reference.resolution == PythonResolution::MayReferTo {
            PythonSemanticEdgeKind::MayReferTo
        } else {
            PythonSemanticEdgeKind::RefersTo
        };
        if !batch.edges.iter().any(|edge| {
            edge.subject_id == reference.reference_id
                && edge.object_id == reference.target_id
                && edge.kind == expected
        }) {
            return Err(FactIngestError::Protocol(format!(
                "Ruff reference {} lacks its explicit {expected:?} edge",
                reference.name
            )));
        }
        match reference.resolution {
            PythonResolution::Resolved if reference.unknown_reason_code.is_some() => {
                return Err(FactIngestError::Protocol(format!(
                    "resolved Ruff reference {} carries an unknown reason",
                    reference.name
                )));
            }
            PythonResolution::MayReferTo | PythonResolution::UnknownSymbol
                if reference.unknown_reason_code.is_none() =>
            {
                return Err(FactIngestError::Protocol(format!(
                    "non-exact Ruff reference {} lacks an unknown reason",
                    reference.name
                )));
            }
            PythonResolution::Resolved
            | PythonResolution::MayReferTo
            | PythonResolution::UnknownSymbol
            | PythonResolution::UnboundLocal => {}
        }
    }
    Ok(())
}

fn validate_import_export_facts(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    let binding_ids = batch
        .bindings
        .iter()
        .map(|binding| binding.binding_id)
        .collect::<std::collections::BTreeSet<_>>();
    let imported_ids = batch
        .imports
        .iter()
        .filter_map(|import| import.imported_entity_id)
        .collect::<std::collections::BTreeSet<_>>();
    for import in &batch.imports {
        if import.kind == PythonImportKind::Star && !import.star_import {
            return Err(FactIngestError::Protocol(
                "Ruff STAR import lacks star_import=true".into(),
            ));
        }
        if import.target_module_name.is_none() && import.unknown_reason_code.is_none() {
            return Err(FactIngestError::Protocol(format!(
                "Ruff import {} has no module and no unknown reason",
                import.source_name
            )));
        }
        match import.resolution {
            PythonResolution::Resolved
                if import.ruff_qualified_name.is_none() || import.unknown_reason_code.is_some() =>
            {
                return Err(FactIngestError::Protocol(format!(
                    "resolved Ruff import {} lacks a qualified name or carries an unknown reason",
                    import.source_name
                )));
            }
            PythonResolution::MayReferTo | PythonResolution::UnknownSymbol
                if import.unknown_reason_code.is_none() =>
            {
                return Err(FactIngestError::Protocol(format!(
                    "non-exact Ruff import {} lacks an unknown reason",
                    import.source_name
                )));
            }
            PythonResolution::Resolved
            | PythonResolution::MayReferTo
            | PythonResolution::UnknownSymbol
            | PythonResolution::UnboundLocal => {}
        }
        if import
            .local_binding_id
            .is_some_and(|binding| !binding_ids.contains(&binding))
        {
            return Err(FactIngestError::Protocol(format!(
                "Ruff import {} references an absent local binding",
                import.source_name
            )));
        }
    }
    for export in &batch.exports {
        if !binding_ids.contains(&export.target_id) && !imported_ids.contains(&export.target_id) {
            return Err(FactIngestError::Protocol(format!(
                "Ruff export {} references an absent binding or imported entity",
                export.name
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One closed census makes cross-family referential checks reviewable.
fn validate_callable_facts(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    if batch.callables.is_empty()
        && batch.parameters.is_empty()
        && batch.callable_syntax.is_empty()
        && batch.call_sites.is_empty()
        && batch.call_arguments.is_empty()
        && batch.unknown_argument_sets.is_empty()
        && batch.members.is_empty()
        && batch.call_diagnostics.is_empty()
    {
        // Older hand-built fixtures exercise only the WP03/WP04 boundary. Real Ruff
        // projections always contain the synthetic module-body callable.
        return Ok(());
    }

    let scope_ids = batch
        .scopes
        .iter()
        .map(|fact| fact.scope_id)
        .collect::<std::collections::BTreeSet<_>>();
    let callable_ids = batch
        .callables
        .iter()
        .map(|fact| fact.callable_id)
        .collect::<std::collections::BTreeSet<_>>();
    if callable_ids.len() != batch.callables.len() {
        return Err(FactIngestError::Protocol(
            "Ruff callable projection contains duplicate callable IDs".into(),
        ));
    }
    for callable in &batch.callables {
        if !scope_ids.contains(&callable.owner_scope_id) {
            return Err(FactIngestError::Protocol(format!(
                "Ruff callable {} references an absent owner scope",
                callable.qualified_name
            )));
        }
    }

    let parameter_ids = batch
        .parameters
        .iter()
        .map(|fact| fact.parameter_id)
        .collect::<std::collections::BTreeSet<_>>();
    if parameter_ids.len() != batch.parameters.len() {
        return Err(FactIngestError::Protocol(
            "Ruff callable projection contains duplicate parameter IDs".into(),
        ));
    }
    for callable in &batch.callables {
        let mut ordinals = batch
            .parameters
            .iter()
            .filter(|parameter| parameter.callable_id == callable.callable_id)
            .map(|parameter| parameter.ordinal)
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        let expected_count = i32::try_from(ordinals.len()).unwrap_or(i32::MAX);
        let expected = (0..expected_count).collect::<Vec<_>>();
        if ordinals != expected || callable.parameter_count != expected_count {
            return Err(FactIngestError::Protocol(format!(
                "Ruff callable {} has a non-contiguous or inconsistent parameter contract",
                callable.qualified_name
            )));
        }
    }
    if batch
        .parameters
        .iter()
        .any(|parameter| !callable_ids.contains(&parameter.callable_id))
    {
        return Err(FactIngestError::Protocol(
            "Ruff parameter references an absent callable".into(),
        ));
    }

    let syntax_by_id = batch
        .callable_syntax
        .iter()
        .map(|fact| (fact.syntax_id, fact))
        .collect::<BTreeMap<_, _>>();
    if syntax_by_id.len() != batch.callable_syntax.len() {
        return Err(FactIngestError::Protocol(
            "Ruff callable projection contains duplicate syntax IDs".into(),
        ));
    }
    let call_site_ids = batch
        .call_sites
        .iter()
        .map(|fact| fact.call_site_id)
        .collect::<std::collections::BTreeSet<_>>();
    if call_site_ids.len() != batch.call_sites.len() {
        return Err(FactIngestError::Protocol(
            "Ruff callable projection contains duplicate call-site IDs".into(),
        ));
    }
    for call_site in &batch.call_sites {
        let call_syntax = syntax_by_id.get(&call_site.syntax_id);
        let callee_syntax = syntax_by_id.get(&call_site.callee_syntax_id);
        if !callable_ids.contains(&call_site.caller_id)
            || call_site.resolved_target_count != 0
            || !matches!(
                call_syntax.map(|fact| (fact.owner_id, fact.role)),
                Some((owner, PythonCallableSyntaxRole::CallExpression)) if owner == call_site.call_site_id
            )
            || !matches!(
                callee_syntax.map(|fact| (fact.owner_id, fact.role)),
                Some((owner, PythonCallableSyntaxRole::CalleeExpression)) if owner == call_site.call_site_id
            )
        {
            return Err(FactIngestError::Protocol(
                "Ruff call site lacks its caller, first-class call syntax, callee syntax, or unresolved WP13 placeholder".into(),
            ));
        }
        if call_site.receiver_syntax_id.is_some_and(|syntax_id| {
            !matches!(
                syntax_by_id.get(&syntax_id).map(|fact| (fact.owner_id, fact.role)),
                Some((owner, PythonCallableSyntaxRole::Receiver)) if owner == call_site.call_site_id
            )
        }) {
            return Err(FactIngestError::Protocol(
                "Ruff call-site receiver does not reference receiver syntax".into(),
            ));
        }
    }
    let syntactic_call_count = batch
        .callable_syntax
        .iter()
        .filter(|fact| fact.role == PythonCallableSyntaxRole::CallExpression)
        .count();
    if syntactic_call_count != batch.call_sites.len() {
        return Err(FactIngestError::Protocol(format!(
            "Ruff call-site parity failed: {syntactic_call_count} call syntax rows but {} call-site facts",
            batch.call_sites.len()
        )));
    }

    let argument_ids = batch
        .call_arguments
        .iter()
        .map(|fact| fact.argument_id)
        .collect::<std::collections::BTreeSet<_>>();
    if argument_ids.len() != batch.call_arguments.len() {
        return Err(FactIngestError::Protocol(
            "Ruff callable projection contains duplicate argument IDs".into(),
        ));
    }
    for call_site_id in &call_site_ids {
        let mut ordinals = batch
            .call_arguments
            .iter()
            .filter(|argument| argument.call_site_id == *call_site_id)
            .map(|argument| argument.ordinal)
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        let expected = (0..i32::try_from(ordinals.len()).unwrap_or(i32::MAX)).collect::<Vec<_>>();
        if ordinals != expected {
            return Err(FactIngestError::Protocol(
                "Ruff call arguments are not contiguous in source/binder order".into(),
            ));
        }
    }
    let unknown_ids = batch
        .unknown_argument_sets
        .iter()
        .map(|fact| fact.unknown_argument_set_id)
        .collect::<std::collections::BTreeSet<_>>();
    for argument in &batch.call_arguments {
        if !call_site_ids.contains(&argument.call_site_id) {
            return Err(FactIngestError::Protocol(
                "Ruff argument references an absent call site".into(),
            ));
        }
        if argument.argument_syntax_id.is_some_and(|syntax_id| {
            !matches!(
                syntax_by_id.get(&syntax_id).map(|fact| (fact.owner_id, fact.role)),
                Some((owner, PythonCallableSyntaxRole::Argument)) if owner == argument.argument_id
            )
        }) {
            return Err(FactIngestError::Protocol(
                "Ruff explicit argument does not reference argument syntax".into(),
            ));
        }
        let dynamic = matches!(
            argument.spread_kind,
            PythonArgumentSpreadKind::PositionalDynamic | PythonArgumentSpreadKind::KeywordDynamic
        );
        if dynamic
            && (argument.binding_status != PythonArgumentBindingStatus::UnknownArgumentSet
                || !argument
                    .parameter_id
                    .is_some_and(|target| unknown_ids.contains(&target)))
        {
            return Err(FactIngestError::Protocol(
                "Ruff dynamic splat lacks an explicit UNKNOWN_ARGUMENT_SET binding".into(),
            ));
        }
        if argument.parameter_id.is_some_and(|target| {
            !parameter_ids.contains(&target) && !unknown_ids.contains(&target)
        }) {
            return Err(FactIngestError::Protocol(
                "Ruff argument binding references an absent parameter/unknown set".into(),
            ));
        }
    }
    if batch.unknown_argument_sets.iter().any(|fact| {
        !call_site_ids.contains(&fact.call_site_id)
            || !matches!(
                fact.spread_kind,
                PythonArgumentSpreadKind::PositionalDynamic
                    | PythonArgumentSpreadKind::KeywordDynamic
            )
    }) {
        return Err(FactIngestError::Protocol(
            "Ruff unknown argument set is not owned by a dynamic call-site splat".into(),
        ));
    }
    if batch.call_diagnostics.iter().any(|fact| {
        !call_site_ids.contains(&fact.call_site_id) || !argument_ids.contains(&fact.argument_id)
    }) {
        return Err(FactIngestError::Protocol(
            "Ruff call diagnostic references an absent call site or argument".into(),
        ));
    }
    Ok(())
}

fn validate_cfg_facts(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    if batch.cfgs.is_empty() && batch.cfg_nodes.is_empty() && batch.cfg_edges.is_empty() {
        return Ok(());
    }
    let callable_ids = batch
        .callables
        .iter()
        .map(|callable| callable.callable_id)
        .collect::<BTreeSet<_>>();
    let cfg_ids = batch
        .cfgs
        .iter()
        .map(|cfg| cfg.cfg_id)
        .collect::<BTreeSet<_>>();
    if cfg_ids.len() != batch.cfgs.len() || batch.cfgs.len() != callable_ids.len() {
        return Err(FactIngestError::Protocol(
            "Ruff CFG projection must contain exactly one graph per callable".into(),
        ));
    }
    for cfg in &batch.cfgs {
        if !callable_ids.contains(&cfg.callable_id) || cfg.owner_id != cfg.callable_id {
            return Err(FactIngestError::Protocol(
                "Ruff CFG references an absent or inconsistent callable owner".into(),
            ));
        }
        validate_python_cfg(cfg, &batch.cfg_nodes, &batch.cfg_edges).map_err(|error| {
            FactIngestError::Protocol(format!("Ruff CFG validation failed: {}", error.message))
        })?;
    }
    let node_ids = batch
        .cfg_nodes
        .iter()
        .map(|node| node.cfg_node_id)
        .collect::<BTreeSet<_>>();
    if node_ids.len() != batch.cfg_nodes.len()
        || batch
            .cfg_nodes
            .iter()
            .any(|node| !cfg_ids.contains(&node.cfg_id) || !callable_ids.contains(&node.owner_id))
        || batch.cfg_edges.iter().any(|edge| {
            !cfg_ids.contains(&edge.cfg_id)
                || !callable_ids.contains(&edge.owner_id)
                || !node_ids.contains(&edge.source_node_id)
                || !node_ids.contains(&edge.target_node_id)
        })
    {
        return Err(FactIngestError::Protocol(
            "Ruff CFG detail rows contain duplicate or dangling identities".into(),
        ));
    }
    Ok(())
}

fn validate_dataflow_facts(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    let callable_ids = batch
        .callables
        .iter()
        .map(|fact| fact.callable_id)
        .collect::<BTreeSet<_>>();
    let cfg_node_ids = batch
        .cfg_nodes
        .iter()
        .map(|fact| fact.cfg_node_id)
        .collect::<BTreeSet<_>>();
    let binding_ids = batch
        .bindings
        .iter()
        .map(|fact| fact.binding_id)
        .collect::<BTreeSet<_>>();
    let syntax_ids = batch
        .references
        .iter()
        .map(|fact| fact.reference_id)
        .chain(batch.callable_syntax.iter().map(|fact| fact.syntax_id))
        .collect::<BTreeSet<_>>();
    let value_ids = batch
        .values
        .iter()
        .map(|fact| fact.value_id)
        .collect::<BTreeSet<_>>();
    let operation_ids = batch
        .operations
        .iter()
        .map(|fact| fact.operation_id)
        .collect::<BTreeSet<_>>();
    let event_ids = batch
        .dataflow_events
        .iter()
        .map(|fact| fact.event_id)
        .collect::<BTreeSet<_>>();
    let location_ids = batch
        .memory_locations
        .iter()
        .map(|fact| fact.location_id)
        .collect::<BTreeSet<_>>();
    let component_ids = batch
        .access_path_components
        .iter()
        .map(|fact| fact.component_id)
        .collect::<BTreeSet<_>>();
    if value_ids.len() != batch.values.len()
        || operation_ids.len() != batch.operations.len()
        || event_ids.len() != batch.dataflow_events.len()
        || location_ids.len() != batch.memory_locations.len()
        || component_ids.len() != batch.access_path_components.len()
    {
        return Err(FactIngestError::Protocol(
            "Python dataflow projection contains duplicate application-owned IDs".into(),
        ));
    }
    let stamped = |precision: &str, bundle: &str| {
        precision == crate::ruff_adapter::PYTHON_DATAFLOW_PRECISION_PROFILE
            && bundle == crate::ruff_adapter::PYTHON_DATAFLOW_BUNDLE_ID
    };
    if batch.values.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
            || fact
                .producer_operation_id
                .is_some_and(|id| !operation_ids.contains(&id))
            || fact
                .syntax_id
                .is_some_and(|id| !syntax_ids.contains(&id))
    }) || batch.operations.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
            || fact
                .cfg_node_id
                .is_some_and(|id| !cfg_node_ids.contains(&id))
            || fact
                .result_value_id
                .is_some_and(|id| !value_ids.contains(&id))
            || fact
                .syntax_id
                .is_some_and(|id| !syntax_ids.contains(&id))
    }) || batch.dataflow_events.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
            || fact
                .cfg_node_id
                .is_some_and(|id| !cfg_node_ids.contains(&id))
            || fact
                .binding_id
                .is_some_and(|id| !binding_ids.contains(&id))
            || fact.value_id.is_some_and(|id| !value_ids.contains(&id))
            || fact
                .location_id
                .is_some_and(|id| !location_ids.contains(&id))
            || fact
                .syntax_id
                .is_some_and(|id| !syntax_ids.contains(&id))
    }) || batch.memory_locations.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
            || fact
                .parent_location_id
                .is_some_and(|id| !location_ids.contains(&id))
    }) || batch.access_path_components.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !location_ids.contains(&fact.location_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
            || fact
                .index_value_id
                .is_some_and(|id| !value_ids.contains(&id))
    }) {
        return Err(FactIngestError::Protocol(
            "Python dataflow projection contains dangling identities or missing derivation stamps"
                .into(),
        ));
    }
    let endpoints = value_ids
        .union(&event_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    if batch.dataflow_relations.iter().any(|fact| {
        !callable_ids.contains(&fact.owner_id)
            || !endpoints.contains(&fact.source_id)
            || !endpoints.contains(&fact.target_id)
            || !stamped(fact.precision_profile_id, fact.derivation_bundle_id)
    }) {
        return Err(FactIngestError::Protocol(
            "Python dataflow relation contains a dangling endpoint or missing derivation stamp"
                .into(),
        ));
    }
    Ok(())
}

fn observation_batch(
    batch: &PythonFrontendBatch,
    payloads: &[Vec<u8>; 24],
) -> Result<RecordBatch, FactIngestError> {
    let schema = Arc::new(registered_provider_observation_arrow_schema(
        OBSERVATION_SCHEMA_ID,
    )?);
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![Some(batch.module_name.as_str())])),
        Arc::new(StringArray::from(vec![Some(
            batch.provider_image_fingerprint.as_str(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(payloads[0].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[1].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[2].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[3].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[4].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[5].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[6].as_slice())])),
        Arc::new(StringArray::from(vec![Some(export_status_name(
            batch.export_status,
        ))])),
        Arc::new(BinaryArray::from(vec![Some(payloads[7].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[8].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[9].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[10].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[11].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[12].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[13].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[14].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[15].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[16].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[17].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[18].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[19].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[20].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[21].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[22].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[23].as_slice())])),
    ];
    RecordBatch::try_new(schema, columns).map_err(FactIngestError::from)
}

#[allow(clippy::too_many_lines)] // One ordered payload census keeps the registered observation schema auditable.
fn observation_payloads(batch: &PythonFrontendBatch) -> Result<[Vec<u8>; 24], FactIngestError> {
    let scopes = batch
        .scopes
        .iter()
        .map(|fact| {
            json!({
                "scope_id": hex_id(fact.scope_id),
                "parent_scope_id": fact.parent_scope_id.map(hex_id),
                "kind": scope_kind_name(fact.kind),
                "name": fact.name,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let bindings = batch
        .bindings
        .iter()
        .map(|fact| {
            json!({
                "binding_id": hex_id(fact.binding_id),
                "scope_id": hex_id(fact.scope_id),
                "name": fact.name,
                "kind": binding_kind_name(fact.kind),
                "target_form": target_form_name(fact.target_form),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let references = batch
        .references
        .iter()
        .map(|fact| {
            json!({
                "reference_id": hex_id(fact.reference_id),
                "scope_id": hex_id(fact.scope_id),
                "target_id": hex_id(fact.target_id),
                "name": fact.name,
                "class": reference_class_name(fact.class),
                "resolution": resolution_name(fact.resolution),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
                "unknown_reason_code": fact.unknown_reason_code,
            })
        })
        .collect::<Vec<_>>();
    let unknowns = batch
        .unknown_symbols
        .iter()
        .map(|fact| {
            json!({
                "unknown_symbol_id": hex_id(fact.unknown_symbol_id),
                "scope_id": hex_id(fact.scope_id),
                "name": fact.name,
                "reason_code": fact.reason_code,
            })
        })
        .collect::<Vec<_>>();
    let edges = batch
        .edges
        .iter()
        .map(|edge| {
            json!({
                "subject_id": hex_id(edge.subject_id),
                "object_id": hex_id(edge.object_id),
                "kind": edge_kind_name(edge.kind),
            })
        })
        .collect::<Vec<_>>();
    let imports = batch
        .imports
        .iter()
        .map(|fact| {
            json!({
                "import_id": hex_id(fact.import_id),
                "scope_id": hex_id(fact.scope_id),
                "kind": import_kind_name(fact.kind),
                "relative_level": fact.relative_level,
                "source_name": fact.source_name,
                "alias_name": fact.alias_name,
                "star_import": fact.star_import,
                "target_module_id": hex_id(fact.target_module_id),
                "target_module_name": fact.target_module_name,
                "ruff_qualified_name": fact.ruff_qualified_name,
                "resolution": resolution_name(fact.resolution),
                "imported_entity_id": fact.imported_entity_id.map(hex_id),
                "imported_name": fact.imported_name,
                "local_binding_id": fact.local_binding_id.map(hex_id),
                "unknown_reason_code": fact.unknown_reason_code,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let exports = batch
        .exports
        .iter()
        .map(|fact| {
            json!({
                "export_id": hex_id(fact.export_id),
                "name": fact.name,
                "target_id": hex_id(fact.target_id),
                "reexport": fact.reexport,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let callables = batch
        .callables
        .iter()
        .map(|fact| {
            json!({
                "callable_id": hex_id(fact.callable_id),
                "owner_scope_id": hex_id(fact.owner_scope_id),
                "declared_binding_id": fact.declared_binding_id.map(hex_id),
                "class_id": fact.class_id.map(hex_id),
                "name": fact.name,
                "qualified_name": fact.qualified_name,
                "parameter_count": fact.parameter_count,
                "generic_parameter_count": fact.generic_parameter_count,
                "flags": fact.flags,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let parameters = batch
        .parameters
        .iter()
        .map(|fact| {
            json!({
                "parameter_id": hex_id(fact.parameter_id),
                "callable_id": hex_id(fact.callable_id),
                "ordinal": fact.ordinal,
                "name": fact.name,
                "kind": parameter_kind_name(fact.kind),
                "annotation_syntax_id": fact.annotation_syntax_id.map(hex_id),
                "default_syntax_id": fact.default_syntax_id.map(hex_id),
                "flags": fact.flags,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let callable_syntax = batch
        .callable_syntax
        .iter()
        .map(|fact| {
            json!({
                "syntax_id": hex_id(fact.syntax_id),
                "owner_id": hex_id(fact.owner_id),
                "role": callable_syntax_role_name(fact.role),
                "ordinal": fact.ordinal,
                "text": fact.text,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let call_sites = batch
        .call_sites
        .iter()
        .map(|fact| {
            json!({
                "call_site_id": hex_id(fact.call_site_id),
                "caller_id": hex_id(fact.caller_id),
                "syntax_id": hex_id(fact.syntax_id),
                "callee_syntax_id": hex_id(fact.callee_syntax_id),
                "receiver_syntax_id": fact.receiver_syntax_id.map(hex_id),
                "declared_target_id": fact.declared_target_id.map(hex_id),
                "dispatch_kind": dispatch_kind_name(fact.dispatch_kind),
                "resolved_target_count": fact.resolved_target_count,
                "resolution_status": "UNRESOLVED_WP13_PLACEHOLDER",
                "flags": fact.flags,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let call_arguments = batch
        .call_arguments
        .iter()
        .map(|fact| {
            json!({
                "argument_id": hex_id(fact.argument_id),
                "call_site_id": hex_id(fact.call_site_id),
                "ordinal": fact.ordinal,
                "keyword_name": fact.keyword_name,
                "argument_syntax_id": fact.argument_syntax_id.map(hex_id),
                "parameter_id": fact.parameter_id.map(hex_id),
                "binding_status": argument_binding_status_name(fact.binding_status),
                "spread_kind": argument_spread_kind_name(fact.spread_kind),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let unknown_argument_sets = batch
        .unknown_argument_sets
        .iter()
        .map(|fact| {
            json!({
                "unknown_argument_set_id": hex_id(fact.unknown_argument_set_id),
                "call_site_id": hex_id(fact.call_site_id),
                "spread_kind": argument_spread_kind_name(fact.spread_kind),
            })
        })
        .collect::<Vec<_>>();
    let members = batch
        .members
        .iter()
        .map(|fact| {
            json!({
                "member_id": hex_id(fact.member_id),
                "class_id": hex_id(fact.class_id),
                "declared_entity_id": fact.declared_entity_id.map(hex_id),
                "name": fact.name,
                "kind": member_kind_name(fact.kind),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let call_diagnostics = batch
        .call_diagnostics
        .iter()
        .map(|fact| {
            json!({
                "diagnostic_id": hex_id(fact.diagnostic_id),
                "call_site_id": hex_id(fact.call_site_id),
                "argument_id": hex_id(fact.argument_id),
                "code": fact.code,
                "message": fact.message,
            })
        })
        .collect::<Vec<_>>();
    let cfgs = batch
        .cfgs
        .iter()
        .map(|fact| {
            json!({
                "cfg_id": hex_id(fact.cfg_id),
                "owner_id": hex_id(fact.owner_id),
                "callable_id": hex_id(fact.callable_id),
                "kind": cfg_kind_name(fact.kind),
                "entry_node_id": hex_id(fact.entry_node_id),
                "exit_node_id": hex_id(fact.exit_node_id),
                "exceptional_exit_node_id": hex_id(fact.exceptional_exit_node_id),
                "node_count": fact.node_count,
                "edge_count": fact.edge_count,
                "flags": fact.flags,
            })
        })
        .collect::<Vec<_>>();
    let cfg_nodes = batch
        .cfg_nodes
        .iter()
        .map(|fact| {
            json!({
                "cfg_node_id": hex_id(fact.cfg_node_id),
                "cfg_id": hex_id(fact.cfg_id),
                "owner_id": hex_id(fact.owner_id),
                "kind": cfg_node_kind_name(fact.kind),
                "ordinal": fact.ordinal,
                "label": fact.label,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
                "flags": fact.flags,
            })
        })
        .collect::<Vec<_>>();
    let cfg_edges = batch
        .cfg_edges
        .iter()
        .map(|fact| {
            json!({
                "relation_id": hex_id(fact.relation_id),
                "cfg_id": hex_id(fact.cfg_id),
                "owner_id": hex_id(fact.owner_id),
                "source_node_id": hex_id(fact.source_node_id),
                "target_node_id": hex_id(fact.target_node_id),
                "kind": fact.kind.registry_name(),
                "condition_node_id": fact.condition_node_id.map(hex_id),
                "case_value_text": fact.case_value_text,
                "exception_category": fact.exception_category,
                "flags": fact.flags,
            })
        })
        .collect::<Vec<_>>();
    let values = batch
        .values
        .iter()
        .map(|fact| {
            json!({
                "value_id": hex_id(fact.value_id),
                "owner_id": hex_id(fact.owner_id),
                "value_kind_code": fact.kind.code(),
                "producer_operation_id": fact.producer_operation_id.map(hex_id),
                "syntax_id": fact.syntax_id.map(hex_id),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    let operations = batch
        .operations
        .iter()
        .map(|fact| {
            json!({
                "operation_id": hex_id(fact.operation_id),
                "owner_id": hex_id(fact.owner_id),
                "cfg_node_id": fact.cfg_node_id.map(hex_id),
                "operation_kind_code": fact.kind.code(),
                "result_value_id": fact.result_value_id.map(hex_id),
                "syntax_id": fact.syntax_id.map(hex_id),
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    let dataflow_events = batch
        .dataflow_events
        .iter()
        .map(|fact| {
            json!({
                "event_id": hex_id(fact.event_id),
                "owner_id": hex_id(fact.owner_id),
                "cfg_node_id": fact.cfg_node_id.map(hex_id),
                "event_kind_code": fact.kind.code(),
                "binding_id": fact.binding_id.map(hex_id),
                "value_id": fact.value_id.map(hex_id),
                "location_id": fact.location_id.map(hex_id),
                "syntax_id": fact.syntax_id.map(hex_id),
                "ordinal": fact.ordinal,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    let memory_locations = batch
        .memory_locations
        .iter()
        .map(|fact| {
            json!({
                "location_id": hex_id(fact.location_id),
                "owner_id": hex_id(fact.owner_id),
                "location_kind_code": fact.kind.code(),
                "base_entity_id": fact.base_entity_id.map(hex_id),
                "base_local_id": fact.base_local_id.map(hex_id),
                "parent_location_id": fact.parent_location_id.map(hex_id),
                "projection_depth": fact.projection_depth,
                "canonical_path_hash": hex_hash(fact.canonical_path_hash),
                "display_path": fact.display_path,
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    let access_path_components = batch
        .access_path_components
        .iter()
        .map(|fact| {
            json!({
                "component_id": hex_id(fact.component_id),
                "owner_id": hex_id(fact.owner_id),
                "location_id": hex_id(fact.location_id),
                "ordinal": fact.ordinal,
                "projection_kind_code": fact.kind.code(),
                "field_entity_id": fact.field_entity_id.map(hex_id),
                "index_value_id": fact.index_value_id.map(hex_id),
                "constant_index": fact.constant_index,
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    let dataflow_relations = batch
        .dataflow_relations
        .iter()
        .map(|fact| {
            json!({
                "relation_id": hex_id(fact.relation_id),
                "owner_id": hex_id(fact.owner_id),
                "source_id": hex_id(fact.source_id),
                "target_id": hex_id(fact.target_id),
                "kind": fact.kind.canonical_name(),
                "flags": fact.flags,
                "precision_profile_id": fact.precision_profile_id,
                "derivation_bundle_id": fact.derivation_bundle_id,
            })
        })
        .collect::<Vec<_>>();
    Ok([
        json_bytes(&scopes)?,
        json_bytes(&bindings)?,
        json_bytes(&references)?,
        json_bytes(&unknowns)?,
        json_bytes(&edges)?,
        json_bytes(&imports)?,
        json_bytes(&exports)?,
        json_bytes(&callables)?,
        json_bytes(&parameters)?,
        json_bytes(&callable_syntax)?,
        json_bytes(&call_sites)?,
        json_bytes(&call_arguments)?,
        json_bytes(&unknown_argument_sets)?,
        json_bytes(&members)?,
        json_bytes(&call_diagnostics)?,
        json_bytes(&cfgs)?,
        json_bytes(&cfg_nodes)?,
        json_bytes(&cfg_edges)?,
        json_bytes(&values)?,
        json_bytes(&operations)?,
        json_bytes(&dataflow_events)?,
        json_bytes(&memory_locations)?,
        json_bytes(&access_path_components)?,
        json_bytes(&dataflow_relations)?,
    ])
}

fn json_bytes(value: &[Value]) -> Result<Vec<u8>, FactIngestError> {
    serde_json::to_vec(value)
        .map_err(|error| FactIngestError::Protocol(format!("Ruff observation JSON: {error}")))
}

fn canonical_identity(
    identities: &BTreeMap<[u8; 16], DerivedIdentity>,
    semantic_id: [u8; 16],
    role: &str,
) -> Result<DerivedIdentity, FactIngestError> {
    identities.get(&semantic_id).copied().ok_or_else(|| {
        FactIngestError::Protocol(format!("Ruff {role} references an absent semantic ID"))
    })
}

fn required_entity_kind(
    name: &str,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    entity_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("entity kind {name} is absent")))
}

fn required_relation_kind(
    name: &str,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    relation_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("relation kind {name} is absent")))
}

fn required_fact_form(name: &str) -> Result<i16, FactIngestError> {
    fact_kind_code(name)
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol(format!("fact form {name} is absent")))
}

#[allow(clippy::too_many_arguments)]
fn push_relation(
    output: &mut Vec<RelationRow>,
    scope: FactScope,
    file_id: [u8; 16],
    kind: crate::registries::OntologyCodeEntry,
    source_id: [u8; 16],
    target_id: [u8; 16],
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    certainty_code: i16,
    resolution_code: i16,
) {
    let identity = derived_identity(
        b"relation",
        &[
            &scope.owner_id,
            &kind.code.to_be_bytes(),
            &source_id,
            &target_id,
        ],
    );
    output.push(RelationRow {
        scope,
        fact_id: identity.id,
        language: Language::Python as i16,
        relation_family_code: kind.family_code,
        relation_kind_code: kind.code,
        source_id,
        target_id,
        ordinal: None,
        role_code: None,
        distance: None,
        directness_code: Directness::Direct as i16,
        file_id: Some(file_id),
        start_byte,
        end_byte,
        certainty_code,
        resolution_code,
        producer_code: ProviderCode::RuffPython as i16,
        derivation_code: None,
        flags: 0,
        fact_hash64: digest_hash64(identity.digest),
    });
}

#[allow(clippy::too_many_arguments)]
fn push_ordered_relation(
    output: &mut Vec<RelationRow>,
    scope: FactScope,
    file_id: [u8; 16],
    kind: crate::registries::OntologyCodeEntry,
    source_id: [u8; 16],
    target_id: [u8; 16],
    ordinal: i32,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    certainty_code: i16,
    resolution_code: i16,
) {
    let identity = derived_identity(
        b"ordered-relation",
        &[
            &scope.owner_id,
            &kind.code.to_be_bytes(),
            &source_id,
            &target_id,
            &ordinal.to_be_bytes(),
        ],
    );
    output.push(RelationRow {
        scope,
        fact_id: identity.id,
        language: Language::Python as i16,
        relation_family_code: kind.family_code,
        relation_kind_code: kind.code,
        source_id,
        target_id,
        ordinal: Some(ordinal),
        role_code: None,
        distance: None,
        directness_code: Directness::Direct as i16,
        file_id: Some(file_id),
        start_byte,
        end_byte,
        certainty_code,
        resolution_code,
        producer_code: ProviderCode::RuffPython as i16,
        derivation_code: None,
        flags: 0,
        fact_hash64: digest_hash64(identity.digest),
    });
}

#[allow(clippy::too_many_arguments)]
fn evidence_row(
    scope: FactScope,
    provider_run_id: [u8; 16],
    fact_id: [u8; 16],
    fact_form_code: i16,
    file_id: Option<[u8; 16]>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    certainty_code: i16,
    resolution_code: i16,
) -> FactEvidenceRow {
    let observation_id = derived_identity(b"observation", &[&provider_run_id, &fact_id]).id;
    FactEvidenceRow {
        evidence_id: crate::identity::fact_evidence_id(provider_run_id, observation_id, fact_id),
        scope,
        fact_id,
        fact_form_code,
        provider_code: ProviderCode::RuffPython as i16,
        provider_version: PROVIDER_VERSION.into(),
        provider_run_id,
        observation_id,
        raw_kind_code: None,
        file_id,
        start_byte,
        end_byte,
        certainty_code,
        resolution_code,
        conflict_disposition_code: 10,
        cold_payload: None,
    }
}

fn derived_identity(domain: &[u8], parts: &[&[u8]]) -> DerivedIdentity {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-canonical-fact.v1");
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    let digest = *hasher.finalize().as_bytes();
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    DerivedIdentity { id, digest }
}

fn digest_hash64(digest: [u8; 32]) -> i64 {
    i64::from_be_bytes(digest[..8].try_into().expect("eight digest bytes"))
}

fn stable_text_hash64(value: &str) -> i64 {
    digest_hash64(*blake3::hash(value.as_bytes()).as_bytes())
}

fn coordinate(value: u64) -> Result<i64, FactIngestError> {
    i64::try_from(value)
        .map_err(|_| FactIngestError::Protocol("Ruff byte coordinate exceeds Int64".into()))
}

fn canonical_resolution(resolution: PythonResolution) -> (i16, i16) {
    match resolution {
        PythonResolution::Resolved => (
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        ),
        PythonResolution::MayReferTo => (
            EvidenceCertainty::SoundMay as i16,
            ResolutionClass::SoundPossible as i16,
        ),
        PythonResolution::UnknownSymbol | PythonResolution::UnboundLocal => (
            EvidenceCertainty::Unresolved as i16,
            ResolutionClass::Unresolved as i16,
        ),
    }
}

fn hex_id(id: [u8; 16]) -> String {
    let mut output = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn hex_hash(hash: [u8; 32]) -> String {
    let mut output = String::with_capacity(67);
    output.push_str("b3:");
    for byte in hash {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

const fn parameter_kind_code(kind: PythonParameterKind) -> i16 {
    match kind {
        PythonParameterKind::PositionalOnly => ParameterKind::PositionalOnly as i16,
        PythonParameterKind::PositionalOrKeyword => ParameterKind::PositionalOrKeyword as i16,
        PythonParameterKind::VarPositional => ParameterKind::VarPositional as i16,
        PythonParameterKind::KeywordOnly => ParameterKind::KeywordOnly as i16,
        PythonParameterKind::VarKeyword => ParameterKind::VarKeyword as i16,
    }
}

const fn cfg_kind_name(kind: PythonCfgKind) -> &'static str {
    match kind {
        PythonCfgKind::Module => "MODULE",
        PythonCfgKind::Function => "FUNCTION",
        PythonCfgKind::AsyncFunction => "ASYNC_FUNCTION",
        PythonCfgKind::Lambda => "LAMBDA",
    }
}

const fn cfg_node_kind_name(kind: PythonCfgNodeKind) -> &'static str {
    match kind {
        PythonCfgNodeKind::Entry => "ENTRY",
        PythonCfgNodeKind::Exit => "EXIT",
        PythonCfgNodeKind::BasicBlock => "BASIC_BLOCK",
        PythonCfgNodeKind::ExpressionOperation => "EXPRESSION_OPERATION",
        PythonCfgNodeKind::StatementOperation => "STATEMENT_OPERATION",
        PythonCfgNodeKind::Branch => "BRANCH",
        PythonCfgNodeKind::Switch => "SWITCH",
        PythonCfgNodeKind::LoopHeader => "LOOP_HEADER",
        PythonCfgNodeKind::ReturnPoint => "RETURN_POINT",
        PythonCfgNodeKind::ExceptionalExit => "EXCEPTIONAL_EXIT",
        PythonCfgNodeKind::SuspendPoint => "SUSPEND_POINT",
        PythonCfgNodeKind::ResumePoint => "RESUME_POINT",
    }
}

const fn parameter_kind_name(kind: PythonParameterKind) -> &'static str {
    match kind {
        PythonParameterKind::PositionalOnly => "POSITIONAL_ONLY",
        PythonParameterKind::PositionalOrKeyword => "POSITIONAL_OR_KEYWORD",
        PythonParameterKind::VarPositional => "VAR_POSITIONAL",
        PythonParameterKind::KeywordOnly => "KEYWORD_ONLY",
        PythonParameterKind::VarKeyword => "VAR_KEYWORD",
    }
}

const fn dispatch_kind_code(kind: PythonDispatchKind) -> i16 {
    match kind {
        PythonDispatchKind::DirectName => CallDispatchKind::DirectName as i16,
        PythonDispatchKind::Attribute => CallDispatchKind::Attribute as i16,
        PythonDispatchKind::Unknown => CallDispatchKind::Unknown as i16,
    }
}

const fn dispatch_kind_name(kind: PythonDispatchKind) -> &'static str {
    match kind {
        PythonDispatchKind::DirectName => "DIRECT_NAME",
        PythonDispatchKind::Attribute => "ATTRIBUTE",
        PythonDispatchKind::Unknown => "UNKNOWN",
    }
}

const fn argument_binding_status_code(status: PythonArgumentBindingStatus) -> i16 {
    match status {
        PythonArgumentBindingStatus::Bound => ArgumentBindingStatus::Bound as i16,
        PythonArgumentBindingStatus::BoundReceiver => ArgumentBindingStatus::BoundReceiver as i16,
        PythonArgumentBindingStatus::Defaulted => ArgumentBindingStatus::Defaulted as i16,
        PythonArgumentBindingStatus::MissingRequired => {
            ArgumentBindingStatus::MissingRequired as i16
        }
        PythonArgumentBindingStatus::Duplicate => ArgumentBindingStatus::DuplicateArgument as i16,
        PythonArgumentBindingStatus::PositionalOnlyKeyword => {
            ArgumentBindingStatus::PositionalOnlyKeyword as i16
        }
        PythonArgumentBindingStatus::TooManyPositional => {
            ArgumentBindingStatus::TooManyPositional as i16
        }
        PythonArgumentBindingStatus::UnmatchedKeyword => {
            ArgumentBindingStatus::UnmatchedKeyword as i16
        }
        PythonArgumentBindingStatus::UnresolvedTarget => {
            ArgumentBindingStatus::UnresolvedTarget as i16
        }
        PythonArgumentBindingStatus::UnknownArgumentSet => {
            ArgumentBindingStatus::UnknownArgumentSet as i16
        }
    }
}

const fn argument_binding_status_name(status: PythonArgumentBindingStatus) -> &'static str {
    match status {
        PythonArgumentBindingStatus::Bound => "BOUND",
        PythonArgumentBindingStatus::BoundReceiver => "BOUND_RECEIVER",
        PythonArgumentBindingStatus::Defaulted => "DEFAULTED",
        PythonArgumentBindingStatus::MissingRequired => "MISSING_REQUIRED",
        PythonArgumentBindingStatus::Duplicate => "DUPLICATE_ARGUMENT",
        PythonArgumentBindingStatus::PositionalOnlyKeyword => "POSITIONAL_ONLY_KEYWORD",
        PythonArgumentBindingStatus::TooManyPositional => "TOO_MANY_POSITIONAL",
        PythonArgumentBindingStatus::UnmatchedKeyword => "UNMATCHED_KEYWORD",
        PythonArgumentBindingStatus::UnresolvedTarget => "UNRESOLVED_TARGET",
        PythonArgumentBindingStatus::UnknownArgumentSet => "UNKNOWN_ARGUMENT_SET",
    }
}

const fn argument_spread_kind_code(kind: PythonArgumentSpreadKind) -> i16 {
    match kind {
        PythonArgumentSpreadKind::None => ArgumentSpreadKind::None as i16,
        PythonArgumentSpreadKind::PositionalStatic => ArgumentSpreadKind::PositionalStatic as i16,
        PythonArgumentSpreadKind::KeywordStatic => ArgumentSpreadKind::KeywordStatic as i16,
        PythonArgumentSpreadKind::PositionalDynamic => ArgumentSpreadKind::PositionalDynamic as i16,
        PythonArgumentSpreadKind::KeywordDynamic => ArgumentSpreadKind::KeywordDynamic as i16,
        PythonArgumentSpreadKind::BoundReceiver => ArgumentSpreadKind::BoundReceiver as i16,
        PythonArgumentSpreadKind::Default => ArgumentSpreadKind::Default as i16,
        PythonArgumentSpreadKind::Missing => ArgumentSpreadKind::Missing as i16,
    }
}

const fn argument_spread_kind_name(kind: PythonArgumentSpreadKind) -> &'static str {
    match kind {
        PythonArgumentSpreadKind::None => "NONE",
        PythonArgumentSpreadKind::PositionalStatic => "POSITIONAL_STATIC",
        PythonArgumentSpreadKind::KeywordStatic => "KEYWORD_STATIC",
        PythonArgumentSpreadKind::PositionalDynamic => "POSITIONAL_DYNAMIC",
        PythonArgumentSpreadKind::KeywordDynamic => "KEYWORD_DYNAMIC",
        PythonArgumentSpreadKind::BoundReceiver => "BOUND_RECEIVER",
        PythonArgumentSpreadKind::Default => "DEFAULT",
        PythonArgumentSpreadKind::Missing => "MISSING",
    }
}

const fn callable_syntax_role_name(role: PythonCallableSyntaxRole) -> &'static str {
    match role {
        PythonCallableSyntaxRole::CallExpression => "CALL_EXPRESSION",
        PythonCallableSyntaxRole::CalleeExpression => "CALLEE_EXPRESSION",
        PythonCallableSyntaxRole::Receiver => "RECEIVER",
        PythonCallableSyntaxRole::Argument => "ARGUMENT",
        PythonCallableSyntaxRole::Decorator => "DECORATOR",
        PythonCallableSyntaxRole::ReturnAnnotation => "RETURN_ANNOTATION",
        PythonCallableSyntaxRole::ParameterAnnotation => "PARAMETER_ANNOTATION",
        PythonCallableSyntaxRole::ParameterDefault => "PARAMETER_DEFAULT",
        PythonCallableSyntaxRole::TypeParameter => "TYPE_PARAMETER",
    }
}

const fn member_kind_name(kind: PythonMemberKind) -> &'static str {
    match kind {
        PythonMemberKind::Method => "METHOD",
        PythonMemberKind::ClassVariable => "CLASS_VARIABLE",
        PythonMemberKind::PropertyCandidate => "PROPERTY_CANDIDATE",
        PythonMemberKind::NestedType => "NESTED_TYPE",
        PythonMemberKind::InstanceVariable => "INSTANCE_VARIABLE",
    }
}

fn call_diagnostic_code(code: &str) -> i32 {
    match code {
        "DUPLICATE_ARGUMENT" => ArgumentBindingStatus::DuplicateArgument as i32,
        "POSITIONAL_ONLY_KEYWORD" => ArgumentBindingStatus::PositionalOnlyKeyword as i32,
        "TOO_MANY_POSITIONAL" => ArgumentBindingStatus::TooManyPositional as i32,
        "UNMATCHED_KEYWORD" => ArgumentBindingStatus::UnmatchedKeyword as i32,
        _ => 0,
    }
}

const fn import_kind_code(kind: PythonImportKind) -> i16 {
    match kind {
        PythonImportKind::Module => 10,
        PythonImportKind::FromName => 20,
        PythonImportKind::Star => 30,
        PythonImportKind::Dynamic => 40,
    }
}

const fn import_kind_name(kind: PythonImportKind) -> &'static str {
    match kind {
        PythonImportKind::Module => "MODULE",
        PythonImportKind::FromName => "FROM_NAME",
        PythonImportKind::Star => "STAR",
        PythonImportKind::Dynamic => "DYNAMIC",
    }
}

const fn export_status_name(status: PythonExportStatus) -> &'static str {
    match status {
        PythonExportStatus::Complete => "COMPLETE",
        PythonExportStatus::IncompleteDynamic => "INCOMPLETE_DYNAMIC",
    }
}

const fn scope_kind_name(kind: PythonScopeKind) -> &'static str {
    match kind {
        PythonScopeKind::Module => "MODULE",
        PythonScopeKind::Function => "FUNCTION",
        PythonScopeKind::Class => "CLASS",
        PythonScopeKind::Lambda => "LAMBDA",
        PythonScopeKind::Comprehension => "COMPREHENSION",
        PythonScopeKind::Annotation => "ANNOTATION",
        PythonScopeKind::TypeParameter => "TYPE_PARAMETER",
    }
}

const fn binding_kind_name(kind: PythonBindingKind) -> &'static str {
    match kind {
        PythonBindingKind::Local => "LOCAL",
        PythonBindingKind::Parameter => "PARAMETER",
        PythonBindingKind::Global => "GLOBAL",
        PythonBindingKind::Nonlocal => "NONLOCAL",
        PythonBindingKind::Import => "IMPORT",
        PythonBindingKind::ClassAttribute => "CLASS_ATTRIBUTE",
        PythonBindingKind::InstanceAttribute => "INSTANCE_ATTRIBUTE",
        PythonBindingKind::Comprehension => "COMPREHENSION",
        PythonBindingKind::Loop => "LOOP",
        PythonBindingKind::With => "WITH",
        PythonBindingKind::Exception => "EXCEPTION",
        PythonBindingKind::Match => "MATCH",
        PythonBindingKind::Walrus => "WALRUS",
        PythonBindingKind::TypeParameter => "TYPE_PARAMETER",
        PythonBindingKind::TypeAlias => "TYPE_ALIAS",
        PythonBindingKind::Free => "FREE",
        PythonBindingKind::Cell => "CELL",
        PythonBindingKind::Builtin => "BUILTIN",
        PythonBindingKind::Function => "FUNCTION",
        PythonBindingKind::Class => "CLASS",
    }
}

const fn target_form_name(form: PythonTargetForm) -> &'static str {
    match form {
        PythonTargetForm::FunctionName => "FUNCTION_NAME",
        PythonTargetForm::ClassName => "CLASS_NAME",
        PythonTargetForm::Parameter => "PARAMETER",
        PythonTargetForm::Assignment => "ASSIGNMENT",
        PythonTargetForm::AnnotatedAssignment => "ANNOTATED_ASSIGNMENT",
        PythonTargetForm::AugmentedAssignment => "AUGMENTED_ASSIGNMENT",
        PythonTargetForm::NamedExpression => "NAMED_EXPRESSION",
        PythonTargetForm::ImportAlias => "IMPORT_ALIAS",
        PythonTargetForm::LoopTarget => "LOOP_TARGET",
        PythonTargetForm::WithTarget => "WITH_TARGET",
        PythonTargetForm::ExceptionTarget => "EXCEPTION_TARGET",
        PythonTargetForm::MatchCapture => "MATCH_CAPTURE",
        PythonTargetForm::ComprehensionTarget => "COMPREHENSION_TARGET",
        PythonTargetForm::GlobalDeclaration => "GLOBAL_DECLARATION",
        PythonTargetForm::NonlocalDeclaration => "NONLOCAL_DECLARATION",
        PythonTargetForm::TypeParameter => "TYPE_PARAMETER",
        PythonTargetForm::TypeAlias => "TYPE_ALIAS",
    }
}

const fn reference_class_name(class: PythonReferenceClass) -> &'static str {
    match class {
        PythonReferenceClass::Read => "READ",
        PythonReferenceClass::Write => "WRITE",
        PythonReferenceClass::ReadWrite => "READ_WRITE",
        PythonReferenceClass::Delete => "DELETE",
        PythonReferenceClass::TypeReference => "TYPE_REFERENCE",
        PythonReferenceClass::CallReference => "CALL_REFERENCE",
        PythonReferenceClass::ImportReference => "IMPORT_REFERENCE",
    }
}

const fn resolution_name(resolution: PythonResolution) -> &'static str {
    match resolution {
        PythonResolution::Resolved => "RESOLVED",
        PythonResolution::MayReferTo => "MAY_REFER_TO",
        PythonResolution::UnknownSymbol => "UNKNOWN_SYMBOL",
        PythonResolution::UnboundLocal => "UNBOUND_LOCAL",
    }
}

const fn edge_kind_name(kind: PythonSemanticEdgeKind) -> &'static str {
    match kind {
        PythonSemanticEdgeKind::RefersTo => "REFERS_TO",
        PythonSemanticEdgeKind::MayReferTo => "MAY_REFER_TO",
        PythonSemanticEdgeKind::Shadows => "SHADOWS",
        PythonSemanticEdgeKind::Rebinds => "REBINDS",
        PythonSemanticEdgeKind::GlobalResolution => "GLOBAL_RESOLUTION",
        PythonSemanticEdgeKind::NonlocalResolution => "NONLOCAL_RESOLUTION",
        PythonSemanticEdgeKind::Captures => "CAPTURES",
        PythonSemanticEdgeKind::CapturedFrom => "CAPTURED_FROM",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::fabric::batch_checksum;
    use crate::ruff_adapter::{
        PythonBindingFact, PythonReferenceFact, PythonScopeFact, PythonSemanticEdge,
        PythonSemanticMetrics, PythonSemanticTerminal,
    };

    fn scope(owner: u8) -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: [2; 16],
            source_generation: 7,
            owner_id: [owner; 16],
        }
    }

    fn fixture(module: &str, extra_binding: bool) -> PythonFrontendBatch {
        let module_scope = [10; 16];
        let binding = [20; 16];
        let reference = [30; 16];
        let mut bindings = vec![PythonBindingFact {
            binding_id: binding,
            scope_id: module_scope,
            name: "value".into(),
            kind: PythonBindingKind::Local,
            target_form: PythonTargetForm::Assignment,
            start_byte: 0,
            end_byte: 5,
        }];
        if extra_binding {
            bindings.push(PythonBindingFact {
                binding_id: [21; 16],
                scope_id: module_scope,
                name: "changed".into(),
                kind: PythonBindingKind::Local,
                target_form: PythonTargetForm::Assignment,
                start_byte: 12,
                end_byte: 19,
            });
        }
        PythonFrontendBatch {
            module_id: [9; 16],
            module_name: module.into(),
            provider_image_fingerprint: format!("b3:{module}"),
            scopes: vec![PythonScopeFact {
                scope_id: module_scope,
                parent_scope_id: None,
                kind: PythonScopeKind::Module,
                name: Some(module.into()),
                start_byte: 0,
                end_byte: if extra_binding { 20 } else { 11 },
            }],
            bindings,
            references: vec![PythonReferenceFact {
                reference_id: reference,
                scope_id: module_scope,
                name: "value".into(),
                class: PythonReferenceClass::Read,
                resolution: PythonResolution::Resolved,
                target_id: binding,
                start_byte: 6,
                end_byte: 11,
                unknown_reason_code: None,
            }],
            unknown_symbols: Vec::new(),
            edges: vec![PythonSemanticEdge {
                subject_id: reference,
                object_id: binding,
                kind: PythonSemanticEdgeKind::RefersTo,
            }],
            imports: Vec::new(),
            exports: Vec::new(),
            export_status: PythonExportStatus::Complete,
            callables: Vec::new(),
            parameters: Vec::new(),
            callable_syntax: Vec::new(),
            call_sites: Vec::new(),
            call_arguments: Vec::new(),
            unknown_argument_sets: Vec::new(),
            members: Vec::new(),
            call_diagnostics: Vec::new(),
            cfgs: Vec::new(),
            cfg_nodes: Vec::new(),
            cfg_edges: Vec::new(),
            values: Vec::new(),
            operations: Vec::new(),
            dataflow_events: Vec::new(),
            memory_locations: Vec::new(),
            access_path_components: Vec::new(),
            dataflow_relations: Vec::new(),
            metrics: PythonSemanticMetrics {
                binding_pass_duration: Duration::ZERO,
                traversal_pass_duration: Duration::ZERO,
                cleanup_duration: Duration::ZERO,
                visited_nodes: 3,
                scope_count: 1,
                binding_count: if extra_binding { 2 } else { 1 },
                reference_count: 1,
                unresolved_reference_count: 0,
                import_count: 0,
                export_count: 0,
                unresolved_module_count: 0,
                callable_count: 0,
                parameter_count: 0,
                call_site_count: 0,
                call_argument_count: 0,
                member_count: 0,
                cfg_count: 0,
                cfg_node_count: 0,
                cfg_edge_count: 0,
                dataflow_value_count: 0,
                dataflow_operation_count: 0,
                dataflow_event_count: 0,
                memory_location_count: 0,
                access_path_component_count: 0,
                dataflow_relation_count: 0,
                dataflow_iteration_count: 0,
            },
            terminal: PythonSemanticTerminal {
                pass_id: "PASS_RUFF_SCOPE_BINDING_V1",
                provider_id: "ruff-python",
                terminal_state: "completed",
                failure_code: None,
            },
        }
    }

    #[test]
    fn py_scope_binding_owner_replacement_gate() {
        let changed_before =
            project_ruff_semantic_batch(scope(10), [40; 16], &fixture("owner_a", false)).unwrap();
        let changed_after =
            project_ruff_semantic_batch(scope(10), [40; 16], &fixture("owner_a", true)).unwrap();
        let stable_before =
            project_ruff_semantic_batch(scope(11), [41; 16], &fixture("owner_b", false)).unwrap();
        let stable_after =
            project_ruff_semantic_batch(scope(11), [41; 16], &fixture("owner_b", false)).unwrap();

        assert_eq!(
            changed_before.observation.schema(),
            stable_before.observation.schema()
        );
        for table_code in [200, 210, 220] {
            let a_before =
                batch_checksum(changed_before.batch(table_code).unwrap().batch()).unwrap();
            let a_after = batch_checksum(changed_after.batch(table_code).unwrap().batch()).unwrap();
            let b_before =
                batch_checksum(stable_before.batch(table_code).unwrap().batch()).unwrap();
            let b_after = batch_checksum(stable_after.batch(table_code).unwrap().batch()).unwrap();
            assert_eq!(b_before, b_after, "unchanged owner table {table_code}");
            if table_code == 220 {
                assert_eq!(a_before, a_after, "unchanged reference detail table");
            } else {
                assert_ne!(a_before, a_after, "changed owner table {table_code}");
            }
        }
        assert_ne!(
            changed_before.provider_run_id,
            stable_before.provider_run_id
        );
    }
}
