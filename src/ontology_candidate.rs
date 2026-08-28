//! Bootstrap-discovered semantic closure and opaque candidate-bound gate receipts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array as _, BooleanArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder, col};
use thiserror::Error;

use crate::fabric::{PublicationOutcome, PublicationTableRecord};
use crate::governed_session::{GovernedSession, GovernedSessionError};
use crate::ontology_gate::{GateResourceEnvelope, OntologyGateOutcome};
use crate::ontology_program::{
    OntologyProgramError, OntologyProgramPackage, validate_ontology_program_package,
};

const EXPECTED_RESULT_CONTRACT: &str = "zero-violation-rows.v1";

/// One requirement discovered from the stable bootstrap member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapRequirement {
    pub ordinal: u16,
    pub family_id: String,
    pub binding_kind: String,
    pub relation_id: String,
    pub depends_on: Option<String>,
}

/// Immutable URI/version identity copied from the candidate publication manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTableBinding {
    table_code: i16,
    table_uri: String,
    delta_version: u64,
    schema_fingerprint: [u8; 32],
    table_checksum: [u8; 32],
}

impl ExactTableBinding {
    #[must_use]
    pub const fn table_code(&self) -> i16 {
        self.table_code
    }

    #[must_use]
    pub fn table_uri(&self) -> &str {
        &self.table_uri
    }

    #[must_use]
    pub const fn delta_version(&self) -> u64 {
        self.delta_version
    }
}

/// Frozen exact table set. No latest-version lookup or raw-file fallback is represented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTableSet {
    bindings: BTreeMap<i16, ExactTableBinding>,
    identity: String,
}

/// One semantic gate receipt. All trust-bearing fields and its constructor are private.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGateReceipt {
    operation_id: String,
    execution_identity: String,
    candidate_identity: String,
    program_identity: String,
    package_identity: String,
    session_identity: String,
    config_identity: String,
    policy_identity: String,
    exact_table_set_identity: String,
    semantic_checksum: String,
    expected_result_contract: String,
    artifact_identity: String,
    receipt_identity: String,
}

impl CandidateGateReceipt {
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.receipt_identity
    }
}

/// Successful closure evidence with a private 1:1 receipt ledger.
#[derive(Clone, Debug)]
pub struct CandidateClosureReport {
    candidate_identity: String,
    requirements: Vec<BootstrapRequirement>,
    receipts: BTreeMap<String, CandidateGateReceipt>,
    durable: DurableCandidateEvidence,
}

impl CandidateClosureReport {
    #[must_use]
    pub fn candidate_identity(&self) -> &str {
        &self.candidate_identity
    }

    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.requirements.len()
    }

    #[must_use]
    pub fn receipt_count(&self) -> usize {
        self.receipts.len()
    }

    #[must_use]
    pub fn receipt_identities(&self) -> BTreeSet<&str> {
        self.receipts
            .values()
            .map(CandidateGateReceipt::identity)
            .collect()
    }

    /// Accountable policy identity that an authenticated owner decision must bind.
    #[must_use]
    pub fn policy_identity(&self) -> &str {
        &self.durable.policy_identity
    }

    pub(crate) const fn durable_evidence(&self) -> &DurableCandidateEvidence {
        &self.durable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DurableExactTableBinding {
    pub table_code: i16,
    pub table_uri: String,
    pub delta_version: u64,
    pub schema_identity: [u8; 32],
    pub content_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DurableGateEvidence {
    pub operation_id: String,
    pub execution_identity: String,
    pub semantic_checksum: String,
    pub artifact_identity: String,
    pub artifact_bytes: Vec<u8>,
    pub receipt_identity: String,
    pub receipt_bytes: Vec<u8>,
    pub expected_result_contract: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DurableCandidateEvidence {
    pub candidate_identity: String,
    pub workspace_id: [u8; 16],
    pub manifest_bytes: Vec<u8>,
    pub manifest_digest: String,
    pub program_identity: String,
    pub package_identity: String,
    pub config_identity: String,
    pub policy_identity: String,
    pub result_policy_identity: String,
    pub exact_table_set_identity: String,
    pub function_catalog_identity: String,
    pub query_form_identity: String,
    pub checksum_version: String,
    pub result_authority_identity: String,
    pub predecessor_epoch_identity: Option<String>,
    pub rollback_retain_until: i64,
    pub exact_tables: Vec<DurableExactTableBinding>,
    pub gate_evidence: Vec<DurableGateEvidence>,
    pub receipt_set_identity: String,
}

/// Candidate semantic-closure failures. No error can advance activation authority.
#[derive(Debug, Error)]
pub enum CandidateClosureError {
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
    #[error(transparent)]
    Program(#[from] OntologyProgramError),
    #[error(transparent)]
    Session(#[from] GovernedSessionError),
    #[error("ONTOLOGY_CANDIDATE_CLOSURE_INVALID:{0}")]
    Invalid(String),
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

fn digest_is_valid(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical_value_bytes(value: &serde_json::Value) -> Result<Vec<u8>, CandidateClosureError> {
    crate::contracts::jcs::canonicalize_value(value)
        .map_err(|error| CandidateClosureError::Invalid(error.to_string()))
}

impl ExactTableSet {
    /// Bind every table to the exact immutable manifest tuple.
    fn from_publication(publication: &PublicationOutcome) -> Result<Self, CandidateClosureError> {
        if publication.tables.is_empty() {
            return Err(CandidateClosureError::Invalid(
                "candidate publication has no exact table bindings".into(),
            ));
        }
        let mut bindings = BTreeMap::new();
        for (&table_code, record) in &publication.tables {
            validate_table_binding(publication, table_code, record)?;
            bindings.insert(
                table_code,
                ExactTableBinding {
                    table_code,
                    table_uri: record.table_uri.clone(),
                    delta_version: record.delta_version,
                    schema_fingerprint: record.schema_fingerprint,
                    table_checksum: record.table_checksum,
                },
            );
        }
        let identity = framed(bindings.values().map(|binding| {
            framed([
                binding.table_code.to_be_bytes().as_slice(),
                binding.table_uri.as_bytes(),
                binding.delta_version.to_be_bytes().as_slice(),
                binding.schema_fingerprint.as_slice(),
                binding.table_checksum.as_slice(),
            ])
        }));
        Ok(Self { bindings, identity })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &ExactTableBinding> {
        self.bindings.values()
    }
}

fn validate_table_binding(
    publication: &PublicationOutcome,
    table_code: i16,
    record: &PublicationTableRecord,
) -> Result<(), CandidateClosureError> {
    if record.table_code != table_code
        || record.publication_id != publication.publication_id
        || record.workspace_id != publication.scope.workspace_id
        || !record.required
        || !record.validated
        || !record.table_uri.starts_with("file://")
    {
        return Err(CandidateClosureError::Invalid(format!(
            "table {table_code} is not an exact validated candidate tuple"
        )));
    }
    Ok(())
}

/// Decode the bootstrap member and validate its complete non-bootstrap member census.
///
/// # Errors
///
/// Rejects an invalid package, missing or malformed bootstrap columns, duplicate relations,
/// incomplete membership, or inconsistent content-set identities.
pub fn decode_bootstrap(
    package: &OntologyProgramPackage,
) -> Result<Vec<BootstrapRequirement>, CandidateClosureError> {
    validate_ontology_program_package(package)?;
    let member = package
        .members
        .get("program.bootstrap")
        .ok_or_else(|| CandidateClosureError::Invalid("program.bootstrap is absent".into()))?;
    let batch = member
        .batches
        .first()
        .ok_or_else(|| CandidateClosureError::Invalid("program.bootstrap has no batch".into()))?;
    let relations = column::<StringArray>(batch, "relation_id")?;
    let addresses = column::<StringArray>(batch, "member_address")?;
    let roles = column::<StringArray>(batch, "relation_role")?;
    let schema_identities = column::<StringArray>(batch, "schema_identity")?;
    let content_identities = column::<StringArray>(batch, "content_identity")?;
    let required = column::<BooleanArray>(batch, "required")?;
    let content_sets = column::<StringArray>(batch, "content_set_identity")?;
    let mut requirements = Vec::with_capacity(batch.num_rows());
    let mut seen = BTreeSet::new();
    let mut content_set = None;
    for row in 0..batch.num_rows() {
        if relations.is_null(row)
            || addresses.is_null(row)
            || roles.is_null(row)
            || schema_identities.is_null(row)
            || content_identities.is_null(row)
            || required.is_null(row)
            || content_sets.is_null(row)
        {
            return Err(CandidateClosureError::Invalid(format!(
                "bootstrap row {row} is not canonical"
            )));
        }
        let relation_id = relations.value(row).to_owned();
        let member = package.members.get(&relation_id).ok_or_else(|| {
            CandidateClosureError::Invalid(format!("bootstrap names absent relation {relation_id}"))
        })?;
        if relation_id == "program.bootstrap"
            || addresses.value(row) != relation_id
            || roles.value(row) != "program_relation"
            || !required.value(row)
            || !seen.insert(relation_id.clone())
        {
            return Err(CandidateClosureError::Invalid(format!(
                "bootstrap relation {relation_id:?} is invalid or duplicated"
            )));
        }
        let expected_schema = format!(
            "b3:{}",
            blake3::hash(format!("{:?}", member.schema).as_bytes()).to_hex()
        );
        let expected_content = format!("b3:{}", blake3::hash(&member.ipc_bytes).to_hex());
        if schema_identities.value(row) != expected_schema
            || content_identities.value(row) != expected_content
        {
            return Err(CandidateClosureError::Invalid(format!(
                "bootstrap relation {relation_id} has changed schema or content identity"
            )));
        }
        let row_content_set = content_sets.value(row);
        if content_set
            .replace(row_content_set)
            .is_some_and(|prior| prior != row_content_set)
        {
            return Err(CandidateClosureError::Invalid(
                "bootstrap rows disagree on content-set identity".into(),
            ));
        }
        requirements.push(BootstrapRequirement {
            ordinal: u16::try_from(row).map_err(|_| {
                CandidateClosureError::Invalid("bootstrap relation census exceeds UInt16".into())
            })?,
            family_id: relation_id.clone(),
            binding_kind: "program_member".into(),
            relation_id,
            depends_on: None,
        });
    }
    if requirements.is_empty() {
        return Err(CandidateClosureError::Invalid(
            "bootstrap closure is empty".into(),
        ));
    }
    let expected = package
        .members
        .keys()
        .filter(|relation| relation.as_str() != "program.bootstrap")
        .cloned()
        .collect::<BTreeSet<_>>();
    if seen != expected {
        return Err(CandidateClosureError::Invalid(
            "bootstrap census differs from the non-bootstrap package census".into(),
        ));
    }
    Ok(requirements)
}

fn column<'a, T: 'static>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a T, CandidateClosureError> {
    batch
        .column_by_name(name)
        .and_then(|value| value.as_any().downcast_ref::<T>())
        .ok_or_else(|| CandidateClosureError::Invalid(format!("bootstrap.{name} has wrong type")))
}

fn expected_binding_identity(
    requirement: &BootstrapRequirement,
    package: &OntologyProgramPackage,
    publication: &PublicationOutcome,
    session: &GovernedSession,
    exact_tables: &ExactTableSet,
) -> Result<String, CandidateClosureError> {
    match requirement.binding_kind.as_str() {
        "authored" => Ok(framed([
            b"ontology-authored-closure-family.v1".as_slice(),
            requirement.family_id.as_bytes(),
            requirement.relation_id.as_bytes(),
            crate::ontology_plane::ontology_input_digest().as_bytes(),
            crate::schema_registry::schema_contract_digest().as_bytes(),
        ])),
        "program_member" => package
            .manifest
            .member_identities
            .get(&requirement.relation_id)
            .cloned()
            .ok_or_else(|| {
                CandidateClosureError::Invalid(format!(
                    "{} names absent program member {}",
                    requirement.family_id, requirement.relation_id
                ))
            }),
        "candidate" => candidate_binding_identity(
            &requirement.relation_id,
            package,
            publication,
            session,
            exact_tables,
        ),
        other => Err(CandidateClosureError::Invalid(format!(
            "{} has unsupported binding kind {other}",
            requirement.family_id
        ))),
    }
}

fn candidate_binding_identity(
    relation_id: &str,
    package: &OntologyProgramPackage,
    publication: &PublicationOutcome,
    session: &GovernedSession,
    exact_tables: &ExactTableSet,
) -> Result<String, CandidateClosureError> {
    let value = match relation_id {
        "candidate.snapshot" => framed([
            b"candidate-snapshot.v1".as_slice(),
            publication.scope.workspace_id.as_slice(),
            publication.scope.source_generation.to_be_bytes().as_slice(),
            publication.scope.analysis_context_set_id.as_slice(),
        ]),
        "candidate.publication" => framed([
            b"candidate-publication.v1".as_slice(),
            publication.publication_id.as_slice(),
            publication
                .pointer
                .pointer_generation
                .to_be_bytes()
                .as_slice(),
        ]),
        "candidate.plan" => framed([
            b"candidate-plan.v1".as_slice(),
            package.manifest.logical_program_identity.as_bytes(),
            session.session_identity().as_bytes(),
            session.config_identity().as_bytes(),
        ]),
        "candidate.package" => package.manifest.package_identity.clone(),
        "candidate.policy" => framed([
            b"candidate-policy.v1".as_slice(),
            session.policy_identity().as_bytes(),
        ]),
        "candidate.exact_tables" => exact_tables.identity().to_owned(),
        other => {
            return Err(CandidateClosureError::Invalid(format!(
                "unknown candidate bootstrap relation {other}"
            )));
        }
    };
    Ok(value)
}

fn closure_plan(
    family_id: &str,
    observed_identity: &str,
    expected_identity: &str,
) -> Result<LogicalPlan, CandidateClosureError> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("family_id", DataType::Utf8, false),
        Field::new("observed_identity", DataType::Utf8, false),
        Field::new("expected_identity", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from(vec![family_id])),
            Arc::new(StringArray::from(vec![observed_identity])),
            Arc::new(StringArray::from(vec![expected_identity])),
        ],
    )?;
    let provider = Arc::new(MemTable::try_new(schema, vec![vec![batch]])?);
    Ok(LogicalPlanBuilder::scan(
        format!("closure_{}", family_id.replace('-', "_")),
        provider_as_source(provider),
        None,
    )?
    .filter(col("observed_identity").not_eq(col("expected_identity")))?
    .project(vec![
        col("family_id"),
        col("observed_identity"),
        col("expected_identity"),
    ])?
    .build()?)
}

fn candidate_identity(
    package: &OntologyProgramPackage,
    publication: &PublicationOutcome,
    session: &GovernedSession,
    exact_tables: &ExactTableSet,
    expected_bindings: &BTreeMap<String, String>,
) -> String {
    framed(
        [
            b"ontology-candidate.v1".to_vec(),
            publication.publication_id.to_vec(),
            package
                .manifest
                .logical_program_identity
                .as_bytes()
                .to_vec(),
            package.manifest.package_identity.as_bytes().to_vec(),
            session.session_identity().as_bytes().to_vec(),
            session.config_identity().as_bytes().to_vec(),
            session.policy_identity().as_bytes().to_vec(),
            exact_tables.identity().as_bytes().to_vec(),
        ]
        .into_iter()
        .chain(
            expected_bindings
                .iter()
                .map(|(family, identity)| format!("{family}:{identity}").into_bytes()),
        ),
    )
}

fn make_receipt(
    operation_id: &str,
    candidate_identity: &str,
    package: &OntologyProgramPackage,
    session: &GovernedSession,
    exact_tables: &ExactTableSet,
    outcome: &OntologyGateOutcome,
) -> CandidateGateReceipt {
    let semantic_checksum = outcome.receipt.gate_checksum.checksum.clone();
    let artifact_identity = outcome.artifact.artifact_identity.clone();
    let mut receipt = CandidateGateReceipt {
        operation_id: operation_id.to_owned(),
        execution_identity: outcome.receipt.execution_id.clone(),
        candidate_identity: candidate_identity.to_owned(),
        program_identity: package.manifest.logical_program_identity.clone(),
        package_identity: package.manifest.package_identity.clone(),
        session_identity: session.session_identity().to_owned(),
        config_identity: session.config_identity().to_owned(),
        policy_identity: session.policy_identity().to_owned(),
        exact_table_set_identity: exact_tables.identity().to_owned(),
        semantic_checksum,
        expected_result_contract: EXPECTED_RESULT_CONTRACT.into(),
        artifact_identity,
        receipt_identity: String::new(),
    };
    receipt.receipt_identity = framed([
        b"ontology-candidate-gate-receipt.v1".as_slice(),
        receipt.operation_id.as_bytes(),
        receipt.execution_identity.as_bytes(),
        receipt.candidate_identity.as_bytes(),
        receipt.program_identity.as_bytes(),
        receipt.package_identity.as_bytes(),
        receipt.session_identity.as_bytes(),
        receipt.config_identity.as_bytes(),
        receipt.policy_identity.as_bytes(),
        receipt.exact_table_set_identity.as_bytes(),
        receipt.semantic_checksum.as_bytes(),
        receipt.expected_result_contract.as_bytes(),
        receipt.artifact_identity.as_bytes(),
    ]);
    receipt
}

/// Trusted gate orchestrator. It computes all bindings itself and issues receipts only after the
/// sealed session returns zero violation rows.
pub struct CandidateClosureRunner {
    package: OntologyProgramPackage,
    publication: PublicationOutcome,
    session: GovernedSession,
    exact_tables: ExactTableSet,
    requirements: Vec<BootstrapRequirement>,
    expected_bindings: BTreeMap<String, String>,
    observed_bindings: BTreeMap<String, String>,
    candidate_identity: String,
    predecessor_epoch_identity: Option<String>,
    rollback_retain_until: i64,
}

impl CandidateClosureRunner {
    /// Build a closure runner from a digest-checked package and exact candidate manifest.
    ///
    /// # Errors
    ///
    /// Rejects an invalid package, publication, governed session, or closure binding.
    pub fn new(
        package: OntologyProgramPackage,
        publication: PublicationOutcome,
        session: GovernedSession,
    ) -> Result<Self, CandidateClosureError> {
        Self::new_for_epoch(package, publication, session, None, 0)
    }

    /// Build a closure runner whose sealed candidate is explicitly bound to its predecessor
    /// ontology epoch and rollback-retention deadline.
    ///
    /// # Errors
    ///
    /// Rejects an invalid package, publication, governed session, or epoch binding.
    pub fn new_for_epoch(
        package: OntologyProgramPackage,
        publication: PublicationOutcome,
        session: GovernedSession,
        predecessor_epoch_identity: Option<String>,
        rollback_retain_until: i64,
    ) -> Result<Self, CandidateClosureError> {
        Self::new_with_observed(
            package,
            publication,
            session,
            predecessor_epoch_identity,
            rollback_retain_until,
            None,
        )
    }

    fn new_with_observed(
        package: OntologyProgramPackage,
        publication: PublicationOutcome,
        session: GovernedSession,
        predecessor_epoch_identity: Option<String>,
        rollback_retain_until: i64,
        observed_overrides: Option<BTreeMap<String, String>>,
    ) -> Result<Self, CandidateClosureError> {
        let requirements = decode_bootstrap(&package)?;
        let exact_tables = ExactTableSet::from_publication(&publication)?;
        let expected_bindings = requirements
            .iter()
            .map(|requirement| {
                Ok((
                    requirement.family_id.clone(),
                    expected_binding_identity(
                        requirement,
                        &package,
                        &publication,
                        &session,
                        &exact_tables,
                    )?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, CandidateClosureError>>()?;
        if expected_bindings
            .values()
            .any(|value| !digest_is_valid(value))
        {
            return Err(CandidateClosureError::Invalid(
                "closure contains a malformed expected identity".into(),
            ));
        }
        let mut observed_bindings = expected_bindings.clone();
        if let Some(overrides) = observed_overrides {
            for (family, identity) in overrides {
                let slot = observed_bindings.get_mut(&family).ok_or_else(|| {
                    CandidateClosureError::Invalid(format!(
                        "observed binding names undiscovered family {family}"
                    ))
                })?;
                *slot = identity;
            }
        }
        let mut candidate_identity = candidate_identity(
            &package,
            &publication,
            &session,
            &exact_tables,
            &expected_bindings,
        );
        candidate_identity = framed([
            candidate_identity.as_bytes(),
            predecessor_epoch_identity
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
            rollback_retain_until.to_be_bytes().as_slice(),
        ]);
        Ok(Self {
            package,
            publication,
            session,
            exact_tables,
            requirements,
            expected_bindings,
            observed_bindings,
            candidate_identity,
            predecessor_epoch_identity,
            rollback_retain_until,
        })
    }

    /// Open and freeze the exact Delta provider graph from the candidate manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when any exact manifest-pinned provider cannot be opened.
    pub async fn open_frozen_catalog(
        &self,
    ) -> Result<crate::fabric::SnapshotProviderCatalog, CandidateClosureError> {
        crate::fabric::SnapshotProviderCatalog::build(
            &self.publication,
            &crate::fabric::EmptySnapshotOverlay,
        )
        .await
        .map_err(|error| CandidateClosureError::Invalid(error.to_string()))
    }

    /// Execute every bootstrap-discovered closure gate once and issue one opaque receipt each.
    ///
    /// # Errors
    ///
    /// Rejects planning, execution, resource, checksum, receipt, or binding failures.
    #[allow(clippy::too_many_lines)] // One pass preserves auditable gate-to-receipt causality.
    pub async fn execute(
        &self,
        limits: &GateResourceEnvelope,
    ) -> Result<CandidateClosureReport, CandidateClosureError> {
        let mut receipts = BTreeMap::new();
        let mut gate_evidence = BTreeMap::new();
        for requirement in &self.requirements {
            let expected = &self.expected_bindings[&requirement.family_id];
            let observed = &self.observed_bindings[&requirement.family_id];
            let plan = closure_plan(&requirement.family_id, observed, expected)?;
            let governed = self.session.seal_plan(plan)?;
            let execution_id = framed([
                b"ontology-closure-execution.v1".as_slice(),
                self.candidate_identity.as_bytes(),
                requirement.ordinal.to_be_bytes().as_slice(),
                requirement.family_id.as_bytes(),
            ]);
            let outcome = self
                .session
                .execute_gate(
                    &governed,
                    &execution_id,
                    &self.candidate_identity,
                    &requirement.family_id,
                    limits,
                )
                .await?;
            if outcome
                .batches
                .iter()
                .map(RecordBatch::num_rows)
                .sum::<usize>()
                != 0
            {
                return Err(CandidateClosureError::Invalid(format!(
                    "semantic closure rejected family {}",
                    requirement.family_id
                )));
            }
            let receipt = make_receipt(
                &requirement.family_id,
                &self.candidate_identity,
                &self.package,
                &self.session,
                &self.exact_tables,
                &outcome,
            );
            let artifact_bytes = canonical_value_bytes(&serde_json::json!({
                "execution_identity": outcome.artifact.execution_id,
                "candidate_identity": outcome.artifact.candidate_id,
                "operation_id": outcome.artifact.action_id,
                "terminal_action_count": outcome.artifact.terminal_action_count,
                "physical_plan_diagnostic": outcome.artifact.physical_plan_diagnostic,
                "metrics": outcome.artifact.metrics,
                "artifact_identity": outcome.artifact.artifact_identity,
            }))?;
            let receipt_bytes = canonical_value_bytes(&serde_json::json!({
                "operation_id": receipt.operation_id,
                "execution_identity": receipt.execution_identity,
                "candidate_identity": receipt.candidate_identity,
                "program_identity": receipt.program_identity,
                "package_identity": receipt.package_identity,
                "session_identity": receipt.session_identity,
                "config_identity": receipt.config_identity,
                "policy_identity": receipt.policy_identity,
                "exact_table_set_identity": receipt.exact_table_set_identity,
                "semantic_checksum": receipt.semantic_checksum,
                "expected_result_contract": receipt.expected_result_contract,
                "artifact_identity": receipt.artifact_identity,
                "receipt_identity": receipt.receipt_identity,
            }))?;
            let durable_gate = DurableGateEvidence {
                operation_id: requirement.family_id.clone(),
                execution_identity: outcome.receipt.execution_id.clone(),
                semantic_checksum: receipt.semantic_checksum.clone(),
                artifact_identity: receipt.artifact_identity.clone(),
                artifact_bytes,
                receipt_identity: receipt.receipt_identity.clone(),
                receipt_bytes,
                expected_result_contract: receipt.expected_result_contract.clone(),
            };
            if gate_evidence
                .insert(requirement.family_id.clone(), durable_gate)
                .is_some()
            {
                return Err(CandidateClosureError::Invalid(
                    "one operation produced multiple durable evidence records".into(),
                ));
            }
            if receipts
                .insert(requirement.family_id.clone(), receipt)
                .is_some()
            {
                return Err(CandidateClosureError::Invalid(
                    "one operation produced multiple receipts".into(),
                ));
            }
        }
        if receipts.len() != self.requirements.len()
            || receipts
                .values()
                .map(CandidateGateReceipt::identity)
                .collect::<BTreeSet<_>>()
                .len()
                != receipts.len()
        {
            return Err(CandidateClosureError::Invalid(
                "program/execution/checksum/artifact/receipt mapping is not bijective".into(),
            ));
        }
        let receipt_set_identity = framed(gate_evidence.values().map(|evidence| {
            framed([
                evidence.operation_id.as_bytes(),
                evidence.execution_identity.as_bytes(),
                evidence.semantic_checksum.as_bytes(),
                evidence.artifact_identity.as_bytes(),
                evidence.receipt_identity.as_bytes(),
            ])
        }));
        let exact_tables = self
            .exact_tables
            .bindings()
            .map(|binding| DurableExactTableBinding {
                table_code: binding.table_code,
                table_uri: binding.table_uri.clone(),
                delta_version: binding.delta_version,
                schema_identity: binding.schema_fingerprint,
                content_identity: binding.table_checksum,
            })
            .collect::<Vec<_>>();
        let manifest_bytes = canonical_value_bytes(&serde_json::json!({
            "candidate_identity": self.candidate_identity,
            "workspace_id": self.publication.scope.workspace_id,
            "program_identity": self.package.manifest.logical_program_identity,
            "package_identity": self.package.manifest.package_identity,
            "session_identity": self.session.session_identity(),
            "config_identity": self.session.config_identity(),
            "policy_identity": self.session.policy_identity(),
            "result_policy_identity": framed([b"candidate-policy.v1".as_slice(), self.session.policy_identity().as_bytes()]),
            "exact_table_set_identity": self.exact_tables.identity(),
            "function_catalog_identity": self.package.manifest.member_identities["program.calculation_catalog"],
            "query_form_identity": self.package.manifest.member_identities["program.phrase_operation"],
            "checksum_version": crate::ontology_program::result_checksum_version(&self.package)?,
            "predecessor_epoch_identity": self.predecessor_epoch_identity,
            "rollback_retain_until": self.rollback_retain_until,
            "receipt_set_identity": receipt_set_identity,
            "operation_count": self.requirements.len(),
        }))?;
        let function_catalog_identity =
            self.package.manifest.member_identities["program.calculation_catalog"].clone();
        let query_form_identity =
            self.package.manifest.member_identities["program.phrase_operation"].clone();
        let checksum_version = crate::ontology_program::result_checksum_version(&self.package)?;
        let result_policy_identity = framed([
            b"candidate-policy.v1".as_slice(),
            self.session.policy_identity().as_bytes(),
        ]);
        let result_authority_identity = framed([
            b"ontology-result-authority.v1".as_slice(),
            self.package.manifest.logical_program_identity.as_bytes(),
            function_catalog_identity.as_bytes(),
            result_policy_identity.as_bytes(),
            query_form_identity.as_bytes(),
            checksum_version.as_bytes(),
            self.exact_tables.identity().as_bytes(),
        ]);
        let durable = DurableCandidateEvidence {
            candidate_identity: self.candidate_identity.clone(),
            workspace_id: self.publication.scope.workspace_id,
            manifest_digest: crate::integrity::framed_digest(&manifest_bytes),
            manifest_bytes,
            program_identity: self.package.manifest.logical_program_identity.clone(),
            package_identity: self.package.manifest.package_identity.clone(),
            config_identity: self.session.config_identity().to_owned(),
            policy_identity: self.session.policy_identity().to_owned(),
            result_policy_identity,
            exact_table_set_identity: self.exact_tables.identity().to_owned(),
            function_catalog_identity,
            query_form_identity,
            checksum_version,
            result_authority_identity,
            predecessor_epoch_identity: self.predecessor_epoch_identity.clone(),
            rollback_retain_until: self.rollback_retain_until,
            exact_tables,
            gate_evidence: gate_evidence.into_values().collect(),
            receipt_set_identity,
        };
        Ok(CandidateClosureReport {
            candidate_identity: self.candidate_identity.clone(),
            requirements: self.requirements.clone(),
            receipts,
            durable,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray};
    use arrow_ipc::writer::StreamWriter;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::prelude::SessionConfig;

    use super::{CandidateClosureRunner, ExactTableSet, decode_bootstrap, framed};
    use crate::fabric::{
        CurrentPublicationRecord, PublicationOutcome, PublicationScope, PublicationTableRecord,
    };
    use crate::governed_session::GovernedSession;
    use crate::ontology_gate::GateResourceEnvelope;
    use crate::ontology_program::{
        OntologyPackagingProfile, OntologyProgramPackage, build_ontology_program_package,
        reseal_ontology_program_package,
    };

    fn publication() -> PublicationOutcome {
        let publication_id = [0x31; 16];
        PublicationOutcome {
            publication_id,
            scope: PublicationScope {
                workspace_id: [0x32; 16],
                source_generation: 7,
                analysis_context_set_id: [0x33; 16],
                analysis_context_ids: vec![[0x34; 16]],
            },
            pointer: CurrentPublicationRecord {
                workspace_id: [0x32; 16],
                publication_id,
                pointer_generation: 5,
                updated_at_micros: 1_000,
            },
            tables: BTreeMap::from([(
                1,
                PublicationTableRecord {
                    publication_id,
                    workspace_id: [0x32; 16],
                    table_code: 1,
                    table_uri: "file:///candidate/workspace".into(),
                    delta_version: 9,
                    schema_fingerprint: [0x35; 32],
                    row_count: 1,
                    owner_count: 1,
                    table_checksum: [0x36; 32],
                    primary_key_digest: [0x37; 32],
                    required: true,
                    validated: true,
                },
            )]),
        }
    }

    fn runner() -> CandidateClosureRunner {
        CandidateClosureRunner::new(
            build_ontology_program_package(&OntologyPackagingProfile::default())
                .expect("program package"),
            publication(),
            GovernedSession::new(SessionConfig::new(), "policy.ontology.v1")
                .expect("governed session"),
        )
        .expect("candidate runner")
    }

    #[tokio::test]
    async fn ontology_bootstrap_program_package_closure() {
        let runner = runner();
        let requirements = decode_bootstrap(&runner.package).expect("bootstrap closure");
        assert_eq!(requirements.len(), runner.package.members.len() - 1);
        assert!(
            requirements
                .iter()
                .any(|value| value.family_id == "program.rule_operation")
        );
        let report = runner
            .execute(&GateResourceEnvelope::default())
            .await
            .expect("complete closure");
        assert_eq!(report.operation_count(), requirements.len());
        assert_eq!(report.receipt_count(), requirements.len());
    }

    #[tokio::test]
    async fn ontology_semantic_closure_corruption_matrix() {
        let base = runner();
        let families = base
            .requirements
            .iter()
            .map(|value| value.family_id.clone())
            .collect::<Vec<_>>();
        drop(base);
        for family in families {
            let corrupted = CandidateClosureRunner::new_with_observed(
                build_ontology_program_package(&OntologyPackagingProfile::default())
                    .expect("program package"),
                publication(),
                GovernedSession::new(SessionConfig::new(), "policy.ontology.v1")
                    .expect("governed session"),
                None,
                0,
                Some(BTreeMap::from([(
                    family.clone(),
                    "b3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
                )])),
            )
            .expect("corruption fixture");
            let error = corrupted
                .execute(&GateResourceEnvelope::default())
                .await
                .expect_err("every corrupted authority family must reject");
            assert!(error.to_string().contains(&family), "{family}: {error}");
        }
    }

    fn append_additive_family(package: &mut OntologyProgramPackage) {
        let additive_relation = "authority.additive_domain";
        let mut additive = package
            .members
            .get("program.enum_value")
            .expect("source member")
            .clone();
        additive.relation_id = additive_relation.into();
        additive.member_identity = framed([additive_relation.as_bytes(), &additive.ipc_bytes]);
        package.members.insert(additive_relation.into(), additive);

        let content_rows = package
            .members
            .iter()
            .filter(|(relation, _)| relation.as_str() != "program.bootstrap")
            .map(|(relation, member)| {
                (
                    relation.clone(),
                    format!(
                        "b3:{}",
                        blake3::hash(format!("{:?}", member.schema).as_bytes()).to_hex()
                    ),
                    format!("b3:{}", blake3::hash(&member.ipc_bytes).to_hex()),
                )
            })
            .collect::<Vec<_>>();
        let mut content_set = blake3::Hasher::new();
        content_set.update(b"codefabric.ontology-program.content-set.v1\0");
        for (relation, schema_identity, content_identity) in &content_rows {
            for value in [relation, schema_identity, content_identity] {
                content_set.update(&(value.len() as u64).to_be_bytes());
                content_set.update(value.as_bytes());
            }
        }
        let content_set_identity = format!("b3:{}", content_set.finalize().to_hex());
        let schema = Arc::new(Schema::new(vec![
            Field::new("relation_id", DataType::Utf8, false),
            Field::new("member_address", DataType::Utf8, false),
            Field::new("relation_role", DataType::Utf8, false),
            Field::new("schema_identity", DataType::Utf8, false),
            Field::new("content_identity", DataType::Utf8, false),
            Field::new("required", DataType::Boolean, false),
            Field::new("content_set_identity", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(StringArray::from_iter_values(
                    content_rows.iter().map(|row| row.0.as_str()),
                )) as ArrayRef,
                Arc::new(StringArray::from_iter_values(
                    content_rows.iter().map(|row| row.0.as_str()),
                )),
                Arc::new(StringArray::from(vec![
                    "program_relation";
                    content_rows.len()
                ])),
                Arc::new(StringArray::from_iter_values(
                    content_rows.iter().map(|row| row.1.as_str()),
                )),
                Arc::new(StringArray::from_iter_values(
                    content_rows.iter().map(|row| row.2.as_str()),
                )),
                Arc::new(BooleanArray::from(vec![true; content_rows.len()])),
                Arc::new(StringArray::from(vec![
                    content_set_identity.as_str();
                    content_rows.len()
                ])),
            ],
        )
        .expect("additive bootstrap batch");
        assert_eq!(batch.num_rows(), content_rows.len());
        let member = package
            .members
            .get_mut("program.bootstrap")
            .expect("bootstrap member");
        let mut ipc_bytes = Vec::new();
        {
            let mut writer =
                StreamWriter::try_new(&mut ipc_bytes, schema.as_ref()).expect("IPC writer");
            writer.write(&batch).expect("write bootstrap");
            writer.finish().expect("finish bootstrap");
        }
        member.schema = schema;
        member.batches = vec![batch];
        member.ipc_bytes = ipc_bytes;
        member.member_identity =
            framed([member.relation_id.as_bytes(), member.ipc_bytes.as_slice()]);
        reseal_ontology_program_package(package).expect("reseal additive package");
    }

    #[tokio::test]
    async fn ontology_self_description_additive_relation() {
        let mut package =
            build_ontology_program_package(&OntologyPackagingProfile::default()).expect("package");
        append_additive_family(&mut package);
        let expected = package.members.len() - 1;
        let runner = CandidateClosureRunner::new(
            package,
            publication(),
            GovernedSession::new(SessionConfig::new(), "policy.ontology.v1")
                .expect("governed session"),
        )
        .expect("additive runner");
        let report = runner
            .execute(&GateResourceEnvelope::default())
            .await
            .expect("additive closure");
        assert_eq!(report.operation_count(), expected);
        assert_eq!(report.receipt_count(), expected);
    }

    #[tokio::test]
    async fn ontology_program_execution_receipt_bijection() {
        let report = runner()
            .execute(&GateResourceEnvelope::default())
            .await
            .expect("closure report");
        assert_eq!(report.operation_count(), report.receipt_count());
        assert_eq!(report.receipt_identities().len(), report.receipt_count());
        assert!(super::digest_is_valid(report.candidate_identity()));
    }

    #[test]
    fn ontology_candidate_delta_binding_exact_version() {
        let publication = publication();
        let tables = ExactTableSet::from_publication(&publication).expect("exact table set");
        let binding = tables.bindings().next().expect("table binding");
        assert_eq!(binding.table_code(), 1);
        assert_eq!(binding.table_uri(), "file:///candidate/workspace");
        assert_eq!(binding.delta_version(), 9);
        let mut invalid = publication;
        invalid.tables.get_mut(&1).expect("table").validated = false;
        assert!(ExactTableSet::from_publication(&invalid).is_err());
    }

    #[tokio::test]
    async fn ontology_plan_artifact_receipt_boundary() {
        let report = runner()
            .execute(&GateResourceEnvelope::default())
            .await
            .expect("closure report");
        assert!(report.receipts.values().all(|receipt| {
            receipt.artifact_identity != receipt.receipt_identity
                && receipt.semantic_checksum != receipt.artifact_identity
                && receipt.expected_result_contract == super::EXPECTED_RESULT_CONTRACT
        }));
    }
}
