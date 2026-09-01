//! Production construction of the released semantic-query program catalogs.
//!
//! This module is an application-owned composition boundary.  The compiled release privately
//! constructs the complete eight-form program and scope set, checks every epoch-owned relation
//! and field against the exact sealed [`ProgrammaticFabricEpoch`], and emits the two catalogs
//! consumed by the programmatic query ports.  Callers may vary only source, policy, and resource
//! inputs; they cannot supply a serialized semantic manifest, program, scope, catalog, or release
//! pin.  Pins emitted here use explicit typed framing. Semantic validity comes from exact
//! relation/field checks and executed producer-closure proof, never from a digest alone.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray, UInt64Array};

use crate::fabric::derived_producer_closure::{
    DerivedProducerClosureExecution, FamilyClosureFields, ProducerClosureCompilationDependency,
};
use crate::fabric::production_kernel::CompiledQueryAuthority;
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpoch;
use crate::fabric::programmatic_schema::ProgrammaticRelationId;
use crate::fabric::programmatic_workspace::programmatic_fabric_epoch_authority_pin;
use crate::relational_program::{
    AggregateOperator, FieldId, JoinKind, RelationId, ScalarOperator, UnionKind,
};
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
use crate::semantic_query_contract::COMPILED_V2_0_SCOPE_DEFINITIONS;

const PRODUCTION_SEMANTIC_QUERY_RELEASE_ID: &str =
    "codefabric.semantic-query.release.v2.2.0:datafusion=55.0.0:arrow=59.2.0";
const RELEASE_FACTUAL_SEMANTIC_CLASS_ID: &str = "semantic.fact.v2";
const RELEASE_SELECTION_MAXIMUM_VALUES: usize = 64;

/// Whether a relation is owned by the sealed epoch or exists only inside one compiled request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductionRelationAuthority {
    /// The exact relation and field sequence must exist in the sealed epoch.
    Epoch,
    /// The relation is supplied through a validated request-input or prior-result handoff.
    QueryLocal,
    /// The relation names the result schema of this application-owned program.
    ProgramResult,
}

/// One relation schema referenced by a typed production program.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionRelationDefinition {
    relation_id: RelationId,
    fields: Vec<FieldId>,
    authority: ProductionRelationAuthority,
}

/// One operator node.  Its program and execution pins are derived by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionOperatorDefinition {
    node_id: Arc<str>,
    ordinal: u32,
    input_node_ids: Vec<Arc<str>>,
    operator: ProgramRelationalOperator,
    output_fields: Vec<FieldId>,
}

/// Ingress and execution realization for one repeatable selection.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionSelectionDefinition {
    selection_id: Arc<str>,
    value_kind: SemanticValueKind,
    minimum_values: usize,
    maximum_values: usize,
    operator_node_id: Arc<str>,
    input_field_id: FieldId,
    scalar_operator: ScalarOperator,
    fold: EpochBoundSelectionFold,
}

/// One exact return value and its programmatic realization.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionReturnRealization {
    value: SemanticClauseValue,
    realization_node_id: Arc<str>,
    realization_field_ids: Vec<FieldId>,
}

/// Ingress contract and finite execution realizations for one return directive.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionReturnDefinition {
    return_id: Arc<str>,
    value_kind: SemanticValueKind,
    minimum_values: usize,
    maximum_values: usize,
    realizations: Vec<ProductionReturnRealization>,
}

/// One request-owned relation consumed by a program.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionRequestInputDefinition {
    input_id: Arc<str>,
    relation_id: RelationId,
    fields: Vec<EpochBoundRequestInputField>,
    minimum_rows: usize,
    maximum_rows: usize,
}

/// One prior-result consumer slot consumed by a program.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionConsumerSlotDefinition {
    consumer_slot_id: Arc<str>,
    consumer_role_id: Arc<str>,
    input_relation_id: RelationId,
    minimum_edges: usize,
    maximum_edges: usize,
    composition: EpochBoundConsumerComposition,
}

/// Complete compiled-Rust definition of one released form.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionSemanticFormProgram {
    form: ReleasedSemanticForm,
    program_binding_id: Arc<str>,
    output_role_id: Arc<str>,
    root_node_id: Arc<str>,
    output_relation_id: RelationId,
    output_fields: Vec<FieldId>,
    relations: Vec<ProductionRelationDefinition>,
    operators: Vec<ProductionOperatorDefinition>,
    selections: Vec<ProductionSelectionDefinition>,
    returns: Vec<ProductionReturnDefinition>,
    request_inputs: Vec<ProductionRequestInputDefinition>,
    consumer_slots: Vec<ProductionConsumerSlotDefinition>,
    required_fact_families: Vec<Arc<str>>,
}

/// Request-global scope contract and its child-authorization handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionScopeDefinition {
    scope_id: Arc<str>,
    value_kind: SemanticValueKind,
    minimum_values: usize,
    maximum_values: usize,
    authorization_input_id: Arc<str>,
}

/// Explicit application inputs that are not derivable from the sealed epoch.
#[derive(Clone, Debug)]
pub struct ProductionSemanticQueryRecipeInput {
    source_pin: [u8; 32],
    policy_pin: [u8; 32],
    limits: EpochBoundSemanticIngressLimits,
}

impl ProductionSemanticQueryRecipeInput {
    /// Construct the complete set of caller-variable query-recipe inputs.
    ///
    /// The release program, semantic class, scopes, relation contracts, and program operands are
    /// intentionally absent from this interface.
    ///
    /// # Errors
    ///
    /// Rejects an absent source or policy authority pin.
    pub fn try_new(
        source_pin: [u8; 32],
        policy_pin: [u8; 32],
        limits: EpochBoundSemanticIngressLimits,
    ) -> Result<Self, ProductionQueryRecipeError> {
        validate_pin("source", source_pin)?;
        validate_pin("policy", policy_pin)?;
        Ok(Self {
            source_pin,
            policy_pin,
            limits,
        })
    }
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
    pub(crate) fn try_from_executed_closure(
        compiled_release: &CompiledQueryAuthority,
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
        closure_execution: &DerivedProducerClosureExecution,
    ) -> Result<Self, ProductionQueryRecipeError> {
        let producer_closure = decode_executed_closure(
            epoch,
            closure_execution,
            closure_execution.family_closure_fields(),
            &Arc::from(RELEASE_FACTUAL_SEMANTIC_CLASS_ID),
        )?;
        Self::assemble(compiled_release, epoch, input, producer_closure)
    }

    fn assemble(
        _compiled_release: &CompiledQueryAuthority,
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
        producer_closure: ProducerClosureProof,
    ) -> Result<Self, ProductionQueryRecipeError> {
        validate_pin("source", input.source_pin)?;
        validate_pin("policy", input.policy_pin)?;
        validate_identity("factual semantic class", RELEASE_FACTUAL_SEMANTIC_CLASS_ID)?;
        let forms = compiled_released_form_programs()?;
        let scopes = compiled_release_scopes();
        let program_release_pin = compiled_release_identity_pin(&forms, &scopes);
        validate_pin("program release", program_release_pin)?;
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
            program_release_pin,
            producer_closure.proof_pin,
            &forms,
            &scopes,
        );
        let execution_catalog_pin = catalog_identity_pin(
            b"codefabric.production-semantic-execution-catalog.v1",
            fabric_epoch_pin,
            input.source_pin,
            input.policy_pin,
            program_release_pin,
            producer_closure.proof_pin,
            &forms,
            &scopes,
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
            program_release_pin,
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::clone(
                &producer_closure.application_authority_id,
            )),
            semantic_class: SemanticQueryClass::Fact(Arc::from(RELEASE_FACTUAL_SEMANTIC_CLASS_ID)),
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
            scopes,
            &mut ingress,
            &mut execution,
            program_catalog_pin,
            input.limits,
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

#[derive(Clone, Copy)]
struct ReleasedEpochFormSpec {
    program_binding_id: &'static str,
    output_role_id: &'static str,
    relation_id: &'static str,
    field_id: &'static str,
    selection_id: &'static str,
    required_fact_family: &'static str,
}

fn compiled_released_form_programs()
-> Result<BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>, ProductionQueryRecipeError>
{
    let programs = ReleasedSemanticForm::ALL
        .into_iter()
        .map(compiled_released_form_program)
        .collect::<Result<Vec<_>, _>>()?;
    validate_form_coverage(programs)
}

/// Return the exact query-family edges owned by the compiled eight-form release.
///
/// This is consumed while constructing the candidate's producer-closure relations. The private
/// query authority prevents a caller from substituting a form or family edge, while the returned
/// rows remain ordinary typed values that DataFusion later executes and proves.
pub(crate) fn released_query_family_requirements(
    _authority: &CompiledQueryAuthority,
) -> Result<Vec<(Arc<str>, Arc<str>)>, ProductionQueryRecipeError> {
    Ok(compiled_released_form_programs()?
        .into_values()
        .flat_map(|program| {
            let query_family = program.program_binding_id;
            program
                .required_fact_families
                .into_iter()
                .map(move |family| (Arc::clone(&query_family), family))
        })
        .collect())
}

fn compiled_released_form_program(
    form: ReleasedSemanticForm,
) -> Result<ProductionSemanticFormProgram, ProductionQueryRecipeError> {
    match form {
        ReleasedSemanticForm::FindCodeEntities => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.find-code-entities.v2",
                output_role_id: "result.semantic-entities",
                relation_id: "public.semantic_entity",
                field_id: "public.semantic_entity.record_id",
                selection_id: "looking_for",
                required_fact_family: "fact-family.semantic-entity",
            },
        ),
        ReleasedSemanticForm::RetrieveFactsAboutCode => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.retrieve-facts-about-code.v2",
                output_role_id: "result.semantic-facts",
                relation_id: "public.semantic_fact",
                field_id: "public.semantic_fact.record_id",
                selection_id: "about",
                required_fact_family: "fact-family.semantic-fact",
            },
        ),
        ReleasedSemanticForm::FollowCodeRelationships => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.follow-code-relationships.v2",
                output_role_id: "result.semantic-relationships",
                relation_id: "public.semantic_relationship",
                field_id: "public.semantic_relationship.record_id",
                selection_id: "starting_from",
                required_fact_family: "fact-family.semantic-relationship",
            },
        ),
        ReleasedSemanticForm::FindConnectingFactPaths => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.find-connecting-fact-paths.v2",
                output_role_id: "result.semantic-fact-paths",
                relation_id: "public.semantic_fact_path",
                field_id: "public.semantic_fact_path.record_id",
                selection_id: "from",
                required_fact_family: "fact-family.semantic-path",
            },
        ),
        ReleasedSemanticForm::MatchCodeFactPattern => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.match-code-fact-pattern.v2",
                output_role_id: "result.semantic-pattern-matches",
                relation_id: "public.semantic_pattern_match",
                field_id: "public.semantic_pattern_match.record_id",
                selection_id: "pattern",
                required_fact_family: "fact-family.semantic-pattern",
            },
        ),
        ReleasedSemanticForm::CombineResultSets => compiled_combine_result_sets_program(),
        ReleasedSemanticForm::SummarizeObjectiveFacts => {
            compiled_summarize_objective_facts_program()
        }
        ReleasedSemanticForm::RetrieveSourceAndSyntaxContext => compiled_epoch_filter_program(
            form,
            ReleasedEpochFormSpec {
                program_binding_id: "program.semantic-query.retrieve-source-syntax-context.v2",
                output_role_id: "result.source-syntax-contexts",
                relation_id: "public.source_syntax_context",
                field_id: "public.source_syntax_context.record_id",
                selection_id: "about",
                required_fact_family: "fact-family.source-context",
            },
        ),
    }
}

fn compiled_epoch_filter_program(
    form: ReleasedSemanticForm,
    spec: ReleasedEpochFormSpec,
) -> Result<ProductionSemanticFormProgram, ProductionQueryRecipeError> {
    let relation_id = release_relation_id(spec.relation_id)?;
    let field_id = release_field_id(spec.field_id)?;
    let input_node_id: Arc<str> = Arc::from(format!("{}.input", spec.program_binding_id));
    let filter_node_id: Arc<str> = Arc::from(format!("{}.filter", spec.program_binding_id));
    Ok(ProductionSemanticFormProgram {
        form,
        program_binding_id: Arc::from(spec.program_binding_id),
        output_role_id: Arc::from(spec.output_role_id),
        root_node_id: Arc::clone(&filter_node_id),
        output_relation_id: relation_id.clone(),
        output_fields: vec![field_id.clone()],
        relations: vec![ProductionRelationDefinition {
            relation_id: relation_id.clone(),
            fields: vec![field_id.clone()],
            authority: ProductionRelationAuthority::Epoch,
        }],
        operators: vec![
            ProductionOperatorDefinition {
                node_id: Arc::clone(&input_node_id),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input { relation_id },
                output_fields: vec![field_id.clone()],
            },
            ProductionOperatorDefinition {
                node_id: Arc::clone(&filter_node_id),
                ordinal: 1,
                input_node_ids: vec![input_node_id],
                operator: ProgramRelationalOperator::Filter,
                output_fields: vec![field_id.clone()],
            },
        ],
        selections: vec![ProductionSelectionDefinition {
            selection_id: Arc::from(spec.selection_id),
            value_kind: SemanticValueKind::Text,
            minimum_values: 0,
            maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
            operator_node_id: Arc::clone(&filter_node_id),
            input_field_id: field_id.clone(),
            scalar_operator: ScalarOperator::Equal,
            fold: EpochBoundSelectionFold::Any,
        }],
        returns: vec![identity_return(filter_node_id, field_id)],
        request_inputs: Vec::new(),
        consumer_slots: Vec::new(),
        required_fact_families: vec![Arc::from(spec.required_fact_family)],
    })
}

fn compiled_combine_result_sets_program()
-> Result<ProductionSemanticFormProgram, ProductionQueryRecipeError> {
    let left_relation = release_relation_id("input.semantic-query.combine.left")?;
    let right_relation = release_relation_id("input.semantic-query.combine.right")?;
    let output_relation = release_relation_id("program.semantic-query.combine.output")?;
    let field_id = release_field_id("query-local.semantic-result.record_id")?;
    let left_node: Arc<str> = Arc::from("program.semantic-query.combine-result-sets.v2.left");
    let right_node: Arc<str> = Arc::from("program.semantic-query.combine-result-sets.v2.right");
    let root_node: Arc<str> = Arc::from("program.semantic-query.combine-result-sets.v2.union");
    Ok(ProductionSemanticFormProgram {
        form: ReleasedSemanticForm::CombineResultSets,
        program_binding_id: Arc::from("program.semantic-query.combine-result-sets.v2"),
        output_role_id: Arc::from("result.combined-semantic-results"),
        root_node_id: Arc::clone(&root_node),
        output_relation_id: output_relation.clone(),
        output_fields: vec![field_id.clone()],
        relations: vec![
            ProductionRelationDefinition {
                relation_id: left_relation.clone(),
                fields: vec![field_id.clone()],
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: right_relation.clone(),
                fields: vec![field_id.clone()],
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: output_relation,
                fields: vec![field_id.clone()],
                authority: ProductionRelationAuthority::ProgramResult,
            },
        ],
        operators: vec![
            ProductionOperatorDefinition {
                node_id: Arc::clone(&left_node),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: left_relation.clone(),
                },
                output_fields: vec![field_id.clone()],
            },
            ProductionOperatorDefinition {
                node_id: Arc::clone(&right_node),
                ordinal: 1,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: right_relation.clone(),
                },
                output_fields: vec![field_id.clone()],
            },
            ProductionOperatorDefinition {
                node_id: Arc::clone(&root_node),
                ordinal: 2,
                input_node_ids: vec![left_node, right_node],
                operator: ProgramRelationalOperator::Union {
                    kind: UnionKind::Distinct,
                },
                output_fields: vec![field_id.clone()],
            },
        ],
        selections: Vec::new(),
        returns: vec![identity_return(root_node, field_id)],
        request_inputs: Vec::new(),
        consumer_slots: vec![
            ProductionConsumerSlotDefinition {
                consumer_slot_id: Arc::from("input.left-results"),
                consumer_role_id: Arc::from("result.semantic-records"),
                input_relation_id: left_relation,
                minimum_edges: 0,
                maximum_edges: 64,
                composition: EpochBoundConsumerComposition::Single,
            },
            ProductionConsumerSlotDefinition {
                consumer_slot_id: Arc::from("input.right-results"),
                consumer_role_id: Arc::from("result.semantic-records"),
                input_relation_id: right_relation,
                minimum_edges: 0,
                maximum_edges: 64,
                composition: EpochBoundConsumerComposition::Union(UnionKind::Distinct),
            },
        ],
        required_fact_families: vec![Arc::from("fact-family.result-set")],
    })
}

fn compiled_summarize_objective_facts_program()
-> Result<ProductionSemanticFormProgram, ProductionQueryRecipeError> {
    let input_relation = release_relation_id("input.semantic-query.objective-summary")?;
    let output_relation = release_relation_id("program.semantic-query.objective-summary")?;
    let input_field = release_field_id("query-local.objective-summary.record_id")?;
    let output_field = release_field_id("program.objective-summary.count")?;
    let input_node: Arc<str> =
        Arc::from("program.semantic-query.summarize-objective-facts.v2.input");
    let root_node: Arc<str> =
        Arc::from("program.semantic-query.summarize-objective-facts.v2.aggregate");
    Ok(ProductionSemanticFormProgram {
        form: ReleasedSemanticForm::SummarizeObjectiveFacts,
        program_binding_id: Arc::from("program.semantic-query.summarize-objective-facts.v2"),
        output_role_id: Arc::from("result.objective-fact-summary"),
        root_node_id: Arc::clone(&root_node),
        output_relation_id: output_relation.clone(),
        output_fields: vec![output_field.clone()],
        relations: vec![
            ProductionRelationDefinition {
                relation_id: input_relation.clone(),
                fields: vec![input_field.clone()],
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: output_relation,
                fields: vec![output_field.clone()],
                authority: ProductionRelationAuthority::ProgramResult,
            },
        ],
        operators: vec![
            ProductionOperatorDefinition {
                node_id: Arc::clone(&input_node),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: input_relation.clone(),
                },
                output_fields: vec![input_field.clone()],
            },
            ProductionOperatorDefinition {
                node_id: Arc::clone(&root_node),
                ordinal: 1,
                input_node_ids: vec![input_node],
                operator: ProgramRelationalOperator::Aggregate {
                    group_by: Vec::new(),
                    aggregates: vec![crate::relational_semantic_query::ProgramAggregateField {
                        input_field_id: input_field.clone(),
                        output_field_id: output_field.clone(),
                        aggregate_operator: AggregateOperator::Count,
                    }],
                },
                output_fields: vec![output_field.clone()],
            },
        ],
        selections: Vec::new(),
        returns: vec![ProductionReturnDefinition {
            return_id: Arc::from("include"),
            value_kind: SemanticValueKind::Text,
            minimum_values: 0,
            maximum_values: 1,
            realizations: vec![ProductionReturnRealization {
                value: SemanticClauseValue::Text(Arc::from("count")),
                realization_node_id: root_node,
                realization_field_ids: vec![output_field],
            }],
        }],
        request_inputs: vec![ProductionRequestInputDefinition {
            input_id: Arc::from("facts"),
            relation_id: input_relation,
            fields: vec![EpochBoundRequestInputField {
                field_id: input_field,
                value_kind: SemanticValueKind::Text,
                required: true,
            }],
            minimum_rows: 0,
            maximum_rows: 10_000,
        }],
        consumer_slots: Vec::new(),
        required_fact_families: vec![Arc::from("fact-family.objective-summary")],
    })
}

fn identity_return(realization_node_id: Arc<str>, field_id: FieldId) -> ProductionReturnDefinition {
    ProductionReturnDefinition {
        return_id: Arc::from("include"),
        value_kind: SemanticValueKind::Text,
        minimum_values: 0,
        maximum_values: 1,
        realizations: vec![ProductionReturnRealization {
            value: SemanticClauseValue::Text(Arc::from("canonical-id")),
            realization_node_id,
            realization_field_ids: vec![field_id],
        }],
    }
}

fn compiled_release_scopes() -> Vec<ProductionScopeDefinition> {
    COMPILED_V2_0_SCOPE_DEFINITIONS
        .into_iter()
        .map(|definition| ProductionScopeDefinition {
            scope_id: Arc::from(definition.scope_id),
            value_kind: SemanticValueKind::Text,
            minimum_values: definition.minimum_values,
            maximum_values: definition.maximum_values,
            authorization_input_id: Arc::from(definition.authorization_input_id),
        })
        .collect()
}

fn release_relation_id(value: &str) -> Result<RelationId, ProductionQueryRecipeError> {
    RelationId::new(value).map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
        detail: error.to_string(),
    })
}

fn release_field_id(value: &str) -> Result<FieldId, ProductionQueryRecipeError> {
    FieldId::new(value).map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
        detail: error.to_string(),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ProductionQueryRecipeError {
    #[error("required {0} pin is absent")]
    MissingPin(&'static str),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("released form coverage is incomplete or duplicated: {0}")]
    ReleasedFormCoverage(String),
    #[error("compiled semantic-query release is invalid: {detail}")]
    InvalidCompiledRelease { detail: String },
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
    validate_program_bindings(program, &nodes, &relation_schemas, limits)
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
    limits: EpochBoundSemanticIngressLimits,
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
            || selection.maximum_values > limits.max_selection_rows()
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
            || return_definition.maximum_values > limits.max_return_rows()
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
            || input.maximum_rows > limits.max_request_input_rows()
            || input.fields.len() > limits.max_fields_per_request_input_row()
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
            || slot.maximum_edges > limits.compiler().max_fanin()
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
                realization_pin: encode_return_realization(realization)
                    .finish(b"codefabric.semantic-return-realization.v2"),
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
                handoff_pin: encode_request_input(input)
                    .finish(b"codefabric.semantic-request-input-handoff.v2"),
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
    limits: EpochBoundSemanticIngressLimits,
) -> Result<(), ProductionQueryRecipeError> {
    let mut seen = BTreeSet::new();
    for scope in scopes {
        validate_identity("scope", &scope.scope_id)?;
        validate_identity("scope authorization input", &scope.authorization_input_id)?;
        if scope.maximum_values == 0
            || scope.minimum_values > scope.maximum_values
            || scope.maximum_values > limits.max_scope_rows()
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
            handoff_pin: {
                let mut frame = CanonicalIdentityFrame::default();
                frame.pin(1, catalog_pin);
                frame.nested(2, encode_scope(&scope));
                frame.finish(b"codefabric.semantic-scope-handoff.v2")
            },
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
    let proof_pin = {
        let mut frame = CanonicalIdentityFrame::default();
        frame.text(1, operation_id);
        frame.frames(2, decoded.iter().map(encode_producer_family_closure));
        frame.finish(b"codefabric.executed-producer-closure.v2")
    };
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
    encode_program(program).finish(b"codefabric.production-semantic-program.v2")
}

fn binding_identity_pin(
    program: &ProductionSemanticFormProgram,
    execution_pin: [u8; 32],
) -> [u8; 32] {
    let mut frame = CanonicalIdentityFrame::default();
    frame.u64(1, released_form_code(program.form));
    frame.text(2, &program.program_binding_id);
    frame.text(3, &program.output_role_id);
    frame.pin(4, execution_pin);
    frame.finish(b"codefabric.production-semantic-binding.v2")
}

fn compiled_release_identity_pin(
    forms: &BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>,
    scopes: &[ProductionScopeDefinition],
) -> [u8; 32] {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, PRODUCTION_SEMANTIC_QUERY_RELEASE_ID);
    frame.frames(2, forms.values().map(encode_program));
    frame.frames(3, scopes.iter().map(encode_scope));
    frame.finish(b"codefabric.production-semantic-query-release.v2")
}

fn catalog_identity_pin(
    domain: &[u8],
    epoch_pin: [u8; 32],
    source_pin: [u8; 32],
    policy_pin: [u8; 32],
    release_pin: [u8; 32],
    producer_closure_pin: [u8; 32],
    forms: &BTreeMap<ReleasedSemanticForm, ProductionSemanticFormProgram>,
    scopes: &[ProductionScopeDefinition],
) -> [u8; 32] {
    let mut frame = CanonicalIdentityFrame::default();
    frame.pin(1, epoch_pin);
    frame.pin(2, source_pin);
    frame.pin(3, policy_pin);
    frame.pin(4, release_pin);
    frame.pin(5, producer_closure_pin);
    frame.frames(6, forms.values().map(encode_program));
    frame.frames(7, scopes.iter().map(encode_scope));
    frame.finish(domain)
}

fn typed_identity_pin(domain: &[u8], value: &str) -> [u8; 32] {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, value);
    frame.finish(domain)
}

#[derive(Default)]
struct CanonicalIdentityFrame {
    bytes: Vec<u8>,
}

impl CanonicalIdentityFrame {
    fn bytes(&mut self, tag: u16, value: &[u8]) {
        self.bytes.extend_from_slice(&tag.to_be_bytes());
        self.bytes.extend_from_slice(
            &u64::try_from(value.len())
                .expect("identity frame length fits u64")
                .to_be_bytes(),
        );
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, tag: u16, value: &str) {
        self.bytes(tag, value.as_bytes());
    }

    fn u64(&mut self, tag: u16, value: u64) {
        self.bytes(tag, &value.to_be_bytes());
    }

    fn i64(&mut self, tag: u16, value: i64) {
        self.bytes(tag, &value.to_be_bytes());
    }

    fn bool(&mut self, tag: u16, value: bool) {
        self.bytes(tag, &[u8::from(value)]);
    }

    fn pin(&mut self, tag: u16, value: [u8; 32]) {
        self.bytes(tag, &value);
    }

    fn nested(&mut self, tag: u16, value: Self) {
        self.bytes(tag, &value.bytes);
    }

    fn frames(&mut self, tag: u16, values: impl IntoIterator<Item = Self>) {
        let values = values.into_iter().collect::<Vec<_>>();
        let mut sequence = Self::default();
        sequence.u64(
            1,
            u64::try_from(values.len()).expect("identity sequence length fits u64"),
        );
        for value in values {
            sequence.nested(2, value);
        }
        self.nested(tag, sequence);
    }

    fn finish(self, domain: &[u8]) -> [u8; 32] {
        let mut envelope = Self::default();
        envelope.text(1, "codefabric.typed-identity-frame.v1");
        envelope.bytes(2, domain);
        envelope.bytes(3, &self.bytes);
        *blake3::hash(&envelope.bytes).as_bytes()
    }
}

fn encode_program(program: &ProductionSemanticFormProgram) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.u64(1, released_form_code(program.form));
    frame.text(2, &program.program_binding_id);
    frame.text(3, &program.output_role_id);
    frame.text(4, &program.root_node_id);
    frame.text(5, program.output_relation_id.as_str());
    frame.frames(6, program.output_fields.iter().map(encode_field_id));
    frame.frames(7, program.relations.iter().map(encode_relation));
    frame.frames(8, program.operators.iter().map(encode_operator));
    frame.frames(9, program.selections.iter().map(encode_selection));
    frame.frames(10, program.returns.iter().map(encode_return));
    frame.frames(11, program.request_inputs.iter().map(encode_request_input));
    frame.frames(12, program.consumer_slots.iter().map(encode_consumer_slot));
    frame.frames(
        13,
        program
            .required_fact_families
            .iter()
            .map(|value| encode_text(value)),
    );
    frame
}

fn encode_relation(value: &ProductionRelationDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, value.relation_id.as_str());
    frame.frames(2, value.fields.iter().map(encode_field_id));
    frame.u64(3, relation_authority_code(value.authority));
    frame
}

fn encode_operator(value: &ProductionOperatorDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.node_id);
    frame.u64(2, u64::from(value.ordinal));
    frame.frames(
        3,
        value
            .input_node_ids
            .iter()
            .map(|node_id| encode_text(node_id)),
    );
    frame.nested(4, encode_relational_operator(&value.operator));
    frame.frames(5, value.output_fields.iter().map(encode_field_id));
    frame
}

fn encode_relational_operator(value: &ProgramRelationalOperator) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    match value {
        ProgramRelationalOperator::Input { relation_id } => {
            frame.u64(1, 1);
            frame.text(2, relation_id.as_str());
        }
        ProgramRelationalOperator::Projection { fields } => {
            frame.u64(1, 2);
            frame.frames(
                2,
                fields.iter().map(|field| {
                    let mut field_frame = CanonicalIdentityFrame::default();
                    field_frame.text(1, field.input_field_id.as_str());
                    field_frame.text(2, field.output_field_id.as_str());
                    field_frame
                }),
            );
        }
        ProgramRelationalOperator::Filter => frame.u64(1, 3),
        ProgramRelationalOperator::Join { kind, predicates } => {
            frame.u64(1, 4);
            frame.u64(2, join_kind_code(*kind));
            frame.frames(
                3,
                predicates.iter().map(|predicate| {
                    let mut predicate_frame = CanonicalIdentityFrame::default();
                    predicate_frame.text(1, predicate.left_field_id.as_str());
                    predicate_frame.text(2, predicate.right_field_id.as_str());
                    predicate_frame.u64(3, scalar_operator_code(predicate.scalar_operator));
                    predicate_frame
                }),
            );
        }
        ProgramRelationalOperator::Union { kind } => {
            frame.u64(1, 5);
            frame.u64(2, union_kind_code(*kind));
        }
        ProgramRelationalOperator::Aggregate {
            group_by,
            aggregates,
        } => {
            frame.u64(1, 6);
            frame.frames(
                2,
                group_by.iter().map(|field| {
                    let mut field_frame = CanonicalIdentityFrame::default();
                    field_frame.text(1, field.input_field_id.as_str());
                    field_frame.text(2, field.output_field_id.as_str());
                    field_frame
                }),
            );
            frame.frames(
                3,
                aggregates.iter().map(|field| {
                    let mut field_frame = CanonicalIdentityFrame::default();
                    field_frame.text(1, field.input_field_id.as_str());
                    field_frame.text(2, field.output_field_id.as_str());
                    field_frame.u64(3, aggregate_operator_code(field.aggregate_operator));
                    field_frame
                }),
            );
        }
        ProgramRelationalOperator::Sort { fields } => {
            frame.u64(1, 7);
            frame.frames(
                2,
                fields.iter().map(|field| {
                    let mut field_frame = CanonicalIdentityFrame::default();
                    field_frame.text(1, field.input_field_id.as_str());
                    field_frame.bool(2, field.ascending);
                    field_frame.bool(3, field.nulls_first);
                    field_frame
                }),
            );
        }
        ProgramRelationalOperator::Limit { skip } => {
            frame.u64(1, 8);
            frame.u64(
                2,
                u64::try_from(*skip).expect("validated limit skip fits u64"),
            );
        }
    }
    frame
}

fn encode_selection(value: &ProductionSelectionDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.selection_id);
    frame.u64(2, semantic_value_kind_code(value.value_kind));
    frame.u64(3, usize_identity(value.minimum_values));
    frame.u64(4, usize_identity(value.maximum_values));
    frame.text(5, &value.operator_node_id);
    frame.text(6, value.input_field_id.as_str());
    frame.u64(7, scalar_operator_code(value.scalar_operator));
    frame.u64(8, selection_fold_code(value.fold));
    frame
}

fn encode_return(value: &ProductionReturnDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.return_id);
    frame.u64(2, semantic_value_kind_code(value.value_kind));
    frame.u64(3, usize_identity(value.minimum_values));
    frame.u64(4, usize_identity(value.maximum_values));
    frame.frames(5, value.realizations.iter().map(encode_return_realization));
    frame
}

fn encode_return_realization(value: &ProductionReturnRealization) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.nested(1, encode_clause_value(&value.value));
    frame.text(2, &value.realization_node_id);
    frame.frames(3, value.realization_field_ids.iter().map(encode_field_id));
    frame
}

fn encode_request_input(value: &ProductionRequestInputDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.input_id);
    frame.text(2, value.relation_id.as_str());
    frame.frames(
        3,
        value.fields.iter().map(|field| {
            let mut field_frame = CanonicalIdentityFrame::default();
            field_frame.text(1, field.field_id.as_str());
            field_frame.u64(2, semantic_value_kind_code(field.value_kind));
            field_frame.bool(3, field.required);
            field_frame
        }),
    );
    frame.u64(4, usize_identity(value.minimum_rows));
    frame.u64(5, usize_identity(value.maximum_rows));
    frame
}

fn encode_consumer_slot(value: &ProductionConsumerSlotDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.consumer_slot_id);
    frame.text(2, &value.consumer_role_id);
    frame.text(3, value.input_relation_id.as_str());
    frame.u64(4, usize_identity(value.minimum_edges));
    frame.u64(5, usize_identity(value.maximum_edges));
    match value.composition {
        EpochBoundConsumerComposition::Single => frame.u64(6, 1),
        EpochBoundConsumerComposition::Union(kind) => {
            frame.u64(6, 2);
            frame.u64(7, union_kind_code(kind));
        }
    }
    frame
}

fn encode_scope(value: &ProductionScopeDefinition) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.scope_id);
    frame.u64(2, semantic_value_kind_code(value.value_kind));
    frame.u64(3, usize_identity(value.minimum_values));
    frame.u64(4, usize_identity(value.maximum_values));
    frame.text(5, &value.authorization_input_id);
    frame
}

fn encode_clause_value(value: &SemanticClauseValue) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    match value {
        SemanticClauseValue::Boolean(value) => {
            frame.u64(1, 1);
            frame.bool(2, *value);
        }
        SemanticClauseValue::Int64(value) => {
            frame.u64(1, 2);
            frame.i64(2, *value);
        }
        SemanticClauseValue::UInt64(value) => {
            frame.u64(1, 3);
            frame.u64(2, *value);
        }
        SemanticClauseValue::Text(value) => {
            frame.u64(1, 4);
            frame.text(2, value);
        }
    }
    frame
}

fn encode_producer_family_closure(value: &ProducerFamilyClosureRow) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, &value.family_id);
    match &value.disposition {
        ProducerFamilyDisposition::RuntimeProducer(producer) => {
            frame.u64(2, 1);
            frame.text(3, &producer.producer_id);
            frame.text(4, &producer.authority_id);
            frame.text(5, &producer.algorithm_release);
            frame.text(6, &producer.precision_id);
            frame.pin(7, producer.input_pin);
            frame.pin(8, producer.invalidation_pin);
            frame.pin(9, producer.materialization_pin);
            frame.u64(10, producer.requested_units);
            frame.u64(11, producer.completed_units);
            frame.u64(12, producer.remainder_units);
            frame.u64(13, producer.unknown_units);
            frame.pin(14, producer.completeness_proof_pin);
            frame.pin(15, producer.producer_proof_pin);
        }
        ProducerFamilyDisposition::UnsupportedRemainder(remainder) => {
            frame.u64(2, 2);
            frame.text(3, &remainder.remainder_id);
            frame.text(4, &remainder.authority_id);
            frame.text(5, &remainder.reason_id);
            frame.pin(6, remainder.proof_pin);
        }
    }
    frame
}

fn encode_text(value: &str) -> CanonicalIdentityFrame {
    let mut frame = CanonicalIdentityFrame::default();
    frame.text(1, value);
    frame
}

fn encode_field_id(value: &FieldId) -> CanonicalIdentityFrame {
    encode_text(value.as_str())
}

fn usize_identity(value: usize) -> u64 {
    u64::try_from(value).expect("validated identity cardinality fits u64")
}

const fn released_form_code(value: ReleasedSemanticForm) -> u64 {
    match value {
        ReleasedSemanticForm::FindCodeEntities => 1,
        ReleasedSemanticForm::RetrieveFactsAboutCode => 2,
        ReleasedSemanticForm::FollowCodeRelationships => 3,
        ReleasedSemanticForm::FindConnectingFactPaths => 4,
        ReleasedSemanticForm::MatchCodeFactPattern => 5,
        ReleasedSemanticForm::CombineResultSets => 6,
        ReleasedSemanticForm::SummarizeObjectiveFacts => 7,
        ReleasedSemanticForm::RetrieveSourceAndSyntaxContext => 8,
    }
}

const fn relation_authority_code(value: ProductionRelationAuthority) -> u64 {
    match value {
        ProductionRelationAuthority::Epoch => 1,
        ProductionRelationAuthority::QueryLocal => 2,
        ProductionRelationAuthority::ProgramResult => 3,
    }
}

const fn semantic_value_kind_code(value: SemanticValueKind) -> u64 {
    match value {
        SemanticValueKind::Boolean => 1,
        SemanticValueKind::Int64 => 2,
        SemanticValueKind::UInt64 => 3,
        SemanticValueKind::Text => 4,
    }
}

const fn scalar_operator_code(value: ScalarOperator) -> u64 {
    match value {
        ScalarOperator::Equal => 1,
        ScalarOperator::NotEqual => 2,
        ScalarOperator::LessThan => 3,
        ScalarOperator::LessThanOrEqual => 4,
        ScalarOperator::GreaterThan => 5,
        ScalarOperator::GreaterThanOrEqual => 6,
        ScalarOperator::And => 7,
        ScalarOperator::Or => 8,
        ScalarOperator::Not => 9,
        ScalarOperator::Add => 10,
        ScalarOperator::Subtract => 11,
        ScalarOperator::Multiply => 12,
        ScalarOperator::Divide => 13,
        ScalarOperator::IsNull => 14,
        ScalarOperator::IsNotNull => 15,
    }
}

const fn aggregate_operator_code(value: AggregateOperator) -> u64 {
    match value {
        AggregateOperator::Count => 1,
        AggregateOperator::CountDistinct => 2,
        AggregateOperator::Sum => 3,
        AggregateOperator::Average => 4,
        AggregateOperator::Minimum => 5,
        AggregateOperator::Maximum => 6,
    }
}

const fn join_kind_code(value: JoinKind) -> u64 {
    match value {
        JoinKind::Inner => 1,
        JoinKind::Left => 2,
        JoinKind::Right => 3,
        JoinKind::Full => 4,
        JoinKind::LeftSemi => 5,
        JoinKind::RightSemi => 6,
        JoinKind::LeftAnti => 7,
        JoinKind::RightAnti => 8,
    }
}

const fn union_kind_code(value: UnionKind) -> u64 {
    match value {
        UnionKind::All => 1,
        UnionKind::Distinct => 2,
    }
}

const fn selection_fold_code(value: EpochBoundSelectionFold) -> u64 {
    match value {
        EpochBoundSelectionFold::All => 1,
        EpochBoundSelectionFold::Any => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{ArrayRef, Int64Array, StringArray, UInt64Array};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;
    use datafusion::datasource::MemTable;

    use super::*;
    use crate::fabric::epoch_runtime::{
        FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole,
    };
    use crate::fabric::production_kernel::CompiledSemanticRelease;
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
        let epoch_relations = compiled_released_form_programs()
            .expect("compiled programs")
            .into_values()
            .flat_map(|program| program.relations)
            .filter(|relation| relation.authority == ProductionRelationAuthority::Epoch)
            .map(|relation| (relation.relation_id, relation.fields))
            .collect::<BTreeMap<_, _>>();
        for (index, (relation_id, field_ids)) in epoch_relations.into_iter().enumerate() {
            let schema = Arc::new(
                Schema::new(
                    field_ids
                        .iter()
                        .enumerate()
                        .map(|(field_index, field_id)| {
                            Field::new(format!("field_{field_index}"), DataType::Int64, false)
                                .with_metadata(HashMap::from([(
                                    FIELD_ID_METADATA_KEY.to_owned(),
                                    field_id.as_str().to_owned(),
                                )]))
                        })
                        .collect::<Vec<_>>(),
                )
                .with_metadata(HashMap::from([(
                    RELATION_ID_METADATA_KEY.to_owned(),
                    relation_id.as_str().to_owned(),
                )])),
            );
            let batch = RecordBatch::try_new(
                Arc::clone(&schema),
                field_ids
                    .iter()
                    .map(|_| {
                        Arc::new(Int64Array::from(vec![
                            i64::try_from(index).expect("small test index"),
                        ])) as ArrayRef
                    })
                    .collect(),
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
                    (0..field_ids.len())
                        .map(|field_index| FieldIndexMapping::direct(field_index, field_index))
                        .collect(),
                )
                .expect("schema contract"),
            );
            builder
                .register_provider(ProviderInput::new(
                    ProgrammaticRelationId::new(relation_id.as_str()),
                    table_reference,
                    contract,
                    provider,
                ))
                .expect("provider registration");
        }
        builder.seal_for_test().await.expect("sealed epoch")
    }

    fn limits() -> EpochBoundSemanticIngressLimits {
        use crate::relational_semantic_query::SemanticRequestLimits;

        EpochBoundSemanticIngressLimits::try_new(
            SemanticRequestLimits::try_new(16, 64, 16, 64, 32, 32, 10_000)
                .expect("compiler limits"),
            128,
            128,
            256,
            10_000,
            32,
        )
        .expect("ingress limits")
    }

    fn alternate_limits() -> EpochBoundSemanticIngressLimits {
        use crate::relational_semantic_query::SemanticRequestLimits;

        EpochBoundSemanticIngressLimits::try_new(
            SemanticRequestLimits::try_new(32, 128, 32, 128, 64, 64, 20_000)
                .expect("compiler limits"),
            256,
            256,
            512,
            20_000,
            64,
        )
        .expect("ingress limits")
    }

    fn input(
        source_pin: [u8; 32],
        policy_pin: [u8; 32],
        limits: EpochBoundSemanticIngressLimits,
    ) -> ProductionSemanticQueryRecipeInput {
        ProductionSemanticQueryRecipeInput::try_new(source_pin, policy_pin, limits)
            .expect("operational query input")
    }

    fn closure() -> ProducerClosureProof {
        let families = compiled_released_form_programs()
            .expect("compiled programs")
            .into_values()
            .flat_map(|program| program.required_fact_families)
            .collect::<BTreeSet<_>>();
        ProducerClosureProof {
            proof_pin: [0x14; 32],
            application_authority_id: Arc::from("authority.application"),
            families: families
                .into_iter()
                .enumerate()
                .map(|(index, family_id)| ProducerFamilyClosureRow {
                    family_id,
                    disposition: ProducerFamilyDisposition::RuntimeProducer(RuntimeProducerProof {
                        producer_id: Arc::from(format!("producer.release.{index}")),
                        authority_id: Arc::from("authority.application"),
                        algorithm_release: Arc::from("algorithm.release.v2"),
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
                })
                .collect(),
        }
    }

    fn assemble(
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
    ) -> Result<ProductionSemanticQueryRecipe, ProductionQueryRecipeError> {
        let release = CompiledSemanticRelease::current();
        ProductionSemanticQueryRecipe::assemble(release.query_authority(), epoch, input, closure())
    }

    #[tokio::test]
    async fn compiled_release_builds_all_eight_epoch_checked_programs() {
        let epoch = epoch().await;
        let recipe =
            assemble(&epoch, input([0x11; 32], [0x12; 32], limits())).expect("all eight programs");
        assert_eq!(
            recipe.ingress_catalog().program_bindings.len(),
            ReleasedSemanticForm::ALL.len()
        );
        assert_eq!(
            recipe.execution_catalog().programs.len(),
            ReleasedSemanticForm::ALL.len()
        );

        use crate::relational_semantic_query::{
            EpochBoundBlockBindingRow, EpochBoundScopeRow, EpochBoundSemanticIngress,
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
            scopes: [
                ("scope.workspace-id", "workspace:test"),
                ("scope.codebase", "current"),
                ("scope.analysis-context-mode", "default"),
                ("scope.external-entity-policy", "endpoint-only"),
            ]
            .into_iter()
            .map(|(scope_id, value)| EpochBoundScopeRow {
                scope_id: Arc::from(scope_id),
                ordinal: 0,
                value: SemanticClauseValue::Text(Arc::from(value)),
            })
            .collect(),
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
    async fn recipe_composes_compiled_v2_ports_with_shared_scope_authority() {
        let epoch = epoch().await;
        let policy_pin = [0x12; 32];
        let recipe = assemble(&epoch, input([0x11; 32], policy_pin, limits()))
            .expect("compiled query recipe");
        let release = CompiledSemanticRelease::current();
        let ports = release
            .compose_semantic_query_ports(
                &recipe,
                limits(),
                policy_pin,
                BTreeSet::from([ProgrammaticRelationId::new("public.semantic_entity")]),
                1_000,
            )
            .expect("recipe and release-owned ports share one v2 scope authority");
        assert_eq!(
            ports.application_release(),
            crate::fabric::programmatic_query_backend::compiled_query_release_pin(
                release.query_authority()
            )
        );
    }

    #[tokio::test]
    async fn operational_inputs_cannot_substitute_release_owned_forms_or_scopes() {
        let epoch = epoch().await;
        let first =
            assemble(&epoch, input([0x11; 32], [0x12; 32], limits())).expect("first recipe");
        let changed_authority = assemble(&epoch, input([0x41; 32], [0x42; 32], limits()))
            .expect("changed operational authority");
        let changed_resources = assemble(&epoch, input([0x11; 32], [0x12; 32], alternate_limits()))
            .expect("changed resource policy");

        assert_eq!(
            first.execution_catalog().program_release_pin,
            changed_authority.execution_catalog().program_release_pin
        );
        assert_eq!(
            first.execution_catalog().programs,
            changed_authority.execution_catalog().programs
        );
        assert_eq!(
            first.execution_catalog().operators,
            changed_authority.execution_catalog().operators
        );
        assert_eq!(
            first.execution_catalog().relation_schemas,
            changed_authority.execution_catalog().relation_schemas
        );
        let first_scope_authority = first
            .execution_catalog()
            .scopes
            .iter()
            .map(|scope| (&scope.scope_id, &scope.authorization_input_id))
            .collect::<Vec<_>>();
        let changed_scope_authority = changed_authority
            .execution_catalog()
            .scopes
            .iter()
            .map(|scope| (&scope.scope_id, &scope.authorization_input_id))
            .collect::<Vec<_>>();
        assert_eq!(first_scope_authority, changed_scope_authority);
        assert!(
            first
                .execution_catalog()
                .scopes
                .iter()
                .zip(&changed_authority.execution_catalog().scopes)
                .all(|(first, changed)| first.handoff_pin != changed.handoff_pin)
        );
        assert_eq!(
            first.execution_catalog().programs,
            changed_resources.execution_catalog().programs
        );
        assert_eq!(
            first.execution_catalog().scopes,
            changed_resources.execution_catalog().scopes
        );
        assert_ne!(
            first.ingress_catalog().program_catalog_pin,
            changed_authority.ingress_catalog().program_catalog_pin
        );
        assert_eq!(
            first.ingress_catalog().program_catalog_pin,
            changed_resources.ingress_catalog().program_catalog_pin
        );
        assert_ne!(
            first.ingress_catalog().limits_pin,
            changed_resources.ingress_catalog().limits_pin
        );
    }

    #[tokio::test]
    async fn compiled_operand_and_epoch_schema_mutations_fail_closed() {
        let epoch = epoch().await;
        let forms = compiled_released_form_programs().expect("compiled programs");
        let original = forms
            .get(&ReleasedSemanticForm::FindCodeEntities)
            .expect("entity program");
        let mut changed_operand = original.clone();
        changed_operand.selections[0].scalar_operator = ScalarOperator::NotEqual;
        assert_ne!(
            program_identity_pin(original),
            program_identity_pin(&changed_operand)
        );

        let mut missing = original.clone();
        missing.relations[0].relation_id = relation("public.absent");
        assert!(matches!(
            validate_program(&epoch, &missing, limits()),
            Err(ProductionQueryRecipeError::MissingEpochRelation { .. })
        ));

        let mut drifted = original.clone();
        drifted.relations[0].fields[0] = field("public.wrong-field");
        assert!(matches!(
            validate_program(&epoch, &drifted, limits()),
            Err(ProductionQueryRecipeError::EpochFieldDrift { .. })
        ));
    }

    #[test]
    fn typed_identity_framing_distinguishes_field_boundaries() {
        let mut left = CanonicalIdentityFrame::default();
        left.text(1, "ab");
        left.text(2, "c");
        let mut right = CanonicalIdentityFrame::default();
        right.text(1, "a");
        right.text(2, "bc");
        assert_ne!(
            left.finish(b"test.identity"),
            right.finish(b"test.identity")
        );
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
                text(Some("fact-family.semantic-fact")),
                text(Some(RELEASE_FACTUAL_SEMANTIC_CLASS_ID)),
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
                &Arc::from(RELEASE_FACTUAL_SEMANTIC_CLASS_ID),
                &Arc::from("operation.test"),
                &Arc::from("authority.application"),
            ),
            Err(ProductionQueryRecipeError::ProducerClosureRow { .. })
        ));
    }
}
