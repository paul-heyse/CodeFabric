//! Production construction of the released semantic-query program catalogs.
//!
//! This module is an application-owned composition boundary.  It accepts typed Rust program
//! definitions, checks every epoch-owned relation and field against the exact sealed
//! [`ProgrammaticFabricEpoch`], and emits the two catalogs consumed by the programmatic query
//! ports.  It deliberately accepts neither a serialized semantic manifest nor caller-selected
//! catalog/program pins.  Pins emitted here are only canonical identities of typed program rows;
//! semantic validity comes from the relation/field checks and native compiler execution.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray, UInt64Array};

use crate::fabric::derived_producer_closure::{
    DerivedProducerClosureExecution, FamilyClosureFields, ProducerClosureCompilationDependency,
};
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpoch;
use crate::fabric::programmatic_schema::ProgrammaticRelationId;
use crate::fabric::programmatic_workspace::programmatic_fabric_epoch_authority_pin;
use crate::relational_program::{FieldId, RelationId, ScalarOperator};
use crate::relational_semantic_query::{
    EpochBoundConsumerComposition, EpochBoundConsumerSlotBindingRow,
    EpochBoundExecutionConsumerSlotRow, EpochBoundExecutionOperatorRow,
    EpochBoundExecutionProgramRow, EpochBoundExecutionRequestInputRow,
    EpochBoundExecutionRequiredFamilyRow, EpochBoundExecutionReturnRow,
    EpochBoundExecutionScopeRow, EpochBoundExecutionSelectionRow, EpochBoundProgramBindingRow,
    EpochBoundRequestInputBindingRow, EpochBoundRequestInputField, EpochBoundReturnBindingRow,
    EpochBoundScopeBindingRow, EpochBoundSelectionBindingRow, EpochBoundSelectionFold,
    EpochBoundSemanticExecutionCatalog, EpochBoundSemanticIngressCatalog,
    EpochBoundSemanticIngressLimits, ProducerClosureProof, ProducerFamilyClosureRow,
    ProducerFamilyDisposition, ProgramRelationSchemaRow, ProgramRelationalOperator,
    ReleasedSemanticForm, RuntimeProducerProof, SemanticClauseValue, SemanticQueryAuthority,
    SemanticQueryClass, SemanticValueKind, UnsupportedFamilyRemainder,
    epoch_bound_semantic_ingress_limits_pin,
};
use crate::schema_contract::SchemaRole;

/// Whether a relation is owned by the sealed epoch or exists only inside one compiled request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRelationAuthority {
    /// The exact relation and field sequence must exist in the sealed epoch.
    Epoch,
    /// The relation is supplied through a validated request-input or prior-result handoff.
    QueryLocal,
    /// The relation names the result schema of this application-owned program.
    ProgramResult,
}

/// One relation schema referenced by a typed production program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRelationDefinition {
    pub relation_id: RelationId,
    pub fields: Vec<FieldId>,
    pub authority: ProductionRelationAuthority,
}

/// One operator node.  Its program and execution pins are derived by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionOperatorDefinition {
    pub node_id: Arc<str>,
    pub ordinal: u32,
    pub input_node_ids: Vec<Arc<str>>,
    pub operator: ProgramRelationalOperator,
    pub output_fields: Vec<FieldId>,
}

/// Ingress and execution realization for one repeatable selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSelectionDefinition {
    pub selection_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
    pub operator_node_id: Arc<str>,
    pub input_field_id: FieldId,
    pub scalar_operator: ScalarOperator,
    pub fold: EpochBoundSelectionFold,
}

/// One exact return value and its programmatic realization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionReturnRealization {
    pub value: SemanticClauseValue,
    pub realization_node_id: Arc<str>,
    pub realization_field_ids: Vec<FieldId>,
}

/// Ingress contract and finite execution realizations for one return directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionReturnDefinition {
    pub return_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
    pub realizations: Vec<ProductionReturnRealization>,
}

/// One request-owned relation consumed by a program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRequestInputDefinition {
    pub input_id: Arc<str>,
    pub relation_id: RelationId,
    pub fields: Vec<EpochBoundRequestInputField>,
    pub minimum_rows: usize,
    pub maximum_rows: usize,
}

/// One prior-result consumer slot consumed by a program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionConsumerSlotDefinition {
    pub consumer_slot_id: Arc<str>,
    pub consumer_role_id: Arc<str>,
    pub input_relation_id: RelationId,
    pub minimum_edges: usize,
    pub maximum_edges: usize,
    pub composition: EpochBoundConsumerComposition,
}

/// Complete compiled-Rust definition of one released form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticFormProgram {
    pub form: ReleasedSemanticForm,
    pub program_binding_id: Arc<str>,
    pub output_role_id: Arc<str>,
    pub root_node_id: Arc<str>,
    pub output_relation_id: RelationId,
    pub output_fields: Vec<FieldId>,
    pub relations: Vec<ProductionRelationDefinition>,
    pub operators: Vec<ProductionOperatorDefinition>,
    pub selections: Vec<ProductionSelectionDefinition>,
    pub returns: Vec<ProductionReturnDefinition>,
    pub request_inputs: Vec<ProductionRequestInputDefinition>,
    pub consumer_slots: Vec<ProductionConsumerSlotDefinition>,
    pub required_fact_families: Vec<Arc<str>>,
}

/// Request-global scope contract and its child-authorization handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionScopeDefinition {
    pub scope_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
    pub authorization_input_id: Arc<str>,
}

/// Explicit application inputs that are not derivable from the sealed epoch.
#[derive(Clone, Debug)]
pub struct ProductionSemanticQueryRecipeInput {
    pub source_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub program_release_pin: [u8; 32],
    pub factual_semantic_class_id: Arc<str>,
    pub limits: EpochBoundSemanticIngressLimits,
    pub scopes: Vec<ProductionScopeDefinition>,
    pub forms: Vec<ProductionSemanticFormProgram>,
}

/// Complete epoch-bound products accepted by workspace construction and concrete query ports.
#[derive(Clone, Debug)]
pub struct ProductionSemanticQueryRecipe {
    ingress_catalog: Arc<EpochBoundSemanticIngressCatalog>,
    execution_catalog: Arc<EpochBoundSemanticExecutionCatalog>,
    producer_closure: Arc<ProducerClosureProof>,
}

impl ProductionSemanticQueryRecipe {
    /// Construct the released program catalogs from typed Rust definitions and an actually
    /// executed producer-closure compilation.
    ///
    /// `closure_execution` cannot be fabricated outside its owning module: it is returned only by
    /// [`crate::fabric::derived_producer_closure::CompiledDerivedProducerClosure::execute`].  A
    /// non-empty violation relation fails construction before any query catalog is returned.
    ///
    /// # Errors
    ///
    /// Rejects incomplete released-form coverage, relation/field drift, invalid operator graphs,
    /// incomplete ingress realization, or any producer-closure violation.
    pub fn try_from_executed_closure(
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
        closure_execution: &DerivedProducerClosureExecution,
        closure_fields: &FamilyClosureFields,
    ) -> Result<Self, ProductionQueryRecipeError> {
        let producer_closure = decode_executed_closure(
            epoch,
            closure_execution,
            closure_fields,
            &input.factual_semantic_class_id,
        )?;
        Self::assemble(epoch, input, producer_closure)
    }

    fn assemble(
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
        producer_closure: ProducerClosureProof,
    ) -> Result<Self, ProductionQueryRecipeError> {
        validate_pin("source", input.source_pin)?;
        validate_pin("policy", input.policy_pin)?;
        validate_pin("program release", input.program_release_pin)?;
        validate_identity("factual semantic class", &input.factual_semantic_class_id)?;
        let forms = validate_form_coverage(input.forms)?;
        let fabric_epoch_pin = programmatic_fabric_epoch_authority_pin(epoch);
        let limits_pin = epoch_bound_semantic_ingress_limits_pin(input.limits);

        let mut program_pins = BTreeMap::new();
        for (form, definition) in &forms {
            validate_program(epoch, definition, input.limits)?;
            program_pins.insert(*form, program_identity_pin(definition));
        }
        validate_required_closure(&forms, &producer_closure)?;

        let program_catalog_pin = catalog_identity_pin(
            b"codefabric.production-semantic-program-catalog.v1",
            fabric_epoch_pin,
            input.source_pin,
            input.policy_pin,
            input.program_release_pin,
            &forms,
        );
        let execution_catalog_pin = catalog_identity_pin(
            b"codefabric.production-semantic-execution-catalog.v1",
            fabric_epoch_pin,
            input.source_pin,
            input.policy_pin,
            input.program_release_pin,
            &forms,
        );

        let mut ingress = EpochBoundSemanticIngressCatalog {
            fabric_epoch_pin,
            program_catalog_pin,
            source_pin: input.source_pin,
            policy_pin: input.policy_pin,
            producer_closure_proof_pin: producer_closure.proof_pin,
            limits_pin,
            program_bindings: Vec::new(),
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            scopes: Vec::new(),
            request_inputs: Vec::new(),
        };
        let mut execution = EpochBoundSemanticExecutionCatalog {
            fabric_epoch_pin,
            program_catalog_pin,
            source_pin: input.source_pin,
            policy_pin: input.policy_pin,
            producer_closure_proof_pin: producer_closure.proof_pin,
            execution_catalog_pin,
            program_release_pin: input.program_release_pin,
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::clone(
                &producer_closure.application_authority_id,
            )),
            semantic_class: SemanticQueryClass::Fact(Arc::clone(&input.factual_semantic_class_id)),
            programs: Vec::new(),
            operators: Vec::new(),
            relation_schemas: Vec::new(),
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            required_fact_families: Vec::new(),
            request_inputs: Vec::new(),
            scopes: Vec::new(),
        };

        let mut schemas = BTreeMap::<RelationId, Vec<FieldId>>::new();
        for (form, definition) in forms {
            let execution_program_pin = program_pins[&form];
            let binding_pin = binding_identity_pin(&definition, execution_program_pin);
            ingress.program_bindings.push(EpochBoundProgramBindingRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                program_binding_pin: binding_pin,
                compatibility_form: form,
                output_role_id: Arc::clone(&definition.output_role_id),
                execution_program_pin,
            });
            execution.programs.push(EpochBoundExecutionProgramRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                execution_program_pin,
                root_node_id: Arc::clone(&definition.root_node_id),
                output_relation_id: definition.output_relation_id.clone(),
                output_fields: definition.output_fields.clone(),
            });
            for relation in &definition.relations {
                match schemas.entry(relation.relation_id.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(relation.fields.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(entry)
                        if entry.get() == &relation.fields => {}
                    std::collections::btree_map::Entry::Occupied(_) => {
                        return Err(ProductionQueryRecipeError::RelationSchemaConflict(
                            relation.relation_id.as_str().to_owned(),
                        ));
                    }
                }
            }
            for operator in &definition.operators {
                execution.operators.push(EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::clone(&definition.program_binding_id),
                    execution_program_pin,
                    node_id: Arc::clone(&operator.node_id),
                    ordinal: operator.ordinal,
                    input_node_ids: operator.input_node_ids.clone(),
                    operator: operator.operator.clone(),
                    output_fields: operator.output_fields.clone(),
                });
            }
            append_selections(
                &definition,
                execution_program_pin,
                &mut ingress,
                &mut execution,
            );
            append_returns(
                &definition,
                execution_program_pin,
                &mut ingress,
                &mut execution,
            );
            append_request_inputs(
                &definition,
                execution_program_pin,
                &mut ingress,
                &mut execution,
            );
            append_consumer_slots(
                &definition,
                execution_program_pin,
                &mut ingress,
                &mut execution,
            );
            for family_id in &definition.required_fact_families {
                execution
                    .required_fact_families
                    .push(EpochBoundExecutionRequiredFamilyRow {
                        program_binding_id: Arc::clone(&definition.program_binding_id),
                        execution_program_pin,
                        family_id: Arc::clone(family_id),
                    });
            }
        }
        execution.relation_schemas = schemas
            .into_iter()
            .map(|(relation_id, fields)| ProgramRelationSchemaRow {
                relation_id,
                fields,
            })
            .collect();
        append_scopes(
            input.scopes,
            &mut ingress,
            &mut execution,
            program_catalog_pin,
        )?;

        Ok(Self {
            ingress_catalog: Arc::new(ingress),
            execution_catalog: Arc::new(execution),
            producer_closure: Arc::new(producer_closure),
        })
    }

    #[must_use]
    pub const fn ingress_catalog(&self) -> &Arc<EpochBoundSemanticIngressCatalog> {
        &self.ingress_catalog
    }

    #[must_use]
    pub const fn execution_catalog(&self) -> &Arc<EpochBoundSemanticExecutionCatalog> {
        &self.execution_catalog
    }

    #[must_use]
    pub const fn producer_closure(&self) -> &Arc<ProducerClosureProof> {
        &self.producer_closure
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Arc<EpochBoundSemanticIngressCatalog>,
        Arc<EpochBoundSemanticExecutionCatalog>,
        Arc<ProducerClosureProof>,
    ) {
        (
            self.ingress_catalog,
            self.execution_catalog,
            self.producer_closure,
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProductionQueryRecipeError {
    #[error("required {0} pin is absent")]
    MissingPin(&'static str),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("released form coverage is incomplete or duplicated: {0}")]
    ReleasedFormCoverage(String),
    #[error("program {program} relation {relation} is absent from the sealed epoch")]
    MissingEpochRelation { program: String, relation: String },
    #[error("program {program} relation {relation} field contract differs from the sealed epoch")]
    EpochFieldDrift { program: String, relation: String },
    #[error("relation {0} has conflicting program schemas")]
    RelationSchemaConflict(String),
    #[error("program {program} is invalid: {detail}")]
    InvalidProgram { program: String, detail: String },
    #[error("producer closure execution emitted conformance violations")]
    ProducerClosureViolations,
    #[error("producer closure schema is invalid: {0}")]
    ProducerClosureSchema(String),
    #[error("producer closure row {row} is invalid: {detail}")]
    ProducerClosureRow { row: usize, detail: String },
    #[error("program {program} requires absent producer family {family}")]
    MissingProducerFamily { program: String, family: String },
}

fn validate_pin(kind: &'static str, pin: [u8; 32]) -> Result<(), ProductionQueryRecipeError> {
    if pin == [0; 32] {
        Err(ProductionQueryRecipeError::MissingPin(kind))
    } else {
        Ok(())
    }
}

fn validate_identity(kind: &'static str, value: &str) -> Result<(), ProductionQueryRecipeError> {
    if value.is_empty()
        || value.len() > 1_024
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(ProductionQueryRecipeError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_form_coverage(
    forms: Vec<ProductionSemanticFormProgram>,
) -> Result<BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>, ProductionQueryRecipeError>
{
    let mut indexed = BTreeMap::new();
    for form in forms {
        if indexed.insert(form.form, form).is_some() {
            return Err(ProductionQueryRecipeError::ReleasedFormCoverage(
                "duplicate form".to_owned(),
            ));
        }
    }
    let expected = ReleasedSemanticForm::ALL
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = indexed.keys().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ProductionQueryRecipeError::ReleasedFormCoverage(format!(
            "expected {expected:?}, observed {actual:?}"
        )));
    }
    Ok(indexed)
}

fn validate_program(
    epoch: &ProgrammaticFabricEpoch,
    program: &ProductionSemanticFormProgram,
    limits: EpochBoundSemanticIngressLimits,
) -> Result<(), ProductionQueryRecipeError> {
    validate_identity("program binding", &program.program_binding_id)?;
    validate_identity("output role", &program.output_role_id)?;
    validate_identity("root node", &program.root_node_id)?;
    if program.operators.is_empty()
        || program.operators.len() > limits.compiler().max_operator_nodes_per_block()
    {
        return invalid(
            program,
            "operator count is empty or exceeds the compiler bound",
        );
    }
    let mut relation_schemas = BTreeMap::new();
    for relation in &program.relations {
        if relation.fields.is_empty()
            || relation.fields.len() > limits.compiler().max_fields_per_node()
            || relation_schemas
                .insert(relation.relation_id.clone(), relation.fields.clone())
                .is_some()
        {
            return invalid(
                program,
                "relation schema is empty, duplicated, or over bound",
            );
        }
        if relation.authority == ProductionRelationAuthority::Epoch {
            let epoch_id = ProgrammaticRelationId::new(relation.relation_id.as_str());
            let sealed = epoch.relation(&epoch_id).ok_or_else(|| {
                ProductionQueryRecipeError::MissingEpochRelation {
                    program: program.program_binding_id.to_string(),
                    relation: relation.relation_id.as_str().to_owned(),
                }
            })?;
            let observed = (0..sealed.contract.logical_schema().fields().len())
                .map(|index| {
                    sealed
                        .contract
                        .field_id_at(SchemaRole::Logical, index)
                        .map(FieldId::new)
                        .map_err(|error| error.to_string())?
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ProductionQueryRecipeError::EpochFieldDrift {
                    program: program.program_binding_id.to_string(),
                    relation: relation.relation_id.as_str().to_owned(),
                })?;
            if observed != relation.fields {
                return Err(ProductionQueryRecipeError::EpochFieldDrift {
                    program: program.program_binding_id.to_string(),
                    relation: relation.relation_id.as_str().to_owned(),
                });
            }
        }
    }
    let query_local = program
        .request_inputs
        .iter()
        .map(|input| &input.relation_id)
        .chain(
            program
                .consumer_slots
                .iter()
                .map(|slot| &slot.input_relation_id),
        )
        .collect::<BTreeSet<_>>();
    for relation in &program.relations {
        if relation.authority == ProductionRelationAuthority::QueryLocal
            && !query_local.contains(&relation.relation_id)
        {
            return invalid(
                program,
                &format!(
                    "query-local relation {} has no request/prior-result handoff",
                    relation.relation_id.as_str()
                ),
            );
        }
    }

    let mut nodes = BTreeMap::<Arc<str>, &ProductionOperatorDefinition>::new();
    for (expected_ordinal, node) in program.operators.iter().enumerate() {
        validate_identity("operator node", &node.node_id)?;
        if usize::try_from(node.ordinal).ok() != Some(expected_ordinal)
            || nodes.contains_key(node.node_id.as_ref())
        {
            return invalid(
                program,
                "operator ordinals or node identities are not unique",
            );
        }
        validate_operator_node(program, node, &nodes, &relation_schemas)?;
        nodes.insert(Arc::clone(&node.node_id), node);
    }
    let root = nodes.get(program.root_node_id.as_ref()).ok_or_else(|| {
        ProductionQueryRecipeError::InvalidProgram {
            program: program.program_binding_id.to_string(),
            detail: "root node is absent".to_owned(),
        }
    })?;
    if root.output_fields != program.output_fields {
        return invalid(program, "root fields differ from program output fields");
    }
    let output_schema = relation_schemas
        .get(&program.output_relation_id)
        .ok_or_else(|| ProductionQueryRecipeError::InvalidProgram {
            program: program.program_binding_id.to_string(),
            detail: "output relation schema is absent".to_owned(),
        })?;
    if output_schema != &program.output_fields {
        return invalid(program, "output relation schema differs from root fields");
    }
    validate_program_bindings(program, &nodes, &relation_schemas)
}

fn validate_operator_node(
    program: &ProductionSemanticFormProgram,
    node: &ProductionOperatorDefinition,
    preceding: &BTreeMap<Arc<str>, &ProductionOperatorDefinition>,
    schemas: &BTreeMap<RelationId, Vec<FieldId>>,
) -> Result<(), ProductionQueryRecipeError> {
    let inputs = node
        .input_node_ids
        .iter()
        .map(|id| {
            preceding.get(id.as_ref()).copied().ok_or_else(|| {
                ProductionQueryRecipeError::InvalidProgram {
                    program: program.program_binding_id.to_string(),
                    detail: format!("node {} has unresolved or forward input {id}", node.node_id),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match &node.operator {
        ProgramRelationalOperator::Input { relation_id } => {
            if !inputs.is_empty() {
                return invalid(program, "input node has operator inputs");
            }
            if schemas.get(relation_id) != Some(&node.output_fields) {
                return invalid(program, "input node differs from its relation schema");
            }
        }
        ProgramRelationalOperator::Projection { fields } => {
            if inputs.len() != 1
                || fields
                    .iter()
                    .any(|field| !inputs[0].output_fields.contains(&field.input_field_id))
                || fields
                    .iter()
                    .map(|field| &field.output_field_id)
                    .ne(node.output_fields.iter())
            {
                return invalid(program, "projection field lineage is invalid");
            }
        }
        ProgramRelationalOperator::Filter
        | ProgramRelationalOperator::Sort { .. }
        | ProgramRelationalOperator::Limit { .. } => {
            if inputs.len() != 1 || inputs[0].output_fields != node.output_fields {
                return invalid(program, "unary operator changes its field contract");
            }
        }
        ProgramRelationalOperator::Join { predicates, .. } => {
            if inputs.len() != 2
                || predicates.iter().any(|predicate| {
                    !inputs[0].output_fields.contains(&predicate.left_field_id)
                        || !inputs[1].output_fields.contains(&predicate.right_field_id)
                })
                || node.output_fields.iter().any(|field| {
                    !inputs[0].output_fields.contains(field)
                        && !inputs[1].output_fields.contains(field)
                })
            {
                return invalid(program, "join field lineage is invalid");
            }
        }
        ProgramRelationalOperator::Union { .. } => {
            if inputs.len() < 2
                || inputs
                    .iter()
                    .any(|input| input.output_fields != node.output_fields)
            {
                return invalid(program, "union schemas differ");
            }
        }
        ProgramRelationalOperator::Aggregate {
            group_by,
            aggregates,
        } => {
            if inputs.len() != 1
                || group_by
                    .iter()
                    .any(|field| !inputs[0].output_fields.contains(&field.input_field_id))
                || aggregates
                    .iter()
                    .any(|field| !inputs[0].output_fields.contains(&field.input_field_id))
            {
                return invalid(program, "aggregate input lineage is invalid");
            }
            let outputs = group_by
                .iter()
                .map(|field| &field.output_field_id)
                .chain(aggregates.iter().map(|field| &field.output_field_id));
            if outputs.ne(node.output_fields.iter()) {
                return invalid(program, "aggregate output lineage is invalid");
            }
        }
    }
    Ok(())
}

fn validate_program_bindings(
    program: &ProductionSemanticFormProgram,
    nodes: &BTreeMap<Arc<str>, &ProductionOperatorDefinition>,
    schemas: &BTreeMap<RelationId, Vec<FieldId>>,
) -> Result<(), ProductionQueryRecipeError> {
    for selection in &program.selections {
        let node = nodes
            .get(selection.operator_node_id.as_ref())
            .ok_or_else(|| ProductionQueryRecipeError::InvalidProgram {
                program: program.program_binding_id.to_string(),
                detail: format!("selection {} has no filter node", selection.selection_id),
            })?;
        let input = node
            .input_node_ids
            .first()
            .and_then(|id| nodes.get(id.as_ref()))
            .copied();
        if !matches!(node.operator, ProgramRelationalOperator::Filter)
            || input.is_none_or(|input| !input.output_fields.contains(&selection.input_field_id))
            || selection.maximum_values == 0
            || selection.minimum_values > selection.maximum_values
        {
            return invalid(
                program,
                "selection binding is not causally attached to a filter",
            );
        }
    }
    for return_definition in &program.returns {
        if return_definition.maximum_values == 0
            || return_definition.minimum_values > return_definition.maximum_values
            || return_definition.realizations.is_empty()
            || return_definition.realizations.iter().any(|realization| {
                nodes
                    .get(realization.realization_node_id.as_ref())
                    .is_none_or(|node| {
                        realization.realization_field_ids.is_empty()
                            || realization
                                .realization_field_ids
                                .iter()
                                .any(|field| !node.output_fields.contains(field))
                    })
            })
        {
            return invalid(program, "return realization is incomplete");
        }
    }
    for input in &program.request_inputs {
        let declared = input
            .fields
            .iter()
            .map(|field| &field.field_id)
            .collect::<Vec<_>>();
        if input.maximum_rows == 0
            || input.minimum_rows > input.maximum_rows
            || schemas
                .get(&input.relation_id)
                .is_none_or(|schema| schema.iter().ne(declared))
            || !program.operators.iter().any(|node| {
                matches!(&node.operator, ProgramRelationalOperator::Input { relation_id } if relation_id == &input.relation_id)
            })
        {
            return invalid(program, "request-input contract is not consumed exactly");
        }
    }
    for slot in &program.consumer_slots {
        if slot.maximum_edges == 0
            || slot.minimum_edges > slot.maximum_edges
            || !schemas.contains_key(&slot.input_relation_id)
            || !program.operators.iter().any(|node| {
                matches!(&node.operator, ProgramRelationalOperator::Input { relation_id } if relation_id == &slot.input_relation_id)
            })
        {
            return invalid(program, "consumer-slot contract is not consumed exactly");
        }
    }
    Ok(())
}

fn invalid<T>(
    program: &ProductionSemanticFormProgram,
    detail: &str,
) -> Result<T, ProductionQueryRecipeError> {
    Err(ProductionQueryRecipeError::InvalidProgram {
        program: program.program_binding_id.to_string(),
        detail: detail.to_owned(),
    })
}

fn append_selections(
    definition: &ProductionSemanticFormProgram,
    execution_program_pin: [u8; 32],
    ingress: &mut EpochBoundSemanticIngressCatalog,
    execution: &mut EpochBoundSemanticExecutionCatalog,
) {
    for selection in &definition.selections {
        ingress.selections.push(EpochBoundSelectionBindingRow {
            program_binding_id: Arc::clone(&definition.program_binding_id),
            selection_id: Arc::clone(&selection.selection_id),
            value_kind: selection.value_kind,
            minimum_values: selection.minimum_values,
            maximum_values: selection.maximum_values,
        });
        execution.selections.push(EpochBoundExecutionSelectionRow {
            program_binding_id: Arc::clone(&definition.program_binding_id),
            execution_program_pin,
            selection_id: Arc::clone(&selection.selection_id),
            operator_node_id: Arc::clone(&selection.operator_node_id),
            input_field_id: selection.input_field_id.clone(),
            scalar_operator: selection.scalar_operator,
            fold: selection.fold,
        });
    }
}

fn append_returns(
    definition: &ProductionSemanticFormProgram,
    execution_program_pin: [u8; 32],
    ingress: &mut EpochBoundSemanticIngressCatalog,
    execution: &mut EpochBoundSemanticExecutionCatalog,
) {
    for return_definition in &definition.returns {
        ingress.returns.push(EpochBoundReturnBindingRow {
            program_binding_id: Arc::clone(&definition.program_binding_id),
            return_id: Arc::clone(&return_definition.return_id),
            value_kind: return_definition.value_kind,
            minimum_values: return_definition.minimum_values,
            maximum_values: return_definition.maximum_values,
        });
        for realization in &return_definition.realizations {
            execution.returns.push(EpochBoundExecutionReturnRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                execution_program_pin,
                return_id: Arc::clone(&return_definition.return_id),
                value: realization.value.clone(),
                realization_node_id: Arc::clone(&realization.realization_node_id),
                realization_field_ids: realization.realization_field_ids.clone(),
                realization_pin: typed_identity_pin(
                    b"codefabric.semantic-return-realization.v1",
                    &format!("{realization:?}"),
                ),
            });
        }
    }
}

fn append_request_inputs(
    definition: &ProductionSemanticFormProgram,
    execution_program_pin: [u8; 32],
    ingress: &mut EpochBoundSemanticIngressCatalog,
    execution: &mut EpochBoundSemanticExecutionCatalog,
) {
    for input in &definition.request_inputs {
        ingress
            .request_inputs
            .push(EpochBoundRequestInputBindingRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                input_id: Arc::clone(&input.input_id),
                input_relation_id: input.relation_id.clone(),
                fields: input.fields.clone(),
                minimum_rows: input.minimum_rows,
                maximum_rows: input.maximum_rows,
            });
        execution
            .request_inputs
            .push(EpochBoundExecutionRequestInputRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                execution_program_pin,
                input_id: Arc::clone(&input.input_id),
                input_relation_id: input.relation_id.clone(),
                fields: input.fields.clone(),
                handoff_pin: typed_identity_pin(
                    b"codefabric.semantic-request-input-handoff.v1",
                    &format!("{input:?}"),
                ),
            });
    }
}

fn append_consumer_slots(
    definition: &ProductionSemanticFormProgram,
    execution_program_pin: [u8; 32],
    ingress: &mut EpochBoundSemanticIngressCatalog,
    execution: &mut EpochBoundSemanticExecutionCatalog,
) {
    for slot in &definition.consumer_slots {
        ingress
            .consumer_slots
            .push(EpochBoundConsumerSlotBindingRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                consumer_slot_id: Arc::clone(&slot.consumer_slot_id),
                consumer_role_id: Arc::clone(&slot.consumer_role_id),
                minimum_edges: slot.minimum_edges,
                maximum_edges: slot.maximum_edges,
            });
        execution
            .consumer_slots
            .push(EpochBoundExecutionConsumerSlotRow {
                program_binding_id: Arc::clone(&definition.program_binding_id),
                execution_program_pin,
                consumer_slot_id: Arc::clone(&slot.consumer_slot_id),
                consumer_role_id: Arc::clone(&slot.consumer_role_id),
                input_relation_id: slot.input_relation_id.clone(),
                composition: slot.composition,
            });
    }
}

fn append_scopes(
    scopes: Vec<ProductionScopeDefinition>,
    ingress: &mut EpochBoundSemanticIngressCatalog,
    execution: &mut EpochBoundSemanticExecutionCatalog,
    catalog_pin: [u8; 32],
) -> Result<(), ProductionQueryRecipeError> {
    let mut seen = BTreeSet::new();
    for scope in scopes {
        validate_identity("scope", &scope.scope_id)?;
        validate_identity("scope authorization input", &scope.authorization_input_id)?;
        if scope.maximum_values == 0
            || scope.minimum_values > scope.maximum_values
            || !seen.insert(Arc::clone(&scope.scope_id))
        {
            return Err(ProductionQueryRecipeError::InvalidIdentity {
                kind: "scope cardinality or duplicate",
                value: scope.scope_id.to_string(),
            });
        }
        ingress.scopes.push(EpochBoundScopeBindingRow {
            scope_id: Arc::clone(&scope.scope_id),
            value_kind: scope.value_kind,
            minimum_values: scope.minimum_values,
            maximum_values: scope.maximum_values,
        });
        execution.scopes.push(EpochBoundExecutionScopeRow {
            scope_id: Arc::clone(&scope.scope_id),
            authorization_input_id: Arc::clone(&scope.authorization_input_id),
            handoff_pin: typed_identity_pin(
                b"codefabric.semantic-scope-handoff.v1",
                &format!("{catalog_pin:?}:{scope:?}"),
            ),
        });
    }
    Ok(())
}

fn validate_required_closure(
    forms: &BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>,
    closure: &ProducerClosureProof,
) -> Result<(), ProductionQueryRecipeError> {
    let available = closure
        .families
        .iter()
        .map(|row| Arc::clone(&row.family_id))
        .collect::<BTreeSet<_>>();
    for program in forms.values() {
        for family in &program.required_fact_families {
            if !available.contains(family) {
                return Err(ProductionQueryRecipeError::MissingProducerFamily {
                    program: program.program_binding_id.to_string(),
                    family: family.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn decode_executed_closure(
    epoch: &ProgrammaticFabricEpoch,
    execution: &DerivedProducerClosureExecution,
    fields: &FamilyClosureFields,
    factual_semantic_class_id: &Arc<str>,
) -> Result<ProducerClosureProof, ProductionQueryRecipeError> {
    if !execution.is_conformant() {
        return Err(ProductionQueryRecipeError::ProducerClosureViolations);
    }
    let application_authority_id =
        validate_closure_dependencies(epoch, execution, factual_semantic_class_id)?;
    decode_closure_batches(
        execution.family_closure(),
        fields,
        factual_semantic_class_id,
        execution.observation().operation_id(),
        &application_authority_id,
    )
}

fn validate_closure_dependencies(
    epoch: &ProgrammaticFabricEpoch,
    execution: &DerivedProducerClosureExecution,
    factual_semantic_class_id: &Arc<str>,
) -> Result<Arc<str>, ProductionQueryRecipeError> {
    let mut input_relations = BTreeSet::new();
    let mut input_fields = BTreeSet::new();
    let mut authorities = BTreeSet::new();
    let mut observed_semantic_class = false;
    for dependency in execution.observation().dependencies() {
        match dependency {
            ProducerClosureCompilationDependency::InputRelation(relation_id) => {
                let relation = epoch
                    .relation(&ProgrammaticRelationId::new(relation_id.as_str()))
                    .ok_or_else(|| {
                        ProductionQueryRecipeError::ProducerClosureSchema(format!(
                            "compiler input relation {} is absent from the sealed epoch",
                            relation_id.as_str()
                        ))
                    })?;
                input_relations.insert(relation_id.clone());
                for index in 0..relation.contract.logical_schema().fields().len() {
                    let field_id = relation
                        .contract
                        .field_id_at(SchemaRole::Logical, index)
                        .map_err(|error| {
                            ProductionQueryRecipeError::ProducerClosureSchema(error.to_string())
                        })?;
                    input_fields.insert(FieldId::new(field_id).map_err(|error| {
                        ProductionQueryRecipeError::ProducerClosureSchema(error.to_string())
                    })?);
                }
            }
            ProducerClosureCompilationDependency::InputField(field_id) => {
                if !input_fields.contains(field_id) {
                    return Err(ProductionQueryRecipeError::ProducerClosureSchema(format!(
                        "compiler input field {} is absent from its sealed epoch contracts",
                        field_id.as_str()
                    )));
                }
            }
            ProducerClosureCompilationDependency::ApplicationOwnedAuthority(authority) => {
                authorities.insert(Arc::clone(authority));
            }
            ProducerClosureCompilationDependency::FactualSemanticClass(semantic_class) => {
                if semantic_class != factual_semantic_class_id {
                    return Err(ProductionQueryRecipeError::ProducerClosureSchema(
                        "compiler semantic class differs from the query recipe".to_owned(),
                    ));
                }
                observed_semantic_class = true;
            }
            _ => {}
        }
    }
    if input_relations.is_empty() || !observed_semantic_class || authorities.len() != 1 {
        return Err(ProductionQueryRecipeError::ProducerClosureSchema(
            "compiler observation lacks exact epoch inputs, factual class, or one authority"
                .to_owned(),
        ));
    }
    Ok(authorities
        .into_iter()
        .next()
        .expect("one observed application authority"))
}

fn decode_closure_batches(
    batches: &[RecordBatch],
    fields: &FamilyClosureFields,
    factual_semantic_class_id: &Arc<str>,
    operation_id: &Arc<str>,
    expected_authority: &Arc<str>,
) -> Result<ProducerClosureProof, ProductionQueryRecipeError> {
    let mut decoded = Vec::new();
    let mut row_number = 0usize;
    for batch in batches {
        let schema = batch.schema();
        let indices = ClosureIndices::resolve(schema.as_ref(), fields)?;
        for row in 0..batch.num_rows() {
            let family_id = required_text(batch, indices.family_id, row, row_number, "family")?;
            let semantic_class = required_text(
                batch,
                indices.semantic_class_id,
                row,
                row_number,
                "semantic class",
            )?;
            if semantic_class.as_ref() != factual_semantic_class_id.as_ref() {
                return closure_row_error(row_number, "semantic class is not factual");
            }
            let state = required_text(batch, indices.closure_state, row, row_number, "state")?;
            let disposition = match state.as_ref() {
                "supported" => {
                    let producer_authority = required_text(
                        batch,
                        indices.authority_id,
                        row,
                        row_number,
                        "producer authority",
                    )?;
                    validate_row_authority(expected_authority, &producer_authority, row_number)?;
                    ProducerFamilyDisposition::RuntimeProducer(RuntimeProducerProof {
                        producer_id: required_text(
                            batch,
                            indices.producer_id,
                            row,
                            row_number,
                            "producer",
                        )?,
                        authority_id: producer_authority,
                        algorithm_release: required_text(
                            batch,
                            indices.algorithm_release,
                            row,
                            row_number,
                            "algorithm release",
                        )?,
                        precision_id: required_text(
                            batch,
                            indices.precision_id,
                            row,
                            row_number,
                            "precision",
                        )?,
                        input_pin: text_pin(
                            batch,
                            indices.input_pin,
                            row,
                            row_number,
                            "input pin",
                        )?,
                        invalidation_pin: text_pin(
                            batch,
                            indices.invalidation_pin,
                            row,
                            row_number,
                            "invalidation pin",
                        )?,
                        materialization_pin: text_pin(
                            batch,
                            indices.materialization_pin,
                            row,
                            row_number,
                            "materialization pin",
                        )?,
                        requested_units: required_u64(
                            batch,
                            indices.requested_units,
                            row,
                            row_number,
                            "requested units",
                        )?,
                        completed_units: required_u64(
                            batch,
                            indices.completed_units,
                            row,
                            row_number,
                            "completed units",
                        )?,
                        remainder_units: required_u64(
                            batch,
                            indices.remainder_units,
                            row,
                            row_number,
                            "remainder units",
                        )?,
                        unknown_units: required_u64(
                            batch,
                            indices.unknown_units,
                            row,
                            row_number,
                            "unknown units",
                        )?,
                        completeness_proof_pin: text_pin(
                            batch,
                            indices.completeness_proof_pin,
                            row,
                            row_number,
                            "completeness proof identity",
                        )?,
                        producer_proof_pin: text_pin(
                            batch,
                            indices.producer_proof_pin,
                            row,
                            row_number,
                            "producer proof identity",
                        )?,
                    })
                }
                "unsupported" => {
                    let remainder_authority = required_text(
                        batch,
                        indices.authority_id,
                        row,
                        row_number,
                        "remainder authority",
                    )?;
                    validate_row_authority(expected_authority, &remainder_authority, row_number)?;
                    ProducerFamilyDisposition::UnsupportedRemainder(UnsupportedFamilyRemainder {
                        remainder_id: required_text(
                            batch,
                            indices.unsupported_remainder_id,
                            row,
                            row_number,
                            "unsupported remainder",
                        )?,
                        authority_id: remainder_authority,
                        reason_id: required_text(
                            batch,
                            indices.unsupported_reason_id,
                            row,
                            row_number,
                            "unsupported reason",
                        )?,
                        proof_pin: text_pin(
                            batch,
                            indices.unsupported_proof_pin,
                            row,
                            row_number,
                            "unsupported proof identity",
                        )?,
                    })
                }
                other => {
                    return closure_row_error(row_number, &format!("non-closed state {other:?}"));
                }
            };
            decoded.push(ProducerFamilyClosureRow {
                family_id,
                disposition,
            });
            row_number += 1;
        }
    }
    decoded.sort_by(|left, right| left.family_id.cmp(&right.family_id));
    if decoded
        .windows(2)
        .any(|pair| pair[0].family_id == pair[1].family_id)
    {
        return closure_row_error(0, "family has multiple closure dispositions");
    }
    let proof_pin = typed_identity_pin(
        b"codefabric.executed-producer-closure.v1",
        &format!("{operation_id:?}:{decoded:?}"),
    );
    Ok(ProducerClosureProof {
        proof_pin,
        application_authority_id: Arc::clone(expected_authority),
        families: decoded,
    })
}

struct ClosureIndices {
    family_id: usize,
    semantic_class_id: usize,
    closure_state: usize,
    producer_id: usize,
    authority_id: usize,
    algorithm_release: usize,
    precision_id: usize,
    input_pin: usize,
    invalidation_pin: usize,
    materialization_pin: usize,
    requested_units: usize,
    completed_units: usize,
    remainder_units: usize,
    unknown_units: usize,
    completeness_proof_pin: usize,
    producer_proof_pin: usize,
    unsupported_remainder_id: usize,
    unsupported_reason_id: usize,
    unsupported_proof_pin: usize,
}

impl ClosureIndices {
    fn resolve(
        schema: &arrow_schema::Schema,
        fields: &FamilyClosureFields,
    ) -> Result<Self, ProductionQueryRecipeError> {
        let index = |field: &FieldId| {
            schema.index_of(field.as_str()).map_err(|error| {
                ProductionQueryRecipeError::ProducerClosureSchema(error.to_string())
            })
        };
        Ok(Self {
            family_id: index(&fields.family_id)?,
            semantic_class_id: index(&fields.semantic_class_id)?,
            closure_state: index(&fields.closure_state)?,
            producer_id: index(&fields.producer_id)?,
            authority_id: index(&fields.authority_id)?,
            algorithm_release: index(&fields.algorithm_release)?,
            precision_id: index(&fields.precision_id)?,
            input_pin: index(&fields.input_pin)?,
            invalidation_pin: index(&fields.invalidation_pin)?,
            materialization_pin: index(&fields.materialization_pin)?,
            requested_units: index(&fields.requested_unit_count)?,
            completed_units: index(&fields.completed_unit_count)?,
            remainder_units: index(&fields.remainder_unit_count)?,
            unknown_units: index(&fields.unknown_unit_count)?,
            completeness_proof_pin: index(&fields.completeness_proof_pin)?,
            producer_proof_pin: index(&fields.producer_proof_pin)?,
            unsupported_remainder_id: index(&fields.unsupported_remainder_id)?,
            unsupported_reason_id: index(&fields.unsupported_reason_id)?,
            unsupported_proof_pin: index(&fields.unsupported_proof_pin)?,
        })
    }
}

fn required_text(
    batch: &RecordBatch,
    column: usize,
    row: usize,
    row_number: usize,
    role: &str,
) -> Result<Arc<str>, ProductionQueryRecipeError> {
    let values = batch
        .column(column)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| {
            ProductionQueryRecipeError::ProducerClosureSchema(format!("{role} is not Utf8"))
        })?;
    if values.is_null(row) {
        return closure_row_error(row_number, &format!("{role} is null"));
    }
    let value: Arc<str> = Arc::from(values.value(row));
    validate_identity("producer closure text", &value)?;
    Ok(value)
}

fn required_u64(
    batch: &RecordBatch,
    column: usize,
    row: usize,
    row_number: usize,
    role: &str,
) -> Result<u64, ProductionQueryRecipeError> {
    let values = batch
        .column(column)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| {
            ProductionQueryRecipeError::ProducerClosureSchema(format!("{role} is not UInt64"))
        })?;
    if values.is_null(row) {
        closure_row_error(row_number, &format!("{role} is null"))
    } else {
        Ok(values.value(row))
    }
}

fn text_pin(
    batch: &RecordBatch,
    column: usize,
    row: usize,
    row_number: usize,
    role: &str,
) -> Result<[u8; 32], ProductionQueryRecipeError> {
    let text = required_text(batch, column, row, row_number, role)?;
    Ok(typed_identity_pin(
        b"codefabric.producer-contract-identity.v1",
        &text,
    ))
}

fn validate_row_authority(
    expected: &Arc<str>,
    observed: &Arc<str>,
    row: usize,
) -> Result<(), ProductionQueryRecipeError> {
    if expected != observed {
        closure_row_error(row, "row authority differs from compiler authority")
    } else {
        Ok(())
    }
}

fn closure_row_error<T>(row: usize, detail: &str) -> Result<T, ProductionQueryRecipeError> {
    Err(ProductionQueryRecipeError::ProducerClosureRow {
        row,
        detail: detail.to_owned(),
    })
}

fn program_identity_pin(program: &ProductionSemanticFormProgram) -> [u8; 32] {
    typed_identity_pin(
        b"codefabric.production-semantic-program.v1",
        &format!("{program:?}"),
    )
}

fn binding_identity_pin(
    program: &ProductionSemanticFormProgram,
    execution_pin: [u8; 32],
) -> [u8; 32] {
    typed_identity_pin(
        b"codefabric.production-semantic-binding.v1",
        &format!(
            "{:?}:{:?}:{:?}:{execution_pin:?}",
            program.form, program.program_binding_id, program.output_role_id
        ),
    )
}

fn catalog_identity_pin(
    domain: &[u8],
    epoch_pin: [u8; 32],
    source_pin: [u8; 32],
    policy_pin: [u8; 32],
    release_pin: [u8; 32],
    forms: &BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>,
) -> [u8; 32] {
    typed_identity_pin(
        domain,
        &format!("{epoch_pin:?}:{source_pin:?}:{policy_pin:?}:{release_pin:?}:{forms:?}"),
    )
}

fn typed_identity_pin(domain: &[u8], value: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{Int64Array, StringArray, UInt64Array};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;
    use datafusion::datasource::MemTable;

    use super::*;
    use crate::fabric::epoch_runtime::{
        FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole,
    };
    use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
    use crate::fabric::programmatic_schema::ProviderInput;
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
    };

    fn relation(value: impl Into<String>) -> RelationId {
        RelationId::new(value).expect("valid test relation")
    }

    fn field(value: impl Into<String>) -> FieldId {
        FieldId::new(value).expect("valid test field")
    }

    async fn epoch() -> ProgrammaticFabricEpoch {
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([0x61; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .expect("epoch builder");
        for index in 0..ReleasedSemanticForm::ALL.len() {
            let relation_id = format!("fact.production-query.{index}");
            let field_id = format!("fact.production-query.{index}.identity");
            let schema = Arc::new(
                Schema::new(vec![
                    Field::new("identity", DataType::Int64, false).with_metadata(HashMap::from([
                        (FIELD_ID_METADATA_KEY.to_owned(), field_id),
                    ])),
                ])
                .with_metadata(HashMap::from([(
                    RELATION_ID_METADATA_KEY.to_owned(),
                    relation_id.clone(),
                )])),
            );
            let batch = RecordBatch::try_new(
                Arc::clone(&schema),
                vec![Arc::new(Int64Array::from(vec![
                    i64::try_from(index).expect("small test index"),
                ]))],
            )
            .expect("batch");
            let provider = Arc::new(
                MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).expect("provider"),
            );
            let table_reference = TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Fact.as_str(),
                format!("production_query_{index}"),
            );
            let contract = Arc::new(
                SchemaContract::try_new(
                    format!("test:production-query:{index}"),
                    table_reference.clone(),
                    Arc::clone(&schema),
                    schema,
                    vec![FieldIndexMapping::direct(0, 0)],
                )
                .expect("schema contract"),
            );
            builder
                .register_provider(ProviderInput::new(
                    ProgrammaticRelationId::new(relation_id),
                    table_reference,
                    contract,
                    provider,
                ))
                .expect("provider registration");
        }
        builder.seal_for_test().await.expect("sealed epoch")
    }

    fn form_programs() -> Vec<ProductionSemanticFormProgram> {
        ReleasedSemanticForm::ALL
            .into_iter()
            .enumerate()
            .map(|(index, form)| {
                let relation_id = relation(format!("fact.production-query.{index}"));
                let field_id = field(format!("fact.production-query.{index}.identity"));
                let node_id: Arc<str> = Arc::from(format!("node.production-query.{index}.input"));
                ProductionSemanticFormProgram {
                    form,
                    program_binding_id: Arc::from(format!(
                        "program.production-query.{}",
                        form.label().replace(' ', "-")
                    )),
                    output_role_id: Arc::from(format!("role.production-query.{index}")),
                    root_node_id: Arc::clone(&node_id),
                    output_relation_id: relation_id.clone(),
                    output_fields: vec![field_id.clone()],
                    relations: vec![ProductionRelationDefinition {
                        relation_id: relation_id.clone(),
                        fields: vec![field_id.clone()],
                        authority: ProductionRelationAuthority::Epoch,
                    }],
                    operators: vec![ProductionOperatorDefinition {
                        node_id,
                        ordinal: 0,
                        input_node_ids: Vec::new(),
                        operator: ProgramRelationalOperator::Input { relation_id },
                        output_fields: vec![field_id],
                    }],
                    selections: Vec::new(),
                    returns: Vec::new(),
                    request_inputs: Vec::new(),
                    consumer_slots: Vec::new(),
                    required_fact_families: vec![Arc::from("family.core")],
                }
            })
            .collect()
    }

    fn limits() -> EpochBoundSemanticIngressLimits {
        use crate::relational_semantic_query::SemanticRequestLimits;

        EpochBoundSemanticIngressLimits::try_new(
            SemanticRequestLimits::try_new(16, 64, 16, 16, 32, 32, 10_000)
                .expect("compiler limits"),
            128,
            128,
            64,
            256,
            32,
        )
        .expect("ingress limits")
    }

    fn input(forms: Vec<ProductionSemanticFormProgram>) -> ProductionSemanticQueryRecipeInput {
        ProductionSemanticQueryRecipeInput {
            source_pin: [0x11; 32],
            policy_pin: [0x12; 32],
            program_release_pin: [0x13; 32],
            factual_semantic_class_id: Arc::from("semantic.fact"),
            limits: limits(),
            scopes: Vec::new(),
            forms,
        }
    }

    fn closure() -> ProducerClosureProof {
        ProducerClosureProof {
            proof_pin: [0x14; 32],
            application_authority_id: Arc::from("authority.application"),
            families: vec![ProducerFamilyClosureRow {
                family_id: Arc::from("family.core"),
                disposition: ProducerFamilyDisposition::RuntimeProducer(RuntimeProducerProof {
                    producer_id: Arc::from("producer.core"),
                    authority_id: Arc::from("authority.application"),
                    algorithm_release: Arc::from("algorithm.core.v1"),
                    precision_id: Arc::from("precision.exact"),
                    input_pin: [0x21; 32],
                    invalidation_pin: [0x22; 32],
                    materialization_pin: [0x23; 32],
                    requested_units: 1,
                    completed_units: 1,
                    remainder_units: 0,
                    unknown_units: 0,
                    completeness_proof_pin: [0x24; 32],
                    producer_proof_pin: [0x25; 32],
                }),
            }],
        }
    }

    #[tokio::test]
    async fn all_eight_forms_are_built_from_epoch_checked_programs() {
        let epoch = epoch().await;
        let recipe =
            ProductionSemanticQueryRecipe::assemble(&epoch, input(form_programs()), closure())
                .expect("all eight programs");
        assert_eq!(
            recipe.ingress_catalog().program_bindings.len(),
            ReleasedSemanticForm::ALL.len()
        );
        assert_eq!(
            recipe.execution_catalog().programs.len(),
            ReleasedSemanticForm::ALL.len()
        );

        use crate::relational_semantic_query::{
            EpochBoundBlockBindingRow, EpochBoundSemanticIngress,
            compile_epoch_bound_semantic_request, validate_epoch_bound_semantic_ingress,
        };
        let catalog = recipe.ingress_catalog();
        let limits = limits();
        let blocks = catalog
            .program_bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| EpochBoundBlockBindingRow {
                query_id: Arc::from(format!("query.production.{index}")),
                compatibility_form: binding.compatibility_form,
                program_binding_id: Arc::clone(&binding.program_binding_id),
                program_binding_pin: binding.program_binding_pin,
                output_role_id: Arc::clone(&binding.output_role_id),
                explicit_result_limit: None,
            })
            .collect::<Vec<_>>();
        let dependency_order = blocks
            .iter()
            .map(|block| Arc::clone(&block.query_id))
            .collect();
        let request = EpochBoundSemanticIngress {
            semantic_request_id: Arc::from("request.production.all-eight"),
            request_content_pin: [0x31; 32],
            fabric_epoch_pin: catalog.fabric_epoch_pin,
            program_catalog_pin: catalog.program_catalog_pin,
            source_pin: catalog.source_pin,
            policy_pin: catalog.policy_pin,
            producer_closure_proof_pin: catalog.producer_closure_proof_pin,
            limits_pin: catalog.limits_pin,
            limits,
            blocks,
            selections: Vec::new(),
            returns: Vec::new(),
            scopes: Vec::new(),
            request_inputs: Vec::new(),
            dependencies: Vec::new(),
            dependency_order,
        };
        let validated = validate_epoch_bound_semantic_ingress(request, catalog)
            .expect("recipe ingress validates");
        let compiled = compile_epoch_bound_semantic_request(
            &validated,
            recipe.execution_catalog(),
            recipe.producer_closure(),
        )
        .expect("recipe execution catalog compiles all forms");
        assert_eq!(
            compiled.compiled().blocks().len(),
            ReleasedSemanticForm::ALL.len()
        );
    }

    #[tokio::test]
    async fn typed_program_input_is_causal_to_catalog_identity() {
        let epoch = epoch().await;
        let first =
            ProductionSemanticQueryRecipe::assemble(&epoch, input(form_programs()), closure())
                .expect("first recipe");
        let mut changed = form_programs();
        changed[0].output_role_id = Arc::from("role.production-query.changed");
        let second = ProductionSemanticQueryRecipe::assemble(&epoch, input(changed), closure())
            .expect("changed recipe");
        assert_ne!(
            first.ingress_catalog().program_catalog_pin,
            second.ingress_catalog().program_catalog_pin
        );
    }

    #[tokio::test]
    async fn missing_epoch_relation_and_field_drift_are_rejected() {
        let epoch = epoch().await;
        let mut missing = form_programs();
        missing[0].relations[0].relation_id = relation("fact.absent");
        assert!(matches!(
            ProductionSemanticQueryRecipe::assemble(&epoch, input(missing), closure()),
            Err(ProductionQueryRecipeError::MissingEpochRelation { .. })
        ));

        let mut drifted = form_programs();
        drifted[0].relations[0].fields[0] = field("fact.wrong-field");
        assert!(matches!(
            ProductionSemanticQueryRecipe::assemble(&epoch, input(drifted), closure()),
            Err(ProductionQueryRecipeError::EpochFieldDrift { .. })
        ));
    }

    fn closure_fields() -> FamilyClosureFields {
        FamilyClosureFields {
            family_id: field("family"),
            semantic_class_id: field("semantic_class"),
            closure_state: field("state"),
            producer_id: field("producer"),
            authority_id: field("authority"),
            algorithm_release: field("algorithm"),
            precision_id: field("precision"),
            input_pin: field("input_pin"),
            invalidation_pin: field("invalidation_pin"),
            materialization_pin: field("materialization_pin"),
            requested_unit_count: field("requested"),
            completed_unit_count: field("completed"),
            remainder_unit_count: field("remainder"),
            unknown_unit_count: field("unknown"),
            completeness_proof_pin: field("completeness_pin"),
            producer_proof_pin: field("producer_pin"),
            unsupported_remainder_id: field("unsupported_id"),
            unsupported_reason_id: field("unsupported_reason"),
            unsupported_proof_pin: field("unsupported_pin"),
        }
    }

    #[test]
    fn non_closed_executed_row_is_rejected_as_a_closure_violation() {
        let fields = closure_fields();
        let string_field = |name: &str, nullable| Field::new(name, DataType::Utf8, nullable);
        let schema = Arc::new(Schema::new(vec![
            string_field(fields.family_id.as_str(), false),
            string_field(fields.semantic_class_id.as_str(), false),
            string_field(fields.closure_state.as_str(), false),
            string_field(fields.producer_id.as_str(), true),
            string_field(fields.authority_id.as_str(), true),
            string_field(fields.algorithm_release.as_str(), true),
            string_field(fields.precision_id.as_str(), true),
            string_field(fields.input_pin.as_str(), true),
            string_field(fields.invalidation_pin.as_str(), true),
            string_field(fields.materialization_pin.as_str(), true),
            Field::new(fields.requested_unit_count.as_str(), DataType::UInt64, true),
            Field::new(fields.completed_unit_count.as_str(), DataType::UInt64, true),
            Field::new(fields.remainder_unit_count.as_str(), DataType::UInt64, true),
            Field::new(fields.unknown_unit_count.as_str(), DataType::UInt64, true),
            string_field(fields.completeness_proof_pin.as_str(), true),
            string_field(fields.producer_proof_pin.as_str(), true),
            string_field(fields.unsupported_remainder_id.as_str(), true),
            string_field(fields.unsupported_reason_id.as_str(), true),
            string_field(fields.unsupported_proof_pin.as_str(), true),
        ]));
        let text = |value: Option<&str>| Arc::new(StringArray::from(vec![value])) as _;
        let count = |value: Option<u64>| Arc::new(UInt64Array::from(vec![value])) as _;
        let batch = RecordBatch::try_new(
            schema,
            vec![
                text(Some("family.core")),
                text(Some("semantic.fact")),
                text(Some("unknown")),
                text(None),
                text(None),
                text(None),
                text(None),
                text(None),
                text(None),
                text(None),
                count(None),
                count(None),
                count(None),
                count(None),
                text(None),
                text(None),
                text(None),
                text(None),
                text(None),
            ],
        )
        .expect("closure batch");
        assert!(matches!(
            decode_closure_batches(
                &[batch],
                &fields,
                &Arc::from("semantic.fact"),
                &Arc::from("operation.test"),
                &Arc::from("authority.application"),
            ),
            Err(ProductionQueryRecipeError::ProducerClosureRow { .. })
        ));
    }
}
