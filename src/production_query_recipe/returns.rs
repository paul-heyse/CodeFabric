//! Released semantic ordering maps to native sort fields, never SQL supplied by the caller.

use super::{
    Arc, EpochBoundReturnAction, ProductionReturnDefinition, ProductionReturnRealization,
    ProductionSemanticFormProgram, ProgramRelationalOperator, RELEASE_SELECTION_MAXIMUM_VALUES,
    SemanticClauseValue, SemanticValueKind,
};
use crate::relational_semantic_query::ProgramSortField;

pub(super) fn install(programs: &mut [ProductionSemanticFormProgram]) {
    for program in programs {
        if program.operators.iter().any(|node| {
            node.node_id == program.root_node_id
                && matches!(node.operator, ProgramRelationalOperator::Limit { .. })
        }) {
            program.returns.push(ProductionReturnDefinition {
                return_id: Arc::from("return.when-exceeded"),
                value_kind: SemanticValueKind::Text,
                minimum_values: 0,
                maximum_values: 1,
                realizations: vec![ProductionReturnRealization {
                    value: SemanticClauseValue::Text(Arc::from("truncate")),
                    realization_node_id: program.root_node_id.clone(),
                    realization_field_ids: program.output_fields.clone(),
                    action: EpochBoundReturnAction::Truncate,
                }],
            });
        }
        let Some(sort) = program.operators.iter().find(|node| {
            matches!(node.operator, ProgramRelationalOperator::Sort { .. })
                && node.output_fields == program.output_fields
        }) else {
            continue;
        };
        let mut realizations = Vec::new();
        for field in &program.output_fields {
            for meaning in meanings(field.as_str().rsplit('.').next().unwrap_or_default()) {
                for (suffix, ascending) in
                    [("", true), (" ascending", true), (" descending", false)]
                {
                    realizations.push(ProductionReturnRealization {
                        value: SemanticClauseValue::Text(Arc::from(format!("{meaning}{suffix}"))),
                        realization_node_id: sort.node_id.clone(),
                        realization_field_ids: program.output_fields.clone(),
                        action: EpochBoundReturnAction::OrderBy {
                            fields: vec![ProgramSortField {
                                input_field_id: field.clone(),
                                ascending,
                                nulls_first: false,
                            }],
                        },
                    });
                }
            }
        }
        if !realizations.is_empty() {
            program.returns.push(ProductionReturnDefinition {
                return_id: Arc::from("return.order-by"),
                value_kind: SemanticValueKind::Text,
                minimum_values: 0,
                maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
                realizations,
            });
        }
    }
}

fn meanings(field: &str) -> &'static [&'static str] {
    match field {
        "entity-name" | "name" => &["name"],
        "qualified-name" | "qualified_name" => &["qualified name"],
        "entity-kind" | "entity_kind" => &["semantic kind", "kind"],
        "entity-language" | "language" => &["language"],
        "public-entity-id" | "public_entity_id" => &["canonical semantic identity"],
        "analysis-context-id" | "context_id" => &["analysis context"],
        "relative_path" | "source-file-path" => &["source file path"],
        "start_byte" | "start-byte" => &["source position"],
        "end_byte" | "end-byte" => &["source end position"],
        "raw_kind" => &["raw kind"],
        "provider" => &["provider"],
        "resolution" => &["resolution"],
        "dispatch_kind" => &["dispatch kind"],
        "caller_name" => &["caller name"],
        "target_name" => &["target name"],
        "source_name" => &["source name"],
        _ => &[],
    }
}
