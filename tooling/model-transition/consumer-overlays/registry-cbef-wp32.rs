//! Reviewed WP32 consumer shape compiled only against staged model-generated bindings.
#![allow(dead_code)]

#[path = "../../../src/generated/model_identity_recipes.rs"]
mod identity_recipes;
#[path = "../../../src/generated/model_registries.rs"]
mod registries;

use identity_recipes::{
    EntityFields, RecipeRecord, RecipeValue, RelationFactFields, entity, relation_fact,
};
use registries::{
    OccurrenceFamily, ProviderNodeFlags, RangeReconciliationStep, RawKindDisposition,
};

/// Named occurrence identity inputs after the WP32 transition.
pub struct SourceOccurrenceIdentityFields {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub entity_kind_code: u16,
    pub owner_id: [u8; 16],
    pub semantic_key: Vec<u8>,
}

/// Construct the released five-field ENTITY recipe through its generated API.
pub fn source_occurrence_identity(
    fields: SourceOccurrenceIdentityFields,
) -> Result<RecipeRecord, identity_recipes::RecipeError> {
    entity(EntityFields {
        workspace_id: RecipeValue::Id(fields.workspace_id),
        analysis_context_id: RecipeValue::Id(fields.analysis_context_id),
        kind_code: RecipeValue::Unsigned(fields.entity_kind_code.to_be_bytes().to_vec()),
        owner_id: RecipeValue::Id(fields.owner_id),
        semantic_key: RecipeValue::Bytes(fields.semantic_key),
    })
}

/// Named relation identity inputs after the WP32 transition.
pub struct SourceRelationIdentityFields {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub relation_kind_code: u16,
    pub subject_entity_id: [u8; 16],
    pub object_entity_id: [u8; 16],
    pub role: Option<String>,
}

/// Construct the released six-field RELATION_FACT recipe through its generated API.
pub fn source_relation_identity(
    fields: SourceRelationIdentityFields,
) -> Result<RecipeRecord, identity_recipes::RecipeError> {
    let role = fields.role.map_or_else(
        || RecipeValue::TaggedUnion(0, Box::new(RecipeValue::Absent)),
        |role| RecipeValue::TaggedUnion(1, Box::new(RecipeValue::Utf8(role))),
    );
    relation_fact(RelationFactFields {
        workspace_id: RecipeValue::Id(fields.workspace_id),
        analysis_context_id: RecipeValue::Id(fields.analysis_context_id),
        relation_kind_code: RecipeValue::Unsigned(
            fields.relation_kind_code.to_be_bytes().to_vec(),
        ),
        subject_entity_id: RecipeValue::Id(fields.subject_entity_id),
        object_entity_id: RecipeValue::Id(fields.object_entity_id),
        role,
    })
}

/// Dedicated typed syntax fields that replace undocumented provider flag packing.
pub struct SourceSyntaxGovernedFields {
    pub occurrence_family: OccurrenceFamily,
    pub reconciliation_step: Option<RangeReconciliationStep>,
    pub raw_kind_disposition: RawKindDisposition,
    pub provider_node_flags: ProviderNodeFlags,
    pub error: bool,
    pub missing: bool,
    pub explicitly_parenthesized: bool,
}

/// A representative syntax projection keeps all three syntax observations as typed columns.
pub fn syntax_projection() -> SourceSyntaxGovernedFields {
    SourceSyntaxGovernedFields {
        occurrence_family: OccurrenceFamily::Syntax,
        reconciliation_step: Some(RangeReconciliationStep::SmallestEnclosingCompatible),
        raw_kind_disposition: RawKindDisposition::Normalize,
        provider_node_flags: ProviderNodeFlags::empty(),
        error: true,
        missing: false,
        explicitly_parenthesized: true,
    }
}
