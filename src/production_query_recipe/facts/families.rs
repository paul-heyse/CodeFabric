//! Family-specific query templates read canonical relations without persisted selector copies.

use super::{
    Arc, EpochBoundRequestInputField, EpochBoundSelectionFold, EpochBoundSelectionValueResolution,
    JoinKind, ProductionOperatorDefinition, ProductionQueryRecipeError,
    ProductionRelationAuthority, ProductionRelationDefinition, ProductionRequestInputDefinition,
    ProductionSelectionDefinition, ProductionSemanticFormProgram, ProgramJoinPredicate,
    ProgramProjectionField, ProgramRelationalOperator, ProgramSortField, ProgrammaticFabricEpoch,
    ProgrammaticRelationId, RELEASE_SELECTION_MAXIMUM_VALUES, ReleasedSemanticForm, ResultRole,
    ScalarOperator, SchemaRole, SemanticClauseValue, SemanticValueKind, release_field_id,
    release_relation_id,
};
use crate::relational_semantic_query::EpochBoundSelectionTarget;

#[derive(Clone, Copy)]
pub(crate) struct Family {
    pub slug: &'static str,
    pub relation: &'static str,
    pub meaning: &'static str,
    pub coverage: &'static str,
    // Besides exact workspace/context, match this canonical fact key to the selected entity key.
    keys: Option<(&'static str, &'static str)>,
}

pub(crate) const FAMILIES: &[Family] = &[
    Family {
        slug: "modules",
        relation: "fact.code_module",
        meaning: "module metadata",
        coverage: "modules",
        keys: Some(("entity_id", "entity_id")),
    },
    Family {
        slug: "imports",
        relation: "fact.code_import",
        meaning: "imports in declaring files",
        coverage: "imports",
        keys: Some(("file_id", "file_id")),
    },
    Family {
        slug: "semantic-references",
        relation: "fact.code_semantic_reference",
        meaning: "semantic references to entities",
        coverage: "semantic-references",
        keys: Some(("target_entity_id", "entity_id")),
    },
    Family {
        slug: "type-observations",
        relation: "fact.code_type_observation",
        meaning: "type observations in declaring files",
        coverage: "types",
        keys: Some(("file_id", "file_id")),
    },
    Family {
        slug: "types",
        relation: "fact.code_type",
        meaning: "structural types in analysis contexts",
        coverage: "types",
        keys: None,
    },
    Family {
        slug: "type-components",
        relation: "fact.code_type_component",
        meaning: "type components in declaring files",
        coverage: "types",
        keys: Some(("file_id", "file_id")),
    },
    Family {
        slug: "diagnostics",
        relation: "fact.code_diagnostic",
        meaning: "diagnostic messages in analysis contexts",
        coverage: "diagnostic-messages",
        keys: None,
    },
    Family {
        slug: "diagnostic-children",
        relation: "fact.code_diagnostic_child",
        meaning: "diagnostic children in analysis contexts",
        coverage: "diagnostic-children",
        keys: None,
    },
    Family {
        slug: "diagnostic-spans",
        relation: "fact.code_diagnostic_span",
        meaning: "diagnostic locations in analysis contexts",
        coverage: "diagnostic-locations",
        keys: None,
    },
    Family {
        slug: "diagnostic-suggestions",
        relation: "fact.code_diagnostic_suggestion",
        meaning: "diagnostic suggestions in analysis contexts",
        coverage: "diagnostic-suggestions",
        keys: None,
    },
    Family {
        slug: "diagnostic-edits",
        relation: "fact.code_diagnostic_edit",
        meaning: "diagnostic edits in analysis contexts",
        coverage: "diagnostic-suggestions",
        keys: None,
    },
];

pub(crate) fn known_meaning(value: &str) -> bool {
    matches!(
        value,
        "declarations" | "declaration locations and provenance"
    ) || FAMILIES.iter().any(|family| family.meaning == value)
}

pub(crate) fn result_family(relation: &str) -> Option<&'static Family> {
    let slug = relation.strip_prefix("query.result.canonical-")?;
    FAMILIES.iter().find(|family| family.slug == slug)
}

pub(in crate::production_query_recipe) fn programs(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Vec<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    FAMILIES
        .iter()
        .filter_map(|family| match program(epoch, *family) {
            Ok(Some(program)) => Some(Ok(program)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

// Keep each typed plan's joins, field lineage and selected coverage auditable in one template.
#[allow(clippy::too_many_lines)]
fn program(
    epoch: &ProgrammaticFabricEpoch,
    family: Family,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let entity_id = "fact.code_entity_selector";
    let Some(source) = definition(epoch, family.relation)? else {
        return Ok(None);
    };
    let Some(entities) = definition(epoch, entity_id)? else {
        return Ok(None);
    };
    let schema = epoch
        .relation(&ProgrammaticRelationId::new(family.relation))
        .expect("bound relation")
        .contract
        .logical_schema();
    // Older retained epochs can predate a family's scoped public schema.
    if ["language", "context_id", "workspace_id"]
        .iter()
        .any(|name| schema.field_with_name(name).is_err())
    {
        return Ok(None);
    }
    let field = |relation: &str, name: &str| release_field_id(&format!("{relation}.{name}"));
    let output_id = format!("query.result.canonical-{}", family.slug);
    let input_id = release_relation_id(&format!("query.input.canonical-{}-about", family.slug))?;
    let input_fields = ["about.kind", "about.value", "about.producer-role"]
        .into_iter()
        .map(release_field_id)
        .collect::<Result<Vec<_>, _>>()?;
    let output_fields = schema
        .fields()
        .iter()
        .map(|f| field(&output_id, f.name()))
        .collect::<Result<Vec<_>, _>>()?;
    let projections = source
        .fields
        .iter()
        .zip(&output_fields)
        .map(|(input, output)| ProgramProjectionField {
            output_name: None,
            output_nullable: None,
            public_entity_kind: None,
            input_field_id: input.clone(),
            output_field_id: output.clone(),
        })
        .collect();
    let binding = format!("program.semantic-query.retrieve-facts.{}.v1", family.slug);
    let node = |suffix: &str| Arc::<str>::from(format!("{binding}.{suffix}"));
    let operator = |name: &str, ordinal, inputs: &[&str], operator, output_fields| {
        ProductionOperatorDefinition {
            node_id: node(name),
            ordinal,
            input_node_ids: inputs.iter().map(|n| node(n)).collect(),
            operator,
            output_fields,
        }
    };
    let mut predicates = ["workspace_id", "context_id"]
        .into_iter()
        .map(|name| {
            Ok(ProgramJoinPredicate {
                left_field_id: field(family.relation, name)?,
                right_field_id: field(entity_id, name)?,
                scalar_operator: ScalarOperator::Equal,
            })
        })
        .collect::<Result<Vec<_>, ProductionQueryRecipeError>>()?;
    if let Some((fact, entity)) = family.keys {
        predicates.push(ProgramJoinPredicate {
            left_field_id: field(family.relation, fact)?,
            right_field_id: field(entity_id, entity)?,
            scalar_operator: ScalarOperator::Equal,
        });
    }
    let output_relation = release_relation_id(&output_id)?;
    let operators = vec![
        operator(
            "source",
            0,
            &[],
            ProgramRelationalOperator::Input {
                relation_id: source.relation_id.clone(),
            },
            source.fields.clone(),
        ),
        operator(
            "entities",
            1,
            &[],
            ProgramRelationalOperator::Input {
                relation_id: entities.relation_id.clone(),
            },
            entities.fields.clone(),
        ),
        operator(
            "about",
            2,
            &[],
            ProgramRelationalOperator::Input {
                relation_id: input_id.clone(),
            },
            input_fields.clone(),
        ),
        operator(
            "subjects",
            3,
            &["entities", "about"],
            ProgramRelationalOperator::Join {
                kind: JoinKind::LeftSemi,
                predicates: vec![ProgramJoinPredicate {
                    left_field_id: field(entity_id, "public_entity_id")?,
                    right_field_id: input_fields[1].clone(),
                    scalar_operator: ScalarOperator::Equal,
                }],
            },
            entities.fields.clone(),
        ),
        operator(
            "facts",
            4,
            &["source", "subjects"],
            ProgramRelationalOperator::Join {
                kind: JoinKind::LeftSemi,
                predicates,
            },
            source.fields.clone(),
        ),
        operator(
            "family",
            5,
            &["facts"],
            ProgramRelationalOperator::Filter,
            source.fields.clone(),
        ),
        operator(
            "project",
            6,
            &["family"],
            ProgramRelationalOperator::Projection {
                fields: projections,
            },
            output_fields.clone(),
        ),
        operator(
            "sort",
            7,
            &["project"],
            ProgramRelationalOperator::Sort {
                fields: output_fields
                    .iter()
                    .map(|field| ProgramSortField {
                        input_field_id: field.clone(),
                        ascending: true,
                        nulls_first: false,
                    })
                    .collect(),
            },
            output_fields.clone(),
        ),
        operator(
            "limit",
            8,
            &["sort"],
            ProgramRelationalOperator::Limit { skip: 0 },
            output_fields.clone(),
        ),
    ];
    Ok(Some(ProductionSemanticFormProgram {
        form: ReleasedSemanticForm::RetrieveFactsAboutCode,
        program_binding_id: Arc::from(binding.clone()),
        output_role_id: Arc::from(ResultRole::Facts.released_id()),
        root_node_id: node("limit"),
        output_relation_id: output_relation.clone(),
        output_fields: output_fields.clone(),
        relations: vec![
            source,
            entities,
            ProductionRelationDefinition {
                relation_id: input_id.clone(),
                fields: input_fields.clone(),
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: output_relation,
                fields: output_fields,
                authority: ProductionRelationAuthority::ProgramResult,
            },
        ],
        operators,
        selections: vec![ProductionSelectionDefinition {
            selection_id: Arc::from("selection.facts"),
            value_kind: SemanticValueKind::Text,
            minimum_values: 1,
            maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
            operator_node_id: node("family"),
            target: EpochBoundSelectionTarget::Program,
            fold: EpochBoundSelectionFold::Any,
            resolutions: vec![EpochBoundSelectionValueResolution {
                request_value: SemanticClauseValue::Text(Arc::from(family.meaning)),
                execution_value: SemanticClauseValue::Text(Arc::from(family.meaning)),
            }],
        }],
        request_inputs: vec![ProductionRequestInputDefinition {
            input_id: Arc::from("input.about"),
            relation_id: input_id,
            fields: input_fields
                .into_iter()
                .enumerate()
                .map(|(index, field_id)| EpochBoundRequestInputField {
                    field_id,
                    value_kind: SemanticValueKind::Text,
                    required: index < 2,
                })
                .collect(),
            minimum_rows: 1,
            maximum_rows: RELEASE_SELECTION_MAXIMUM_VALUES,
        }],
        returns: vec![],
        consumer_slots: vec![],
        required_fact_families: vec![],
    }))
}

pub(super) fn definition(
    epoch: &ProgrammaticFabricEpoch,
    id: &str,
) -> Result<Option<ProductionRelationDefinition>, ProductionQueryRecipeError> {
    let Some(relation) = epoch.relation(&ProgrammaticRelationId::new(id)) else {
        return Ok(None);
    };
    let fields = (0..relation.contract.logical_schema().fields().len())
        .map(|index| {
            relation
                .contract
                .field_id_at(SchemaRole::Logical, index)
                .map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
                    detail: error.to_string(),
                })
                .and_then(release_field_id)
        })
        .collect::<Result<_, _>>()?;
    Ok(Some(ProductionRelationDefinition {
        relation_id: release_relation_id(id)?,
        fields,
        authority: ProductionRelationAuthority::Epoch,
    }))
}
