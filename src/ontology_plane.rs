//! Generated, normalized ontology-plane batches.
//!
//! Runtime code consumes only compiled Rust values. YAML/JSON authorities are parsed once by the
//! model compiler and never reopened while bootstrapping or serving a workspace.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{
    ArrayRef, BooleanArray, Int16Array, Int32Array, Int64Array, RecordBatch, StringArray,
};
use arrow_schema::{DataType, SchemaRef};

use crate::compiled_ontology::{CompiledAuthority, compiled_ontology};
use crate::fabric::FabricError;
use crate::schema_registry::{
    LogicalType, SemanticAuthority, schema_contract_digest, semantic_type_bindings,
    table_column_contracts, table_spec, table_specs,
};

#[derive(Clone, Debug)]
enum Cell {
    Null,
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    Utf8(String),
    Hash32([u8; 32]),
}

impl Cell {
    fn utf8(value: impl Into<String>) -> Self {
        Self::Utf8(value.into())
    }
}

fn digest(value: &str, table: &str) -> Result<[u8; 32], FabricError> {
    let payload = value
        .strip_prefix("b3:")
        .filter(|payload| payload.len() == 64)
        .ok_or_else(|| FabricError::TableInvariant {
            table: table.to_owned(),
            detail: format!("compiled authority digest has invalid framing: {value}"),
        })?;
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&payload[index * 2..index * 2 + 2], 16).map_err(|_| {
            FabricError::TableInvariant {
                table: table.to_owned(),
                detail: "compiled authority digest has invalid hex".to_owned(),
            }
        })?;
    }
    Ok(result)
}

fn authority_cells(authority: CompiledAuthority, table: &str) -> Result<Vec<Cell>, FabricError> {
    Ok(vec![
        Cell::utf8(authority.authority_id),
        Cell::utf8(authority.authority_version),
        Cell::Hash32(digest(authority.canonical_digest, table)?),
    ])
}

fn schema_authority() -> CompiledAuthority {
    CompiledAuthority {
        authority_id: "codefabric.schema.contract-ir",
        authority_version: crate::schema_registry::ontology_version(),
        canonical_digest: schema_contract_digest(),
        canonical_source_path: "contracts/schema/schema-contract-ir.json",
    }
}

fn push_authority(
    row: &mut Vec<Cell>,
    authority: CompiledAuthority,
    table: &str,
) -> Result<(), FabricError> {
    row.extend(authority_cells(authority, table)?);
    Ok(())
}

fn column_array(
    schema: &SchemaRef,
    column_index: usize,
    rows: &[Vec<Cell>],
    table: &str,
) -> Result<ArrayRef, FabricError> {
    let field = schema.field(column_index);
    let values = rows
        .iter()
        .map(|row| row.get(column_index).unwrap_or(&Cell::Null))
        .collect::<Vec<_>>();
    let wrong = |cell: &Cell| FabricError::TableInvariant {
        table: table.to_owned(),
        detail: format!(
            "ontology builder cell {cell:?} is incompatible with field {} ({})",
            field.name(),
            field.data_type()
        ),
    };
    match field.data_type() {
        DataType::Boolean => Ok(Arc::new(BooleanArray::from(
            values
                .iter()
                .map(|value| match value {
                    Cell::Null => Ok(None),
                    Cell::Bool(value) => Ok(Some(*value)),
                    value => Err(wrong(value)),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        DataType::Int16 => Ok(Arc::new(Int16Array::from(
            values
                .iter()
                .map(|value| match value {
                    Cell::Null => Ok(None),
                    Cell::I16(value) => Ok(Some(*value)),
                    value => Err(wrong(value)),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        DataType::Int32 => Ok(Arc::new(Int32Array::from(
            values
                .iter()
                .map(|value| match value {
                    Cell::Null => Ok(None),
                    Cell::I32(value) => Ok(Some(*value)),
                    value => Err(wrong(value)),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        DataType::Int64 => Ok(Arc::new(Int64Array::from(
            values
                .iter()
                .map(|value| match value {
                    Cell::Null => Ok(None),
                    Cell::I64(value) => Ok(Some(*value)),
                    value => Err(wrong(value)),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        DataType::Utf8 => Ok(Arc::new(StringArray::from(
            values
                .iter()
                .map(|value| match value {
                    Cell::Null => Ok(None),
                    Cell::Utf8(value) => Ok(Some(value.as_str())),
                    value => Err(wrong(value)),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        DataType::FixedSizeBinary(32) => {
            let mut builder = FixedSizeBinaryBuilder::new(32);
            for value in values {
                match value {
                    Cell::Null => builder.append_null(),
                    Cell::Hash32(value) => builder.append_value(value)?,
                    value => return Err(wrong(value)),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        data_type => Err(FabricError::TableInvariant {
            table: table.to_owned(),
            detail: format!("ontology builder does not support {data_type}"),
        }),
    }
}

#[allow(clippy::needless_pass_by_value)] // Callers transfer one completed row set into the batch.
fn batch(table_code: i16, rows: Vec<Vec<Cell>>) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(table_code).ok_or_else(|| FabricError::TableInvariant {
        table: table_code.to_string(),
        detail: "compiled ontology table is absent".to_owned(),
    })?;
    if rows
        .iter()
        .any(|row| row.len() != spec.arrow_schema.fields().len())
    {
        return Err(FabricError::TableInvariant {
            table: spec.name.to_owned(),
            detail: "compiled ontology row width differs from its generated schema".to_owned(),
        });
    }
    let columns = (0..spec.arrow_schema.fields().len())
        .map(|index| column_array(&spec.arrow_schema, index, &rows, spec.name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn logical_type_name(value: LogicalType) -> &'static str {
    match value {
        LogicalType::Id16 => "id16",
        LogicalType::Hash32 => "hash32",
        LogicalType::Code16 => "code16",
        LogicalType::Code32 => "code32",
        LogicalType::Bucket16 => "bucket16",
        LogicalType::Int16 => "int16",
        LogicalType::Int32 => "int32",
        LogicalType::Int64 => "int64",
        LogicalType::UInt64 => "uint64",
        LogicalType::Float64 => "float64",
        LogicalType::Boolean => "boolean",
        LogicalType::Utf8 => "utf8",
        LogicalType::Binary => "binary",
        LogicalType::TimestampUtc => "timestamp_utc",
        LogicalType::IdList => "id_list",
        LogicalType::Int64List => "int64_list",
        LogicalType::StringMap => "string_map",
    }
}

fn semantic_authority_name(value: SemanticAuthority) -> &'static str {
    match value {
        SemanticAuthority::EnumRegistry => "enum_registry",
        SemanticAuthority::TypeAlgebra => "type_algebra",
        SemanticAuthority::OntologyEntityRegistry => "ontology_entity_registry",
        SemanticAuthority::OntologyRelationRegistry => "ontology_relation_registry",
        SemanticAuthority::OntologyPropertyRegistry => "ontology_property_registry",
        SemanticAuthority::OntologyFactRegistry => "ontology_fact_registry",
        SemanticAuthority::CapabilityRegistry => "capability_registry",
        SemanticAuthority::SchemaIr => "schema_ir",
        SemanticAuthority::Intrinsic => "intrinsic",
        SemanticAuthority::ProviderCatalog => "provider_catalog",
        SemanticAuthority::DiagnosticProtocol => "diagnostic_protocol",
    }
}

fn framed(value: &str) -> Result<[u8; 32], FabricError> {
    digest(
        &crate::integrity::framed_digest(value.as_bytes()),
        "ontology_contract",
    )
}

/// Build the exact twenty normalized ontology relations from compiled authority values.
///
/// # Errors
///
/// Returns an invariant error when generated rows and generated schemas diverge.
///
/// # Panics
///
/// Panics only if a generated schema contains more columns than fit in its governed `i32`
/// ordinal, which cannot occur for a validated model pack.
#[allow(clippy::too_many_lines)] // One exhaustive compiler owns all normalized relations.
pub fn ontology_dimension_batches() -> Result<BTreeMap<i16, RecordBatch>, FabricError> {
    let ontology = compiled_ontology();
    let mut result = BTreeMap::new();

    let mut rows = Vec::new();
    for value in ontology.enum_values {
        let mut row = vec![
            Cell::utf8(value.domain),
            Cell::I32(value.code),
            Cell::utf8(value.name),
        ];
        push_authority(&mut row, value.authority, "enum_domain")?;
        rows.push(row);
    }
    result.insert(11, batch(11, rows)?);

    let mut rows = Vec::new();
    for value in ontology.entity_kinds {
        let mut row = vec![
            Cell::I32(value.code),
            Cell::utf8(value.name),
            Cell::I16(value.family_code),
            Cell::utf8(value.language_applicability),
            Cell::Bool(value.query_visible),
        ];
        push_authority(&mut row, value.authority, "entity_kind")?;
        rows.push(row);
    }
    result.insert(12, batch(12, rows)?);

    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for value in ontology.entity_kinds {
        if seen.insert(value.family_code) {
            let name = ontology
                .enum_values
                .iter()
                .find(|entry| {
                    entry.domain == "ENTITY_FAMILY" && entry.code == i32::from(value.family_code)
                })
                .map_or_else(
                    || format!("ENTITY_FAMILY_{}", value.family_code),
                    |entry| entry.name.to_owned(),
                );
            let mut row = vec![Cell::I16(value.family_code), Cell::utf8(name)];
            push_authority(&mut row, value.authority, "entity_family")?;
            rows.push(row);
        }
    }
    result.insert(13, batch(13, rows)?);

    let mut rows = Vec::new();
    for value in ontology.relation_kinds {
        let mut row = vec![
            Cell::I32(value.code),
            Cell::utf8(value.name),
            Cell::I16(value.family_code),
            Cell::utf8(value.cardinality),
            Cell::Bool(value.symmetric),
            Cell::Bool(value.transitive),
            Cell::utf8(value.self_edge_policy),
            Cell::utf8(value.owner_selection_rule),
            Cell::Bool(value.query_visible),
        ];
        push_authority(&mut row, value.authority, "relation_kind")?;
        rows.push(row);
    }
    result.insert(14, batch(14, rows)?);

    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for value in ontology.relation_kinds {
        if seen.insert(value.family_code) {
            let mut row = vec![Cell::I16(value.family_code), Cell::utf8(value.family_name)];
            push_authority(&mut row, value.authority, "relation_family")?;
            rows.push(row);
        }
    }
    result.insert(15, batch(15, rows)?);

    let mut rows = Vec::new();
    for value in ontology.property_kinds {
        let mut row = vec![
            Cell::I32(value.code),
            Cell::utf8(value.name),
            Cell::I16(value.value_kind_code),
            Cell::utf8(value.cardinality),
            Cell::utf8(value.storage_mapping),
        ];
        push_authority(&mut row, value.authority, "property_kind")?;
        rows.push(row);
    }
    result.insert(16, batch(16, rows)?);

    let mut rows = Vec::new();
    for value in ontology.fact_kinds {
        let mut row = vec![
            Cell::I16(value.code),
            Cell::utf8(value.name),
            Cell::utf8(value.fact_form),
        ];
        push_authority(&mut row, value.authority, "fact_kind")?;
        rows.push(row);
    }
    result.insert(17, batch(17, rows)?);

    let mut rows = Vec::new();
    for value in ontology.provider_raw_kinds {
        let mut row = vec![
            Cell::I16(value.provider_code),
            Cell::utf8(value.raw_catalog_id),
            Cell::utf8(value.raw_namespace),
            Cell::I32(value.raw_kind_code),
            Cell::utf8(value.raw_name),
            value.normalized_kind_code.map_or(Cell::Null, Cell::I32),
        ];
        push_authority(&mut row, value.authority, "provider_raw_kind")?;
        rows.push(row);
    }
    result.insert(18, batch(18, rows)?);

    let schema_authority = schema_authority();
    let mut rows = Vec::new();
    for value in crate::schema_registry::id_domains() {
        let mut row = vec![
            Cell::utf8(value.domain_slug),
            Cell::utf8(value.extension_name),
            Cell::utf8(value.preimage_recipe_id),
            Cell::utf8(value.preimage_version),
        ];
        push_authority(&mut row, schema_authority, "id_domain")?;
        rows.push(row);
    }
    result.insert(19, batch(19, rows)?);

    let mut rows = Vec::new();
    for value in ontology.enum_values {
        let mut row = vec![
            Cell::utf8(format!("enum:{}:{}", value.domain, value.code)),
            Cell::utf8(format!("enum:{}", value.domain)),
            Cell::I64(i64::from(value.code)),
            Cell::Null,
            Cell::utf8(value.name),
        ];
        push_authority(&mut row, value.authority, "ontology_term")?;
        rows.push(row);
    }
    let mut seen_entity_families = BTreeSet::new();
    for value in ontology.entity_kinds {
        if seen_entity_families.insert(value.family_code) {
            let canonical_name = ontology
                .enum_values
                .iter()
                .find(|entry| {
                    entry.domain == "ENTITY_FAMILY" && entry.code == i32::from(value.family_code)
                })
                .map_or_else(
                    || format!("ENTITY_FAMILY_{}", value.family_code),
                    |entry| entry.name.to_owned(),
                );
            let mut row = vec![
                Cell::utf8(format!("entity_family:{}", value.family_code)),
                Cell::utf8("ontology:entity-family"),
                Cell::I64(i64::from(value.family_code)),
                Cell::Null,
                Cell::utf8(canonical_name),
            ];
            push_authority(&mut row, value.authority, "ontology_term")?;
            rows.push(row);
        }
    }
    for value in ontology.entity_kinds {
        let mut row = vec![
            Cell::utf8(format!("entity_kind:{}", value.code)),
            Cell::utf8("ontology:entity-kind"),
            Cell::I64(i64::from(value.code)),
            Cell::Null,
            Cell::utf8(value.name),
        ];
        push_authority(&mut row, value.authority, "ontology_term")?;
        rows.push(row);
    }
    for value in ontology.relation_kinds {
        let mut row = vec![
            Cell::utf8(format!("relation_kind:{}", value.code)),
            Cell::utf8("ontology:relation-kind"),
            Cell::I64(i64::from(value.code)),
            Cell::Null,
            Cell::utf8(value.name),
        ];
        push_authority(&mut row, value.authority, "ontology_term")?;
        rows.push(row);
    }
    let mut term_ids = rows
        .iter()
        .filter_map(|row| match row.first() {
            Some(Cell::Utf8(value)) => Some(value.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for edge in ontology.edges {
        for term_id in [
            edge.subject_term_id,
            edge.predicate_term_id,
            edge.object_term_id,
        ] {
            if term_ids.insert(term_id.to_owned()) {
                let mut row = vec![
                    Cell::utf8(term_id),
                    Cell::Null,
                    Cell::Null,
                    Cell::utf8(term_id),
                    Cell::utf8(term_id),
                ];
                push_authority(&mut row, edge.authority, "ontology_term")?;
                rows.push(row);
            }
        }
    }
    result.insert(20, batch(20, rows)?);

    let mut rows = Vec::new();
    for value in ontology.edges {
        let mut row = vec![
            Cell::utf8(value.subject_term_id),
            Cell::utf8(value.predicate_term_id),
            Cell::utf8(value.object_term_id),
            Cell::I32(value.ordinal),
        ];
        push_authority(&mut row, value.authority, "ontology_edge")?;
        rows.push(row);
    }
    result.insert(21, batch(21, rows)?);

    let mut authorities = BTreeMap::new();
    for authority in ontology
        .enum_values
        .iter()
        .map(|value| value.authority)
        .chain(ontology.entity_kinds.iter().map(|value| value.authority))
        .chain(ontology.relation_kinds.iter().map(|value| value.authority))
        .chain(ontology.property_kinds.iter().map(|value| value.authority))
        .chain(ontology.fact_kinds.iter().map(|value| value.authority))
        .chain(
            ontology
                .provider_raw_kinds
                .iter()
                .map(|value| value.authority),
        )
        .chain([ontology.phrase_authority, ontology.query_form_authority])
        .chain(std::iter::once(schema_authority))
    {
        authorities
            .entry(authority.authority_id)
            .or_insert(authority);
    }
    let mut rows = Vec::new();
    for authority in authorities.values().copied() {
        let mut row = vec![
            Cell::utf8(authority.authority_id),
            Cell::utf8("compiled_registry"),
            Cell::utf8(authority.canonical_source_path),
        ];
        push_authority(&mut row, authority, "registry_authority")?;
        rows.push(row);
    }
    result.insert(22, batch(22, rows)?);

    let mut rows = Vec::new();
    for binding in semantic_type_bindings() {
        let (resolver_table, resolver_column) = match binding.authority {
            SemanticAuthority::EnumRegistry => (Some("enum_domain"), Some("code")),
            SemanticAuthority::OntologyEntityRegistry => (Some("entity_kind"), Some("code")),
            SemanticAuthority::OntologyRelationRegistry => (Some("relation_kind"), Some("code")),
            SemanticAuthority::OntologyPropertyRegistry => (Some("property_kind"), Some("code")),
            SemanticAuthority::OntologyFactRegistry => (Some("fact_kind"), Some("code")),
            SemanticAuthority::ProviderCatalog => {
                (Some("provider_raw_kind"), Some("raw_kind_code"))
            }
            SemanticAuthority::CapabilityRegistry
            | SemanticAuthority::TypeAlgebra
            | SemanticAuthority::SchemaIr
            | SemanticAuthority::Intrinsic
            | SemanticAuthority::DiagnosticProtocol => (None, None),
        };
        let mut row = vec![
            Cell::utf8(binding.semantic_type),
            Cell::utf8(
                binding
                    .authority_artifact_id
                    .unwrap_or_else(|| semantic_authority_name(binding.authority)),
            ),
            binding.domain.map_or(Cell::Null, Cell::utf8),
            resolver_table.map_or(Cell::Null, Cell::utf8),
            resolver_column.map_or(Cell::Null, Cell::utf8),
        ];
        push_authority(&mut row, schema_authority, "semantic_type_binding")?;
        rows.push(row);
    }
    result.insert(23, batch(23, rows)?);

    let mut rows = Vec::new();
    for spec in table_specs() {
        let mut row = vec![
            Cell::I16(spec.table_code),
            Cell::utf8(if spec.family == "ontology" {
                "cpg_ontology"
            } else {
                "cpg_base"
            }),
            Cell::utf8(spec.name),
            Cell::utf8(format!("{:?}", spec.materialization_role)),
            Cell::Bool(spec.required_for_publication),
            Cell::Hash32(framed(&spec.primary_key.join(","))?),
        ];
        push_authority(&mut row, schema_authority, "table_contract")?;
        rows.push(row);
    }
    result.insert(24, batch(24, rows)?);

    let mut rows = Vec::new();
    for spec in table_specs() {
        let columns =
            table_column_contracts(spec.table_code).ok_or_else(|| FabricError::TableInvariant {
                table: spec.name.to_owned(),
                detail: "compiled table lacks its merged column authority".to_owned(),
            })?;
        for (ordinal, column) in columns.iter().enumerate() {
            let primary_key_ordinal = spec
                .primary_key
                .iter()
                .position(|name| *name == column.name);
            let metadata_identity = format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}",
                column.name,
                column.logical_type,
                column.nullable,
                column.semantic_type,
                column.id_domain,
                column.element_id_domain,
                column.foreign_key,
            );
            let mut row = vec![
                Cell::I16(spec.table_code),
                Cell::I32(i32::try_from(ordinal).expect("column ordinal fits i32")),
                Cell::utf8(column.name),
                Cell::utf8(logical_type_name(column.logical_type)),
                column.semantic_type.map_or(Cell::Null, Cell::utf8),
                column
                    .id_domain
                    .or(column.element_id_domain)
                    .map_or(Cell::Null, Cell::utf8),
                Cell::Bool(column.nullable),
                column.foreign_key.map_or(Cell::Null, Cell::utf8),
                Cell::utf8(
                    crate::schema_registry::structure_class(spec.table_code, column.name)
                        .map_or_else(
                            || "ScalarIndependent".to_owned(),
                            |value| format!("{value:?}"),
                        ),
                ),
                primary_key_ordinal.map_or(Cell::Null, |value| {
                    Cell::I16(i16::try_from(value).expect("primary-key ordinal fits i16"))
                }),
                Cell::Hash32(framed(&metadata_identity)?),
            ];
            push_authority(&mut row, schema_authority, "column_contract")?;
            rows.push(row);
        }
    }
    result.insert(25, batch(25, rows)?);

    let mut result_rows = Vec::new();
    let mut field_rows = Vec::new();
    for schema in crate::schema_registry::result_schema_contracts() {
        let mut row = vec![
            Cell::utf8(schema.result_schema_id),
            Cell::I16(i16::try_from(schema.query_form_code).expect("query form code fits i16")),
            Cell::utf8(schema.result_role),
            Cell::utf8(schema.version),
        ];
        push_authority(&mut row, schema_authority, "result_schema")?;
        result_rows.push(row);
        for (ordinal, field) in schema.fields.iter().enumerate() {
            let mut row = vec![
                Cell::utf8(schema.result_schema_id),
                Cell::I32(i32::try_from(ordinal).expect("result ordinal fits i32")),
                Cell::utf8(field.name),
                Cell::utf8(logical_type_name(field.logical_type)),
                field.semantic_type.map_or(Cell::Null, Cell::utf8),
                field
                    .id_domain
                    .or(field.element_id_domain)
                    .map_or(Cell::Null, Cell::utf8),
                Cell::Bool(field.nullable),
            ];
            push_authority(&mut row, schema_authority, "result_field")?;
            field_rows.push(row);
        }
    }
    result.insert(26, batch(26, result_rows)?);
    result.insert(27, batch(27, field_rows)?);

    let mut rows = Vec::new();
    for domain in crate::schema_registry::id_domains() {
        let mut row = vec![
            Cell::utf8(domain.preimage_recipe_id),
            Cell::utf8(domain.preimage_version),
            Cell::utf8(domain.domain_slug),
            Cell::utf8("id16"),
        ];
        push_authority(&mut row, schema_authority, "identity_recipe")?;
        rows.push(row);
    }
    result.insert(28, batch(28, rows)?);

    let mut rows = Vec::new();
    for operation in crate::model_generated::schema_tables::SEMANTIC_OPERATION_SPECS {
        for (ordinal, operand) in operation.operand_codes.iter().enumerate() {
            let mut row = vec![
                Cell::utf8(operation.phrase_id),
                Cell::I32(i32::try_from(ordinal).expect("operand ordinal fits i32")),
                Cell::utf8(operation.canonical_text),
                Cell::utf8(format!("{:?}", operation.operator)),
                Cell::utf8(operation.column_role),
                Cell::utf8(operation.operand_domain),
                Cell::I64(i64::from(*operand)),
                Cell::utf8(format!("{:?}", operation.null_policy)),
                Cell::utf8(operation.output_role),
                Cell::utf8(operation.diagnostic_code),
            ];
            push_authority(&mut row, ontology.phrase_authority, "phrase_binding")?;
            rows.push(row);
        }
    }
    result.insert(29, batch(29, rows)?);

    let mut rows = Vec::new();
    for rule in crate::ontology_rules::rule_contracts() {
        let mut row = vec![
            Cell::utf8(rule.rule_id),
            Cell::utf8(format!("{:?}", rule.operation_kind)),
            Cell::Hash32(framed(rule.input_contract)?),
            Cell::Hash32(framed(rule.output_contract)?),
            Cell::utf8(rule.determinism_class),
            Cell::utf8(rule.diagnostic_code),
        ];
        push_authority(&mut row, schema_authority, "rule_contract")?;
        rows.push(row);
    }
    result.insert(30, batch(30, rows)?);
    Ok(result)
}

/// Digest of every ontology authority input. Ordinary fact publications reuse the exact Delta
/// versions while this value is unchanged.
#[must_use]
pub fn ontology_input_digest() -> String {
    let ontology = compiled_ontology();
    let mut identities = BTreeSet::new();
    for authority in ontology
        .enum_values
        .iter()
        .map(|value| value.authority)
        .chain(ontology.entity_kinds.iter().map(|value| value.authority))
        .chain(ontology.relation_kinds.iter().map(|value| value.authority))
        .chain(ontology.property_kinds.iter().map(|value| value.authority))
        .chain(ontology.fact_kinds.iter().map(|value| value.authority))
        .chain(
            ontology
                .provider_raw_kinds
                .iter()
                .map(|value| value.authority),
        )
        .chain([ontology.phrase_authority, ontology.query_form_authority])
    {
        identities.insert((authority.authority_id, authority.canonical_digest));
    }
    identities.insert(("codefabric.schema.contract-ir", schema_contract_digest()));
    let payload = identities
        .into_iter()
        .map(|(id, digest)| format!("{id}={digest}"))
        .collect::<Vec<_>>()
        .join("\n");
    crate::integrity::framed_digest(payload.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{ontology_dimension_batches, ontology_input_digest};
    use crate::schema_registry::table_spec;

    #[test]
    fn odf_dimension_registry_parity() {
        let batches = ontology_dimension_batches().expect("compiled ontology batches");
        assert_eq!(batches.len(), 20);
        assert_eq!(
            batches.keys().copied().collect::<Vec<_>>(),
            (11_i16..=30).collect::<Vec<_>>()
        );
        for (&table_code, batch) in &batches {
            let spec = table_spec(table_code).expect("generated ontology table");
            assert_eq!(spec.family, "ontology");
            assert_eq!(batch.schema(), spec.arrow_schema);
            assert!(batch.num_rows() > 0, "{} must be populated", spec.name);
        }
    }

    #[test]
    fn odf_ontology_candidate_version_stability() {
        let first_digest = ontology_input_digest();
        let first = ontology_dimension_batches().expect("first ontology build");
        let second = ontology_dimension_batches().expect("second ontology build");
        assert_eq!(first_digest, ontology_input_digest());
        assert_eq!(first, second);
    }
}
