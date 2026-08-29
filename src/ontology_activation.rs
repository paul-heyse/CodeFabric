//! Leased ontology-catalog resolution for the unified candidate runtime.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::{Array as _, Int16Array, RecordBatch, StringArray};
use datafusion::prelude::SessionContext;
#[cfg(feature = "daemon")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "daemon")]
use thiserror::Error;

use crate::fabric::FabricError;
#[cfg(feature = "daemon")]
use crate::fabric::PublicationOutcome;
#[cfg(feature = "daemon")]
use crate::ontology_candidate::{CandidateClosureError, CandidateClosureRunner};
#[cfg(feature = "daemon")]
use crate::ontology_gate::GateResourceEnvelope;
#[cfg(feature = "daemon")]
use crate::operational_store::{OperationalStore, OperationalStoreError};
#[cfg(feature = "daemon")]
use crate::snapshot::{ResultAuthorityPin, ServingSnapshotManifestBody};
#[cfg(feature = "daemon")]
use crate::snapshot_runtime::{ServingSnapshotCandidate, SnapshotRuntimeError};

/// Untrusted candidate material accepted by the private administrative boundary. The daemon
/// derives program, policy, predecessor, proof, decision, pointer, and result authority itself.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OntologyCandidateSubmission {
    pub publication: PublicationOutcome,
    pub manifest_body: ServingSnapshotManifestBody,
    #[serde(default)]
    pub source_blob_digests: Vec<[u8; 32]>,
    pub rollback_retain_until: i64,
}

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

/// Opaque outcome from the only production proof-and-stage route.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvedOntologyCandidate {
    candidate_identity: String,
    serving_snapshot_id: [u8; 16],
}

#[cfg(feature = "daemon")]
impl ProvedOntologyCandidate {
    #[must_use]
    pub fn candidate_identity(&self) -> &str {
        &self.candidate_identity
    }

    #[must_use]
    pub const fn serving_snapshot_id(&self) -> [u8; 16] {
        self.serving_snapshot_id
    }
}

/// Failures before owner acceptance. None of these paths can advance a durable pointer.
#[cfg(feature = "daemon")]
#[derive(Debug, Error)]
pub enum OntologyCandidateStageError {
    #[error(transparent)]
    Candidate(#[from] CandidateClosureError),
    #[error(transparent)]
    Snapshot(#[from] SnapshotRuntimeError),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Program(#[from] crate::ontology_program::OntologyProgramError),
    #[error(transparent)]
    Session(#[from] crate::governed_session::GovernedSessionError),
}

/// Trusted production coordinator for bounded proof and durable READY staging.
#[cfg(feature = "daemon")]
pub struct OntologyActivationCoordinator;

#[cfg(feature = "daemon")]
impl OntologyActivationCoordinator {
    /// Authenticate, compile, prove, stage, decide, and atomically activate one submission.
    /// The caller supplies no candidate identity, predecessor, policy, receipts, decision, epoch,
    /// result authority, or pointer generation.
    ///
    /// # Errors
    ///
    /// Returns a staging error when authentication, package compilation, candidate proof,
    /// snapshot staging, owner decision, or the atomic activation transaction fails.
    pub async fn submit_and_activate(
        store: &mut OperationalStore,
        workspace_id: [u8; 16],
        submission: OntologyCandidateSubmission,
        administrative_key: &[u8],
        request_key: &str,
        requested_at: i64,
    ) -> Result<crate::operational_store::OntologyActivationOutcome, OntologyCandidateStageError>
    {
        store.verify_workspace_owner_key(workspace_id, administrative_key)?;
        if submission.publication.scope.workspace_id != workspace_id {
            return Err(OperationalStoreError::OntologyActivation(
                "candidate submission belongs to another workspace".into(),
            )
            .into());
        }
        if let Some(outcome) = store.replay_completed_ontology_activation(
            workspace_id,
            submission.publication.publication_id,
            administrative_key,
            request_key,
        )? {
            return Ok(outcome);
        }
        let active = store.active_ontology_authority(workspace_id)?;
        if let Some(authority) = &active {
            let active_candidate = store
                .ontology_candidate(&authority.candidate_identity)?
                .ok_or_else(|| {
                    OperationalStoreError::OntologyActivation(
                        "active ontology authority has no candidate projection".into(),
                    )
                })?;
            if active_candidate.publication_id == submission.publication.publication_id {
                return Err(OperationalStoreError::OntologyActivation(
                    "candidate publication is already bound to the active ontology epoch".into(),
                )
                .into());
            }
        }
        let predecessor = active.map(|authority| authority.epoch_identity);
        let package = crate::ontology_program::build_ontology_program_package(
            &crate::ontology_program::OntologyPackagingProfile::default(),
        )?;
        let session = crate::governed_session::GovernedSession::new(
            datafusion::prelude::SessionConfig::new(),
            "policy.ontology.v1",
        )?;
        let runner = CandidateClosureRunner::new_for_epoch(
            package,
            submission.publication,
            session,
            predecessor,
            submission.rollback_retain_until,
        )?;
        let proved = Self::prove_and_stage(
            store,
            &runner,
            &GateResourceEnvelope::default(),
            submission.manifest_body,
            &submission.source_blob_digests,
            requested_at,
        )
        .await?;
        store
            .activate_proved_ontology_candidate(
                workspace_id,
                proved.candidate_identity(),
                administrative_key,
                request_key,
                requested_at,
            )
            .map_err(Into::into)
    }

    /// Execute the candidate's exact-Delta relational program, derive the immutable result pin,
    /// construct the serving snapshot from the same provider graph, and persist PROVED + READY
    /// together. Owner acceptance and pointer movement remain a later short admin transaction.
    ///
    /// # Errors
    ///
    /// Returns without a pointer mutation on program, resource, provider, manifest, or storage
    /// failure.
    pub async fn prove_and_stage(
        store: &mut OperationalStore,
        runner: &CandidateClosureRunner,
        limits: &GateResourceEnvelope,
        mut manifest_body: ServingSnapshotManifestBody,
        source_blob_digests: &[[u8; 32]],
        persisted_at: i64,
    ) -> Result<ProvedOntologyCandidate, OntologyCandidateStageError> {
        let report = runner.execute(limits).await?;
        let evidence = report.durable_evidence();
        manifest_body.manifest_version = "2.0".into();
        manifest_body.result_authority = Some(ResultAuthorityPin {
            result_authority_identity: evidence.result_authority_identity.clone(),
            program_identity: evidence.program_identity.clone(),
            function_catalog_identity: evidence.function_catalog_identity.clone(),
            policy_identity: evidence.result_policy_identity.clone(),
            query_form_identity: evidence.query_form_identity.clone(),
            checksum_version: evidence.checksum_version.clone(),
            exact_table_set_identity: evidence.exact_table_set_identity.clone(),
        });
        let catalog = std::sync::Arc::new(runner.open_frozen_catalog().await?);
        let snapshot =
            ServingSnapshotCandidate::build(manifest_body, catalog, source_blob_digests)?;
        let staged = snapshot.staged_record()?;
        store.persist_proved_ontology_candidate_with_snapshot(&report, &staged, persisted_at)?;
        Ok(ProvedOntologyCandidate {
            candidate_identity: report.candidate_identity().into(),
            serving_snapshot_id: staged.snapshot_id,
        })
    }
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
