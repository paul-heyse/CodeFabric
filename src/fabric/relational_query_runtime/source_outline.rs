//! Exact source anchors select a flat, identity-preserving CST outline with native joins.

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema};
use datafusion::common::ScalarValue;

use crate::relational_program::{
    FieldId, JoinKind, NamedExpression, RelationalExpression as R, RelationalProgramError,
    ScalarExpression as E, ScalarOperator as O, SortExpression, SupplementalProgramRelationBinding,
};
use crate::schema_contract::SchemaRole;

use super::super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::super::programmatic_schema::ProgrammaticRelationId;
use super::super::source_context_query::SourceContextParameters;

impl super::SelectedQueryOutput {
    pub(crate) fn with_syntax_outline(
        mut self,
        epoch: &ProgrammaticFabricEpoch,
        parameters: SourceContextParameters,
    ) -> Result<Self, RelationalProgramError> {
        let invalid = |detail: &str| RelationalProgramError::InvalidProgram(detail.into());
        let nodes = epoch
            .relation(&ProgrammaticRelationId::new("fact.code_syntax_node"))
            .ok_or_else(|| invalid("syntax outline requires canonical CST facts"))?;
        let source = nodes.contract.logical_schema();
        let node_field = |name: &str| -> Result<FieldId, RelationalProgramError> {
            let index = source
                .index_of(name)
                .map_err(datafusion::common::DataFusionError::from)?;
            let id = nodes
                .contract
                .field_id_at(SchemaRole::Logical, index)
                .map_err(|_| invalid("syntax field identity is missing"))?;
            FieldId::new(id)
        };
        let binding = self
            .program_result_binding
            .as_ref()
            .ok_or_else(|| invalid("source outline result schema is absent"))?;
        let result_field =
            |name: &str| FieldId::new(format!("{}.{}", self.relation_id.as_str(), name));
        let bytes = result_field("source_bytes")?;
        let mut ids = Vec::new();
        let mut fields = Vec::new();
        for (id, field) in binding.field_ids().iter().zip(binding.schema().fields()) {
            // Intermediate compiled projections still declare the private byte field. Retain
            // its schema binding, then remove its value before outline expansion and delivery.
            ids.push(id.clone());
            fields.push(Arc::clone(field));
        }
        let anchor_expressions = self
            .program
            .output_fields
            .iter()
            .filter(|id| *id != &bytes)
            .map(|id| NamedExpression {
                field_id: id.clone(),
                expression: E::Field(id.clone()),
            })
            .collect::<Vec<_>>();
        let mut expressions = anchor_expressions.clone();
        for (field, expression) in node_projection(source, &node_field, &result_field)? {
            fields.push(field);
            ids.push(expression.field_id.clone());
            expressions.push(expression);
        }
        let predicates = outline_predicates(&result_field, &node_field)?;
        // CST identities belong to the source context, separately from the semantic anchor's
        // analysis context. Exact workspace/file/digest/generation joins establish correspondence.
        let input = R::Input(crate::relational_program::RelationId::new(
            "fact.code_syntax_node",
        )?);
        self.program.output_fields = expressions
            .iter()
            .map(|field| field.field_id.clone())
            .collect();
        self.program.root = super::result_order::before_order(self.program.root, |root| {
            let anchors = R::Projection {
                input: Box::new(root),
                expressions: anchor_expressions,
            };
            R::Filter {
                input: Box::new(R::Projection {
                    input: Box::new(R::Join {
                        left: Box::new(anchors),
                        right: Box::new(input),
                        kind: JoinKind::Inner,
                        predicates,
                    }),
                    expressions,
                }),
                predicate: E::SourceAccessCheck { parameters },
            }
        });
        let order = [
            "syntax_start_byte",
            "syntax_depth",
            "syntax_ordinal",
            "syntax_node_id",
        ]
        .into_iter()
        .map(|name| {
            Ok(SortExpression {
                expression: E::Field(result_field(name)?),
                ascending: true,
                nulls_first: false,
            })
        })
        .collect::<Result<Vec<_>, RelationalProgramError>>()?;
        super::result_order::append(&mut self.program.root, &order);
        self.program_result_binding = Some(SupplementalProgramRelationBinding::try_new(
            self.relation_id.clone(),
            binding.table_reference().clone(),
            Arc::new(Schema::new(fields)),
            ids,
            binding.authority_pin(),
        )?);
        Ok(self)
    }
}

fn outline_predicates(
    result_field: &impl Fn(&str) -> Result<FieldId, RelationalProgramError>,
    node_field: &impl Fn(&str) -> Result<FieldId, RelationalProgramError>,
) -> Result<Vec<E>, RelationalProgramError> {
    let mut predicates = [
        "workspace_id",
        "file_id",
        "content_digest",
        "source_generation",
        "language",
    ]
    .into_iter()
    .map(|name| {
        Ok(E::Call {
            operator: O::Equal,
            arguments: vec![E::Field(result_field(name)?), E::Field(node_field(name)?)],
        })
    })
    .collect::<Result<Vec<_>, RelationalProgramError>>()?;
    for (name, operator) in [
        ("start_byte", O::LessThanOrEqual),
        ("end_byte", O::GreaterThanOrEqual),
    ] {
        predicates.push(E::Call {
            operator,
            arguments: vec![E::Field(result_field(name)?), E::Field(node_field(name)?)],
        });
    }
    Ok(predicates)
}

fn node_projection(
    source: &Schema,
    node_field: &impl Fn(&str) -> Result<FieldId, RelationalProgramError>,
    result_field: &impl Fn(&str) -> Result<FieldId, RelationalProgramError>,
) -> Result<Vec<(Arc<Field>, NamedExpression)>, RelationalProgramError> {
    let mut columns = Vec::new();
    for (input, output) in [
        ("public_entity_id", "syntax_node_id"),
        ("parent_entity_id", "syntax_parent_id"),
        ("context_id", "syntax_context_id"),
        ("provider_context_id", "syntax_provider_context_id"),
        ("provider", "syntax_provider"),
        ("provider_run_id", "syntax_provider_run_id"),
        ("raw_kind", "syntax_raw_kind"),
        ("normalized_kind_code", "syntax_normalized_kind_code"),
        ("field_name", "syntax_field_name"),
        ("ordinal", "syntax_ordinal"),
        ("depth", "syntax_depth"),
        ("start_byte", "syntax_start_byte"),
        ("end_byte", "syntax_end_byte"),
        ("named", "syntax_named"),
        ("extra", "syntax_extra"),
        ("error", "syntax_error"),
        ("missing", "syntax_missing"),
    ] {
        let source_field = source
            .field_with_name(input)
            .map_err(datafusion::common::DataFusionError::from)?;
        let id = result_field(output)?;
        let parent = input == "parent_entity_id";
        let field = Arc::new(Field::new(
            output,
            if parent {
                DataType::Utf8
            } else {
                source_field.data_type().clone()
            },
            source_field.is_nullable(),
        ));
        columns.push((
            field,
            NamedExpression {
                field_id: id,
                expression: if parent {
                    E::PublicEntityId {
                        arguments: vec![
                            E::Field(node_field(input)?),
                            E::Literal(ScalarValue::Utf8(Some("syntax-node".into()))),
                        ],
                    }
                } else {
                    E::Field(node_field(input)?)
                },
            },
        ));
    }
    Ok(columns)
}
