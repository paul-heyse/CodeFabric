//! Atomic Stage-2b ontology-plane candidate acceptance.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::{Array as _, Int16Array, RecordBatch, StringArray};
use datafusion::prelude::SessionContext;

use crate::fabric::{FabricError, PublicationOutcome};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OntologyActivationFaultPoint {
    BeforeCandidateValidation,
    AfterCandidateValidation,
    BeforeAcceptanceRecord,
    AfterAcceptanceRecord,
    BeforePointerAdvance,
    AfterPointerAdvance,
}

impl OntologyActivationFaultPoint {
    /// Closed fault registry for the Stage-2b acceptance transaction.
    pub const ALL: [Self; 6] = [
        Self::BeforeCandidateValidation,
        Self::AfterCandidateValidation,
        Self::BeforeAcceptanceRecord,
        Self::AfterAcceptanceRecord,
        Self::BeforePointerAdvance,
        Self::AfterPointerAdvance,
    ];

    /// Stable registry code for this executable injection seam.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::BeforeCandidateValidation => "ONTOLOGY_BEFORE_CANDIDATE_VALIDATION",
            Self::AfterCandidateValidation => "ONTOLOGY_AFTER_CANDIDATE_VALIDATION",
            Self::BeforeAcceptanceRecord => "ONTOLOGY_BEFORE_ACCEPTANCE_RECORD",
            Self::AfterAcceptanceRecord => "ONTOLOGY_AFTER_ACCEPTANCE_RECORD",
            Self::BeforePointerAdvance => "ONTOLOGY_BEFORE_POINTER_ADVANCE",
            Self::AfterPointerAdvance => "ONTOLOGY_AFTER_POINTER_ADVANCE",
        }
    }
}

/// Proving inputs that must be independently complete before Stage 2b can activate.
pub const REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS: [&str; 5] =
    ["WP09", "WP10", "WP11", "WP12", "WP16"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyCandidateDossier {
    pub ontology_input_digest: String,
    pub table_versions: BTreeMap<i16, u64>,
    pub proving_artifact_digests: BTreeMap<String, String>,
    pub dossier_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyOwnerAcceptance {
    pub owner_identity: String,
    pub accepted_at_micros: i64,
    pub dossier_digest: String,
    pub acceptance_digest: String,
}

/// One ontology relation discovered from `table_contract` and resolved through the leased catalog.
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OntologyActivationState {
    pub pointer_generation: u64,
    pub active_input_digest: Option<String>,
    pub active_table_versions: BTreeMap<i16, u64>,
    pub acceptance: Option<OntologyOwnerAcceptance>,
}

fn framed(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
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

impl OntologyCandidateDossier {
    /// Build the complete version-pinned ontology activation dossier.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when discovery is empty, a required relation is absent or
    /// unvalidated, or a required proving-artifact digest is missing or malformed.
    pub fn build(
        publication: &PublicationOutcome,
        discovered_relations: &BTreeSet<i16>,
        proving_artifact_digests: BTreeMap<String, String>,
    ) -> Result<Self, FabricError> {
        if discovered_relations.is_empty() {
            return Err(FabricError::PublicationIntegrity(
                "Stage-2b discovery returned no ontology relations".into(),
            ));
        }
        let table_versions = discovered_relations
            .iter()
            .map(|&code| {
                let record = publication.tables.get(&code).ok_or_else(|| {
                    FabricError::PublicationIntegrity(format!(
                        "ontology candidate relation {code} is absent from the publication manifest"
                    ))
                })?;
                if !record.validated || !record.required {
                    return Err(FabricError::PublicationIntegrity(format!(
                        "ontology candidate relation {code} is not validated and required"
                    )));
                }
                Ok((code, record.delta_version))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let missing_proofs = REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS
            .into_iter()
            .filter(|id| !proving_artifact_digests.contains_key(*id))
            .collect::<Vec<_>>();
        if !missing_proofs.is_empty()
            || proving_artifact_digests
                .values()
                .any(|digest| !digest.starts_with("b3:") || digest.len() != 67)
        {
            return Err(FabricError::PublicationIntegrity(format!(
                "Stage-2b dossier has missing or malformed proving artifacts: {missing_proofs:?}"
            )));
        }
        let ontology_input_digest = crate::ontology_plane::ontology_input_digest();
        let mut parts = vec![ontology_input_digest.clone()];
        parts.extend(
            table_versions
                .iter()
                .map(|(code, version)| format!("{code}:{version}")),
        );
        parts.extend(
            proving_artifact_digests
                .iter()
                .map(|(id, digest)| format!("{id}:{digest}")),
        );
        let dossier_digest = framed(parts.iter().map(String::as_bytes));
        Ok(Self {
            ontology_input_digest,
            table_versions,
            proving_artifact_digests,
            dossier_digest,
        })
    }
}

fn acceptance_digest(acceptance: &OntologyOwnerAcceptance) -> String {
    framed([
        acceptance.owner_identity.as_bytes(),
        &acceptance.accepted_at_micros.to_be_bytes(),
        acceptance.dossier_digest.as_bytes(),
    ])
}

impl OntologyActivationState {
    /// Validate, accept, and advance one candidate in a single in-memory transaction. Persistence
    /// writes the returned state through the existing publication CAS; injected failures leave
    /// this prior state byte-for-byte unchanged.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the dossier or acceptance is incomplete, stale, or fails
    /// at an injected activation seam.
    pub fn activate(
        &mut self,
        dossier: &OntologyCandidateDossier,
        mut acceptance: OntologyOwnerAcceptance,
        discovered_relations: &BTreeSet<i16>,
        fault: Option<OntologyActivationFaultPoint>,
    ) -> Result<bool, FabricError> {
        let inject = |point| {
            if fault == Some(point) {
                Err(FabricError::PublicationIntegrity(format!(
                    "injected ontology activation fault at {point:?}"
                )))
            } else {
                Ok(())
            }
        };
        inject(OntologyActivationFaultPoint::BeforeCandidateValidation)?;
        if discovered_relations != &dossier.table_versions.keys().copied().collect()
            || dossier.ontology_input_digest != crate::ontology_plane::ontology_input_digest()
            || REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS
                .into_iter()
                .any(|id| !dossier.proving_artifact_digests.contains_key(id))
            || dossier
                .proving_artifact_digests
                .values()
                .any(|digest| !digest.starts_with("b3:") || digest.len() != 67)
        {
            return Err(FabricError::PublicationIntegrity(
                "ontology candidate dossier is incomplete or stale".into(),
            ));
        }
        if self.active_input_digest.as_deref() == Some(&dossier.ontology_input_digest)
            && self.active_table_versions == dossier.table_versions
        {
            return Ok(false);
        }
        inject(OntologyActivationFaultPoint::AfterCandidateValidation)?;
        inject(OntologyActivationFaultPoint::BeforeAcceptanceRecord)?;
        if acceptance.owner_identity.trim().is_empty()
            || acceptance.accepted_at_micros <= 0
            || acceptance.dossier_digest != dossier.dossier_digest
        {
            return Err(FabricError::PublicationIntegrity(
                "ontology owner acceptance does not bind the candidate".into(),
            ));
        }
        acceptance.acceptance_digest = acceptance_digest(&acceptance);
        inject(OntologyActivationFaultPoint::AfterAcceptanceRecord)?;
        let mut candidate = self.clone();
        candidate.pointer_generation =
            candidate.pointer_generation.checked_add(1).ok_or_else(|| {
                FabricError::PublicationIntegrity("ontology pointer generation overflow".into())
            })?;
        candidate.active_input_digest = Some(dossier.ontology_input_digest.clone());
        candidate.active_table_versions = dossier.table_versions.clone();
        candidate.acceptance = Some(acceptance);
        inject(OntologyActivationFaultPoint::BeforePointerAdvance)?;
        // Both pointer boundaries are inside the candidate transaction. An injected crash after
        // the SQL pointer statement still rolls the transaction back before this value is exposed.
        inject(OntologyActivationFaultPoint::AfterPointerAdvance)?;
        *self = candidate;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use arrow_array::{ArrayRef, BooleanArray, Int16Array, RecordBatch, StringArray};
    use arrow_select::concat::concat_batches;
    use datafusion::catalog::memory::{MemoryCatalogProvider, MemorySchemaProvider};
    use datafusion::catalog::{CatalogProvider, SchemaProvider};
    use datafusion::datasource::MemTable;
    use datafusion::prelude::SessionContext;

    use super::{
        OntologyActivationState, OntologyCandidateDossier, OntologyOwnerAcceptance,
        REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS, resolve_ontology_catalog,
    };
    use crate::ontology_plane::{ontology_dimension_batches, ontology_input_digest};
    use crate::schema_registry::table_spec;

    fn proof_digests() -> BTreeMap<String, String> {
        REQUIRED_STAGE2B_PROVING_ARTIFACT_IDS
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    id.to_owned(),
                    crate::integrity::framed_digest(&[u8::try_from(index).expect("proof index")]),
                )
            })
            .collect()
    }

    fn dossier() -> OntologyCandidateDossier {
        OntologyCandidateDossier {
            ontology_input_digest: ontology_input_digest(),
            table_versions: (11_i16..=30).map(|code| (code, 1_u64)).collect(),
            proving_artifact_digests: proof_digests(),
            dossier_digest: crate::integrity::framed_digest(b"stage2b-dossier"),
        }
    }

    fn acceptance(dossier: &OntologyCandidateDossier) -> OntologyOwnerAcceptance {
        OntologyOwnerAcceptance {
            owner_identity: "ontology-owner".into(),
            accepted_at_micros: 1,
            dossier_digest: dossier.dossier_digest.clone(),
            acceptance_digest: String::new(),
        }
    }

    #[test]
    fn odf_stage2b_candidate_closure() {
        let dossier = dossier();
        let discovered = dossier
            .table_versions
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut state = OntologyActivationState::default();
        assert!(
            state
                .activate(&dossier, acceptance(&dossier), &discovered, None)
                .expect("complete Stage-2b dossier")
        );
        assert!(
            !state
                .activate(&dossier, acceptance(&dossier), &discovered, None)
                .expect("identical dossier retry")
        );

        let mut incomplete_discovery = discovered.clone();
        incomplete_discovery.remove(&30);
        assert!(
            OntologyActivationState::default()
                .activate(&dossier, acceptance(&dossier), &incomplete_discovery, None,)
                .is_err()
        );
        let mut missing_proof = dossier.clone();
        missing_proof.proving_artifact_digests.remove("WP16");
        assert!(
            OntologyActivationState::default()
                .activate(
                    &missing_proof,
                    acceptance(&missing_proof),
                    &discovered,
                    None,
                )
                .is_err()
        );
    }

    #[tokio::test]
    async fn odf_stage2b_recursive_self_description() {
        const NEW_CODE: i16 = 31;
        let mut batches = ontology_dimension_batches().expect("ontology batches");
        let table_contract = batches.get(&24).expect("table contract");
        let digest = [7_u8; 32];
        let new_contract = RecordBatch::try_new(
            table_contract.schema(),
            vec![
                Arc::new(Int16Array::from(vec![NEW_CODE])) as ArrayRef,
                Arc::new(StringArray::from(vec!["cpg_ontology"])),
                Arc::new(StringArray::from(vec!["seeded_new_domain"])),
                Arc::new(StringArray::from(vec!["BundleDimension"])),
                Arc::new(BooleanArray::from(vec![true])),
                crate::fabric::hash32_array([Some(&digest)]),
                Arc::new(StringArray::from(vec!["seed.authority"])),
                Arc::new(StringArray::from(vec!["1"])),
                crate::fabric::hash32_array([Some(&digest)]),
            ],
        )
        .expect("new table contract row");
        let extended = concat_batches(&table_contract.schema(), [table_contract, &new_contract])
            .expect("extended table contract");
        batches.insert(24, extended);

        let seeded = batches.get(&11).expect("seeded relation shape").clone();
        let schema = Arc::new(MemorySchemaProvider::new());
        for (&code, batch) in &batches {
            let name = table_spec(code).expect("ontology table").name.to_owned();
            let provider = Arc::new(
                MemTable::try_new(batch.schema(), vec![vec![batch.clone()]])
                    .expect("ontology provider"),
            );
            schema
                .register_table(name, provider)
                .expect("ontology table registration");
        }
        schema
            .register_table(
                "seeded_new_domain".into(),
                Arc::new(
                    MemTable::try_new(seeded.schema(), vec![vec![seeded]])
                        .expect("seeded provider"),
                ),
            )
            .expect("seeded relation registration");
        let catalog = Arc::new(MemoryCatalogProvider::new());
        catalog
            .register_schema("cpg_ontology", schema)
            .expect("ontology schema registration");
        let context = SessionContext::new();
        context.register_catalog("codefabric", catalog);

        let result_digest = crate::integrity::framed_digest(b"delivered-result");
        let resolution = resolve_ontology_catalog(&context, &result_digest, "ResultChecksumV2")
            .await
            .expect("recursive catalog resolution");
        assert_eq!(resolution.relations.len(), 21);
        assert_eq!(
            resolution
                .relations
                .get(&NEW_CODE)
                .map(|value| value.table_name.as_str()),
            Some("seeded_new_domain")
        );
        assert!(!resolution.registry_authorities.is_empty());
    }
}
