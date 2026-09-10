//! Direct canonical relationships select exact occurrence source, without name/range guesses.

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema};
use datafusion::common::ScalarValue;

use crate::relational_program::{
    FieldId, JoinKind, NamedExpression, RelationId, RelationalExpression as R,
    RelationalProgramError as Error, ScalarExpression as E, ScalarOperator as O, SortExpression,
    SupplementalProgramRelationBinding, UnionKind,
};
use crate::schema_contract::SchemaRole;

use super::super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::super::programmatic_schema::ProgrammaticRelationId;

const DESCRIPTORS: &str = "fact.code_source_context";
const BYTES: &str = "source.exact_source_bytes";
const EXTRA: [(&str, bool); 4] = [
    // Preserve the canonical public-ID field's Arrow nullability; matching does not refine it.
    ("related_subject_id", true),
    ("related_relationship", false),
    ("related_resolution", false),
    ("related_unknown_reason", true),
];

struct Relationship {
    relation: &'static str,
    occurrence: &'static str,
    endpoint: &'static str,
    family: &'static str,
    meaning: &'static str,
}

const RELATIONSHIPS: [Relationship; 5] = [
    Relationship {
        relation: "fact.code_call_site",
        occurrence: "call_site_id",
        endpoint: "caller_entity_id",
        family: "call-targets",
        meaning: "outgoing call occurrence",
    },
    Relationship {
        relation: "fact.code_call_site",
        occurrence: "call_site_id",
        endpoint: "target_entity_id",
        family: "call-targets",
        meaning: "incoming call occurrence",
    },
    Relationship {
        relation: "fact.code_semantic_reference",
        occurrence: "reference_id",
        endpoint: "target_entity_id",
        family: "semantic-references",
        meaning: "semantic reference occurrence",
    },
    Relationship {
        relation: "fact.code_reference",
        occurrence: "reference_id",
        endpoint: "target_entity_id",
        family: "lexical-references",
        meaning: "lexical reference occurrence",
    },
    Relationship {
        relation: "fact.code_import",
        occurrence: "import_id",
        endpoint: "target_entity_id",
        family: "imports",
        meaning: "import occurrence",
    },
];

impl super::SelectedQueryOutput {
    pub(crate) fn with_related_occurrences(
        mut self,
        epoch: &ProgrammaticFabricEpoch,
    ) -> Result<Self, Error> {
        let binding = self.program_result_binding.as_ref().ok_or_else(|| {
            Error::InvalidProgram("related source result schema is absent".into())
        })?;
        let result = |name: &str| FieldId::new(format!("{}.{}", self.relation_id.as_str(), name));
        let mut ids = binding.field_ids().to_vec();
        let mut fields = binding.schema().fields().to_vec();
        for (name, nullable) in EXTRA {
            let id = result(name)?;
            let mut metadata = std::collections::HashMap::from([(
                crate::schema_contract::FIELD_ID_METADATA_KEY.into(),
                id.as_str().into(),
            )]);
            if name == "related_subject_id" {
                // The requested anchor is not the reusable identity of the returned occurrence.
                metadata.insert(
                    crate::schema_contract::SEMANTIC_ROLE_METADATA_KEY.into(),
                    "semantic.source.related-subject".into(),
                );
            }
            fields.push(Arc::new(
                Field::new(name, DataType::Utf8, nullable).with_metadata(metadata),
            ));
            ids.push(id);
        }
        let anchor_fields = [
            "entity_id",
            "public_entity_id",
            "context_id",
            "workspace_id",
            "source_generation",
        ]
        .into_iter()
        .map(|name| {
            let id = result(name)?;
            Ok(NamedExpression {
                field_id: id.clone(),
                expression: E::Field(id),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
        let projections = descriptor_projection(epoch, binding, &result)?;
        self.program.root = super::result_order::try_before_order(self.program.root, |input| {
            let anchors = R::Projection {
                input: Box::new(input),
                expressions: anchor_fields,
            };
            let branches = RELATIONSHIPS
                .iter()
                .map(|relationship| branch(epoch, &anchors, relationship, &projections, &result))
                .collect::<Result<Vec<_>, Error>>()?;
            let related = R::Union {
                inputs: branches,
                kind: UnionKind::Distinct,
            };
            // Distinct operates on narrow descriptors; join whole-file byte buffers only once.
            attach_bytes(epoch, related, &projections, &result)
        })?;
        self.program.output_fields = self
            .program
            .output_fields
            .iter()
            .cloned()
            .chain(
                EXTRA
                    .into_iter()
                    .map(|(name, _)| result(name).expect("bound extra field")),
            )
            .collect();
        let order = EXTRA
            .into_iter()
            .map(|(name, _)| {
                Ok(SortExpression {
                    expression: E::Field(result(name)?),
                    ascending: true,
                    nulls_first: false,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
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

fn field(epoch: &ProgrammaticFabricEpoch, relation: &str, name: &str) -> Result<FieldId, Error> {
    let relation = epoch
        .relation(&ProgrammaticRelationId::new(relation))
        .ok_or_else(|| {
            Error::InvalidProgram("related occurrence relation is unavailable".into())
        })?;
    let index = relation
        .contract
        .logical_schema()
        .index_of(name)
        .map_err(datafusion::common::DataFusionError::from)?;
    let id = relation
        .contract
        .field_id_at(SchemaRole::Logical, index)
        .map_err(|_| Error::InvalidProgram("related occurrence field identity is absent".into()))?;
    FieldId::new(id)
}

fn equal(left: FieldId, right: FieldId) -> E {
    E::Call {
        operator: O::Equal,
        arguments: vec![E::Field(left), E::Field(right)],
    }
}

fn text(value: &str) -> E {
    E::Literal(ScalarValue::Utf8(Some(value.into())))
}

fn descriptor_projection(
    epoch: &ProgrammaticFabricEpoch,
    binding: &SupplementalProgramRelationBinding,
    result: &impl Fn(&str) -> Result<FieldId, Error>,
) -> Result<Vec<NamedExpression>, Error> {
    binding
        .schema()
        .fields()
        .iter()
        .filter(|f| f.name() != "source_bytes")
        .map(|f| {
            Ok(NamedExpression {
                field_id: result(f.name())?,
                expression: if f.name() == "context_kind" {
                    text("related occurrence")
                } else {
                    E::Field(field(epoch, DESCRIPTORS, f.name())?)
                },
            })
        })
        .collect()
}

fn branch(
    epoch: &ProgrammaticFabricEpoch,
    anchors: &R,
    relationship: &Relationship,
    projections: &[NamedExpression],
    result: &impl Fn(&str) -> Result<FieldId, Error>,
) -> Result<R, Error> {
    let edge = |name| field(epoch, relationship.relation, name);
    let descriptor = |name| field(epoch, DESCRIPTORS, name);
    let mut predicates = vec![equal(result("entity_id")?, edge(relationship.endpoint)?)];
    for name in ["context_id", "workspace_id", "source_generation"] {
        predicates.push(equal(result(name)?, edge(name)?));
    }
    let selected = R::Join {
        left: Box::new(anchors.clone()),
        right: Box::new(R::Input(RelationId::new(relationship.relation)?)),
        kind: JoinKind::Inner,
        predicates,
    };
    let descriptors = R::Filter {
        input: Box::new(R::Input(RelationId::new(DESCRIPTORS)?)),
        predicate: E::Call {
            operator: O::And,
            arguments: vec![
                E::Call {
                    operator: O::Equal,
                    arguments: vec![
                        E::Field(descriptor("context_kind")?),
                        text("exact source span"),
                    ],
                },
                E::Call {
                    operator: O::Equal,
                    arguments: vec![
                        E::Field(descriptor("fact_family")?),
                        text(relationship.family),
                    ],
                },
            ],
        },
    };
    let mut predicates = vec![equal(
        edge(relationship.occurrence)?,
        descriptor("entity_id")?,
    )];
    for name in [
        "context_id",
        "workspace_id",
        "source_generation",
        "file_id",
        "content_digest",
    ] {
        predicates.push(equal(edge(name)?, descriptor(name)?));
    }
    let input = R::Join {
        left: Box::new(selected),
        right: Box::new(descriptors),
        kind: JoinKind::Inner,
        predicates,
    };
    let mut expressions = projections.to_vec();
    for (name, expression) in [
        ("related_subject_id", E::Field(result("public_entity_id")?)),
        ("related_relationship", text(relationship.meaning)),
        ("related_resolution", E::Field(edge("resolution")?)),
        ("related_unknown_reason", E::Field(edge("unknown_reason")?)),
    ] {
        expressions.push(NamedExpression {
            field_id: result(name)?,
            expression,
        });
    }
    Ok(R::Projection {
        input: Box::new(input),
        expressions,
    })
}

fn attach_bytes(
    epoch: &ProgrammaticFabricEpoch,
    related: R,
    projections: &[NamedExpression],
    result: &impl Fn(&str) -> Result<FieldId, Error>,
) -> Result<R, Error> {
    let predicates = [
        "workspace_id",
        "file_id",
        "content_digest",
        "source_generation",
    ]
    .into_iter()
    .map(|name| Ok(equal(result(name)?, field(epoch, BYTES, name)?)))
    .collect::<Result<_, Error>>()?;
    let input = R::Join {
        left: Box::new(related),
        right: Box::new(R::Input(RelationId::new(BYTES)?)),
        kind: JoinKind::Inner,
        predicates,
    };
    let mut expressions = projections
        .iter()
        .map(|p| NamedExpression {
            field_id: p.field_id.clone(),
            expression: E::Field(p.field_id.clone()),
        })
        .collect::<Vec<_>>();
    expressions.push(NamedExpression {
        field_id: result("source_bytes")?,
        expression: E::Field(field(epoch, BYTES, "source_bytes")?),
    });
    for (name, _) in EXTRA {
        let field_id = result(name)?;
        expressions.push(NamedExpression {
            expression: E::Field(field_id.clone()),
            field_id,
        });
    }
    Ok(R::Projection {
        input: Box::new(input),
        expressions,
    })
}
