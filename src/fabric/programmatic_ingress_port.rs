//! Application-owned projection of the released semantic request into epoch-bound relations.
//!
//! Released form names are compatibility observations, not executor identities. This port uses
//! explicit typed mappings for every wire field, resolves one installed program by the tuple of
//! released form and semantic output role, and then validates the complete product against the
//! already-admitted epoch catalog. No program binding ID or execution program pin is embedded in
//! this module.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::contracts::jcs::canonicalize_slice;
use crate::relational_program::FieldId;
use crate::relational_semantic_query::{
    EpochBoundBlockBindingRow, EpochBoundConsumerSlotBindingRow, EpochBoundDependencyRow,
    EpochBoundProgramBindingRow, EpochBoundRequestInputFieldValue, EpochBoundRequestInputRow,
    EpochBoundReturnRow, EpochBoundScopeRow, EpochBoundSelectionRow, EpochBoundSemanticIngress,
    EpochBoundSemanticIngressCatalog, EpochBoundSemanticIngressLimits, ReleasedSemanticForm,
    SemanticClauseValue, epoch_bound_semantic_ingress_limits_pin,
    validate_epoch_bound_semantic_ingress,
};
use crate::semantic_query_contract::{
    COMPILED_V2_0_SCOPE_DEFINITIONS, CompiledV20ScopeRole, ParsedSemanticRequest, PatternBinding,
    PatternRelationship, PriorResultReference, ResultRole, ReturnSpec, SemanticQueryClause,
    SemanticQueryRequest, SemanticReference, parse_request,
};

use super::production_kernel::CompiledQueryAuthority;
use super::programmatic_query_backend::{
    ProgrammaticQueryPortError, ProgrammaticSemanticIngressPort, canonical_request_content_pin,
    compiled_query_release_pin,
};
use super::programmatic_workspace::{ProgrammaticWorkspaceRuntime, WorkspaceEpochQueryAuthority};

/// Mapping from one compiled v2.0 scope role to its installed relation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProgrammaticGlobalIngressMapping {
    role: CompiledV20ScopeRole,
    scope_id: Arc<str>,
}

/// Every field carried by one of the eight released query forms.
///
/// Common return fields are explicit entries so adding, removing, or forgetting a projection is
/// detected by the mapping-set equality check rather than silently ignored.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProgrammaticFormIngressField {
    Label,
    LookingFor,
    Within,
    Where,
    About,
    Facts,
    At,
    StartingFrom,
    Relationship,
    Direction,
    Distance,
    StopWhen,
    EndingAt,
    Through,
    PathPolicy,
    MaximumLength,
    PatternBindings,
    PatternRelationships,
    Inputs,
    Combination,
    Identity,
    PreserveOrigin,
    Input,
    Summaries,
    GroupBy,
    IncludeSupport,
    ForInputs,
    Context,
    TextHandling,
    ReturnInclude,
    ReturnExclude,
    ReturnResultShape,
    ReturnGroupBy,
    ReturnOrderBy,
    ReturnDeduplicateBy,
    ReturnSupportingFacts,
    ReturnIncludeQueryResult,
    ReturnMaximumResults,
    ReturnPer,
    ReturnWhenExceeded,
}

const COMMON_FIELDS: [ProgrammaticFormIngressField; 12] = [
    ProgrammaticFormIngressField::Label,
    ProgrammaticFormIngressField::ReturnInclude,
    ProgrammaticFormIngressField::ReturnExclude,
    ProgrammaticFormIngressField::ReturnResultShape,
    ProgrammaticFormIngressField::ReturnGroupBy,
    ProgrammaticFormIngressField::ReturnOrderBy,
    ProgrammaticFormIngressField::ReturnDeduplicateBy,
    ProgrammaticFormIngressField::ReturnSupportingFacts,
    ProgrammaticFormIngressField::ReturnIncludeQueryResult,
    ProgrammaticFormIngressField::ReturnMaximumResults,
    ProgrammaticFormIngressField::ReturnPer,
    ProgrammaticFormIngressField::ReturnWhenExceeded,
];

/// Tuple mapping for a list of semantic references.
///
/// Every reference becomes one request-owned tuple. Prior-result references additionally become
/// a dependency in `consumer_slot_id`; the exact consumer role is read from the selected program
/// catalog rather than supplied here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticReferenceInputMapping {
    pub input_id: Arc<str>,
    pub kind_field_id: FieldId,
    pub value_field_id: FieldId,
    pub producer_role_field_id: FieldId,
    pub consumer_slot_id: Arc<str>,
}

/// Request-owned tuple mappings for one pattern binding and its nested repeated values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticPatternBindingInputMapping {
    pub binding_input_id: Arc<str>,
    pub binding_name_field_id: FieldId,
    pub looking_for_field_id: FieldId,
    pub within: ProgrammaticReferenceInputMapping,
    pub within_binding_name_field_id: FieldId,
    pub where_input_id: Arc<str>,
    pub where_binding_name_field_id: FieldId,
    pub where_value_field_id: FieldId,
}

/// Request-owned tuple mapping for one pattern relationship record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticPatternRelationshipInputMapping {
    pub input_id: Arc<str>,
    pub from_field_id: FieldId,
    pub to_field_id: FieldId,
    pub relationship_field_id: FieldId,
    pub direction_field_id: FieldId,
    pub distance_field_id: FieldId,
}

/// Typed destination for one released-form field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammaticFormIngressTarget {
    Selection {
        selection_id: Arc<str>,
    },
    Return {
        return_id: Arc<str>,
    },
    References(ProgrammaticReferenceInputMapping),
    PatternBindings(ProgrammaticPatternBindingInputMapping),
    PatternRelationships(ProgrammaticPatternRelationshipInputMapping),
    /// The effective released limit is bound both to the block and to this return relation.
    ExplicitResultLimit {
        return_id: Arc<str>,
    },
}

/// One explicit source-field to normalized-relation mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticFormIngressMappingRow {
    pub field: ProgrammaticFormIngressField,
    pub target: ProgrammaticFormIngressTarget,
}

/// Complete field mapping for one released compatibility form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticFormIngressMapping {
    pub form: ReleasedSemanticForm,
    pub fields: Vec<ProgrammaticFormIngressMappingRow>,
}

/// Data-owned mapping from a released result role to the installed semantic role identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticResultRoleMapping {
    pub role: ResultRole,
    pub role_id: Arc<str>,
}

/// Concrete application-owned semantic ingress port.
///
/// Production construction accepts only the compiled query-release capability and operational
/// limits. Semantic identities and field mappings are closed private data; program binding IDs and
/// execution pins are selected from the admitted epoch catalog at request projection time.
#[derive(Clone, Debug)]
pub struct ApplicationOwnedSemanticIngressPort {
    authority_pin: [u8; 32],
    released_specification: Arc<str>,
    released_version: Arc<str>,
    limits: EpochBoundSemanticIngressLimits,
    roles: BTreeMap<ResultRole, Arc<str>>,
    globals: BTreeMap<CompiledV20ScopeRole, Arc<str>>,
    forms: BTreeMap<
        ReleasedSemanticForm,
        BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    >,
}

impl ApplicationOwnedSemanticIngressPort {
    /// Construct the sole compiled application-owned projection for the released 2.0 contract.
    ///
    /// Version 2.0 retains the eight semantic forms while replacing several external field
    /// spellings and the request-global scope/freshness objects. [`parse_request`] normalizes
    /// those released spellings into the same typed form vocabulary before this port binds them
    /// to an admitted epoch catalog. The opaque release capability and fixed mappings prevent an
    /// operational caller from selecting another version, role, global, or form mapping; only the
    /// resource limits remain variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the compiled mapping is internally incomplete or ambiguous.
    pub(crate) fn try_compiled_v2_0(
        compiled_release: &CompiledQueryAuthority,
        limits: EpochBoundSemanticIngressLimits,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        Self::build(
            compiled_query_release_pin(compiled_release),
            "composable semantic CPG fact query",
            "2.0",
            limits,
            compiled_v2_0_roles(),
            compiled_v2_0_globals(),
            compiled_v2_0_forms(),
        )
    }

    /// Test-only construction hook retaining an explicit pin for independent evidence fixtures.
    #[cfg(test)]
    pub(crate) fn try_released_v2_0(
        authority_pin: [u8; 32],
        limits: EpochBoundSemanticIngressLimits,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        Self::build(
            authority_pin,
            "composable semantic CPG fact query",
            "2.0",
            limits,
            compiled_v2_0_roles(),
            compiled_v2_0_globals(),
            compiled_v2_0_forms(),
        )
    }

    /// Test-only generic constructor for exercising mapping validation faults.
    ///
    /// # Errors
    ///
    /// Rejects a sentinel authority, an empty released-contract identity, duplicate or incomplete
    /// role/global/form mappings, incompatible field destinations, or reused semantic binding IDs.
    #[cfg(test)]
    fn try_new(
        authority_pin: [u8; 32],
        released_specification: impl Into<Arc<str>>,
        released_version: impl Into<Arc<str>>,
        limits: EpochBoundSemanticIngressLimits,
        roles: Vec<ProgrammaticResultRoleMapping>,
        globals: Vec<ProgrammaticGlobalIngressMapping>,
        forms: Vec<ProgrammaticFormIngressMapping>,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        Self::build(
            authority_pin,
            released_specification,
            released_version,
            limits,
            roles,
            globals,
            forms,
        )
    }

    fn build(
        authority_pin: [u8; 32],
        released_specification: impl Into<Arc<str>>,
        released_version: impl Into<Arc<str>>,
        limits: EpochBoundSemanticIngressLimits,
        roles: Vec<ProgrammaticResultRoleMapping>,
        globals: Vec<ProgrammaticGlobalIngressMapping>,
        forms: Vec<ProgrammaticFormIngressMapping>,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        if authority_pin == [0; 32] {
            return Err(rejected("semantic ingress authority pin is a sentinel"));
        }
        let released_specification = released_specification.into();
        let released_version = released_version.into();
        validate_text_identity("released specification", &released_specification)?;
        validate_text_identity("released version", &released_version)?;

        let mut role_map = BTreeMap::new();
        let mut role_ids = BTreeSet::new();
        for mapping in roles {
            validate_text_identity("semantic result role", &mapping.role_id)?;
            if role_map
                .insert(mapping.role, Arc::clone(&mapping.role_id))
                .is_some()
            {
                return Err(rejected("duplicate semantic result-role mapping"));
            }
            if !role_ids.insert(mapping.role_id) {
                return Err(rejected("semantic result-role identities are ambiguous"));
            }
        }
        let expected_roles = all_result_roles().into_iter().collect::<BTreeSet<_>>();
        if role_map.keys().copied().collect::<BTreeSet<_>>() != expected_roles {
            return Err(rejected("semantic result-role mapping is incomplete"));
        }

        let mut global_map = BTreeMap::new();
        let mut global_ids = BTreeSet::new();
        for mapping in globals {
            validate_text_identity("semantic scope", &mapping.scope_id)?;
            if global_map
                .insert(mapping.role, Arc::clone(&mapping.scope_id))
                .is_some()
            {
                return Err(rejected("duplicate request-global field mapping"));
            }
            if !global_ids.insert(mapping.scope_id) {
                return Err(rejected("request-global scope identities are ambiguous"));
            }
        }
        if global_map.keys().copied().collect::<BTreeSet<_>>()
            != COMPILED_V2_0_SCOPE_DEFINITIONS
                .into_iter()
                .map(|definition| definition.role)
                .collect::<BTreeSet<_>>()
        {
            return Err(rejected("request-global field mapping is incomplete"));
        }

        let mut form_map = BTreeMap::new();
        for mapping in forms {
            if form_map.contains_key(&mapping.form) {
                return Err(rejected(format!(
                    "duplicate mapping for released form {}",
                    mapping.form.label()
                )));
            }
            let expected = expected_fields(mapping.form);
            let mut fields = BTreeMap::new();
            let mut target_ids = BTreeSet::new();
            for row in mapping.fields {
                validate_target(mapping.form, row.field, &row.target, &mut target_ids)?;
                if fields.insert(row.field, row.target).is_some() {
                    return Err(rejected(format!(
                        "duplicate field mapping for {}::{:?}",
                        mapping.form.label(),
                        row.field
                    )));
                }
            }
            if fields.keys().copied().collect::<BTreeSet<_>>() != expected {
                return Err(rejected(format!(
                    "field mapping for {} is incomplete or contains an unmapped field",
                    mapping.form.label()
                )));
            }
            form_map.insert(mapping.form, fields);
        }
        if form_map.keys().copied().collect::<BTreeSet<_>>()
            != ReleasedSemanticForm::ALL
                .into_iter()
                .collect::<BTreeSet<_>>()
        {
            return Err(rejected("released-form mapping is not exhaustive"));
        }

        Ok(Self {
            authority_pin,
            released_specification,
            released_version,
            limits,
            roles: role_map,
            globals: global_map,
            forms: form_map,
        })
    }

    fn project_against_catalog(
        &self,
        request: &ParsedSemanticRequest,
        catalog: &EpochBoundSemanticIngressCatalog,
    ) -> Result<EpochBoundSemanticIngress, ProgrammaticQueryPortError> {
        self.validate_request_shape(request)?;
        let expected_limits_pin = epoch_bound_semantic_ingress_limits_pin(self.limits);
        if expected_limits_pin != catalog.limits_pin {
            return Err(rejected(
                "semantic ingress limits do not match the admitted epoch catalog",
            ));
        }

        let mut projection = IngressProjection::default();
        self.project_globals(&request.request, &mut projection)?;
        let mut blocks = Vec::with_capacity(request.request.queries.len());

        for clause in &request.request.queries {
            let form = released_form(clause);
            let output_role_id = self.role_id(clause.output_role())?;
            let binding = select_program_binding(catalog, form, output_role_id)?;
            let query_id: Arc<str> = Arc::from(clause.query_id());
            blocks.push(EpochBoundBlockBindingRow {
                query_id: Arc::clone(&query_id),
                compatibility_form: form,
                program_binding_id: Arc::clone(&binding.program_binding_id),
                program_binding_pin: binding.program_binding_pin,
                output_role_id: Arc::clone(output_role_id),
                explicit_result_limit: Some(clause.maximum_results()),
            });
            let fields = self.forms.get(&form).ok_or_else(|| {
                rejected(format!(
                    "no field mapping for released form {}",
                    form.label()
                ))
            })?;
            self.project_clause(clause, fields, binding, catalog, &mut projection)?;
        }

        let dependency_order = dependency_order(&blocks, &projection.dependencies)?;
        let ingress = EpochBoundSemanticIngress {
            semantic_request_id: Arc::from(request.request.semantic_request_id.as_str()),
            request_content_pin: canonical_request_content_pin(&request.canonical_bytes),
            fabric_epoch_pin: catalog.fabric_epoch_pin,
            program_catalog_pin: catalog.program_catalog_pin,
            source_pin: catalog.source_pin,
            policy_pin: catalog.policy_pin,
            producer_closure_proof_pin: catalog.producer_closure_proof_pin,
            limits_pin: catalog.limits_pin,
            limits: self.limits,
            blocks,
            selections: projection.selections,
            returns: projection.returns,
            scopes: projection.scopes,
            request_inputs: projection.request_inputs,
            dependencies: projection.dependencies,
            dependency_order,
        };
        validate_epoch_bound_semantic_ingress(ingress.clone(), catalog).map_err(|error| {
            rejected(format!("epoch-bound semantic ingress is invalid: {error}"))
        })?;
        Ok(ingress)
    }

    fn role_id(&self, role: ResultRole) -> Result<&Arc<str>, ProgrammaticQueryPortError> {
        self.roles
            .get(&role)
            .ok_or_else(|| rejected(format!("unmapped semantic result role {role:?}")))
    }

    fn validate_request_shape(
        &self,
        parsed: &ParsedSemanticRequest,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let canonical = canonicalize_slice(&parsed.canonical_bytes)
            .map_err(|error| rejected(format!("request canonical bytes are invalid: {error}")))?;
        if canonical != parsed.canonical_bytes {
            return Err(rejected("request bytes are not canonical"));
        }
        let decoded = parse_request(&parsed.canonical_bytes)
            .map_err(|error| rejected(format!("canonical request cannot be decoded: {error}")))?;
        if decoded.request != parsed.request
            || decoded.canonical_bytes != parsed.canonical_bytes
            || decoded.request_digest != parsed.request_digest
        {
            return Err(rejected(
                "canonical request bytes and parsed request value disagree",
            ));
        }
        if crate::integrity::framed_digest(&parsed.canonical_bytes) != parsed.request_digest {
            return Err(rejected("canonical request digest is inconsistent"));
        }
        let request = &parsed.request;
        if request.specification.as_str() != self.released_specification.as_ref()
            || request.version.as_str() != self.released_version.as_ref()
        {
            return Err(rejected(
                "released semantic request identity is unsupported",
            ));
        }
        if !valid_wire_id(&request.semantic_request_id, 128)
            || !valid_wire_id(&request.workspace_id, 128)
            || request.queries.is_empty()
            || request.queries.len() > self.limits.compiler().max_blocks()
        {
            return Err(rejected(
                "semantic request identity or query count is outside the admitted bounds",
            ));
        }
        if contains_evaluative_intent(&parsed.canonical_bytes) {
            return Err(rejected(
                "evaluative intent is outside the objective fact substrate",
            ));
        }

        let mut forms = BTreeMap::new();
        for clause in &request.queries {
            if !valid_wire_id(clause.query_id(), 128)
                || forms
                    .insert(clause.query_id(), clause.output_role())
                    .is_some()
            {
                return Err(rejected(
                    "query IDs must be unique bounded released identifiers",
                ));
            }
            if !self.forms.contains_key(&released_form(clause)) {
                return Err(rejected(format!(
                    "released form {} has no projection mapping",
                    released_form(clause).label()
                )));
            }
            for identity in clause
                .direct_entity_ids()
                .into_iter()
                .chain(clause.direct_fact_ids())
            {
                if !valid_wire_id(identity, 192) {
                    return Err(rejected("query contains an invalid public fact identity"));
                }
            }
            validate_clause_values(clause)?;
            let maximum_results = clause.maximum_results();
            if maximum_results == 0
                || maximum_results > self.limits.compiler().max_explicit_result_rows()
            {
                return Err(rejected(
                    "query result limit is outside the admitted compiler bounds",
                ));
            }
        }
        let mut edge_count = 0_usize;
        let mut edges = BTreeSet::new();
        let mut fanin = BTreeMap::<&str, usize>::new();
        let mut fanout = BTreeMap::<&str, usize>::new();
        for clause in &request.queries {
            let mut seen_producers = BTreeSet::new();
            for reference in clause.result_references() {
                if reference.results_of == clause.query_id()
                    || !seen_producers.insert(reference.results_of.as_str())
                {
                    return Err(rejected(format!(
                        "query {} has a self or duplicate dependency",
                        clause.query_id()
                    )));
                }
                let Some(producer_role) = forms.get(reference.results_of.as_str()) else {
                    // A released combine request may consume already-admitted result relations
                    // from an earlier request. Those external producer identities do not create
                    // edges in this envelope's dependency DAG; their selected role and physical
                    // relation compatibility are validated when the admitted inputs are bound.
                    if matches!(clause, SemanticQueryClause::CombineResults { .. })
                        && valid_wire_id(&reference.results_of, 128)
                    {
                        continue;
                    }
                    return Err(rejected(format!(
                        "query {} references unknown producer {}",
                        clause.query_id(),
                        reference.results_of
                    )));
                };
                if *producer_role != reference.select {
                    return Err(rejected(format!(
                        "query {} selects the wrong role from producer {}",
                        clause.query_id(),
                        reference.results_of
                    )));
                }
                edge_count = edge_count
                    .checked_add(1)
                    .ok_or_else(|| rejected("semantic dependency count overflow"))?;
                edges.insert((reference.results_of.as_str(), clause.query_id()));
                *fanin.entry(clause.query_id()).or_default() += 1;
                *fanout.entry(reference.results_of.as_str()).or_default() += 1;
            }
        }
        let compiler_limits = self.limits.compiler();
        if edge_count > compiler_limits.max_dependencies()
            || fanin
                .values()
                .any(|count| *count > compiler_limits.max_fanin())
            || fanout
                .values()
                .any(|count| *count > compiler_limits.max_fanout())
        {
            return Err(rejected(
                "semantic dependency graph exceeds the admitted compiler bounds",
            ));
        }
        dependency_order_from_ids(forms.keys().copied(), edges.iter().copied())?;
        Ok(())
    }

    fn project_globals(
        &self,
        request: &SemanticQueryRequest,
        projection: &mut IngressProjection,
    ) -> Result<(), ProgrammaticQueryPortError> {
        for definition in COMPILED_V2_0_SCOPE_DEFINITIONS {
            let scope_id = self.global_scope(definition.role)?;
            for operand in request.compiled_v2_0_scope_operands(definition.role) {
                projection.push_scope(
                    scope_id,
                    // Source-boundary operands arrive here as opaque RFC/JCS canonical JSON
                    // UTF-8. This projection deliberately performs no JSON interpretation.
                    text(operand)?,
                )?;
            }
        }
        Ok(())
    }

    fn global_scope(
        &self,
        role: CompiledV20ScopeRole,
    ) -> Result<&Arc<str>, ProgrammaticQueryPortError> {
        self.globals
            .get(&role)
            .ok_or_else(|| rejected(format!("unmapped compiled scope role {role:?}")))
    }

    #[allow(clippy::too_many_lines)]
    fn project_clause(
        &self,
        clause: &SemanticQueryClause,
        fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
        binding: &EpochBoundProgramBindingRow,
        catalog: &EpochBoundSemanticIngressCatalog,
        projection: &mut IngressProjection,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let query_id = clause.query_id();
        let mut consumed = BTreeSet::new();
        project_selection(
            fields,
            &mut consumed,
            ProgrammaticFormIngressField::Label,
            query_id,
            clause
                .label()
                .map(|value| text(value))
                .transpose()?
                .into_iter()
                .collect(),
            projection,
        )?;
        match clause {
            SemanticQueryClause::FindEntities {
                looking_for,
                within,
                where_conditions,
                ..
            } => {
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::LookingFor,
                    query_id,
                    vec![text(looking_for)?],
                    projection,
                )?;
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Within,
                    query_id,
                    within.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::RetrieveFacts {
                about,
                facts,
                at,
                where_conditions,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::About,
                    query_id,
                    about.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Facts,
                    query_id,
                    facts,
                    projection,
                )?;
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::At,
                    query_id,
                    at.as_ref()
                        .map(|value| text(value))
                        .transpose()?
                        .into_iter()
                        .collect(),
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::FollowRelationships {
                starting_from,
                relationship,
                direction,
                distance,
                stop_when,
                where_conditions,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::StartingFrom,
                    query_id,
                    starting_from.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Relationship,
                    query_id,
                    vec![text(relationship)?],
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Direction,
                    query_id,
                    direction.as_ref(),
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Distance,
                    query_id,
                    distance.as_ref(),
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::StopWhen,
                    query_id,
                    stop_when,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::FindPaths {
                starting_from,
                ending_at,
                through,
                path_policy,
                direction,
                maximum_length,
                where_conditions,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::StartingFrom,
                    query_id,
                    starting_from.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::EndingAt,
                    query_id,
                    ending_at.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Through,
                    query_id,
                    through,
                    projection,
                )?;
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::PathPolicy,
                    query_id,
                    vec![text(path_policy)?],
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Direction,
                    query_id,
                    direction.as_ref(),
                    projection,
                )?;
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::MaximumLength,
                    query_id,
                    maximum_length
                        .map(|value| {
                            to_u64(value, "path maximum length").map(SemanticClauseValue::UInt64)
                        })
                        .transpose()?
                        .into_iter()
                        .collect(),
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::MatchPattern {
                bindings,
                relationships,
                where_conditions,
                ..
            } => {
                self.project_pattern_bindings(
                    fields,
                    &mut consumed,
                    query_id,
                    bindings,
                    binding,
                    catalog,
                    projection,
                )?;
                project_pattern_relationships(
                    fields,
                    &mut consumed,
                    query_id,
                    relationships,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::CombineResults {
                inputs,
                combination,
                identity,
                preserve_origin,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Inputs,
                    query_id,
                    inputs.iter().map(ReferenceValue::Prior),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Combination,
                    query_id,
                    vec![text(combination)?],
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Identity,
                    query_id,
                    identity.as_ref(),
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::PreserveOrigin,
                    query_id,
                    preserve_origin.as_ref(),
                    projection,
                )?;
            }
            SemanticQueryClause::SummarizeFacts {
                input,
                summaries,
                group_by,
                include_support,
                where_conditions,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Input,
                    query_id,
                    input.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Summaries,
                    query_id,
                    summaries,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::GroupBy,
                    query_id,
                    group_by,
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::IncludeSupport,
                    query_id,
                    include_support.as_ref(),
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
            SemanticQueryClause::RetrieveSourceContext {
                for_inputs,
                context,
                text_handling,
                where_conditions,
                ..
            } => {
                self.project_references(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::ForInputs,
                    query_id,
                    for_inputs.iter().map(ReferenceValue::Semantic),
                    binding,
                    catalog,
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Context,
                    query_id,
                    context,
                    projection,
                )?;
                project_optional_text_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::TextHandling,
                    query_id,
                    text_handling.as_ref(),
                    projection,
                )?;
                project_selection_texts(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Where,
                    query_id,
                    where_conditions,
                    projection,
                )?;
            }
        }
        project_return_spec(
            fields,
            &mut consumed,
            query_id,
            return_spec(clause),
            clause.maximum_results(),
            projection,
        )?;
        if consumed != fields.keys().copied().collect::<BTreeSet<_>>() {
            return Err(rejected(format!(
                "wire fields for query {query_id} were not consumed exactly once"
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn project_references<'a>(
        &self,
        fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
        consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
        field: ProgrammaticFormIngressField,
        query_id: &str,
        references: impl IntoIterator<Item = ReferenceValue<'a>>,
        binding: &EpochBoundProgramBindingRow,
        catalog: &EpochBoundSemanticIngressCatalog,
        projection: &mut IngressProjection,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let ProgrammaticFormIngressTarget::References(mapping) =
            consume_target(fields, consumed, field)?
        else {
            return Err(incompatible_target(field));
        };
        for reference in references {
            self.project_reference(
                query_id, reference, mapping, None, binding, catalog, projection,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn project_reference(
        &self,
        query_id: &str,
        reference: ReferenceValue<'_>,
        mapping: &ProgrammaticReferenceInputMapping,
        parent: Option<(&FieldId, &str)>,
        binding: &EpochBoundProgramBindingRow,
        catalog: &EpochBoundSemanticIngressCatalog,
        projection: &mut IngressProjection,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let mut fields = Vec::with_capacity(parent.map_or(3, |_| 4));
        if let Some((field_id, value)) = parent {
            fields.push(field_value(field_id, text(value)?));
        }
        let (kind, value, prior) = match reference {
            ReferenceValue::Semantic(SemanticReference::Phrase(value)) => {
                ("phrase", value.as_str(), None)
            }
            ReferenceValue::Semantic(SemanticReference::Entity { entity_id }) => {
                ("entity", entity_id.as_str(), None)
            }
            ReferenceValue::Semantic(SemanticReference::Fact { fact_id }) => {
                ("fact", fact_id.as_str(), None)
            }
            ReferenceValue::Semantic(SemanticReference::PriorResult(prior))
            | ReferenceValue::Prior(prior) => {
                ("prior_result", prior.results_of.as_str(), Some(prior))
            }
        };
        fields.push(field_value(&mapping.kind_field_id, text(kind)?));
        fields.push(field_value(&mapping.value_field_id, text(value)?));
        if let Some(prior) = prior {
            let producer_role_id = self.role_id(prior.select)?;
            fields.push(field_value(
                &mapping.producer_role_field_id,
                SemanticClauseValue::Text(Arc::clone(producer_role_id)),
            ));
            let slot = select_consumer_slot(catalog, binding, &mapping.consumer_slot_id)?;
            projection.push_dependency(
                prior.results_of.as_str(),
                producer_role_id,
                query_id,
                &mapping.consumer_slot_id,
                &slot.consumer_role_id,
            )?;
        }
        projection.push_input(query_id, &mapping.input_id, fields)
    }

    #[allow(clippy::too_many_arguments)]
    fn project_pattern_bindings(
        &self,
        fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
        consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
        query_id: &str,
        bindings: &[PatternBinding],
        binding: &EpochBoundProgramBindingRow,
        catalog: &EpochBoundSemanticIngressCatalog,
        projection: &mut IngressProjection,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let ProgrammaticFormIngressTarget::PatternBindings(mapping) = consume_target(
            fields,
            consumed,
            ProgrammaticFormIngressField::PatternBindings,
        )?
        else {
            return Err(incompatible_target(
                ProgrammaticFormIngressField::PatternBindings,
            ));
        };
        for pattern in bindings {
            projection.push_input(
                query_id,
                &mapping.binding_input_id,
                vec![
                    field_value(&mapping.binding_name_field_id, text(&pattern.name)?),
                    field_value(&mapping.looking_for_field_id, text(&pattern.looking_for)?),
                ],
            )?;
            if let Some(within) = &pattern.within {
                self.project_reference(
                    query_id,
                    ReferenceValue::Semantic(within),
                    &mapping.within,
                    Some((&mapping.within_binding_name_field_id, &pattern.name)),
                    binding,
                    catalog,
                    projection,
                )?;
            }
            for condition in &pattern.where_conditions {
                projection.push_input(
                    query_id,
                    &mapping.where_input_id,
                    vec![
                        field_value(&mapping.where_binding_name_field_id, text(&pattern.name)?),
                        field_value(&mapping.where_value_field_id, text(condition)?),
                    ],
                )?;
            }
        }
        Ok(())
    }
}

fn compiled_v2_0_arc(value: impl Into<String>) -> Arc<str> {
    Arc::from(value.into())
}

fn compiled_v2_0_field(value: impl Into<String>) -> FieldId {
    // These identities are a closed, compile-time release mapping. Keeping construction fallible
    // at the public boundary would only turn an application-owned invariant into a runtime
    // fallback opportunity.
    FieldId::new(value).expect("compiled 2.0 field identity is valid")
}

fn compiled_v2_0_roles() -> Vec<ProgrammaticResultRoleMapping> {
    all_result_roles()
        .into_iter()
        .map(|role| ProgrammaticResultRoleMapping {
            role,
            role_id: compiled_v2_0_arc(format!(
                "role.{}",
                match role {
                    ResultRole::Entities => "entities",
                    ResultRole::Facts => "facts",
                    ResultRole::Paths => "paths",
                    ResultRole::PatternBindings => "pattern-bindings",
                    ResultRole::Groups => "groups",
                    ResultRole::Summary => "summary",
                    ResultRole::SourceContexts => "source-contexts",
                }
            )),
        })
        .collect()
}

fn compiled_v2_0_globals() -> Vec<ProgrammaticGlobalIngressMapping> {
    COMPILED_V2_0_SCOPE_DEFINITIONS
        .into_iter()
        .map(|definition| ProgrammaticGlobalIngressMapping {
            role: definition.role,
            scope_id: Arc::from(definition.scope_id),
        })
        .collect()
}

fn compiled_v2_0_selection(
    field: ProgrammaticFormIngressField,
    slug: &str,
) -> ProgrammaticFormIngressMappingRow {
    ProgrammaticFormIngressMappingRow {
        field,
        target: ProgrammaticFormIngressTarget::Selection {
            selection_id: compiled_v2_0_arc(format!("selection.{slug}")),
        },
    }
}

fn compiled_v2_0_return(
    field: ProgrammaticFormIngressField,
    slug: &str,
) -> ProgrammaticFormIngressMappingRow {
    ProgrammaticFormIngressMappingRow {
        field,
        target: ProgrammaticFormIngressTarget::Return {
            return_id: compiled_v2_0_arc(format!("return.{slug}")),
        },
    }
}

fn compiled_v2_0_reference(
    field: ProgrammaticFormIngressField,
    slug: &str,
) -> ProgrammaticFormIngressMappingRow {
    ProgrammaticFormIngressMappingRow {
        field,
        target: ProgrammaticFormIngressTarget::References(ProgrammaticReferenceInputMapping {
            input_id: compiled_v2_0_arc(format!("input.{slug}")),
            kind_field_id: compiled_v2_0_field(format!("{slug}.kind")),
            value_field_id: compiled_v2_0_field(format!("{slug}.value")),
            producer_role_field_id: compiled_v2_0_field(format!("{slug}.producer-role")),
            consumer_slot_id: compiled_v2_0_arc(format!("slot.{slug}")),
        }),
    }
}

fn compiled_v2_0_form(
    form: ReleasedSemanticForm,
    specific: Vec<ProgrammaticFormIngressMappingRow>,
) -> ProgrammaticFormIngressMapping {
    use ProgrammaticFormIngressField as Field;

    let mut fields = vec![
        compiled_v2_0_selection(Field::Label, "label"),
        compiled_v2_0_return(Field::ReturnInclude, "include"),
        compiled_v2_0_return(Field::ReturnExclude, "exclude"),
        compiled_v2_0_return(Field::ReturnResultShape, "result-shape"),
        compiled_v2_0_return(Field::ReturnGroupBy, "group-by"),
        compiled_v2_0_return(Field::ReturnOrderBy, "order-by"),
        compiled_v2_0_return(Field::ReturnDeduplicateBy, "deduplicate-by"),
        compiled_v2_0_return(Field::ReturnSupportingFacts, "supporting-facts"),
        compiled_v2_0_return(Field::ReturnIncludeQueryResult, "include-query-result"),
        ProgrammaticFormIngressMappingRow {
            field: Field::ReturnMaximumResults,
            target: ProgrammaticFormIngressTarget::ExplicitResultLimit {
                return_id: compiled_v2_0_arc("return.maximum-results"),
            },
        },
        compiled_v2_0_return(Field::ReturnPer, "per"),
        compiled_v2_0_return(Field::ReturnWhenExceeded, "when-exceeded"),
    ];
    fields.extend(specific);
    ProgrammaticFormIngressMapping { form, fields }
}

#[allow(clippy::too_many_lines)]
fn compiled_v2_0_forms() -> Vec<ProgrammaticFormIngressMapping> {
    use ProgrammaticFormIngressField as Field;

    vec![
        compiled_v2_0_form(
            ReleasedSemanticForm::FindCodeEntities,
            vec![
                compiled_v2_0_selection(Field::LookingFor, "looking-for"),
                compiled_v2_0_reference(Field::Within, "within"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::RetrieveFactsAboutCode,
            vec![
                compiled_v2_0_reference(Field::About, "about"),
                compiled_v2_0_selection(Field::Facts, "facts"),
                compiled_v2_0_selection(Field::At, "at"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::FollowCodeRelationships,
            vec![
                compiled_v2_0_reference(Field::StartingFrom, "starting-from"),
                compiled_v2_0_selection(Field::Relationship, "relationship"),
                compiled_v2_0_selection(Field::Direction, "direction"),
                compiled_v2_0_selection(Field::Distance, "distance"),
                compiled_v2_0_selection(Field::StopWhen, "stop-when"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::FindConnectingFactPaths,
            vec![
                compiled_v2_0_reference(Field::StartingFrom, "starting-from"),
                compiled_v2_0_reference(Field::EndingAt, "ending-at"),
                compiled_v2_0_selection(Field::Through, "through"),
                compiled_v2_0_selection(Field::PathPolicy, "path-policy"),
                compiled_v2_0_selection(Field::Direction, "direction"),
                compiled_v2_0_selection(Field::MaximumLength, "maximum-length"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::MatchCodeFactPattern,
            vec![
                ProgrammaticFormIngressMappingRow {
                    field: Field::PatternBindings,
                    target: ProgrammaticFormIngressTarget::PatternBindings(
                        ProgrammaticPatternBindingInputMapping {
                            binding_input_id: compiled_v2_0_arc("input.pattern-bindings"),
                            binding_name_field_id: compiled_v2_0_field("pattern-binding.name"),
                            looking_for_field_id: compiled_v2_0_field(
                                "pattern-binding.looking-for",
                            ),
                            within: ProgrammaticReferenceInputMapping {
                                input_id: compiled_v2_0_arc("input.pattern-binding-within"),
                                kind_field_id: compiled_v2_0_field("pattern-within.kind"),
                                value_field_id: compiled_v2_0_field("pattern-within.value"),
                                producer_role_field_id: compiled_v2_0_field(
                                    "pattern-within.producer-role",
                                ),
                                consumer_slot_id: compiled_v2_0_arc("slot.pattern-binding-within"),
                            },
                            within_binding_name_field_id: compiled_v2_0_field(
                                "pattern-within.binding-name",
                            ),
                            where_input_id: compiled_v2_0_arc("input.pattern-binding-where"),
                            where_binding_name_field_id: compiled_v2_0_field(
                                "pattern-where.binding-name",
                            ),
                            where_value_field_id: compiled_v2_0_field("pattern-where.value"),
                        },
                    ),
                },
                ProgrammaticFormIngressMappingRow {
                    field: Field::PatternRelationships,
                    target: ProgrammaticFormIngressTarget::PatternRelationships(
                        ProgrammaticPatternRelationshipInputMapping {
                            input_id: compiled_v2_0_arc("input.pattern-relationships"),
                            from_field_id: compiled_v2_0_field("pattern-relationship.from"),
                            to_field_id: compiled_v2_0_field("pattern-relationship.to"),
                            relationship_field_id: compiled_v2_0_field(
                                "pattern-relationship.relationship",
                            ),
                            direction_field_id: compiled_v2_0_field(
                                "pattern-relationship.direction",
                            ),
                            distance_field_id: compiled_v2_0_field("pattern-relationship.distance"),
                        },
                    ),
                },
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::CombineResultSets,
            vec![
                compiled_v2_0_reference(Field::Inputs, "inputs"),
                compiled_v2_0_selection(Field::Combination, "combination"),
                compiled_v2_0_selection(Field::Identity, "identity"),
                compiled_v2_0_selection(Field::PreserveOrigin, "preserve-origin"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::SummarizeObjectiveFacts,
            vec![
                compiled_v2_0_reference(Field::Input, "input"),
                compiled_v2_0_selection(Field::Summaries, "summaries"),
                compiled_v2_0_selection(Field::GroupBy, "group-by"),
                compiled_v2_0_selection(Field::IncludeSupport, "include-support"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
            vec![
                compiled_v2_0_reference(Field::ForInputs, "for-inputs"),
                compiled_v2_0_selection(Field::Context, "context"),
                compiled_v2_0_selection(Field::TextHandling, "text-handling"),
                compiled_v2_0_selection(Field::Where, "where"),
            ],
        ),
    ]
}

impl ProgrammaticSemanticIngressPort for ApplicationOwnedSemanticIngressPort {
    fn authority_pin(&self) -> [u8; 32] {
        self.authority_pin
    }

    fn validate_request(
        &self,
        request: &ParsedSemanticRequest,
    ) -> Result<(), ProgrammaticQueryPortError> {
        self.validate_request_shape(request)
    }

    fn project(
        &self,
        request: &ParsedSemanticRequest,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
    ) -> Result<EpochBoundSemanticIngress, ProgrammaticQueryPortError> {
        if workspace.workspace_id() != authority.workspace_id() {
            return Err(rejected(
                "workspace runtime and epoch query authority identities differ",
            ));
        }
        let public_workspace_id = workspace.public_workspace_id().map_err(|error| {
            rejected(format!(
                "workspace public identity cannot be encoded: {error}"
            ))
        })?;
        if request.request.workspace_id != public_workspace_id {
            return Err(rejected(
                "request workspace identity does not match the admitted workspace",
            ));
        }
        self.project_against_catalog(request, authority.ingress_catalog())
    }
}

#[derive(Default)]
struct IngressProjection {
    selections: Vec<EpochBoundSelectionRow>,
    returns: Vec<EpochBoundReturnRow>,
    scopes: Vec<EpochBoundScopeRow>,
    request_inputs: Vec<EpochBoundRequestInputRow>,
    dependencies: Vec<EpochBoundDependencyRow>,
    selection_ordinals: BTreeMap<(Arc<str>, Arc<str>), u32>,
    return_ordinals: BTreeMap<(Arc<str>, Arc<str>), u32>,
    scope_ordinals: BTreeMap<Arc<str>, u32>,
    input_ordinals: BTreeMap<(Arc<str>, Arc<str>), u32>,
    dependency_ordinals: BTreeMap<(Arc<str>, Arc<str>), u32>,
}

impl IngressProjection {
    fn push_selection(
        &mut self,
        query_id: &str,
        selection_id: &Arc<str>,
        value: SemanticClauseValue,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let query_id: Arc<str> = Arc::from(query_id);
        let key = (Arc::clone(&query_id), Arc::clone(selection_id));
        let ordinal = next_ordinal(&mut self.selection_ordinals, key)?;
        self.selections.push(EpochBoundSelectionRow {
            query_id,
            selection_id: Arc::clone(selection_id),
            ordinal,
            value,
        });
        Ok(())
    }

    fn push_return(
        &mut self,
        query_id: &str,
        return_id: &Arc<str>,
        value: SemanticClauseValue,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let query_id: Arc<str> = Arc::from(query_id);
        let key = (Arc::clone(&query_id), Arc::clone(return_id));
        let ordinal = next_ordinal(&mut self.return_ordinals, key)?;
        self.returns.push(EpochBoundReturnRow {
            query_id,
            return_id: Arc::clone(return_id),
            ordinal,
            value,
        });
        Ok(())
    }

    fn push_scope(
        &mut self,
        scope_id: &Arc<str>,
        value: SemanticClauseValue,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let ordinal = next_ordinal(&mut self.scope_ordinals, Arc::clone(scope_id))?;
        self.scopes.push(EpochBoundScopeRow {
            scope_id: Arc::clone(scope_id),
            ordinal,
            value,
        });
        Ok(())
    }

    fn push_input(
        &mut self,
        query_id: &str,
        input_id: &Arc<str>,
        fields: Vec<EpochBoundRequestInputFieldValue>,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let query_id: Arc<str> = Arc::from(query_id);
        let key = (Arc::clone(&query_id), Arc::clone(input_id));
        let ordinal = next_ordinal(&mut self.input_ordinals, key)?;
        let mut field_ids = BTreeSet::new();
        if fields.is_empty()
            || fields
                .iter()
                .any(|field| !field_ids.insert(field.field_id.clone()))
        {
            return Err(rejected(
                "request-owned tuple is empty or contains duplicate fields",
            ));
        }
        self.request_inputs.push(EpochBoundRequestInputRow {
            query_id,
            input_id: Arc::clone(input_id),
            row_id: Arc::from(format!("row.{ordinal}")),
            ordinal,
            fields,
        });
        Ok(())
    }

    fn push_dependency(
        &mut self,
        producer_query_id: &str,
        producer_role_id: &Arc<str>,
        consumer_query_id: &str,
        consumer_slot_id: &Arc<str>,
        consumer_role_id: &Arc<str>,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let consumer_query_id: Arc<str> = Arc::from(consumer_query_id);
        let key = (Arc::clone(&consumer_query_id), Arc::clone(consumer_slot_id));
        let ordinal = next_ordinal(&mut self.dependency_ordinals, key)?;
        self.dependencies.push(EpochBoundDependencyRow {
            producer_query_id: Arc::from(producer_query_id),
            producer_role_id: Arc::clone(producer_role_id),
            consumer_query_id,
            consumer_slot_id: Arc::clone(consumer_slot_id),
            consumer_role_id: Arc::clone(consumer_role_id),
            ordinal,
        });
        Ok(())
    }
}

fn next_ordinal<K: Ord>(
    ordinals: &mut BTreeMap<K, u32>,
    key: K,
) -> Result<u32, ProgrammaticQueryPortError> {
    let next = ordinals.entry(key).or_default();
    let ordinal = *next;
    *next = next
        .checked_add(1)
        .ok_or_else(|| rejected("semantic relation ordinal overflow"))?;
    Ok(ordinal)
}

fn all_result_roles() -> [ResultRole; 7] {
    [
        ResultRole::Entities,
        ResultRole::Facts,
        ResultRole::Paths,
        ResultRole::PatternBindings,
        ResultRole::Groups,
        ResultRole::Summary,
        ResultRole::SourceContexts,
    ]
}

fn expected_fields(form: ReleasedSemanticForm) -> BTreeSet<ProgrammaticFormIngressField> {
    use ProgrammaticFormIngressField as Field;
    let specific: &[Field] = match form {
        ReleasedSemanticForm::FindCodeEntities => &[Field::LookingFor, Field::Within, Field::Where],
        ReleasedSemanticForm::RetrieveFactsAboutCode => {
            &[Field::About, Field::Facts, Field::At, Field::Where]
        }
        ReleasedSemanticForm::FollowCodeRelationships => &[
            Field::StartingFrom,
            Field::Relationship,
            Field::Direction,
            Field::Distance,
            Field::StopWhen,
            Field::Where,
        ],
        ReleasedSemanticForm::FindConnectingFactPaths => &[
            Field::StartingFrom,
            Field::EndingAt,
            Field::Through,
            Field::PathPolicy,
            Field::Direction,
            Field::MaximumLength,
            Field::Where,
        ],
        ReleasedSemanticForm::MatchCodeFactPattern => &[
            Field::PatternBindings,
            Field::PatternRelationships,
            Field::Where,
        ],
        ReleasedSemanticForm::CombineResultSets => &[
            Field::Inputs,
            Field::Combination,
            Field::Identity,
            Field::PreserveOrigin,
        ],
        ReleasedSemanticForm::SummarizeObjectiveFacts => &[
            Field::Input,
            Field::Summaries,
            Field::GroupBy,
            Field::IncludeSupport,
            Field::Where,
        ],
        ReleasedSemanticForm::RetrieveSourceAndSyntaxContext => &[
            Field::ForInputs,
            Field::Context,
            Field::TextHandling,
            Field::Where,
        ],
    };
    COMMON_FIELDS
        .into_iter()
        .chain(specific.iter().copied())
        .collect()
}

fn validate_target(
    form: ReleasedSemanticForm,
    field: ProgrammaticFormIngressField,
    target: &ProgrammaticFormIngressTarget,
    target_ids: &mut BTreeSet<(&'static str, String)>,
) -> Result<(), ProgrammaticQueryPortError> {
    use ProgrammaticFormIngressField as Field;
    let expected_kind = match field {
        Field::Within
        | Field::About
        | Field::StartingFrom
        | Field::EndingAt
        | Field::Inputs
        | Field::Input
        | Field::ForInputs => "references",
        Field::PatternBindings => "pattern bindings",
        Field::PatternRelationships => "pattern relationships",
        Field::ReturnMaximumResults => "explicit result limit",
        Field::ReturnInclude
        | Field::ReturnExclude
        | Field::ReturnResultShape
        | Field::ReturnGroupBy
        | Field::ReturnOrderBy
        | Field::ReturnDeduplicateBy
        | Field::ReturnSupportingFacts
        | Field::ReturnIncludeQueryResult
        | Field::ReturnPer
        | Field::ReturnWhenExceeded => "return",
        _ => "selection",
    };
    let observed_kind = match target {
        ProgrammaticFormIngressTarget::Selection { .. } => "selection",
        ProgrammaticFormIngressTarget::Return { .. } => "return",
        ProgrammaticFormIngressTarget::References(_) => "references",
        ProgrammaticFormIngressTarget::PatternBindings(_) => "pattern bindings",
        ProgrammaticFormIngressTarget::PatternRelationships(_) => "pattern relationships",
        ProgrammaticFormIngressTarget::ExplicitResultLimit { .. } => "explicit result limit",
    };
    if expected_kind != observed_kind {
        return Err(rejected(format!(
            "{}::{field:?} requires {expected_kind}, not {observed_kind}",
            form.label()
        )));
    }
    match target {
        ProgrammaticFormIngressTarget::Selection { selection_id } => {
            insert_target_id(target_ids, "selection", selection_id)?;
        }
        ProgrammaticFormIngressTarget::Return { return_id }
        | ProgrammaticFormIngressTarget::ExplicitResultLimit { return_id } => {
            insert_target_id(target_ids, "return", return_id)?;
        }
        ProgrammaticFormIngressTarget::References(mapping) => {
            validate_reference_mapping(mapping, target_ids)?;
        }
        ProgrammaticFormIngressTarget::PatternBindings(mapping) => {
            insert_target_id(target_ids, "request input", &mapping.binding_input_id)?;
            insert_target_id(target_ids, "request input", &mapping.where_input_id)?;
            validate_distinct_fields(&[
                &mapping.binding_name_field_id,
                &mapping.looking_for_field_id,
            ])?;
            validate_distinct_fields(&[
                &mapping.where_binding_name_field_id,
                &mapping.where_value_field_id,
            ])?;
            validate_reference_mapping(&mapping.within, target_ids)?;
            validate_distinct_fields(&[
                &mapping.within_binding_name_field_id,
                &mapping.within.kind_field_id,
                &mapping.within.value_field_id,
                &mapping.within.producer_role_field_id,
            ])?;
        }
        ProgrammaticFormIngressTarget::PatternRelationships(mapping) => {
            insert_target_id(target_ids, "request input", &mapping.input_id)?;
            validate_distinct_fields(&[
                &mapping.from_field_id,
                &mapping.to_field_id,
                &mapping.relationship_field_id,
                &mapping.direction_field_id,
                &mapping.distance_field_id,
            ])?;
        }
    }
    Ok(())
}

fn validate_reference_mapping(
    mapping: &ProgrammaticReferenceInputMapping,
    target_ids: &mut BTreeSet<(&'static str, String)>,
) -> Result<(), ProgrammaticQueryPortError> {
    insert_target_id(target_ids, "request input", &mapping.input_id)?;
    insert_target_id(target_ids, "consumer slot", &mapping.consumer_slot_id)?;
    validate_distinct_fields(&[
        &mapping.kind_field_id,
        &mapping.value_field_id,
        &mapping.producer_role_field_id,
    ])
}

fn insert_target_id(
    target_ids: &mut BTreeSet<(&'static str, String)>,
    family: &'static str,
    value: &Arc<str>,
) -> Result<(), ProgrammaticQueryPortError> {
    validate_text_identity(family, value)?;
    if !target_ids.insert((family, value.to_string())) {
        return Err(rejected(format!(
            "{family} mapping {} is ambiguous within one form",
            value
        )));
    }
    Ok(())
}

fn validate_distinct_fields(fields: &[&FieldId]) -> Result<(), ProgrammaticQueryPortError> {
    let mut identities = BTreeSet::new();
    if fields
        .iter()
        .any(|field| !identities.insert(field.as_str()))
    {
        return Err(rejected(
            "request-owned tuple mapping contains duplicate field identities",
        ));
    }
    Ok(())
}

fn released_form(clause: &SemanticQueryClause) -> ReleasedSemanticForm {
    match clause {
        SemanticQueryClause::FindEntities { .. } => ReleasedSemanticForm::FindCodeEntities,
        SemanticQueryClause::RetrieveFacts { .. } => ReleasedSemanticForm::RetrieveFactsAboutCode,
        SemanticQueryClause::FollowRelationships { .. } => {
            ReleasedSemanticForm::FollowCodeRelationships
        }
        SemanticQueryClause::FindPaths { .. } => ReleasedSemanticForm::FindConnectingFactPaths,
        SemanticQueryClause::MatchPattern { .. } => ReleasedSemanticForm::MatchCodeFactPattern,
        SemanticQueryClause::CombineResults { .. } => ReleasedSemanticForm::CombineResultSets,
        SemanticQueryClause::SummarizeFacts { .. } => ReleasedSemanticForm::SummarizeObjectiveFacts,
        SemanticQueryClause::RetrieveSourceContext { .. } => {
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext
        }
    }
}

fn select_program_binding<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    form: ReleasedSemanticForm,
    output_role_id: &Arc<str>,
) -> Result<&'a EpochBoundProgramBindingRow, ProgrammaticQueryPortError> {
    let mut matches = catalog.program_bindings.iter().filter(|binding| {
        binding.compatibility_form == form && binding.output_role_id == *output_role_id
    });
    let selected = matches.next().ok_or_else(|| {
        rejected(format!(
            "admitted catalog has no program for {} and output role {}",
            form.label(),
            output_role_id
        ))
    })?;
    if matches.next().is_some() {
        return Err(rejected(format!(
            "admitted catalog has ambiguous programs for {} and output role {}",
            form.label(),
            output_role_id
        )));
    }
    Ok(selected)
}

fn select_consumer_slot<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    binding: &EpochBoundProgramBindingRow,
    slot_id: &Arc<str>,
) -> Result<&'a EpochBoundConsumerSlotBindingRow, ProgrammaticQueryPortError> {
    let mut matches = catalog.consumer_slots.iter().filter(|slot| {
        slot.program_binding_id == binding.program_binding_id && slot.consumer_slot_id == *slot_id
    });
    let selected = matches.next().ok_or_else(|| {
        rejected(format!(
            "program {} has no consumer slot {}",
            binding.program_binding_id, slot_id
        ))
    })?;
    if matches.next().is_some() {
        return Err(rejected(format!(
            "program {} has ambiguous consumer slot {}",
            binding.program_binding_id, slot_id
        )));
    }
    Ok(selected)
}

fn consume_target<'a>(
    fields: &'a BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
) -> Result<&'a ProgrammaticFormIngressTarget, ProgrammaticQueryPortError> {
    if !consumed.insert(field) {
        return Err(rejected(format!(
            "wire field {field:?} was consumed more than once"
        )));
    }
    fields
        .get(&field)
        .ok_or_else(|| rejected(format!("wire field {field:?} is unmapped")))
}

fn project_selection(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
    query_id: &str,
    values: Vec<SemanticClauseValue>,
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    let ProgrammaticFormIngressTarget::Selection { selection_id } =
        consume_target(fields, consumed, field)?
    else {
        return Err(incompatible_target(field));
    };
    for value in values {
        projection.push_selection(query_id, selection_id, value)?;
    }
    Ok(())
}

fn project_selection_texts(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
    query_id: &str,
    values: &[String],
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    project_selection(
        fields,
        consumed,
        field,
        query_id,
        values
            .iter()
            .map(|value| text(value))
            .collect::<Result<Vec<_>, _>>()?,
        projection,
    )
}

fn project_optional_text_selection(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
    query_id: &str,
    value: Option<&String>,
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    project_selection(
        fields,
        consumed,
        field,
        query_id,
        value
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )
}

fn project_return(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
    query_id: &str,
    values: Vec<SemanticClauseValue>,
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    let ProgrammaticFormIngressTarget::Return { return_id } =
        consume_target(fields, consumed, field)?
    else {
        return Err(incompatible_target(field));
    };
    for value in values {
        projection.push_return(query_id, return_id, value)?;
    }
    Ok(())
}

fn project_return_texts(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    field: ProgrammaticFormIngressField,
    query_id: &str,
    values: &[String],
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    project_return(
        fields,
        consumed,
        field,
        query_id,
        values
            .iter()
            .map(|value| text(value))
            .collect::<Result<Vec<_>, _>>()?,
        projection,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_return_spec(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    query_id: &str,
    spec: Option<&ReturnSpec>,
    effective_maximum_results: usize,
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    project_return_texts(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnInclude,
        query_id,
        spec.map_or(&[], |value| value.include.as_slice()),
        projection,
    )?;
    project_return_texts(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnExclude,
        query_id,
        spec.map_or(&[], |value| value.exclude.as_slice()),
        projection,
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnResultShape,
        query_id,
        spec.and_then(|value| value.result_shape.as_ref())
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )?;
    project_return_texts(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnGroupBy,
        query_id,
        spec.map_or(&[], |value| value.group_by.as_slice()),
        projection,
    )?;
    project_return_texts(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnOrderBy,
        query_id,
        spec.map_or(&[], |value| value.order_by.as_slice()),
        projection,
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnDeduplicateBy,
        query_id,
        spec.and_then(|value| value.deduplicate_by.as_ref())
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnSupportingFacts,
        query_id,
        spec.and_then(|value| value.supporting_facts.as_ref())
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnIncludeQueryResult,
        query_id,
        spec.and_then(|value| value.include_query_result)
            .map(SemanticClauseValue::Boolean)
            .into_iter()
            .collect(),
        projection,
    )?;
    let ProgrammaticFormIngressTarget::ExplicitResultLimit { return_id } = consume_target(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnMaximumResults,
    )?
    else {
        return Err(incompatible_target(
            ProgrammaticFormIngressField::ReturnMaximumResults,
        ));
    };
    projection.push_return(
        query_id,
        return_id,
        SemanticClauseValue::UInt64(to_u64(
            effective_maximum_results,
            "effective maximum results",
        )?),
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnPer,
        query_id,
        spec.and_then(|value| value.limit.as_ref())
            .and_then(|limit| limit.per.as_ref())
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )?;
    project_return(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnWhenExceeded,
        query_id,
        spec.and_then(|value| value.limit.as_ref())
            .and_then(|limit| limit.when_exceeded.as_ref())
            .map(|value| text(value))
            .transpose()?
            .into_iter()
            .collect(),
        projection,
    )
}

fn project_pattern_relationships(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    query_id: &str,
    relationships: &[PatternRelationship],
    projection: &mut IngressProjection,
) -> Result<(), ProgrammaticQueryPortError> {
    let ProgrammaticFormIngressTarget::PatternRelationships(mapping) = consume_target(
        fields,
        consumed,
        ProgrammaticFormIngressField::PatternRelationships,
    )?
    else {
        return Err(incompatible_target(
            ProgrammaticFormIngressField::PatternRelationships,
        ));
    };
    for relationship in relationships {
        let mut values = vec![
            field_value(&mapping.from_field_id, text(&relationship.from)?),
            field_value(&mapping.to_field_id, text(&relationship.to)?),
            field_value(
                &mapping.relationship_field_id,
                text(&relationship.relationship)?,
            ),
        ];
        if let Some(direction) = &relationship.direction {
            values.push(field_value(&mapping.direction_field_id, text(direction)?));
        }
        if let Some(distance) = &relationship.distance {
            values.push(field_value(&mapping.distance_field_id, text(distance)?));
        }
        projection.push_input(query_id, &mapping.input_id, values)?;
    }
    Ok(())
}

fn return_spec(clause: &SemanticQueryClause) -> Option<&ReturnSpec> {
    match clause {
        SemanticQueryClause::FindEntities { return_spec, .. }
        | SemanticQueryClause::RetrieveFacts { return_spec, .. }
        | SemanticQueryClause::FollowRelationships { return_spec, .. }
        | SemanticQueryClause::FindPaths { return_spec, .. }
        | SemanticQueryClause::MatchPattern { return_spec, .. }
        | SemanticQueryClause::CombineResults { return_spec, .. }
        | SemanticQueryClause::SummarizeFacts { return_spec, .. }
        | SemanticQueryClause::RetrieveSourceContext { return_spec, .. } => return_spec.as_ref(),
    }
}

enum ReferenceValue<'a> {
    Semantic(&'a SemanticReference),
    Prior(&'a PriorResultReference),
}

fn field_value(field_id: &FieldId, value: SemanticClauseValue) -> EpochBoundRequestInputFieldValue {
    EpochBoundRequestInputFieldValue {
        field_id: field_id.clone(),
        value,
    }
}

fn text(value: &str) -> Result<SemanticClauseValue, ProgrammaticQueryPortError> {
    if value.trim().is_empty() {
        return Err(rejected("semantic field contains an empty text value"));
    }
    Ok(SemanticClauseValue::Text(Arc::from(value)))
}

fn to_u64(value: usize, field: &str) -> Result<u64, ProgrammaticQueryPortError> {
    u64::try_from(value).map_err(|_| rejected(format!("{field} cannot be represented as u64")))
}

fn dependency_order(
    blocks: &[EpochBoundBlockBindingRow],
    dependencies: &[EpochBoundDependencyRow],
) -> Result<Vec<Arc<str>>, ProgrammaticQueryPortError> {
    dependency_order_from_ids(
        blocks.iter().map(|block| block.query_id.as_ref()),
        dependencies.iter().map(|edge| {
            (
                edge.producer_query_id.as_ref(),
                edge.consumer_query_id.as_ref(),
            )
        }),
    )
    .map(|order| order.into_iter().map(Arc::from).collect())
}

fn dependency_order_from_ids<'a>(
    query_ids: impl IntoIterator<Item = &'a str>,
    edges: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Vec<String>, ProgrammaticQueryPortError> {
    let mut indegree = query_ids
        .into_iter()
        .map(|query_id| (query_id.to_owned(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = indegree
        .keys()
        .map(|query_id| (query_id.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (producer, consumer) in edges {
        let Some(consumers) = outgoing.get_mut(producer) else {
            return Err(rejected(format!("unknown dependency producer {producer}")));
        };
        if !indegree.contains_key(consumer) {
            return Err(rejected(format!("unknown dependency consumer {consumer}")));
        }
        if consumers.insert(consumer.to_owned()) {
            *indegree
                .get_mut(consumer)
                .expect("consumer existence checked") += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(query_id, _)| query_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(indegree.len());
    while let Some(query_id) = ready.pop_first() {
        order.push(query_id.clone());
        for consumer in &outgoing[query_id.as_str()] {
            let degree = indegree
                .get_mut(consumer)
                .expect("outgoing consumer exists in indegree");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(consumer.clone());
            }
        }
    }
    if order.len() != indegree.len() {
        return Err(rejected(
            "semantic request dependency graph contains a cycle",
        ));
    }
    Ok(order)
}

fn validate_clause_values(clause: &SemanticQueryClause) -> Result<(), ProgrammaticQueryPortError> {
    if let Some(label) = clause.label() {
        text(label)?;
    }
    let validate_references = |references: &[SemanticReference]| {
        for reference in references {
            match reference {
                SemanticReference::Phrase(value) => {
                    text(value)?;
                }
                SemanticReference::PriorResult(reference) => {
                    if !valid_wire_id(&reference.results_of, 128) {
                        return Err(rejected("prior-result reference has an invalid query ID"));
                    }
                }
                SemanticReference::Entity { entity_id } => {
                    if !valid_wire_id(entity_id, 192) {
                        return Err(rejected("entity reference has an invalid public ID"));
                    }
                }
                SemanticReference::Fact { fact_id } => {
                    if !valid_wire_id(fact_id, 192) {
                        return Err(rejected("fact reference has an invalid public ID"));
                    }
                }
            }
        }
        Ok(())
    };
    match clause {
        SemanticQueryClause::FindEntities {
            looking_for,
            within,
            where_conditions,
            ..
        } => {
            validate_texts(std::iter::once(looking_for).chain(where_conditions))?;
            validate_references(within)?;
        }
        SemanticQueryClause::RetrieveFacts {
            about,
            facts,
            at,
            where_conditions,
            ..
        } => {
            validate_references(about)?;
            validate_texts(facts.iter().chain(at.iter()).chain(where_conditions.iter()))?;
        }
        SemanticQueryClause::FollowRelationships {
            starting_from,
            relationship,
            direction,
            distance,
            stop_when,
            where_conditions,
            ..
        } => {
            validate_references(starting_from)?;
            validate_texts(
                std::iter::once(relationship)
                    .chain(direction.iter())
                    .chain(distance.iter())
                    .chain(stop_when.iter())
                    .chain(where_conditions.iter()),
            )?;
        }
        SemanticQueryClause::FindPaths {
            starting_from,
            ending_at,
            through,
            path_policy,
            direction,
            where_conditions,
            ..
        } => {
            validate_references(starting_from)?;
            validate_references(ending_at)?;
            validate_texts(
                through
                    .iter()
                    .chain(std::iter::once(path_policy))
                    .chain(direction.iter())
                    .chain(where_conditions.iter()),
            )?;
        }
        SemanticQueryClause::MatchPattern {
            bindings,
            relationships,
            where_conditions,
            ..
        } => {
            for binding in bindings {
                validate_texts(
                    [&binding.name, &binding.looking_for]
                        .into_iter()
                        .chain(binding.where_conditions.iter()),
                )?;
                if let Some(within) = &binding.within {
                    validate_references(std::slice::from_ref(within))?;
                }
            }
            for relationship in relationships {
                validate_texts(
                    [
                        &relationship.from,
                        &relationship.to,
                        &relationship.relationship,
                    ]
                    .into_iter()
                    .chain(relationship.direction.iter())
                    .chain(relationship.distance.iter()),
                )?;
            }
            validate_texts(where_conditions)?;
        }
        SemanticQueryClause::CombineResults {
            inputs,
            combination,
            identity,
            preserve_origin,
            ..
        } => {
            if inputs
                .iter()
                .any(|reference| !valid_wire_id(&reference.results_of, 128))
            {
                return Err(rejected("combine input has an invalid producer query ID"));
            }
            validate_texts(
                std::iter::once(combination)
                    .chain(identity.iter())
                    .chain(preserve_origin.iter()),
            )?;
        }
        SemanticQueryClause::SummarizeFacts {
            input,
            summaries,
            group_by,
            include_support,
            where_conditions,
            ..
        } => {
            validate_references(input)?;
            validate_texts(
                summaries
                    .iter()
                    .chain(group_by.iter())
                    .chain(include_support.iter())
                    .chain(where_conditions.iter()),
            )?;
        }
        SemanticQueryClause::RetrieveSourceContext {
            for_inputs,
            context,
            text_handling,
            where_conditions,
            ..
        } => {
            validate_references(for_inputs)?;
            validate_texts(
                context
                    .iter()
                    .chain(text_handling.iter())
                    .chain(where_conditions.iter()),
            )?;
        }
    }
    if let Some(spec) = return_spec(clause) {
        validate_texts(
            spec.include
                .iter()
                .chain(spec.exclude.iter())
                .chain(spec.result_shape.iter())
                .chain(spec.group_by.iter())
                .chain(spec.order_by.iter())
                .chain(spec.deduplicate_by.iter())
                .chain(spec.supporting_facts.iter())
                .chain(spec.limit.iter().flat_map(|limit| limit.per.iter()))
                .chain(
                    spec.limit
                        .iter()
                        .flat_map(|limit| limit.when_exceeded.iter()),
                ),
        )?;
    }
    Ok(())
}

fn validate_texts<'a>(
    values: impl IntoIterator<Item = &'a String>,
) -> Result<(), ProgrammaticQueryPortError> {
    for value in values {
        text(value)?;
    }
    Ok(())
}

fn contains_evaluative_intent(canonical_bytes: &[u8]) -> bool {
    let normalized = String::from_utf8_lossy(canonical_bytes).to_ascii_lowercase();
    [
        "safe_to_refactor",
        "safe to refactor",
        "high_risk",
        "high risk",
        "should_change",
        "should change",
        "test_impacted",
        "test impacted",
        "runtime-covered",
        "runtime covered",
        "runtime coverage",
        "complexity verdict",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

fn valid_wire_id(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn validate_text_identity(kind: &str, value: &str) -> Result<(), ProgrammaticQueryPortError> {
    if value.trim().is_empty() || value.len() > 256 {
        return Err(rejected(format!("{kind} identity is empty or oversized")));
    }
    Ok(())
}

fn incompatible_target(field: ProgrammaticFormIngressField) -> ProgrammaticQueryPortError {
    rejected(format!(
        "wire field {field:?} has an incompatible normalized target"
    ))
}

fn rejected(message: impl Into<String>) -> ProgrammaticQueryPortError {
    ProgrammaticQueryPortError::Rejected(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_program::RelationId;
    use crate::relational_semantic_query::{
        EpochBoundRequestInputBindingRow, EpochBoundRequestInputField, EpochBoundReturnBindingRow,
        EpochBoundScopeBindingRow, EpochBoundSelectionBindingRow, SemanticRequestLimits,
        SemanticValueKind,
    };
    use crate::semantic_query_contract::parse_request;

    fn field(value: impl Into<String>) -> FieldId {
        FieldId::new(value).expect("test field identity")
    }

    fn relation(value: impl Into<String>) -> RelationId {
        RelationId::new(value).expect("test relation identity")
    }

    fn arc(value: impl Into<String>) -> Arc<str> {
        Arc::from(value.into())
    }

    fn limits() -> EpochBoundSemanticIngressLimits {
        EpochBoundSemanticIngressLimits::try_new(
            SemanticRequestLimits::try_new(16, 64, 32, 32, 64, 32, 1_000).expect("compiler limits"),
            512,
            512,
            256,
            512,
            8,
        )
        .expect("ingress limits")
    }

    fn roles() -> Vec<ProgrammaticResultRoleMapping> {
        all_result_roles()
            .into_iter()
            .map(|role| ProgrammaticResultRoleMapping {
                role,
                role_id: arc(format!("role.{}", role_slug(role))),
            })
            .collect()
    }

    fn role_slug(role: ResultRole) -> &'static str {
        match role {
            ResultRole::Entities => "entities",
            ResultRole::Facts => "facts",
            ResultRole::Paths => "paths",
            ResultRole::PatternBindings => "pattern-bindings",
            ResultRole::Groups => "groups",
            ResultRole::Summary => "summary",
            ResultRole::SourceContexts => "source-contexts",
        }
    }

    fn globals() -> Vec<ProgrammaticGlobalIngressMapping> {
        COMPILED_V2_0_SCOPE_DEFINITIONS
            .into_iter()
            .map(|definition| ProgrammaticGlobalIngressMapping {
                role: definition.role,
                scope_id: arc(definition.scope_id),
            })
            .collect()
    }

    fn reference_target(slug: &str) -> ProgrammaticFormIngressTarget {
        ProgrammaticFormIngressTarget::References(ProgrammaticReferenceInputMapping {
            input_id: arc(format!("input.{slug}")),
            kind_field_id: field(format!("{slug}.kind")),
            value_field_id: field(format!("{slug}.value")),
            producer_role_field_id: field(format!("{slug}.producer-role")),
            consumer_slot_id: arc(format!("slot.{slug}")),
        })
    }

    fn selection(
        field: ProgrammaticFormIngressField,
        slug: &str,
    ) -> ProgrammaticFormIngressMappingRow {
        ProgrammaticFormIngressMappingRow {
            field,
            target: ProgrammaticFormIngressTarget::Selection {
                selection_id: arc(format!("selection.{slug}")),
            },
        }
    }

    fn references(
        field: ProgrammaticFormIngressField,
        slug: &str,
    ) -> ProgrammaticFormIngressMappingRow {
        ProgrammaticFormIngressMappingRow {
            field,
            target: reference_target(slug),
        }
    }

    fn common_rows() -> Vec<ProgrammaticFormIngressMappingRow> {
        use ProgrammaticFormIngressField as Field;
        vec![
            selection(Field::Label, "label"),
            return_row(Field::ReturnInclude, "include"),
            return_row(Field::ReturnExclude, "exclude"),
            return_row(Field::ReturnResultShape, "result-shape"),
            return_row(Field::ReturnGroupBy, "group-by"),
            return_row(Field::ReturnOrderBy, "order-by"),
            return_row(Field::ReturnDeduplicateBy, "deduplicate-by"),
            return_row(Field::ReturnSupportingFacts, "supporting-facts"),
            return_row(Field::ReturnIncludeQueryResult, "include-query-result"),
            ProgrammaticFormIngressMappingRow {
                field: Field::ReturnMaximumResults,
                target: ProgrammaticFormIngressTarget::ExplicitResultLimit {
                    return_id: arc("return.maximum-results"),
                },
            },
            return_row(Field::ReturnPer, "per"),
            return_row(Field::ReturnWhenExceeded, "when-exceeded"),
        ]
    }

    fn return_row(
        field: ProgrammaticFormIngressField,
        slug: &str,
    ) -> ProgrammaticFormIngressMappingRow {
        ProgrammaticFormIngressMappingRow {
            field,
            target: ProgrammaticFormIngressTarget::Return {
                return_id: arc(format!("return.{slug}")),
            },
        }
    }

    fn form_mapping(
        form: ReleasedSemanticForm,
        specific: Vec<ProgrammaticFormIngressMappingRow>,
    ) -> ProgrammaticFormIngressMapping {
        let mut fields = common_rows();
        fields.extend(specific);
        ProgrammaticFormIngressMapping { form, fields }
    }

    #[allow(clippy::too_many_lines)]
    fn forms() -> Vec<ProgrammaticFormIngressMapping> {
        use ProgrammaticFormIngressField as Field;
        vec![
            form_mapping(
                ReleasedSemanticForm::FindCodeEntities,
                vec![
                    selection(Field::LookingFor, "looking-for"),
                    references(Field::Within, "within"),
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::RetrieveFactsAboutCode,
                vec![
                    references(Field::About, "about"),
                    selection(Field::Facts, "facts"),
                    selection(Field::At, "at"),
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::FollowCodeRelationships,
                vec![
                    references(Field::StartingFrom, "starting-from"),
                    selection(Field::Relationship, "relationship"),
                    selection(Field::Direction, "direction"),
                    selection(Field::Distance, "distance"),
                    selection(Field::StopWhen, "stop-when"),
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::FindConnectingFactPaths,
                vec![
                    references(Field::StartingFrom, "starting-from"),
                    references(Field::EndingAt, "ending-at"),
                    selection(Field::Through, "through"),
                    selection(Field::PathPolicy, "path-policy"),
                    selection(Field::Direction, "direction"),
                    selection(Field::MaximumLength, "maximum-length"),
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::MatchCodeFactPattern,
                vec![
                    ProgrammaticFormIngressMappingRow {
                        field: Field::PatternBindings,
                        target: ProgrammaticFormIngressTarget::PatternBindings(
                            ProgrammaticPatternBindingInputMapping {
                                binding_input_id: arc("input.pattern-bindings"),
                                binding_name_field_id: field("pattern-binding.name"),
                                looking_for_field_id: field("pattern-binding.looking-for"),
                                within: ProgrammaticReferenceInputMapping {
                                    input_id: arc("input.pattern-binding-within"),
                                    kind_field_id: field("pattern-within.kind"),
                                    value_field_id: field("pattern-within.value"),
                                    producer_role_field_id: field("pattern-within.producer-role"),
                                    consumer_slot_id: arc("slot.pattern-binding-within"),
                                },
                                within_binding_name_field_id: field("pattern-within.binding-name"),
                                where_input_id: arc("input.pattern-binding-where"),
                                where_binding_name_field_id: field("pattern-where.binding-name"),
                                where_value_field_id: field("pattern-where.value"),
                            },
                        ),
                    },
                    ProgrammaticFormIngressMappingRow {
                        field: Field::PatternRelationships,
                        target: ProgrammaticFormIngressTarget::PatternRelationships(
                            ProgrammaticPatternRelationshipInputMapping {
                                input_id: arc("input.pattern-relationships"),
                                from_field_id: field("pattern-relationship.from"),
                                to_field_id: field("pattern-relationship.to"),
                                relationship_field_id: field("pattern-relationship.relationship"),
                                direction_field_id: field("pattern-relationship.direction"),
                                distance_field_id: field("pattern-relationship.distance"),
                            },
                        ),
                    },
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::CombineResultSets,
                vec![
                    references(Field::Inputs, "inputs"),
                    selection(Field::Combination, "combination"),
                    selection(Field::Identity, "identity"),
                    selection(Field::PreserveOrigin, "preserve-origin"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::SummarizeObjectiveFacts,
                vec![
                    references(Field::Input, "input"),
                    selection(Field::Summaries, "summaries"),
                    selection(Field::GroupBy, "group-by"),
                    selection(Field::IncludeSupport, "include-support"),
                    selection(Field::Where, "where"),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
                vec![
                    references(Field::ForInputs, "for-inputs"),
                    selection(Field::Context, "context"),
                    selection(Field::TextHandling, "text-handling"),
                    selection(Field::Where, "where"),
                ],
            ),
        ]
    }

    fn port() -> ApplicationOwnedSemanticIngressPort {
        let release = super::super::production_kernel::CompiledSemanticRelease::current();
        ApplicationOwnedSemanticIngressPort::try_compiled_v2_0(release.query_authority(), limits())
            .expect("complete compiled 2.0 mapping")
    }

    fn output_role(form: ReleasedSemanticForm) -> ResultRole {
        match form {
            ReleasedSemanticForm::FindCodeEntities => ResultRole::Entities,
            ReleasedSemanticForm::RetrieveFactsAboutCode
            | ReleasedSemanticForm::FollowCodeRelationships => ResultRole::Facts,
            ReleasedSemanticForm::FindConnectingFactPaths => ResultRole::Paths,
            ReleasedSemanticForm::MatchCodeFactPattern => ResultRole::PatternBindings,
            ReleasedSemanticForm::CombineResultSets => ResultRole::Groups,
            ReleasedSemanticForm::SummarizeObjectiveFacts => ResultRole::Summary,
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext => ResultRole::SourceContexts,
        }
    }

    fn value_kind(field: ProgrammaticFormIngressField) -> SemanticValueKind {
        match field {
            ProgrammaticFormIngressField::MaximumLength
            | ProgrammaticFormIngressField::ReturnMaximumResults => SemanticValueKind::UInt64,
            ProgrammaticFormIngressField::ReturnIncludeQueryResult => SemanticValueKind::Boolean,
            _ => SemanticValueKind::Text,
        }
    }

    fn request_input(
        program_binding_id: &Arc<str>,
        input_id: &Arc<str>,
        suffix: &str,
        fields: Vec<EpochBoundRequestInputField>,
    ) -> EpochBoundRequestInputBindingRow {
        EpochBoundRequestInputBindingRow {
            program_binding_id: Arc::clone(program_binding_id),
            input_id: Arc::clone(input_id),
            input_relation_id: relation(format!("request.{suffix}")),
            fields,
            minimum_rows: 0,
            maximum_rows: 64,
        }
    }

    fn text_field(field_id: &FieldId, required: bool) -> EpochBoundRequestInputField {
        EpochBoundRequestInputField {
            field_id: field_id.clone(),
            value_kind: SemanticValueKind::Text,
            required,
        }
    }

    fn add_reference_catalog(
        program_binding_id: &Arc<str>,
        mapping: &ProgrammaticReferenceInputMapping,
        suffix: &str,
        parent: Option<&FieldId>,
        request_inputs: &mut Vec<EpochBoundRequestInputBindingRow>,
        slots: &mut Vec<EpochBoundConsumerSlotBindingRow>,
    ) {
        let mut fields = Vec::new();
        if let Some(parent) = parent {
            fields.push(text_field(parent, true));
        }
        fields.extend([
            text_field(&mapping.kind_field_id, true),
            text_field(&mapping.value_field_id, true),
            text_field(&mapping.producer_role_field_id, false),
        ]);
        request_inputs.push(request_input(
            program_binding_id,
            &mapping.input_id,
            suffix,
            fields,
        ));
        slots.push(EpochBoundConsumerSlotBindingRow {
            program_binding_id: Arc::clone(program_binding_id),
            consumer_slot_id: Arc::clone(&mapping.consumer_slot_id),
            consumer_role_id: arc(format!("consumer.{suffix}")),
            minimum_edges: 0,
            maximum_edges: 32,
        });
    }

    #[allow(clippy::too_many_lines)]
    fn catalog(port: &ApplicationOwnedSemanticIngressPort) -> EpochBoundSemanticIngressCatalog {
        let mut program_bindings = Vec::new();
        let mut consumer_slots = Vec::new();
        let mut selections = Vec::new();
        let mut returns = Vec::new();
        let mut request_inputs = Vec::new();
        for (index, (form, fields)) in port.forms.iter().enumerate() {
            let program_binding_id = arc(format!("installed.program.{index}"));
            let role_id = Arc::clone(&port.roles[&output_role(*form)]);
            program_bindings.push(EpochBoundProgramBindingRow {
                program_binding_id: Arc::clone(&program_binding_id),
                program_binding_pin: [u8::try_from(index + 10).expect("small index"); 32],
                compatibility_form: *form,
                output_role_id: role_id,
                execution_program_pin: [u8::try_from(index + 30).expect("small index"); 32],
            });
            for (field, target) in fields {
                match target {
                    ProgrammaticFormIngressTarget::Selection { selection_id } => {
                        selections.push(EpochBoundSelectionBindingRow {
                            program_binding_id: Arc::clone(&program_binding_id),
                            selection_id: Arc::clone(selection_id),
                            value_kind: value_kind(*field),
                            minimum_values: 0,
                            maximum_values: 64,
                        });
                    }
                    ProgrammaticFormIngressTarget::Return { return_id } => {
                        returns.push(EpochBoundReturnBindingRow {
                            program_binding_id: Arc::clone(&program_binding_id),
                            return_id: Arc::clone(return_id),
                            value_kind: value_kind(*field),
                            minimum_values: 0,
                            maximum_values: 64,
                        });
                    }
                    ProgrammaticFormIngressTarget::ExplicitResultLimit { return_id } => {
                        returns.push(EpochBoundReturnBindingRow {
                            program_binding_id: Arc::clone(&program_binding_id),
                            return_id: Arc::clone(return_id),
                            value_kind: SemanticValueKind::UInt64,
                            minimum_values: 1,
                            maximum_values: 1,
                        });
                    }
                    ProgrammaticFormIngressTarget::References(mapping) => {
                        add_reference_catalog(
                            &program_binding_id,
                            mapping,
                            &format!("{index}.{}", mapping.input_id),
                            None,
                            &mut request_inputs,
                            &mut consumer_slots,
                        );
                    }
                    ProgrammaticFormIngressTarget::PatternBindings(mapping) => {
                        request_inputs.push(request_input(
                            &program_binding_id,
                            &mapping.binding_input_id,
                            &format!("{index}.pattern-bindings"),
                            vec![
                                text_field(&mapping.binding_name_field_id, true),
                                text_field(&mapping.looking_for_field_id, true),
                            ],
                        ));
                        add_reference_catalog(
                            &program_binding_id,
                            &mapping.within,
                            &format!("{index}.pattern-within"),
                            Some(&mapping.within_binding_name_field_id),
                            &mut request_inputs,
                            &mut consumer_slots,
                        );
                        request_inputs.push(request_input(
                            &program_binding_id,
                            &mapping.where_input_id,
                            &format!("{index}.pattern-where"),
                            vec![
                                text_field(&mapping.where_binding_name_field_id, true),
                                text_field(&mapping.where_value_field_id, true),
                            ],
                        ));
                    }
                    ProgrammaticFormIngressTarget::PatternRelationships(mapping) => {
                        request_inputs.push(request_input(
                            &program_binding_id,
                            &mapping.input_id,
                            &format!("{index}.pattern-relationships"),
                            vec![
                                text_field(&mapping.from_field_id, true),
                                text_field(&mapping.to_field_id, true),
                                text_field(&mapping.relationship_field_id, true),
                                text_field(&mapping.direction_field_id, false),
                                text_field(&mapping.distance_field_id, false),
                            ],
                        ));
                    }
                }
            }
        }
        let scopes = COMPILED_V2_0_SCOPE_DEFINITIONS
            .into_iter()
            .map(|definition| EpochBoundScopeBindingRow {
                scope_id: Arc::clone(&port.globals[&definition.role]),
                value_kind: SemanticValueKind::Text,
                minimum_values: definition.minimum_values,
                maximum_values: definition.maximum_values,
            })
            .collect();
        EpochBoundSemanticIngressCatalog {
            fabric_epoch_pin: [1; 32],
            program_catalog_pin: [2; 32],
            source_pin: [3; 32],
            policy_pin: [4; 32],
            producer_closure_proof_pin: [5; 32],
            limits_pin: epoch_bound_semantic_ingress_limits_pin(port.limits),
            program_bindings,
            consumer_slots,
            selections,
            returns,
            scopes,
            request_inputs,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn eight_form_request() -> ParsedSemanticRequest {
        let value = serde_json::json!({
            "specification": "composable semantic CPG fact query",
            "version": "2.0",
            "semantic_request_id": "request.all-eight",
            "scope": {
                "workspace_id": "workspace:00112233445566778899aabbccddeeff",
                "codebase": "codebase:current",
                "languages": ["Rust", "Python"],
                "source_boundaries": [{"root": "src", "kind": "path"}],
                "analysis_contexts": {
                    "mode": "explicit",
                    "context_ids": ["analysis:one", "analysis:two"]
                },
                "representations": ["syntax", "semantic"],
                "external_entities": "endpoint-only"
            },
            "freshness": {"policy": "require_current_for_targets"},
            "queries": [
                {
                    "request": "find code entities",
                    "query_id": "q1",
                    "label": "entities",
                    "looking_for": "functions",
                    "within": ["workspace", {"entity_id": "entity:one"}],
                    "where": ["language is Rust", "visibility is public"],
                    "return": {
                        "include": ["identity", "semantic_kind"],
                        "exclude": ["debug"],
                        "result_shape": "rows",
                        "group_by": ["semantic_kind"],
                        "order_by": ["identity", "semantic_kind"],
                        "deduplicate_by": "identity",
                        "supporting_facts": "include",
                        "include_query_result": true,
                        "limit": {"maximum_results": 5, "per": "workspace", "when_exceeded": "truncate"}
                    }
                },
                {
                    "request": "retrieve facts about code",
                    "query_id": "q2",
                    "about": [{"results_of": "q1", "select": "entities"}, {"fact_id": "fact:one"}],
                    "facts": ["definition", "type"],
                    "at": "present",
                    "where": ["supported"],
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "follow code relationships",
                    "query_id": "q3",
                    "starting_from": [{"results_of": "q2", "select": "facts"}],
                    "relationship": "calls",
                    "direction": "outgoing",
                    "distance": "one",
                    "stop_when": ["boundary", "unknown"],
                    "where": ["resolved"],
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "find connecting fact paths",
                    "query_id": "q4",
                    "from": [{"results_of": "q1", "select": "entities"}],
                    "to": [{"results_of": "q2", "select": "facts"}],
                    "using": ["call", "definition"],
                    "path_policy": "shortest",
                    "direction": "outgoing",
                    "maximum_length": 4,
                    "where": ["supported"],
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "match a code fact pattern",
                    "query_id": "q5",
                    "pattern": {
                        "nodes": [
                            {
                                "binding": "caller",
                                "semantic_kind": "function",
                                "module_id": "entity:module:caller",
                                "name": "public"
                            },
                            {
                                "binding": "callee",
                                "semantic_kind": "function",
                                "module_id": "entity:module:callee",
                                "name": "typed"
                            }
                        ],
                        "facts": [{
                            "from": "caller", "to": "callee", "relationship": "calls",
                            "direction": "outgoing", "distance": "one"
                        }]
                    },
                    "where": ["resolved"],
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "combine result sets",
                    "query_id": "q6",
                    "inputs": [
                        {"results_of": "q1", "select": "entities"},
                        {"results_of": "q2", "select": "facts"}
                    ],
                    "operation": "union",
                    "identity": "canonical",
                    "preserve_origin": "yes",
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "summarize objective facts",
                    "query_id": "q7",
                    "about": [{"results_of": "q2", "select": "facts"}],
                    "measure": "count",
                    "group_by": ["semantic_kind", "language"],
                    "include_support": "yes",
                    "where": ["known"],
                    "return": {"limit": {"maximum_results": 5}}
                },
                {
                    "request": "retrieve source and syntax context",
                    "query_id": "q8",
                    "about": [{"results_of": "q2", "select": "facts"}],
                    "context": "source",
                    "where": ["available"],
                    "return": {"limit": {"maximum_results": 5}}
                }
            ]
        });
        parse_request(&serde_json::to_vec(&value).expect("request JSON"))
            .expect("released request parses")
    }

    #[test]
    fn all_eight_forms_project_exact_rows_pins_repetitions_and_dependencies() {
        let port = port();
        let catalog = catalog(&port);
        let request = eight_form_request();
        port.validate_request(&request).expect("preflight");
        let ingress = port
            .project_against_catalog(&request, &catalog)
            .expect("exact ingress projection");

        assert_eq!(ingress.blocks.len(), 8);
        for (index, block) in ingress.blocks.iter().enumerate() {
            assert_eq!(
                block.program_binding_id.as_ref(),
                format!("installed.program.{index}")
            );
            assert_eq!(block.explicit_result_limit, Some(5));
        }
        assert_eq!(
            ingress.request_content_pin,
            canonical_request_content_pin(&request.canonical_bytes)
        );
        assert_eq!(ingress.fabric_epoch_pin, catalog.fabric_epoch_pin);
        assert_eq!(ingress.program_catalog_pin, catalog.program_catalog_pin);
        assert_eq!(ingress.source_pin, catalog.source_pin);
        assert_eq!(ingress.policy_pin, catalog.policy_pin);
        assert_eq!(
            ingress.producer_closure_proof_pin,
            catalog.producer_closure_proof_pin
        );
        assert_eq!(ingress.limits_pin, catalog.limits_pin);
        assert_eq!(ingress.scopes.len(), 11);
        assert!(ingress.scopes.iter().all(|row| {
            COMPILED_V2_0_SCOPE_DEFINITIONS
                .iter()
                .any(|definition| definition.scope_id == row.scope_id.as_ref())
        }));
        assert!(ingress.scopes.iter().any(|row| {
            row.scope_id.as_ref() == "scope.source-boundary"
                && row.value
                    == SemanticClauseValue::Text(Arc::from(r#"{"kind":"path","root":"src"}"#))
        }));
        assert!(!ingress.scopes.iter().any(|row| {
            matches!(
                row.scope_id.as_ref(),
                "scope.specification" | "scope.version" | "scope.freshness"
            )
        }));

        let order_by = ingress
            .returns
            .iter()
            .filter(|row| {
                row.query_id.as_ref() == "q1" && row.return_id.as_ref() == "return.order-by"
            })
            .collect::<Vec<_>>();
        assert_eq!(order_by.len(), 2);
        assert_eq!(order_by[0].ordinal, 0);
        assert_eq!(order_by[1].ordinal, 1);
        assert_eq!(
            order_by[1].value,
            SemanticClauseValue::Text(Arc::from("semantic_kind"))
        );

        let within = ingress
            .request_inputs
            .iter()
            .filter(|row| row.query_id.as_ref() == "q1" && row.input_id.as_ref() == "input.within")
            .collect::<Vec<_>>();
        assert_eq!(within.len(), 2);
        assert_eq!((within[0].ordinal, within[1].ordinal), (0, 1));
        let pattern_where = ingress
            .request_inputs
            .iter()
            .filter(|row| {
                row.query_id.as_ref() == "q5"
                    && row.input_id.as_ref() == "input.pattern-binding-where"
            })
            .collect::<Vec<_>>();
        assert_eq!(pattern_where.len(), 2);
        assert_eq!((pattern_where[0].ordinal, pattern_where[1].ordinal), (0, 1));

        let combine_dependencies = ingress
            .dependencies
            .iter()
            .filter(|edge| edge.consumer_query_id.as_ref() == "q6")
            .collect::<Vec<_>>();
        assert_eq!(combine_dependencies.len(), 2);
        assert_eq!(
            (
                combine_dependencies[0].ordinal,
                combine_dependencies[1].ordinal
            ),
            (0, 1)
        );
        assert_eq!(
            ingress
                .dependency_order
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>(),
            vec!["q1", "q2", "q3", "q4", "q5", "q6", "q7", "q8"]
        );
    }

    #[test]
    fn compiled_constructor_is_v2_only_and_has_no_caller_mapping_parameters() {
        type ProductionConstructor =
            fn(
                &super::super::production_kernel::CompiledQueryAuthority,
                EpochBoundSemanticIngressLimits,
            )
                -> Result<ApplicationOwnedSemanticIngressPort, ProgrammaticQueryPortError>;

        let constructor: ProductionConstructor =
            ApplicationOwnedSemanticIngressPort::try_compiled_v2_0;
        let release = super::super::production_kernel::CompiledSemanticRelease::current();
        let port = constructor(release.query_authority(), limits()).expect("compiled v2 mapping");
        assert_eq!(
            port.authority_pin(),
            compiled_query_release_pin(release.query_authority())
        );
        assert_ne!(port.authority_pin(), [0; 32]);
        assert_eq!(port.released_version.as_ref(), "2.0");

        let legacy = serde_json::json!({
            "specification": "composable semantic CPG fact query",
            "version": "1.3",
            "semantic_request_id": "request.legacy",
            "workspace_id": "workspace:00112233445566778899aabbccddeeff",
            "freshness_policy": "current_required",
            "queries": [{
                "request": "find code entities",
                "query_id": "q1",
                "label": null,
                "looking_for": "functions",
                "within": [],
                "where": [],
                "return": {"limit": {"maximum_results": 5}}
            }]
        });
        let error = parse_request(&serde_json::to_vec(&legacy).expect("legacy JSON"))
            .expect_err("the removed v1.3 envelope must not remain operable");
        assert!(
            error
                .to_string()
                .contains("unsupported semantic request version 1.3")
        );
    }

    #[test]
    fn incomplete_mapping_and_ambiguous_catalog_program_are_rejected() {
        let mut incomplete = forms();
        incomplete.pop();
        let error = ApplicationOwnedSemanticIngressPort::try_new(
            [0xa1; 32],
            "composable semantic CPG fact query",
            "2.0",
            limits(),
            roles(),
            globals(),
            incomplete,
        )
        .expect_err("missing form must fail");
        assert!(error.to_string().contains("not exhaustive"));

        let port = port();
        let mut catalog = catalog(&port);
        let mut duplicate = catalog.program_bindings[0].clone();
        duplicate.program_binding_id = arc("installed.program.ambiguous");
        duplicate.program_binding_pin = [0xee; 32];
        catalog.program_bindings.push(duplicate);
        let error = port
            .project_against_catalog(&eight_form_request(), &catalog)
            .expect_err("form and role must select exactly one installed program");
        assert!(error.to_string().contains("ambiguous programs"));
    }

    #[test]
    fn canonical_bytes_must_describe_the_exact_parsed_value() {
        let port = port();
        let mut request = eight_form_request();
        request.request.semantic_request_id = "request.forged".to_owned();
        let error = port
            .validate_request(&request)
            .expect_err("forged parsed envelope must fail");
        assert!(
            error
                .to_string()
                .contains("canonical request bytes and parsed request value disagree")
        );
    }
}
