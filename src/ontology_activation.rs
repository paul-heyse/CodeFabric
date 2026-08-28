//! Leased ontology-catalog resolution for the unified candidate runtime.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::{Array as _, Int16Array, RecordBatch, StringArray};
use datafusion::prelude::SessionContext;

use crate::fabric::FabricError;

/// One ontology relation discovered from table-contract rows and resolved through the lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOntologyRelation {
    pub table_code: i16,
    pub table_name: String,
    pub row_count: usize,
    pub field_names: Vec<String>,
}

/// Dynamic closure reached from the two self-description roots and one delivered result artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyCatalogResolution {
    pub registry_authorities: BTreeSet<String>,
    pub relations: BTreeMap<i16, ResolvedOntologyRelation>,
    pub delivered_result_schema_digest: String,
    pub delivered_result_checksum_version: String,
}

/// Discover ontology relations from the catalog's own `table_contract` rows. No fixed relation
/// list or generated table constant participates in discovery.
///
/// # Errors
///
/// Returns an integrity error when the generated table-contract columns are absent or have the
/// wrong physical types.
pub fn discover_ontology_relations(
    table_contract: &RecordBatch,
) -> Result<BTreeSet<i16>, FabricError> {
    let codes = table_contract
        .column_by_name("table_code")
        .and_then(|column| column.as_any().downcast_ref::<Int16Array>())
        .ok_or_else(|| {
            FabricError::PublicationIntegrity("table_contract.table_code is not code16".into())
        })?;
    let namespaces = table_contract
        .column_by_name("namespace")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| {
            FabricError::PublicationIntegrity("table_contract.namespace is not Utf8".into())
        })?;
    Ok((0..table_contract.num_rows())
        .filter(|&row| !namespaces.is_null(row) && namespaces.value(row) == "cpg_ontology")
        .map(|row| codes.value(row))
        .collect())
}

fn catalog_relation_name(code: i16, table_contract: &[RecordBatch]) -> Result<String, FabricError> {
    let mut resolved = None;
    for batch in table_contract {
        let codes = batch
            .column_by_name("table_code")
            .and_then(|column| column.as_any().downcast_ref::<Int16Array>())
            .ok_or_else(|| {
                FabricError::PublicationIntegrity("table_contract.table_code is not code16".into())
            })?;
        let namespaces = batch
            .column_by_name("namespace")
            .and_then(|column| column.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                FabricError::PublicationIntegrity("table_contract.namespace is not Utf8".into())
            })?;
        let names = batch
            .column_by_name("table_name")
            .and_then(|column| column.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                FabricError::PublicationIntegrity("table_contract.table_name is not Utf8".into())
            })?;
        for row in 0..batch.num_rows() {
            if !codes.is_null(row)
                && codes.value(row) == code
                && !namespaces.is_null(row)
                && namespaces.value(row) == "cpg_ontology"
                && !names.is_null(row)
            {
                let name = names.value(row).to_owned();
                if resolved.replace(name.clone()).is_some() {
                    return Err(FabricError::PublicationIntegrity(format!(
                        "ontology table code {code} resolves more than once"
                    )));
                }
            }
        }
    }
    resolved.ok_or_else(|| {
        FabricError::PublicationIntegrity(format!(
            "ontology table code {code} has no table_contract name"
        ))
    })
}

async fn collect_catalog_relation(
    context: &SessionContext,
    table_name: &str,
) -> Result<Vec<RecordBatch>, FabricError> {
    Ok(context
        .table(format!("codefabric.cpg_ontology.{table_name}"))
        .await?
        .collect()
        .await?)
}

/// Recursively resolve the normalized ontology plane from a leased catalog.
///
/// Only `registry_authority` and `table_contract` are bootstrap names. Every remaining
/// relation is discovered from delivered table-contract rows, including relations introduced by
/// a later ontology domain. The delivered result artifact contributes its governed schema and
/// checksum identities without granting it a second catalog authority.
///
/// # Errors
///
/// Returns an integrity or DataFusion error when delivered identities are malformed, bootstrap
/// relations are unavailable, or discovered ontology relations are ambiguous or unreadable.
pub async fn resolve_ontology_catalog(
    context: &SessionContext,
    delivered_result_schema_digest: &str,
    delivered_result_checksum_version: &str,
) -> Result<OntologyCatalogResolution, FabricError> {
    if !delivered_result_schema_digest.starts_with("b3:")
        || delivered_result_schema_digest.len() != 67
        || delivered_result_checksum_version.is_empty()
    {
        return Err(FabricError::PublicationIntegrity(
            "delivered result artifact lacks a governed schema/checksum identity".into(),
        ));
    }

    let authority_batches = collect_catalog_relation(context, "registry_authority").await?;
    let table_contract_batches = collect_catalog_relation(context, "table_contract").await?;
    let mut registry_authorities = BTreeSet::new();
    for batch in &authority_batches {
        let ids = batch
            .column_by_name("registry_authority_id")
            .and_then(|column| column.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                FabricError::PublicationIntegrity(
                    "registry_authority.registry_authority_id is not Utf8".into(),
                )
            })?;
        for row in 0..batch.num_rows() {
            if !ids.is_null(row) && !registry_authorities.insert(ids.value(row).to_owned()) {
                return Err(FabricError::PublicationIntegrity(format!(
                    "registry authority {} resolves more than once",
                    ids.value(row)
                )));
            }
        }
    }
    if registry_authorities.is_empty() {
        return Err(FabricError::PublicationIntegrity(
            "registry_authority is empty".into(),
        ));
    }

    let mut discovered_relations = BTreeSet::new();
    for batch in &table_contract_batches {
        discovered_relations.extend(discover_ontology_relations(batch)?);
    }
    let mut relations = BTreeMap::new();
    for code in discovered_relations {
        let table_name = catalog_relation_name(code, &table_contract_batches)?;
        let batches = collect_catalog_relation(context, &table_name).await?;
        let field_names = batches.first().map_or_else(Vec::new, |batch| {
            batch
                .schema()
                .fields()
                .iter()
                .map(|field| field.name().clone())
                .collect()
        });
        let row_count = batches.iter().map(RecordBatch::num_rows).sum();
        relations.insert(
            code,
            ResolvedOntologyRelation {
                table_code: code,
                table_name,
                row_count,
                field_names,
            },
        );
    }
    if relations.is_empty() {
        return Err(FabricError::PublicationIntegrity(
            "table_contract discovers no cpg_ontology relations".into(),
        ));
    }
    Ok(OntologyCatalogResolution {
        registry_authorities,
        relations,
        delivered_result_schema_digest: delivered_result_schema_digest.to_owned(),
        delivered_result_checksum_version: delivered_result_checksum_version.to_owned(),
    })
}
