use std::collections::BTreeMap;

use super::{ModelError, ModelValue, require_identifier};

/// One row in a model-defined relation.
///
/// Bootstrap rows describe the schema of these relations.  Their data remains
/// part of the replayed model rather than entering through an adjacent Rust
/// registry or a checked-in Arrow bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDataRow {
    relation_id: String,
    values: BTreeMap<String, ModelValue>,
}

impl ModelDataRow {
    #[must_use]
    pub fn relation_id(&self) -> &str {
        &self.relation_id
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

/// Construction path for a row whose schema is itself replayed model data.
///
/// Identifier and duplicate-field checks happen here.  Exact field/type/key
/// validation happens when the reducer applies the row against the model
/// schema active at that point in the migration chain.
#[derive(Clone, Debug)]
pub struct ModelDataRowBuilder {
    relation_id: String,
    values: BTreeMap<String, ModelValue>,
}

impl ModelDataRowBuilder {
    pub fn new(relation_id: impl Into<String>) -> Result<Self, ModelError> {
        let relation_id = relation_id.into();
        require_identifier(&relation_id, "model data relation")?;
        Ok(Self {
            relation_id,
            values: BTreeMap::new(),
        })
    }

    pub fn value(
        mut self,
        field: impl Into<String>,
        value: impl Into<ModelValue>,
    ) -> Result<Self, ModelError> {
        let field = field.into();
        require_identifier(&field, "model data field")?;
        if self.values.insert(field.clone(), value.into()).is_some() {
            return Err(ModelError::InvalidRow {
                relation: self.relation_id,
                message: format!("duplicate field {field}"),
            });
        }
        Ok(self)
    }

    pub fn null(self, field: impl Into<String>) -> Result<Self, ModelError> {
        self.value(field, ModelValue::Null)
    }

    pub fn build(self) -> Result<ModelDataRow, ModelError> {
        if self.values.is_empty() {
            return Err(ModelError::InvalidRow {
                relation: self.relation_id,
                message: "model data row has no fields".to_owned(),
            });
        }
        Ok(ModelDataRow {
            relation_id: self.relation_id,
            values: self.values,
        })
    }
}

/// Exact primary-key reference to one model-defined relation row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDataRowReference {
    relation_id: String,
    key: Vec<ModelValue>,
}

impl ModelDataRowReference {
    pub fn new(
        relation_id: impl Into<String>,
        key: impl IntoIterator<Item = ModelValue>,
    ) -> Result<Self, ModelError> {
        let relation_id = relation_id.into();
        require_identifier(&relation_id, "model data relation")?;
        let key = key.into_iter().collect::<Vec<_>>();
        if key.is_empty() || key.iter().any(|value| matches!(value, ModelValue::Null)) {
            return Err(ModelError::OperationRejected(format!(
                "{relation_id} model data reference has an empty or null key"
            )));
        }
        Ok(Self { relation_id, key })
    }

    #[must_use]
    pub fn relation_id(&self) -> &str {
        &self.relation_id
    }

    #[must_use]
    pub fn key(&self) -> &[ModelValue] {
        &self.key
    }
}
