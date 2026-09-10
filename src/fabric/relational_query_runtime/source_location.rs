//! `FindEntities` location scope narrows the compiled result before its explicit limit.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::relational_program::{
    FieldId, JoinKind, RelationId, RelationalExpression as R, ScalarExpression as E,
    ScalarOperator as O,
};
use crate::relational_semantic_query::{SemanticClauseValue, location_predicate};
use crate::semantic_query_contract::SourceLocation;

const RELATION: &str = "fact.code_entity_location";

impl super::SelectedQueryOutput {
    pub(crate) fn with_source_locations(
        mut self,
        locations: &[&SourceLocation],
    ) -> Result<Self, String> {
        if locations.is_empty() {
            return Ok(self);
        }
        if locations.len() > crate::production_query_recipe::RELEASE_SELECTION_MAXIMUM_VALUES {
            return Err("source locations exceed the release operand bound".into());
        }
        let binding = self
            .program_result_binding
            .as_ref()
            .ok_or("source location output schema unavailable")?;
        let output = |name: &str| {
            binding
                .field_ids()
                .iter()
                .position(|field| field.as_str().ends_with(&format!(".{name}")))
                .map(|index| E::Field(binding.field_ids()[index].clone()))
                .ok_or_else(|| format!("source location requires an entity result with {name}"))
        };
        let field =
            |name| FieldId::new(format!("{RELATION}.{name}")).map_err(|error| error.to_string());
        let fields = [
            "relative_path",
            "start_byte",
            "end_byte",
            "start_line",
            "start_column",
            "end_line",
            "end_column",
            "entity_kind",
            "language",
            "raw_kind",
        ]
        .into_iter()
        .map(|name| Ok((Arc::from(name), field(name)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
        let predicates = locations
            .iter()
            .map(|location| {
                let value = SemanticClauseValue::Text(Arc::from(
                    serde_json::to_string(location).map_err(|error| error.to_string())?,
                ));
                location_predicate::expression(&value, &fields)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected = R::Filter {
            input: Box::new(R::Input(
                RelationId::new(RELATION).map_err(|error| error.to_string())?,
            )),
            predicate: super::source_boundary::balanced_union(predicates, |left, right| E::Call {
                operator: O::Or,
                arguments: vec![left, right],
            }),
        };
        let predicates = [
            ("public-entity-id", "public_entity_id"),
            ("analysis-context-id", "context_id"),
        ]
        .into_iter()
        .map(|(left, right)| {
            Ok(E::Call {
                operator: O::Equal,
                arguments: vec![output(left)?, E::Field(field(right)?)],
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
        let narrow = |input| R::Join {
            left: Box::new(input),
            right: Box::new(selected),
            kind: JoinKind::LeftSemi,
            predicates,
        };
        self.program.root = super::result_order::before_order(self.program.root, narrow);
        Ok(self)
    }
}
