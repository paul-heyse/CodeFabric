//! Request-owned Arrow relations installed as exact query-local DataFusion inputs.
//!
//! The epoch-bound semantic compiler emits typed handoffs rather than JSON values or a
//! process-wide temporary catalog. This module verifies the compiler's exact content pin,
//! independently revalidates the handoff contract, materializes one immutable Arrow batch, and
//! retains the exact provider capability used by the resulting logical scan.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroUsize;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use datafusion::catalog::TableProvider;
use datafusion::common::TableReference;
use datafusion::datasource::{MemTable, provider_as_source, source_as_provider};
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};

use crate::relational_program::{
    FieldId, RelationId, RelationInput, RelationalProgramError, SupplementalProgramRelationBinding,
};
use crate::relational_semantic_query::{
    CompiledEpochBoundRequestInputHandoff, EpochBoundRequestInputField, EpochBoundRequestInputRow,
    SemanticClauseValue, SemanticValueKind,
};
use crate::schema_contract::{FIELD_ID_METADATA_KEY, RELATION_ID_METADATA_KEY};

/// Domain separator used by the epoch-bound compiler for request-input content pins.
pub const REQUEST_INPUT_CONTENT_PIN_DOMAIN: &[u8] = b"epoch-bound-request-input-handoff";

const REQUEST_INPUT_PROGRAM_AUTHORITY_DOMAIN: &[u8] =
    b"codefabric.request-owned-relation.program-authority.v1";

/// Explicit allocation bounds for request-owned Arrow relations.
///
/// There is deliberately no default. Configuration ingress must name every bound, and this type
/// rejects zero before any request-owned data is inspected or allocated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestOwnedRelationLimits {
    max_relations: NonZeroUsize,
    max_rows_per_relation: NonZeroUsize,
    max_fields_per_relation: NonZeroUsize,
    max_cells_per_relation: NonZeroUsize,
    max_total_rows: NonZeroUsize,
    max_total_cells: NonZeroUsize,
    max_total_text_bytes: NonZeroUsize,
}

impl RequestOwnedRelationLimits {
    /// Construct a complete non-zero request-owned relation envelope.
    ///
    /// # Errors
    ///
    /// Rejects any zero bound.
    pub fn try_new(
        max_relations: usize,
        max_rows_per_relation: usize,
        max_fields_per_relation: usize,
        max_cells_per_relation: usize,
        max_total_rows: usize,
        max_total_cells: usize,
        max_total_text_bytes: usize,
    ) -> Result<Self, RequestOwnedRelationError> {
        Ok(Self {
            max_relations: nonzero(max_relations, "max_relations")?,
            max_rows_per_relation: nonzero(max_rows_per_relation, "max_rows_per_relation")?,
            max_fields_per_relation: nonzero(max_fields_per_relation, "max_fields_per_relation")?,
            max_cells_per_relation: nonzero(max_cells_per_relation, "max_cells_per_relation")?,
            max_total_rows: nonzero(max_total_rows, "max_total_rows")?,
            max_total_cells: nonzero(max_total_cells, "max_total_cells")?,
            max_total_text_bytes: nonzero(max_total_text_bytes, "max_total_text_bytes")?,
        })
    }

    #[must_use]
    pub const fn max_relations(self) -> usize {
        self.max_relations.get()
    }

    #[must_use]
    pub const fn max_rows_per_relation(self) -> usize {
        self.max_rows_per_relation.get()
    }

    #[must_use]
    pub const fn max_fields_per_relation(self) -> usize {
        self.max_fields_per_relation.get()
    }

    #[must_use]
    pub const fn max_cells_per_relation(self) -> usize {
        self.max_cells_per_relation.get()
    }

    #[must_use]
    pub const fn max_total_rows(self) -> usize {
        self.max_total_rows.get()
    }

    #[must_use]
    pub const fn max_total_cells(self) -> usize {
        self.max_total_cells.get()
    }

    #[must_use]
    pub const fn max_total_text_bytes(self) -> usize {
        self.max_total_text_bytes.get()
    }
}

/// Exact pins retained alongside one provider capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestOwnedRelationAuthority {
    execution_program_pin: [u8; 32],
    handoff_pin: [u8; 32],
    content_pin: [u8; 32],
}

impl RequestOwnedRelationAuthority {
    #[must_use]
    pub const fn execution_program_pin(self) -> [u8; 32] {
        self.execution_program_pin
    }

    #[must_use]
    pub const fn handoff_pin(self) -> [u8; 32] {
        self.handoff_pin
    }

    #[must_use]
    pub const fn content_pin(self) -> [u8; 32] {
        self.content_pin
    }

    /// Derive one supplemental-binding pin from all compiler handoff authorities.
    ///
    /// The supplemental binding separately frames the exact relation, table, Arrow schema, and
    /// stable field identities, so this pin has the narrow job of retaining the three request
    /// authority inputs without collapsing any of them.
    #[must_use]
    pub fn program_binding_authority_pin(self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hash_part(&mut hasher, REQUEST_INPUT_PROGRAM_AUTHORITY_DOMAIN);
        hash_part(&mut hasher, b"execution-program-pin");
        hash_part(&mut hasher, &self.execution_program_pin);
        hash_part(&mut hasher, b"handoff-pin");
        hash_part(&mut hasher, &self.handoff_pin);
        hash_part(&mut hasher, b"content-pin");
        hash_part(&mut hasher, &self.content_pin);
        *hasher.finalize().as_bytes()
    }
}

/// One verified request-owned Arrow relation and its exact scan capability.
#[derive(Clone)]
pub struct MaterializedRequestOwnedRelation {
    query_id: Arc<str>,
    program_binding_id: Arc<str>,
    input_id: Arc<str>,
    relation_id: RelationId,
    table_reference: TableReference,
    fields: Arc<[EpochBoundRequestInputField]>,
    schema: SchemaRef,
    batch: RecordBatch,
    provider: Arc<dyn TableProvider>,
    input: RelationInput,
    authority: RequestOwnedRelationAuthority,
    cell_count: usize,
    text_bytes: usize,
}

impl std::fmt::Debug for MaterializedRequestOwnedRelation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MaterializedRequestOwnedRelation")
            .field("query_id", &self.query_id)
            .field("program_binding_id", &self.program_binding_id)
            .field("input_id", &self.input_id)
            .field("relation_id", &self.relation_id)
            .field("fields", &self.fields)
            .field("rows", &self.batch.num_rows())
            .field("authority", &self.authority)
            .field("cell_count", &self.cell_count)
            .field("text_bytes", &self.text_bytes)
            .finish_non_exhaustive()
    }
}

impl MaterializedRequestOwnedRelation {
    #[must_use]
    pub const fn query_id(&self) -> &Arc<str> {
        &self.query_id
    }

    #[must_use]
    pub const fn program_binding_id(&self) -> &Arc<str> {
        &self.program_binding_id
    }

    #[must_use]
    pub const fn input_id(&self) -> &Arc<str> {
        &self.input_id
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    /// Exact query-local scan name retained by this direct logical input.
    #[must_use]
    pub const fn table_reference(&self) -> &TableReference {
        &self.table_reference
    }

    #[must_use]
    pub fn fields(&self) -> &[EpochBoundRequestInputField] {
        &self.fields
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn batch(&self) -> &RecordBatch {
        &self.batch
    }

    /// Return the exact provider capability embedded in the logical scan.
    #[must_use]
    pub const fn provider_capability(&self) -> &Arc<dyn TableProvider> {
        &self.provider
    }

    #[must_use]
    pub const fn authority(&self) -> RequestOwnedRelationAuthority {
        self.authority
    }

    /// Clone the query-local compiler input while retaining this object for authority checks.
    #[must_use]
    pub fn relation_input(&self) -> RelationInput {
        self.input.clone()
    }

    /// Build an immutable compiler binding for this exact request-owned relation.
    ///
    /// This does not register the provider in an epoch or child catalog. The binding carries the
    /// exact table name, Arrow schema, stable field identities, and a derived pin that frames the
    /// execution-program, compiler-handoff, and content pins.
    ///
    /// # Errors
    ///
    /// Fails closed if the verified schema/field contract cannot be represented by the
    /// relational compiler's supplemental binding contract.
    pub fn supplemental_program_binding(
        &self,
    ) -> Result<SupplementalProgramRelationBinding, RelationalProgramError> {
        SupplementalProgramRelationBinding::try_new(
            self.relation_id.clone(),
            self.table_reference.clone(),
            Arc::clone(&self.schema),
            self.fields
                .iter()
                .map(|field| field.field_id.clone())
                .collect::<Vec<FieldId>>(),
            self.authority.program_binding_authority_pin(),
        )
    }

    /// Prove that an input is the direct scan created from this exact provider capability.
    ///
    /// This check is intentionally stronger than schema equality. It is the narrow hook a child
    /// planner can use before accepting a request-local input into its authorized plan closure.
    ///
    /// # Errors
    ///
    /// Rejects relation identity, direct-scan shape, provider pointer, or Arrow schema drift.
    pub fn validate_exact_input(
        &self,
        input: &RelationInput,
    ) -> Result<(), RequestOwnedRelationError> {
        if input.relation_id != self.relation_id {
            return Err(RequestOwnedRelationError::RelationInputAuthority {
                relation_id: self.relation_id.as_str().to_owned(),
                detail: "relation identity differs".to_owned(),
            });
        }
        let LogicalPlan::TableScan(scan) = &input.plan else {
            return Err(RequestOwnedRelationError::RelationInputAuthority {
                relation_id: self.relation_id.as_str().to_owned(),
                detail: "input root is not a direct table scan".to_owned(),
            });
        };
        let observed = source_as_provider(&scan.source).map_err(|error| {
            RequestOwnedRelationError::RelationInputAuthority {
                relation_id: self.relation_id.as_str().to_owned(),
                detail: format!("scan source is not a provider capability: {error}"),
            }
        })?;
        if !Arc::ptr_eq(&observed, &self.provider) {
            return Err(RequestOwnedRelationError::RelationInputAuthority {
                relation_id: self.relation_id.as_str().to_owned(),
                detail: "scan retains a different provider capability".to_owned(),
            });
        }
        if input.plan.schema().as_arrow() != self.schema.as_ref() {
            return Err(RequestOwnedRelationError::RelationInputAuthority {
                relation_id: self.relation_id.as_str().to_owned(),
                detail: "scan schema differs from the verified Arrow schema".to_owned(),
            });
        }
        Ok(())
    }
}

/// Bounded, duplicate-free request-local input relation set.
#[derive(Debug)]
pub struct RequestOwnedRelationCollection {
    relations: BTreeMap<RelationId, MaterializedRequestOwnedRelation>,
    total_rows: usize,
    total_cells: usize,
    total_text_bytes: usize,
}

impl RequestOwnedRelationCollection {
    /// Validate and materialize all compiled handoffs under one explicit resource envelope.
    ///
    /// # Errors
    ///
    /// Rejects resource overrun, duplicate relation or request identities, invalid contracts,
    /// content-pin drift, Arrow construction failure, or DataFusion planning failure.
    pub fn try_materialize(
        handoffs: impl IntoIterator<Item = CompiledEpochBoundRequestInputHandoff>,
        limits: RequestOwnedRelationLimits,
    ) -> Result<Self, RequestOwnedRelationError> {
        let mut relations = BTreeMap::new();
        let mut request_keys = BTreeSet::new();
        let mut total_rows = 0_usize;
        let mut total_cells = 0_usize;
        let mut total_text_bytes = 0_usize;
        for (index, handoff) in handoffs.into_iter().enumerate() {
            let observed_relations =
                index
                    .checked_add(1)
                    .ok_or(RequestOwnedRelationError::SizeOverflow(
                        "request-owned relation count",
                    ))?;
            enforce_limit("max_relations", observed_relations, limits.max_relations())?;
            let request_key = (Arc::clone(&handoff.query_id), Arc::clone(&handoff.input_id));
            if !request_keys.insert(request_key.clone()) {
                return Err(RequestOwnedRelationError::DuplicateRequestInput {
                    query_id: request_key.0.to_string(),
                    input_id: request_key.1.to_string(),
                });
            }
            if relations.contains_key(&handoff.relation_id) {
                return Err(RequestOwnedRelationError::DuplicateRelation(
                    handoff.relation_id.as_str().to_owned(),
                ));
            }

            let materialized = materialize_relation(handoff, limits)?;
            total_rows = checked_add(
                "total request-owned rows",
                total_rows,
                materialized.batch.num_rows(),
            )?;
            total_cells = checked_add(
                "total request-owned cells",
                total_cells,
                materialized.cell_count,
            )?;
            total_text_bytes = checked_add(
                "total request-owned text bytes",
                total_text_bytes,
                materialized.text_bytes,
            )?;
            enforce_limit("max_total_rows", total_rows, limits.max_total_rows())?;
            enforce_limit("max_total_cells", total_cells, limits.max_total_cells())?;
            enforce_limit(
                "max_total_text_bytes",
                total_text_bytes,
                limits.max_total_text_bytes(),
            )?;
            relations.insert(materialized.relation_id.clone(), materialized);
        }
        Ok(Self {
            relations,
            total_rows,
            total_cells,
            total_text_bytes,
        })
    }

    #[must_use]
    pub fn get(&self, relation_id: &RelationId) -> Option<&MaterializedRequestOwnedRelation> {
        self.relations.get(relation_id)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MaterializedRequestOwnedRelation> {
        self.relations.values()
    }

    #[must_use]
    pub fn relation_inputs(&self) -> Vec<RelationInput> {
        self.relations
            .values()
            .map(MaterializedRequestOwnedRelation::relation_input)
            .collect()
    }

    /// Build deterministic supplemental compiler bindings without mutating epoch authority.
    ///
    /// Iteration follows canonical relation-identity order from the collection's `BTreeMap`.
    ///
    /// # Errors
    ///
    /// Fails closed if any verified request relation cannot be represented by the relational
    /// compiler's supplemental binding contract.
    pub fn supplemental_program_bindings(
        &self,
    ) -> Result<Vec<SupplementalProgramRelationBinding>, RelationalProgramError> {
        self.relations
            .values()
            .map(MaterializedRequestOwnedRelation::supplemental_program_binding)
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    #[must_use]
    pub const fn total_rows(&self) -> usize {
        self.total_rows
    }

    #[must_use]
    pub const fn total_cells(&self) -> usize {
        self.total_cells
    }

    #[must_use]
    pub const fn total_text_bytes(&self) -> usize {
        self.total_text_bytes
    }
}

/// Recompute the exact content pin emitted by the epoch-bound semantic compiler.
///
/// The framing is two length-prefixed byte strings: the compiler domain followed by the `Debug`
/// rendering of `(relation_id, &fields, &rows)`. Keeping this function handoff-shaped avoids
/// silently hashing a normalized or reordered Arrow representation instead.
#[must_use]
pub fn compiler_request_input_content_pin(
    handoff: &CompiledEpochBoundRequestInputHandoff,
) -> [u8; 32] {
    let rendered = format!(
        "{:?}",
        &(handoff.relation_id.clone(), &handoff.fields, &handoff.rows)
    );
    let mut hasher = blake3::Hasher::new();
    hash_part(&mut hasher, REQUEST_INPUT_CONTENT_PIN_DOMAIN);
    hash_part(&mut hasher, rendered.as_bytes());
    *hasher.finalize().as_bytes()
}

fn materialize_relation(
    handoff: CompiledEpochBoundRequestInputHandoff,
    limits: RequestOwnedRelationLimits,
) -> Result<MaterializedRequestOwnedRelation, RequestOwnedRelationError> {
    validate_identity("query", &handoff.query_id)?;
    validate_identity("program binding", &handoff.program_binding_id)?;
    validate_identity("request input", &handoff.input_id)?;
    validate_identity("relation", handoff.relation_id.as_str())?;
    validate_pin("execution_program_pin", handoff.execution_program_pin)?;
    validate_pin("handoff_pin", handoff.handoff_pin)?;
    validate_pin("content_pin", handoff.content_pin)?;
    let expected_content_pin = compiler_request_input_content_pin(&handoff);
    if handoff.content_pin != expected_content_pin {
        return Err(RequestOwnedRelationError::ContentPinMismatch {
            relation_id: handoff.relation_id.as_str().to_owned(),
            declared: handoff.content_pin,
            computed: expected_content_pin,
        });
    }

    if handoff.fields.is_empty() {
        return Err(RequestOwnedRelationError::EmptyFieldContract(
            handoff.relation_id.as_str().to_owned(),
        ));
    }
    enforce_limit(
        "max_fields_per_relation",
        handoff.fields.len(),
        limits.max_fields_per_relation(),
    )?;
    enforce_limit(
        "max_rows_per_relation",
        handoff.rows.len(),
        limits.max_rows_per_relation(),
    )?;
    let cell_count = checked_mul(
        "request-owned relation cells",
        handoff.rows.len(),
        handoff.fields.len(),
    )?;
    enforce_limit(
        "max_cells_per_relation",
        cell_count,
        limits.max_cells_per_relation(),
    )?;

    let mut declared_fields = BTreeMap::new();
    for field in &handoff.fields {
        validate_identity("request input field", field.field_id.as_str())?;
        if declared_fields
            .insert(field.field_id.clone(), field)
            .is_some()
        {
            return Err(RequestOwnedRelationError::DuplicateFieldContract {
                relation_id: handoff.relation_id.as_str().to_owned(),
                field_id: field.field_id.as_str().to_owned(),
            });
        }
    }

    let mut row_ids = BTreeSet::new();
    let mut ordinals = BTreeSet::new();
    let mut text_bytes = 0_usize;
    for (expected_ordinal, row) in handoff.rows.iter().enumerate() {
        validate_identity("request input row", &row.row_id)?;
        if row.query_id != handoff.query_id || row.input_id != handoff.input_id {
            return Err(RequestOwnedRelationError::RowOwnershipMismatch {
                relation_id: handoff.relation_id.as_str().to_owned(),
                row_id: row.row_id.to_string(),
            });
        }
        if !row_ids.insert(Arc::clone(&row.row_id)) {
            return Err(RequestOwnedRelationError::DuplicateRowId {
                relation_id: handoff.relation_id.as_str().to_owned(),
                row_id: row.row_id.to_string(),
            });
        }
        if !ordinals.insert(row.ordinal) {
            return Err(RequestOwnedRelationError::DuplicateRowOrdinal {
                relation_id: handoff.relation_id.as_str().to_owned(),
                ordinal: row.ordinal,
            });
        }
        let actual_ordinal = usize::try_from(row.ordinal).map_err(|_| {
            RequestOwnedRelationError::NonContiguousRowOrdinal {
                relation_id: handoff.relation_id.as_str().to_owned(),
                expected: expected_ordinal,
                actual: usize::MAX,
            }
        })?;
        if actual_ordinal != expected_ordinal {
            return Err(RequestOwnedRelationError::NonContiguousRowOrdinal {
                relation_id: handoff.relation_id.as_str().to_owned(),
                expected: expected_ordinal,
                actual: actual_ordinal,
            });
        }
        if row.fields.is_empty() {
            return Err(RequestOwnedRelationError::EmptyRow {
                relation_id: handoff.relation_id.as_str().to_owned(),
                row_id: row.row_id.to_string(),
            });
        }

        let mut observed_fields = BTreeSet::new();
        for value in &row.fields {
            validate_identity("request input row field", value.field_id.as_str())?;
            if !observed_fields.insert(value.field_id.clone()) {
                return Err(RequestOwnedRelationError::DuplicateRowField {
                    relation_id: handoff.relation_id.as_str().to_owned(),
                    row_id: row.row_id.to_string(),
                    field_id: value.field_id.as_str().to_owned(),
                });
            }
            let Some(field) = declared_fields.get(&value.field_id) else {
                return Err(RequestOwnedRelationError::UndeclaredField {
                    relation_id: handoff.relation_id.as_str().to_owned(),
                    row_id: row.row_id.to_string(),
                    field_id: value.field_id.as_str().to_owned(),
                });
            };
            let actual = semantic_kind(&value.value);
            if actual != field.value_kind {
                return Err(RequestOwnedRelationError::ValueKindMismatch {
                    relation_id: handoff.relation_id.as_str().to_owned(),
                    row_id: row.row_id.to_string(),
                    field_id: value.field_id.as_str().to_owned(),
                    expected: field.value_kind,
                    actual,
                });
            }
            if let SemanticClauseValue::Text(text) = &value.value {
                text_bytes = checked_add("request-owned text bytes", text_bytes, text.len())?;
                enforce_limit(
                    "max_total_text_bytes",
                    text_bytes,
                    limits.max_total_text_bytes(),
                )?;
            }
        }
        for field in &handoff.fields {
            if field.required && !observed_fields.contains(&field.field_id) {
                return Err(RequestOwnedRelationError::MissingRequiredField {
                    relation_id: handoff.relation_id.as_str().to_owned(),
                    row_id: row.row_id.to_string(),
                    field_id: field.field_id.as_str().to_owned(),
                });
            }
        }
    }

    let schema = request_owned_schema(&handoff.relation_id, &handoff.fields);
    let arrays = handoff
        .fields
        .iter()
        .map(|field| materialize_field(field, &handoff.rows))
        .collect::<Result<Vec<_>, _>>()?;
    let batch = RecordBatch::try_new(Arc::clone(&schema), arrays)?;
    let provider: Arc<dyn TableProvider> = Arc::new(MemTable::try_new(
        Arc::clone(&schema),
        vec![vec![batch.clone()]],
    )?);
    let table_reference = TableReference::bare(handoff.relation_id.as_str());
    let plan = LogicalPlanBuilder::scan(
        table_reference.clone(),
        provider_as_source(Arc::clone(&provider)),
        None,
    )?
    .build()?;
    let input = RelationInput {
        relation_id: handoff.relation_id.clone(),
        plan,
    };
    let materialized = MaterializedRequestOwnedRelation {
        query_id: handoff.query_id,
        program_binding_id: handoff.program_binding_id,
        input_id: handoff.input_id,
        relation_id: handoff.relation_id,
        table_reference,
        fields: Arc::from(handoff.fields),
        schema,
        batch,
        provider,
        input,
        authority: RequestOwnedRelationAuthority {
            execution_program_pin: handoff.execution_program_pin,
            handoff_pin: handoff.handoff_pin,
            content_pin: handoff.content_pin,
        },
        cell_count,
        text_bytes,
    };
    materialized.validate_exact_input(&materialized.input)?;
    Ok(materialized)
}

fn request_owned_schema(
    relation_id: &RelationId,
    fields: &[EpochBoundRequestInputField],
) -> SchemaRef {
    let fields = fields
        .iter()
        .map(|field| {
            Field::new(
                field.field_id.as_str(),
                arrow_type(field.value_kind),
                !field.required,
            )
            .with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                field.field_id.as_str().to_owned(),
            )]))
        })
        .collect::<Vec<_>>();
    Arc::new(Schema::new_with_metadata(
        fields,
        HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            relation_id.as_str().to_owned(),
        )]),
    ))
}

fn materialize_field(
    field: &EpochBoundRequestInputField,
    rows: &[EpochBoundRequestInputRow],
) -> Result<ArrayRef, RequestOwnedRelationError> {
    let values = rows
        .iter()
        .map(|row| {
            row.fields
                .iter()
                .find(|value| value.field_id == field.field_id)
                .map(|value| &value.value)
        })
        .collect::<Vec<_>>();
    let array: ArrayRef = match field.value_kind {
        SemanticValueKind::Boolean => Arc::new(BooleanArray::from_iter(values.into_iter().map(
            |value| match value {
                Some(SemanticClauseValue::Boolean(value)) => Some(*value),
                None => None,
                _ => unreachable!("value kinds were validated before Arrow construction"),
            },
        ))),
        SemanticValueKind::Int64 => Arc::new(Int64Array::from_iter(values.into_iter().map(
            |value| match value {
                Some(SemanticClauseValue::Int64(value)) => Some(*value),
                None => None,
                _ => unreachable!("value kinds were validated before Arrow construction"),
            },
        ))),
        SemanticValueKind::UInt64 => Arc::new(UInt64Array::from_iter(values.into_iter().map(
            |value| match value {
                Some(SemanticClauseValue::UInt64(value)) => Some(*value),
                None => None,
                _ => unreachable!("value kinds were validated before Arrow construction"),
            },
        ))),
        SemanticValueKind::Text => Arc::new(StringArray::from_iter(values.into_iter().map(
            |value| match value {
                Some(SemanticClauseValue::Text(value)) => Some(value.as_ref()),
                None => None,
                _ => unreachable!("value kinds were validated before Arrow construction"),
            },
        ))),
    };
    Ok(array)
}

const fn arrow_type(kind: SemanticValueKind) -> DataType {
    match kind {
        SemanticValueKind::Boolean => DataType::Boolean,
        SemanticValueKind::Int64 => DataType::Int64,
        SemanticValueKind::UInt64 => DataType::UInt64,
        SemanticValueKind::Text => DataType::Utf8,
    }
}

const fn semantic_kind(value: &SemanticClauseValue) -> SemanticValueKind {
    match value {
        SemanticClauseValue::Boolean(_) => SemanticValueKind::Boolean,
        SemanticClauseValue::Int64(_) => SemanticValueKind::Int64,
        SemanticClauseValue::UInt64(_) => SemanticValueKind::UInt64,
        SemanticClauseValue::Text(_) => SemanticValueKind::Text,
    }
}

fn validate_identity(kind: &'static str, value: &str) -> Result<(), RequestOwnedRelationError> {
    if value.is_empty()
        || value.len() > 1_024
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(RequestOwnedRelationError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_pin(kind: &'static str, pin: [u8; 32]) -> Result<(), RequestOwnedRelationError> {
    if pin == [0; 32] {
        return Err(RequestOwnedRelationError::MissingPin(kind));
    }
    Ok(())
}

fn nonzero(value: usize, limit: &'static str) -> Result<NonZeroUsize, RequestOwnedRelationError> {
    NonZeroUsize::new(value).ok_or(RequestOwnedRelationError::ZeroLimit(limit))
}

fn enforce_limit(
    limit: &'static str,
    observed: usize,
    maximum: usize,
) -> Result<(), RequestOwnedRelationError> {
    if observed > maximum {
        return Err(RequestOwnedRelationError::Limit {
            limit,
            observed,
            maximum,
        });
    }
    Ok(())
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, RequestOwnedRelationError> {
    left.checked_add(right)
        .ok_or(RequestOwnedRelationError::SizeOverflow(resource))
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, RequestOwnedRelationError> {
    left.checked_mul(right)
        .ok_or(RequestOwnedRelationError::SizeOverflow(resource))
}

fn hash_part(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

/// Fail-closed request-owned relation validation or construction failure.
#[derive(Debug, thiserror::Error)]
pub enum RequestOwnedRelationError {
    #[error("request-owned relation resource limit {0} must be non-zero")]
    ZeroLimit(&'static str),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("required request-owned relation pin {0} is absent")]
    MissingPin(&'static str),
    #[error(
        "request-owned relation {relation_id} content pin differs: declared {declared:02x?}, computed {computed:02x?}"
    )]
    ContentPinMismatch {
        relation_id: String,
        declared: [u8; 32],
        computed: [u8; 32],
    },
    #[error("request-owned relation {0} has an empty field contract")]
    EmptyFieldContract(String),
    #[error("request-owned relation {relation_id} repeats field contract {field_id}")]
    DuplicateFieldContract {
        relation_id: String,
        field_id: String,
    },
    #[error("request-owned relation {relation_id} row {row_id} belongs to another query or input")]
    RowOwnershipMismatch { relation_id: String, row_id: String },
    #[error("request-owned relation {relation_id} repeats row identity {row_id}")]
    DuplicateRowId { relation_id: String, row_id: String },
    #[error("request-owned relation {relation_id} repeats row ordinal {ordinal}")]
    DuplicateRowOrdinal { relation_id: String, ordinal: u32 },
    #[error(
        "request-owned relation {relation_id} row ordinal is not contiguous: expected {expected}, actual {actual}"
    )]
    NonContiguousRowOrdinal {
        relation_id: String,
        expected: usize,
        actual: usize,
    },
    #[error("request-owned relation {relation_id} row {row_id} has no typed fields")]
    EmptyRow { relation_id: String, row_id: String },
    #[error("request-owned relation {relation_id} row {row_id} repeats field {field_id}")]
    DuplicateRowField {
        relation_id: String,
        row_id: String,
        field_id: String,
    },
    #[error(
        "request-owned relation {relation_id} row {row_id} supplies undeclared field {field_id}"
    )]
    UndeclaredField {
        relation_id: String,
        row_id: String,
        field_id: String,
    },
    #[error(
        "request-owned relation {relation_id} row {row_id} field {field_id} has kind {actual:?}; expected {expected:?}"
    )]
    ValueKindMismatch {
        relation_id: String,
        row_id: String,
        field_id: String,
        expected: SemanticValueKind,
        actual: SemanticValueKind,
    },
    #[error("request-owned relation {relation_id} row {row_id} omits required field {field_id}")]
    MissingRequiredField {
        relation_id: String,
        row_id: String,
        field_id: String,
    },
    #[error("request-owned relation repeats relation identity {0}")]
    DuplicateRelation(String),
    #[error("request-owned relation repeats query/input identity {query_id}/{input_id}")]
    DuplicateRequestInput { query_id: String, input_id: String },
    #[error("request-owned relation exceeds {limit}: observed {observed}, maximum {maximum}")]
    Limit {
        limit: &'static str,
        observed: usize,
        maximum: usize,
    },
    #[error("request-owned relation size overflow while counting {0}")]
    SizeOverflow(&'static str),
    #[error("request-owned relation {relation_id} input authority failed: {detail}")]
    RelationInputAuthority { relation_id: String, detail: String },
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
}

#[cfg(test)]
mod tests {
    use arrow_array::Array as _;
    use datafusion::datasource::source_as_provider;

    use super::*;
    use crate::relational_program::FieldId;
    use crate::relational_semantic_query::{
        EpochBoundRequestInputFieldValue, EpochBoundRequestInputRow,
    };

    fn field(value: &str) -> FieldId {
        FieldId::new(value).unwrap()
    }

    fn relation(value: &str) -> RelationId {
        RelationId::new(value).unwrap()
    }

    fn limits() -> RequestOwnedRelationLimits {
        RequestOwnedRelationLimits::try_new(4, 16, 8, 128, 32, 256, 4_096).unwrap()
    }

    fn handoff(relation_id: &str) -> CompiledEpochBoundRequestInputHandoff {
        let mut handoff = CompiledEpochBoundRequestInputHandoff {
            query_id: Arc::from("query.entities"),
            program_binding_id: Arc::from("program.find-entities"),
            execution_program_pin: [6; 32],
            input_id: Arc::from("input.within"),
            relation_id: relation(relation_id),
            fields: vec![
                EpochBoundRequestInputField {
                    field_id: field("request.entity_id"),
                    value_kind: SemanticValueKind::Text,
                    required: true,
                },
                EpochBoundRequestInputField {
                    field_id: field("request.include_unknown"),
                    value_kind: SemanticValueKind::Boolean,
                    required: false,
                },
                EpochBoundRequestInputField {
                    field_id: field("request.rank"),
                    value_kind: SemanticValueKind::UInt64,
                    required: true,
                },
            ],
            rows: vec![
                EpochBoundRequestInputRow {
                    query_id: Arc::from("query.entities"),
                    input_id: Arc::from("input.within"),
                    row_id: Arc::from("row.0"),
                    ordinal: 0,
                    fields: vec![
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.entity_id"),
                            value: SemanticClauseValue::Text(Arc::from("entity:first")),
                        },
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.include_unknown"),
                            value: SemanticClauseValue::Boolean(true),
                        },
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.rank"),
                            value: SemanticClauseValue::UInt64(1),
                        },
                    ],
                },
                EpochBoundRequestInputRow {
                    query_id: Arc::from("query.entities"),
                    input_id: Arc::from("input.within"),
                    row_id: Arc::from("row.1"),
                    ordinal: 1,
                    fields: vec![
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.entity_id"),
                            value: SemanticClauseValue::Text(Arc::from("entity:second")),
                        },
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.rank"),
                            value: SemanticClauseValue::UInt64(2),
                        },
                    ],
                },
            ],
            handoff_pin: [7; 32],
            content_pin: [1; 32],
        };
        handoff.content_pin = compiler_request_input_content_pin(&handoff);
        handoff
    }

    #[test]
    fn materializes_typed_arrow_nulls_and_retains_exact_scan_capability() {
        let collection =
            RequestOwnedRelationCollection::try_materialize([handoff("request.within")], limits())
                .unwrap();
        let relation_id = relation("request.within");
        let materialized = collection.get(&relation_id).unwrap();

        assert_eq!(collection.len(), 1);
        assert_eq!(collection.total_rows(), 2);
        assert_eq!(collection.total_cells(), 6);
        assert_eq!(materialized.authority().execution_program_pin(), [6; 32]);
        assert_eq!(materialized.authority().handoff_pin(), [7; 32]);
        assert_eq!(
            materialized.schema().metadata()[RELATION_ID_METADATA_KEY],
            "request.within"
        );
        assert_eq!(
            materialized.schema().field(1).data_type(),
            &DataType::Boolean
        );
        assert!(materialized.schema().field(1).is_nullable());

        let optional = materialized
            .batch()
            .column(1)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap();
        assert!(optional.value(0));
        assert!(optional.is_null(1));

        let input = materialized.relation_input();
        materialized.validate_exact_input(&input).unwrap();
        let LogicalPlan::TableScan(scan) = &input.plan else {
            panic!("request-owned relation must be a direct scan");
        };
        let observed = source_as_provider(&scan.source).unwrap();
        assert!(Arc::ptr_eq(&observed, materialized.provider_capability()));
    }

    #[test]
    fn changed_row_without_a_new_compiler_pin_is_causally_rejected() {
        let mut changed = handoff("request.within");
        changed.rows[0].fields[0].value = SemanticClauseValue::Text(Arc::from("entity:changed"));
        let error = RequestOwnedRelationCollection::try_materialize([changed], limits())
            .expect_err("content drift must fail before Arrow construction");
        assert!(matches!(
            error,
            RequestOwnedRelationError::ContentPinMismatch { .. }
        ));
    }

    #[test]
    fn rejects_wrong_kind_missing_required_field_and_non_contiguous_ordinal() {
        let mut wrong_kind = handoff("request.wrong-kind");
        wrong_kind.rows[0].fields[2].value = SemanticClauseValue::Int64(1);
        wrong_kind.content_pin = compiler_request_input_content_pin(&wrong_kind);
        assert!(matches!(
            RequestOwnedRelationCollection::try_materialize([wrong_kind], limits()),
            Err(RequestOwnedRelationError::ValueKindMismatch { .. })
        ));

        let mut missing = handoff("request.missing");
        missing.rows[0]
            .fields
            .retain(|value| value.field_id.as_str() != "request.entity_id");
        missing.content_pin = compiler_request_input_content_pin(&missing);
        assert!(matches!(
            RequestOwnedRelationCollection::try_materialize([missing], limits()),
            Err(RequestOwnedRelationError::MissingRequiredField { .. })
        ));

        let mut ordinal = handoff("request.ordinal");
        ordinal.rows[1].ordinal = 3;
        ordinal.content_pin = compiler_request_input_content_pin(&ordinal);
        assert!(matches!(
            RequestOwnedRelationCollection::try_materialize([ordinal], limits()),
            Err(RequestOwnedRelationError::NonContiguousRowOrdinal { .. })
        ));
    }

    #[test]
    fn rejects_zero_limits_duplicate_relations_and_total_resource_overrun() {
        assert!(matches!(
            RequestOwnedRelationLimits::try_new(0, 1, 1, 1, 1, 1, 1),
            Err(RequestOwnedRelationError::ZeroLimit("max_relations"))
        ));

        let first = handoff("request.duplicate");
        let mut second = handoff("request.duplicate");
        second.query_id = Arc::from("query.other");
        second.input_id = Arc::from("input.other");
        for row in &mut second.rows {
            row.query_id = Arc::clone(&second.query_id);
            row.input_id = Arc::clone(&second.input_id);
        }
        second.content_pin = compiler_request_input_content_pin(&second);
        assert!(matches!(
            RequestOwnedRelationCollection::try_materialize([first, second], limits()),
            Err(RequestOwnedRelationError::DuplicateRelation(_))
        ));

        let tight = RequestOwnedRelationLimits::try_new(1, 2, 3, 6, 2, 5, 100).unwrap();
        assert!(matches!(
            RequestOwnedRelationCollection::try_materialize(
                [handoff("request.too-many-cells")],
                tight
            ),
            Err(RequestOwnedRelationError::Limit {
                limit: "max_total_cells",
                ..
            })
        ));
    }
}
