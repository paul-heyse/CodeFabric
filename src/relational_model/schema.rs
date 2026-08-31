use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};

use super::ModelError;

/// Scalar storage types required by the closed bootstrap metamodel.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ScalarType {
    Bool,
    UInt32,
    UInt64,
    Utf8,
    Binary,
}

impl ScalarType {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::UInt32 => "u32",
            Self::UInt64 => "u64",
            Self::Utf8 => "utf8",
            Self::Binary => "binary",
        }
    }

    #[must_use]
    pub const fn arrow_data_type(self) -> DataType {
        match self {
            Self::Bool => DataType::Boolean,
            Self::UInt32 => DataType::UInt32,
            Self::UInt64 => DataType::UInt64,
            Self::Utf8 => DataType::Utf8,
            Self::Binary => DataType::Binary,
        }
    }
}

/// Typed model cell. `Null` is accepted only for nullable fields.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ModelValue {
    Null,
    Bool(bool),
    UInt32(u32),
    UInt64(u64),
    Utf8(String),
    Binary(Vec<u8>),
}

impl ModelValue {
    #[must_use]
    pub const fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Null => None,
            Self::Bool(_) => Some(ScalarType::Bool),
            Self::UInt32(_) => Some(ScalarType::UInt32),
            Self::UInt64(_) => Some(ScalarType::UInt64),
            Self::Utf8(_) => Some(ScalarType::Utf8),
            Self::Binary(_) => Some(ScalarType::Binary),
        }
    }

    pub(crate) fn as_utf8(&self) -> Option<&str> {
        match self {
            Self::Utf8(value) => Some(value),
            _ => None,
        }
    }
}

impl From<&str> for ModelValue {
    fn from(value: &str) -> Self {
        Self::Utf8(value.to_owned())
    }
}

impl From<String> for ModelValue {
    fn from(value: String) -> Self {
        Self::Utf8(value)
    }
}

impl From<bool> for ModelValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<u32> for ModelValue {
    fn from(value: u32) -> Self {
        Self::UInt32(value)
    }
}

impl From<u64> for ModelValue {
    fn from(value: u64) -> Self {
        Self::UInt64(value)
    }
}

impl From<Vec<u8>> for ModelValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Binary(value)
    }
}

/// The fixed model-relation families from D-20 plus installer-derived primitives.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ModelRelation {
    ModelEpoch,
    ModelMigration,
    ModelDecision,
    SemanticType,
    Relation,
    Field,
    Key,
    ForeignKey,
    AuthorityRule,
    NormalizationRule,
    UnknownRule,
    Derivation,
    DerivationInput,
    DerivationOutput,
    Program,
    ProgramStep,
    StepInput,
    StepOutput,
    PrimitiveBinding,
    QueryForm,
    Phrase,
    PhraseBinding,
    ResultRole,
    Policy,
    Invariant,
    Oracle,
    CapabilityRequirement,
    Projection,
    Representation,
    PublicSymbol,
    StateMachine,
    State,
    Transition,
    MaterializationPolicy,
    PhysicalBinding,
    IntrinsicPrimitive,
}

impl ModelRelation {
    pub const ALL: [Self; 36] = [
        Self::ModelEpoch,
        Self::ModelMigration,
        Self::ModelDecision,
        Self::SemanticType,
        Self::Relation,
        Self::Field,
        Self::Key,
        Self::ForeignKey,
        Self::AuthorityRule,
        Self::NormalizationRule,
        Self::UnknownRule,
        Self::Derivation,
        Self::DerivationInput,
        Self::DerivationOutput,
        Self::Program,
        Self::ProgramStep,
        Self::StepInput,
        Self::StepOutput,
        Self::PrimitiveBinding,
        Self::QueryForm,
        Self::Phrase,
        Self::PhraseBinding,
        Self::ResultRole,
        Self::Policy,
        Self::Invariant,
        Self::Oracle,
        Self::CapabilityRequirement,
        Self::Projection,
        Self::Representation,
        Self::PublicSymbol,
        Self::StateMachine,
        Self::State,
        Self::Transition,
        Self::MaterializationPolicy,
        Self::PhysicalBinding,
        Self::IntrinsicPrimitive,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelEpoch => "model_epoch",
            Self::ModelMigration => "model_migration",
            Self::ModelDecision => "model_decision",
            Self::SemanticType => "semantic_type",
            Self::Relation => "relation",
            Self::Field => "field",
            Self::Key => "key",
            Self::ForeignKey => "foreign_key",
            Self::AuthorityRule => "authority_rule",
            Self::NormalizationRule => "normalization_rule",
            Self::UnknownRule => "unknown_rule",
            Self::Derivation => "derivation",
            Self::DerivationInput => "derivation_input",
            Self::DerivationOutput => "derivation_output",
            Self::Program => "program",
            Self::ProgramStep => "program_step",
            Self::StepInput => "step_input",
            Self::StepOutput => "step_output",
            Self::PrimitiveBinding => "primitive_binding",
            Self::QueryForm => "query_form",
            Self::Phrase => "phrase",
            Self::PhraseBinding => "phrase_binding",
            Self::ResultRole => "result_role",
            Self::Policy => "policy",
            Self::Invariant => "invariant",
            Self::Oracle => "oracle",
            Self::CapabilityRequirement => "capability_requirement",
            Self::Projection => "projection",
            Self::Representation => "representation",
            Self::PublicSymbol => "public_symbol",
            Self::StateMachine => "state_machine",
            Self::State => "state",
            Self::Transition => "transition",
            Self::MaterializationPolicy => "materialization_policy",
            Self::PhysicalBinding => "physical_binding",
            Self::IntrinsicPrimitive => "intrinsic_primitive",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.as_str() == name)
    }
}

/// One field in a bootstrap model relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldSpec {
    name: &'static str,
    scalar_type: ScalarType,
    nullable: bool,
    semantic_role: &'static str,
}

impl FieldSpec {
    const fn new(
        name: &'static str,
        scalar_type: ScalarType,
        nullable: bool,
        semantic_role: &'static str,
    ) -> Self {
        Self {
            name,
            scalar_type,
            nullable,
            semantic_role,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn scalar_type(&self) -> ScalarType {
        self.scalar_type
    }

    #[must_use]
    pub const fn nullable(&self) -> bool {
        self.nullable
    }

    #[must_use]
    pub const fn semantic_role(&self) -> &'static str {
        self.semantic_role
    }
}

/// Logical contract for one bootstrap relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationSpec {
    relation: ModelRelation,
    fields: Vec<FieldSpec>,
    primary_key: Vec<&'static str>,
}

impl RelationSpec {
    fn new(relation: ModelRelation, primary_key: &[&'static str], fields: Vec<FieldSpec>) -> Self {
        Self {
            relation,
            fields,
            primary_key: primary_key.to_vec(),
        }
    }

    #[must_use]
    pub const fn relation(&self) -> ModelRelation {
        self.relation
    }

    #[must_use]
    pub fn fields(&self) -> &[FieldSpec] {
        &self.fields
    }

    #[must_use]
    pub fn primary_key(&self) -> &[&'static str] {
        &self.primary_key
    }

    #[must_use]
    pub fn arrow_schema(&self) -> SchemaRef {
        let fields = self.fields.iter().map(|field| {
            let metadata = HashMap::from([
                (
                    "codefabric.model_relation".to_owned(),
                    self.relation.as_str().to_owned(),
                ),
                (
                    "codefabric.semantic_role".to_owned(),
                    field.semantic_role.to_owned(),
                ),
            ]);
            Field::new(
                field.name,
                field.scalar_type.arrow_data_type(),
                field.nullable,
            )
            .with_metadata(metadata)
        });
        let metadata = HashMap::from([
            ("codefabric.schema_role".to_owned(), "model".to_owned()),
            (
                "codefabric.model_relation".to_owned(),
                self.relation.as_str().to_owned(),
            ),
        ]);
        Arc::new(Schema::new_with_metadata(
            fields.collect::<Vec<_>>(),
            metadata,
        ))
    }
}

/// The only hard-coded model schema. It is checked against its replayed description.
#[derive(Clone, Debug)]
pub struct BootstrapMetamodel {
    specs: BTreeMap<ModelRelation, RelationSpec>,
}

impl BootstrapMetamodel {
    #[must_use]
    pub fn new() -> Self {
        let specs = ModelRelation::ALL
            .into_iter()
            .map(|relation| (relation, relation_spec(relation)))
            .collect();
        Self { specs }
    }

    #[must_use]
    pub fn relations(&self) -> impl ExactSizeIterator<Item = ModelRelation> + '_ {
        self.specs.keys().copied()
    }

    #[must_use]
    pub fn relation_spec(&self, relation: ModelRelation) -> &RelationSpec {
        &self.specs[&relation]
    }

    pub(crate) fn validate_row(&self, row: &ModelRow) -> Result<(), ModelError> {
        let spec = self.relation_spec(row.relation);
        let expected: BTreeSet<_> = spec.fields.iter().map(|field| field.name).collect();
        let observed: BTreeSet<_> = row.values.keys().map(String::as_str).collect();
        if expected != observed {
            return Err(ModelError::InvalidRow {
                relation: row.relation.as_str().to_owned(),
                message: format!(
                    "field closure differs: missing={:?}, extra={:?}",
                    expected.difference(&observed).collect::<Vec<_>>(),
                    observed.difference(&expected).collect::<Vec<_>>()
                ),
            });
        }
        for field in &spec.fields {
            let value = &row.values[field.name];
            if matches!(value, ModelValue::Null) {
                if field.nullable {
                    continue;
                }
                return Err(ModelError::InvalidRow {
                    relation: row.relation.as_str().to_owned(),
                    message: format!("non-null field {} is null", field.name),
                });
            }
            if value.scalar_type() != Some(field.scalar_type) {
                return Err(ModelError::InvalidRow {
                    relation: row.relation.as_str().to_owned(),
                    message: format!(
                        "field {} expects {}, observed {:?}",
                        field.name,
                        field.scalar_type.id(),
                        value.scalar_type()
                    ),
                });
            }
        }
        self.row_key(row).map(|_| ())
    }

    pub(crate) fn row_key(&self, row: &ModelRow) -> Result<Vec<ModelValue>, ModelError> {
        self.relation_spec(row.relation)
            .primary_key
            .iter()
            .map(|name| {
                let value = row
                    .values
                    .get(*name)
                    .ok_or_else(|| ModelError::InvalidRow {
                        relation: row.relation.as_str().to_owned(),
                        message: format!("primary-key field {name} is absent"),
                    })?;
                if matches!(value, ModelValue::Null) {
                    return Err(ModelError::InvalidRow {
                        relation: row.relation.as_str().to_owned(),
                        message: format!("primary-key field {name} is null"),
                    });
                }
                Ok(value.clone())
            })
            .collect()
    }
}

impl Default for BootstrapMetamodel {
    fn default() -> Self {
        Self::new()
    }
}

/// One fully typed row in a bootstrap relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRow {
    relation: ModelRelation,
    values: BTreeMap<String, ModelValue>,
}

impl ModelRow {
    #[must_use]
    pub const fn relation(&self) -> ModelRelation {
        self.relation
    }

    #[must_use]
    pub fn value(&self, field: &str) -> Option<&ModelValue> {
        self.values.get(field)
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<String, ModelValue> {
        &self.values
    }
}

/// Typed construction path; unchecked model rows are not exposed.
#[derive(Clone, Debug)]
pub struct ModelRowBuilder {
    relation: ModelRelation,
    values: BTreeMap<String, ModelValue>,
}

impl ModelRowBuilder {
    #[must_use]
    pub fn new(relation: ModelRelation) -> Self {
        Self {
            relation,
            values: BTreeMap::new(),
        }
    }

    pub fn value(
        mut self,
        field: impl Into<String>,
        value: impl Into<ModelValue>,
    ) -> Result<Self, ModelError> {
        let field = field.into();
        if self.values.insert(field.clone(), value.into()).is_some() {
            return Err(ModelError::InvalidRow {
                relation: self.relation.as_str().to_owned(),
                message: format!("duplicate field {field}"),
            });
        }
        Ok(self)
    }

    pub fn null(self, field: impl Into<String>) -> Result<Self, ModelError> {
        self.value(field, ModelValue::Null)
    }

    pub fn build(self, metamodel: &BootstrapMetamodel) -> Result<ModelRow, ModelError> {
        let row = ModelRow {
            relation: self.relation,
            values: self.values,
        };
        metamodel.validate_row(&row)?;
        Ok(row)
    }
}

fn f(
    name: &'static str,
    scalar_type: ScalarType,
    nullable: bool,
    semantic_role: &'static str,
) -> FieldSpec {
    FieldSpec::new(name, scalar_type, nullable, semantic_role)
}

fn id(name: &'static str) -> FieldSpec {
    f(name, ScalarType::Utf8, false, "canonical-id")
}

fn reference(name: &'static str, nullable: bool) -> FieldSpec {
    f(name, ScalarType::Utf8, nullable, "reference")
}

fn text(name: &'static str, nullable: bool) -> FieldSpec {
    f(name, ScalarType::Utf8, nullable, "semantic-text")
}

#[allow(clippy::too_many_lines)]
fn relation_spec(relation: ModelRelation) -> RelationSpec {
    let (primary_key, fields): (&[&str], Vec<FieldSpec>) = match relation {
        ModelRelation::ModelEpoch => (
            &["model_epoch_id"],
            vec![
                id("model_epoch_id"),
                reference("predecessor_model_epoch_id", true),
                reference("compiler_release_id", false),
                f("migration_ordinal", ScalarType::UInt64, false, "sequence"),
            ],
        ),
        ModelRelation::ModelMigration => (
            &["migration_id"],
            vec![
                id("migration_id"),
                reference("predecessor_migration_id", true),
                reference("target_model_epoch_id", false),
                f("ordinal", ScalarType::UInt64, false, "sequence"),
                reference("compiler_release_id", false),
                text("accepted_by", false),
            ],
        ),
        ModelRelation::ModelDecision => (
            &["decision_id"],
            vec![
                id("decision_id"),
                reference("migration_id", false),
                text("owner", false),
                text("applicability", false),
                text("rationale", false),
            ],
        ),
        ModelRelation::SemanticType => (
            &["semantic_type_id"],
            vec![
                id("semantic_type_id"),
                text("name", false),
                text("logical_type", false),
                f("allows_null", ScalarType::Bool, false, "constraint"),
            ],
        ),
        ModelRelation::Relation => (
            &["relation_id"],
            vec![
                id("relation_id"),
                text("schema_name", false),
                text("relation_name", false),
                text("semantic_role", false),
            ],
        ),
        ModelRelation::Field => (
            &["field_id"],
            vec![
                id("field_id"),
                reference("relation_id", false),
                text("field_name", false),
                reference("semantic_type_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                f("nullable", ScalarType::Bool, false, "constraint"),
                text("semantic_role", false),
            ],
        ),
        ModelRelation::Key => (
            &["key_id", "ordinal"],
            vec![
                id("key_id"),
                reference("relation_id", false),
                reference("field_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                text("key_kind", false),
            ],
        ),
        ModelRelation::ForeignKey => (
            &["foreign_key_id", "ordinal"],
            vec![
                id("foreign_key_id"),
                reference("source_relation_id", false),
                reference("source_field_id", false),
                reference("target_relation_id", false),
                reference("target_field_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                text("on_missing", false),
            ],
        ),
        ModelRelation::AuthorityRule => (
            &["authority_rule_id"],
            vec![
                id("authority_rule_id"),
                text("fact_family", false),
                text("provider_id", false),
                f("precedence", ScalarType::UInt32, false, "ordering"),
                reference("guard_program_id", true),
                text("conflict_policy", false),
            ],
        ),
        ModelRelation::NormalizationRule => (
            &["normalization_rule_id"],
            vec![
                id("normalization_rule_id"),
                reference("input_relation_id", false),
                reference("output_relation_id", false),
                reference("program_id", false),
            ],
        ),
        ModelRelation::UnknownRule => (
            &["unknown_rule_id"],
            vec![
                id("unknown_rule_id"),
                text("fact_family", false),
                reference("capability_requirement_id", false),
                text("unknown_kind", false),
            ],
        ),
        ModelRelation::Derivation => (
            &["derivation_id"],
            vec![
                id("derivation_id"),
                reference("program_id", false),
                reference("materialization_policy_id", true),
                text("producer_authority", false),
            ],
        ),
        ModelRelation::DerivationInput => (
            &["derivation_id", "ordinal"],
            vec![
                reference("derivation_id", false),
                reference("input_relation_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                f("required", ScalarType::Bool, false, "constraint"),
            ],
        ),
        ModelRelation::DerivationOutput => (
            &["derivation_id", "ordinal"],
            vec![
                reference("derivation_id", false),
                reference("output_relation_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
            ],
        ),
        ModelRelation::Program => (
            &["program_id"],
            vec![
                id("program_id"),
                text("name", false),
                text("program_kind", false),
                reference("result_semantic_type_id", true),
            ],
        ),
        ModelRelation::ProgramStep => (
            &["step_id"],
            vec![
                id("step_id"),
                reference("program_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                reference("primitive_binding_id", false),
            ],
        ),
        ModelRelation::StepInput => (
            &["step_id", "ordinal"],
            vec![
                reference("step_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                text("source_kind", false),
                reference("source_id", false),
                reference("semantic_type_id", false),
            ],
        ),
        ModelRelation::StepOutput => (
            &["step_id", "ordinal"],
            vec![
                reference("step_id", false),
                f("ordinal", ScalarType::UInt32, false, "sequence"),
                id("binding_id"),
                reference("semantic_type_id", false),
            ],
        ),
        ModelRelation::PrimitiveBinding => (
            &["primitive_binding_id"],
            vec![
                id("primitive_binding_id"),
                reference("primitive_id", false),
                reference("program_id", false),
                reference("semantic_parameter_relation_id", true),
            ],
        ),
        ModelRelation::QueryForm => (
            &["query_form_id"],
            vec![
                id("query_form_id"),
                text("name", false),
                reference("resolver_program_id", false),
                reference("result_role_id", false),
            ],
        ),
        ModelRelation::Phrase => (
            &["phrase_id"],
            vec![
                id("phrase_id"),
                text("locale", false),
                text("phrase", false),
            ],
        ),
        ModelRelation::PhraseBinding => (
            &["phrase_binding_id"],
            vec![
                id("phrase_binding_id"),
                reference("phrase_id", false),
                reference("query_form_id", false),
                text("parameter_name", true),
                text("bound_value", true),
            ],
        ),
        ModelRelation::ResultRole => (
            &["result_role_id"],
            vec![
                id("result_role_id"),
                reference("relation_id", false),
                reference("projection_id", true),
                text("cardinality", false),
            ],
        ),
        ModelRelation::Policy => (
            &["policy_id"],
            vec![
                id("policy_id"),
                text("scope", false),
                reference("program_id", false),
                f("unknown_fails_closed", ScalarType::Bool, false, "policy"),
            ],
        ),
        ModelRelation::Invariant => (
            &["invariant_id"],
            vec![
                id("invariant_id"),
                reference("scope_relation_id", false),
                reference("program_id", false),
                text("terminal_semantics", false),
            ],
        ),
        ModelRelation::Oracle => (
            &["oracle_id"],
            vec![
                id("oracle_id"),
                reference("invariant_id", true),
                reference("expectation_relation_id", false),
                reference("violation_relation_id", false),
                text("owner", false),
                text("independence_class", false),
            ],
        ),
        ModelRelation::CapabilityRequirement => (
            &["capability_requirement_id"],
            vec![
                id("capability_requirement_id"),
                reference("consumer_id", false),
                text("capability_id", false),
                text("minimum_status", false),
            ],
        ),
        ModelRelation::Projection => (
            &["projection_id"],
            vec![
                id("projection_id"),
                reference("source_relation_id", false),
                reference("program_id", false),
                text("visibility", false),
            ],
        ),
        ModelRelation::Representation => (
            &["representation_id"],
            vec![
                id("representation_id"),
                reference("semantic_type_id", false),
                text("arrow_data_type", false),
                text("storage_encoding", false),
                text("metadata_class", false),
                text("extension_name", true),
                text("extension_metadata", true),
            ],
        ),
        ModelRelation::PublicSymbol => (
            &["public_symbol_id"],
            vec![
                id("public_symbol_id"),
                text("symbol_kind", false),
                text("released_name", false),
                reference("target_id", false),
                text("wire_version", false),
            ],
        ),
        ModelRelation::StateMachine => (
            &["state_machine_id"],
            vec![id("state_machine_id"), text("name", false)],
        ),
        ModelRelation::State => (
            &["state_id"],
            vec![
                id("state_id"),
                reference("state_machine_id", false),
                text("name", false),
                f("terminal", ScalarType::Bool, false, "state"),
            ],
        ),
        ModelRelation::Transition => (
            &["transition_id"],
            vec![
                id("transition_id"),
                reference("state_machine_id", false),
                reference("from_state_id", false),
                reference("to_state_id", false),
                text("command_kind", false),
                reference("guard_program_id", true),
            ],
        ),
        ModelRelation::MaterializationPolicy => (
            &["materialization_policy_id"],
            vec![
                id("materialization_policy_id"),
                reference("relation_id", false),
                text("posture", false),
                reference("decision_program_id", false),
                reference("measurement_relation_id", false),
            ],
        ),
        ModelRelation::PhysicalBinding => (
            &["physical_binding_id"],
            vec![
                id("physical_binding_id"),
                reference("logical_relation_id", false),
                reference("storage_relation_id", false),
                reference("mapping_program_id", false),
                text("compatibility_mode", false),
            ],
        ),
        ModelRelation::IntrinsicPrimitive => (
            &["primitive_id"],
            vec![
                id("primitive_id"),
                text("signature", false),
                text("semantic_level", false),
                text("implementation_id", false),
                text("package_id", false),
                reference("compiler_release_id", false),
            ],
        ),
    };
    RelationSpec::new(relation, primary_key, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_is_closed_and_every_schema_has_semantic_metadata() {
        let bootstrap = BootstrapMetamodel::new();
        assert_eq!(bootstrap.relations().count(), ModelRelation::ALL.len());
        for relation in bootstrap.relations() {
            let spec = bootstrap.relation_spec(relation);
            assert!(!spec.fields().is_empty());
            assert!(!spec.primary_key().is_empty());
            let schema = spec.arrow_schema();
            assert_eq!(schema.metadata()["codefabric.schema_role"], "model");
            assert_eq!(
                schema.metadata()["codefabric.model_relation"],
                relation.as_str()
            );
            assert!(schema.fields().iter().all(|field| {
                field.metadata().contains_key("codefabric.semantic_role")
                    && field.metadata()["codefabric.model_relation"] == relation.as_str()
            }));
        }
    }

    #[test]
    fn typed_row_builder_rejects_missing_wrong_and_duplicate_fields() {
        let bootstrap = BootstrapMetamodel::new();
        let missing = ModelRowBuilder::new(ModelRelation::StateMachine)
            .value("state_machine_id", "machine")
            .unwrap()
            .build(&bootstrap);
        assert!(matches!(missing, Err(ModelError::InvalidRow { .. })));

        let wrong = ModelRowBuilder::new(ModelRelation::StateMachine)
            .value("state_machine_id", "machine")
            .unwrap()
            .value("name", 7_u32)
            .unwrap()
            .build(&bootstrap);
        assert!(matches!(wrong, Err(ModelError::InvalidRow { .. })));

        let duplicate = ModelRowBuilder::new(ModelRelation::StateMachine)
            .value("state_machine_id", "machine")
            .unwrap()
            .value("state_machine_id", "again");
        assert!(matches!(duplicate, Err(ModelError::InvalidRow { .. })));
    }
}
