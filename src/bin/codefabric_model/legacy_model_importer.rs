//! One-time, caller-fed conversion of frozen predecessor evidence into an initial model migration.
//!
//! This module deliberately has no filesystem reader, daemon port, CLI command, or generated-input
//! discovery. A bounded migration harness supplies decoded legacy evidence and an independently
//! authored review through separate ports. The importer accepts the batch only when every source
//! row has exactly one explicit disposition, every candidate decision is covered without duplicate
//! semantic authority, and the independent v2 expectations match the complete decision set.
//! Legacy rows can explain a disposition, but they never constrain the value of a target model
//! decision: the current v2 model and provider contracts govern those expectations.

use std::collections::{BTreeMap, BTreeSet};

use super::relational_model::{
    BootstrapMetamodel, ModelDecision, ModelError, ModelMigration, ModelOperation, ModelRelation,
    ModelValue, RowReference,
};

const MAX_TEXT_BYTES: usize = 1_024;

/// Semantic areas whose initial rows require an independently authored expectation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticReviewDomain {
    ModelType,
    Authority,
    Normalization,
    Unknown,
    Query,
    Policy,
    State,
    Proof,
}

impl SemanticReviewDomain {
    /// Complete review coverage required before the initial migration can be accepted.
    pub const ALL: [Self; 8] = [
        Self::ModelType,
        Self::Authority,
        Self::Normalization,
        Self::Unknown,
        Self::Query,
        Self::Policy,
        Self::State,
        Self::Proof,
    ];
}

/// Closed classes of caller-supplied predecessor evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyEvidenceClass {
    ModelDecision,
    ReleasedPublicId,
    CanonicalIdentityRule,
    WireAllocation,
    /// Historical acceptance records provenance; it confers no target-model authority.
    AcceptedHistoricalDecision,
    RetainedSemanticMeaning,
    TombstoneCommitment,
}

impl LegacyEvidenceClass {
    const fn is_released_commitment(self) -> bool {
        matches!(
            self,
            Self::ReleasedPublicId | Self::CanonicalIdentityRule | Self::WireAllocation
        )
    }
}

/// One decoded legacy row. Bytes remain caller-owned evidence and are never read by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyEvidenceRow {
    legacy_row_id: String,
    evidence_class: LegacyEvidenceClass,
    source_artifact_id: String,
    source_row_identity: String,
    source_content_identity: String,
}

impl LegacyEvidenceRow {
    /// Construct one immutable evidence-row identity.
    ///
    /// # Errors
    ///
    /// Returns an input error when an identifier, locator, or content identity is malformed.
    pub fn new(
        legacy_row_id: impl Into<String>,
        evidence_class: LegacyEvidenceClass,
        source_artifact_id: impl Into<String>,
        source_row_identity: impl Into<String>,
        source_content_identity: impl Into<String>,
    ) -> Result<Self, LegacyImportError> {
        let row = Self {
            legacy_row_id: legacy_row_id.into(),
            evidence_class,
            source_artifact_id: source_artifact_id.into(),
            source_row_identity: source_row_identity.into(),
            source_content_identity: source_content_identity.into(),
        };
        require_identifier(&row.legacy_row_id, "legacy row")?;
        require_text(&row.source_artifact_id, "source artifact")?;
        require_text(&row.source_row_identity, "source row identity")?;
        require_content_identity(&row.source_content_identity, "legacy source")?;
        Ok(row)
    }

    /// Stable row identity within the frozen import universe.
    #[must_use]
    pub fn legacy_row_id(&self) -> &str {
        &self.legacy_row_id
    }
}

/// One v2-governed candidate decision associated with its independently reviewed domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedDecisionCandidate {
    domain: SemanticReviewDomain,
    decision: ModelDecision,
}

impl ImportedDecisionCandidate {
    /// Bind an importer candidate to the semantic review domain it changes.
    #[must_use]
    pub const fn new(domain: SemanticReviewDomain, decision: ModelDecision) -> Self {
        Self { domain, decision }
    }
}

/// Exhaustive disposition values for predecessor rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDispositionKind {
    Migrated,
    Combined,
    Split,
    Superseded,
    Tombstoned,
    /// A compatibility commitment retained by an independently reviewed v2 target decision.
    PreservedReleasedCommitment,
    RejectedFalseStatic,
}

impl LegacyDispositionKind {
    const fn has_no_target(self) -> bool {
        matches!(
            self,
            Self::Superseded | Self::Tombstoned | Self::RejectedFalseStatic
        )
    }
}

/// One explicit predecessor-to-target reconciliation row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDispositionRow {
    legacy_row_id: String,
    kind: LegacyDispositionKind,
    target_decision_ids: Vec<String>,
    rationale: String,
}

impl LegacyDispositionRow {
    /// Construct a disposition; graph cardinality is validated against the complete batch.
    ///
    /// # Errors
    ///
    /// Returns an input error when an identifier or rationale is malformed.
    pub fn new(
        legacy_row_id: impl Into<String>,
        kind: LegacyDispositionKind,
        target_decision_ids: impl IntoIterator<Item = impl Into<String>>,
        rationale: impl Into<String>,
    ) -> Result<Self, LegacyImportError> {
        let row = Self {
            legacy_row_id: legacy_row_id.into(),
            kind,
            target_decision_ids: target_decision_ids.into_iter().map(Into::into).collect(),
            rationale: rationale.into(),
        };
        require_identifier(&row.legacy_row_id, "disposition legacy row")?;
        require_text(&row.rationale, "disposition rationale")?;
        for target in &row.target_decision_ids {
            require_identifier(target, "disposition target decision")?;
        }
        Ok(row)
    }

    /// Legacy row classified by this disposition.
    #[must_use]
    pub fn legacy_row_id(&self) -> &str {
        &self.legacy_row_id
    }

    /// Accepted disposition category.
    #[must_use]
    pub const fn kind(&self) -> LegacyDispositionKind {
        self.kind
    }

    /// Exact target decisions, empty only for terminal non-migration dispositions.
    #[must_use]
    pub fn target_decision_ids(&self) -> &[String] {
        &self.target_decision_ids
    }
}

/// One independently authored exact v2 expectation for a candidate decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentDecisionExpectation {
    domain: SemanticReviewDomain,
    expected_decision: ModelDecision,
}

impl IndependentDecisionExpectation {
    /// Construct one expectation without consulting importer output.
    #[must_use]
    pub const fn new(domain: SemanticReviewDomain, expected_decision: ModelDecision) -> Self {
        Self {
            domain,
            expected_decision,
        }
    }
}

/// Complete separately owned v2 expectation input for initial-model acceptance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentMigrationReview {
    review_id: String,
    reviewed_by: String,
    review_source_identity: String,
    expectations: Vec<IndependentDecisionExpectation>,
}

impl IndependentMigrationReview {
    /// Construct a review input. Independence from importer/evidence is checked at consumption.
    ///
    /// # Errors
    ///
    /// Returns an input or review error when provenance or expectations are incomplete.
    pub fn new(
        review_id: impl Into<String>,
        reviewed_by: impl Into<String>,
        review_source_identity: impl Into<String>,
        expectations: Vec<IndependentDecisionExpectation>,
    ) -> Result<Self, LegacyImportError> {
        let review = Self {
            review_id: review_id.into(),
            reviewed_by: reviewed_by.into(),
            review_source_identity: review_source_identity.into(),
            expectations,
        };
        require_identifier(&review.review_id, "independent review")?;
        require_identifier(&review.reviewed_by, "independent reviewer")?;
        require_content_identity(&review.review_source_identity, "independent review")?;
        if review.expectations.is_empty() {
            return Err(LegacyImportError::Review(
                "independent review contains no decision expectations".to_owned(),
            ));
        }
        Ok(review)
    }
}

/// Separately implemented source of independently authored expectations.
pub trait IndependentMigrationReviewPort {
    /// Load one immutable review input without receiving importer candidates as an argument.
    ///
    /// # Errors
    ///
    /// Returns a review error when the separately owned input is unavailable or invalid.
    fn load_review(&self) -> Result<IndependentMigrationReview, LegacyImportError>;
}

/// Fixed identity request for construction of the first accepted migration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_field_names)] // Every field names the distinct identity it carries.
pub struct InitialMigrationRequest {
    import_batch_id: String,
    migration_id: String,
    predecessor_model_epoch_id: String,
    target_model_epoch_id: String,
}

impl InitialMigrationRequest {
    /// Construct the immutable identity envelope for ordinal-one migration acceptance.
    ///
    /// # Errors
    ///
    /// Returns an input error for malformed or self-referential migration identities.
    pub fn new(
        import_batch_id: impl Into<String>,
        migration_id: impl Into<String>,
        predecessor_model_epoch_id: impl Into<String>,
        target_model_epoch_id: impl Into<String>,
    ) -> Result<Self, LegacyImportError> {
        let request = Self {
            import_batch_id: import_batch_id.into(),
            migration_id: migration_id.into(),
            predecessor_model_epoch_id: predecessor_model_epoch_id.into(),
            target_model_epoch_id: target_model_epoch_id.into(),
        };
        for (value, context) in [
            (&request.import_batch_id, "import batch"),
            (&request.migration_id, "initial migration"),
            (
                &request.predecessor_model_epoch_id,
                "predecessor model epoch",
            ),
            (&request.target_model_epoch_id, "target model epoch"),
        ] {
            require_identifier(value, context)?;
        }
        if request.predecessor_model_epoch_id == request.target_model_epoch_id {
            return Err(LegacyImportError::Input(
                "initial migration predecessor and target epochs must differ".to_owned(),
            ));
        }
        Ok(request)
    }
}

/// Validated row-level reconciliation evidence retained beside the accepted migration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciledDisposition {
    legacy_row_id: String,
    evidence_class: LegacyEvidenceClass,
    source_artifact_id: String,
    source_row_identity: String,
    source_content_identity: String,
    kind: LegacyDispositionKind,
    target_decision_ids: Vec<String>,
    rationale: String,
}

impl ReconciledDisposition {
    /// Classified legacy row.
    #[must_use]
    pub fn legacy_row_id(&self) -> &str {
        &self.legacy_row_id
    }

    /// Classified predecessor-evidence family.
    #[must_use]
    pub const fn evidence_class(&self) -> LegacyEvidenceClass {
        self.evidence_class
    }

    /// Artifact authority from which the legacy row was decoded.
    #[must_use]
    pub fn source_artifact_id(&self) -> &str {
        &self.source_artifact_id
    }

    /// Stable row coordinate within the source artifact.
    #[must_use]
    pub fn source_row_identity(&self) -> &str {
        &self.source_row_identity
    }

    /// Disposition established by the total reconciliation graph.
    #[must_use]
    pub const fn kind(&self) -> LegacyDispositionKind {
        self.kind
    }

    /// Exact accepted target decisions.
    #[must_use]
    pub fn target_decision_ids(&self) -> &[String] {
        &self.target_decision_ids
    }

    /// Exact frozen content identity from which this row was decoded.
    #[must_use]
    pub fn source_content_identity(&self) -> &str {
        &self.source_content_identity
    }

    /// Human-authored explanation for the explicit disposition.
    #[must_use]
    pub fn rationale(&self) -> &str {
        &self.rationale
    }
}

/// Deterministic acceptance report proving total source and target coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyImportReport {
    import_batch_id: String,
    review_id: String,
    reviewed_by: String,
    review_source_identity: String,
    legacy_input_count: usize,
    accepted_decision_ids: Vec<String>,
    dispositions: Vec<ReconciledDisposition>,
}

impl LegacyImportReport {
    /// Stable identity for this closed import universe.
    #[must_use]
    pub fn import_batch_id(&self) -> &str {
        &self.import_batch_id
    }

    /// Stable identity of the separately supplied review input.
    #[must_use]
    pub fn review_id(&self) -> &str {
        &self.review_id
    }

    /// Number of frozen evidence rows classified exactly once.
    #[must_use]
    pub const fn legacy_input_count(&self) -> usize {
        self.legacy_input_count
    }

    /// Independent reviewer whose exact expectations accepted this batch.
    #[must_use]
    pub fn reviewed_by(&self) -> &str {
        &self.reviewed_by
    }

    /// Exact content identity of the separately authored expectation input.
    #[must_use]
    pub fn review_source_identity(&self) -> &str {
        &self.review_source_identity
    }

    /// Sorted target decision identities included in the migration.
    #[must_use]
    pub fn accepted_decision_ids(&self) -> &[String] {
        &self.accepted_decision_ids
    }

    /// Sorted, total row-level reconciliation report.
    #[must_use]
    pub fn dispositions(&self) -> &[ReconciledDisposition] {
        &self.dispositions
    }
}

/// Accepted initial migration paired with the independently reviewed disposition proof.
pub struct AcceptedInitialMigration {
    migration: ModelMigration,
    report: LegacyImportReport,
}

impl AcceptedInitialMigration {
    /// Ordinal-one typed migration suitable for replay.
    #[must_use]
    pub const fn migration(&self) -> &ModelMigration {
        &self.migration
    }

    /// Total reconciliation and independent-review report.
    #[must_use]
    pub const fn report(&self) -> &LegacyImportReport {
        &self.report
    }
}

/// Failures are closed: importer output is never partially accepted.
#[derive(Debug, thiserror::Error)]
pub enum LegacyImportError {
    #[error("legacy import input rejected: {0}")]
    Input(String),
    #[error("legacy disposition closure rejected: {0}")]
    Disposition(String),
    #[error("legacy semantic authority rejected: {0}")]
    SemanticAuthority(String),
    #[error("independent migration review rejected: {0}")]
    Review(String),
    #[error("initial model migration rejected: {0}")]
    Model(#[from] ModelError),
}

/// Pure one-time importer. It owns no reader, writer, catalog, or runtime authority.
pub struct LegacyModelImporter {
    importer_identity: String,
}

impl LegacyModelImporter {
    /// Bind the importer executable identity used to reject self-authored review input.
    ///
    /// # Errors
    ///
    /// Returns an input error when the importer identity is not bounded and canonical.
    pub fn new(importer_identity: impl Into<String>) -> Result<Self, LegacyImportError> {
        let importer_identity = importer_identity.into();
        require_identifier(&importer_identity, "legacy importer")?;
        Ok(Self { importer_identity })
    }

    /// Reconcile all caller-supplied rows and construct the independently accepted initial event.
    ///
    /// Evidence participates only in total disposition closure. Target decision values come from
    /// `candidates` and must match the separately loaded v2 expectations exactly.
    ///
    /// # Errors
    ///
    /// Returns a closed import error on any incomplete disposition, duplicate authority,
    /// non-independent expectation, or invalid migration construction.
    #[allow(clippy::too_many_lines)]
    pub fn import<R: IndependentMigrationReviewPort>(
        &self,
        request: InitialMigrationRequest,
        evidence_rows: Vec<LegacyEvidenceRow>,
        candidates: Vec<ImportedDecisionCandidate>,
        dispositions: Vec<LegacyDispositionRow>,
        review_port: &R,
    ) -> Result<AcceptedInitialMigration, LegacyImportError> {
        let evidence = index_evidence(evidence_rows)?;
        let candidates = index_candidates(candidates)?;
        let dispositions = reconcile_dispositions(&evidence, &candidates, dispositions)?;
        reject_duplicate_semantic_authority(&candidates)?;

        let review = review_port.load_review()?;
        validate_independent_review(&self.importer_identity, &evidence, &candidates, &review)?;

        let accepted_decision_ids = candidates.keys().cloned().collect::<Vec<_>>();
        let decisions = candidates
            .values()
            .map(|candidate| candidate.decision.clone())
            .collect();
        let migration = ModelMigration::new(
            request.migration_id,
            None,
            request.predecessor_model_epoch_id,
            request.target_model_epoch_id,
            1,
            review.reviewed_by.clone(),
            decisions,
        )?;
        let report = LegacyImportReport {
            import_batch_id: request.import_batch_id,
            review_id: review.review_id,
            reviewed_by: review.reviewed_by,
            review_source_identity: review.review_source_identity,
            legacy_input_count: evidence.len(),
            accepted_decision_ids,
            dispositions,
        };
        Ok(AcceptedInitialMigration { migration, report })
    }
}

fn index_evidence(
    rows: Vec<LegacyEvidenceRow>,
) -> Result<BTreeMap<String, LegacyEvidenceRow>, LegacyImportError> {
    if rows.is_empty() {
        return Err(LegacyImportError::Input(
            "legacy evidence universe is empty".to_owned(),
        ));
    }
    let mut indexed = BTreeMap::new();
    let mut source_rows = BTreeSet::new();
    for row in rows {
        let row_id = row.legacy_row_id.clone();
        let source_key = (
            row.source_artifact_id.clone(),
            row.source_row_identity.clone(),
        );
        if !source_rows.insert(source_key) {
            return Err(LegacyImportError::Input(format!(
                "legacy row {row_id} duplicates a source artifact/row identity"
            )));
        }
        if indexed.insert(row_id.clone(), row).is_some() {
            return Err(LegacyImportError::Input(format!(
                "duplicate legacy row {row_id}"
            )));
        }
    }
    Ok(indexed)
}

fn index_candidates(
    candidates: Vec<ImportedDecisionCandidate>,
) -> Result<BTreeMap<String, ImportedDecisionCandidate>, LegacyImportError> {
    if candidates.is_empty() {
        return Err(LegacyImportError::Input(
            "candidate decision universe is empty".to_owned(),
        ));
    }
    let mut indexed = BTreeMap::new();
    for candidate in candidates {
        let decision_id = candidate.decision.decision_id().to_owned();
        if indexed.insert(decision_id.clone(), candidate).is_some() {
            return Err(LegacyImportError::SemanticAuthority(format!(
                "duplicate candidate decision {decision_id}"
            )));
        }
    }
    Ok(indexed)
}

#[allow(clippy::too_many_lines)]
fn reconcile_dispositions(
    evidence: &BTreeMap<String, LegacyEvidenceRow>,
    candidates: &BTreeMap<String, ImportedDecisionCandidate>,
    rows: Vec<LegacyDispositionRow>,
) -> Result<Vec<ReconciledDisposition>, LegacyImportError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let row_id = row.legacy_row_id.clone();
        if !evidence.contains_key(&row_id) {
            return Err(LegacyImportError::Disposition(format!(
                "disposition references unknown legacy row {row_id}"
            )));
        }
        if indexed.insert(row_id.clone(), row).is_some() {
            return Err(LegacyImportError::Disposition(format!(
                "legacy row {row_id} has more than one disposition"
            )));
        }
    }
    if indexed.len() != evidence.len() {
        let missing = evidence
            .keys()
            .filter(|row_id| !indexed.contains_key(*row_id))
            .cloned()
            .collect::<Vec<_>>();
        return Err(LegacyImportError::Disposition(format!(
            "legacy rows lack dispositions: {}",
            missing.join(",")
        )));
    }

    let mut target_sources: BTreeMap<String, Vec<(String, LegacyDispositionKind)>> =
        BTreeMap::new();
    for (row_id, disposition) in &indexed {
        let unique_targets = disposition
            .target_decision_ids
            .iter()
            .collect::<BTreeSet<_>>();
        if unique_targets.len() != disposition.target_decision_ids.len() {
            return Err(LegacyImportError::Disposition(format!(
                "legacy row {row_id} repeats a target decision"
            )));
        }
        let target_count = disposition.target_decision_ids.len();
        let valid_cardinality = match disposition.kind {
            LegacyDispositionKind::Migrated
            | LegacyDispositionKind::Combined
            | LegacyDispositionKind::PreservedReleasedCommitment => target_count == 1,
            LegacyDispositionKind::Split => target_count >= 2,
            kind if kind.has_no_target() => target_count == 0,
            _ => false,
        };
        if !valid_cardinality {
            return Err(LegacyImportError::Disposition(format!(
                "legacy row {row_id} has invalid {:?} target cardinality {target_count}",
                disposition.kind
            )));
        }
        let evidence_row = &evidence[row_id];
        if disposition.kind == LegacyDispositionKind::PreservedReleasedCommitment
            && !evidence_row.evidence_class.is_released_commitment()
        {
            return Err(LegacyImportError::Disposition(format!(
                "legacy row {row_id} is not a released commitment"
            )));
        }
        for target in &disposition.target_decision_ids {
            if !candidates.contains_key(target) {
                return Err(LegacyImportError::Disposition(format!(
                    "legacy row {row_id} targets unknown decision {target}"
                )));
            }
            target_sources
                .entry(target.clone())
                .or_default()
                .push((row_id.clone(), disposition.kind));
        }
    }

    let uncovered = candidates
        .keys()
        .filter(|decision_id| !target_sources.contains_key(*decision_id))
        .cloned()
        .collect::<Vec<_>>();
    if !uncovered.is_empty() {
        return Err(LegacyImportError::Disposition(format!(
            "candidate decisions lack legacy disposition coverage: {}",
            uncovered.join(",")
        )));
    }
    for (target, sources) in &target_sources {
        if sources.len() > 1
            && !sources
                .iter()
                .all(|(_, kind)| *kind == LegacyDispositionKind::Combined)
        {
            return Err(LegacyImportError::Disposition(format!(
                "decision {target} has overlapping non-combined legacy sources"
            )));
        }
        if sources.len() == 1 && sources[0].1 == LegacyDispositionKind::Combined {
            return Err(LegacyImportError::Disposition(format!(
                "decision {target} is marked combined but has only one legacy source"
            )));
        }
    }

    Ok(indexed
        .into_iter()
        .map(|(row_id, disposition)| {
            let source = &evidence[&row_id];
            ReconciledDisposition {
                legacy_row_id: row_id,
                evidence_class: source.evidence_class,
                source_artifact_id: source.source_artifact_id.clone(),
                source_row_identity: source.source_row_identity.clone(),
                source_content_identity: source.source_content_identity.clone(),
                kind: disposition.kind,
                target_decision_ids: disposition.target_decision_ids,
                rationale: disposition.rationale,
            }
        })
        .collect())
}

fn reject_duplicate_semantic_authority(
    candidates: &BTreeMap<String, ImportedDecisionCandidate>,
) -> Result<(), LegacyImportError> {
    let metamodel = BootstrapMetamodel::new();
    let mut claims: BTreeMap<(ModelRelation, Vec<ModelValue>), String> = BTreeMap::new();
    for (decision_id, candidate) in candidates {
        let mut decision_claims = BTreeSet::new();
        for operation in candidate.decision.operations() {
            let mut operation_claims = BTreeSet::new();
            match operation {
                ModelOperation::Add(row) => {
                    let reference = RowReference::for_row(row, &metamodel)?;
                    operation_claims.insert((reference.relation(), reference.key().to_vec()));
                }
                ModelOperation::Supersede { prior, replacement } => {
                    operation_claims.insert((prior.relation(), prior.key().to_vec()));
                    let replacement = RowReference::for_row(replacement, &metamodel)?;
                    operation_claims.insert((replacement.relation(), replacement.key().to_vec()));
                }
                ModelOperation::Retire(prior) => {
                    operation_claims.insert((prior.relation(), prior.key().to_vec()));
                }
                ModelOperation::AddData(_)
                | ModelOperation::SupersedeData { .. }
                | ModelOperation::RetireData(_) => {
                    return Err(LegacyImportError::SemanticAuthority(
                        "the one-time legacy importer may establish model schemas but cannot author rows in model-defined relations; those rows require a separately accepted replay migration"
                            .to_owned(),
                    ));
                }
            }
            for claim in operation_claims {
                if !decision_claims.insert(claim) {
                    return Err(LegacyImportError::SemanticAuthority(format!(
                        "decision {decision_id} claims the same model row more than once"
                    )));
                }
            }
        }
        for claim in decision_claims {
            if let Some(existing) = claims.insert(claim, decision_id.clone()) {
                return Err(LegacyImportError::SemanticAuthority(format!(
                    "decisions {existing} and {decision_id} claim the same model row"
                )));
            }
        }
    }
    Ok(())
}

fn validate_independent_review(
    importer_identity: &str,
    evidence: &BTreeMap<String, LegacyEvidenceRow>,
    candidates: &BTreeMap<String, ImportedDecisionCandidate>,
    review: &IndependentMigrationReview,
) -> Result<(), LegacyImportError> {
    if review.reviewed_by == importer_identity {
        return Err(LegacyImportError::Review(
            "reviewer identity equals importer identity".to_owned(),
        ));
    }
    if evidence
        .values()
        .any(|row| row.source_content_identity == review.review_source_identity)
    {
        return Err(LegacyImportError::Review(
            "review input reuses a legacy evidence content identity".to_owned(),
        ));
    }

    let mut expectations = BTreeMap::new();
    for expectation in &review.expectations {
        let decision_id = expectation.expected_decision.decision_id().to_owned();
        if expectations
            .insert(decision_id.clone(), expectation)
            .is_some()
        {
            return Err(LegacyImportError::Review(format!(
                "duplicate expectation for decision {decision_id}"
            )));
        }
    }
    let candidate_ids = candidates.keys().cloned().collect::<BTreeSet<_>>();
    let expectation_ids = expectations.keys().cloned().collect::<BTreeSet<_>>();
    if candidate_ids != expectation_ids {
        return Err(LegacyImportError::Review(
            "expectation decision set differs from importer candidate set".to_owned(),
        ));
    }

    let mut reviewed_domains = BTreeSet::new();
    for (decision_id, candidate) in candidates {
        let expectation = expectations[decision_id];
        if expectation.domain != candidate.domain
            || expectation.expected_decision != candidate.decision
        {
            return Err(LegacyImportError::Review(format!(
                "expectation differs for decision {decision_id}"
            )));
        }
        reviewed_domains.insert(expectation.domain);
    }
    let required_domains = SemanticReviewDomain::ALL
        .into_iter()
        .collect::<BTreeSet<_>>();
    if reviewed_domains != required_domains {
        return Err(LegacyImportError::Review(
            "semantic review does not cover model type, authority, normalization, unknown, query, policy, state, and proof decisions".to_owned(),
        ));
    }
    Ok(())
}

fn require_identifier(value: &str, context: &str) -> Result<(), LegacyImportError> {
    if value.is_empty()
        || value.len() > 240
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(LegacyImportError::Input(format!(
            "{context} is not a bounded identifier: {value:?}"
        )));
    }
    Ok(())
}

fn require_text(value: &str, context: &str) -> Result<(), LegacyImportError> {
    if value.trim().is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(LegacyImportError::Input(format!(
            "{context} is empty, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn require_content_identity(value: &str, context: &str) -> Result<(), LegacyImportError> {
    if value.len() != 67
        || !value.starts_with("b3:")
        || !value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(LegacyImportError::Input(format!(
            "{context} content identity is not lowercase b3-256"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::relational_model::{ModelRelation, ModelRowBuilder, ModelValue};

    use super::*;

    #[derive(Clone)]
    struct StaticReviewPort(IndependentMigrationReview);

    impl IndependentMigrationReviewPort for StaticReviewPort {
        fn load_review(&self) -> Result<IndependentMigrationReview, LegacyImportError> {
            Ok(self.0.clone())
        }
    }

    fn digest(byte: char) -> String {
        format!("b3:{}", byte.to_string().repeat(64))
    }

    fn decision(domain: SemanticReviewDomain, ordinal: usize) -> ImportedDecisionCandidate {
        let metamodel = BootstrapMetamodel::new();
        let decision_id = format!("decision.{ordinal}");
        let row = ModelRowBuilder::new(ModelRelation::SemanticType)
            .value("semantic_type_id", format!("semantic.type.{ordinal}"))
            .unwrap()
            .value("name", format!("semantic type {ordinal}"))
            .unwrap()
            .value("logical_type", "utf8")
            .unwrap()
            .value("allows_null", false)
            .unwrap()
            .build(&metamodel)
            .unwrap();
        ImportedDecisionCandidate::new(
            domain,
            ModelDecision::new(
                decision_id,
                "model-owner",
                "initial-model",
                "independently reviewed migration decision",
                vec![ModelOperation::Add(row)],
            )
            .unwrap(),
        )
    }

    fn evidence(id: &str, class: LegacyEvidenceClass, digest_char: char) -> LegacyEvidenceRow {
        LegacyEvidenceRow::new(
            id,
            class,
            "codefabric.legacy.fixture",
            format!("row:{id}"),
            digest(digest_char),
        )
        .unwrap()
    }

    fn disposition(
        id: &str,
        kind: LegacyDispositionKind,
        targets: &[&str],
    ) -> LegacyDispositionRow {
        LegacyDispositionRow::new(
            id,
            kind,
            targets.iter().copied(),
            "explicit test disposition",
        )
        .unwrap()
    }

    fn candidates() -> Vec<ImportedDecisionCandidate> {
        SemanticReviewDomain::ALL
            .into_iter()
            .enumerate()
            .map(|(index, domain)| decision(domain, index + 1))
            .collect()
    }

    fn review(
        candidates: &[ImportedDecisionCandidate],
        reviewer: &str,
    ) -> IndependentMigrationReview {
        IndependentMigrationReview::new(
            "review.initial-model.v1",
            reviewer,
            digest('f'),
            candidates
                .iter()
                .map(|candidate| {
                    IndependentDecisionExpectation::new(
                        candidate.domain,
                        candidate.decision.clone(),
                    )
                })
                .collect(),
        )
        .unwrap()
    }

    fn request() -> InitialMigrationRequest {
        InitialMigrationRequest::new(
            "legacy.batch.v1",
            "migration.initial.v1",
            "model.bootstrap.v1",
            "model.initial.v1",
        )
        .unwrap()
    }

    fn complete_fixture() -> (
        Vec<LegacyEvidenceRow>,
        Vec<ImportedDecisionCandidate>,
        Vec<LegacyDispositionRow>,
    ) {
        let candidates = candidates();
        let evidence = vec![
            evidence("legacy.type", LegacyEvidenceClass::ModelDecision, '0'),
            evidence(
                "legacy.authority.a",
                LegacyEvidenceClass::ModelDecision,
                '1',
            ),
            evidence(
                "legacy.authority.b",
                LegacyEvidenceClass::ModelDecision,
                '2',
            ),
            evidence(
                "legacy.normalization",
                LegacyEvidenceClass::RetainedSemanticMeaning,
                '3',
            ),
            evidence("legacy.query", LegacyEvidenceClass::ModelDecision, '4'),
            evidence("legacy.policy", LegacyEvidenceClass::ModelDecision, '5'),
            evidence("legacy.state", LegacyEvidenceClass::ModelDecision, '6'),
            evidence("legacy.proof", LegacyEvidenceClass::ReleasedPublicId, '7'),
            evidence(
                "legacy.false-static",
                LegacyEvidenceClass::ModelDecision,
                '8',
            ),
            evidence(
                "legacy.tombstone",
                LegacyEvidenceClass::TombstoneCommitment,
                '9',
            ),
            evidence(
                "legacy.superseded",
                LegacyEvidenceClass::AcceptedHistoricalDecision,
                'a',
            ),
        ];
        let dispositions = vec![
            disposition(
                "legacy.type",
                LegacyDispositionKind::Migrated,
                &["decision.1"],
            ),
            disposition(
                "legacy.authority.a",
                LegacyDispositionKind::Combined,
                &["decision.2"],
            ),
            disposition(
                "legacy.authority.b",
                LegacyDispositionKind::Combined,
                &["decision.2"],
            ),
            disposition(
                "legacy.normalization",
                LegacyDispositionKind::Split,
                &["decision.3", "decision.4"],
            ),
            disposition(
                "legacy.query",
                LegacyDispositionKind::Migrated,
                &["decision.5"],
            ),
            disposition(
                "legacy.policy",
                LegacyDispositionKind::Migrated,
                &["decision.6"],
            ),
            disposition(
                "legacy.state",
                LegacyDispositionKind::Migrated,
                &["decision.7"],
            ),
            disposition(
                "legacy.proof",
                LegacyDispositionKind::PreservedReleasedCommitment,
                &["decision.8"],
            ),
            disposition(
                "legacy.false-static",
                LegacyDispositionKind::RejectedFalseStatic,
                &[],
            ),
            disposition("legacy.tombstone", LegacyDispositionKind::Tombstoned, &[]),
            disposition("legacy.superseded", LegacyDispositionKind::Superseded, &[]),
        ];
        (evidence, candidates, dispositions)
    }

    #[test]
    fn importer_constructs_only_a_total_independently_reviewed_initial_migration() {
        let (evidence, candidates, dispositions) = complete_fixture();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let accepted = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .unwrap();

        assert_eq!(accepted.migration().migration_id(), "migration.initial.v1");
        assert_eq!(
            accepted.migration().target_model_epoch_id(),
            "model.initial.v1"
        );
        assert_eq!(accepted.report().legacy_input_count(), 11);
        assert_eq!(accepted.report().accepted_decision_ids().len(), 8);
        assert_eq!(accepted.report().dispositions().len(), 11);
        assert_eq!(accepted.report().reviewed_by(), "independent-review-owner");
        assert_eq!(accepted.report().import_batch_id(), "legacy.batch.v1");
        assert_eq!(accepted.report().review_id(), "review.initial-model.v1");
        assert_eq!(accepted.report().review_source_identity(), digest('f'));
        assert_eq!(
            accepted.report().dispositions()[0].legacy_row_id(),
            "legacy.authority.a"
        );
        assert_eq!(
            accepted.report().dispositions()[0].source_content_identity(),
            digest('1')
        );
    }

    #[test]
    fn importer_rejects_any_legacy_row_without_a_disposition() {
        let (evidence, candidates, mut dispositions) = complete_fixture();
        dispositions.pop();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("lack dispositions"));
    }

    #[test]
    fn importer_rejects_combined_disposition_with_only_one_source() {
        let (evidence, candidates, mut dispositions) = complete_fixture();
        dispositions
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.authority.b")
            .unwrap()
            .kind = LegacyDispositionKind::RejectedFalseStatic;
        dispositions
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.authority.b")
            .unwrap()
            .target_decision_ids = Vec::new();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("only one legacy source"));
    }

    #[test]
    fn importer_rejects_review_authored_by_the_importer() {
        let (evidence, candidates, dispositions) = complete_fixture();
        let review_port = StaticReviewPort(review(&candidates, "legacy-importer-v1"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("equals importer identity"));
    }

    #[test]
    fn importer_rejects_incomplete_semantic_review() {
        let (evidence, mut candidates, mut dispositions) = complete_fixture();
        candidates.pop();
        dispositions
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.proof")
            .unwrap()
            .kind = LegacyDispositionKind::Superseded;
        dispositions
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.proof")
            .unwrap()
            .target_decision_ids = Vec::new();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("semantic review does not cover"));
    }

    #[test]
    fn importer_rejects_a_review_that_differs_from_candidate_rows() {
        let (evidence, candidates, dispositions) = complete_fixture();
        let mut expectations = review(&candidates, "independent-review-owner");
        expectations.expectations[0].expected_decision = ModelDecision::new(
            "decision.1",
            "different-owner",
            "initial-model",
            "independently reviewed migration decision",
            candidates[0].decision.operations().to_vec(),
        )
        .unwrap();
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(
                request(),
                evidence,
                candidates,
                dispositions,
                &StaticReviewPort(expectations),
            )
            .err()
            .unwrap();
        assert!(error.to_string().contains("expectation differs"));
    }

    #[test]
    fn importer_rejects_duplicate_semantic_row_authority() {
        let (evidence, mut candidates, dispositions) = complete_fixture();
        let duplicate_row = match &candidates[0].decision.operations()[0] {
            ModelOperation::Add(row) => row.clone(),
            _ => unreachable!(),
        };
        candidates[1].decision = ModelDecision::new(
            "decision.2",
            "model-owner",
            "initial-model",
            "duplicate semantic authority fixture",
            vec![ModelOperation::Add(duplicate_row)],
        )
        .unwrap();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("claim the same model row"));
    }

    #[test]
    fn importer_rejects_duplicate_source_row_identity() {
        let (mut evidence, candidates, dispositions) = complete_fixture();
        evidence[1].source_artifact_id = evidence[0].source_artifact_id.clone();
        evidence[1].source_row_identity = evidence[0].source_row_identity.clone();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("duplicates a source artifact/row")
        );
    }

    #[test]
    fn importer_rejects_duplicate_row_authority_within_one_decision() {
        let (evidence, mut candidates, dispositions) = complete_fixture();
        let duplicate_row = match &candidates[0].decision.operations()[0] {
            ModelOperation::Add(row) => row.clone(),
            _ => unreachable!(),
        };
        candidates[0].decision = ModelDecision::new(
            "decision.1",
            "model-owner",
            "initial-model",
            "duplicate operation fixture",
            vec![
                ModelOperation::Add(duplicate_row.clone()),
                ModelOperation::Add(duplicate_row),
            ],
        )
        .unwrap();
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("more than once"));
    }

    #[test]
    fn preserved_commitment_requires_released_evidence() {
        let (mut evidence, candidates, dispositions) = complete_fixture();
        evidence
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.proof")
            .unwrap()
            .evidence_class = LegacyEvidenceClass::ModelDecision;
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("not a released commitment"));
    }

    #[test]
    fn accepted_historical_decision_is_not_preserved_target_authority() {
        let (mut evidence, candidates, dispositions) = complete_fixture();
        evidence
            .iter_mut()
            .find(|row| row.legacy_row_id == "legacy.proof")
            .unwrap()
            .evidence_class = LegacyEvidenceClass::AcceptedHistoricalDecision;
        let review_port = StaticReviewPort(review(&candidates, "independent-review-owner"));
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(request(), evidence, candidates, dispositions, &review_port)
            .err()
            .unwrap();
        assert!(error.to_string().contains("not a released commitment"));
    }

    #[test]
    fn review_source_must_not_reuse_legacy_evidence_identity() {
        let (evidence, candidates, dispositions) = complete_fixture();
        let mut independent_review = review(&candidates, "independent-review-owner");
        independent_review.review_source_identity = digest('0');
        let error = LegacyModelImporter::new("legacy-importer-v1")
            .unwrap()
            .import(
                request(),
                evidence,
                candidates,
                dispositions,
                &StaticReviewPort(independent_review),
            )
            .err()
            .unwrap();
        assert!(error.to_string().contains("reuses a legacy evidence"));
    }

    #[test]
    fn disposition_targets_are_exact_identifiers() {
        let row = disposition(
            "legacy.type",
            LegacyDispositionKind::Migrated,
            &["decision.1"],
        );
        assert_eq!(row.target_decision_ids(), &["decision.1"]);
        assert_eq!(row.kind(), LegacyDispositionKind::Migrated);
        assert_eq!(
            ModelValue::from("proof"),
            ModelValue::Utf8("proof".to_owned())
        );
    }
}
