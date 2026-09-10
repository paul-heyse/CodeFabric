//! Application-owned projection of the released semantic request into epoch-bound relations.
//!
//! Released form names are compatibility observations, not executor identities. This port uses
//! explicit typed mappings for every wire field, resolves one installed program by the tuple of
//! released form, semantic output role and admitted family meanings, then validates against the
//! already-admitted epoch catalog. No program binding ID or execution program pin is embedded in
//! this module.

mod unavailable;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::contracts::jcs::canonicalize_slice;
use crate::query_backend::{
    ResolvedSemanticExecutionRequest, SemanticAuthorizedChoice, SemanticInputAnswer,
    SemanticInputConstraints, SemanticInputKind, SemanticInputRequirement, SemanticInputValue,
};
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

use super::production_kernel::CompiledSemanticRelease;
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

impl ProgrammaticFormIngressField {
    /// Exact compiled v2.0 selection identity for fields projected as selection rows.
    ///
    /// `None` identifies fields owned by a different ingress relation family. The exhaustive
    /// match makes adding a new wire field require an explicit selection disposition.
    #[must_use]
    pub const fn compiled_v2_0_selection_id(self) -> Option<&'static str> {
        match self {
            Self::Label => Some("selection.label"),
            Self::LookingFor => Some("selection.looking-for"),
            Self::Where => Some("selection.where"),
            Self::Facts => Some("selection.facts"),
            Self::At => Some("selection.at"),
            Self::Relationship => Some("selection.relationship"),
            Self::Direction => Some("selection.direction"),
            Self::Distance => Some("selection.distance"),
            Self::StopWhen => Some("selection.stop-when"),
            Self::Through => Some("selection.through"),
            Self::PathPolicy => Some("selection.path-policy"),
            Self::MaximumLength => Some("selection.maximum-length"),
            Self::Combination => Some("selection.combination"),
            Self::Identity => Some("selection.identity"),
            Self::PreserveOrigin => Some("selection.preserve-origin"),
            Self::Summaries => Some("selection.summaries"),
            Self::GroupBy => Some("selection.group-by"),
            Self::IncludeSupport => Some("selection.include-support"),
            Self::Context => Some("selection.context"),
            Self::TextHandling => Some("selection.text-handling"),
            Self::Within
            | Self::About
            | Self::StartingFrom
            | Self::EndingAt
            | Self::PatternBindings
            | Self::PatternRelationships
            | Self::Inputs
            | Self::Input
            | Self::ForInputs
            | Self::ReturnInclude
            | Self::ReturnExclude
            | Self::ReturnResultShape
            | Self::ReturnGroupBy
            | Self::ReturnOrderBy
            | Self::ReturnDeduplicateBy
            | Self::ReturnSupportingFacts
            | Self::ReturnIncludeQueryResult
            | Self::ReturnMaximumResults
            | Self::ReturnPer
            | Self::ReturnWhenExceeded => None,
        }
    }
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
    /// The effective released limit is block metadata, not a semantic return row.
    ExplicitResultLimit,
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
        compiled_release: &CompiledSemanticRelease,
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
        let expected_roles = ResultRole::ALL.into_iter().collect::<BTreeSet<_>>();
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
        match self.prepare_against_catalog(request, &[], catalog)? {
            IngressPreparation::Ready(ingress) => Ok(ingress),
            IngressPreparation::InputRequired(_) => Err(rejected(
                "semantic selection value requires guarded input in the admitted epoch",
            )),
        }
    }

    fn prepare_against_catalog(
        &self,
        request: &ParsedSemanticRequest,
        answers: &[SemanticInputAnswer],
        catalog: &EpochBoundSemanticIngressCatalog,
    ) -> Result<IngressPreparation, ProgrammaticQueryPortError> {
        self.validate_request_shape(request)?;
        let expected_limits_pin = epoch_bound_semantic_ingress_limits_pin(self.limits);
        if expected_limits_pin != catalog.limits_pin {
            return Err(rejected(
                "semantic ingress limits do not match the admitted epoch catalog",
            ));
        }

        let mut projection = IngressProjection::try_for_catalog(catalog, answers)?;
        self.project_globals(&request.request, &mut projection)?;
        let mut blocks = Vec::with_capacity(request.request.queries.len());

        let mut failures = unavailable::ProjectionFailures::default();
        for clause in &request.request.queries {
            let requirement_start = projection.requirements.len();
            let result: Result<(), ProgrammaticQueryPortError> = (|| {
                let form = released_form(clause);
                let output_role_id = self.role_id(clause.output_role())?;
                let Some(binding) =
                    select_clause_program(catalog, clause, output_role_id, &mut projection)?
                else {
                    return Ok(());
                };
                let query_id: Arc<str> = Arc::from(clause.query_id());
                projection.bind_query_program(&query_id, &binding.program_binding_id)?;
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
                Ok(())
            })();
            failures.track_requirements(
                clause.query_id(),
                &projection.requirements[requirement_start..],
            );
            match result {
                Ok(()) => {}
                Err(ProgrammaticQueryPortError::SemanticUnavailable { subject_id, detail }) => {
                    tracing::debug!(query_id = clause.query_id(), %subject_id, %detail, "query ingress semantics unavailable");
                    failures.reject(clause, subject_id);
                }
                Err(error) => return Err(error),
            }
        }
        let unavailable_blocks = failures.finish(&request.request, &mut blocks, &mut projection)?;

        projection.validate_answer_consumption()?;
        if !projection.requirements.is_empty() {
            return Ok(IngressPreparation::InputRequired(projection.requirements));
        }
        let dependency_order = dependency_order(&blocks, &projection.dependencies)?;
        let ingress = EpochBoundSemanticIngress {
            semantic_request_id: Arc::from(request.request.semantic_request_id.as_str()),
            unavailable_blocks,
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
        Ok(IngressPreparation::Ready(ingress))
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
            if let Some(limit) = return_spec(clause).and_then(|spec| spec.maximum_source_bytes)
                && (!matches!(clause, SemanticQueryClause::RetrieveSourceContext { .. })
                    || limit == 0
                    || limit > 1024 * 1024)
            {
                return Err(rejected(
                    "source byte limit requires a source-context block and must be in 1..=1048576",
                ));
            }
            let window = return_spec(clause)
                .and_then(crate::semantic_query_contract::ReturnSpec::source_line_window);
            let surrounding = matches!(clause, SemanticQueryClause::RetrieveSourceContext { context, .. } if context.as_slice() == ["surrounding lines"]);
            if surrounding != window.is_some()
                || window.is_some_and(|(before, after)| before > 4096 || after > 4096)
            {
                return Err(rejected(
                    "surrounding lines requires source_lines_before or source_lines_after in 0..=4096; these fields apply only to that context",
                ));
            }
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
            for reference in clause.result_references() {
                if reference.results_of == clause.query_id() {
                    return Err(rejected(format!(
                        "query {} has a self dependency",
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
            clause.label().map(text).transpose()?.into_iter().collect(),
            projection,
        )?;
        match clause {
            SemanticQueryClause::FindEntities {
                looking_for,
                within,
                where_conditions,
                ..
            } => {
                let literal = code_literals::entity_selection(looking_for);
                if looking_for.contains('`') && literal.is_none() {
                    return Err(unavailable(
                        "looking-for",
                        "quoted entity identifiers require a supported kind phrase and one nonempty literal",
                    ));
                }
                let (meaning, predicates) = if let Some((meaning, predicate)) = &literal {
                    let mut predicates = where_conditions.clone();
                    predicates.push(predicate.clone());
                    (meaning.as_str(), predicates)
                } else {
                    (looking_for.as_str(), where_conditions.clone())
                };
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::LookingFor,
                    query_id,
                    vec![text(meaning)?],
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
                    &predicates,
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
                project_selection(
                    fields,
                    &mut consumed,
                    ProgrammaticFormIngressField::Direction,
                    query_id,
                    vec![text(direction.as_deref().unwrap_or("outgoing"))?],
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
        if let ReferenceValue::Semantic(SemanticReference::SourceLocation { source_location }) =
            &reference
        {
            source_location.validate().map_err(rejected)?;
            source_location
                .meaning()
                .map_err(|message| unavailable("source-location", message))?;
            if parent.is_none()
                && mapping.input_id.as_ref() == "input.within"
                && catalog
                    .selections
                    .iter()
                    .any(|selection| selection.selection_id.as_ref() == "selection.source-location")
            {
                // FindEntities applies this optional scope after compilation, like source boundaries.
                // Its empty within operand preserves the unconstrained entity census.
                return Ok(());
            }
            if parent.is_some()
                || !catalog.selections.iter().any(|selection| {
                    selection.program_binding_id == binding.program_binding_id
                        && selection.selection_id.as_ref() == "selection.source-location"
                })
            {
                return Err(unavailable(
                    "source-location",
                    "source-location subjects are unavailable for the selected snapshot or form",
                ));
            }
            return projection.push_selection(
                query_id,
                &Arc::from("selection.source-location"),
                text(
                    &serde_json::to_string(source_location)
                        .map_err(|error| rejected(error.to_string()))?,
                )?,
            );
        }
        if parent.is_none()
            && let ReferenceValue::Semantic(SemanticReference::Phrase(value)) = &reference
            && let Some(named) = code_literals::named_subject(value)
            && catalog.selections.iter().any(|selection| {
                selection.program_binding_id == binding.program_binding_id
                    && selection.selection_id.as_ref() == "selection.named-subject"
            })
        {
            return projection.push_selection(
                query_id,
                &Arc::from("selection.named-subject"),
                text(&serde_json::to_string(&named).map_err(|error| rejected(error.to_string()))?)?,
            );
        }
        let mut fields = Vec::with_capacity(parent.map_or(3, |_| 4));
        if let Some((field_id, value)) = parent {
            fields.push(field_value(field_id, text(value)?));
        }
        let (kind, value, prior) = match reference {
            ReferenceValue::Semantic(SemanticReference::SourceLocation { .. }) => {
                return Err(rejected("source location was not projected"));
            }
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
            if slot.materialized {
                return Ok(());
            }
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

    fn validate_workspace_binding(
        &self,
        request: &ParsedSemanticRequest,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
    ) -> Result<(), ProgrammaticQueryPortError> {
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
    ResultRole::ALL
        .into_iter()
        .map(|role| ProgrammaticResultRoleMapping {
            role,
            role_id: compiled_v2_0_arc(role.released_id()),
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
) -> ProgrammaticFormIngressMappingRow {
    let selection_id = field
        .compiled_v2_0_selection_id()
        .expect("compiled v2.0 selection field has a typed identity");
    ProgrammaticFormIngressMappingRow {
        field,
        target: ProgrammaticFormIngressTarget::Selection {
            selection_id: compiled_v2_0_arc(selection_id),
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
        compiled_v2_0_selection(Field::Label),
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
            target: ProgrammaticFormIngressTarget::ExplicitResultLimit,
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
                compiled_v2_0_selection(Field::LookingFor),
                compiled_v2_0_reference(Field::Within, "within"),
                compiled_v2_0_selection(Field::Where),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::RetrieveFactsAboutCode,
            vec![
                compiled_v2_0_reference(Field::About, "about"),
                compiled_v2_0_selection(Field::Facts),
                compiled_v2_0_selection(Field::At),
                compiled_v2_0_selection(Field::Where),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::FollowCodeRelationships,
            vec![
                compiled_v2_0_reference(Field::StartingFrom, "starting-from"),
                compiled_v2_0_selection(Field::Relationship),
                compiled_v2_0_selection(Field::Direction),
                compiled_v2_0_selection(Field::Distance),
                compiled_v2_0_selection(Field::StopWhen),
                compiled_v2_0_selection(Field::Where),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::FindConnectingFactPaths,
            vec![
                compiled_v2_0_reference(Field::StartingFrom, "starting-from"),
                compiled_v2_0_reference(Field::EndingAt, "ending-at"),
                compiled_v2_0_selection(Field::Through),
                compiled_v2_0_selection(Field::PathPolicy),
                compiled_v2_0_selection(Field::Direction),
                compiled_v2_0_selection(Field::MaximumLength),
                compiled_v2_0_selection(Field::Where),
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
                compiled_v2_0_selection(Field::Where),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::CombineResultSets,
            vec![
                compiled_v2_0_reference(Field::Inputs, "inputs"),
                compiled_v2_0_selection(Field::Combination),
                compiled_v2_0_selection(Field::Identity),
                compiled_v2_0_selection(Field::PreserveOrigin),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::SummarizeObjectiveFacts,
            vec![
                compiled_v2_0_reference(Field::Input, "input"),
                compiled_v2_0_selection(Field::Summaries),
                compiled_v2_0_selection(Field::GroupBy),
                compiled_v2_0_selection(Field::IncludeSupport),
                compiled_v2_0_selection(Field::Where),
            ],
        ),
        compiled_v2_0_form(
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
            vec![
                compiled_v2_0_reference(Field::ForInputs, "for-inputs"),
                compiled_v2_0_selection(Field::Context),
                compiled_v2_0_selection(Field::TextHandling),
                compiled_v2_0_selection(Field::Where),
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
        self.validate_workspace_binding(request, workspace, authority)?;
        self.project_against_catalog(request, authority.ingress_catalog())
    }

    fn prepare_input_requirements(
        &self,
        request: &ParsedSemanticRequest,
        answers: &[SemanticInputAnswer],
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
    ) -> Result<Vec<SemanticInputRequirement>, ProgrammaticQueryPortError> {
        self.validate_workspace_binding(request, workspace, authority)?;
        match self.prepare_against_catalog(request, answers, authority.ingress_catalog())? {
            IngressPreparation::Ready(_) => Ok(Vec::new()),
            IngressPreparation::InputRequired(requirements) => Ok(requirements),
        }
    }

    fn project_resolved(
        &self,
        request: &ResolvedSemanticExecutionRequest,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
    ) -> Result<EpochBoundSemanticIngress, ProgrammaticQueryPortError> {
        self.validate_workspace_binding(request.parsed(), workspace, authority)?;
        match self.prepare_against_catalog(
            request.parsed(),
            request.answers(),
            authority.ingress_catalog(),
        )? {
            IngressPreparation::Ready(ingress) => Ok(ingress),
            IngressPreparation::InputRequired(_) => Err(rejected(
                "resolved operation still requires guarded semantic input",
            )),
        }
    }
}

#[derive(Debug)]
enum IngressPreparation {
    InputRequired(Vec<SemanticInputRequirement>),
    Ready(EpochBoundSemanticIngress),
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
    query_programs: BTreeMap<Arc<str>, Arc<str>>,
    installed_selections: BTreeSet<(Arc<str>, Arc<str>)>,
    installed_returns: BTreeSet<(Arc<str>, Arc<str>)>,
    installed_inputs: BTreeSet<(Arc<str>, Arc<str>)>,
    selection_resolution_required: BTreeSet<(Arc<str>, Arc<str>)>,
    selection_resolutions: BTreeMap<(Arc<str>, Arc<str>, SemanticClauseValue), SemanticClauseValue>,
    answers: BTreeMap<String, SemanticInputValue>,
    consumed_answers: BTreeSet<String>,
    requirements: Vec<SemanticInputRequirement>,
}

impl IngressProjection {
    fn try_for_catalog(
        catalog: &EpochBoundSemanticIngressCatalog,
        answers: &[SemanticInputAnswer],
    ) -> Result<Self, ProgrammaticQueryPortError> {
        let mut projection = Self {
            installed_selections: catalog
                .selections
                .iter()
                .map(|row| (row.program_binding_id.clone(), row.selection_id.clone()))
                .collect(),
            installed_returns: catalog
                .returns
                .iter()
                .map(|row| (row.program_binding_id.clone(), row.return_id.clone()))
                .collect(),
            installed_inputs: catalog
                .request_inputs
                .iter()
                .map(|row| (row.program_binding_id.clone(), row.input_id.clone()))
                .collect(),
            ..Self::default()
        };
        for answer in answers {
            if projection
                .answers
                .insert(answer.semantic_field_id.clone(), answer.value.clone())
                .is_some()
            {
                return Err(rejected("guarded input field is duplicated"));
            }
        }
        for binding in &catalog.selections {
            let binding_key = (
                Arc::clone(&binding.program_binding_id),
                Arc::clone(&binding.selection_id),
            );
            if !binding.resolutions.is_empty() {
                projection
                    .selection_resolution_required
                    .insert(binding_key.clone());
            }
            for resolution in &binding.resolutions {
                let key = (
                    Arc::clone(&binding_key.0),
                    Arc::clone(&binding_key.1),
                    resolution.request_value.clone(),
                );
                if projection
                    .selection_resolutions
                    .insert(key, resolution.execution_value.clone())
                    .is_some()
                {
                    return Err(rejected(format!(
                        "program {} selection {} has duplicate value resolution",
                        binding.program_binding_id, binding.selection_id
                    )));
                }
            }
        }
        Ok(projection)
    }

    fn validate_answer_consumption(&self) -> Result<(), ProgrammaticQueryPortError> {
        if self.answers.len() != self.consumed_answers.len()
            || self
                .answers
                .keys()
                .any(|field| !self.consumed_answers.contains(field))
        {
            return Err(rejected(
                "guarded input does not name an unresolved installed selection",
            ));
        }
        Ok(())
    }

    fn bind_query_program(
        &mut self,
        query_id: &Arc<str>,
        program_binding_id: &Arc<str>,
    ) -> Result<(), ProgrammaticQueryPortError> {
        if self
            .query_programs
            .insert(Arc::clone(query_id), Arc::clone(program_binding_id))
            .is_some()
        {
            return Err(rejected(format!(
                "query {query_id} was bound to a program more than once"
            )));
        }
        Ok(())
    }

    fn require_target(
        &self,
        query: &str,
        target: &Arc<str>,
        installed: &BTreeSet<(Arc<str>, Arc<str>)>,
    ) -> Result<(), ProgrammaticQueryPortError> {
        let program = self
            .query_programs
            .get(query)
            .ok_or_else(|| rejected("query has no selected program"))?;
        if !installed.contains(&(program.clone(), target.clone())) {
            return Err(unavailable(
                target,
                "the selected program does not implement this input",
            ));
        }
        Ok(())
    }

    fn push_selection(
        &mut self,
        query_id: &str,
        selection_id: &Arc<str>,
        value: SemanticClauseValue,
    ) -> Result<(), ProgrammaticQueryPortError> {
        self.require_target(query_id, selection_id, &self.installed_selections)?;
        let query_id: Arc<str> = Arc::from(query_id);
        let program_binding_id = self.query_programs.get(query_id.as_ref()).ok_or_else(|| {
            rejected(format!(
                "query {query_id} has no selected program for semantic resolution"
            ))
        })?;
        let key = (Arc::clone(&query_id), Arc::clone(selection_id));
        let ordinal = next_ordinal(&mut self.selection_ordinals, key)?;
        let binding_key = (Arc::clone(program_binding_id), Arc::clone(selection_id));
        let value = if self.selection_resolution_required.contains(&binding_key) {
            if let Some(execution_value) = self.selection_resolutions.get(&(
                Arc::clone(program_binding_id),
                Arc::clone(selection_id),
                value.clone(),
            )) {
                execution_value.clone()
            } else {
                let field_id = guarded_selection_field_id(
                    &query_id,
                    program_binding_id,
                    selection_id,
                    ordinal,
                );
                let candidates = self
                    .selection_resolutions
                    .iter()
                    .filter(|((program, selection, _), _)| {
                        program == program_binding_id && selection == selection_id
                    })
                    .map(|((_, _, request_value), execution_value)| {
                        let choice_id = guarded_selection_choice_id(
                            program_binding_id,
                            selection_id,
                            request_value,
                        );
                        Ok((
                            SemanticAuthorizedChoice {
                                choice_id: choice_id.clone(),
                                presentation_key: selection_presentation(request_value, choice_id),
                                value: semantic_input_value(request_value)?,
                            },
                            execution_value.clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>, ProgrammaticQueryPortError>>()?;
                if candidates.is_empty() {
                    return Err(rejected(format!(
                        "selection {selection_id} has no admitted resolution candidates"
                    )));
                }
                match self.answers.get(&field_id) {
                    Some(SemanticInputValue::Choice(choice_id)) => {
                        let execution_value = candidates
                            .iter()
                            .find_map(|(choice, execution)| {
                                (choice.choice_id == *choice_id).then(|| execution.clone())
                            })
                            .ok_or_else(|| {
                                rejected("guarded input choice is outside the installed catalog")
                            })?;
                        self.consumed_answers.insert(field_id);
                        execution_value
                    }
                    Some(_) => {
                        return Err(rejected(
                            "guarded selection input is not an authorized enum choice",
                        ));
                    }
                    None => {
                        self.requirements.push(SemanticInputRequirement {
                            semantic_field_id: field_id,
                            input_kind: SemanticInputKind::Enum,
                            presentation_key: "input.selection-resolution".to_owned(),
                            description_key: Some(
                                "input.selection-resolution.description".to_owned(),
                            ),
                            required: true,
                            constraints: Some(SemanticInputConstraints::Enum {
                                minimum_selections: 1,
                                maximum_selections: 1,
                            }),
                            authorized_choices: candidates
                                .into_iter()
                                .map(|(choice, _)| choice)
                                .collect(),
                        });
                        return Ok(());
                    }
                }
            }
        } else {
            value
        };
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
        self.require_target(query_id, return_id, &self.installed_returns)?;
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
        self.require_target(query_id, input_id, &self.installed_inputs)?;
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

fn guarded_selection_field_id(
    query_id: &str,
    program_binding_id: &str,
    selection_id: &str,
    ordinal: u32,
) -> String {
    let digest = guarded_selection_digest(
        b"field",
        &[
            query_id.as_bytes(),
            program_binding_id.as_bytes(),
            selection_id.as_bytes(),
            &ordinal.to_be_bytes(),
        ],
    );
    format!("field:{}", hex_bytes(digest.as_bytes()))
}

fn guarded_selection_choice_id(
    program_binding_id: &str,
    selection_id: &str,
    value: &SemanticClauseValue,
) -> String {
    let encoded = semantic_clause_bytes(value);
    let digest = guarded_selection_digest(
        b"choice",
        &[
            program_binding_id.as_bytes(),
            selection_id.as_bytes(),
            &encoded,
        ],
    );
    format!("choice:{}", hex_bytes(digest.as_bytes()))
}

fn guarded_selection_digest(domain: &[u8], values: &[&[u8]]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.semantic-guard.selection.v1\0");
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for value in values {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    hasher.finalize()
}

fn semantic_clause_bytes(value: &SemanticClauseValue) -> Vec<u8> {
    match value {
        SemanticClauseValue::Boolean(value) => vec![0, u8::from(*value)],
        SemanticClauseValue::Int64(value) => {
            let mut encoded = vec![1];
            encoded.extend_from_slice(&value.to_be_bytes());
            encoded
        }
        SemanticClauseValue::UInt64(value) => {
            let mut encoded = vec![2];
            encoded.extend_from_slice(&value.to_be_bytes());
            encoded
        }
        SemanticClauseValue::Text(value) => {
            let mut encoded = vec![3];
            encoded.extend_from_slice(value.as_bytes());
            encoded
        }
    }
}

fn semantic_input_value(
    value: &SemanticClauseValue,
) -> Result<SemanticInputValue, ProgrammaticQueryPortError> {
    match value {
        SemanticClauseValue::Boolean(value) => Ok(SemanticInputValue::Boolean(*value)),
        SemanticClauseValue::Int64(value) => Ok(SemanticInputValue::Integer(*value)),
        SemanticClauseValue::UInt64(value) => i64::try_from(*value)
            .map(SemanticInputValue::Integer)
            .map_err(|_| rejected("guarded selection integer exceeds the transport range")),
        SemanticClauseValue::Text(value) => Ok(SemanticInputValue::String(value.to_string())),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
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
        ProgrammaticFormIngressTarget::ExplicitResultLimit => "explicit result limit",
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
        ProgrammaticFormIngressTarget::Return { return_id } => {
            insert_target_id(target_ids, "return", return_id)?;
        }
        ProgrammaticFormIngressTarget::ExplicitResultLimit => {}
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
            "{family} mapping {value} is ambiguous within one form"
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

pub(crate) mod code_literals;
mod family_selection;

fn select_clause_program<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    clause: &SemanticQueryClause,
    output_role_id: &Arc<str>,
    projection: &mut IngressProjection,
) -> Result<Option<&'a EpochBoundProgramBindingRow>, ProgrammaticQueryPortError> {
    let form = released_form(clause);
    let candidates = catalog
        .program_bindings
        .iter()
        .filter(|binding| {
            binding.compatibility_form == form && binding.output_role_id == *output_role_id
        })
        .collect::<Vec<_>>();
    if candidates.len() > 1
        && let SemanticQueryClause::RetrieveFacts { facts, .. } = clause
    {
        return family_selection::select(
            catalog,
            &candidates,
            clause.query_id(),
            facts,
            projection,
        );
    }
    if candidates.len() > 1
        && let SemanticQueryClause::FollowRelationships {
            relationship,
            direction,
            distance,
            ..
        } = clause
    {
        return family_selection::select_relationship(
            catalog,
            &candidates,
            clause.query_id(),
            &[
                ("selection.relationship", relationship.as_str()),
                (
                    "selection.direction",
                    direction.as_deref().unwrap_or("outgoing"),
                ),
                (
                    "selection.distance",
                    distance.as_deref().unwrap_or("one relationship step"),
                ),
            ],
            projection,
        );
    }
    select_program_binding(catalog, form, output_role_id).map(Some)
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
        unavailable(
            "request",
            format!(
                "admitted catalog has no program for {} and output role {}",
                form.label(),
                output_role_id
            ),
        )
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
        unavailable(
            slot_id.as_ref(),
            format!(
                "program {} has no consumer slot {}",
                binding.program_binding_id, slot_id
            ),
        )
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

fn project_return_spec(
    fields: &BTreeMap<ProgrammaticFormIngressField, ProgrammaticFormIngressTarget>,
    consumed: &mut BTreeSet<ProgrammaticFormIngressField>,
    query_id: &str,
    spec: Option<&ReturnSpec>,
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
    let ProgrammaticFormIngressTarget::ExplicitResultLimit = consume_target(
        fields,
        consumed,
        ProgrammaticFormIngressField::ReturnMaximumResults,
    )?
    else {
        return Err(incompatible_target(
            ProgrammaticFormIngressField::ReturnMaximumResults,
        ));
    };
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

fn selection_presentation(value: &SemanticClauseValue, fallback: String) -> String {
    match value {
        SemanticClauseValue::Text(text)
            if crate::production_query_recipe::CANONICAL_ENTITY_SELECTORS
                .iter()
                .any(|(phrase, _)| *phrase == text.as_ref())
                || crate::production_query_recipe::canonical_fact_meaning(text)
                || crate::production_query_recipe::canonical_relationship_meaning(text) =>
        {
            format!("selection.{}", text.to_ascii_lowercase().replace(' ', "-"))
        }
        _ => fallback,
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
                SemanticReference::SourceLocation { source_location } => {
                    source_location.validate().map_err(rejected)?;
                }
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
    let mut semantic = serde_json::from_slice(canonical_bytes).unwrap_or(serde_json::Value::Null);
    code_literals::without_literal_values(&mut semantic);
    let normalized = semantic.to_string().to_ascii_lowercase();
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

fn unavailable(subject: &str, detail: impl Into<String>) -> ProgrammaticQueryPortError {
    ProgrammaticQueryPortError::SemanticUnavailable {
        subject_id: subject.into(),
        detail: detail.into(),
    }
}

fn rejected(message: impl Into<String>) -> ProgrammaticQueryPortError {
    ProgrammaticQueryPortError::Rejected(message.into())
}

#[cfg(test)]
mod tests {
    mod unavailable;

    use super::*;
    use crate::relational_program::RelationId;
    use crate::relational_semantic_query::{
        EpochBoundRequestInputBindingRow, EpochBoundRequestInputField, EpochBoundReturnBindingRow,
        EpochBoundScopeBindingRow, EpochBoundSelectionBindingRow,
        EpochBoundSelectionValueResolution, SemanticRequestLimits, SemanticValueKind,
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
        ResultRole::ALL
            .into_iter()
            .map(|role| ProgrammaticResultRoleMapping {
                role,
                role_id: arc(role.released_id()),
            })
            .collect()
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

    fn selection(field: ProgrammaticFormIngressField) -> ProgrammaticFormIngressMappingRow {
        ProgrammaticFormIngressMappingRow {
            field,
            target: ProgrammaticFormIngressTarget::Selection {
                selection_id: arc(field
                    .compiled_v2_0_selection_id()
                    .expect("test selection field has a typed identity")),
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
            selection(Field::Label),
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
                target: ProgrammaticFormIngressTarget::ExplicitResultLimit,
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
                    selection(Field::LookingFor),
                    references(Field::Within, "within"),
                    selection(Field::Where),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::RetrieveFactsAboutCode,
                vec![
                    references(Field::About, "about"),
                    selection(Field::Facts),
                    selection(Field::At),
                    selection(Field::Where),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::FollowCodeRelationships,
                vec![
                    references(Field::StartingFrom, "starting-from"),
                    selection(Field::Relationship),
                    selection(Field::Direction),
                    selection(Field::Distance),
                    selection(Field::StopWhen),
                    selection(Field::Where),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::FindConnectingFactPaths,
                vec![
                    references(Field::StartingFrom, "starting-from"),
                    references(Field::EndingAt, "ending-at"),
                    selection(Field::Through),
                    selection(Field::PathPolicy),
                    selection(Field::Direction),
                    selection(Field::MaximumLength),
                    selection(Field::Where),
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
                    selection(Field::Where),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::CombineResultSets,
                vec![
                    references(Field::Inputs, "inputs"),
                    selection(Field::Combination),
                    selection(Field::Identity),
                    selection(Field::PreserveOrigin),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::SummarizeObjectiveFacts,
                vec![
                    references(Field::Input, "input"),
                    selection(Field::Summaries),
                    selection(Field::GroupBy),
                    selection(Field::IncludeSupport),
                    selection(Field::Where),
                ],
            ),
            form_mapping(
                ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
                vec![
                    references(Field::ForInputs, "for-inputs"),
                    selection(Field::Context),
                    selection(Field::TextHandling),
                    selection(Field::Where),
                ],
            ),
        ]
    }

    fn port() -> ApplicationOwnedSemanticIngressPort {
        let release = super::super::production_kernel::compile_test_semantic_release();
        ApplicationOwnedSemanticIngressPort::try_compiled_v2_0(&release, limits())
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
            materialized: false,
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
                            resolutions: Vec::new(),
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
                    ProgrammaticFormIngressTarget::ExplicitResultLimit => {}
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
    fn surrounding_source_line_windows_require_explicit_bounded_context_parameters() {
        let port = port();
        let baseline: serde_json::Value =
            serde_json::from_slice(&eight_form_request().canonical_bytes).unwrap();
        for (context, before, after, valid) in [
            ("exact source span", None, None, true),
            ("surrounding lines", None, None, false),
            ("surrounding lines", Some(0), None, true),
            ("surrounding lines", None, Some(4096), true),
            ("surrounding lines", Some(4097), Some(0), false),
            ("exact source span", Some(1), None, false),
        ] {
            let mut wire = baseline.clone();
            wire["queries"][7]["context"] = serde_json::json!(context);
            wire["queries"][7]["return"]["source_lines_before"] = serde_json::json!(before);
            wire["queries"][7]["return"]["source_lines_after"] = serde_json::json!(after);
            let parsed = parse_request(&serde_json::to_vec(&wire).unwrap()).unwrap();
            assert_eq!(
                port.validate_request_shape(&parsed).is_ok(),
                valid,
                "{context}: {before:?} / {after:?}"
            );
        }
    }

    #[test]
    fn all_eight_forms_project_exact_rows_pins_repetitions_and_dependencies() {
        let port = port();
        let catalog = catalog(&port);
        assert!(
            catalog
                .returns
                .iter()
                .all(|row| row.return_id.as_ref() != "return.maximum-results"),
            "the result limit is block metadata and has no catalog return binding"
        );
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
        assert!(
            ingress
                .returns
                .iter()
                .all(|row| row.return_id.as_ref() != "return.maximum-results"),
            "the result limit must not be duplicated as a semantic return row"
        );
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
    fn wp45_programmatic_guard_carries_live_catalog_execution_value() {
        let port = port();
        let mut catalog = catalog(&port);
        let selection_id = ProgrammaticFormIngressField::LookingFor
            .compiled_v2_0_selection_id()
            .expect("looking-for selection");
        let binding = catalog
            .selections
            .iter_mut()
            .find(|binding| {
                binding.program_binding_id.as_ref() == "installed.program.0"
                    && binding.selection_id.as_ref() == selection_id
            })
            .expect("find-entities looking-for binding");
        binding.resolutions = vec![EpochBoundSelectionValueResolution {
            request_value: SemanticClauseValue::Text(Arc::from("functions")),
            execution_value: SemanticClauseValue::Text(Arc::from("function")),
        }];

        let request = eight_form_request();
        let ingress = port
            .project_against_catalog(&request, &catalog)
            .expect("catalog-owned resolution");
        let executable = ingress
            .selections
            .iter()
            .find(|row| row.query_id.as_ref() == "q1" && row.selection_id.as_ref() == selection_id)
            .expect("projected executable selection");
        assert_eq!(
            executable.value,
            SemanticClauseValue::Text(Arc::from("function"))
        );

        catalog
            .selections
            .iter_mut()
            .find(|candidate| {
                candidate.program_binding_id.as_ref() == "installed.program.0"
                    && candidate.selection_id.as_ref() == selection_id
            })
            .expect("find-entities looking-for binding")
            .resolutions[0]
            .request_value = SemanticClauseValue::Text(Arc::from("classes"));
        let IngressPreparation::InputRequired(requirements) = port
            .prepare_against_catalog(&request, &[], &catalog)
            .expect("unavailable phrase becomes typed guarded input")
        else {
            panic!("unavailable phrase must not reach execution");
        };
        assert_eq!(requirements.len(), 1);
        let requirement = &requirements[0];
        assert_eq!(requirement.input_kind, SemanticInputKind::Enum);
        assert_eq!(requirement.authorized_choices.len(), 1);
        assert_eq!(
            requirement.authorized_choices[0].value,
            SemanticInputValue::String("classes".to_owned()),
            "the candidate must come only from the installed request_value row"
        );
        let answer = SemanticInputAnswer {
            semantic_field_id: requirement.semantic_field_id.clone(),
            value: SemanticInputValue::Choice(requirement.authorized_choices[0].choice_id.clone()),
        };
        let IngressPreparation::Ready(resolved) = port
            .prepare_against_catalog(&request, std::slice::from_ref(&answer), &catalog)
            .expect("catalog-owned answer resolves the request")
        else {
            panic!("authorized answer must produce executable ingress");
        };
        let executable = resolved
            .selections
            .iter()
            .find(|row| row.query_id.as_ref() == "q1" && row.selection_id.as_ref() == selection_id)
            .expect("resolved executable selection");
        assert_eq!(
            executable.value,
            SemanticClauseValue::Text(Arc::from("function")),
            "the accepted answer must carry the catalog execution_value into execution ingress"
        );

        let forged = SemanticInputAnswer {
            semantic_field_id: answer.semantic_field_id.clone(),
            value: SemanticInputValue::Choice("choice:forged".to_owned()),
        };
        assert!(
            port.prepare_against_catalog(&request, &[forged], &catalog)
                .expect_err("forged choice must fail closed")
                .to_string()
                .contains("outside the installed catalog")
        );

        catalog
            .selections
            .iter_mut()
            .find(|candidate| {
                candidate.program_binding_id.as_ref() == "installed.program.0"
                    && candidate.selection_id.as_ref() == selection_id
            })
            .expect("find-entities looking-for binding")
            .resolutions[0]
            .request_value = SemanticClauseValue::Text(Arc::from("traits"));
        assert!(
            port.prepare_against_catalog(&request, &[answer], &catalog)
                .expect_err("answer must be revalidated against the live catalog")
                .to_string()
                .contains("outside the installed catalog")
        );
    }

    #[test]
    fn compiled_selection_mappings_use_the_typed_field_identity() {
        for mapping in compiled_v2_0_forms() {
            for row in mapping.fields {
                if let ProgrammaticFormIngressTarget::Selection { selection_id } = row.target {
                    assert_eq!(
                        Some(selection_id.as_ref()),
                        row.field.compiled_v2_0_selection_id(),
                        "selection mapping for {:?} bypassed its typed identity",
                        row.field
                    );
                }
            }
        }
        assert_eq!(
            ProgrammaticFormIngressField::ReturnMaximumResults.compiled_v2_0_selection_id(),
            None
        );
    }

    #[test]
    fn compiled_constructor_is_v2_only_and_has_no_caller_mapping_parameters() {
        type ProductionConstructor =
            fn(
                &super::super::production_kernel::CompiledSemanticRelease,
                EpochBoundSemanticIngressLimits,
            )
                -> Result<ApplicationOwnedSemanticIngressPort, ProgrammaticQueryPortError>;

        let constructor: ProductionConstructor =
            ApplicationOwnedSemanticIngressPort::try_compiled_v2_0;
        let release = super::super::production_kernel::compile_test_semantic_release();
        let port = constructor(&release, limits()).expect("compiled v2 mapping");
        assert_eq!(port.authority_pin(), compiled_query_release_pin(&release));
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
    // One guarded round-trip also rejects cross-family requests and forged choices.
    #[allow(clippy::too_many_lines)]
    #[test]
    fn fact_family_program_selection_and_guard_are_catalog_bound() {
        let port = port();
        let mut catalog = catalog(&port);
        let base = catalog
            .program_bindings
            .iter()
            .find(|b| b.compatibility_form == ReleasedSemanticForm::RetrieveFactsAboutCode)
            .unwrap()
            .clone();
        let mut extra = base.clone();
        extra.program_binding_id = arc("installed.program.types");
        extra.program_binding_pin = [0x31; 32];
        catalog.program_bindings.push(extra.clone());
        let selection = catalog
            .selections
            .iter_mut()
            .find(|b| {
                b.program_binding_id == base.program_binding_id
                    && b.selection_id.as_ref() == "selection.facts"
            })
            .unwrap();
        selection.resolutions = vec![EpochBoundSelectionValueResolution {
            request_value: text("declarations").unwrap(),
            execution_value: text("declarations").unwrap(),
        }];
        let mut typed = selection.clone();
        typed.program_binding_id = extra.program_binding_id.clone();
        typed.resolutions = vec![EpochBoundSelectionValueResolution {
            request_value: text("type observations").unwrap(),
            execution_value: text("types").unwrap(),
        }];
        catalog.selections.push(typed);
        let candidates = vec![
            &catalog.program_bindings[catalog.program_bindings.len() - 1],
            catalog
                .program_bindings
                .iter()
                .find(|b| b.program_binding_id == base.program_binding_id)
                .unwrap(),
        ];
        let mut projection = IngressProjection::try_for_catalog(&catalog, &[]).unwrap();
        let chosen = family_selection::select(
            &catalog,
            &candidates,
            "q",
            &["type observations".to_owned()],
            &mut projection,
        )
        .unwrap()
        .unwrap();
        assert_eq!(chosen.program_binding_id, extra.program_binding_id);
        assert!(
            family_selection::select(
                &catalog,
                &candidates,
                "q",
                &["declarations".to_owned(), "type observations".to_owned()],
                &mut projection
            )
            .is_err()
        );
        assert!(
            family_selection::select(
                &catalog,
                &candidates,
                "q",
                &["unclear".to_owned()],
                &mut projection
            )
            .unwrap()
            .is_none()
        );
        let requirement = &projection.requirements[0];
        assert_eq!(requirement.authorized_choices.len(), 2);
        let choice = requirement
            .authorized_choices
            .iter()
            .find(|choice| {
                choice.value == SemanticInputValue::String("type observations".to_owned())
            })
            .unwrap();
        let answer = SemanticInputAnswer {
            semantic_field_id: requirement.semantic_field_id.clone(),
            value: SemanticInputValue::Choice(choice.choice_id.clone()),
        };
        let mut answered =
            IngressProjection::try_for_catalog(&catalog, std::slice::from_ref(&answer)).unwrap();
        let chosen = family_selection::select(
            &catalog,
            &candidates,
            "q",
            &["unclear".to_owned()],
            &mut answered,
        )
        .unwrap()
        .unwrap();
        answered
            .bind_query_program(&arc("q"), &chosen.program_binding_id)
            .unwrap();
        answered
            .push_selection("q", &arc("selection.facts"), text("unclear").unwrap())
            .unwrap();
        assert_eq!(answered.selections[0].value, text("types").unwrap());
        answered.validate_answer_consumption().unwrap();
        let mut forged = answer;
        forged.value = SemanticInputValue::Choice("choice:forged".to_owned());
        let mut projection = IngressProjection::try_for_catalog(&catalog, &[forged]).unwrap();
        assert!(
            family_selection::select(
                &catalog,
                &candidates,
                "q",
                &["unclear".to_owned()],
                &mut projection
            )
            .is_err()
        );
    }
    // The same catalog and request cross both guarded rounds and the forged-answer check.
    #[allow(clippy::too_many_lines)]
    #[test]
    fn relationship_guard_preserves_direction_candidates_and_binds_both_answers() {
        let port = port();
        let mut catalog = catalog(&port);
        let base = catalog
            .program_bindings
            .iter()
            .find(|p| p.compatibility_form == ReleasedSemanticForm::FollowCodeRelationships)
            .unwrap()
            .clone();
        let template = catalog
            .selections
            .iter()
            .find(|s| s.program_binding_id == base.program_binding_id)
            .unwrap()
            .clone();
        catalog.program_bindings.clear();
        catalog.selections.clear();
        for (ordinal, direction) in ["incoming", "outgoing"].into_iter().enumerate() {
            let mut program = base.clone();
            program.program_binding_id = arc(format!("installed.relationship.{direction}"));
            program.program_binding_pin = [u8::try_from(ordinal + 1).unwrap(); 32];
            for (id, values) in [
                ("selection.relationship", vec!["imports"]),
                ("selection.direction", vec![direction]),
            ] {
                let mut selection = template.clone();
                selection.program_binding_id = program.program_binding_id.clone();
                selection.selection_id = arc(id);
                selection.resolutions = values
                    .into_iter()
                    .map(|value| EpochBoundSelectionValueResolution {
                        request_value: text(value).unwrap(),
                        execution_value: text(value).unwrap(),
                    })
                    .collect();
                catalog.selections.push(selection);
            }
            catalog.program_bindings.push(program);
        }
        let candidates = catalog.program_bindings.iter().collect::<Vec<_>>();
        let dimensions = [
            ("selection.relationship", "unclear family"),
            ("selection.direction", "unclear direction"),
        ];
        let mut projection = IngressProjection::try_for_catalog(&catalog, &[]).unwrap();
        assert!(
            family_selection::select_relationship(
                &catalog,
                &candidates,
                "q",
                &dimensions,
                &mut projection
            )
            .unwrap()
            .is_none()
        );
        let requirement = &projection.requirements[0];
        assert_eq!(
            requirement.authorized_choices.len(),
            1,
            "family choices cannot duplicate per direction"
        );
        let family_answer = SemanticInputAnswer {
            semantic_field_id: requirement.semantic_field_id.clone(),
            value: SemanticInputValue::Choice(requirement.authorized_choices[0].choice_id.clone()),
        };
        let mut projection =
            IngressProjection::try_for_catalog(&catalog, std::slice::from_ref(&family_answer))
                .unwrap();
        assert!(
            family_selection::select_relationship(
                &catalog,
                &candidates,
                "q",
                &dimensions,
                &mut projection
            )
            .unwrap()
            .is_none()
        );
        let requirement = &projection.requirements[0];
        assert_eq!(requirement.authorized_choices.len(), 2);
        let direction_answer = SemanticInputAnswer {
            semantic_field_id: requirement.semantic_field_id.clone(),
            value: SemanticInputValue::Choice(
                requirement
                    .authorized_choices
                    .iter()
                    .find(|c| c.value == SemanticInputValue::String("incoming".into()))
                    .unwrap()
                    .choice_id
                    .clone(),
            ),
        };
        let mut projection = IngressProjection::try_for_catalog(
            &catalog,
            &[family_answer.clone(), direction_answer.clone()],
        )
        .unwrap();
        let chosen = family_selection::select_relationship(
            &catalog,
            &candidates,
            "q",
            &dimensions,
            &mut projection,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            chosen.program_binding_id.as_ref(),
            "installed.relationship.incoming"
        );
        projection
            .bind_query_program(&arc("q"), &chosen.program_binding_id)
            .unwrap();
        for (selection, value) in dimensions {
            projection
                .push_selection("q", &arc(selection), text(value).unwrap())
                .unwrap();
        }
        assert_eq!(
            projection
                .selections
                .iter()
                .map(|s| s.value.clone())
                .collect::<Vec<_>>(),
            vec![text("imports").unwrap(), text("incoming").unwrap()]
        );
        projection.validate_answer_consumption().unwrap();
        let mut forged = family_answer;
        forged.value = direction_answer.value;
        let mut projection = IngressProjection::try_for_catalog(&catalog, &[forged]).unwrap();
        assert!(
            family_selection::select_relationship(
                &catalog,
                &candidates,
                "q",
                &dimensions,
                &mut projection
            )
            .is_err()
        );
    }
}
